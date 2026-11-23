//! Presets of plugins (`list_presets`, `load_preset`, `save_preset`) from three sources
//! (`crate::presets`): Surge XT's patch files, VST3 preset files and LV2 presets, which the
//! plugin's host lists with lilv. Loading one puts a state into the plugin as one undo step
//! (`Tools::put_plugin_state`); saving one writes a file of the person and changes nothing in
//! the song.

use std::sync::{Arc, Mutex};

use meloet_core::ai::{AiAnswer, AiAsk, AiRefusal, AiSong, PresetAsk, PresetSaid};
use meloet_model::{PluginPlace, PluginRef};
use rmcp::model::CallToolResult;
use rmcp::{
    ErrorData as McpError, handler::server::wrapper::Parameters, schemars, tool, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::mixer::BusArg;
use super::plugstate::{plugin_place, sha256_hex};
use super::{Tools, closing, done, done_value, failed, refusal_text};
use crate::presets::{Preset, PresetDirs, PresetError, inside, lv2, name_ok, vst3};
use crate::surge;

/// Presets a listing gives when `limit` is left out, and at most.
const DEFAULT_LIMIT: u32 = 50;
const MAX_LIMIT: u32 = 500;
/// Categories a listing names, at most.
const MAX_CATEGORIES: usize = 100;
/// Presets loaded lately that are remembered.
const MAX_LOADED: usize = 64;
const SURGE: &str = "surge";

/// What the preset tools keep for every connection: the folders, and the presets loaded
/// lately.
#[derive(Debug)]
pub(super) struct PresetSide {
    dirs: PresetDirs,
    loaded: Mutex<Vec<Loaded>>,
}

impl PresetSide {
    pub(super) fn new(dirs: PresetDirs) -> PresetSide {
        PresetSide {
            dirs,
            loaded: Mutex::new(Vec::new()),
        }
    }

    fn loaded(&self) -> std::sync::MutexGuard<'_, Vec<Loaded>> {
        self.loaded.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The preset `id` was loaded at `place` and the song has had the plugin's state since.
    fn still(&self, place: PluginPlace, key: &str, id: &str, sha256: &str) -> bool {
        (self.loaded().iter())
            .any(|l| l.place == place && &*l.key == key && l.id == id && l.sha256 == sha256)
    }

    fn remember(&self, loaded: Loaded) {
        let mut all = self.loaded();
        all.retain(|l| !(l.place == loaded.place && l.key == loaded.key));
        if all.len() == MAX_LOADED {
            all.remove(0);
        }
        all.push(loaded);
    }
}

/// A preset loaded, and the sha256 of the plugin's state the song took with it.
#[derive(Debug)]
struct Loaded {
    place: PluginPlace,
    key: Arc<str>,
    id: String,
    sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum KindArg {
    /// Shipped with the plugin.
    Factory,
    /// Written by the person or by meloet.
    User,
    All,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ListPresetsArgs {
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub track: Option<u32>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    /// The insert slot, 1 to 4. Leave out for the track's instrument plugin.
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    /// Only presets whose name holds this (any case).
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub name: Option<String>,
    /// Only presets whose category (folder or bank) holds this (any case).
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub category: Option<String>,
    /// "factory", "user" or "all" (default).
    #[serde(default)]
    pub kind: Option<KindArg>,
    /// How many to give, 1 to 500 (default 50).
    #[serde(default)]
    #[schemars(range(min = 1, max = 500))]
    pub limit: Option<u32>,
    /// How many to pass over first (default 0): `next_offset` of the last answer.
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct LoadPresetArgs {
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub track: Option<u32>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    /// The insert slot, 1 to 4. Leave out for the track's instrument plugin.
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    /// A preset's `id`, as list_presets gives it.
    #[schemars(length(min = 1, max = 8192))]
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SavePresetArgs {
    #[serde(default)]
    #[schemars(range(min = 1, max = 64))]
    pub track: Option<u32>,
    #[serde(default)]
    pub bus: Option<BusArg>,
    /// The insert slot, 1 to 4. Leave out for the track's instrument plugin.
    #[serde(default)]
    #[schemars(range(min = 1, max = 4))]
    pub slot: Option<u32>,
    /// 1 to 64 characters, with no / or \ and no control characters. Characters a file name
    /// does not take are written as _ in the file's name (the answer says which).
    #[schemars(length(min = 1, max = 64))]
    pub name: String,
    /// A category (folder) to keep it in, as `name`. Surge XT and VST3 presets only.
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub category: Option<String>,
    /// Write over a preset of the person of that name. Default false: that is an error.
    #[serde(default)]
    pub overwrite: bool,
}

/// One source's part of a listing.
struct Found {
    source: &'static str,
    presets: Vec<Preset>,
    note: Option<String>,
}

fn format_of(key: &str) -> &str {
    key.split_once(':').map_or("", |(format, _)| format)
}

fn preset_value(p: &Preset, source: &str) -> Value {
    json!({
        "id": p.id,
        "name": p.name,
        "category": p.category,
        "factory": p.factory,
        "source": source,
    })
}

fn source_of(id: &str) -> &str {
    id.split_once(':').map_or("", |(source, _)| source)
}

fn error_text(e: PresetError) -> String {
    match e {
        PresetError::NoSuch => {
            "there is no preset of that id for this plugin; call list_presets".into()
        }
        PresetError::Exists(path) => format!(
            "a preset is there already: {}; give `overwrite: true` to write over it",
            path.display()
        ),
        PresetError::Other(why) => why,
    }
}

/// The plugin at `place` in what was read.
fn plugin_at(read: &AiSong, place: PluginPlace) -> Result<Arc<PluginRef>, CallToolResult> {
    (read.song.doc.plugin(place).cloned())
        .ok_or_else(|| failed(refusal_text(&AiRefusal::NoSuchPlugin)))
}

/// Holds `pattern`, in any case.
fn holds(text: &str, pattern: &str) -> bool {
    text.to_lowercase().contains(&pattern.to_lowercase())
}

/// What a blocking piece of work gives, or "closing" when the pool lost it.
async fn off<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, CallToolResult> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|_| closing())
}

impl Tools {
    /// An ask about the presets of an LV2 plugin, answered by its plugin host.
    async fn host_preset(
        &self,
        place: PluginPlace,
        ask: PresetAsk,
    ) -> Result<PresetSaid, CallToolResult> {
        match self.answer(AiAsk::Preset { place, ask }).await? {
            AiAnswer::Preset(said) => Ok(said),
            AiAnswer::Refused(r) => Err(failed(refusal_text(&r))),
            _ => Err(closing()),
        }
    }

    /// The bundles of LV2 presets of `plugin`, besides its own.
    async fn lv2_bundles(
        &self,
        plugin: &Arc<PluginRef>,
    ) -> Result<Vec<std::path::PathBuf>, CallToolResult> {
        let (side, plugin) = (self.presets.clone(), plugin.clone());
        off(move || {
            let uri = plugin.key.strip_prefix("lv2:").unwrap_or_default();
            lv2::bundles_for(&side.dirs, uri, std::path::Path::new(&*plugin.path))
        })
        .await
    }

    /// Every preset of the plugin at `place`, by source.
    async fn all_presets(
        &self,
        place: PluginPlace,
        plugin: &Arc<PluginRef>,
    ) -> Result<Vec<Found>, CallToolResult> {
        let mut found = Vec::new();
        let p = plugin.clone();
        let surge = off(move || surge::presets::list(&p)).await?;
        let is_surge = surge.is_some();
        if let Some(presets) = surge {
            let note = presets
                .is_empty()
                .then(|| "Surge XT's patch folders hold no patches on this PC".to_owned());
            found.push(Found {
                source: SURGE,
                presets,
                note,
            });
        }
        match format_of(&plugin.key) {
            "vst3" => {
                let (side, p) = (self.presets.clone(), plugin.clone());
                let listed = off(move || vst3::list(&side.dirs, &p)).await?;
                let mut notes = Vec::new();
                if listed.presets.is_empty() {
                    let folders: Vec<String> = (vst3::folders(&self.presets.dirs, plugin)
                        .into_iter())
                    .map(|(f, _)| f.display().to_string())
                    .collect();
                    notes.push(format!(
                        "no .vstpreset files of this plugin in {}",
                        folders.join(", ")
                    ));
                }
                if listed.others > 0 {
                    notes.push(format!(
                        "{} files of other plugins passed over",
                        listed.others
                    ));
                }
                if listed.unreadable > 0 {
                    notes.push(format!("{} files that cannot be read", listed.unreadable));
                }
                if listed.capped {
                    notes.push("more files than are looked at; not all are listed".into());
                }
                if !is_surge {
                    notes.push("programs a VST3 plugin keeps inside itself are not listed".into());
                }
                found.push(Found {
                    source: vst3::SOURCE,
                    presets: listed.presets,
                    note: (!notes.is_empty()).then(|| notes.join("; ")),
                });
            }
            // Surge XT's LV2 lists one empty preset, which only chooses its program 0.
            "lv2" if !is_surge => {
                let bundles = self.lv2_bundles(plugin).await?;
                let PresetSaid::Listed(infos) =
                    self.host_preset(place, PresetAsk::List { bundles }).await?
                else {
                    return Err(failed("the plugin host answered something else"));
                };
                let dirs = &self.presets.dirs;
                let mut presets: Vec<Preset> =
                    infos.iter().map(|i| lv2::preset_of(dirs, i)).collect();
                let note = presets.is_empty().then(|| {
                    "this plugin lists no presets and none are saved for it yet".to_owned()
                });
                presets.sort_by(|a, b| {
                    (&a.category, &a.name, &a.id).cmp(&(&b.category, &b.name, &b.id))
                });
                found.push(Found {
                    source: lv2::SOURCE,
                    presets,
                    note,
                });
            }
            _ => {}
        }
        Ok(found)
    }

    /// The state to put in for the preset `id`.
    async fn preset_state(
        &self,
        place: PluginPlace,
        plugin: &Arc<PluginRef>,
        id: &str,
    ) -> Result<Vec<u8>, CallToolResult> {
        let id = id.to_owned();
        match source_of(&id) {
            SURGE => {
                let now = self.plugin_state(place).await?;
                let p = plugin.clone();
                off(move || surge::presets::load(&p, &id, &now.bytes))
                    .await?
                    .map_err(|e| failed(error_text(e)))
            }
            vst3::SOURCE => {
                let (side, p) = (self.presets.clone(), plugin.clone());
                off(move || vst3::load(&side.dirs, &p, &id))
                    .await?
                    .map_err(|e| failed(error_text(e)))
            }
            lv2::SOURCE if format_of(&plugin.key) == "lv2" => {
                let uri =
                    lv2::uri_of(&id).ok_or_else(|| failed(error_text(PresetError::NoSuch)))?;
                let bundles = self.lv2_bundles(plugin).await?;
                let ask = PresetAsk::State {
                    uri: uri.to_owned(),
                    bundles,
                };
                match self.host_preset(place, ask).await? {
                    PresetSaid::State(state) => Ok(state.to_vec()),
                    _ => Err(failed("the plugin host answered something else")),
                }
            }
            _ => Err(failed(error_text(PresetError::NoSuch))),
        }
    }

    /// The size and the sha256 of the state the song has for the plugin at `place`.
    async fn song_sha(
        &self,
        place: PluginPlace,
    ) -> Result<Option<(usize, String)>, CallToolResult> {
        let read = self.read().await?;
        let Some(plugin) = read.song.doc.plugin(place).cloned() else {
            return Ok(None);
        };
        off(move || Some((plugin.state.len(), sha256_hex(&plugin.state)))).await
    }

    /// Writes the plugin's sound now as the person's preset `name`.
    async fn save_as_preset(
        &self,
        place: PluginPlace,
        plugin: &Arc<PluginRef>,
        name: &str,
        category: Option<&str>,
        overwrite: bool,
    ) -> Result<Written, CallToolResult> {
        let (name, category) = (name.to_owned(), category.map(str::to_owned));
        if surge::is_surge(plugin) {
            let now = self.plugin_state(place).await?;
            let p = plugin.clone();
            let saved = off(move || {
                surge::presets::save(&p, &now.bytes, &name, category.as_deref(), overwrite)
            });
            let preset = saved.await?.map_err(|e| failed(error_text(e)))?;
            let path = (preset.id.strip_prefix("surge:"))
                .filter(|p| p.starts_with('/'))
                .map(str::to_owned);
            return Ok(Written {
                preset,
                source: SURGE,
                path,
                overwrote: None,
                replaced: Vec::new(),
                notes: Vec::new(),
            });
        }
        match format_of(&plugin.key) {
            "vst3" => {
                let now = self.plugin_state(place).await?;
                let (side, p) = (self.presets.clone(), plugin.clone());
                let saved = off(move || {
                    let category = category.as_deref();
                    vst3::save(&side.dirs, &p, &now.bytes, &name, category, overwrite)
                });
                let saved = saved.await?.map_err(|e| failed(error_text(e)))?;
                Ok(Written {
                    preset: saved.preset,
                    source: vst3::SOURCE,
                    path: Some(saved.path.display().to_string()),
                    overwrote: Some(saved.overwrote),
                    replaced: saved.replaced,
                    notes: Vec::new(),
                })
            }
            "lv2" => {
                self.save_lv2(place, plugin, name, category, overwrite)
                    .await
            }
            _ => Err(failed("meloet saves no presets of this plugin's format")),
        }
    }

    /// An LV2 preset of the person, written by the plugin's host as jalv writes them.
    async fn save_lv2(
        &self,
        place: PluginPlace,
        plugin: &Arc<PluginRef>,
        name: String,
        category: Option<String>,
        overwrite: bool,
    ) -> Result<Written, CallToolResult> {
        let dirs = &self.presets.dirs;
        let (Some((dir, file)), Some(root)) =
            (lv2::user_bundle(dirs, &plugin.name, &name), &dirs.lv2_user)
        else {
            return Err(failed("there is no home folder to keep LV2 presets in"));
        };
        let there = std::fs::symlink_metadata(&dir);
        match &there {
            Ok(meta) if !meta.is_dir() || !inside(root, &dir) => {
                return Err(failed(format!(
                    "{} is there and is not a folder of presets; nothing was written",
                    dir.display()
                )));
            }
            Ok(_) if !overwrite => return Err(failed(error_text(PresetError::Exists(dir)))),
            _ if !inside(root, &dir) => {
                return Err(failed(format!(
                    "{} has a symbolic link in its way; nothing was written",
                    dir.display()
                )));
            }
            _ => {}
        }
        let mut notes = Vec::new();
        if category.is_some() {
            notes
                .push("LV2 presets are saved here with no bank, so `category` was not used".into());
        }
        let mut replaced: Vec<char> = Vec::new();
        for c in name.chars().filter(|c| !c.is_ascii_alphanumeric()) {
            if !replaced.contains(&c) {
                replaced.push(c);
            }
        }
        let path = dir.join(&file);
        let ask = PresetAsk::Save {
            dir,
            file,
            label: name,
        };
        let PresetSaid::Saved(info) = self.host_preset(place, ask).await? else {
            return Err(failed("the plugin host answered something else"));
        };
        let path = info.file.clone().unwrap_or(path);
        Ok(Written {
            preset: lv2::preset_of(dirs, &info),
            source: lv2::SOURCE,
            path: Some(path.display().to_string()),
            overwrote: Some(there.is_ok()),
            replaced,
            notes,
        })
    }
}

/// A preset `save_preset` wrote.
struct Written {
    preset: Preset,
    source: &'static str,
    /// The file, when it is known.
    path: Option<String>,
    /// `None` when the source does not say.
    overwrote: Option<bool>,
    /// Characters of the name that are not in the file's name.
    replaced: Vec<char>,
    notes: Vec<String>,
}

#[tool_router(router = presets_router, vis = "pub(super)")]
impl Tools {
    /// The presets a plugin (a track's instrument, or an insert with `slot`) can load: Surge
    /// XT's patches, VST3 preset files, LV2 presets. Each has an id for load_preset, a name,
    /// a category (folder or bank) and whether it came with the plugin. Filter by name,
    /// category and kind; pages with limit and offset.
    #[tool(annotations(title = "List presets", read_only_hint = true, open_world_hint = false))]
    async fn list_presets(
        &self,
        Parameters(a): Parameters<ListPresetsArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let place = check!(plugin_place(&read, a.track, a.bus, a.slot));
        let plugin = check!(plugin_at(&read, place));
        let found = check!(self.all_presets(place, &plugin).await);
        let kind = a.kind.unwrap_or(KindArg::All);
        let matching: Vec<(&Preset, &str)> = (found.iter())
            .flat_map(|f| f.presets.iter().map(|p| (p, f.source)))
            .filter(|(p, _)| a.name.as_deref().is_none_or(|n| holds(&p.name, n)))
            .filter(|(p, _)| {
                (a.category.as_deref())
                    .is_none_or(|c| p.category.as_deref().is_some_and(|pc| holds(pc, c)))
            })
            .filter(|(p, _)| match kind {
                KindArg::Factory => p.factory,
                KindArg::User => !p.factory,
                KindArg::All => true,
            })
            .collect();
        let total = matching.len();
        let limit = a.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT) as usize;
        let offset = (a.offset.unwrap_or(0) as usize).min(total);
        let page: Vec<Value> = (matching.iter().skip(offset).take(limit))
            .map(|(p, source)| preset_value(p, source))
            .collect();
        let count = page.len();
        let next_offset = (offset + count < total).then_some(offset + count);
        let mut categories: std::collections::BTreeMap<Option<&str>, usize> = Default::default();
        for (p, _) in &matching {
            *categories.entry(p.category.as_deref()).or_default() += 1;
        }
        let categories_total = categories.len();
        let categories: Vec<Value> = (categories.into_iter().take(MAX_CATEGORIES))
            .map(|(name, count)| json!({ "name": name, "count": count }))
            .collect();
        let sources: Vec<Value> = (found.iter())
            .map(|f| json!({ "source": f.source, "found": f.presets.len(), "note": f.note }))
            .collect();
        let format = format_of(&plugin.key);
        let value = json!({
            "plugin": {
                "name": &*plugin.name,
                "vendor": &*plugin.vendor,
                "key": &*plugin.key,
                "format": format,
            },
            "total": total,
            "offset": offset,
            "count": count,
            "next_offset": next_offset,
            "categories": categories,
            "categories_total": categories_total,
            "presets": page,
            "sources": sources,
        });
        let mut lines = vec![match (total, next_offset) {
            (0, _) => format!("{} ({format}): no presets match.", plugin.name),
            (_, Some(next)) => format!(
                "{} ({format}): {total} presets match; {count} from {offset} shown, the next from \
                 {next}.",
                plugin.name
            ),
            (_, None) => format!(
                "{} ({format}): {total} presets match; {count} from {offset} shown.",
                plugin.name
            ),
        }];
        if found.is_empty() {
            lines.push("meloet reads no presets of this plugin's format.".into());
        }
        for f in &found {
            if let Some(note) = &f.note {
                lines.push(format!("{}: {note}.", f.source));
            }
        }
        lines.push(value.to_string());
        Ok(done(lines.join("\n"), value))
    }

    /// Loads a preset (its id from list_presets) into the plugin, replacing its sound, as one
    /// undo step that the song keeps when saved. The preset loaded already is no change.
    #[tool(annotations(
        title = "Load preset",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn load_preset(
        &self,
        Parameters(a): Parameters<LoadPresetArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let id = a.id.trim().to_owned();
        let read = check!(self.read().await);
        let place = check!(plugin_place(&read, a.track, a.bus, a.slot));
        let plugin = check!(plugin_at(&read, place));
        let preset = json!({ "id": id, "source": source_of(&id) });
        let p = plugin.clone();
        let sha = check!(off(move || sha256_hex(&p.state)).await);
        if self.presets.still(place, &plugin.key, &id, &sha) {
            let value = json!({
                "changed": false,
                "step": null,
                "undo_steps": read.undo_count,
                "preset": preset,
            });
            return Ok(done(
                format!(
                    "No change: that preset is loaded already, and the plugin has not changed \
                     since.\n{value}"
                ),
                value,
            ));
        }
        let state = check!(self.preset_state(place, &plugin, &id).await);
        let (mut value, text) = match self.put_plugin_state(place, state.into()).await {
            Ok(d) => {
                let value = done_value(&d);
                let text = format!(
                    "Done: loaded the preset into {}, as one undo step.",
                    plugin.name
                );
                (value, text)
            }
            // The song has that state already.
            Err(result) if result.is_error != Some(true) => {
                let value = result.structured_content.unwrap_or_else(|| json!({}));
                (
                    value,
                    "No change: the plugin has that preset's state already.".into(),
                )
            }
            Err(result) => return Ok(result),
        };
        if let Some((bytes, sha256)) = check!(self.song_sha(place).await) {
            value["now"] = json!({ "bytes": bytes, "sha256": sha256 });
            self.presets.remember(Loaded {
                place,
                key: plugin.key.clone(),
                id,
                sha256,
            });
        }
        value["preset"] = preset;
        Ok(done(format!("{text}\n{value}"), value))
    }

    /// Saves the plugin's sound now as a preset of the person, named `name`: Surge XT's
    /// patches, a VST3 preset file, or an LV2 preset (as jalv saves them). A preset of that
    /// name is written over only with `overwrite`. Not a change of the song.
    #[tool(annotations(
        title = "Save preset",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn save_preset(
        &self,
        Parameters(a): Parameters<SavePresetArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let name = check!(name_ok("name", &a.name).map_err(failed));
        let category = match a.category.as_deref() {
            Some(c) => Some(check!(name_ok("category", c).map_err(failed))),
            None => None,
        };
        let read = check!(self.read().await);
        let place = check!(plugin_place(&read, a.track, a.bus, a.slot));
        let plugin = check!(plugin_at(&read, place));
        let category = category.as_deref();
        let written =
            check!((self.save_as_preset(place, &plugin, &name, category, a.overwrite)).await);
        let Written {
            preset,
            source,
            path,
            overwrote,
            replaced,
            mut notes,
        } = written;
        if !replaced.is_empty() {
            let shown: Vec<String> = replaced.iter().map(|c| format!("{c:?}")).collect();
            notes.push(format!(
                "these characters of the name are _ in the file's name: {}",
                shown.join(" ")
            ));
        }
        let value = json!({
            "preset": preset_value(&preset, source),
            "written": path,
            "overwrote": overwrote,
            "replaced_characters": replaced.iter().map(char::to_string).collect::<Vec<_>>(),
            "notes": notes,
        });
        let mut lines = vec![match &path {
            Some(path) => format!(
                "Saved the preset \"{}\" of {} as {path}.",
                preset.name, plugin.name
            ),
            None => format!("Saved the preset \"{}\" of {}.", preset.name, plugin.name),
        }];
        lines.extend(notes.iter().map(|n| format!("{n}.")));
        lines.push(value.to_string());
        Ok(done(lines.join("\n"), value))
    }
}

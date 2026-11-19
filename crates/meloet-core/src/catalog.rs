//! 「あとから入れたもの」 as the rules know it: the SF2 files that were added, the scanned
//! plugins merged into the rows of the shelf, the format each row uses, and the stars.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use meloet_model::{InstrumentGroup, InstrumentRef, Kept, PluginRef, Sf2Ref};
use meloet_plugin::scan::EditorKind;
use meloet_plugin::{
    EffectCategory, InstrumentCategory, MergeKey, PluginFormat as Format, PluginInfo, PluginKind,
    ScanCache,
};
use meloet_protocol::{
    EffectRef, LibraryView, PluginEffectCategory, PluginFormat, PluginFormatView,
    PluginInstrumentCategory, PluginKindView, PluginRowView, PluginWindowKind, Sf2FileView,
    Sf2PresetView, SoundRef,
};

use crate::added::family;
use crate::settings::ShelfSettings;
use crate::sf2store::{Sf2Info, Sf2Preset};

/// One format of a row.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    format: Format,
    key: String,
    name: String,
    vendor: String,
    kind: PluginKind,
    bundle: PathBuf,
    /// meloet can open its window.
    window: bool,
    window_kind: PluginWindowKind,
}

/// One plugin in every format it was found in, VST3 before LV2.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    name: String,
    vendor: String,
    kind: PluginKind,
    entries: Vec<Entry>,
}

impl Row {
    fn has(&self, key: &str) -> bool {
        self.entries.iter().any(|e| e.key == key)
    }

    /// The format the row uses: the one the person chose, else one the song uses already,
    /// else the first whose window meloet opens, else the first (VST3 before LV2).
    fn used(&self, chosen: &[String], song: &[&str]) -> &Entry {
        let of = |key: &str| self.entries.iter().find(|e| e.key == key);
        (chosen.iter().find_map(|key| of(key)))
            .or_else(|| song.iter().find_map(|key| of(key)))
            .or_else(|| self.entries.iter().find(|e| e.window))
            .unwrap_or(&self.entries[0])
    }
}

/// A plugin of the shelf by one of its keys, as a song takes it in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Found {
    pub plugin: Arc<PluginRef>,
    pub instrument: bool,
    /// The group whose phrase ▶ plays (spec 3 Y14).
    pub phrase: InstrumentGroup,
}

#[derive(Debug, Default)]
pub(crate) struct Catalog {
    sf2: Vec<Sf2Info>,
    opened_sf2: Option<String>,
    rows: Vec<Row>,
    shelf: ShelfSettings,
}

fn format_view(format: Format) -> PluginFormat {
    match format {
        Format::Vst3 => PluginFormat::Vst3,
        Format::Lv2 => PluginFormat::Lv2,
    }
}

fn kind_view(kind: PluginKind) -> PluginKindView {
    use EffectCategory as E;
    use InstrumentCategory as I;
    use PluginEffectCategory as PE;
    use PluginInstrumentCategory as PI;
    match kind {
        PluginKind::Instrument(c) => PluginKindView::Instrument(match c {
            I::Synth => PI::Synth,
            I::Sampler => PI::Sampler,
            I::Drum => PI::Drum,
            I::Keys => PI::Keys,
            I::Other => PI::Other,
        }),
        PluginKind::Effect(c) => PluginKindView::Effect(match c {
            E::Eq => PE::Eq,
            E::Dynamics => PE::Dynamics,
            E::Reverb => PE::Reverb,
            E::Delay => PE::Delay,
            E::Modulation => PE::Modulation,
            E::Distortion => PE::Distortion,
            E::Filter => PE::Filter,
            E::Pitch => PE::Pitch,
            E::Spatial => PE::Spatial,
            E::Analysis => PE::Analysis,
            E::Other => PE::Other,
        }),
    }
}

fn is_other(kind: PluginKind) -> bool {
    matches!(
        kind,
        PluginKind::Instrument(InstrumentCategory::Other)
            | PluginKind::Effect(EffectCategory::Other)
    )
}

fn entry(bundle: &std::path::Path, info: &PluginInfo) -> Entry {
    Entry {
        format: info.key.format(),
        key: info.key.to_string(),
        name: info.name.clone(),
        vendor: info.vendor.clone(),
        kind: info.kind,
        bundle: bundle.to_path_buf(),
        window: info.editor == EditorKind::X11,
        window_kind: match info.editor {
            EditorKind::X11 => PluginWindowKind::Opens,
            EditorKind::Unsupported => PluginWindowKind::NotYet,
            EditorKind::None => PluginWindowKind::None,
        },
    }
}

impl Catalog {
    pub fn new(shelf: ShelfSettings) -> Catalog {
        Catalog {
            shelf,
            ..Catalog::default()
        }
    }

    pub fn settings(&self) -> &ShelfSettings {
        &self.shelf
    }

    // ---- SF2 ----

    /// The files that were there at the start.
    pub fn set_sf2(&mut self, files: Vec<Sf2Info>) {
        self.sf2 = files;
        self.sf2.sort_by(|a, b| a.file.cmp(&b.file));
    }

    /// A file was added, in place of one of the same name: its heading is the unfolded one.
    pub fn sf2_added(&mut self, info: Sf2Info) {
        self.opened_sf2 = Some(info.file.clone());
        self.sf2.retain(|f| f.file != info.file);
        self.sf2.push(info);
        self.sf2.sort_by(|a, b| a.file.cmp(&b.file));
    }

    pub fn sf2_preset(&self, file: &str, bank: u16, program: u8) -> Option<(&Sf2Info, &Sf2Preset)> {
        let info = self.sf2.iter().find(|f| f.file == file)?;
        Some((info, info.preset(bank, program)?))
    }

    /// The preset does not read: it leaves the shelf.
    pub fn sf2_unreadable(&mut self, file: &str, bank: u16, program: u8) {
        for f in self.sf2.iter_mut().filter(|f| f.file == file) {
            f.presets.retain(|p| (p.bank, p.program) != (bank, program));
        }
    }

    // ---- plugins ----

    /// Takes in what the scan knows now: the rows are made anew.
    pub fn set_plugins(&mut self, cache: &ScanCache) {
        let mut merged: BTreeMap<MergeKey, Vec<Entry>> = BTreeMap::new();
        for (bundle, info) in cache.plugins() {
            let entries = merged
                .entry(MergeKey::new(info.kind.role(), &info.name, &info.vendor))
                .or_default();
            // The same plugin found in two places is one.
            if !entries.iter().any(|e| e.key == info.key.to_string()) {
                entries.push(entry(&bundle.path, info));
            }
        }
        let place = |format: Format| Format::PREFERENCE.iter().position(|f| *f == format);
        self.rows = merged
            .into_values()
            .map(|mut entries| {
                entries.sort_by(|a, b| (place(a.format), &a.key).cmp(&(place(b.format), &b.key)));
                let first = &entries[0];
                // Of the kinds the formats say, the first that says something
                // (design-notes A-16).
                let kind = (entries.iter().map(|e| e.kind))
                    .find(|k| !is_other(*k))
                    .unwrap_or(first.kind);
                Row {
                    name: first.name.clone(),
                    vendor: first.vendor.clone(),
                    kind,
                    entries,
                }
            })
            .collect();
        self.rows.sort_by(|a, b| {
            (a.name.to_lowercase(), a.vendor.to_lowercase())
                .cmp(&(b.name.to_lowercase(), b.vendor.to_lowercase()))
                .then_with(|| a.entries[0].key.cmp(&b.entries[0].key))
        });
    }

    fn row_of(&self, key: &str) -> Option<&Row> {
        self.rows.iter().find(|row| row.has(key))
    }

    /// Every plugin by its exact key, with the bundle it is loaded from and what the scan
    /// says of its window.
    pub fn bundles(&self) -> Vec<(Arc<str>, PathBuf, PluginWindowKind)> {
        (self.rows.iter())
            .flat_map(|row| &row.entries)
            .map(|e| (Arc::from(e.key.as_str()), e.bundle.clone(), e.window_kind))
            .collect()
    }

    /// The plugin of `key` as it is put into a song from the shelf.
    pub fn plugin(&self, key: &str) -> Option<Found> {
        let row = self.row_of(key)?;
        let e = row.entries.iter().find(|e| e.key == key)?;
        let path = e.bundle.to_string_lossy();
        Some(Found {
            plugin: Arc::new(PluginRef::new(&e.key, &e.name, &e.vendor, &path)),
            instrument: matches!(row.kind, PluginKind::Instrument(_)),
            phrase: match row.kind {
                PluginKind::Instrument(InstrumentCategory::Drum) => InstrumentGroup::Drums,
                PluginKind::Instrument(InstrumentCategory::Keys) => InstrumentGroup::PianoKeys,
                _ => InstrumentGroup::Synth,
            },
        })
    }

    /// A sound of 「あとから入れたもの」 as a track takes it; `None` when the shelf does not
    /// have it, or it is no instrument.
    pub fn instrument(&self, sound: &SoundRef) -> Option<(InstrumentRef, InstrumentGroup)> {
        match sound {
            SoundRef::Builtin { .. } => None,
            SoundRef::Sf2 {
                file,
                bank,
                program,
            } => {
                let (info, preset) = self.sf2_preset(file, *bank, *program)?;
                let made = InstrumentRef::Sf2(Arc::new(Sf2Ref {
                    file: info.file.as_str().into(),
                    sha256: info.sha256.as_str().into(),
                    bank: preset.bank,
                    program: preset.program,
                    name: preset.name.as_str().into(),
                    kept: Kept::NONE,
                }));
                let group = made.group();
                Some((made, group))
            }
            SoundRef::Plugin { key } => {
                let found = self.plugin(key).filter(|f| f.instrument)?;
                Some((InstrumentRef::Plugin(found.plugin), found.phrase))
            }
        }
    }

    /// `UsePluginFormat`. Returns whether the setting changed.
    pub fn use_format(&mut self, key: &str) -> bool {
        let Some(row) = self.rows.iter().find(|row| row.has(key)) else {
            return false;
        };
        let chosen = &mut self.shelf.plugin_formats;
        if chosen.iter().any(|k| k == key) {
            return false;
        }
        chosen.retain(|k| !row.has(k));
        chosen.push(key.to_owned());
        true
    }

    // ---- stars ----

    /// `ToggleFavoriteSound` for an SF2 preset or a plugin. A plugin is starred as a row,
    /// whatever format the row uses. Returns whether the stars changed: what the shelf does
    /// not have gets none.
    pub fn toggle_sound(&mut self, sound: SoundRef) -> bool {
        let there = self.instrument(&sound).is_some();
        let stars = &mut self.shelf.favorite_sounds;
        let before = stars.len();
        match &sound {
            SoundRef::Plugin { key } => {
                let row = self.rows.iter().find(|row| row.has(key));
                stars.retain(|s| match s {
                    SoundRef::Plugin { key: k } => k != key && !row.is_some_and(|r| r.has(k)),
                    _ => true,
                });
            }
            other => stars.retain(|s| s != other),
        }
        if stars.len() == before && there {
            stars.push(sound);
        }
        stars.len() != before
    }

    /// `ToggleFavoriteEffect`. Returns whether the stars changed.
    pub fn toggle_effect(&mut self, effect: EffectRef) -> bool {
        let there = match &effect {
            EffectRef::Builtin { .. } => true,
            EffectRef::Plugin { key } => self.plugin(key).is_some_and(|f| !f.instrument),
        };
        let stars = &mut self.shelf.favorite_effects;
        let before = stars.len();
        match &effect {
            EffectRef::Plugin { key } => {
                let row = self.rows.iter().find(|row| row.has(key));
                stars.retain(|s| match s {
                    EffectRef::Plugin { key: k } => k != key && !row.is_some_and(|r| r.has(k)),
                    EffectRef::Builtin { .. } => true,
                });
            }
            other => stars.retain(|s| s != other),
        }
        if stars.len() == before && there {
            stars.push(effect);
        }
        stars.len() != before
    }

    /// The key the row of `key` uses now; `key` itself when no row has it.
    fn shown_key(&self, key: &str, song: &[&str]) -> String {
        self.row_of(key).map_or_else(
            || key.to_owned(),
            |row| row.used(&self.shelf.plugin_formats, song).key.clone(),
        )
    }

    /// The starred SF2 presets and plugins, a plugin by the key its row uses now.
    pub fn favorite_sounds(&self, song: &[&str]) -> Vec<SoundRef> {
        (self.shelf.favorite_sounds.iter())
            .map(|s| match s {
                SoundRef::Plugin { key } => SoundRef::Plugin {
                    key: self.shown_key(key, song),
                },
                other => other.clone(),
            })
            .collect()
    }

    pub fn favorite_effects(&self, song: &[&str]) -> Vec<EffectRef> {
        (self.shelf.favorite_effects.iter())
            .map(|e| match e {
                EffectRef::Plugin { key } => EffectRef::Plugin {
                    key: self.shown_key(key, song),
                },
                other => other.clone(),
            })
            .collect()
    }

    /// `song`: the keys of the plugins the song holds.
    pub fn view(&self, song: &[&str], pack_missing: bool) -> LibraryView {
        LibraryView {
            sf2: (self.sf2.iter())
                .map(|f| Sf2FileView {
                    file: f.file.clone(),
                    presets: (f.presets.iter())
                        .map(|p| Sf2PresetView {
                            bank: p.bank,
                            program: p.program,
                            name: p.name.clone(),
                            family: family(p.family()),
                        })
                        .collect(),
                })
                .collect(),
            plugins: (self.rows.iter())
                .map(|row| PluginRowView {
                    name: row.name.clone(),
                    vendor: row.vendor.clone(),
                    kind: kind_view(row.kind),
                    key: row.used(&self.shelf.plugin_formats, song).key.clone(),
                    formats: (row.entries.iter())
                        .map(|e| PluginFormatView {
                            format: format_view(e.format),
                            key: e.key.clone(),
                        })
                        .collect(),
                })
                .collect(),
            opened_sf2: self.opened_sf2.clone(),
            pack_missing,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;

//! Surge XT's patch files as presets: the factory and third-party patches, and the
//! person's own. All of these read and write files: call them on the blocking pool.
//!
//! A patch's category is its folder (Surge XT's browser shows folders, not the category the
//! patch names inside). Ids are `surge:factory/<path>`, `surge:3rdparty/<path>` and
//! `surge:user/<path>`, the path of the `.fxp` file under its root.

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use meloet_model::PluginRef;

use super::{Format, format_of, fxp, patch, state};
use crate::presets::{Preset, PresetError};

/// The largest patch file read.
const MOST_FILE: u64 = 32 << 20;
/// Files a listing looks at, at most, and folders it goes into.
const MOST_FILES: usize = 100_000;
const MOST_DEPTH: usize = 8;
/// Characters of a patch name or of one folder of a category.
const MOST_NAME: usize = 64;
const PREFIX: &str = "surge:";
const THIRD_PARTY: &str = "3rd party";

/// Where the patches are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roots {
    /// `patches_factory` of Surge XT's data.
    pub factory: PathBuf,
    /// `patches_3rdparty` of Surge XT's data.
    pub third_party: PathBuf,
    /// The person's patches (`Patches` of Surge XT's user folder).
    pub user: PathBuf,
}

impl Roots {
    /// As Surge XT 1.3.4 finds them on Linux (as its sources and sst-plugininfra's say): its
    /// data in
    /// `$SURGE_DATA_HOME` (or `$PIPELINE_OVERRIDE_DATA_HOME`), else the first folder of
    /// `<data home>/Surge XT`, `<data home>/surge-xt`, `/usr/share/surge-xt`; the person's
    /// patches in `Patches` of the folder `userDataPath` of Surge XT's own settings, else of
    /// `<documents>/Surge XT`. Surge XT's own search beside its binary (`SurgeXTData`) is
    /// not followed.
    pub fn from_env() -> Roots {
        let var = |name: &str| {
            std::env::var_os(name)
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
        };
        let home = var("HOME").unwrap_or_else(|| PathBuf::from("/"));
        let data = var("SURGE_DATA_HOME")
            .or_else(|| var("PIPELINE_OVERRIDE_DATA_HOME"))
            .or_else(|| {
                let share = var("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"));
                [share.join("Surge XT"), share.join("surge-xt")]
                    .into_iter()
                    .find(|d| d.is_dir())
            })
            .unwrap_or_else(|| PathBuf::from("/usr/share/surge-xt"));
        let config = var("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"));
        let dirs = std::fs::read_to_string(config.join("user-dirs.dirs")).ok();
        let default = user_data(&home, dirs.as_deref());
        let settings = std::fs::read_to_string(default.join("SurgeXTUserDefaults.xml")).ok();
        let user = settings
            .as_deref()
            .and_then(user_data_path)
            .unwrap_or(default);
        Roots {
            factory: data.join("patches_factory"),
            third_party: data.join("patches_3rdparty"),
            user: user.join("Patches"),
        }
    }
}

/// Surge XT's own folder for the person, before its settings may name another: in the
/// documents folder of `user-dirs.dirs`, else `~/.Surge XT` or `~/Documents/Surge XT` when
/// there, else in `~/Documents` when that is there, else `~/.Surge XT`.
fn user_data(home: &Path, dirs: Option<&str>) -> PathBuf {
    const NAME: &str = "Surge XT";
    if let Some(docs) = dirs.and_then(|text| documents_of(text, home)) {
        return docs.join(NAME);
    }
    let hidden = home.join(format!(".{NAME}"));
    let documents = home.join("Documents");
    if hidden.is_dir() {
        hidden
    } else if documents.join(NAME).is_dir() || documents.is_dir() {
        documents.join(NAME)
    } else {
        hidden
    }
}

/// `XDG_DOCUMENTS_DIR="…"` of a `user-dirs.dirs` file, `$HOME` at its start replaced.
fn documents_of(text: &str, home: &Path) -> Option<PathBuf> {
    let line = text
        .lines()
        .map(|l| l.trim_start_matches([' ', '\t']))
        .find_map(|l| l.strip_prefix("XDG_DOCUMENTS_DIR"))?;
    let value = line.trim_start_matches([' ', '\t']).strip_prefix('=')?;
    let value = value.trim_start_matches([' ', '\t']).strip_prefix('"')?;
    let value = &value[..value.find('"')?];
    let path = match value.strip_prefix("$HOME") {
        Some(rest) => PathBuf::from(format!("{}{rest}", home.display())),
        None => PathBuf::from(value),
    };
    (!value.is_empty() && path.is_absolute()).then_some(path)
}

/// The folder `<default key="userDataPath" value="…" type="1"/>` of Surge XT's settings
/// (`<defaults version="1">`) names.
fn user_data_path(text: &str) -> Option<PathBuf> {
    let root = super::xml::parse(text).ok()?;
    if root.name(text) != "defaults" || root.value(text, "version").as_deref() != Some("1") {
        return None;
    }
    let entry = (root.children.iter()).find(|e| {
        e.name(text) == "default" && e.value(text, "key").as_deref() == Some("userDataPath")
    })?;
    if entry.value(text, "type").as_deref() != Some("1") {
        return None;
    }
    let path = PathBuf::from(&*entry.value(text, "value")?);
    path.is_absolute().then_some(path)
}

/// Roots that tests put in place of [`Roots::from_env`] for the whole process.
static ROOTS: RwLock<Option<Roots>> = RwLock::new(None);

/// Puts `roots` in place of the ones Surge XT would use (`None`: those again). For tests,
/// which must not touch the person's folders.
pub fn set_roots(roots: Option<Roots>) {
    *ROOTS.write().unwrap_or_else(|e| e.into_inner()) = roots;
}

fn roots() -> Roots {
    (ROOTS.read().unwrap_or_else(|e| e.into_inner()).clone()).unwrap_or_else(Roots::from_env)
}

/// The patches Surge XT would list, ids starting with `surge:`. `None`: the plugin is not
/// Surge XT.
pub fn list(plugin: &PluginRef) -> Option<Vec<Preset>> {
    list_in(&roots(), plugin)
}

/// The state to put into the plugin to load the patch `id`: `state` is the plugin's state
/// now (meloet's container of its format), which the answer keeps but for the patch.
pub fn load(plugin: &PluginRef, id: &str, state: &[u8]) -> Result<Vec<u8>, PresetError> {
    load_in(&roots(), plugin, id, state)
}

/// Writes the patch inside `state` as a patch of the person, named `name` in the folder
/// `category` (`None`: Surge XT's folder for patches without one). An existing file only
/// with `overwrite`.
pub fn save(
    plugin: &PluginRef,
    state: &[u8],
    name: &str,
    category: Option<&str>,
    overwrite: bool,
) -> Result<Preset, PresetError> {
    save_in(&roots(), plugin, state, name, category, overwrite)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Root {
    Factory,
    ThirdParty,
    User,
}

impl Root {
    const ALL: [Root; 3] = [Root::Factory, Root::ThirdParty, Root::User];

    fn tag(self) -> &'static str {
        match self {
            Root::Factory => "factory",
            Root::ThirdParty => "3rdparty",
            Root::User => "user",
        }
    }

    fn path(self, roots: &Roots) -> &Path {
        match self {
            Root::Factory => &roots.factory,
            Root::ThirdParty => &roots.third_party,
            Root::User => &roots.user,
        }
    }
}

fn is_fxp(name: &str) -> bool {
    name.len() > 4
        && (name.get(name.len() - 4..)).is_some_and(|end| end.eq_ignore_ascii_case(".fxp"))
}

pub fn list_in(roots: &Roots, plugin: &PluginRef) -> Option<Vec<Preset>> {
    format_of(plugin).ok()?;
    let mut found = Vec::new();
    for root in Root::ALL {
        let mut files = Vec::new();
        walk(root.path(roots), Path::new(""), 0, &mut files);
        files.sort();
        for rel in files {
            // Surge XT takes no loose files in its own folders.
            if root != Root::User && rel.parent() == Some(Path::new("")) {
                continue;
            }
            let Some(text) = rel.to_str() else { continue };
            let name = text.rsplit('/').next().unwrap_or(text);
            let folder = rel
                .parent()
                .and_then(Path::to_str)
                .filter(|f| !f.is_empty());
            let category = match root {
                Root::ThirdParty => folder.map(|f| format!("{THIRD_PARTY}/{f}")),
                _ => folder.map(str::to_owned),
            };
            found.push(Preset {
                id: format!("{PREFIX}{}/{text}", root.tag()),
                name: name[..name.len() - 4].to_owned(),
                category,
                factory: root != Root::User,
            });
        }
    }
    Some(found)
}

/// The `.fxp` files under `dir` (not following links), as paths under the root.
fn walk(dir: &Path, rel: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= MOST_FILES {
            return;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name();
        let Some(text) = name.to_str() else { continue };
        if text.starts_with('.') {
            continue;
        }
        if kind.is_dir() && depth < MOST_DEPTH {
            walk(&entry.path(), &rel.join(text), depth + 1, out);
        } else if kind.is_file() && is_fxp(text) {
            out.push(rel.join(text));
        }
    }
}

/// The file of a patch id, inside its root (no links out of it), or why not.
fn file_of(roots: &Roots, id: &str) -> Result<(Root, PathBuf), PresetError> {
    let rest = id.strip_prefix(PREFIX).ok_or(PresetError::NoSuch)?;
    let (tag, text) = rest.split_once('/').ok_or(PresetError::NoSuch)?;
    let root = (Root::ALL.into_iter())
        .find(|r| r.tag() == tag)
        .ok_or(PresetError::NoSuch)?;
    let rel = Path::new(text);
    // `Path` passes over "." and doubled separators: the text itself must have none.
    let plain = text
        .split('/')
        .all(|c| !c.is_empty() && c != "." && c != "..");
    if !plain || text.contains('\0') || !is_fxp(text) {
        return Err(PresetError::NoSuch);
    }
    let base = root.path(roots);
    let (Ok(base), Ok(file)) = (base.canonicalize(), base.join(rel).canonicalize()) else {
        return Err(PresetError::NoSuch);
    };
    if !file.starts_with(&base) || !file.is_file() {
        return Err(PresetError::NoSuch);
    }
    Ok((root, file))
}

pub fn load_in(
    roots: &Roots,
    plugin: &PluginRef,
    id: &str,
    state: &[u8],
) -> Result<Vec<u8>, PresetError> {
    let format = format_of(plugin).map_err(PresetError::Other)?;
    let (_, file) = file_of(roots, id)?;
    let other = |e: String| PresetError::Other(format!("{}: {e}", file.display()));
    let size = std::fs::metadata(&file)
        .map_err(|e| other(e.to_string()))?
        .len();
    if size > MOST_FILE {
        return Err(other("too large for a patch".into()));
    }
    let bytes = std::fs::read(&file).map_err(|e| other(e.to_string()))?;
    let chunk = fxp::chunk_of(&bytes).map_err(other)?;
    // Surge XT reads its old patches whose XML is not UTF-8 as they are; only their shape is
    // checked here.
    let xml = String::from_utf8_lossy(super::chunk::parse(chunk).map_err(other)?.xml);
    patch::Patch::parse(&xml).map_err(other)?;
    state::with_chunk(format, state, chunk).map_err(PresetError::Other)
}

/// A name of a file or a folder the person may give: no separators, no control
/// characters, not hidden.
fn name_ok(what: &str, name: &str) -> Result<(), PresetError> {
    let count = name.chars().count();
    if count == 0
        || count > MOST_NAME
        || name.trim() != name
        || name.starts_with('.')
        || name.contains(['/', '\\'])
        || name.chars().any(char::is_control)
    {
        return Err(PresetError::Other(format!(
            "a {what} is 1 to {MOST_NAME} characters with no \"/\", \"\\\" or control \
             characters, no spaces at its ends, and does not start with \".\""
        )));
    }
    Ok(())
}

pub fn save_in(
    roots: &Roots,
    plugin: &PluginRef,
    state: &[u8],
    name: &str,
    category: Option<&str>,
    overwrite: bool,
) -> Result<Preset, PresetError> {
    let format = format_of(plugin).map_err(PresetError::Other)?;
    let name = match name.len() > 4 && is_fxp(name) {
        true => &name[..name.len() - 4],
        false => name,
    };
    name_ok("patch name", name)?;
    let folders: Vec<&str> = match category {
        Some(c) => c.split('/').collect(),
        None => Vec::new(),
    };
    if folders.len() > MOST_DEPTH {
        return Err(PresetError::Other(format!(
            "a category is at most {MOST_DEPTH} folders"
        )));
    }
    for folder in &folders {
        name_ok("folder of a category", folder)?;
    }
    let bytes = patch_file(format, state, name)?;
    let other = |e: std::io::Error| PresetError::Other(format!("the patch was not saved ({e})"));
    let folder = folders.iter().fold(roots.user.clone(), |p, f| p.join(f));
    std::fs::create_dir_all(&folder).map_err(other)?;
    let base = roots.user.canonicalize().map_err(other)?;
    if !folder.canonicalize().map_err(other)?.starts_with(&base) {
        return Err(PresetError::Other(
            "that folder leads out of the folder of the person's patches".into(),
        ));
    }
    let file = folder.join(format!("{name}.fxp"));
    match std::fs::symlink_metadata(&file) {
        Ok(meta) if !meta.is_file() => {
            return Err(PresetError::Other(format!(
                "{} is there and is not a file",
                file.display()
            )));
        }
        Ok(_) if !overwrite => return Err(PresetError::Exists(file)),
        _ => {}
    }
    write_new(&folder, &file, &bytes, overwrite).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => PresetError::Exists(file.clone()),
        _ => other(e),
    })?;
    let rel: Vec<&str> = folders.iter().copied().chain([name]).collect();
    Ok(Preset {
        id: format!("{PREFIX}{}/{}.fxp", Root::User.tag(), rel.join("/")),
        name: name.to_owned(),
        category: category.map(str::to_owned),
        factory: false,
    })
}

/// The `.fxp` file of the patch in `state`, named `name` inside too.
fn patch_file(format: Format, state: &[u8], name: &str) -> Result<Vec<u8>, PresetError> {
    let bytes = state::chunk_of(format, state).map_err(PresetError::Other)?;
    let chunk = super::chunk::parse(&bytes).map_err(PresetError::Other)?;
    let text = chunk.text().map_err(PresetError::Other)?;
    let xml = patch::set_name(text, name).map_err(PresetError::Other)?;
    let new = super::chunk::with_xml(&chunk, &xml).map_err(PresetError::Other)?;
    fxp::file_of(&new, name).map_err(PresetError::Other)
}

/// Writes `bytes` to a new file beside `file`, then puts it in `file`'s place: over it with
/// `overwrite`, else only when `file` is not there.
fn write_new(folder: &Path, file: &Path, bytes: &[u8], overwrite: bool) -> std::io::Result<()> {
    let temp = folder.join(format!(
        ".meloet-{}-{}.fxp.part",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let written = (|| {
        let mut out = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&temp)?;
        out.write_all(bytes)?;
        out.sync_all()?;
        match overwrite {
            true => std::fs::rename(&temp, file),
            false => std::fs::hard_link(&temp, file),
        }
    })();
    if written.is_err() || !overwrite {
        let _ = std::fs::remove_file(&temp);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::super::chunk::tests::made;
    use super::super::state::tests::{lv2_state, vst3_state};
    use super::*;

    fn surge(format: &str) -> PluginRef {
        let key = match format {
            "vst3" => super::super::VST3_KEY.to_owned(),
            _ => format!("lv2:{}", super::super::LV2_URI),
        };
        PluginRef::new(&key, "Surge XT", "Surge Synth Team", "")
    }

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("k11surge-presets-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    const XML: &str = "<?xml version=\"1.0\" ?><patch revision=\"24\"><meta name=\"Init Saw\" \
        category=\"Templates\" /><parameters><a_pitch type=\"2\" value=\"0.0\" /></parameters></patch>";

    fn roots(dir: &Path) -> Roots {
        let roots = Roots {
            factory: dir.join("factory"),
            third_party: dir.join("third"),
            user: dir.join("user/Surge XT/Patches"),
        };
        for (at, xml) in [
            (
                "factory/Leads/Sine Lead.fxp",
                "<patch><meta name=\"Sinlead\"/><parameters/></patch>",
            ),
            ("factory/Loose.fxp", XML),
            ("factory/Leads/Notes.txt", XML),
            ("third/Vospi/Plucks/Popcorn.FXP", XML),
            ("factory/Pads/.hidden.fxp", XML),
        ] {
            let path = dir.join(at);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, fxp::file_of(&made(xml, true), "x").unwrap()).unwrap();
        }
        std::fs::write(dir.join("factory/Leads/Broken.fxp"), b"CcnK....").unwrap();
        std::fs::write(
            dir.join("outside.fxp"),
            fxp::file_of(&made(XML, false), "x").unwrap(),
        )
        .unwrap();
        std::os::unix::fs::symlink(dir.join("outside.fxp"), dir.join("factory/Leads/Link.fxp"))
            .unwrap();
        std::os::unix::fs::symlink(dir, dir.join("factory/Loop")).unwrap();
        roots
    }

    #[test]
    fn patches_are_listed_by_their_folders() {
        let dir = temp("list");
        let roots = roots(&dir);
        let list = list_in(&roots, &surge("vst3")).unwrap();
        let ids: Vec<&str> = list.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "surge:factory/Leads/Broken.fxp",
                "surge:factory/Leads/Sine Lead.fxp",
                "surge:3rdparty/Vospi/Plucks/Popcorn.FXP",
            ]
        );
        assert_eq!(list[1].name, "Sine Lead");
        assert_eq!(list[1].category.as_deref(), Some("Leads"));
        assert!(list[1].factory);
        assert_eq!(list[2].category.as_deref(), Some("3rd party/Vospi/Plucks"));
        let fake = PluginRef::new("lv2:urn:meloet:fake:sine", "Sine", "meloet", "");
        assert_eq!(list_in(&roots, &fake), None);
    }

    #[test]
    fn a_patch_is_loaded_into_the_state_of_either_format() {
        let dir = temp("load");
        let roots = roots(&dir);
        let now = made(XML, false);
        for (format, state) in [("vst3", vst3_state(&now)), ("lv2", lv2_state(&now, 1))] {
            let plugin = surge(format);
            let id = "surge:factory/Leads/Sine Lead.fxp";
            let put = load_in(&roots, &plugin, id, &state).unwrap();
            let f = Format::of_key(&plugin.key).unwrap();
            let chunk = state::chunk_of(f, &put).unwrap();
            assert!(
                super::super::chunk::parse(&chunk)
                    .unwrap()
                    .text()
                    .unwrap()
                    .contains("Sinlead")
            );
            assert_eq!(
                chunk.len(),
                made("<patch><meta name=\"Sinlead\"/><parameters/></patch>", true).len()
            );
            for bad in [
                "surge:factory/Leads/Nope.fxp",
                "surge:factory/../outside.fxp",
                "surge:factory/Leads/Link.fxp",
                "surge:factory/Loop/outside.fxp",
                "surge:factory//Leads/Sine Lead.fxp",
                "surge:factory/Leads/Notes.txt",
                "surge:other/Leads/Sine Lead.fxp",
                "factory/Leads/Sine Lead.fxp",
                "surge:factory/",
                "surge:factory/Leads/./Sine Lead.fxp",
            ] {
                assert_eq!(
                    load_in(&roots, &plugin, bad, &state),
                    Err(PresetError::NoSuch),
                    "{bad}"
                );
            }
            let broken = load_in(&roots, &plugin, "surge:factory/Leads/Broken.fxp", &state);
            assert!(matches!(broken, Err(PresetError::Other(_))), "{broken:?}");
        }
    }

    #[test]
    fn a_patch_is_saved_as_the_persons_and_loads_back() {
        let dir = temp("save");
        let roots = roots(&dir);
        let plugin = surge("lv2");
        let state = lv2_state(&made(XML, true), 0);
        let saved = save_in(
            &roots,
            &plugin,
            &state,
            "My Vibrato",
            Some("Leads/Mine"),
            false,
        )
        .unwrap();
        assert_eq!(saved.id, "surge:user/Leads/Mine/My Vibrato.fxp");
        assert!(roots.user.join("Leads/Mine/My Vibrato.fxp").is_file());
        let loose = save_in(&roots, &plugin, &state, "Loose.fxp", None, false).unwrap();
        assert_eq!(loose.id, "surge:user/Loose.fxp");
        assert_eq!(loose.category, None);
        let again = save_in(
            &roots,
            &plugin,
            &state,
            "My Vibrato",
            Some("Leads/Mine"),
            false,
        );
        assert!(matches!(again, Err(PresetError::Exists(_))), "{again:?}");
        save_in(
            &roots,
            &plugin,
            &state,
            "My Vibrato",
            Some("Leads/Mine"),
            true,
        )
        .unwrap();
        let listed = list_in(&roots, &plugin).unwrap();
        let mine: Vec<&Preset> = listed.iter().filter(|p| !p.factory).collect();
        assert_eq!(mine.len(), 2);
        assert_eq!(mine[0].category.as_deref(), Some("Leads/Mine"));
        let put = load_in(&roots, &plugin, &saved.id, &state).unwrap();
        let back = state::chunk_of(Format::Lv2, &put).unwrap();
        let xml = super::super::chunk::parse(&back)
            .unwrap()
            .text()
            .unwrap()
            .to_owned();
        assert_eq!(xml, XML.replace("Init Saw", "My Vibrato"));
        // No file is left half-written beside them.
        let names: Vec<_> = std::fs::read_dir(roots.user.join("Leads/Mine"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(names.len(), 1, "{names:?}");
        for (name, category) in [
            ("a/b", None),
            ("", None),
            (".hidden", None),
            (" x", None),
            ("x\n", None),
            ("ok", Some("..")),
            ("ok", Some("a//b")),
            ("ok", Some("/abs")),
        ] {
            let refused = save_in(&roots, &plugin, &state, name, category, false);
            assert!(
                matches!(refused, Err(PresetError::Other(_))),
                "{name:?} {category:?}"
            );
        }
        // A folder that leads out through a link.
        std::os::unix::fs::symlink(&dir, roots.user.join("Out")).unwrap();
        let out = save_in(&roots, &plugin, &state, "x", Some("Out"), false);
        assert!(matches!(out, Err(PresetError::Other(_))), "{out:?}");
    }

    /// Surge XT's own patches of this PC: listed fast, and every one is read.
    /// `cargo test -p meloet-ai --lib real_factory -- --ignored --nocapture`
    #[test]
    #[ignore = "needs Surge XT's patches in /usr/share/surge-xt"]
    fn real_factory_patches_list_fast_and_all_read() {
        let dir = temp("real");
        let roots = Roots {
            user: dir.join("Patches"),
            ..Roots::from_env()
        };
        let plugin = surge("vst3");
        let started = std::time::Instant::now();
        let list = list_in(&roots, &plugin).unwrap();
        let listed = started.elapsed();
        let (factory, third) = list.iter().fold((0, 0), |(f, t), p| {
            match p.id.starts_with("surge:factory/") {
                true => (f + 1, t),
                false => (f, t + 1),
            }
        });
        eprintln!(
            "{} patches ({factory} factory, {third} third-party) listed in {listed:?}",
            list.len()
        );
        let state = vst3_state(&made(XML, false));
        let started = std::time::Instant::now();
        let mut revisions = std::collections::BTreeMap::new();
        for p in &list {
            let put = load_in(&roots, &plugin, &p.id, &state)
                .unwrap_or_else(|e| panic!("{}: {e:?}", p.id));
            let chunk = state::chunk_of(Format::Vst3, &put).unwrap();
            let xml = String::from_utf8_lossy(super::super::chunk::parse(&chunk).unwrap().xml)
                .into_owned();
            let patch = patch::Patch::parse(&xml).unwrap();
            *revisions.entry(patch.meta().revision).or_insert(0) += 1;
        }
        eprintln!(
            "all loaded and read in {:?}; revisions {revisions:?}",
            started.elapsed()
        );
        assert!(list.len() > 3000, "{}", list.len());
        assert!(listed < std::time::Duration::from_secs(1), "{listed:?}");
    }

    #[test]
    fn the_persons_folder_is_found_as_surge_xt_finds_it() {
        let home = Path::new("/home/x");
        let text =
            "# c\nXDG_DESKTOP_DIR=\"$HOME/Desktop\"\n  XDG_DOCUMENTS_DIR=\"$HOME/ドキュメント\"\n";
        assert_eq!(
            documents_of(text, home),
            Some(PathBuf::from("/home/x/ドキュメント"))
        );
        assert_eq!(
            user_data(home, Some(text)),
            PathBuf::from("/home/x/ドキュメント/Surge XT")
        );
        assert_eq!(
            documents_of("XDG_DOCUMENTS_DIR=\"/data/docs\"", home),
            Some(PathBuf::from("/data/docs"))
        );
        for bad in [
            "XDG_DOCUMENTS_DIR=docs",
            "XDG_DOCUMENTS_DIR=\"docs\"",
            "XDG_DOCUMENTS_DIR=\"\"",
            "",
        ] {
            assert_eq!(documents_of(bad, home), None, "{bad}");
        }
        let dir = temp("home");
        std::fs::create_dir_all(dir.join("Documents")).unwrap();
        assert_eq!(user_data(&dir, None), dir.join("Documents/Surge XT"));
        std::fs::create_dir_all(dir.join(".Surge XT")).unwrap();
        assert_eq!(user_data(&dir, None), dir.join(".Surge XT"));
        let settings = "<?xml version=\"1.0\" ?><defaults version=\"1\"><default key=\"lastPatchPath\" \
            value=\"/a\" type=\"1\"/><default key=\"userDataPath\" value=\"/mnt/Surge Data\" type=\"1\"/></defaults>";
        assert_eq!(
            user_data_path(settings),
            Some(PathBuf::from("/mnt/Surge Data"))
        );
        assert_eq!(
            user_data_path(&settings.replace("version=\"1\"", "version=\"2\"")),
            None
        );
        assert_eq!(
            user_data_path(&settings.replace("value=\"/mnt", "value=\"mnt")),
            None
        );
        assert_eq!(user_data_path("<defaults version=\"1\"/>"), None);
    }
}

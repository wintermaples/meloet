//! VST3 preset files (`.vstpreset`; docs/research/2026-11-23-plugin-presets-state.md Q8): the
//! person's in `$HOME/.vst3/presets/<vendor>/<plugin>/`, and those that came with plugins in
//! the same place under `/usr/share/vst3/presets` and `/usr/local/share/vst3/presets`. A
//! folder below the plugin's is a category. These read and write files: call them on the
//! blocking pool.
//!
//! A file is `VST3`, the version 1 (i32), the plugin's class id as 32 hex digits, where the
//! chunk list is (i64), the chunks, and then the list: `List`, a count (i32), and each chunk's
//! id (4 bytes), place and size (i64 each); little endian. `Comp` is the component's state
//! and `Cont` the controller's: the two halves of meloet's VST3 state (`MV3S`, the plugin
//! host's `formats/vst3.rs`).

use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use meloet_model::PluginRef;

use super::{Preset, PresetDirs, PresetError, inside};

/// Ids of these presets start with this and a colon; the rest is the file's path.
pub const SOURCE: &str = "vstpreset";
const ENDING: &str = "vstpreset";
const MAGIC: &[u8; 4] = b"VST3";
const VERSION: i32 = 1;
const HEADER: usize = 48;
const LIST: &[u8; 4] = b"List";
const COMPONENT: &[u8; 4] = b"Comp";
const CONTROLLER: &[u8; 4] = b"Cont";
/// meloet's VST3 state: this, the length of the component's state (u64), the component's
/// state, the controller's.
const STATE_MAGIC: &[u8; 4] = b"MV3S";
/// Files looked at in one listing, at most.
const MAX_FILES: usize = 5000;
/// Folders below the plugin's looked into.
const MAX_DEPTH: usize = 4;
/// The largest preset file read.
const MAX_FILE: u64 = 64 << 20;

/// What a `.vstpreset` holds.
#[derive(Debug, PartialEq, Eq)]
pub struct Parsed<'a> {
    pub class_id: &'a str,
    pub component: &'a [u8],
    pub controller: &'a [u8],
}

fn i64_at(bytes: &[u8], at: usize) -> Option<u64> {
    let raw: [u8; 8] = bytes.get(at..at + 8)?.try_into().ok()?;
    u64::try_from(i64::from_le_bytes(raw)).ok()
}

fn i32_at(bytes: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// The class id of a file's first bytes, when they are a `.vstpreset`'s.
fn class_id(head: &[u8]) -> Option<&str> {
    if head.get(..4)? != MAGIC || i32_at(head, 4)? != VERSION {
        return None;
    }
    let id = std::str::from_utf8(head.get(8..40)?).ok()?;
    id.bytes().all(|b| b.is_ascii_hexdigit()).then_some(id)
}

pub fn parse(bytes: &[u8]) -> Result<Parsed<'_>, String> {
    let bad = |why: &str| format!("not a VST3 preset file ({why})");
    let class_id = class_id(bytes).ok_or_else(|| bad("no VST3 header"))?;
    let list = (i64_at(bytes, 40))
        .and_then(|at| usize::try_from(at).ok())
        .filter(|at| *at >= HEADER && at + 8 <= bytes.len())
        .ok_or_else(|| bad("no chunk list"))?;
    if &bytes[list..list + 4] != LIST {
        return Err(bad("no chunk list"));
    }
    let count = (i32_at(bytes, list + 4))
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| list + 8 + n * 20 <= bytes.len())
        .ok_or_else(|| bad("a cut chunk list"))?;
    let mut component = None;
    let mut controller: &[u8] = &[];
    for entry in 0..count {
        let at = list + 8 + entry * 20;
        let (place, size) = (i64_at(bytes, at + 4), i64_at(bytes, at + 12));
        let chunk = (place.zip(size))
            .and_then(|(place, size)| {
                let place = usize::try_from(place).ok()?;
                bytes.get(place..place.checked_add(usize::try_from(size).ok()?)?)
            })
            .ok_or_else(|| bad("a chunk out of the file"))?;
        match &bytes[at..at + 4] {
            id if id == COMPONENT => component = Some(chunk),
            id if id == CONTROLLER => controller = chunk,
            _ => {}
        }
    }
    Ok(Parsed {
        class_id,
        component: component.ok_or_else(|| bad("no component state"))?,
        controller,
    })
}

/// A `.vstpreset` of the two states; the controller's only when it has one.
pub fn build(class_id: &str, component: &[u8], controller: &[u8]) -> Vec<u8> {
    let list = HEADER + component.len() + controller.len();
    let mut out = Vec::with_capacity(list + 48);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend(class_id.bytes().map(|b| b.to_ascii_uppercase()));
    out.extend_from_slice(&(list as i64).to_le_bytes());
    out.extend_from_slice(component);
    out.extend_from_slice(controller);
    out.extend_from_slice(LIST);
    let chunks = if controller.is_empty() { 1i32 } else { 2 };
    out.extend_from_slice(&chunks.to_le_bytes());
    let mut chunk = |id: &[u8; 4], place: usize, size: usize| {
        out.extend_from_slice(id);
        out.extend_from_slice(&(place as i64).to_le_bytes());
        out.extend_from_slice(&(size as i64).to_le_bytes());
    };
    chunk(COMPONENT, HEADER, component.len());
    if !controller.is_empty() {
        chunk(CONTROLLER, HEADER + component.len(), controller.len());
    }
    out
}

/// The class id of a plugin's key `vst3:<32 hex digits>`.
pub fn class_of(key: &str) -> Option<&str> {
    let id = key.strip_prefix("vst3:")?;
    (id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit())).then_some(id)
}

/// Whether a file's class id names the class `ours`: the same digits, or the first three
/// fields byte-swapped, as Windows (COM) builds may have written it.
pub fn same_class(ours: &str, theirs: &str) -> bool {
    if ours.eq_ignore_ascii_case(theirs) {
        return true;
    }
    const SWAPPED: [usize; 16] = [3, 2, 1, 0, 5, 4, 7, 6, 8, 9, 10, 11, 12, 13, 14, 15];
    let ours = ours.as_bytes();
    ours.len() == 32
        && theirs.len() == 32
        && (SWAPPED.iter().enumerate()).all(|(at, from)| {
            ours[from * 2..from * 2 + 2]
                .eq_ignore_ascii_case(&theirs.as_bytes()[at * 2..at * 2 + 2])
        })
}

/// meloet's state of the two halves.
fn container(component: &[u8], controller: &[u8]) -> Vec<u8> {
    let mut state = Vec::with_capacity(12 + component.len() + controller.len());
    state.extend_from_slice(STATE_MAGIC);
    state.extend_from_slice(&(component.len() as u64).to_le_bytes());
    state.extend_from_slice(component);
    state.extend_from_slice(controller);
    state
}

/// The two halves of meloet's state.
fn halves(state: &[u8]) -> Result<(&[u8], &[u8]), String> {
    let bad = || "not a VST3 state of meloet".to_string();
    let rest = state.strip_prefix(STATE_MAGIC).ok_or_else(bad)?;
    let (len, rest) = rest.split_first_chunk::<8>().ok_or_else(bad)?;
    let len = usize::try_from(u64::from_le_bytes(*len)).map_err(|_| bad())?;
    if len > rest.len() {
        return Err(bad());
    }
    Ok(rest.split_at(len))
}

/// A name as a folder or file name: `\*?/:<>|` become `_`, as the VST3 SDK asks, and so does
/// a dot at the start. The characters replaced are given too.
pub fn file_name(name: &str) -> (String, Vec<char>) {
    let mut replaced = Vec::new();
    let text = (name.chars().enumerate())
        .map(|(at, c)| {
            if "\\*?/:<>|".contains(c) || c.is_control() || (at == 0 && c == '.') {
                if !replaced.contains(&c) {
                    replaced.push(c);
                }
                '_'
            } else {
                c
            }
        })
        .collect();
    (text, replaced)
}

/// The plugin's folder under a folder of presets.
fn plugin_folder(root: &Path, plugin: &PluginRef) -> PathBuf {
    let mut folder = root.to_path_buf();
    if !plugin.vendor.trim().is_empty() {
        folder.push(file_name(plugin.vendor.trim()).0);
    }
    folder.push(file_name(plugin.name.trim()).0);
    folder
}

/// The plugin's folders, and whether each is the person's.
pub fn folders(dirs: &PresetDirs, plugin: &PluginRef) -> Vec<(PathBuf, bool)> {
    let user = (dirs.vst3_user.iter()).map(|root| (plugin_folder(root, plugin), true));
    let shared = (dirs.vst3_shared.iter()).map(|root| (plugin_folder(root, plugin), false));
    user.chain(shared).collect()
}

/// What a listing found.
#[derive(Debug, Default)]
pub struct Listed {
    pub presets: Vec<Preset>,
    /// The folders looked in that are there.
    pub looked_in: Vec<PathBuf>,
    /// Preset files of other plugins, passed over.
    pub others: usize,
    pub unreadable: usize,
    /// More files were there than are looked at.
    pub capped: bool,
}

fn head_of(path: &Path) -> Option<[u8; HEADER]> {
    let mut head = [0; HEADER];
    std::fs::File::open(path).ok()?.read_exact(&mut head).ok()?;
    Some(head)
}

/// The `.vstpreset` files of the plugin, sorted by category and name. Symbolic links below a
/// plugin's folder are not followed.
pub fn list(dirs: &PresetDirs, plugin: &PluginRef) -> Listed {
    let mut listed = Listed::default();
    let Some(ours) = class_of(&plugin.key) else {
        return listed;
    };
    let mut files = 0;
    for (folder, user) in folders(dirs, plugin) {
        if !folder.is_dir() {
            continue;
        }
        listed.looked_in.push(folder.clone());
        let mut stack = vec![(folder.clone(), 0)];
        while let Some((dir, depth)) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                let path = entry.path();
                if kind.is_dir() && depth < MAX_DEPTH {
                    stack.push((path, depth + 1));
                    continue;
                }
                if !kind.is_file() || path.extension().is_none_or(|e| e != ENDING) {
                    continue;
                }
                if files == MAX_FILES {
                    listed.capped = true;
                    continue;
                }
                files += 1;
                let head = head_of(&path);
                match head.as_ref().and_then(|head| class_id(head)) {
                    Some(id) if same_class(ours, id) => {}
                    Some(_) => {
                        listed.others += 1;
                        continue;
                    }
                    None => {
                        listed.unreadable += 1;
                        continue;
                    }
                }
                let category = (dir.strip_prefix(&folder).ok())
                    .map(|rest| rest.to_string_lossy().into_owned())
                    .filter(|rest| !rest.is_empty());
                listed.presets.push(Preset {
                    id: format!("{SOURCE}:{}", path.display()),
                    name: (path.file_stem().unwrap_or_default().to_string_lossy()).into(),
                    category,
                    factory: !user,
                });
            }
        }
    }
    (listed.presets)
        .sort_by(|a, b| (&a.category, &a.name, &a.id).cmp(&(&b.category, &b.name, &b.id)));
    listed
}

/// The file of the preset `id` of the plugin, when it is one of the plugin's folders.
fn file_of(dirs: &PresetDirs, plugin: &PluginRef, id: &str) -> Option<PathBuf> {
    let path = PathBuf::from(id.strip_prefix(SOURCE)?.strip_prefix(':')?);
    if !path.is_absolute() || path.extension().is_none_or(|e| e != ENDING) {
        return None;
    }
    let ok = (folders(dirs, plugin).iter()).any(|(folder, _)| inside(folder, &path));
    ok.then_some(path)
}

/// meloet's state of the preset `id`.
pub fn load(dirs: &PresetDirs, plugin: &PluginRef, id: &str) -> Result<Vec<u8>, PresetError> {
    let ours = class_of(&plugin.key).ok_or(PresetError::NoSuch)?;
    let path = file_of(dirs, plugin, id).ok_or(PresetError::NoSuch)?;
    let meta = std::fs::symlink_metadata(&path).map_err(|_| PresetError::NoSuch)?;
    if !meta.is_file() {
        return Err(PresetError::NoSuch);
    }
    if meta.len() > MAX_FILE {
        return Err(PresetError::Other(format!(
            "{} is larger than {} MiB",
            path.display(),
            MAX_FILE >> 20
        )));
    }
    let bytes = std::fs::read(&path)
        .map_err(|e| PresetError::Other(format!("{} cannot be read ({e})", path.display())))?;
    let parsed =
        parse(&bytes).map_err(|why| PresetError::Other(format!("{}: {why}", path.display())))?;
    if !same_class(ours, parsed.class_id) {
        return Err(PresetError::Other(format!(
            "{} is a preset of another plugin (class {})",
            path.display(),
            parsed.class_id
        )));
    }
    Ok(container(parsed.component, parsed.controller))
}

/// A preset written.
#[derive(Debug)]
pub struct Saved {
    pub preset: Preset,
    pub path: PathBuf,
    pub overwrote: bool,
    /// Characters of the name or the category that are not in the file's name.
    pub replaced: Vec<char>,
}

/// Writes `bytes` as `path`: a new file, or with `overwrite` in place of the file there,
/// whole or not at all.
fn write(path: &Path, bytes: &[u8], overwrite: bool) -> std::io::Result<()> {
    if !overwrite {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(path)?;
        file.write_all(bytes)?;
        return file.sync_all();
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temp = path.with_file_name(format!(".{name}.{}.part", std::process::id()));
    let written = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written
}

/// Writes meloet's state `state` of the plugin as the person's preset `name` (in the folder
/// `category`). A file of that name is written over only with `overwrite`.
pub fn save(
    dirs: &PresetDirs,
    plugin: &PluginRef,
    state: &[u8],
    name: &str,
    category: Option<&str>,
    overwrite: bool,
) -> Result<Saved, PresetError> {
    let other = PresetError::Other;
    let ours = class_of(&plugin.key).ok_or_else(|| other("not a VST3 plugin".into()))?;
    let root = (dirs.vst3_user.as_ref())
        .ok_or_else(|| other("there is no home folder to keep presets in".into()))?;
    let (component, controller) = halves(state).map_err(other)?;
    let (file, mut replaced) = file_name(name);
    let mut folder = plugin_folder(root, plugin);
    if let Some(category) = category {
        let (category, more) = file_name(category);
        for c in more {
            if !replaced.contains(&c) {
                replaced.push(c);
            }
        }
        folder.push(category);
    }
    let path = folder.join(format!("{file}.{ENDING}"));
    if !inside(root, &path) {
        return Err(other(format!(
            "{} has a symbolic link in its way; nothing was written",
            path.display()
        )));
    }
    let overwrote = match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.is_file() && overwrite => true,
        Ok(meta) if meta.is_file() => return Err(PresetError::Exists(path)),
        Ok(_) => {
            return Err(other(format!(
                "{} is there and is not a file; nothing was written",
                path.display()
            )));
        }
        Err(_) => false,
    };
    std::fs::create_dir_all(&folder)
        .map_err(|e| other(format!("{} cannot be made ({e})", folder.display())))?;
    let bytes = build(ours, component, controller);
    write(&path, &bytes, overwrote)
        .map_err(|e| other(format!("{} cannot be written ({e})", path.display())))?;
    let category = category.map(|c| file_name(c).0);
    Ok(Saved {
        preset: Preset {
            id: format!("{SOURCE}:{}", path.display()),
            name: file,
            category,
            factory: false,
        },
        path,
        overwrote,
        replaced,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CID: &str = "5A616D436F6D70000000000000000000";

    fn plugin() -> PluginRef {
        PluginRef::new(&format!("vst3:{CID}"), "ZamComp", "Damien Zammit", "")
    }

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("k11presets-vst3-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_file_reads_back_as_it_was_built() {
        let built = build(CID, b"component", b"controller");
        assert_eq!(&built[..4], b"VST3");
        assert_eq!(&built[8..40], CID.as_bytes());
        let parsed = parse(&built).unwrap();
        assert_eq!(parsed.class_id, CID);
        assert_eq!(parsed.component, b"component");
        assert_eq!(parsed.controller, b"controller");
        // With no controller state there is no `Cont` chunk.
        let built = build(&CID.to_lowercase(), b"c", b"");
        assert_eq!(&built[8..40], CID.as_bytes());
        assert_eq!(i32_at(&built, HEADER + 1 + 4), Some(1));
        assert_eq!(parse(&built).unwrap().controller, b"");
    }

    #[test]
    fn a_file_of_the_sdk_layout_is_read() {
        // As the SDK's PresetFile writes one: chunks, an `Info` chunk last, and the list.
        let mut file = b"VST3".to_vec();
        file.extend(1i32.to_le_bytes());
        file.extend(CID.as_bytes());
        file.extend(0i64.to_le_bytes());
        let comp = (file.len(), b"COMP".len());
        file.extend(b"COMP");
        let cont = (file.len(), b"CONT!".len());
        file.extend(b"CONT!");
        let info = (file.len(), b"<xml/>".len());
        file.extend(b"<xml/>");
        let list = file.len();
        file[40..48].copy_from_slice(&(list as i64).to_le_bytes());
        file.extend(b"List");
        file.extend(3i32.to_le_bytes());
        for (id, (at, len)) in [(b"Comp", comp), (b"Cont", cont), (b"Info", info)] {
            file.extend(id);
            file.extend((at as i64).to_le_bytes());
            file.extend((len as i64).to_le_bytes());
        }
        let parsed = parse(&file).unwrap();
        assert_eq!(
            (parsed.component, parsed.controller),
            (&b"COMP"[..], &b"CONT!"[..])
        );
    }

    #[test]
    fn broken_files_are_errors() {
        let good = build(CID, b"component", b"controller");
        assert!(parse(b"VST3").is_err());
        assert!(parse(&good[..good.len() - 1]).is_err());
        let mut far = good.clone();
        far[40..48].copy_from_slice(&(1i64 << 40).to_le_bytes());
        assert!(parse(&far).is_err());
        let mut chunk_out = good.clone();
        let at = good.len() - 40 + 12;
        chunk_out[at..at + 8].copy_from_slice(&(1i64 << 40).to_le_bytes());
        assert!(parse(&chunk_out).is_err());
        let mut no_comp = good;
        let at = no_comp.len() - 40;
        no_comp[at..at + 4].copy_from_slice(b"Prog");
        assert!(parse(&no_comp).is_err());
    }

    #[test]
    fn a_class_written_by_windows_is_the_same_class() {
        assert!(same_class(CID, &CID.to_lowercase()));
        // The first three fields byte-swapped.
        assert!(same_class(
            "0123456789ABCDEF0011223344556677",
            "67452301AB89EFCD0011223344556677"
        ));
        assert!(!same_class(CID, "5A616D436F6D70000000000000000001"));
        assert!(!same_class(CID, "short"));
    }

    #[test]
    fn names_lose_what_file_names_do_not_take() {
        assert_eq!(file_name("Soft Pad"), ("Soft Pad".into(), vec![]));
        assert_eq!(file_name("a:b*c?"), ("a_b_c_".into(), vec![':', '*', '?']));
        assert_eq!(file_name(".hidden"), ("_hidden".into(), vec!['.']));
    }

    #[test]
    fn a_preset_saved_lists_and_loads_back_and_is_not_written_over_unasked() {
        let home = temp("save");
        let dirs = PresetDirs::home(&home);
        let shared = home.join("shared");
        let dirs = PresetDirs {
            vst3_shared: vec![shared.clone()],
            ..dirs
        };
        let plugin = plugin();
        let state = container(b"the component", b"the controller");
        let saved = save(&dirs, &plugin, &state, "Soft: Pad", Some("Pads"), false).unwrap();
        let folder = home.join(".vst3/presets/Damien Zammit/ZamComp/Pads");
        assert_eq!(saved.path, folder.join("Soft_ Pad.vstpreset"));
        assert_eq!(saved.replaced, vec![':']);
        assert!(!saved.overwrote);
        assert_eq!(load(&dirs, &plugin, &saved.preset.id).unwrap(), state);

        // A file of another plugin and a factory one beside it.
        let factory = shared.join("Damien Zammit/ZamComp");
        std::fs::create_dir_all(&factory).unwrap();
        std::fs::write(factory.join("Zero.vstpreset"), build(CID, b"zero", b"")).unwrap();
        let other = build("00000000000000000000000000000001", b"x", b"");
        std::fs::write(factory.join("Other.vstpreset"), other).unwrap();
        std::fs::write(factory.join("Broken.vstpreset"), b"VST3").unwrap();
        let listed = list(&dirs, &plugin);
        let names: Vec<_> = listed
            .presets
            .iter()
            .map(|p| (p.name.as_str(), p.factory))
            .collect();
        assert_eq!(names, [("Zero", true), ("Soft_ Pad", false)]);
        assert_eq!(listed.presets[1].category.as_deref(), Some("Pads"));
        assert_eq!(
            (listed.others, listed.unreadable, listed.capped),
            (1, 1, false)
        );
        let zero = &listed.presets[0];
        assert_eq!(
            load(&dirs, &plugin, &zero.id).unwrap(),
            container(b"zero", b"")
        );

        // The same name again: only with `overwrite`.
        let louder = container(b"louder", b"");
        let again = save(&dirs, &plugin, &louder, "Soft: Pad", Some("Pads"), false);
        assert_eq!(again.unwrap_err(), PresetError::Exists(saved.path.clone()));
        assert_eq!(load(&dirs, &plugin, &saved.preset.id).unwrap(), state);
        let over = save(&dirs, &plugin, &louder, "Soft: Pad", Some("Pads"), true).unwrap();
        assert!(over.overwrote);
        assert_eq!(load(&dirs, &plugin, &saved.preset.id).unwrap(), louder);
        let left: Vec<_> = std::fs::read_dir(&folder)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(left.len(), 1, "no temporary file is left: {left:?}");
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn only_files_in_the_plugins_folders_load() {
        let home = temp("outside");
        let dirs = PresetDirs::home(&home);
        let plugin = plugin();
        let elsewhere = home.join("elsewhere.vstpreset");
        std::fs::write(&elsewhere, build(CID, b"x", b"")).unwrap();
        let id = format!("{SOURCE}:{}", elsewhere.display());
        assert_eq!(load(&dirs, &plugin, &id).unwrap_err(), PresetError::NoSuch);
        let folder = home.join(".vst3/presets/Damien Zammit/ZamComp");
        std::fs::create_dir_all(&folder).unwrap();
        let sneaky = format!(
            "{SOURCE}:{}/../../../../elsewhere.vstpreset",
            folder.display()
        );
        assert_eq!(
            load(&dirs, &plugin, &sneaky).unwrap_err(),
            PresetError::NoSuch
        );
        // A link in the folder is neither listed nor loaded.
        std::os::unix::fs::symlink(&elsewhere, folder.join("Link.vstpreset")).unwrap();
        assert!(list(&dirs, &plugin).presets.is_empty());
        let link = format!("{SOURCE}:{}", folder.join("Link.vstpreset").display());
        assert_eq!(
            load(&dirs, &plugin, &link).unwrap_err(),
            PresetError::NoSuch
        );
        // Nor is anything written through a linked folder.
        std::os::unix::fs::symlink(&home, folder.join("Pads")).unwrap();
        let state = container(b"c", b"");
        assert!(matches!(
            save(&dirs, &plugin, &state, "x", Some("Pads"), false),
            Err(PresetError::Other(_))
        ));
        assert!(!home.join("x.vstpreset").exists());
        std::fs::remove_dir_all(&home).unwrap();
    }
}

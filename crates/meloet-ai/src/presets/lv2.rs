//! LV2 presets (`pset:Preset`; docs/research/2026-11-23-plugin-presets-state.md Q9). lilv,
//! which reads and writes them, is in the plugin host (`meloet_core::ai::PresetAsk`); here are
//! the bundles of presets to hand it, and where a preset of the person is written. These read
//! folders: call them on the blocking pool.

use std::path::{Path, PathBuf};

use meloet_core::ai::PresetInfo;

use super::{Preset, PresetDirs};

/// Ids of these presets start with this and a colon; the rest is the preset's URI.
pub const SOURCE: &str = "lv2";
/// Bundles looked at in one folder, at most.
const MAX_ENTRIES: usize = 4096;
/// No manifest longer than this is looked into.
const MAX_MANIFEST: u64 = 4 << 20;
/// Bundles of presets handed to the plugin host, at most.
const MAX_BUNDLES: usize = 256;

/// Whether the Turtle `text` names `uri`: whole (`<uri>`), or as a prefixed name of one of its
/// `@prefix` lines.
fn mentions(text: &str, uri: &str) -> bool {
    if text.contains(&format!("<{uri}>")) {
        return true;
    }
    (text.lines())
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("@prefix")?.trim();
            let (name, iri) = rest.split_once(':')?;
            let iri = iri.trim().strip_prefix('<')?.split_once('>')?.0;
            Some((name.trim(), iri))
        })
        .any(|(name, iri)| {
            let local = uri.strip_prefix(iri).unwrap_or("\u{0}");
            !local.is_empty()
                && !local.contains('\u{0}')
                && local
                    .chars()
                    .all(|c| c.is_alphanumeric() || "_-.".contains(c))
                && text.contains(&format!("{name}:{local}"))
        })
}

/// Whether the manifest of the bundle `bundle` names presets of `plugin_uri` and no plugin.
fn holds_presets_of(bundle: &Path, plugin_uri: &str) -> bool {
    let manifest = bundle.join("manifest.ttl");
    let Ok(meta) = std::fs::symlink_metadata(&manifest) else {
        return false;
    };
    if !meta.is_file() || meta.len() > MAX_MANIFEST {
        return false;
    }
    let Ok(text) = std::fs::read_to_string(&manifest) else {
        return false;
    };
    text.contains("Preset")
        && !text.contains(":Plugin")
        && !text.contains("#Plugin>")
        && mentions(&text, plugin_uri)
}

/// The bundles of `dirs` (the person's first) that hold presets of the plugin `plugin_uri`
/// and describe no plugin; `own` is the plugin's bundle. Bundles that are symbolic links are
/// passed over.
pub fn bundles_for(dirs: &PresetDirs, plugin_uri: &str, own: &Path) -> Vec<PathBuf> {
    let own = own.canonicalize().ok();
    let mut found = Vec::new();
    for dir in dirs.lv2_user.iter().chain(&dirs.lv2_shared) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        let mut bundles: Vec<PathBuf> = (entries.flatten().take(MAX_ENTRIES))
            .filter(|e| e.file_type().is_ok_and(|kind| kind.is_dir()))
            .map(|e| e.path())
            .collect();
        bundles.sort();
        for bundle in bundles {
            if found.len() == MAX_BUNDLES {
                return found;
            }
            if own.is_some() && bundle.canonicalize().ok() == own {
                continue;
            }
            if holds_presets_of(&bundle, plugin_uri) {
                found.push(bundle);
            }
        }
    }
    found
}

/// Every character but ASCII letters and digits as `_`, as jalv names a preset's bundle and
/// file.
pub fn symbolify(text: &str) -> String {
    (text.chars())
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// Where the person's preset `name` of the plugin `plugin_name` is written, as jalv does it:
/// the bundle folder `~/.lv2/<Plugin>_<Name>.preset.lv2/` and its file `<Name>.ttl`.
pub fn user_bundle(dirs: &PresetDirs, plugin_name: &str, name: &str) -> Option<(PathBuf, String)> {
    let root = dirs.lv2_user.as_ref()?;
    let bundle = format!("{}.preset.lv2", symbolify(&format!("{plugin_name}_{name}")));
    Some((root.join(bundle), format!("{}.ttl", symbolify(name))))
}

/// A preset the plugin host listed. The person's are those in their own LV2 folder.
pub fn preset_of(dirs: &PresetDirs, info: &PresetInfo) -> Preset {
    let user = (dirs.lv2_user.as_ref())
        .zip(info.file.as_ref())
        .is_some_and(|(root, file)| file.starts_with(root));
    Preset {
        id: format!("{SOURCE}:{}", info.uri),
        name: info.label.clone(),
        category: info.bank.clone(),
        factory: !user,
    }
}

/// The URI of the preset `id` of [`preset_of`].
pub fn uri_of(id: &str) -> Option<&str> {
    id.strip_prefix(SOURCE)?
        .strip_prefix(':')
        .filter(|uri| !uri.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("k11presets-lv2-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_uri_is_found_whole_or_by_its_prefix() {
        let whole = "x lv2:appliesTo <http://zynaddsubfx.sourceforge.net> ;";
        assert!(mentions(whole, "http://zynaddsubfx.sourceforge.net"));
        assert!(!mentions(whole, "http://zynaddsubfx.sourceforge.net/other"));
        let prefixed = "@prefix zam: <urn:zamaudio:> .\n<p> lv2:appliesTo zam:ZamComp .";
        assert!(mentions(prefixed, "urn:zamaudio:ZamComp"));
        assert!(!mentions(prefixed, "urn:zamaudio:ZamGate"));
        assert!(!mentions(prefixed, "urn:other:ZamComp"));
    }

    #[test]
    fn bundles_of_presets_are_found_but_not_plugins_or_links() {
        let home = temp("bundles");
        let dirs = PresetDirs::home(&home);
        let lv2 = home.join(".lv2");
        let write = |bundle: &str, text: &str| {
            std::fs::create_dir_all(lv2.join(bundle)).unwrap();
            std::fs::write(lv2.join(bundle).join("manifest.ttl"), text).unwrap();
        };
        let preset = "<a> a pset:Preset ; lv2:appliesTo <urn:x:synth> .";
        write("a.preset.lv2", preset);
        write(
            "b.preset.lv2",
            "<b> a pset:Preset ; lv2:appliesTo <urn:x:other> .",
        );
        write(
            "own.lv2",
            &format!("<urn:x:synth> a lv2:Plugin .\n{preset}"),
        );
        write(
            "copy.lv2",
            &format!("<urn:x:synth> a lv2:Plugin .\n{preset}"),
        );
        std::os::unix::fs::symlink(lv2.join("a.preset.lv2"), lv2.join("link.preset.lv2")).unwrap();
        let found = bundles_for(&dirs, "urn:x:synth", &lv2.join("own.lv2"));
        assert_eq!(found, [lv2.join("a.preset.lv2")]);
        std::fs::remove_dir_all(&home).unwrap();
    }

    #[test]
    fn a_preset_of_the_person_is_written_as_jalv_writes_it() {
        let dirs = PresetDirs::home(Path::new("/home/me"));
        let (bundle, file) = user_bundle(&dirs, "ZamComp", "Soft pad #2").unwrap();
        assert_eq!(
            bundle,
            Path::new("/home/me/.lv2/ZamComp_Soft_pad__2.preset.lv2")
        );
        assert_eq!(file, "Soft_pad__2.ttl");
        let info = PresetInfo {
            uri: "file:///home/me/.lv2/ZamComp_Soft.preset.lv2/Soft.ttl".into(),
            label: "Soft".into(),
            bank: None,
            file: Some("/home/me/.lv2/ZamComp_Soft.preset.lv2/Soft.ttl".into()),
        };
        let preset = preset_of(&dirs, &info);
        assert!(!preset.factory);
        assert_eq!(uri_of(&preset.id), Some(info.uri.as_str()));
        let factory = PresetInfo {
            file: Some("/usr/lib/lv2/ZamComp.lv2/presets.ttl".into()),
            ..info
        };
        assert!(preset_of(&dirs, &factory).factory);
        assert_eq!(uri_of("vstpreset:/x"), None);
    }
}

//! What a scan found, and the file it is remembered in (docs/architecture.md 20-4).
//!
//! A bundle (a `.vst3` or `.lv2` directory) is scanned by one child process, one plugin after
//! another. Its record is kept with the bundle's stamp, and the bundle is scanned again only
//! when the stamp changed. A plugin that stopped the scan is kept in the record too, with the
//! reason, and is skipped until the person asks for it again or the stamp changes; the other
//! plugins of the bundle go on the shelf.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::category::PluginKind;
use crate::key::{PluginFormat, PluginKey};

/// One plugin of a bundle, as the shelf needs it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub key: PluginKey,
    /// As the plugin says it; never changed here.
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub kind: PluginKind,
    pub editor: EditorKind,
    /// Channels of the main audio input and output. An instrument has 0 inputs.
    pub audio_inputs: u32,
    pub audio_outputs: u32,
}

/// The plugin's own window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EditorKind {
    /// Opens inside an X11 window (VST3 `X11EmbedWindowID`, LV2 `ui:X11UI`).
    X11,
    /// Has a window of a type meloet does not open (Gtk, Qt, an external window).
    Unsupported,
    /// Has no window: the parameter list is all there is.
    None,
}

/// Spec 3 (Y13), the three reasons the dialog 「スキャンで止まったプラグイン」 gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StopReason {
    /// 「読みこむと止まりました」: the scanning process died.
    Crashed,
    /// 「30 秒たっても終わりませんでした」.
    TimedOut,
    /// 「作りがこわれています」: the process answered that the plugin, or the bundle, cannot be
    /// read.
    Broken,
}

/// A scan of one bundle gives up after this long.
pub const SCAN_TIMEOUT_S: u64 = 30;

/// A plugin, or a whole bundle, that a scan stopped at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoppedEntry {
    /// The plugin's id in its bundle (`PluginKey::id`); `None` is the whole bundle.
    pub id: Option<String>,
    /// What the dialog shows; see [`stopped_name`].
    pub name: String,
    pub reason: StopReason,
}

/// The name a stopped plugin is listed by: what the child said while reading it (`Probing`),
/// else the name a scan before this one read for the same id, else the bundle's file name.
pub fn stopped_name(
    bundle: &Path,
    id: Option<&str>,
    probing: Option<&str>,
    before: Option<&BundleRecord>,
) -> String {
    let earlier = || {
        let id = id?;
        let before = before?;
        before
            .plugins
            .iter()
            .find(|p| p.key.id() == id)
            .map(|p| p.name.clone())
            .or_else(|| {
                before
                    .stopped
                    .iter()
                    .find(|s| s.id.as_deref() == Some(id))
                    .map(|s| s.name.clone())
            })
    };
    probing
        .filter(|n| !n.is_empty())
        .map(str::to_owned)
        .or_else(earlier)
        .unwrap_or_else(|| file_stem(bundle))
}

fn file_stem(bundle: &Path) -> String {
    bundle
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// What tells a changed bundle from an unchanged one: the newest change time of any file in
/// it, and the sizes of its files added up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleStamp {
    /// Nanoseconds since the Unix epoch.
    pub modified_ns: u64,
    pub size: u64,
}

/// A bundle is never this deep; anything deeper is not looked at.
const STAMP_DEPTH: u32 = 8;

impl BundleStamp {
    /// Reads the bundle's files. A bundle that is a link is read where it points; links in
    /// it are not followed.
    pub fn read(bundle: &Path) -> std::io::Result<BundleStamp> {
        let mut stamp = BundleStamp {
            modified_ns: 0,
            size: 0,
        };
        stamp.add(bundle, true, STAMP_DEPTH)?;
        Ok(stamp)
    }

    fn add(&mut self, path: &Path, top: bool, depth: u32) -> std::io::Result<()> {
        let meta = match top {
            true => std::fs::metadata(path)?,
            false => std::fs::symlink_metadata(path)?,
        };
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos().min(u128::from(u64::MAX)) as u64);
        self.modified_ns = self.modified_ns.max(modified);
        if meta.is_dir() {
            if depth == 0 {
                return Ok(());
            }
            for entry in std::fs::read_dir(path)? {
                self.add(&entry?.path(), false, depth - 1)?;
            }
        } else {
            self.size = self.size.saturating_add(meta.len());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleRecord {
    pub path: PathBuf,
    pub format: PluginFormat,
    pub stamp: BundleStamp,
    /// What went on the shelf. May be empty: a bundle with nothing meloet can use.
    pub plugins: Vec<PluginInfo>,
    pub stopped: Vec<StoppedEntry>,
}

impl BundleRecord {
    /// The bundle's file name without `.vst3` or `.lv2`: the name of a stopped plugin when
    /// nothing better is known.
    pub fn file_name(&self) -> String {
        file_stem(&self.path)
    }
}

/// A cache of another version is thrown away and every bundle is scanned again. 2: the host
/// reads VST3 and LV2 (version 1 came from a host that could not, and marked them broken).
pub const SCAN_CACHE_VERSION: u32 = 2;
const SCAN_FILE: &str = "scan.json";

/// Everything the last scans found, and what stopped them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScanCache {
    pub bundles: Vec<BundleRecord>,
}

#[derive(Serialize, Deserialize)]
struct OnDisk {
    version: u32,
    bundles: Vec<BundleRecord>,
}

impl ScanCache {
    pub fn get(&self, bundle: &Path) -> Option<&BundleRecord> {
        self.bundles.iter().find(|b| b.path == bundle)
    }

    /// Puts the record in place of the one for the same path.
    pub fn put(&mut self, record: BundleRecord) {
        match self.bundles.iter_mut().find(|b| b.path == record.path) {
            Some(old) => *old = record,
            None => self.bundles.push(record),
        }
    }

    pub fn remove(&mut self, bundle: &Path) -> Option<BundleRecord> {
        let at = self.bundles.iter().position(|b| b.path == bundle)?;
        Some(self.bundles.remove(at))
    }

    pub fn plugins(&self) -> impl Iterator<Item = (&BundleRecord, &PluginInfo)> {
        self.bundles
            .iter()
            .flat_map(|b| b.plugins.iter().map(move |p| (b, p)))
    }

    pub fn stopped(&self) -> impl Iterator<Item = (&BundleRecord, &StoppedEntry)> {
        self.bundles
            .iter()
            .flat_map(|b| b.stopped.iter().map(move |s| (b, s)))
    }

    /// Whether a scan was ever written to `dir`. Without one, the scan at start is the first:
    /// it shows its count.
    pub fn is_saved(dir: &Path) -> bool {
        dir.join(SCAN_FILE).exists()
    }

    /// A missing, unreadable or broken file, or one of another version, is an empty cache:
    /// everything is scanned again.
    pub fn load(dir: &Path) -> ScanCache {
        let Ok(text) = std::fs::read_to_string(dir.join(SCAN_FILE)) else {
            return ScanCache::default();
        };
        match serde_json::from_str::<OnDisk>(&text) {
            Ok(disk) if disk.version == SCAN_CACHE_VERSION => ScanCache {
                bundles: disk.bundles,
            },
            _ => ScanCache::default(),
        }
    }

    /// Writes a temporary file next to the real one and renames it over it.
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let disk = OnDisk {
            version: SCAN_CACHE_VERSION,
            bundles: self.bundles.clone(),
        };
        let text = serde_json::to_string(&disk).map_err(std::io::Error::other)?;
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(format!("{SCAN_FILE}.tmp"));
        let mut file = std::fs::File::create(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(tmp, dir.join(SCAN_FILE))
    }
}

/// Where the scan file lives: `$XDG_CACHE_HOME/meloet/plugins`, else
/// `$HOME/.cache/meloet/plugins`.
pub fn cache_dir(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let set = |name: &str| var(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    set("XDG_CACHE_HOME")
        .map(|dir| dir.join("meloet"))
        .or_else(|| set("HOME").map(|dir| dir.join(".cache").join("meloet")))
        .map(|dir| dir.join("plugins"))
}

/// The places plugins usually are on Linux, in the order the settings list them: LV2 (and
/// what `LV2_PATH` names), then VST3. CLAP's places are not here (20-9). They cannot be
/// removed; the places a person adds are kept in the settings.
pub fn default_places(var: impl Fn(&str) -> Option<OsString>) -> Vec<PathBuf> {
    let home = var("HOME").filter(|v| !v.is_empty()).map(PathBuf::from);
    let mut places: Vec<PathBuf> = Vec::new();
    let mut add = |place: PathBuf| {
        if place.is_absolute() && !places.contains(&place) {
            places.push(place);
        }
    };
    if let Some(home) = &home {
        add(home.join(".lv2"));
    }
    add(PathBuf::from("/usr/lib/lv2"));
    add(PathBuf::from("/usr/local/lib/lv2"));
    if let Some(paths) = var("LV2_PATH") {
        std::env::split_paths(&paths).for_each(&mut add);
    }
    if let Some(home) = &home {
        add(home.join(".vst3"));
    }
    add(PathBuf::from("/usr/lib/vst3"));
    add(PathBuf::from("/usr/local/lib/vst3"));
    places
}

/// The format of a bundle, by the ending of its name.
pub fn bundle_format(path: &Path) -> Option<PluginFormat> {
    match path.extension()?.to_str()? {
        "vst3" => Some(PluginFormat::Vst3),
        "lv2" => Some(PluginFormat::Lv2),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::category::{EffectCategory, InstrumentCategory};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("meloet-plugin-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn surge() -> PluginInfo {
        PluginInfo {
            key: "lv2:https://surge-synthesizer.github.io/lv2/surge-xt"
                .parse()
                .unwrap(),
            name: "Surge XT".into(),
            vendor: "Surge Synth Team".into(),
            version: "1.3.4".into(),
            kind: PluginKind::Instrument(InstrumentCategory::Other),
            editor: EditorKind::X11,
            audio_inputs: 0,
            audio_outputs: 2,
        }
    }

    fn sample() -> ScanCache {
        let zam = PluginInfo {
            key: "vst3:647370206B31732020202020756E7363".parse().unwrap(),
            name: "ZamComp".into(),
            vendor: "Damien Zammit".into(),
            version: "4.2".into(),
            kind: PluginKind::Effect(EffectCategory::Dynamics),
            editor: EditorKind::None,
            audio_inputs: 1,
            audio_outputs: 1,
        };
        let stamp = BundleStamp {
            modified_ns: 1_763_500_000_000_000_000,
            size: 4096,
        };
        ScanCache {
            bundles: vec![
                BundleRecord {
                    path: "/usr/lib/lv2/Surge XT.lv2".into(),
                    format: PluginFormat::Lv2,
                    stamp,
                    plugins: vec![surge()],
                    stopped: vec![],
                },
                BundleRecord {
                    path: "/usr/lib/vst3/ZamComp.vst3".into(),
                    format: PluginFormat::Vst3,
                    stamp,
                    plugins: vec![zam],
                    stopped: vec![],
                },
                BundleRecord {
                    path: "/home/me/.vst3/OldChorus.vst3".into(),
                    format: PluginFormat::Vst3,
                    stamp,
                    plugins: vec![],
                    stopped: vec![StoppedEntry {
                        id: None,
                        name: "OldChorus".into(),
                        reason: StopReason::TimedOut,
                    }],
                },
                BundleRecord {
                    path: "/usr/lib/lv2/lsp-plugins.lv2".into(),
                    format: PluginFormat::Lv2,
                    stamp,
                    plugins: vec![lsp("comp")],
                    stopped: vec![StoppedEntry {
                        id: Some("http://lsp-plug.in/plugins/lv2/gate".into()),
                        name: "LSP Gate".into(),
                        reason: StopReason::Crashed,
                    }],
                },
            ],
        }
    }

    fn lsp(what: &str) -> PluginInfo {
        PluginInfo {
            key: PluginKey::lv2(&format!("http://lsp-plug.in/plugins/lv2/{what}")).unwrap(),
            name: format!("LSP {what}"),
            vendor: "LSP".into(),
            version: "1.2".into(),
            kind: PluginKind::Effect(EffectCategory::Dynamics),
            editor: EditorKind::X11,
            audio_inputs: 2,
            audio_outputs: 2,
        }
    }

    #[test]
    fn cache_round_trips_through_its_file() {
        let dir = temp_dir("cache");
        assert_eq!(ScanCache::load(&dir), ScanCache::default());
        let cache = sample();
        cache.save(&dir).unwrap();
        assert_eq!(ScanCache::load(&dir), cache);
        assert!(!dir.join("scan.json.tmp").exists());
        assert_eq!(cache.plugins().count(), 3);
        let stopped: Vec<_> = cache.stopped().collect();
        assert_eq!(stopped.len(), 2);
        assert_eq!(stopped[0].0.file_name(), "OldChorus");
        assert_eq!(stopped[0].1.reason, StopReason::TimedOut);
        assert_eq!(stopped[1].0.file_name(), "lsp-plugins");
        assert_eq!(stopped[1].1.name, "LSP Gate");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn record_json_shape() {
        let cache = sample();
        let json = serde_json::to_value(&cache.bundles[2]).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "path": "/home/me/.vst3/OldChorus.vst3",
                "format": "vst3",
                "stamp": { "modifiedNs": 1_763_500_000_000_000_000u64, "size": 4096 },
                "plugins": [],
                "stopped": [{ "id": null, "name": "OldChorus", "reason": "timedOut" }],
            })
        );
        let json = serde_json::to_value(&cache.bundles[3]).unwrap();
        assert_eq!(
            json["stopped"],
            serde_json::json!([{
                "id": "http://lsp-plug.in/plugins/lv2/gate",
                "name": "LSP Gate",
                "reason": "crashed",
            }])
        );
        assert_eq!(
            json["plugins"][0]["key"],
            "lv2:http://lsp-plug.in/plugins/lv2/comp"
        );
        let back: BundleRecord = serde_json::from_value(json).unwrap();
        assert_eq!(back, cache.bundles[3]);
        let json = serde_json::to_value(surge()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "key": "lv2:https://surge-synthesizer.github.io/lv2/surge-xt",
                "name": "Surge XT",
                "vendor": "Surge Synth Team",
                "version": "1.3.4",
                "kind": { "role": "instrument", "category": "other" },
                "editor": "x11",
                "audioInputs": 0,
                "audioOutputs": 2,
            })
        );
    }

    #[test]
    fn a_broken_file_or_another_version_is_an_empty_cache() {
        let dir = temp_dir("broken");
        std::fs::write(dir.join("scan.json"), "{ not json").unwrap();
        assert_eq!(ScanCache::load(&dir), ScanCache::default());
        std::fs::write(dir.join("scan.json"), r#"{"version":999,"bundles":[]}"#).unwrap();
        assert_eq!(ScanCache::load(&dir), ScanCache::default());
        // One record with a key this meloet cannot read spoils the file: scan again.
        let bad = r#"{"version":1,"bundles":[{"path":"/x.lv2","format":"lv2","stamp":{"modifiedNs":1,"size":1},
            "plugins":[{"key":"clap:x","name":"","vendor":"","version":"",
            "kind":{"role":"effect","category":"other"},"editor":"none","audioInputs":2,"audioOutputs":2}],
            "stopped":[]}]}"#;
        std::fs::write(dir.join("scan.json"), bad).unwrap();
        assert_eq!(ScanCache::load(&dir), ScanCache::default());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn put_replaces_the_record_of_the_same_bundle() {
        let mut cache = sample();
        let mut again = cache.bundles[2].clone();
        again.stopped.clear();
        cache.put(again.clone());
        assert_eq!(cache.bundles.len(), 4);
        assert_eq!(cache.get(&again.path), Some(&again));
        assert_eq!(cache.stopped().count(), 1);
        assert!(cache.remove(&again.path).is_some());
        assert!(cache.get(&again.path).is_none());
    }

    #[test]
    fn a_stopped_plugin_is_named_by_probing_then_the_scan_before_then_the_file() {
        let cache = sample();
        let lsp_bundle = &cache.bundles[3];
        let path = lsp_bundle.path.as_path();
        let comp = Some("http://lsp-plug.in/plugins/lv2/comp");
        let gate = Some("http://lsp-plug.in/plugins/lv2/gate");
        assert_eq!(
            stopped_name(path, comp, Some("LSP Compressor"), Some(lsp_bundle)),
            "LSP Compressor"
        );
        assert_eq!(stopped_name(path, comp, None, Some(lsp_bundle)), "LSP comp");
        assert_eq!(
            stopped_name(path, gate, Some(""), Some(lsp_bundle)),
            "LSP Gate"
        );
        assert_eq!(stopped_name(path, comp, None, None), "lsp-plugins");
        assert_eq!(
            stopped_name(path, Some("urn:other"), None, Some(lsp_bundle)),
            "lsp-plugins"
        );
        assert_eq!(
            stopped_name(path, None, None, Some(lsp_bundle)),
            "lsp-plugins"
        );
    }

    #[test]
    fn the_stamp_changes_when_a_file_in_the_bundle_changes() {
        let dir = temp_dir("stamp");
        let bundle = dir.join("Thing.vst3");
        std::fs::create_dir_all(bundle.join("Contents/x86_64-linux")).unwrap();
        std::fs::write(bundle.join("Contents/x86_64-linux/Thing.so"), [0u8; 100]).unwrap();
        let first = BundleStamp::read(&bundle).unwrap();
        assert_eq!(first.size, 100);
        assert!(first.modified_ns > 0);
        assert_eq!(BundleStamp::read(&bundle).unwrap(), first);
        std::fs::write(bundle.join("Contents/x86_64-linux/Thing.so"), [0u8; 101]).unwrap();
        assert_ne!(BundleStamp::read(&bundle).unwrap(), first);
        assert!(BundleStamp::read(&dir.join("Missing.vst3")).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_bundle_that_is_a_link_is_stamped_by_what_it_points_to() {
        let dir = temp_dir("stamp-link");
        let target = dir.join("elsewhere/Thing.vst3");
        std::fs::create_dir_all(target.join("Contents")).unwrap();
        std::fs::write(target.join("Contents/Thing.so"), [0u8; 100]).unwrap();
        let link = dir.join("Thing.vst3");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let first = BundleStamp::read(&link).unwrap();
        assert_eq!(first.size, 100);
        std::fs::write(target.join("Contents/Thing.so"), [0u8; 101]).unwrap();
        assert_ne!(BundleStamp::read(&link).unwrap(), first);
        // Pointing nowhere, it cannot be read.
        std::fs::remove_dir_all(&target).unwrap();
        assert!(BundleStamp::read(&link).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| OsString::from(v))
        }
    }

    #[test]
    fn default_places_are_lv2_then_vst3() {
        let places = default_places(env(&[("HOME", "/home/me")]));
        let expected = [
            "/home/me/.lv2",
            "/usr/lib/lv2",
            "/usr/local/lib/lv2",
            "/home/me/.vst3",
            "/usr/lib/vst3",
            "/usr/local/lib/vst3",
        ];
        assert_eq!(places, expected.map(PathBuf::from));
    }

    #[test]
    fn lv2_path_adds_its_places_once() {
        let vars = [
            ("HOME", "/home/me"),
            ("LV2_PATH", "/opt/lv2:/usr/lib/lv2::relative:/srv/lv2"),
        ];
        let places = default_places(env(&vars));
        let expected = [
            "/home/me/.lv2",
            "/usr/lib/lv2",
            "/usr/local/lib/lv2",
            "/opt/lv2",
            "/srv/lv2",
            "/home/me/.vst3",
            "/usr/lib/vst3",
            "/usr/local/lib/vst3",
        ];
        assert_eq!(places, expected.map(PathBuf::from));
        assert!(places.iter().all(|p| !p.to_string_lossy().contains("clap")));
    }

    #[test]
    fn cache_dir_follows_xdg() {
        assert_eq!(
            cache_dir(env(&[("HOME", "/home/me")])),
            Some("/home/me/.cache/meloet/plugins".into())
        );
        let vars = [("HOME", "/home/me"), ("XDG_CACHE_HOME", "/tmp/c")];
        assert_eq!(cache_dir(env(&vars)), Some("/tmp/c/meloet/plugins".into()));
        assert_eq!(cache_dir(env(&[])), None);
    }

    #[test]
    fn bundle_format_is_read_from_the_ending() {
        assert_eq!(
            bundle_format(Path::new("/usr/lib/vst3/Surge XT.vst3")),
            Some(PluginFormat::Vst3)
        );
        assert_eq!(
            bundle_format(Path::new("/usr/lib/lv2/ZamComp.lv2")),
            Some(PluginFormat::Lv2)
        );
        assert_eq!(
            bundle_format(Path::new("/usr/lib/clap/Surge XT.clap")),
            None
        );
        assert_eq!(bundle_format(Path::new("/usr/lib/lv2")), None);
    }
}

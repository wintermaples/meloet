//! Development measurements. Nothing here runs unless `MELOET_MEASURE_DIR` is set.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use meloet_engine::LatencyRecord;

use crate::plugins::hosts::ProcessInfo;

const FILE_NAME: &str = "key-to-sound.json";
const PLUGINS_FILE_NAME: &str = "plugins.json";
const WRITE_EVERY: Duration = Duration::from_secs(2);
const MAX_RECORDS: usize = 100_000;

/// `MELOET_MEASURE_DIR`, when set.
pub fn dir_from_env() -> Option<PathBuf> {
    std::env::var_os("MELOET_MEASURE_DIR")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Writes `text` to `<dir>/<name>`. The name may hold only ASCII letters, digits and `-_.`.
pub fn write_report(dir: &Path, name: &str, text: &str) -> Result<(), String> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
    if name.is_empty() || !name.chars().all(allowed) || name.chars().all(|c| c == '.') {
        return Err(format!("not a report name: {name:?}"));
    }
    write_whole(dir, name, text).map_err(|e| format!("{}: {e}", dir.join(name).display()))
}

fn write_whole(dir: &Path, name: &str, text: &str) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let temporary = dir.join(format!(".{name}.tmp"));
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, dir.join(name))
}

/// Writes what the engine saw of each plugin process to `plugins.json`: the blocks that got no
/// answer in time, the time waited for answers, the delay the plugins say they have. A process
/// that has ended keeps the numbers it had when last looked at.
pub(crate) struct PluginLog {
    dir: PathBuf,
    seen: BTreeMap<u32, ProcessInfo>,
    looked_at: Instant,
}

impl PluginLog {
    pub fn new(dir: PathBuf) -> PluginLog {
        PluginLog {
            dir,
            seen: BTreeMap::new(),
            looked_at: Instant::now(),
        }
    }

    /// Looks and writes when the last look is old enough (or `now` is set).
    pub fn look(&mut self, now: bool, processes: impl FnOnce() -> Vec<ProcessInfo>) {
        if !(now || self.looked_at.elapsed() >= WRITE_EVERY) {
            return;
        }
        self.looked_at = Instant::now();
        let found = processes();
        if found.is_empty() && self.seen.is_empty() {
            return;
        }
        self.seen.extend(found.into_iter().map(|p| (p.pid, p)));
        let report = serde_json::json!({
            "processes": self.seen.values().map(|p| serde_json::json!({
                "key": p.key.to_string(),
                "pid": p.pid,
                "plugins": p.slots.len(),
                "unansweredBlocks": p.unanswered,
                "waitedMs": p.waited_ns as f64 / 1e6,
                "latencyFrames": p.latency,
            })).collect::<Vec<_>>(),
        });
        if let Err(e) = write_whole(&self.dir, PLUGINS_FILE_NAME, &format!("{report:#}\n")) {
            eprintln!("meloet: MELOET_MEASURE_DIR: {e}");
        }
    }
}

/// Collects every key-to-sound record and writes them all to `key-to-sound.json`.
pub(crate) struct KeyToSound {
    dir: PathBuf,
    records: Vec<LatencyRecord>,
    unwritten: bool,
    written_at: Instant,
}

impl KeyToSound {
    pub fn new(dir: PathBuf) -> KeyToSound {
        KeyToSound {
            dir,
            records: Vec::new(),
            unwritten: false,
            written_at: Instant::now(),
        }
    }

    pub fn add(&mut self, record: LatencyRecord) {
        if self.records.len() < MAX_RECORDS {
            self.records.push(record);
            self.unwritten = true;
        }
    }

    /// Writes when there is something new and the last write is old enough (or `now` is set).
    pub fn write(&mut self, now: bool, sample_rate: u32, requested: u32, actual: Option<u32>) {
        if !self.unwritten || !(now || self.written_at.elapsed() >= WRITE_EVERY) {
            return;
        }
        let ms: Vec<f64> = self
            .records
            .iter()
            .map(|r| (r.playback_us as f64 - r.key_us as f64) / 1000.0)
            .collect();
        let report = serde_json::json!({
            "sampleRate": sample_rate,
            "requestedFrames": requested,
            "actualFrames": actual,
            "count": ms.len(),
            "keyToDacMs": {
                "mean": ms.iter().sum::<f64>() / ms.len().max(1) as f64,
                "min": ms.iter().copied().fold(f64::INFINITY, f64::min),
                "max": ms.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            },
            "records": self.records.iter().map(|r| serde_json::json!({
                "keyUs": r.key_us,
                "callbackUs": r.callback_us,
                "playbackUs": r.playback_us,
            })).collect::<Vec<_>>(),
        });
        if let Err(e) = write_whole(&self.dir, FILE_NAME, &format!("{report:#}\n")) {
            eprintln!("meloet: MELOET_MEASURE_DIR: {e}");
        }
        self.unwritten = false;
        self.written_at = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_names_stay_inside_the_directory() {
        let dir = crate::settings::temp_dir("measure");
        write_report(&dir, "frame-times_1.json", "{}").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("frame-times_1.json")).unwrap(),
            "{}"
        );
        for name in ["", ".", "..", "a/b", "../x", "a b", "名前.json", "a\\b"] {
            assert!(write_report(&dir, name, "{}").is_err(), "{name:?}");
        }
    }

    #[test]
    fn key_to_sound_file() {
        let dir = crate::settings::temp_dir("key-to-sound");
        let mut log = KeyToSound::new(dir.clone());
        log.write(true, 48_000, 256, Some(256));
        assert!(!dir.join(FILE_NAME).exists(), "nothing to write yet");

        for key_us in [1_000_000, 2_000_000] {
            log.add(LatencyRecord {
                key_us,
                callback_us: key_us + 3_000,
                playback_us: key_us + 13_000,
            });
        }
        log.write(true, 48_000, 256, Some(256));
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join(FILE_NAME)).unwrap()).unwrap();
        assert_eq!(json["count"], 2);
        assert_eq!(json["actualFrames"], 256);
        assert_eq!(json["keyToDacMs"]["mean"], 13.0);
        assert_eq!(json["records"][1]["playbackUs"], 2_013_000);
    }
}

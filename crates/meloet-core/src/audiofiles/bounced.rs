//! 「音のクリップにする」 (round 12, A7) on the thread that reads audio files in: the file a
//! job of the export rendered into the work place under a name of its own gets a name the
//! song can use, and its peaks.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::time::{SystemTime, UNIX_EPOCH};

use meloet_model::NewAudio;
use sha2::{Digest, Sha256};

use super::AudioStore;
use super::clips::put_in_place;
use super::wav::WavPcm;
use crate::pcm::PcmFile;
use crate::peaks::PeakBuilder;

/// Frames read at a time.
const STEP: usize = 1 << 15;

static SERIAL: AtomicU64 = AtomicU64::new(0);

/// Takes the file at `temp` (in the work place) as audio of the song: its peaks are made and
/// kept under a fresh uid, and it is named `wanted`, or `wanted` with " 2", " 3" … when a
/// file of the song's places or a name of `taken` has that name. The file at `temp` is gone
/// afterwards, also when this fails.
pub fn adopt(
    store: &AudioStore,
    temp: &Path,
    wanted: &str,
    taken: &[String],
) -> Result<NewAudio, String> {
    let made = (|| {
        let work = store.work_dir().ok_or("no work place")?;
        let mut wav = WavPcm::open(temp).map_err(|e| e.to_string())?;
        let channels = wav.channels();
        let width = usize::from(channels);
        let mut peaks = PeakBuilder::new(channels);
        let mut block = vec![0.0f32; STEP * width];
        let mut at = 0u64;
        while at < wav.frames() {
            let got = wav.read(at, &mut block).map_err(|e| e.to_string())?;
            if got == 0 {
                break;
            }
            peaks.push(&block[..got * width]);
            at += got as u64;
        }
        if at == 0 {
            return Err("the file is empty".to_owned());
        }
        let audio = (at, wav.sample_rate(), channels, wav.bits());
        drop(wav);
        let file = put_in_place(store, &work, temp, wanted, taken).map_err(|e| e.to_string())?;
        Ok((file, audio, peaks))
    })();
    let (file, (frames, sample_rate, channels, bits), peaks) = match made {
        Ok(made) => made,
        Err(e) => {
            let _ = std::fs::remove_file(temp);
            return Err(e);
        }
    };
    let uid = fresh_uid(&file);
    if let Some(path) = store.peaks_path(&uid)
        && let Err(e) = peaks.write(&path)
    {
        eprintln!("meloet: the peaks of {file} were not kept: {e}");
    }
    store.file_arrived();
    Ok(NewAudio {
        uid,
        file,
        frames,
        sample_rate,
        channels,
        bits: Some(bits),
    })
}

/// A number no other file gets: what is kept beside the file (peaks, the cache) goes by it.
fn fresh_uid(file: &str) -> [u8; 16] {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let mut hash = Sha256::new();
    hash.update(b"meloet bounced audio");
    hash.update(file.as_bytes());
    hash.update(std::process::id().to_le_bytes());
    hash.update(now.as_nanos().to_le_bytes());
    hash.update(SERIAL.fetch_add(1, Relaxed).to_le_bytes());
    let mut uid = [0u8; 16];
    uid.copy_from_slice(&hash.finalize()[..16]);
    uid
}

#[cfg(test)]
mod tests;

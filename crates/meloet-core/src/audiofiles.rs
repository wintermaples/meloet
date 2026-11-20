//! The audio files of a song: bringing them in, the place they are kept in, the song's
//! folder (docs/architecture.md 22-4, 22-8). So far only what the drag over the window
//! needs; the rest comes with the second half of round 7 step 3.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use meloet_protocol::AUDIO_EXTENSIONS;

use crate::pcm::{PcmFile, PcmStore};

/// Where the read-ahead thread finds the song's audio at the engine's rate (22-4, 22-8):
/// the song's `音/`, the work place, and the cache of decoded and resampled files. One for
/// the life of the core, shared with that thread. Round 7 step 3: the audio files half (A2)
/// fills in where it looks and `open`; the playing half (A1) hands it to the thread and
/// keeps `set_sources` up to date.
pub struct AudioStore {
    places: Mutex<Places>,
}

struct Places {
    /// `$MELOET_STATE_DIR`… as the recovery file has it; `None` in tests without one.
    state_dir: Option<PathBuf>,
    /// The folder of the song file; `None` while the song was never saved.
    song_dir: Option<PathBuf>,
    /// `AudioSource::uid` → `AudioSource::file`, for the song as it is.
    sources: Vec<([u8; 16], String)>,
}

impl AudioStore {
    pub fn new(state_dir: Option<PathBuf>) -> AudioStore {
        AudioStore {
            places: Mutex::new(Places {
                state_dir,
                song_dir: None,
                sources: Vec::new(),
            }),
        }
    }

    /// The song was opened, saved, or saved under another name; `None` for a new song.
    pub fn set_song_dir(&self, dir: Option<PathBuf>) {
        self.places
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .song_dir = dir;
    }

    /// The song's audio table changed.
    pub fn set_sources(&self, sources: Vec<([u8; 16], String)>) {
        self.places
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .sources = sources;
    }
}

impl PcmStore for AudioStore {
    fn open(&self, _uid: &[u8; 16], _sample_rate: u32) -> Option<Box<dyn PcmFile>> {
        let places = self.places.lock().unwrap_or_else(|e| e.into_inner());
        let _ = (&places.state_dir, &places.song_dir, &places.sources);
        None
    }
}

/// Whether a file held over the window may be audio that reads, going by its name alone
/// (spec 3 「読めるファイル」). What it holds is found out when it is brought in.
pub fn looks_like_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| AUDIO_EXTENSIONS.iter().any(|a| a.eq_ignore_ascii_case(e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn by_the_ending_of_the_name() {
        for name in ["ドラム.wav", "a.MP3", "b.Flac", "/tmp/x/声.ogg"] {
            assert!(looks_like_audio(Path::new(name)), "{name}");
        }
        for name in [
            "声.aac",
            "FluidR3_GM.sf2",
            "wav",
            "曲.meloet",
            "x.wav.txt",
            "",
        ] {
            assert!(!looks_like_audio(Path::new(name)), "{name}");
        }
    }
}

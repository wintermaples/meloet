//! The audio files of a song: bringing them in, the place they are kept in, the song's
//! folder (docs/architecture.md 22-4, 22-8). So far only what the drag over the window
//! needs; the rest comes with the second half of round 7 step 3.

use std::path::Path;

use meloet_protocol::AUDIO_EXTENSIONS;

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

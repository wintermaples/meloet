use std::path::Path;

use super::*;
use crate::audiofiles::tests::write_wav;
use crate::peaks::read_peaks_head;
use crate::settings::temp_dir;

fn store(root: &Path) -> AudioStore {
    AudioStore::new(Some(root.join("state"))).with_cache(Some(root.join("cache")))
}

#[test]
fn the_file_gets_a_free_name_and_its_peaks_and_the_temporary_one_is_gone() {
    let root = temp_dir("bounced-adopt");
    let store = store(&root);
    let work = store.work_dir().unwrap();
    std::fs::create_dir_all(&work).unwrap();
    // A file a recording has, and a name the song's table has whose file is not found.
    write_wav(
        &work.join("ピアノ（音） 7.wav"),
        48_000,
        1,
        16,
        10,
        |_, _| 0.0,
    );
    let taken = vec!["ピアノ（音） 7 2.wav".to_owned()];
    let temp = work.join(".bounce-1-2.wav");
    write_wav(&temp, 48_000, 2, 24, 3_000, |i, c| {
        (i as f32 / 3_000.0) * if c == 0 { 0.5 } else { -0.25 }
    });
    let before = store.generation();

    let audio = adopt(&store, &temp, "ピアノ（音） 7.wav", &taken).unwrap();
    assert_eq!(audio.file, "ピアノ（音） 7 3.wav");
    assert_eq!(
        (audio.frames, audio.sample_rate, audio.channels, audio.bits),
        (3_000, 48_000, 2, Some(24))
    );
    assert!(!temp.exists(), "the temporary name is gone");
    assert!(work.join(&audio.file).is_file());
    let head = read_peaks_head(&store.peaks_path(&audio.uid).unwrap()).unwrap();
    assert_eq!((head.channels, head.frames), (2, 3_000));
    assert!(store.generation() > before, "the store hears of it");

    let again = adopt(&store, &temp, "x.wav", &[]);
    assert!(again.is_err(), "no file");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_file_that_does_not_read_is_taken_away() {
    let root = temp_dir("bounced-unread");
    let store = store(&root);
    let work = store.work_dir().unwrap();
    std::fs::create_dir_all(&work).unwrap();
    let temp = work.join(".bounce-1-3.wav");
    std::fs::write(&temp, b"not a wav").unwrap();
    assert!(adopt(&store, &temp, "a.wav", &[]).is_err());
    assert!(!temp.exists());
    assert!(!work.join("a.wav").exists());
    let _ = std::fs::remove_dir_all(root);
}

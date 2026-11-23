use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::Duration;

use meloet_model::{AudioId, AudioSource, ClipId, PlayedFrames, TakeId};
use sha2::{Digest, Sha256};

use super::*;
use crate::audiofiles::keeper::{AudioNews, Keeper};
use crate::audiofiles::tests::write_wav;
use crate::peaks::read_peaks_head;
use crate::settings::temp_dir;

fn store(root: &Path) -> AudioStore {
    AudioStore::new(Some(root.join("state"))).with_cache(Some(root.join("cache")))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn source(file: &str, frames: u64, rate: u32, channels: u8, bits: Option<u8>) -> Arc<AudioSource> {
    Arc::new(AudioSource {
        id: AudioId(3),
        uid: [file.len() as u8; 16],
        file: file.to_owned(),
        frames,
        sample_rate: rate,
        channels,
        bits,
        sha256: None,
    })
}

fn played(source: &Arc<AudioSource>, from: i64, to: i64, audio_end: u64) -> PlayedFrames {
    PlayedFrames {
        clip: ClipId(1),
        take: TakeId(2),
        source: source.clone(),
        from,
        to,
        audio_end,
    }
}

/// The samples of the wav file, as written: the bytes after its 44 of head.
fn data(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap()[44..].to_vec()
}

fn sha(path: &Path) -> Vec<u8> {
    Sha256::digest(std::fs::read(path).unwrap()).to_vec()
}

fn frames_backwards(bytes: &[u8], size: usize) -> Vec<u8> {
    bytes.chunks_exact(size).rev().flatten().copied().collect()
}

fn floats(path: &Path) -> Vec<f32> {
    let mut wav = WavPcm::open(path).unwrap();
    let mut all = vec![0.0; wav.frames() as usize * usize::from(wav.channels())];
    wav.read(0, &mut all).unwrap();
    all
}

#[test]
fn the_loudest_sample_is_of_the_frames_the_clip_plays_and_no_others() {
    let root = temp_dir("clips-peak");
    let store = store(&root);
    let work = store.work_dir().unwrap();
    std::fs::create_dir_all(&work).unwrap();
    // A quiet sine, and two loud samples: one just before the clip's start, in the same 256
    // frames of the peaks, and one inside.
    let value = |i: usize, _| match i {
        1_000 => 0.9,
        3_000 => -0.5,
        _ => 0.25 * (i as f32 / 20.0).sin(),
    };
    write_wav(&work.join("歌 3.wav"), 48_000, 1, 24, 10_000, value);
    let wav = source("歌 3.wav", 10_000, 48_000, 1, Some(24));
    let peak = |from, to, end| peak(&store, &played(&wav, from, to, end)).unwrap();
    assert!((peak(1_001, 5_000, 10_000) - 0.5).abs() < 1e-6);
    // The take ends before the loud one inside.
    let quiet = peak(1_001, 5_000, 2_000);
    assert!(quiet < 0.26 && quiet > 0.24, "{quiet}");
    // The clip's head is pulled out before the file: from the file's first frame.
    assert!((peak(-500, 1_500, 10_000) - 0.9).abs() < 1e-6);
    assert_eq!(peak(10_000, 12_000, 10_000), 0.0);
    assert_eq!(peak(2_000, 3_000, 1_000), 0.0);

    // A float file says more than full scale as it is.
    write_wav(&work.join("大きい.wav"), 48_000, 2, 32, 100, |i, c| {
        if (i, c) == (40, 1) { -2.5 } else { 0.1 }
    });
    let loud = source("大きい.wav", 100, 48_000, 2, Some(32));
    assert_eq!(
        super::peak(&store, &played(&loud, 0, 100, 100)).unwrap(),
        2.5
    );

    // A file that is decoded: the same as its samples there.
    std::fs::copy(fixture("click.flac"), work.join("click.flac")).unwrap();
    let flac = source("click.flac", 44_100, 44_100, 2, Some(16));
    let (_, _, all) = super::super::tests::decode_all(&work.join("click.flac"));
    let most = |range: std::ops::Range<usize>| {
        all[range.start * 2..range.end * 2]
            .iter()
            .fold(0.0f32, |m, s| m.max(s.abs()))
    };
    let got = super::peak(&store, &played(&flac, 0, 22_000, 44_100)).unwrap();
    assert_eq!(got, most(0..22_000));
    assert!(got < 0.3);
    let got = super::peak(&store, &played(&flac, 20_000, 30_000, 44_100)).unwrap();
    assert_eq!(got, most(20_000..30_000));
    assert!(got > 0.85);

    let gone = source("ない.wav", 100, 48_000, 1, Some(24));
    assert!(super::peak(&store, &played(&gone, 0, 100, 100)).is_err());
}

#[test]
fn a_wav_is_written_backwards_sample_for_sample_and_twice_is_the_file_again() {
    let root = temp_dir("clips-reverse");
    let store = store(&root);
    let work = store.work_dir().unwrap();
    std::fs::create_dir_all(&work).unwrap();
    let path = work.join("歌 3.wav");
    write_wav(&path, 48_000, 2, 24, 20_000, |i, c| {
        ((i * 13 + c * 7) % 1_000) as f32 / 1_000.0 - 0.5
    });
    let before = sha(&path);
    let wav = source("歌 3.wav", 20_000, 48_000, 2, Some(24));
    let changed = store.generation();

    let once = reverse(&store, &played(&wav, 500, 12_500, 20_000), "歌 12.wav", &[]).unwrap();
    assert_eq!(
        (
            once.file.as_str(),
            once.frames,
            once.sample_rate,
            once.channels,
            once.bits
        ),
        ("歌 12.wav", 12_000, 48_000, 2, Some(24))
    );
    assert!(store.generation() > changed);
    let made = work.join("歌 12.wav");
    let source_data = data(&path);
    assert_eq!(
        data(&made),
        frames_backwards(&source_data[500 * 6..12_500 * 6], 6)
    );
    // The peaks are there, of the new file.
    let head = read_peaks_head(&store.peaks_path(&once.uid).unwrap()).unwrap();
    assert_eq!((head.frames, head.channels), (12_000, 2));
    // Nothing else is left in the work place, and the file it was made of is as it was.
    let mut names: Vec<String> = (std::fs::read_dir(&work).unwrap())
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["歌 12.wav", "歌 3.wav"]);
    assert_eq!(sha(&path), before);

    let new = source("歌 12.wav", once.frames, 48_000, 2, once.bits);
    let twice = reverse(&store, &played(&new, 0, 12_000, 12_000), "歌 14.wav", &[]).unwrap();
    assert_ne!(twice.uid, once.uid);
    assert_eq!(
        data(&work.join("歌 14.wav")),
        source_data[500 * 6..12_500 * 6]
    );
    assert_eq!(sha(&path), before);
}

#[test]
fn silence_where_the_take_ended_is_written_and_silence_before_the_file_is_not() {
    let root = temp_dir("clips-silence");
    let store = store(&root);
    let work = store.work_dir().unwrap();
    std::fs::create_dir_all(&work).unwrap();
    write_wav(&work.join("歌.wav"), 48_000, 1, 16, 1_000, |i, _| {
        (i as f32 + 1.0) / 2_000.0
    });
    let wav = source("歌.wav", 1_000, 48_000, 1, Some(16));
    // From 100 frames before the file to 400 past the take's end at 900.
    let made = reverse(&store, &played(&wav, -100, 1_300, 900), "歌 5.wav", &[]).unwrap();
    assert_eq!(made.frames, 1_300);
    let all = floats(&work.join(&made.file));
    assert_eq!(all.len(), 1_300);
    assert!(all[..400].iter().all(|&s| s == 0.0));
    let file = floats(&work.join("歌.wav"));
    let want: Vec<f32> = file[..900].iter().rev().copied().collect();
    assert_eq!(all[400..], want);

    // Unsigned 8-bit samples are silent at 128.
    let mut bytes = std::fs::read(work.join("歌.wav")).unwrap()[..44].to_vec();
    bytes[22..24].copy_from_slice(&1u16.to_le_bytes());
    bytes[28..32].copy_from_slice(&48_000u32.to_le_bytes());
    bytes[32..34].copy_from_slice(&1u16.to_le_bytes());
    bytes[34..36].copy_from_slice(&8u16.to_le_bytes());
    bytes[40..44].copy_from_slice(&10u32.to_le_bytes());
    bytes[4..8].copy_from_slice(&46u32.to_le_bytes());
    bytes.extend((0..10u8).map(|i| 200 + i));
    std::fs::write(work.join("8.wav"), &bytes).unwrap();
    let eight = source("8.wav", 10, 48_000, 1, Some(8));
    let made = reverse(&store, &played(&eight, 0, 14, 10), "8 2.wav", &[]).unwrap();
    assert_eq!(made.bits, Some(8));
    let data = data(&work.join("8 2.wav"));
    assert_eq!(data[..4], [128; 4]);
    assert_eq!(
        data[4..],
        [209, 208, 207, 206, 205, 204, 203, 202, 201, 200]
    );

    // Nothing that sounds: no file.
    assert!(reverse(&store, &played(&wav, 950, 1_300, 900), "歌 6.wav", &[]).is_err());
    assert!(!work.join("歌 6.wav").exists());
}

#[test]
fn a_file_that_is_decoded_is_written_backwards_as_floats() {
    let root = temp_dir("clips-decoded");
    let store = store(&root);
    let work = store.work_dir().unwrap();
    std::fs::create_dir_all(&work).unwrap();
    std::fs::copy(fixture("click.flac"), work.join("click.flac")).unwrap();
    let flac = source("click.flac", 44_100, 44_100, 2, Some(16));
    let made = reverse(
        &store,
        &played(&flac, 1_000, 30_000, 44_100),
        "click 2.wav",
        &[],
    )
    .unwrap();
    assert_eq!(
        (made.frames, made.sample_rate, made.bits),
        (29_000, 44_100, Some(32))
    );
    let (_, _, all) = super::super::tests::decode_all(&work.join("click.flac"));
    let want: Vec<f32> = (all[2_000..60_000]
        .as_chunks::<2>()
        .0
        .iter()
        .rev()
        .flatten()
        .copied())
    .collect();
    assert_eq!(floats(&work.join("click 2.wav")), want);
    // Only the new file is left beside it.
    assert_eq!(std::fs::read_dir(&work).unwrap().count(), 2);
}

#[test]
fn a_name_that_is_taken_gets_a_number() {
    let root = temp_dir("clips-names");
    let store = store(&root);
    let work = store.work_dir().unwrap();
    let song = root.join("曲");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::create_dir_all(song.join(crate::audiofiles::SONG_AUDIO)).unwrap();
    store.set_song_dir(Some(song.clone()));
    write_wav(&work.join("歌 3.wav"), 48_000, 1, 16, 100, |_, _| 0.5);
    std::fs::write(work.join("歌 12.wav"), b"x").unwrap();
    std::fs::write(
        song.join(crate::audiofiles::SONG_AUDIO).join("歌 12 2.wav"),
        b"x",
    )
    .unwrap();
    let wav = source("歌 3.wav", 100, 48_000, 1, Some(16));
    let taken = vec!["歌 12 3.wav".to_owned()];
    let made = reverse(&store, &played(&wav, 0, 100, 100), "歌 12.wav", &taken).unwrap();
    assert_eq!(made.file, "歌 12 4.wav");
    assert_eq!(std::fs::read(work.join("歌 12.wav")).unwrap(), b"x");
}

#[test]
fn the_keeper_measures_and_reverses_in_the_order_asked() {
    let root = temp_dir("clips-keeper");
    let store = Arc::new(store(&root));
    let work = store.work_dir().unwrap();
    std::fs::create_dir_all(&work).unwrap();
    write_wav(&work.join("歌 3.wav"), 48_000, 1, 16, 1_000, |i, _| {
        if i == 10 { 0.5 } else { 0.0 }
    });
    let wav = source("歌 3.wav", 1_000, 48_000, 1, Some(16));
    let gone = source("ない.wav", 1_000, 48_000, 1, Some(16));
    let (tx, rx) = mpsc::channel();
    let keeper = Keeper::spawn(store.clone(), move |news| {
        let _ = tx.send(news);
    })
    .unwrap();
    let next = || rx.recv_timeout(Duration::from_secs(30)).unwrap();
    keeper.measure(
        4,
        vec![
            played(&wav, 0, 1_000, 1_000),
            played(&gone, 0, 1_000, 1_000),
        ],
    );
    match next() {
        AudioNews::Measured { ticket: 4, peaks } => {
            assert_eq!(peaks.len(), 2);
            assert!((peaks[0].unwrap() - 0.5).abs() < 1e-4);
            assert_eq!(peaks[1], None);
        }
        news => panic!("{news:?}"),
    }
    keeper.reverse(
        5,
        vec![
            (played(&gone, 0, 1_000, 1_000), "歌 7.wav".to_owned()),
            (played(&wav, 0, 1_000, 1_000), "歌 7.wav".to_owned()),
        ],
        vec!["歌 3.wav".to_owned()],
    );
    match next() {
        AudioNews::Reversed { ticket: 5, files } => {
            assert_eq!(files[0], None);
            let made = files[1].as_ref().unwrap();
            assert_eq!((made.file.as_str(), made.frames), ("歌 7.wav", 1_000));
            let all = floats(&work.join("歌 7.wav"));
            assert!((all[989] - 0.5).abs() < 1e-4);
        }
        news => panic!("{news:?}"),
    }
}

#[test]
fn a_file_that_cannot_be_read_or_a_place_that_cannot_be_written_gives_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let mode = |path: &Path, mode: u32| {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    };
    let root = temp_dir("clips-cannot");
    let store = Arc::new(store(&root));
    let work = store.work_dir().unwrap();
    std::fs::create_dir_all(&work).unwrap();
    write_wav(&work.join("歌 3.wav"), 48_000, 1, 16, 1_000, |_, _| 0.5);
    write_wav(&work.join("歌 4.wav"), 48_000, 1, 16, 1_000, |_, _| 0.5);
    let shut = work.join("歌 4.wav");
    mode(&shut, 0o000);
    let wav = source("歌 3.wav", 1_000, 48_000, 1, Some(16));
    let unreadable = source("歌 4.wav", 1_000, 48_000, 1, Some(16));
    let (tx, rx) = mpsc::channel();
    let keeper = Keeper::spawn(store.clone(), move |news| {
        let _ = tx.send(news);
    })
    .unwrap();
    let next = || rx.recv_timeout(Duration::from_secs(30)).unwrap();
    keeper.measure(1, vec![played(&unreadable, 0, 1_000, 1_000)]);
    let news = next();
    mode(&shut, 0o644);
    match news {
        AudioNews::Measured { ticket: 1, peaks } => assert_eq!(peaks, [None]),
        news => panic!("{news:?}"),
    }
    // The work place no longer takes new files.
    mode(&work, 0o555);
    keeper.reverse(
        2,
        vec![(played(&wav, 0, 1_000, 1_000), "歌 7.wav".to_owned())],
        vec!["歌 3.wav".to_owned()],
    );
    let news = next();
    mode(&work, 0o755);
    match news {
        AudioNews::Reversed { ticket: 2, files } => assert_eq!(files, [None]),
        news => panic!("{news:?}"),
    }
    assert!(!work.join("歌 7.wav").exists());
}

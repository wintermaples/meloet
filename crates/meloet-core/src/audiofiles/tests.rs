use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use meloet_model::{AudioId, AudioSource};

use super::decode::{self, Decoder, Facts, Unread, facts};
use super::keeper::{AudioNews, Brought, Finding, Keeper, SourceState};
use super::wav::WavPcm;
use super::*;
use crate::peaks::read_peaks_head;
use crate::settings::temp_dir;

/// The files of `tests/fixtures` (made by `make.sh` there).
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// What `make.sh` made the fixtures from: 1 second at 44100 Hz, stereo. Left a 441 Hz sine
/// at 0.25 with a click at frame 22050, right the sine at half the size.
fn click_frame(i: usize) -> [f32; 2] {
    let s = 0.25 * (std::f32::consts::TAU * 441.0 * i as f32 / 44_100.0).sin();
    [if i == 22_050 { 0.9 } else { s }, s * 0.5]
}

/// Writes a wav of plain samples: 16 or 24 bits, or 32-bit float.
pub(crate) fn write_wav(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    bits: u16,
    frames: usize,
    sample: impl Fn(usize, usize) -> f32,
) {
    let bytes = usize::from(bits / 8);
    let data = frames * usize::from(channels) * bytes;
    let block = channels * bits / 8;
    let mut out = Vec::with_capacity(44 + data);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&(if bits == 32 { 3u16 } else { 1 }).to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * u32::from(block)).to_le_bytes());
    out.extend_from_slice(&block.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data as u32).to_le_bytes());
    for i in 0..frames {
        for c in 0..usize::from(channels) {
            let s = sample(i, c);
            match bits {
                16 => out.extend_from_slice(&((s * 32_767.0).round() as i16).to_le_bytes()),
                24 => out.extend_from_slice(&((s * 8_388_607.0).round() as i32).to_le_bytes()[..3]),
                _ => out.extend_from_slice(&s.to_le_bytes()),
            }
        }
    }
    std::fs::write(path, out).unwrap();
}

/// `click.wav` as `make.sh` writes it.
pub(crate) fn write_click_wav(path: &Path) {
    write_wav(path, 44_100, 2, 16, 44_100, |i, c| click_frame(i)[c]);
}

pub(crate) fn decode_all(path: &Path) -> (u32, u8, Vec<f32>) {
    let mut decoder = Decoder::open(path).unwrap();
    let mut all = Vec::new();
    let mut count = 0;
    while let Some((channels, block)) = decoder.next_block().unwrap() {
        count = channels;
        all.extend_from_slice(block);
    }
    (decoder.sample_rate(), count, all)
}

/// The frame where the left channel is largest.
fn loudest(interleaved: &[f32], channels: usize) -> usize {
    (0..interleaved.len() / channels)
        .max_by(|&a, &b| {
            interleaved[a * channels]
                .abs()
                .total_cmp(&interleaved[b * channels].abs())
        })
        .unwrap()
}

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

#[test]
fn the_four_kinds_decode_to_the_same_second() {
    let dir = temp_dir("audio-kinds");
    let wav = dir.join("click.wav");
    write_click_wav(&wav);
    // A file with its head whole says how long it is, to the frame: the silence an mp3 or
    // ogg encoder puts in front and behind is cut off (the tag of lame and of ffmpeg says
    // how much).
    for (path, bits, exact) in [
        (wav.clone(), Some(16), true),
        (fixture("click.flac"), Some(16), true),
        (fixture("click.mp3"), None, false),
        (fixture("click-ffmpeg.mp3"), None, false),
        (fixture("click.ogg"), None, false),
    ] {
        let f = facts(&path).unwrap();
        assert_eq!(
            f,
            Facts {
                frames: 44_100,
                sample_rate: 44_100,
                channels: 2,
                bits
            },
            "{}",
            path.display()
        );
        let (rate, channels, all) = decode_all(&path);
        assert_eq!((rate, channels, all.len()), (44_100, 2, 88_200));
        assert_eq!(loudest(&all, 2), 22_050, "{}", path.display());
        if exact {
            for i in [0, 1, 100, 22_049, 22_050, 44_099] {
                let want = click_frame(i);
                assert!((all[2 * i] - want[0]).abs() < 1e-4, "{i}");
                assert!((all[2 * i + 1] - want[1]).abs() < 1e-4, "{i}");
            }
        } else {
            // The right channel is the plain sine at 0.125.
            let worst = (2_000..42_000)
                .map(|i| (all[2 * i + 1] - click_frame(i)[1]).abs())
                .fold(0.0, f32::max);
            assert!(worst < 0.02, "{}: {worst}", path.display());
        }
    }
}

/// What was measured for docs/architecture.md 22-18 A2: an mp3 without the tag keeps the
/// silence of the encoder and of the decoder in front.
#[test]
fn an_mp3_without_its_tag_starts_1105_frames_late() {
    let path = fixture("click-notag.mp3");
    let f = facts(&path).unwrap();
    let (_, _, all) = decode_all(&path);
    // 40 mp3 frames of 1152.
    assert_eq!((f.frames, all.len()), (46_080, 92_160));
    assert_eq!(loudest(&all, 2), 22_050 + 1_105);
}

#[test]
fn what_is_not_audio_does_not_read() {
    let dir = temp_dir("audio-not");
    let text = dir.join("声.wav");
    std::fs::write(&text, "これは音ではありません").unwrap();
    let why = |path: &Path| facts(path).unwrap_err().why;
    // A kind meloet reads by its ending is broken; another kind is not audio it reads.
    assert_eq!(why(&text), Unread::Broken);
    let other = dir.join("声.aac");
    std::fs::write(&other, "これは音ではありません").unwrap();
    assert_eq!(why(&other), Unread::NotAudio);
    let empty = dir.join("空.wav");
    write_wav(&empty, 48_000, 1, 16, 0, |_, _| 0.0);
    assert_eq!(why(&empty), Unread::Broken);
    let many = dir.join("5.1.wav");
    write_wav(&many, 48_000, 6, 16, 100, |_, _| 0.0);
    assert_eq!(why(&many), Unread::TooManyChannels);
    let three = dir.join("3.wav");
    write_wav(&three, 48_000, 3, 24, 100, |_, _| 0.0);
    assert_eq!(why(&three), Unread::TooManyChannels);
    assert_eq!(why(&dir.join("ない.mp3")), Unread::Broken);
}

#[test]
fn a_wav_is_read_a_stretch_at_a_time_in_every_kind_of_sample() {
    let dir = temp_dir("audio-wav");
    let value = |i: usize, c: usize| ((i * 7 + c * 3) % 200) as f32 / 100.0 - 1.0;
    for (bits, step) in [(16, 1.0 / 32_767.0), (24, 1.0 / 8_388_607.0), (32, 0.0)] {
        let path = dir.join(format!("{bits}.wav"));
        write_wav(&path, 48_000, 2, bits, 1_000, value);
        let mut wav = WavPcm::open(&path).unwrap();
        assert_eq!(
            (wav.channels(), wav.frames(), wav.sample_rate(), wav.bits()),
            (2, 1_000, 48_000, bits as u8)
        );
        let mut out = [0.0f32; 20];
        assert_eq!(wav.read(995, &mut out).unwrap(), 5);
        for (k, got) in out[..10].iter().enumerate() {
            let want = value(995 + k / 2, k % 2);
            assert!(
                (got - want).abs() <= step + 4e-5,
                "{bits} bits, {k}: {got} {want}"
            );
        }
        assert_eq!(wav.read(1_000, &mut out).unwrap(), 0);
        assert_eq!(wav.read(0, &mut out).unwrap(), 10);
        // The decoder gives the same samples.
        let (_, _, all) = decode_all(&path);
        assert_eq!(all.len(), 2_000);
        assert!((all[0] - out[0]).abs() <= 4e-5 && (all[19] - out[19]).abs() <= 4e-5);
    }
    // A recording that ended early says "as long as the file" and is read to its end.
    let cut = dir.join("cut.wav");
    write_wav(&cut, 48_000, 1, 16, 300, value);
    let mut bytes = std::fs::read(&cut).unwrap();
    bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    bytes[40..44].copy_from_slice(&u32::MAX.to_le_bytes());
    bytes.truncate(44 + 2 * 250);
    std::fs::write(&cut, bytes).unwrap();
    assert_eq!(WavPcm::open(&cut).unwrap().frames(), 250);
    assert_eq!(facts(&cut).unwrap().frames, 250);
    assert!(WavPcm::open(&fixture("click.flac")).is_err());
}

fn store(root: &Path) -> AudioStore {
    AudioStore::new(Some(root.join("state"))).with_cache(Some(root.join("cache")))
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn a_file_is_copied_under_its_name_and_a_second_one_gets_a_number() {
    let root = temp_dir("audio-bring");
    let store = store(&root);
    let work = store.work_dir().unwrap();
    let from = root.join("from");
    std::fs::create_dir_all(from.join("b")).unwrap();
    let first = from.join("ドラム.wav");
    write_wav(&first, 48_000, 1, 16, 100, |i, _| i as f32 / 100.0);
    let other = from.join("b").join("ドラム.wav");
    write_wav(&other, 48_000, 1, 16, 100, |i, _| -(i as f32) / 100.0);

    let before = std::fs::read(&first).unwrap();
    let (name, sha) = store.bring_in(&first, &[]).unwrap();
    assert_eq!(name, "ドラム.wav");
    assert_eq!(sha, sha256_of(&first).unwrap());
    // The copy holds the same bytes, and the file itself is as it was.
    assert_eq!(std::fs::read(work.join(&name)).unwrap(), before);
    assert_eq!(std::fs::read(&first).unwrap(), before);
    // The same file again: the copy that is there is used.
    assert_eq!(store.bring_in(&first, &[]).unwrap().0, "ドラム.wav");
    assert_eq!(names(&work), ["ドラム.wav"]);
    // Another file of the same name.
    let (second, sha2) = store.bring_in(&other, &[]).unwrap();
    assert_eq!(second, "ドラム 2.wav");
    assert_ne!(sha, sha2);
    assert_eq!(store.bring_in(&other, &[]).unwrap().0, "ドラム 2.wav");
    // A name the song's table has for a file that is not found is not given either.
    let third = from.join("声.mp3");
    std::fs::write(&third, b"x").unwrap();
    let taken = ["声.mp3".to_owned(), "声 2.mp3".to_owned()];
    assert_eq!(store.bring_in(&third, &taken).unwrap().0, "声 3.mp3");
    assert_eq!(names(&work), ["ドラム 2.wav", "ドラム.wav", "声 3.mp3"]);
    // A file of that name in the song's folder counts as well.
    let song = root.join("songs/曲");
    std::fs::create_dir_all(song.join(SONG_AUDIO)).unwrap();
    std::fs::copy(&other, song.join(SONG_AUDIO).join("歌.wav")).unwrap();
    store.set_song_dir(Some(song.clone()));
    let fourth = from.join("歌.wav");
    std::fs::copy(&first, &fourth).unwrap();
    assert_eq!(store.bring_in(&fourth, &[]).unwrap().0, "歌 2.wav");
    assert_eq!(
        store.find("歌.wav"),
        Some(song.join(SONG_AUDIO).join("歌.wav"))
    );
    assert_eq!(store.find("歌 2.wav"), Some(work.join("歌 2.wav")));
    assert_eq!(store.find("../歌.wav"), None);
    assert_eq!(store.find("ない.wav"), None);
}

/// Every file of `names` is whole in one of the places the song looks in.
fn all_found(store: &AudioStore, names: &[String], bytes: &[Vec<u8>]) {
    for (name, want) in names.iter().zip(bytes) {
        let path = store
            .find(name)
            .unwrap_or_else(|| panic!("{name} is found nowhere"));
        assert_eq!(&std::fs::read(path).unwrap(), want, "{name}");
    }
}

#[test]
fn a_save_moves_the_audio_into_the_folder_and_a_save_cut_short_loses_nothing() {
    let root = temp_dir("audio-settle");
    let store = store(&root);
    let work = store.work_dir().unwrap();
    std::fs::create_dir_all(&work).unwrap();
    let names: Vec<String> = ["歌 5.wav", "ドラム.mp3", "ベース.ogg"]
        .map(String::from)
        .to_vec();
    let bytes: Vec<Vec<u8>> = (0..3u8).map(|i| vec![i; 5_000 + usize::from(i)]).collect();
    for (name, bytes) in names.iter().zip(&bytes) {
        std::fs::write(work.join(name), bytes).unwrap();
    }
    // What only the history names stays in the work place.
    std::fs::write(work.join("取り消した歌 9.wav"), b"undone").unwrap();
    let song = root.join("songs/はじめての曲");
    std::fs::create_dir_all(&song).unwrap();
    let audio = song.join(SONG_AUDIO);

    // Cut short before the first file, and after the first: wherever it stops, every file
    // is whole in one of the two places.
    assert!(store.settle_until(&song, &names, 0, &mut |_| {}).is_err());
    all_found(&store, &names, &bytes);
    assert!(store.settle_until(&song, &names, 1, &mut |_| {}).is_err());
    assert_eq!(super::tests::names(&audio), ["歌 5.wav"]);
    all_found(&store, &names, &bytes);
    // A name listed twice, a name that is no plain name and one that is found nowhere.
    let mut asked = names.clone();
    asked.extend(["歌 5.wav", "../x.wav", "ない.wav"].map(String::from));
    store.settle(&song, &asked, &mut |_| {}).unwrap();
    assert_eq!(
        super::tests::names(&audio),
        ["ドラム.mp3", "ベース.ogg", "歌 5.wav"]
    );
    assert_eq!(super::tests::names(&work), ["取り消した歌 9.wav"]);
    all_found(&store, &names, &bytes);
    store.song_moved(song.clone());
    assert_eq!(store.find("歌 5.wav"), Some(audio.join("歌 5.wav")));
    assert_eq!(
        store.find("取り消した歌 9.wav"),
        Some(work.join("取り消した歌 9.wav"))
    );
    // Again: nothing to do.
    store.settle(&song, &names, &mut |_| {}).unwrap();

    // A save under another name copies; the folder before is left as it is, and stays a
    // place to look in for what only the history names.
    std::fs::write(audio.join("前の歌 2.wav"), b"only the history").unwrap();
    let copy = root.join("songs/別の名前");
    std::fs::create_dir_all(&copy).unwrap();
    std::fs::write(work.join("新しい歌 11.wav"), b"new").unwrap();
    let mut saved = names.clone();
    saved.push("新しい歌 11.wav".into());
    store.settle(&copy, &saved, &mut |_| {}).unwrap();
    assert_eq!(
        super::tests::names(&copy.join(SONG_AUDIO)),
        ["ドラム.mp3", "ベース.ogg", "新しい歌 11.wav", "歌 5.wav"]
    );
    assert_eq!(
        super::tests::names(&audio),
        ["ドラム.mp3", "ベース.ogg", "前の歌 2.wav", "歌 5.wav"]
    );
    store.song_moved(copy.clone());
    assert_eq!(
        store.find("歌 5.wav"),
        Some(copy.join(SONG_AUDIO).join("歌 5.wav"))
    );
    assert_eq!(store.find("前の歌 2.wav"), Some(audio.join("前の歌 2.wav")));
    // Undone and saved again: the file comes over from the folder before.
    saved.push("前の歌 2.wav".into());
    store.settle(&copy, &saved, &mut |_| {}).unwrap();
    assert!(copy.join(SONG_AUDIO).join("前の歌 2.wav").is_file());
    assert!(audio.join("前の歌 2.wav").is_file());

    // Another song: the folders of the one before are no place to look in.
    store.set_song_dir(None);
    assert_eq!(store.find("歌 5.wav"), None);
    // The work place is emptied of files, and of nothing else.
    std::fs::create_dir_all(work.join("sub")).unwrap();
    store.clear_work();
    assert_eq!(super::tests::names(&work), ["sub"]);
    assert_eq!(super::tests::names(&audio).len(), 4);
}

#[test]
fn a_save_says_how_far_it_is_at_most_four_times_a_second() {
    let mut said = Vec::new();
    let mut tell = |s: Settling| said.push(s);
    let mut teller = SettlingTeller {
        tell: &mut tell,
        now: Settling {
            done: 0,
            total: 2,
            done_bytes: 0,
            total_bytes: 300,
        },
        // A quarter of a second has gone by.
        told: Instant::now() - SETTLING_EVERY,
    };
    teller.bytes(100);
    // Not again so soon.
    teller.bytes(50);
    // A file moved by a rename counts all of its bytes when it is in place.
    teller.file_done(200, 150);
    teller.told = Instant::now() - SETTLING_EVERY;
    teller.file_done(100, 0);
    let at = |done, done_bytes| Settling {
        done,
        total: 2,
        done_bytes,
        total_bytes: 300,
    };
    assert_eq!(said, [at(0, 100), at(2, 300)]);

    // Quick work says nothing.
    let root = temp_dir("audio-settling");
    let store = store(&root);
    let work = store.work_dir().unwrap();
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join("歌 1.wav"), b"a").unwrap();
    let mut said = 0;
    let song = root.join("曲");
    std::fs::create_dir_all(&song).unwrap();
    (store.settle(&song, &["歌 1.wav".to_owned()], &mut |_| said += 1)).unwrap();
    assert_eq!(said, 0);
    assert!(song.join(SONG_AUDIO).join("歌 1.wav").is_file());
}

#[test]
fn a_move_across_file_systems_is_a_copy_that_is_whole_before_the_original_goes() {
    let root = temp_dir("audio-move");
    let (from, to) = (root.join("a.wav"), root.join("b.wav"));
    std::fs::write(&from, vec![7u8; 100_000]).unwrap();
    move_file(&from, &to, true, &mut |_| {}).unwrap();
    assert!(!from.exists());
    assert_eq!(std::fs::read(&to).unwrap(), vec![7u8; 100_000]);
    assert_eq!(names(&root), ["b.wav"]);
    // The copy cannot be made: the original stays.
    let nowhere = root.join("no/such/folder/c.wav");
    assert!(move_file(&to, &nowhere, true, &mut |_| {}).is_err());
    assert!(to.is_file());
}

fn source(id: u32, file: &ImportedFile) -> Arc<AudioSource> {
    Arc::new(AudioSource {
        id: AudioId(id),
        uid: file.audio.uid,
        file: file.audio.file.clone(),
        frames: file.audio.frames,
        sample_rate: file.audio.sample_rate,
        channels: file.audio.channels,
        bits: file.audio.bits,
        sha256: Some(file.sha256),
    })
}

type ImportedFile = meloet_model::ImportedAudio;

fn next(rx: &Receiver<AudioNews>) -> AudioNews {
    rx.recv_timeout(Duration::from_secs(60))
        .expect("the audio thread answers")
}

fn keeper(store: &Arc<AudioStore>) -> (Keeper, Receiver<AudioNews>) {
    let (tx, rx) = mpsc::channel();
    let keeper = Keeper::spawn(store.clone(), move |news| {
        let _ = tx.send(news);
    })
    .unwrap();
    (keeper, rx)
}

#[test]
fn files_are_brought_in_made_ready_and_played_at_the_rate_of_the_engine() {
    let root = temp_dir("audio-keeper");
    let store = Arc::new(store(&root));
    let (keeper, rx) = keeper(&store);
    let from = root.join("from");
    std::fs::create_dir_all(&from).unwrap();
    let wav = from.join("click.wav");
    write_click_wav(&wav);
    let straight = from.join("48k.wav");
    write_wav(&straight, 48_000, 1, 24, 48_000, |i, _| {
        if i == 24_000 { 0.9 } else { 0.0 }
    });
    let text = from.join("声.wav");
    std::fs::write(&text, "音ではない").unwrap();
    let paths = vec![
        wav,
        fixture("click.flac"),
        fixture("click.mp3"),
        fixture("click.ogg"),
        text,
        straight,
    ];
    keeper.bring(7, paths, Vec::new());
    let AudioNews::Brought { ticket: 7, files } = next(&rx) else {
        panic!("not the answer to the bring");
    };
    assert_eq!(files.len(), 6);
    assert_eq!(
        files[4],
        Brought::Unreadable("声.wav".into(), decode::Unread::Broken)
    );
    let read: Vec<ImportedFile> = (files.into_iter())
        .filter_map(|f| match f {
            Brought::In(file) => Some(file),
            Brought::Unreadable(..) => None,
        })
        .collect();
    assert_eq!(
        read.iter()
            .map(|f| (f.audio.file.as_str(), f.name.as_str()))
            .collect::<Vec<_>>(),
        [
            ("click.wav", "click"),
            ("click.flac", "click"),
            ("click.mp3", "click"),
            ("click.ogg", "click"),
            ("48k.wav", "48k")
        ]
    );
    let work = store.work_dir().unwrap();
    assert_eq!(
        names(&work),
        [
            "48k.wav",
            "click.flac",
            "click.mp3",
            "click.ogg",
            "click.wav"
        ]
    );
    for f in &read[..4] {
        assert_eq!(
            (f.audio.frames, f.audio.sample_rate, f.audio.channels),
            (44_100, 44_100, 2)
        );
        assert_eq!(f.sha256, sha256_of(&work.join(&f.audio.file)).unwrap());
    }
    // The same bytes get the same key, other bytes another.
    assert_ne!(read[0].audio.uid, read[1].audio.uid);

    let sources: Vec<Arc<AudioSource>> = read
        .iter()
        .enumerate()
        .map(|(i, f)| source(10 + i as u32, f))
        .collect();
    let table = sources.iter().map(|s| (s.uid, s.file.clone())).collect();
    store.set_sources(table);
    // Nothing plays yet but the wav that is at the rate already.
    for (i, s) in sources.iter().enumerate() {
        assert_eq!(store.open(&s.uid, 48_000).is_some(), i == 4, "{}", s.file);
    }
    let before = store.generation();
    keeper.watch(3, sources.clone(), Some(48_000));
    let mut done = 0;
    let mut growing = 0;
    while done < 5 {
        match next(&rx) {
            AudioNews::States { serial, states } => {
                assert_eq!(serial, 3);
                let want: Vec<_> = (sources.iter().enumerate())
                    .map(|(i, s)| {
                        (
                            s.uid,
                            if i == 4 {
                                SourceState::Ready
                            } else {
                                SourceState::Loading
                            },
                        )
                    })
                    .collect();
                assert_eq!(states, want);
            }
            AudioNews::Peaks { .. } => growing += 1,
            AudioNews::Progress { percent, .. } => assert!(percent <= 99),
            AudioNews::Done { ok, .. } => {
                assert!(ok);
                done += 1;
            }
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(growing, 5);
    assert!(store.generation() > before);
    // Every file plays at 48 kHz now: one second, the click in its middle.
    for s in &sources {
        let mut pcm = store
            .open(&s.uid, 48_000)
            .unwrap_or_else(|| panic!("{}", s.file));
        assert_eq!(pcm.frames(), 48_000, "{}", s.file);
        let channels = usize::from(pcm.channels());
        assert_eq!(channels, usize::from(s.channels));
        let mut all = vec![0.0f32; 48_000 * channels];
        assert_eq!(pcm.read(0, &mut all).unwrap(), 48_000);
        assert_eq!(loudest(&all, channels), 24_000, "{}", s.file);
        // Their peaks are those of the file as it is, in its own frames.
        let head = read_peaks_head(&store.peaks_path(&s.uid).unwrap()).unwrap();
        assert_eq!(
            (head.frames, head.channels),
            (s.frames, s.channels),
            "{}",
            s.file
        );
        assert_eq!(head.counts[0], s.frames.div_ceil(256) as u32);
    }
    assert_eq!(names(&root.join("cache/audio")).len(), 4);
    // At the rate of the files themselves the wav plays as it is; the others were not
    // made for that rate.
    assert!(store.open(&sources[0].uid, 44_100).is_some());
    assert!(store.open(&sources[2].uid, 44_100).is_none());
    assert!(store.open(&[0; 16], 48_000).is_none());

    // Asked again: everything is ready, and nothing is made twice.
    keeper.watch(4, sources.clone(), Some(48_000));
    let AudioNews::States { states, .. } = next(&rx) else {
        panic!("not the states");
    };
    assert!(states.iter().all(|(_, state)| *state == SourceState::Ready));

    // A file that is gone is missing, and is found again from another place.
    std::fs::remove_file(work.join("click.ogg")).unwrap();
    keeper.watch(5, sources.clone(), Some(48_000));
    let AudioNews::States { states, .. } = next(&rx) else {
        panic!("not the states");
    };
    assert_eq!(states[3], (sources[3].uid, SourceState::Missing));
    assert!(store.open(&sources[3].uid, 48_000).is_none());
    // Not that audio: another rate.
    keeper.find(sources[3].clone(), work.join("48k.wav"));
    assert_eq!(
        next(&rx),
        AudioNews::Found {
            audio: 13,
            file: "48k.wav".into(),
            found: Finding::Mismatch
        }
    );
    keeper.find(sources[3].clone(), fixture("click.ogg"));
    assert_eq!(
        next(&rx),
        AudioNews::Found {
            audio: 13,
            file: "click.ogg".into(),
            found: Finding::Taken
        }
    );
    assert!(store.open(&sources[3].uid, 48_000).is_some());

    // With the work place emptied nothing is found; without a stream nothing is made.
    store.clear_work();
    keeper.watch(6, sources, None);
    let AudioNews::States { states, .. } = next(&rx) else {
        panic!("not the states");
    };
    assert!(
        states
            .iter()
            .all(|(_, state)| *state == SourceState::Missing)
    );
    assert!(names(&work).is_empty());
}

#[test]
fn a_long_file_says_how_far_it_is_and_work_for_a_song_that_is_gone_is_given_up() {
    let root = temp_dir("audio-long");
    let store = Arc::new(store(&root));
    // A release build makes the four minutes ready faster than it says how far it is twice.
    let (tx, rx) = mpsc::channel();
    let tell = move |news| {
        let _ = tx.send(news);
    };
    let keeper = Keeper::spawn_paused(store.clone(), Duration::from_micros(150), tell).unwrap();
    let from = root.join("from");
    std::fs::create_dir_all(&from).unwrap();
    // Four minutes at 44.1 kHz.
    let long = from.join("long.wav");
    write_wav(&long, 44_100, 1, 16, 240 * 44_100, |i, _| {
        ((i % 100) as f32 - 50.0) / 100.0
    });
    keeper.bring(1, vec![long], Vec::new());
    let AudioNews::Brought { files, .. } = next(&rx) else {
        panic!("not the answer to the bring");
    };
    let Brought::In(file) = &files[0] else {
        panic!("the file reads");
    };
    let long = source(1, file);
    keeper.watch(1, vec![long.clone()], Some(48_000));
    let mut percents = Vec::new();
    let started = std::time::Instant::now();
    loop {
        match next(&rx) {
            AudioNews::Progress { percent, .. } => {
                percents.push((percent, started.elapsed()));
                if percents.len() == 2 {
                    break;
                }
            }
            AudioNews::Done { .. } => panic!("done before it said how far it is"),
            _ => {}
        }
    }
    // Going up, and no more often than a few times a second.
    assert!(percents[0].0 < percents[1].0);
    assert!(percents[1].1 - percents[0].1 >= Duration::from_millis(200));
    // Another song: the work is given up, and the half-made file is gone.
    keeper.watch(2, Vec::new(), Some(48_000));
    loop {
        if let AudioNews::States { serial: 2, states } = next(&rx) {
            assert!(states.is_empty());
            break;
        }
    }
    assert!(names(&root.join("cache/audio")).is_empty());
    assert!(store.open(&long.uid, 48_000).is_none());
}

/// Not a measurement, only the order of it (run alone: `cargo test -p meloet-core --lib
/// how_long -- --ignored --nocapture`): what bringing in a five-minute mp3 costs. The mp3
/// is made here with ffmpeg.
#[test]
#[ignore = "needs ffmpeg, and prints times"]
fn how_long_a_five_minute_mp3_takes() {
    let root = temp_dir("audio-time");
    let mp3 = root.join("five.mp3");
    let made = std::process::Command::new("ffmpeg")
        .args(["-loglevel", "error", "-y", "-f", "lavfi", "-i"])
        .arg("anoisesrc=d=300:c=pink:r=44100:a=0.3")
        .args(["-ac", "2", "-c:a", "libmp3lame", "-b:a", "192k"])
        .arg(&mp3)
        .status()
        .expect("ffmpeg runs");
    assert!(made.success());
    let store = Arc::new(store(&root));
    let (keeper, rx) = keeper(&store);
    let started = std::time::Instant::now();
    keeper.bring(1, vec![mp3], Vec::new());
    let AudioNews::Brought { files, .. } = next(&rx) else {
        panic!("not the answer to the bring");
    };
    eprintln!("copied, hashed and counted in {:?}", started.elapsed());
    let Brought::In(file) = &files[0] else {
        panic!("the file reads");
    };
    assert_eq!(file.audio.frames, 300 * 44_100);
    let started = std::time::Instant::now();
    keeper.watch(1, vec![source(1, file)], Some(48_000));
    let mut told = 0;
    loop {
        match next(&rx) {
            AudioNews::Progress { .. } => told += 1,
            AudioNews::Done { ok, .. } => {
                assert!(ok);
                break;
            }
            _ => {}
        }
    }
    eprintln!(
        "decoded, resampled to 48 kHz and peaks made in {:?} ({told} percents told)",
        started.elapsed()
    );
}

/// The bytes the recording, the export and the cache wrote before they shared one head.
#[test]
fn the_head_of_a_wav_is_the_bytes_it_always_was() {
    use meloet_protocol::RecordFormat;
    let head = super::wav::wav_head;
    let cases: [(u32, u16, RecordFormat, u32, &[u8; 44]); 3] = [
        (
            48_000,
            2,
            RecordFormat::Bits24,
            0xFFFF_FFFF,
            b"RIFF\xff\xff\xff\xffWAVEfmt \x10\x00\x00\x00\x01\x00\x02\x00\x80\xbb\x00\x00\x00e\x04\x00\x06\x00\x18\x00data\xff\xff\xff\xff",
        ),
        (
            44_100,
            1,
            RecordFormat::Bits32,
            1_000,
            b"RIFF\x0c\x04\x00\x00WAVEfmt \x10\x00\x00\x00\x03\x00\x01\x00D\xac\x00\x00\x10\xb1\x02\x00\x04\x00 \x00data\xe8\x03\x00\x00",
        ),
        (
            44_100,
            2,
            RecordFormat::Bits16,
            0xFFFF_FFF0,
            b"RIFF\xff\xff\xff\xffWAVEfmt \x10\x00\x00\x00\x01\x00\x02\x00D\xac\x00\x00\x10\xb1\x02\x00\x04\x00\x10\x00data\xf0\xff\xff\xff",
        ),
    ];
    for (rate, channels, format, data, bytes) in cases {
        assert_eq!(
            &head(rate, channels, format, data),
            bytes,
            "{rate} {channels} {format:?} {data}"
        );
    }
}

#[test]
fn a_found_file_with_other_contents_is_made_ready_again_from_itself() {
    let root = temp_dir("audio-found-other");
    let store = Arc::new(store(&root));
    let (keeper, rx) = keeper(&store);
    let ogg = fixture("click.ogg");
    let source = Arc::new(AudioSource {
        id: AudioId(13),
        uid: [7; 16],
        file: "声.ogg".into(),
        frames: 44_100,
        sample_rate: 44_100,
        channels: 2,
        bits: None,
        sha256: Some(sha256_of(&ogg).unwrap()),
    });
    store.set_sources(vec![(source.uid, source.file.clone())]);
    // What was made from the file before it went missing, and what another file has.
    let made = |uid: &[u8; 16], rate: u32| {
        let path = store.cache_path(uid, rate).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_wav(&path, rate, 2, 32, 480, |_, _| 0.5);
        path
    };
    let decoded = [made(&source.uid, 48_000), made(&source.uid, 96_000)];
    let other = made(&[8; 16], 48_000);
    let peaks = store.peaks_path(&source.uid).unwrap();
    std::fs::create_dir_all(peaks.parent().unwrap()).unwrap();
    std::fs::write(&peaks, b"old peaks").unwrap();
    let found = |path: &Path| {
        keeper.find(source.clone(), path.to_owned());
        let AudioNews::Found { found, .. } = next(&rx) else {
            panic!("not found");
        };
        found
    };

    // The same bytes: what was made from them is still theirs.
    assert_eq!(found(&ogg), Finding::Taken);
    assert!(decoded.iter().all(|p| p.is_file()) && peaks.is_file());
    assert_eq!(
        store.playable(&source.uid, &source.file, 48_000),
        Playable::Ready
    );

    // Other bytes at the same rate and channels: made again from the file found.
    let work = store.work_dir().unwrap();
    std::fs::remove_file(work.join(&source.file)).unwrap();
    assert_eq!(found(&fixture("click.mp3")), Finding::Taken);
    assert!(decoded.iter().all(|p| !p.exists()) && !peaks.exists());
    assert!(other.is_file());
    assert_eq!(
        store.playable(&source.uid, &source.file, 48_000),
        Playable::NeedsCache(work.join(&source.file))
    );
}

#[test]
fn a_file_that_cannot_be_copied_in_says_so() {
    let root = temp_dir("audio-not-copied");
    // No state folder: no work place.
    let store = Arc::new(AudioStore::new(None).with_cache(Some(root.join("cache"))));
    let (keeper, rx) = keeper(&store);
    keeper.bring(
        1,
        vec![fixture("click.ogg"), root.join("ない.mp3")],
        Vec::new(),
    );
    let AudioNews::Brought { files, .. } = next(&rx) else {
        panic!("not brought");
    };
    assert_eq!(
        files,
        [
            Brought::Unreadable("click.ogg".into(), Unread::NotCopied),
            Brought::Unreadable("ない.mp3".into(), Unread::Broken)
        ]
    );
    let source = Arc::new(AudioSource {
        id: AudioId(13),
        uid: [7; 16],
        file: "声.ogg".into(),
        frames: 44_100,
        sample_rate: 44_100,
        channels: 2,
        bits: None,
        sha256: None,
    });
    keeper.find(source, fixture("click.ogg"));
    assert_eq!(
        next(&rx),
        AudioNews::Found {
            audio: 13,
            file: "click.ogg".into(),
            found: Finding::NotCopied
        }
    );
}

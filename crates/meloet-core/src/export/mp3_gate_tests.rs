//! What a written mp3 is (round 12 F2; spec of round 12, F2 「受け入れ条件」): read back with
//! symphonia, as meloet reads audio files, it is (1) as long as what was given, (2) at
//! 192 kbps 20 dB or more closer to a 1 kHz tone than the tone is loud, and no farther from
//! pink noise than the owner accepts, and (3) whole from its first frame to its last whatever
//! it was given: silence, one sample, ten minutes, given up.

use std::path::Path;
use std::time::Instant;

use meloet_protocol::{
    EXPORT_MP3_KBPS, EXPORT_MP3_SAMPLE_RATES, ExportFormat, ExportLoudness, ExportTail,
    RecordFormat,
};

use super::fixture::{BAR, Place, RATE, song};
use super::sink::{self, SinkSpec};
use super::tags::{self, SongInfo};
use super::{ExportNews, ExportTarget};
use crate::audiofiles::decode::Decoder;
use crate::settings::temp_dir;

/// What an mp3 frame of MPEG-1 Layer III holds of each channel.
const FRAME: u64 = 1_152;
/// The encoder's delay and the decoder's, as the Info tag counts them (LAME's convention).
const DELAYS: u64 = 528 + 529;

fn spec(rate: u32, kbps: u16, channels: u8) -> SinkSpec {
    SinkSpec {
        format: ExportFormat::Mp3,
        sample_rate: rate,
        channels,
        bits: RecordFormat::Bits24,
        mp3_kbps: kbps,
    }
}

/// Writes `audio` (interleaved) in pieces of `piece` frames; returns the frames counted.
fn encode(path: &Path, spec: SinkSpec, audio: &[f32], piece: usize) -> u64 {
    let mut out = sink::create(path, spec).unwrap();
    for part in audio.chunks(piece * usize::from(spec.channels)) {
        out.write(part).unwrap();
    }
    out.finish().unwrap()
}

/// The file as meloet reads audio: the rate, the channels and the samples, interleaved.
fn decode(path: &Path) -> (u32, u8, Vec<f32>) {
    let mut file = Decoder::open(path).unwrap();
    let mut out = Vec::new();
    let mut channels = 0;
    while let Some((c, block)) = file.next_block().unwrap() {
        channels = c;
        out.extend_from_slice(block);
    }
    (file.sample_rate(), channels, out)
}

/// How many audio frames an mp3 of `frames` frames has, after its Info tag.
fn audio_frames(frames: u64) -> u64 {
    (frames + DELAYS).div_ceil(FRAME).max(2)
}

/// Walks the file from its first byte to its last (past an ID3v2 tag): every frame is
/// MPEG-1 Layer III at `rate`, `kbps` and `channels`, the first the Info tag, and the last
/// one ends where the file ends. Returns the audio frames after the tag.
fn whole_frames(path: &Path, rate: u32, kbps: u16, channels: u8) -> u64 {
    const KBPS: [usize; 15] = [
        0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
    ];
    let bytes = std::fs::read(path).unwrap();
    let mut at = 0;
    if bytes.starts_with(b"ID3") {
        let size = (bytes[6..10].iter()).fold(0usize, |s, b| (s << 7) | usize::from(b & 0x7F));
        at = 10 + size + if bytes[5] & 0x10 != 0 { 10 } else { 0 };
    }
    let mut frames = 0;
    let mut first = true;
    while at < bytes.len() {
        assert!(at + 4 <= bytes.len(), "a cut frame head at {at}");
        let h = &bytes[at..at + 4];
        assert!(h[0] == 0xFF && h[1] & 0xE0 == 0xE0, "no frame at byte {at}");
        assert_eq!(h[1] & 0x1E, 0x1A, "not MPEG-1 Layer III at byte {at}");
        let bitrate = KBPS[usize::from(h[2] >> 4)];
        let hz = [44_100usize, 48_000, 32_000][usize::from((h[2] >> 2) & 3)];
        let padding = usize::from((h[2] >> 1) & 1);
        let mono = h[3] >> 6 == 3;
        assert_eq!(
            (bitrate, hz as u32, if mono { 1 } else { 2 }),
            (usize::from(kbps), rate, channels),
            "frame at byte {at}"
        );
        if first {
            let side = if mono { 17 } else { 32 };
            assert_eq!(&bytes[at + 4 + side..at + 8 + side], b"Info");
            first = false;
        } else {
            frames += 1;
        }
        at += 144_000 * bitrate / hz + padding;
    }
    assert_eq!(at, bytes.len(), "the last frame is cut");
    frames
}

/// The residual of `back` against `source`, in dB below `source`: 10 log10 of the energy of
/// the difference over the energy of the source.
fn residual_db(source: &[f32], back: &[f32]) -> f64 {
    let n = source.len().min(back.len());
    let (mut diff, mut energy) = (0.0f64, 0.0f64);
    for (s, b) in source[..n].iter().zip(&back[..n]) {
        let (s, b) = (f64::from(*s), f64::from(*b));
        diff += (b - s) * (b - s);
        energy += s * s;
    }
    10.0 * (diff / energy).log10()
}

/// The same in bands between `edges` (Hz), both channels, Hann windows of 4096 frames.
fn band_residuals_db(source: &[f32], back: &[f32], rate: u32, edges: &[f64]) -> Vec<f64> {
    use super::analysis::fft::{Complex, Fft};
    const N: usize = 4_096;
    let fft = Fft::new(N);
    let window: Vec<f64> = (0..N)
        .map(|i| 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / N as f64).cos())
        .collect();
    let bands = edges.len() - 1;
    let (mut signal, mut diff) = (vec![0.0f64; bands], vec![0.0f64; bands]);
    let frames = source.len().min(back.len()) / 2;
    let band_of = |k: usize| {
        let hz = k as f64 * f64::from(rate) / N as f64;
        (0..bands).find(|&b| hz >= edges[b] && hz < edges[b + 1])
    };
    for start in (0..frames.saturating_sub(N)).step_by(N) {
        for channel in 0..2 {
            let at = |v: &[f32], i: usize| f64::from(v[(start + i) * 2 + channel]) * window[i];
            let complex = |re| Complex { re, im: 0.0 };
            let mut s: Vec<Complex> = (0..N).map(|i| complex(at(source, i))).collect();
            let mut d: Vec<Complex> = (0..N)
                .map(|i| complex(at(back, i) - at(source, i)))
                .collect();
            fft.forward(&mut s);
            fft.forward(&mut d);
            for k in 0..N / 2 {
                if let Some(b) = band_of(k) {
                    signal[b] += s[k].norm_sqr();
                    diff[b] += d[k].norm_sqr();
                }
            }
        }
    }
    (0..bands)
        .map(|b| 10.0 * (diff[b] / signal[b]).log10())
        .collect()
}

const BAND_EDGES: [f64; 9] = [
    0.0, 4_000.0, 8_000.0, 12_000.0, 14_000.0, 16_000.0, 18_000.0, 20_000.0, 24_001.0,
];

fn bands_text(bands: &[f64]) -> String {
    let parts: Vec<String> = (BAND_EDGES.windows(2).zip(bands))
        .map(|(e, db)| format!("{}-{}k {db:.1}", e[0] / 1000.0, (e[1] / 1000.0).floor()))
        .collect();
    parts.join(", ")
}

fn tone(rate: u32, hz: f32, amplitude: f32, frames: usize) -> Vec<f32> {
    (0..frames)
        .flat_map(|i| {
            let s = (i as f32 * hz * std::f32::consts::TAU / rate as f32).sin() * amplitude;
            [s, s]
        })
        .collect()
}

/// Pink noise (Paul Kellet's filter on white noise), a channel of its own on each side, at
/// −15 dBFS RMS.
fn pink(frames: usize, seed: u32) -> Vec<f32> {
    let mut out = vec![0.0f32; frames * 2];
    for channel in 0..2 {
        let mut seed = seed.wrapping_add(channel as u32 * 0x9E37_79B9) | 1;
        let mut b = [0.0f32; 7];
        for i in 0..frames {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let white = (seed >> 8) as f32 / (1u32 << 23) as f32 - 1.0;
            b[0] = 0.99886 * b[0] + white * 0.055_517_9;
            b[1] = 0.99332 * b[1] + white * 0.075_075_9;
            b[2] = 0.969 * b[2] + white * 0.153_852;
            b[3] = 0.8665 * b[3] + white * 0.310_485_6;
            b[4] = 0.55 * b[4] + white * 0.532_952_2;
            b[5] = -0.7616 * b[5] - white * 0.016_898;
            let p = b[..6].iter().sum::<f32>() + b[6] + white * 0.5362;
            b[6] = white * 0.115_926;
            out[i * 2 + channel] = p;
        }
    }
    let squares = out.iter().map(|s| f64::from(*s).powi(2)).sum::<f64>();
    let gain = (10f64.powf(-15.0 / 20.0) / (squares / out.len() as f64).sqrt()) as f32;
    out.iter_mut().for_each(|s| *s *= gain);
    out
}

/// The left channel on both sides.
fn both(mut audio: Vec<f32>) -> Vec<f32> {
    audio.chunks_mut(2).for_each(|f| f[1] = f[0]);
    audio
}

/// Something like a song for ten minutes: chords that change, a bass, clicks and some
/// noise, the two sides apart.
fn song_like(rate: u32, seconds: usize) -> Vec<f32> {
    let frames = seconds * rate as usize;
    let noise = pink(frames, 0xC0FF_EE11);
    let roots = [220.0f32, 174.6, 261.6, 196.0];
    (0..frames)
        .flat_map(|i| {
            let t = i as f32 / rate as f32;
            let root = roots[(t / 2.0) as usize % roots.len()];
            let chord: f32 = [1.0, 1.26, 1.5]
                .iter()
                .map(|r| (t * root * r * std::f32::consts::TAU).sin())
                .sum();
            let bass = (t * root / 2.0 * std::f32::consts::TAU).sin();
            let click = if i % (rate as usize / 2) < 60 {
                0.4
            } else {
                0.0
            };
            let n = &noise[i * 2..i * 2 + 2];
            let l = 0.12 * chord + 0.25 * bass + click + 0.3 * n[0];
            let r = 0.12 * chord * 0.8 + 0.25 * bass + click * 0.6 + 0.3 * n[1];
            [l, r]
        })
        .collect()
}

#[test]
fn gate_one_a_song_exported_as_mp3_is_as_long_as_it_plays_at_44100_and_48000() {
    let place = Place::new("export-mp3-song");
    let (doc, _) = song();
    place.files_of(&doc);
    let span = (0, 6 * BAR);
    let out = place.root.join("out");
    let mut id = 0;
    let mut export = |name: &str, format, rate, bits| {
        id += 1;
        let run = place.start(place.setup(place.job(id, &doc, span, ExportTail::Off)));
        run.rendered();
        let path = out.join(name);
        run.handle.finish(ExportTarget {
            path: path.clone(),
            format,
            sample_rate: rate,
            bits,
            loudness: ExportLoudness::AsIs,
            mp3_kbps: 192,
            info: SongInfo::default(),
        });
        let seen = run.until("finished", |n| {
            matches!(n, ExportNews::Finished { .. } | ExportNews::Failed { .. })
        });
        assert!(
            matches!(seen.last(), Some(ExportNews::Finished { .. })),
            "{seen:?}"
        );
        path
    };
    for rate in EXPORT_MP3_SAMPLE_RATES {
        let floats = export(
            &format!("{rate}.wav"),
            ExportFormat::Wav,
            rate,
            RecordFormat::Bits32,
        );
        let mp3 = export(
            &format!("{rate}.mp3"),
            ExportFormat::Mp3,
            rate,
            RecordFormat::Bits24,
        );
        let (_, _, source) = decode(&floats);
        let (said, channels, back) = decode(&mp3);
        assert_eq!((said, channels), (rate, 2));
        let (want, got) = (source.len() as u64 / 2, back.len() as u64 / 2);
        eprintln!(
            "{rate} Hz: the song is {want} frames, the mp3 reads as {got}; residual {:.1} dB",
            residual_db(&source, &back)
        );
        assert!(want.abs_diff(got) <= FRAME, "{want} frames read as {got}");
        assert_eq!(whole_frames(&mp3, rate, 192, 2), audio_frames(want));
    }
}

/// The residual of pink noise at 192 kbps the owner accepts (user:2026-11-24): it comes back
/// 14.4 dB (44100 Hz) and 13.4 dB (48000 Hz) below, not the 20 dB of a tone; LAME 3.101 is
/// about the same on it. The numbers are printed; the test fails only when they get worse.
const PINK_ACCEPTED_DB: f64 = -13.0;

#[test]
fn gate_two_a_192_kbps_mp3_is_20_db_closer_for_a_tone_and_as_accepted_for_pink_noise() {
    let dir = temp_dir("export-mp3-residual");
    let path = dir.join("a.mp3");
    let mut failed = Vec::new();
    for rate in EXPORT_MP3_SAMPLE_RATES {
        for (name, audio) in [
            ("1 kHz", tone(rate, 1_000.0, 0.5, 10 * rate as usize)),
            ("pink noise", pink(10 * rate as usize, 0x1234_5678)),
            (
                "pink noise L=R",
                both(pink(10 * rate as usize, 0x1234_5678)),
            ),
        ] {
            for kbps in EXPORT_MP3_KBPS {
                encode(&path, spec(rate, kbps, 2), &audio, 4_096);
                let (said, channels, back) = decode(&path);
                assert_eq!((said, channels), (rate, 2));
                assert_eq!(back.len(), audio.len());
                let db = residual_db(&audio, &back);
                let bands = band_residuals_db(&audio, &back, rate, &BAND_EDGES);
                eprintln!(
                    "{rate} Hz, {kbps} kbps, {name}: residual {db:.1} dB ({})",
                    bands_text(&bands)
                );
                let want = if name == "1 kHz" {
                    -20.0
                } else {
                    PINK_ACCEPTED_DB
                };
                if kbps == 192 && db > want {
                    failed.push(format!("{rate} Hz, {name}: {db:.1} dB, not {want} dB"));
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(dir);
    assert!(
        failed.is_empty(),
        "at 192 kbps not as close: {}",
        failed.join("; ")
    );
}

/// For comparison only: the same signals through LAME (`/usr/bin/lame`, not a part of
/// meloet), read back the same way.
#[test]
#[ignore = "needs /usr/bin/lame; prints numbers for comparison"]
fn the_same_signals_through_lame() {
    let dir = temp_dir("export-mp3-lame");
    let (wav, mp3) = (dir.join("a.wav"), dir.join("a.mp3"));
    for rate in EXPORT_MP3_SAMPLE_RATES {
        for (name, audio) in [
            ("1 kHz", tone(rate, 1_000.0, 0.5, 10 * rate as usize)),
            ("pink noise", pink(10 * rate as usize, 0x1234_5678)),
            (
                "pink noise L=R",
                both(pink(10 * rate as usize, 0x1234_5678)),
            ),
        ] {
            let pcm = SinkSpec {
                format: ExportFormat::Wav,
                bits: RecordFormat::Bits16,
                ..spec(rate, 192, 2)
            };
            encode(&wav, pcm, &audio, 4_096);
            let (_, _, source) = decode(&wav);
            for kbps in ["192", "320"] {
                let made = std::process::Command::new("lame")
                    .args(["--quiet", "--cbr", "-b", kbps])
                    .arg(&wav)
                    .arg(&mp3)
                    .status()
                    .unwrap();
                assert!(made.success());
                let (_, _, back) = decode(&mp3);
                let db = residual_db(&source, &back);
                let bands = band_residuals_db(&source, &back, rate, &BAND_EDGES);
                eprintln!(
                    "{rate} Hz, lame --cbr -b {kbps}, {name}: residual {db:.1} dB ({}); length {} of {}",
                    bands_text(&bands),
                    back.len() / 2,
                    source.len() / 2
                );
            }
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn gate_three_silence_one_sample_nothing_and_extremes_make_whole_files() {
    let dir = temp_dir("export-mp3-extremes");
    let path = dir.join("a.mp3");
    for rate in EXPORT_MP3_SAMPLE_RATES {
        for kbps in EXPORT_MP3_KBPS {
            for channels in [1u8, 2] {
                let n = usize::from(channels);
                let square: Vec<f32> = (0..rate as usize / 2 * n)
                    .map(|i| if (i / n / 50) % 2 == 0 { 1.0 } else { -1.0 })
                    .collect();
                let mut wild = square.iter().map(|s| s * 4.0).collect::<Vec<f32>>();
                wild[10] = f32::NAN;
                wild[11] = f32::INFINITY;
                wild[12] = f32::NEG_INFINITY;
                for (what, audio) in [
                    ("silence", vec![0.0; 3 * rate as usize * n]),
                    ("one sample", vec![0.5; n]),
                    ("nothing", Vec::new()),
                    ("full scale", square),
                    ("over full scale, NaN, infinity", wild),
                ] {
                    let frames = (audio.len() / n) as u64;
                    assert_eq!(
                        encode(&path, spec(rate, kbps, channels), &audio, 333),
                        frames
                    );
                    let audio_frames_now = whole_frames(&path, rate, kbps, channels);
                    assert_eq!(audio_frames_now, audio_frames(frames), "{what}");
                    let (said, read_channels, back) = decode(&path);
                    assert_eq!(said, rate);
                    if frames > 0 {
                        assert_eq!(read_channels, channels, "{what}");
                    }
                    assert_eq!(back.len() as u64, frames * u64::from(channels), "{what}");
                    assert!(
                        back.iter().all(|s| s.is_finite() && s.abs() < 1.5),
                        "{rate} Hz {kbps} kbps {channels} ch {what}"
                    );
                    if what == "silence" {
                        assert!(back.iter().all(|s| s.abs() < 1e-4), "{what}");
                    }
                }
            }
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn gate_three_ten_minutes_make_a_whole_file() {
    let dir = temp_dir("export-mp3-long");
    let path = dir.join("a.mp3");
    let rate = 48_000;
    let audio = song_like(rate, 600);
    let started = Instant::now();
    let frames = encode(&path, spec(rate, 192, 2), &audio, 4_096);
    let took = started.elapsed();
    assert_eq!(frames, 600 * u64::from(rate));
    let bytes = std::fs::metadata(&path).unwrap().len();
    eprintln!("ten minutes at 192 kbps: {took:?} to write, {bytes} bytes");
    assert_eq!(whole_frames(&path, rate, 192, 2), audio_frames(frames));
    let (_, _, back) = decode(&path);
    assert_eq!(back.len(), audio.len());
    eprintln!("ten minutes: residual {:.1} dB", residual_db(&audio, &back));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn gate_three_a_file_given_up_is_gone_and_a_job_given_up_leaves_nothing_broken() {
    let dir = temp_dir("export-mp3-given-up");
    let path = dir.join("a.mp3");
    let mut out = sink::create(&path, spec(48_000, 192, 2)).unwrap();
    out.write(&tone(48_000, 440.0, 0.5, 48_000)).unwrap();
    assert!(path.exists());
    out.abandon();
    assert!(!path.exists());
    let _ = std::fs::remove_dir_all(dir);

    // 「やめる」 while the job brings the song into its form: the file is either not there
    // or whole.
    let place = Place::new("export-mp3-cancel");
    let (doc, _) = song();
    place.files_of(&doc);
    let to = place.root.join("out").join("a.mp3");
    for wait_ms in [0, 1, 3, 10] {
        let run =
            place.start(place.setup(place.job(1 + wait_ms, &doc, (0, 6 * BAR), ExportTail::Off)));
        run.rendered();
        let _ = std::fs::remove_file(&to);
        run.handle.finish(ExportTarget {
            path: to.clone(),
            format: ExportFormat::Mp3,
            sample_rate: RATE,
            bits: RecordFormat::Bits24,
            loudness: ExportLoudness::AsIs,
            mp3_kbps: 320,
            info: SongInfo::default(),
        });
        std::thread::sleep(std::time::Duration::from_millis(wait_ms));
        run.handle.give_up();
        let work = run.work.clone();
        run.handle.join();
        assert!(!work.exists());
        if to.exists() {
            whole_frames(&to, RATE, 320, 2);
        }
    }
}

#[test]
fn the_same_audio_in_other_pieces_is_the_same_file() {
    let dir = temp_dir("export-mp3-pieces");
    let audio = song_like(44_100, 3);
    let files: Vec<Vec<u8>> = [1usize, 333, 4_096, 1 << 20]
        .iter()
        .map(|&piece| {
            let path = dir.join(format!("{piece}.mp3"));
            encode(&path, spec(44_100, 256, 2), &audio, piece);
            std::fs::read(&path).unwrap()
        })
        .collect();
    assert!(files.windows(2).all(|w| w[0] == w[1]));
    let _ = std::fs::remove_dir_all(dir);
}

/// What symphonia says of the file: the title, the artist and the pictures.
fn symphonia_info(path: &Path) -> (Option<String>, Option<String>, Vec<Box<[u8]>>) {
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::StandardTag;
    let file = std::fs::File::open(path).unwrap();
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let mut format = symphonia::default::get_probe()
        .probe(&hint, stream, Default::default(), Default::default())
        .unwrap();
    let (mut title, mut artist, mut pictures) = (None, None, Vec::new());
    let mut metadata = format.metadata();
    let mut revisions = Vec::new();
    while let Some(revision) = metadata.pop() {
        revisions.push(revision);
    }
    revisions.extend(metadata.current().cloned());
    for revision in revisions {
        for tag in &revision.media.tags {
            match &tag.std {
                Some(StandardTag::TrackTitle(t)) => title = Some(t.to_string()),
                Some(StandardTag::Artist(a)) => artist = Some(a.to_string()),
                _ => {}
            }
        }
        pictures.extend(revision.media.visuals.iter().map(|v| v.data.clone()));
    }
    (title, artist, pictures)
}

#[test]
fn an_mp3_says_its_title_artist_and_picture_and_still_plays_as_long() {
    use lofty::file::TaggedFileExt;
    use lofty::tag::Accessor;
    let dir = temp_dir("export-mp3-tags");
    let path = dir.join("a.mp3");
    let audio = song_like(48_000, 2);
    // A small png, and a jpg of nearly 5 MiB whose bytes look like frame heads here and
    // there: the tag is passed over as a whole.
    let small = tags::tests::picture(tags::PictureKind::Png);
    let mut large = tags::tests::picture(tags::PictureKind::Jpeg);
    let mut data = large.data.to_vec();
    let mut seed = 0x2545_F491u32;
    data.extend((0..4_900_000).map(|_| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed >> 24) as u8
    }));
    data.extend([0xFF, 0xFB, 0x90, 0x64].repeat(1_000));
    large.data = data.into();
    for picture in [small, large] {
        encode(&path, spec(48_000, 192, 2), &audio, 4_096);
        let info = SongInfo {
            title: "はじめての曲".into(),
            artist: "わたし".into(),
            picture: Some(picture.clone()),
        };
        tags::write(&path, ExportFormat::Mp3, &info).unwrap();

        let file = lofty::read_from_path(&path).unwrap();
        let tag = file.primary_tag().unwrap();
        assert_eq!(tag.title().as_deref(), Some("はじめての曲"));
        assert_eq!(tag.artist().as_deref(), Some("わたし"));
        assert_eq!(tag.pictures().len(), 1);
        assert_eq!(tag.pictures()[0].data(), &picture.data[..]);

        let (title, artist, pictures) = symphonia_info(&path);
        assert_eq!(title.as_deref(), Some("はじめての曲"));
        assert_eq!(artist.as_deref(), Some("わたし"));
        assert_eq!(pictures.len(), 1);
        assert_eq!(&pictures[0][..], &picture.data[..]);

        assert_eq!(whole_frames(&path, 48_000, 192, 2), audio_frames(96_000));
        let (_, _, back) = decode(&path);
        assert_eq!(back.len(), audio.len());
        assert!(residual_db(&audio, &back) < -15.0);
    }
    let _ = std::fs::remove_dir_all(dir);
}

/// A file for the owner's ear: the song of the export's tests at 192 kbps, and the same as a
/// 24-bit wav beside it, in `target/r12-f2-ear/` of the working tree.
#[test]
#[ignore = "writes files for listening"]
fn files_for_the_ear() {
    let place = Place::new("export-mp3-ear");
    let (doc, _) = song();
    place.files_of(&doc);
    let ear = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/r12-f2-ear");
    std::fs::create_dir_all(&ear).unwrap();
    for (id, (name, format, kbps)) in [
        ("test-song-192k.mp3", ExportFormat::Mp3, 192),
        ("test-song-320k.mp3", ExportFormat::Mp3, 320),
        ("test-song-24bit.wav", ExportFormat::Wav, 192),
    ]
    .into_iter()
    .enumerate()
    {
        let run = place.start(place.setup(place.job(
            id as u64 + 1,
            &doc,
            (0, 6 * BAR),
            ExportTail::UntilSilent,
        )));
        run.rendered();
        run.handle.finish(ExportTarget {
            path: ear.join(name),
            format,
            sample_rate: RATE,
            bits: RecordFormat::Bits24,
            loudness: ExportLoudness::AsIs,
            mp3_kbps: kbps,
            info: SongInfo {
                title: "F2 test song".into(),
                artist: "meloet".into(),
                picture: None,
            },
        });
        let seen = run.until("finished", |n| {
            matches!(n, ExportNews::Finished { .. } | ExportNews::Failed { .. })
        });
        assert!(matches!(seen.last(), Some(ExportNews::Finished { .. })));
        eprintln!("wrote {}", ear.join(name).display());
    }
}

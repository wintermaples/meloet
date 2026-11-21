use std::path::Path;

use super::*;
use crate::audiofiles::decode::Decoder;
use crate::audiofiles::wav::WavPcm;
use crate::pcm::PcmFile;
use crate::settings::temp_dir;

/// A little more than two flac frames of something that is not a pattern, stereo, with
/// samples over full scale and one that is no number.
fn audio() -> Vec<f32> {
    let mut seed = 0x1234_5678u32;
    let mut out: Vec<f32> = (0..2 * 9_000)
        .map(|i| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = (seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5;
            (i as f32 * 0.01).sin() * 0.6 + noise * 0.3
        })
        .collect();
    out[100] = 1.5;
    out[101] = -1.5;
    out[102] = f32::NAN;
    out
}

fn spec(format: ExportFormat, bits: RecordFormat) -> SinkSpec {
    SinkSpec {
        format,
        sample_rate: 48_000,
        channels: 2,
        bits,
    }
}

/// Writes `audio` in pieces of `piece` samples and returns the frames the sink counted.
fn write(path: &Path, spec: SinkSpec, audio: &[f32], piece: usize) -> u64 {
    let mut sink = create(path, spec).unwrap();
    for part in audio.chunks(piece) {
        sink.write(part).unwrap();
    }
    sink.finish().unwrap()
}

fn read_wav(path: &Path) -> Vec<f32> {
    let mut wav = WavPcm::open(path).unwrap();
    assert_eq!((wav.sample_rate(), wav.channels()), (48_000, 2));
    let mut out = vec![0.0; wav.frames() as usize * 2];
    assert_eq!(wav.read(0, &mut out).unwrap(), out.len() / 2);
    out
}

fn read_flac(path: &Path) -> Vec<f32> {
    let mut flac = Decoder::open(path).unwrap();
    assert_eq!(flac.sample_rate(), 48_000);
    let mut out = Vec::new();
    while let Some((channels, block)) = flac.next_block().unwrap() {
        assert_eq!(channels, 2);
        out.extend_from_slice(block);
    }
    out
}

#[test]
fn a_wav_of_floats_holds_the_samples_as_they_are() {
    let dir = temp_dir("export-sink-f32");
    let path = dir.join("a.wav");
    let audio = audio();
    let frames = write(
        &path,
        spec(ExportFormat::Wav, RecordFormat::Bits32),
        &audio,
        1_000,
    );
    assert_eq!(frames, 9_000);
    let back = read_wav(&path);
    assert_eq!(back.len(), audio.len());
    for (i, (a, b)) in audio.iter().zip(&back).enumerate() {
        // A sample over full scale stays; one that is no number is silence.
        let a = if a.is_finite() { *a } else { 0.0 };
        assert_eq!(a.to_bits(), b.to_bits(), "sample {i}");
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn wav_and_flac_hold_the_same_whole_numbers() {
    let dir = temp_dir("export-sink-same");
    let audio = audio();
    for (bits, scale) in [
        (RecordFormat::Bits24, 8_388_608.0f32),
        (RecordFormat::Bits16, 32_768.0),
    ] {
        let (wav, flac) = (dir.join("a.wav"), dir.join("a.flac"));
        assert_eq!(
            write(&wav, spec(ExportFormat::Wav, bits), &audio, 4_410),
            9_000
        );
        // Other pieces: where the audio is cut on its way in changes nothing.
        assert_eq!(
            write(&flac, spec(ExportFormat::Flac, bits), &audio, 1_024),
            9_000
        );
        let (w, f) = (read_wav(&wav), read_flac(&flac));
        assert_eq!(w.len(), audio.len());
        assert_eq!(f.len(), audio.len());
        let whole = |v: f32| (v * scale) as i32;
        for (i, (w, f)) in w.iter().zip(&f).enumerate() {
            assert_eq!(whole(*w), whole(*f), "{bits:?} sample {i}");
        }
        if bits == RecordFormat::Bits24 {
            // 24 bits: the nearest step, nothing added.
            for (i, (a, w)) in audio.iter().zip(&w).enumerate() {
                let a = if a.is_finite() {
                    a.clamp(-1.0, 1.0)
                } else {
                    0.0
                };
                let want = (f64::from(a) * 8_388_607.0).round() as i32;
                assert_eq!(whole(*w), want, "sample {i}");
            }
        } else {
            // 16 bits: within the dither (one step each way) and the rounding.
            for (i, (a, w)) in audio.iter().zip(&w).enumerate() {
                let a = if a.is_finite() {
                    a.clamp(-1.0, 1.0)
                } else {
                    0.0
                };
                let off = (whole(*w) as f32 - a * 32_767.0).abs();
                assert!(off <= 1.5, "sample {i} is {off} steps off");
            }
        }
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_dither_of_16_bits_is_the_same_every_time_and_is_there() {
    let dir = temp_dir("export-sink-dither");
    // A level between two steps: without dither every sample would be the same number.
    let audio = vec![0.25 / 32_767.0; 2 * 4_800];
    let (a, b) = (dir.join("a.wav"), dir.join("b.wav"));
    write(
        &a,
        spec(ExportFormat::Wav, RecordFormat::Bits16),
        &audio,
        480,
    );
    write(
        &b,
        spec(ExportFormat::Wav, RecordFormat::Bits16),
        &audio,
        77,
    );
    let (bytes_a, bytes_b) = (std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    assert_eq!(bytes_a, bytes_b);
    let back = read_wav(&a);
    let steps: Vec<i32> = back.iter().map(|v| (v * 32_768.0) as i32).collect();
    assert!(steps.contains(&0) && steps.contains(&1));
    assert!(steps.iter().all(|s| (-1..=1).contains(s)));
    // The mean stays at the level that no step can say.
    let mean = steps.iter().sum::<i32>() as f32 / steps.len() as f32;
    assert!((mean - 0.25).abs() < 0.05, "mean {mean}");
    // 24 bits get no dither.
    write(
        &a,
        spec(ExportFormat::Wav, RecordFormat::Bits24),
        &audio,
        480,
    );
    let back = read_wav(&a);
    assert!(back.iter().all(|v| *v == back[0]));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_flac_says_its_length_and_a_short_one_is_whole_too() {
    let dir = temp_dir("export-sink-flac");
    let path = dir.join("a.flac");
    for frames in [1usize, 31, 4_096, 4_097] {
        let audio = vec![0.5; 2 * frames];
        let counted = write(
            &path,
            spec(ExportFormat::Flac, RecordFormat::Bits24),
            &audio,
            333,
        );
        assert_eq!(counted, frames as u64);
        let flac = Decoder::open(&path).unwrap();
        assert_eq!(flac.header_frames(), Some(frames as u64));
        assert_eq!(read_flac(&path).len(), 2 * frames);
    }
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn what_a_format_does_not_take_is_refused_and_a_file_given_up_is_gone() {
    let dir = temp_dir("export-sink-refuse");
    let path = dir.join("a.flac");
    assert!(create(&path, spec(ExportFormat::Flac, RecordFormat::Bits32)).is_err());
    assert!(!path.exists());
    for format in ExportFormat::ALL {
        let mut sink = create(&path, spec(format, RecordFormat::Bits16)).unwrap();
        sink.write(&[0.1; 64]).unwrap();
        assert!(path.exists());
        sink.abandon();
        assert!(!path.exists());
    }
    // Over what is there.
    std::fs::write(&path, vec![7u8; 100_000]).unwrap();
    write(
        &path,
        spec(ExportFormat::Flac, RecordFormat::Bits16),
        &[0.0; 64],
        64,
    );
    assert!(std::fs::metadata(&path).unwrap().len() < 1_000);
    let _ = std::fs::remove_dir_all(dir);
}

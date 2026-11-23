//! Writes the listening set of 「音程を寄せる」: each sound as it is and tuned at strength 50 and
//! 100, made the way meloet makes a tuned clip (`render_tuned`, `Preset::for_tuning`, the voice's
//! character kept).
//!
//! ```text
//! cargo run -p meloet-stretch --release --example tune_listen -- target/listen/tune [folder of 48 kHz wav files]
//! ```
//!
//! Without a folder only the made voices are written: a sung-like melody in C major (each note a
//! little off, with vibrato, slides between notes and noise for consonants and breath) and a slow
//! slide. With a folder, every WAV file in it (48 kHz, 16/24-bit PCM or 32-bit float) is tuned
//! toward the major scale that most of its notes are nearest to; the scale is printed.

use meloet_stretch::{Preset, RenderError, Stretcher, Tune, render_tuned, track_pitch};
use std::convert::Infallible;
use std::f64::consts::TAU;
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicBool;

const SR: u32 = 48_000;
const MAJOR: [usize; 7] = [0, 2, 4, 5, 7, 9, 11];
const NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

fn major(root: usize) -> [bool; 12] {
    let mut scale = [false; 12];
    for step in MAJOR {
        scale[(root + step) % 12] = true;
    }
    scale
}

fn read_wav(path: &Path) -> Result<Vec<Vec<f32>>, String> {
    let data = fs::read(path).map_err(|e| e.to_string())?;
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("not a WAV file".into());
    }
    let u16_at = |at: usize| u16::from_le_bytes([data[at], data[at + 1]]);
    let u32_at =
        |at: usize| u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
    let (mut format, mut channels, mut rate, mut bits) = (0u16, 0usize, 0u32, 0usize);
    let mut at = 12;
    while at + 8 <= data.len() {
        let (id, len) = (&data[at..at + 4], u32_at(at + 4) as usize);
        let body = at + 8;
        let end = (body + len).min(data.len());
        if id == b"fmt " {
            format = u16_at(body);
            channels = u16_at(body + 2) as usize;
            rate = u32_at(body + 4);
            bits = u16_at(body + 14) as usize;
            if format == 0xfffe {
                format = u16_at(body + 24);
            }
        } else if id == b"data" {
            if rate != SR || channels == 0 {
                return Err(format!("want {SR} Hz, got {rate} Hz, {channels} channels"));
            }
            let width = bits / 8;
            let mut out = vec![Vec::new(); channels];
            for frame in data[body..end].chunks_exact(width * channels) {
                for (c, b) in frame.chunks_exact(width).enumerate() {
                    out[c].push(match (format, bits) {
                        (1, 16) => i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0,
                        (1, 24) => {
                            (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0
                        }
                        (3, 32) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                        _ => return Err(format!("format {format} with {bits} bits")),
                    });
                }
            }
            return Ok(out);
        }
        at = body + len + (len & 1);
    }
    Err("no data".into())
}

fn write_wav(path: &Path, channels: &[Vec<f32>]) -> std::io::Result<()> {
    let frames = channels[0].len();
    let bytes = frames * channels.len() * 3;
    let mut out = Vec::with_capacity(44 + bytes);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + bytes as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&(channels.len() as u16).to_le_bytes());
    out.extend_from_slice(&SR.to_le_bytes());
    out.extend_from_slice(&(SR * channels.len() as u32 * 3).to_le_bytes());
    out.extend_from_slice(&(channels.len() as u16 * 3).to_le_bytes());
    out.extend_from_slice(&24u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(bytes as u32).to_le_bytes());
    for i in 0..frames {
        for c in channels {
            let v = (c[i].clamp(-1.0, 1.0) as f64 * 8_388_607.0).round() as i32;
            out.extend_from_slice(&v.to_le_bytes()[..3]);
        }
    }
    fs::write(path, out)
}

fn read_from(
    file: &[Vec<f32>],
) -> impl FnMut(i64, &mut [&mut [f32]]) -> Result<(), Infallible> + '_ {
    move |start, channels| {
        for (c, out) in channels.iter_mut().enumerate() {
            for (i, v) in out.iter_mut().enumerate() {
                let at = start + i as i64;
                *v = usize::try_from(at)
                    .ok()
                    .and_then(|at| file[c].get(at))
                    .copied()
                    .unwrap_or(0.0);
            }
        }
        Ok(())
    }
}

fn tuned(file: &[Vec<f32>], scale: [bool; 12], strength: f32) -> Vec<Vec<f32>> {
    let frames = file[0].len() as u64;
    let mut stretcher =
        Stretcher::new(file.len(), SR as f32, Preset::for_tuning(SR as f32)).expect("stretcher");
    stretcher.set_formant_factor(1.0, true);
    let tune = Tune {
        scale,
        strength,
        transpose: 0.0,
    };
    let mut out = vec![Vec::with_capacity(frames as usize); file.len()];
    let write = |block: &[&[f32]]| {
        for (o, b) in out.iter_mut().zip(block) {
            o.extend_from_slice(b);
        }
        Ok(())
    };
    let done: Result<(), RenderError<Infallible>> = render_tuned(
        &mut stretcher,
        frames,
        |j| j as f64,
        SR,
        &tune,
        read_from(file),
        write,
        |_| {},
        &AtomicBool::new(false),
    );
    done.expect("render");
    out
}

/// The notes of `file` that have a pitch.
fn notes_of(file: &[Vec<f32>]) -> Vec<f32> {
    let got: Result<_, RenderError<Infallible>> = track_pitch(
        SR,
        file.len(),
        0,
        file[0].len() as i64,
        read_from(file),
        |_| {},
        &AtomicBool::new(false),
    );
    let track = got.expect("pitch");
    track.notes.into_iter().filter(|n| !n.is_nan()).collect()
}

/// The root whose major scale holds the nearest semitone of the most points.
fn likely_major(notes: &[f32]) -> usize {
    let mut seen = [0usize; 12];
    for n in notes {
        seen[(n.round() as i32).rem_euclid(12) as usize] += 1;
    }
    (0..12)
        .max_by_key(|&root| MAJOR.iter().map(|s| seen[(root + s) % 12]).sum::<usize>())
        .unwrap()
}

/// How far the points are from the notes of `scale`, in cents: the median.
fn median_off(notes: &[f32], scale: [bool; 12]) -> f32 {
    let tune = Tune {
        scale,
        strength: 1.0,
        transpose: 0.0,
    };
    let mut off: Vec<f32> = notes
        .iter()
        .map(|n| tune.off_by(*n).abs() * 100.0)
        .collect();
    off.sort_by(f32::total_cmp);
    off.get(off.len() / 2).copied().unwrap_or(f32::NAN)
}

struct Rng(u32);
impl Rng {
    /// −1.0..1.0.
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.0 >> 8) as f32 / (1 << 23) as f32 - 1.0
    }
}

/// A two-pole resonator at `hz` with bandwidth `width`.
struct Formant {
    a1: f64,
    a2: f64,
    gain: f64,
    y1: f64,
    y2: f64,
}
impl Formant {
    fn new(hz: f64, width: f64) -> Formant {
        let r = (-std::f64::consts::PI * width / f64::from(SR)).exp();
        let a1 = 2.0 * r * (TAU * hz / f64::from(SR)).cos();
        Formant {
            a1,
            a2: -r * r,
            gain: 1.0 - r,
            y1: 0.0,
            y2: 0.0,
        }
    }
    fn run(&mut self, x: f64) -> f64 {
        let y = self.gain * x + self.a1 * self.y1 + self.a2 * self.y2;
        (self.y2, self.y1) = (self.y1, y);
        y
    }
}

/// A voice singing `melody` (MIDI note, seconds), each note off by `detune` cents at most.
fn sung(melody: &[(f64, f64)], detune: f32, slide: f64, seed: u32) -> Vec<f32> {
    let mut rng = Rng(seed);
    let rate = f64::from(SR);
    let mut out = Vec::new();
    // "ah": three formants.
    let mut formants = [
        Formant::new(730.0, 90.0),
        Formant::new(1090.0, 110.0),
        Formant::new(2440.0, 170.0),
    ];
    let mut phase = 0.0f64;
    let mut prev = melody[0].0;
    for (k, &(note, seconds)) in melody.iter().enumerate() {
        let off = f64::from(rng.next() * detune) / 100.0;
        let target = note + off;
        let frames = (seconds * rate) as usize;
        // Every third note starts with a consonant: 40 ms of noise, the voice 30 ms after it.
        let consonant = if k % 3 == 0 {
            (0.04 * rate) as usize
        } else {
            0
        };
        for i in 0..frames {
            let t = i as f64 / rate;
            // The pitch slides from the note before, then a vibrato grows in.
            let slid = (t / slide).min(1.0);
            let glide = prev + (target - prev) * (0.5 - 0.5 * (std::f64::consts::PI * slid).cos());
            let vibrato = 0.25 * ((t - 0.25) / 0.3).clamp(0.0, 1.0) * (TAU * 5.5 * t).sin();
            let hz = 440.0 * 2f64.powf((glide + vibrato - 69.0) / 12.0);
            phase += hz / rate;
            let harmonics = ((9000.0 / hz) as usize).max(1);
            let source: f64 = (1..=harmonics)
                .map(|h| (TAU * phase * h as f64).sin() / h as f64)
                .sum();
            let voiced_from = consonant
                + if consonant > 0 {
                    (0.03 * rate) as usize
                } else {
                    0
                };
            let attack = if consonant > 0 {
                ((i as f64 - voiced_from as f64) / (0.03 * rate)).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let release = ((frames - i) as f64 / (0.05 * rate)).min(1.0);
            let last = if k + 1 == melody.len() { release } else { 1.0 };
            let breath = f64::from(rng.next()) * 0.02;
            let voice: f64 = formants
                .iter_mut()
                .map(|f| f.run(source * attack * last + breath))
                .sum();
            let hiss = if i < consonant {
                let window = (std::f64::consts::PI * i as f64 / consonant as f64).sin();
                f64::from(rng.next()) * 0.12 * window
            } else {
                0.0
            };
            out.push((voice * 1.6 + hiss) as f32);
        }
        prev = target;
    }
    let top = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    for s in &mut out {
        *s *= 0.5 / top;
    }
    // Breath before and after.
    let mut whole: Vec<f32> = (0..SR / 2).map(|_| rng.next() * 0.01).collect();
    whole.extend(out);
    whole.extend((0..SR / 2).map(|_| rng.next() * 0.01));
    whole
}

fn set(dir: &Path, name: &str, file: &[Vec<f32>], scale: [bool; 12], scale_name: &str) {
    write_wav(&dir.join(format!("{name}-dry.wav")), file).expect("write");
    let before = median_off(&notes_of(file), scale);
    let mut line = format!("{name}: toward {scale_name}; median off the scale {before:.1} cents");
    for (label, strength) in [("50", 0.5), ("100", 1.0)] {
        let out = tuned(file, scale, strength);
        assert_eq!(out[0].len(), file[0].len());
        write_wav(&dir.join(format!("{name}-tune{label}.wav")), &out).expect("write");
        let after = median_off(&notes_of(&out), scale);
        line += &format!(", at {label} %: {after:.1}");
    }
    println!("{line}");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(out) = args.first() else {
        eprintln!("usage: tune_listen <output folder> [folder of 48 kHz wav files]");
        std::process::exit(2);
    };
    let dir = Path::new(out);
    fs::create_dir_all(dir).expect("output folder");

    // "Twinkle twinkle" in C major around middle C, and the same an octave lower.
    let line = [
        60.0, 60.0, 67.0, 67.0, 69.0, 69.0, 67.0, 65.0, 65.0, 64.0, 64.0, 62.0, 62.0, 60.0,
    ];
    let melody: Vec<(f64, f64)> = (line.iter().enumerate())
        .map(|(i, &n)| (n, if i % 7 == 6 { 1.0 } else { 0.5 }))
        .collect();
    let high = sung(&melody, 40.0, 0.06, 11);
    set(dir, "made-melody", &[high], major(0), "C major");
    let low: Vec<(f64, f64)> = melody.iter().map(|&(n, s)| (n - 12.0, s)).collect();
    set(
        dir,
        "made-melody-low",
        &[sung(&low, 40.0, 0.06, 23)],
        major(0),
        "C major",
    );
    // Slow slides between notes a third apart: a slide must stay a slide.
    let slides = [
        (60.0, 1.2),
        (64.0, 1.2),
        (67.0, 1.2),
        (64.0, 1.2),
        (60.0, 1.5),
    ];
    set(
        dir,
        "made-slides",
        &[sung(&slides, 30.0, 0.5, 5)],
        major(0),
        "C major",
    );

    if let Some(src) = args.get(1) {
        let mut voices: Vec<_> = fs::read_dir(src)
            .expect("source folder")
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "wav"))
            .collect();
        voices.sort();
        for voice in voices {
            let name = voice.file_stem().unwrap().to_string_lossy().into_owned();
            match read_wav(&voice) {
                Ok(file) => {
                    let root = likely_major(&notes_of(&file));
                    set(
                        dir,
                        &name,
                        &file,
                        major(root),
                        &format!("{} major", NAMES[root]),
                    );
                }
                Err(e) => eprintln!("{}: {e}", voice.display()),
            }
        }
    }
}

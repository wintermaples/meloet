//! Writes the listening set: every voice in a folder, stretched and transposed in the ways the
//! owner is asked to judge.
//!
//! ```text
//! python3 -I crates/meloet-stretch/listen/fetch.py                 # voices → ~/.cache/meloet/listen-src/wav
//! cargo run -p meloet-stretch --release --example listen -- ~/.cache/meloet/listen-src/wav target/listen/round7-stretch
//! ```
//!
//! Reads 48 kHz WAV files (16/24-bit PCM or 32-bit float), writes 24-bit WAV files.

use meloet_stretch::{Preset, RenderError, Stretcher, render};
use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

const SR: u32 = 48_000;

struct Sound {
    channels: Vec<Vec<f32>>,
}

fn read_wav(path: &Path) -> Result<Sound, String> {
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
                return Err(format!(
                    "want {SR} Hz, got {rate} Hz and {channels} channels"
                ));
            }
            let width = bits / 8;
            let sample = |b: &[u8]| -> Result<f32, String> {
                Ok(match (format, bits) {
                    (1, 16) => i16::from_le_bytes([b[0], b[1]]) as f32 / 32_768.0,
                    (1, 24) => {
                        (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0
                    }
                    (3, 32) => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                    _ => {
                        return Err(format!(
                            "WAV format {format} with {bits} bits is not read here"
                        ));
                    }
                })
            };
            let mut out = vec![Vec::new(); channels];
            for frame in data[body..end].chunks_exact(width * channels) {
                for (c, b) in frame.chunks_exact(width).enumerate() {
                    out[c].push(sample(b)?);
                }
            }
            return Ok(Sound { channels: out });
        }
        at = body + len + (len & 1);
    }
    Err("no data chunk".into())
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

struct Job {
    name: String,
    preset: Preset,
    /// Length of the result against the source.
    ratio: f64,
    semitones: f32,
    /// None: formants move with the pitch. Some(base): kept in place; base in Hz, 0 = found by the library.
    formant: Option<f32>,
    /// The tempo goes from 0.8 to 1.25 times over the clip instead of one ratio.
    ramp: bool,
}

fn make(sound: &Sound, job: &Job) -> Vec<Vec<f32>> {
    let channels = sound.channels.len();
    let frames = sound.channels[0].len() as f64;
    let mut st = Stretcher::new(channels, SR as f32, job.preset).expect("stretcher");
    st.set_transpose_semitones(job.semitones, None);
    match job.formant {
        Some(base) => {
            st.set_formant_factor(1.0, true);
            st.set_formant_base(base / SR as f32);
        }
        None => st.set_formant_factor(1.0, false),
    }
    // The ramp reads the file at 0.8 times the speed at first and 1.25 times at the end.
    let out_frames = if job.ramp {
        (frames / 1.025) as u64
    } else {
        (frames * job.ratio).round() as u64
    };
    let n = out_frames as f64;
    let (ratio, ramp) = (job.ratio, job.ramp);
    let p = move |j: u64| {
        let x = j as f64;
        if ramp {
            0.8 * x + 0.45 * x * x / (2.0 * n)
        } else {
            x / ratio
        }
    };
    let mut out = vec![Vec::with_capacity(out_frames as usize); channels];
    let read = |start: i64, bufs: &mut [&mut [f32]]| {
        for (c, buf) in bufs.iter_mut().enumerate() {
            let file = &sound.channels[c];
            for (i, v) in buf.iter_mut().enumerate() {
                let at = start + i as i64;
                *v = if at >= 0 && (at as usize) < file.len() {
                    file[at as usize]
                } else {
                    0.0
                };
            }
        }
        Ok(())
    };
    let write = |block: &[&[f32]]| {
        for (o, b) in out.iter_mut().zip(block) {
            o.extend_from_slice(b);
        }
        Ok(())
    };
    let done: Result<(), RenderError<Infallible>> = render(
        &mut st,
        out_frames,
        p,
        read,
        write,
        |_| {},
        &AtomicBool::new(false),
    );
    done.expect("render");
    out
}

fn jobs() -> Vec<Job> {
    // "short" is not a preset of the library: a shorter block, to hear whether voices like it better.
    let presets = [
        ("default", Preset::Default),
        ("cheaper", Preset::Cheaper),
        (
            "short",
            Preset::Blocks {
                block: 3840,
                interval: 960,
            },
        ),
    ];
    let mut jobs = Vec::new();
    for (preset_name, preset) in presets {
        for ratio in [0.75, 1.25, 1.5] {
            jobs.push(Job {
                name: format!("stretch-x{ratio}-{preset_name}"),
                preset,
                ratio,
                semitones: 0.0,
                formant: None,
                ramp: false,
            });
        }
        for semitones in [-12.0f32, -3.0, 3.0, 12.0] {
            for (formant_name, formant) in [("keep", Some(0.0)), ("move", None)] {
                jobs.push(Job {
                    name: format!("pitch{semitones:+}-{formant_name}-{preset_name}"),
                    preset,
                    ratio: 1.0,
                    semitones,
                    formant,
                    ramp: false,
                });
            }
        }
    }
    for semitones in [-12.0f32, -3.0, 3.0, 12.0] {
        jobs.push(Job {
            name: format!("pitch{semitones:+}-keep200-default"),
            preset: Preset::Default,
            ratio: 1.0,
            semitones,
            formant: Some(200.0),
            ramp: false,
        });
    }
    jobs.push(Job {
        name: "both-x1.25+3-keep-default".into(),
        preset: Preset::Default,
        ratio: 1.25,
        semitones: 3.0,
        formant: Some(0.0),
        ramp: false,
    });
    jobs.push(Job {
        name: "both-x0.8-3-keep-default".into(),
        preset: Preset::Default,
        ratio: 0.8,
        semitones: -3.0,
        formant: Some(0.0),
        ramp: false,
    });
    jobs.push(Job {
        name: "ramp-0.8to1.25-default".into(),
        preset: Preset::Default,
        ratio: 1.0,
        semitones: 0.0,
        formant: None,
        ramp: true,
    });
    jobs
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [src, out] = args.as_slice() else {
        eprintln!("usage: listen <folder of 48 kHz wav files> <output folder>");
        std::process::exit(2);
    };
    let mut voices: Vec<PathBuf> = fs::read_dir(src)
        .expect("source folder")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "wav"))
        .collect();
    voices.sort();
    if voices.is_empty() {
        eprintln!("no .wav files in {src}");
        std::process::exit(1);
    }
    let jobs = jobs();
    for voice in &voices {
        let name = voice.file_stem().unwrap().to_string_lossy();
        let sound = match read_wav(voice) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{}: {e}", voice.display());
                std::process::exit(1);
            }
        };
        let dir = Path::new(out).join(&*name);
        fs::create_dir_all(&dir).expect("output folder");
        write_wav(&dir.join("original.wav"), &sound.channels).expect("write");
        let start = std::time::Instant::now();
        for job in &jobs {
            write_wav(&dir.join(format!("{}.wav", job.name)), &make(&sound, job)).expect("write");
        }
        println!(
            "{name}: {} files in {:.1} s",
            jobs.len() + 1,
            start.elapsed().as_secs_f64()
        );
    }
}

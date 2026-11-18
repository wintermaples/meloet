//! Loads every instrument of a sound pack and writes it playing its sample phrase to a WAV file,
//! for listening. Also says how long each instrument took to read and what it added to memory.
//!
//! `cargo run --release -p meloet-sound --example render_pack -- <pack folder> <output folder>
//! [threads] [--only name,name,...] [--levels]`
//!
//! Tempo 120, two bars (4 s) and 2 s more for the sound to die away. 48 kHz, 16 bit, stereo.
//! A piano also gets `<name>-velocity.wav` (C4 sixteen times, velocity 1 to 127), strings and
//! organs get `<name>-long.wav` (C4 held for 12 s, to hear the loop). The instruments in
//! [`range_of`] get `<name>-velocity.wav` on their middle key, `<name>-range.wav` (every key of
//! their range going up, to hear the joins between zones) and, when they hold, `<name>-long.wav`;
//! [`extra_long`] adds `<name>-<note>-long.wav` for other keys.
//!
//! `--only` writes only the named instruments. `--levels` writes nothing and prints, for every
//! instrument, the peak and RMS of one C4 (a kit: the kick) at velocity 100.

mod shared;

use std::path::Path;
use std::time::Instant;

use meloet_sound::sampler::{Sampler, load_instrument, read_pack};
use shared::*;

/// Keys of the drum rows (the piano roll's kit).
const DRUM_KEYS: [u8; 12] = [36, 37, 38, 39, 42, 44, 45, 46, 47, 49, 50, 51];

/// (lowest key, highest key, holds a note) of the instruments that get the range and velocity
/// files.
fn range_of(name: &str) -> Option<(u8, u8, bool)> {
    Some(match name {
        "organQuiet" => (36, 96, true),
        "vibraphone" => (53, 89, true),
        "marimba" => (45, 96, false),
        "guitarNylon" => (40, 83, false),
        "guitarSteel" => (40, 84, false),
        "guitarElectric" => (40, 86, false),
        "guitarDistortion" => (40, 86, false),
        "pickBass" => (28, 67, false),
        "woodBass" => (28, 67, false),
        "pizzicato" => (36, 96, false),
        "trumpet" => (52, 82, true),
        "brassSection" => (41, 84, true),
        "altoSax" => (49, 80, true),
        "flute" => (60, 96, true),
        _ => return None,
    })
}

/// More held notes to hear a loop that is not the C4 one: (file name part, key).
fn extra_long(name: &str) -> &'static [(&'static str, u8)] {
    match name {
        // the sample FluidR3 names "Alto E5" (MIDI key 64), whose loop has the biggest level step
        "altoSax" => &[("E5", 64)],
        _ => &[],
    }
}

fn is_kit(name: &str) -> bool {
    let name = name.to_lowercase();
    name.contains("kit") || name.contains("drum")
}

fn is_wind(name: &str) -> bool {
    matches!(name, "trumpet" | "brassSection" | "altoSax" | "flute")
}

fn at_velocity(phrase: Phrase) -> Vec<(f64, f64, u8, u8)> {
    phrase
        .into_iter()
        .map(|(start, length, key)| (start, length, key, VELOCITY))
        .collect()
}

/// C, Am, F and G strummed in eighths, down and up, each chord as a guitar holds it. A down strum
/// goes from the lowest string up, 12 ms apart; an up strum goes back over the top four strings,
/// 8 ms apart and softer. The last strum rings for a second.
fn guitar_strum() -> Vec<(f64, f64, u8, u8)> {
    let chords: [&[u8]; 4] = [
        &[48, 52, 55, 60, 64],     // C   x32010
        &[45, 52, 57, 60, 64],     // Am  x02210
        &[41, 48, 53, 57, 60, 65], // F   133211
        &[43, 47, 50, 55, 59, 67], // G   320003
    ];
    let mut notes = Vec::new();
    for strum in 0..16 {
        let chord = chords[strum / 4];
        let start = strum as f64 * EIGHTH;
        let last = strum == 15;
        let end = if last { start + 1.0 } else { start + EIGHTH };
        if strum % 2 == 0 {
            for (i, &key) in chord.iter().enumerate() {
                let at = start + i as f64 * 0.012;
                notes.push((at, end - at, key, 100));
            }
        } else {
            for (i, &key) in chord.iter().rev().take(4).enumerate() {
                let at = start + i as f64 * 0.008;
                notes.push((at, end - at, key, 80));
            }
        }
    }
    notes
}

/// A short line from C4 over two bars; the last note is held.
fn wind_line() -> Vec<(f64, f64, u8, u8)> {
    let line: [(f64, f64, u8); 11] = [
        (0.0, 0.45, 60),
        (0.5, 0.22, 62),
        (0.75, 0.22, 64),
        (1.0, 0.45, 65),
        (1.5, 0.45, 67),
        (2.0, 0.22, 69),
        (2.25, 0.22, 67),
        (2.5, 0.22, 65),
        (2.75, 0.22, 64),
        (3.0, 0.45, 62),
        (3.5, 2.0, 60),
    ];
    at_velocity(line.to_vec())
}

/// The pad chords, plucked in quarter notes.
fn pizzicato_chords() -> Vec<(f64, f64, u8, u8)> {
    let mut notes = Vec::new();
    for (chord, keys) in [[48, 55, 60, 64, 67], [53, 57, 60, 65, 69]]
        .iter()
        .enumerate()
    {
        for beat in 0..4 {
            let start = chord as f64 * 2.0 + f64::from(beat) * 0.5;
            notes.extend(keys.iter().map(|&key| (start, 0.2, key, VELOCITY)));
        }
    }
    notes
}

fn phrase_for(name: &str) -> Vec<(f64, f64, u8, u8)> {
    let lower = name.to_lowercase();
    if lower.starts_with("guitar") {
        guitar_strum()
    } else if is_wind(name) {
        wind_line()
    } else if name == "pizzicato" {
        pizzicato_chords()
    } else if lower.contains("bass") {
        at_velocity(bass_line())
    } else if is_kit(name) {
        at_velocity(drum_beat())
    } else if lower.contains("string") || lower.contains("pad") {
        at_velocity(pad_chords())
    } else {
        at_velocity(keys_phrase())
    }
}

/// Resident memory of this process in MiB.
fn rss_mib() -> f64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let kb = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|kb| kb.parse::<f64>().ok());
    kb.unwrap_or(0.0) / 1024.0
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let (Some(pack_dir), Some(out_dir)) = (args.get(1), args.get(2)) else {
        return Err(
            "render_pack <pack folder> <output folder> [threads] [--only a,b] [--levels]".into(),
        );
    };
    let mut threads = 4;
    let mut only: Option<Vec<String>> = None;
    let mut levels = false;
    for arg in &args[3..] {
        if let Some(names) = arg.strip_prefix("--only=") {
            only = Some(names.split(',').map(str::to_string).collect());
        } else if arg == "--levels" {
            levels = true;
        } else {
            threads = arg.parse::<usize>()?;
        }
    }
    let out_dir = Path::new(out_dir);
    if !levels {
        std::fs::create_dir_all(out_dir)?;
    }

    let pack = read_pack(Path::new(pack_dir))?;
    println!("pack {}, {threads} threads", pack.pack);
    for (name, json) in &pack.instruments {
        if !levels && only.as_ref().is_some_and(|o| !o.contains(name)) {
            continue;
        }
        let before = rss_mib();
        let began = Instant::now();
        let bank = match load_instrument(json, threads) {
            Ok(bank) => bank,
            Err(error) => {
                println!("{name}: NOT LOADED: {error}");
                continue;
            }
        };
        println!(
            "{name}: read in {:.3} s, {} files, {} regions, {:.0} s of sound, samples {:.1} MiB, \
             RSS {:.1} -> {:.1} MiB",
            began.elapsed().as_secs_f64(),
            bank.sample_count(),
            bank.region_count(),
            bank.sample_seconds(),
            bank.sample_bytes() as f64 / (1024.0 * 1024.0),
            before,
            rss_mib(),
        );

        if levels {
            let key = if is_kit(name) { 36 } else { 60 };
            let mut sampler = Sampler::new(bank.clone());
            let frames = render_notes(&mut sampler, &[(0.0, 1.0, key, VELOCITY)], 2.0);
            let (peak, rms) = loudness(&frames, 1.0);
            println!(
                "  level {name} key {key} velocity {VELOCITY}: peak {peak:.1} dBFS  RMS of the first 1 s {rms:.1} dBFS"
            );
            continue;
        }

        let mut takes = vec![(format!("{name}.wav"), {
            let mut sampler = Sampler::new(bank.clone());
            render_notes(&mut sampler, &phrase_for(name), PHRASE_S + TAIL_S)
        })];
        let lower = name.to_lowercase();
        let range = range_of(name);
        let velocity_key = match range {
            Some((lo, hi, _)) => Some((lo + hi) / 2),
            None if name == "brushKit" => Some(38),
            None if lower.contains("piano") => Some(60),
            None => None,
        };
        if let Some(key) = velocity_key {
            let notes: Vec<_> = (0..16_u32)
                .map(|i| (f64::from(i) * 0.5, 0.4, key, (1 + i * 126 / 15) as u8))
                .collect();
            let mut sampler = Sampler::new(bank.clone());
            let frames = render_notes(&mut sampler, &notes, 8.0 + TAIL_S);
            takes.push((format!("{name}-velocity.wav"), frames));
        }
        let steps: Option<Vec<(f64, f64, u8, u8)>> = match range {
            Some((lo, hi, _)) => Some(
                (lo..=hi)
                    .enumerate()
                    .map(|(i, key)| (i as f64 * 0.25, 0.2, key, VELOCITY))
                    .collect(),
            ),
            None if name == "brushKit" => Some(
                DRUM_KEYS
                    .iter()
                    .enumerate()
                    .map(|(i, &key)| (i as f64 * 0.5, 0.05, key, VELOCITY))
                    .collect(),
            ),
            None => None,
        };
        if let Some(notes) = steps {
            let end = notes.last().map_or(0.0, |n| n.0 + n.1);
            let mut sampler = Sampler::new(bank.clone());
            let frames = render_notes(&mut sampler, &notes, end + TAIL_S);
            takes.push((format!("{name}-range.wav"), frames));
        }
        let holds = match range {
            Some((_, _, holds)) => holds,
            None => lower.contains("string") || lower.contains("organ"),
        };
        if holds {
            let mut sampler = Sampler::new(bank.clone());
            let frames = render_notes(&mut sampler, &[(0.0, 12.0, 60, VELOCITY)], 12.0 + TAIL_S);
            takes.push((format!("{name}-long.wav"), frames));
        }
        for &(label, key) in extra_long(name) {
            let mut sampler = Sampler::new(bank.clone());
            let frames = render_notes(&mut sampler, &[(0.0, 12.0, key, VELOCITY)], 12.0 + TAIL_S);
            takes.push((format!("{name}-{label}-long.wav"), frames));
        }
        for (file, frames) in takes {
            let path = out_dir.join(file);
            write_wav(&path, &frames)?;
            let (peak, rms) = loudness(&frames, PHRASE_S);
            println!(
                "  {}  peak {peak:.1} dBFS  RMS of the first 4 s {rms:.1} dBFS",
                path.display()
            );
        }
    }
    Ok(())
}

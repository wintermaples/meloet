//! Loads every instrument of a sound pack and writes it playing its sample phrase to a WAV file,
//! for listening. Also says how long each instrument took to read and what it added to memory.
//!
//! `cargo run --release -p meloet-sound --example render_pack -- <pack folder> <output folder> [threads]`
//!
//! Tempo 120, two bars (4 s) and 2 s more for the sound to die away. 48 kHz, 16 bit, stereo.
//! A piano also gets `<name>-velocity.wav` (C4 sixteen times, velocity 1 to 127), strings and
//! organ get `<name>-long.wav` (C4 held for 12 s, to hear the loop).

mod shared;

use std::path::Path;
use std::time::Instant;

use meloet_sound::sampler::{Sampler, load_instrument, read_pack};
use shared::*;

/// C, Am, F and G for two beats each; over the G the right hand climbs.
fn keys_phrase() -> Phrase {
    let chords: [(u8, [u8; 3]); 4] = [
        (48, [60, 64, 67]),
        (45, [60, 64, 69]),
        (41, [60, 65, 69]),
        (43, [59, 62, 67]),
    ];
    let mut phrase = Phrase::new();
    for (i, (bass, hand)) in chords.into_iter().enumerate() {
        let start = i as f64;
        phrase.push((start, 0.95, bass));
        // The last chord is short in the right hand: it goes on into the line.
        let length = if i == 3 { 0.24 } else { 0.95 };
        phrase.extend(hand.map(|key| (start, length, key)));
    }
    for (i, key) in [71, 74, 79].into_iter().enumerate() {
        let last = i == 2;
        let length = if last { 0.45 } else { 0.24 };
        phrase.push((3.0 + (i + 1) as f64 * EIGHTH, length, key));
    }
    phrase
}

fn phrase_for(name: &str) -> Phrase {
    let name = name.to_lowercase();
    if name.contains("bass") {
        bass_line()
    } else if name.contains("kit") || name.contains("drum") {
        drum_beat()
    } else if name.contains("string") || name.contains("pad") {
        pad_chords()
    } else {
        keys_phrase()
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
        return Err("render_pack <pack folder> <output folder> [threads]".into());
    };
    let threads = args.get(3).map_or(Ok(4), |n| n.parse::<usize>())?;
    let out_dir = Path::new(out_dir);
    std::fs::create_dir_all(out_dir)?;

    let pack = read_pack(Path::new(pack_dir))?;
    println!("pack {}, {threads} threads", pack.pack);
    for (name, json) in &pack.instruments {
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

        let mut takes = vec![(format!("{name}.wav"), {
            let mut sampler = Sampler::new(bank.clone());
            render(&mut sampler, &phrase_for(name))
        })];
        let lower = name.to_lowercase();
        if lower.contains("piano") {
            let notes: Vec<_> = (0..16_u32)
                .map(|i| (f64::from(i) * 0.5, 0.4, 60, (1 + i * 126 / 15) as u8))
                .collect();
            let mut sampler = Sampler::new(bank.clone());
            let frames = render_notes(&mut sampler, &notes, 8.0 + TAIL_S);
            takes.push((format!("{name}-velocity.wav"), frames));
        }
        if lower.contains("string") || lower.contains("organ") {
            let mut sampler = Sampler::new(bank.clone());
            let frames = render_notes(&mut sampler, &[(0.0, 12.0, 60, VELOCITY)], 12.0 + TAIL_S);
            takes.push((format!("{name}-long.wav"), frames));
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

//! Plays presets of a SoundFont through meloet's player and through the `fluidsynth` program
//! of this PC (started as a process of its own, never linked), writes both to WAV files for
//! listening and prints numbers to compare them: loudness, the centre of the spectrum, how
//! the loudness moves in time, and a held note's decay and release.
//!
//! `cargo run --release -p meloet-sound --example sf2_listen -- <file.sf2> <output folder>
//! [--pack <pack folder>]`
//!
//! With `--pack`, the instruments of the pack that were made from the same presets are
//! written too, playing the same phrase (`<name>-pack.wav` beside `<name>-meloet.wav`).
//! Nothing is sent to a sound device. FluidSynth runs without reverb and chorus, at the gain
//! meloet gives a SoundFont.

mod shared;

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use meloet_sound::sampler::{Sampler, load_instrument, read_pack};
use meloet_sound::sf2::{GAIN_DB, Sf2File};
use shared::*;

type Notes = Vec<(f64, f64, u8, u8)>;

/// (file name, bank, program, phrase, the pack's instrument made from the same preset)
fn cases() -> Vec<(&'static str, u16, u16, Phrase, Option<&'static str>)> {
    vec![
        ("piano", 0, 0, keys_phrase(), None),
        ("vibraphone", 0, 11, keys_phrase(), Some("vibraphone")),
        ("guitar-nylon", 0, 24, guitar_strum(), Some("guitarNylon")),
        (
            "guitar-clean",
            0,
            27,
            guitar_strum(),
            Some("guitarElectric"),
        ),
        ("strings", 0, 48, pad_chords(), None),
        ("trumpet", 0, 56, wind_line(), Some("trumpet")),
        ("brass-section", 0, 61, pad_chords(), Some("brassSection")),
        ("synth-lead", 0, 80, lead_line(), None),
        ("synth-pad", 0, 89, pad_chords(), None),
        // Presets that use the parts the others do not: a delay before the note, the vibrato
        // LFO, the modulation envelope on the pitch, and all of them at once.
        ("fretless-bass", 0, 35, bass_line(), None),
        ("synth-strings", 0, 50, pad_chords(), None),
        ("polysynth", 0, 90, keys_phrase(), None),
        ("ice-rain", 0, 96, lead_line(), None),
        ("kit-standard", 128, 0, drum_beat(), None),
        ("kit-brush", 128, 40, drum_beat(), Some("brushKit")),
    ]
}

/// FluidSynth takes a note in at the start of a block of 64 frames. When a key is struck
/// again while its last note still rings, a millisecond decides whether the two add up or
/// cancel, so meloet gets each note at the start of the next block. (Found by comparing the
/// two; FluidSynth's own rule was not read.)
fn heard_at(seconds: f64) -> f64 {
    let blocks = f64::from(RATE) / 64.0;
    (seconds * blocks).ceil() / blocks
}

fn as_heard(notes: &Notes) -> Notes {
    let moved = |&(start, length, key, velocity): &(f64, f64, u8, u8)| {
        let on = heard_at(start);
        (on, heard_at(start + length) - on, key, velocity)
    };
    notes.iter().map(moved).collect()
}

/// A MIDI file (format 0, 960 ticks a second) of the notes on one channel; drums go on
/// channel 10, where FluidSynth reads the program as the kit. The track lasts `total_s`.
fn midi_file(bank: u16, program: u16, notes: &Notes, total_s: f64) -> Vec<u8> {
    let channel: u8 = if bank == 128 { 9 } else { 0 };
    let tick = |seconds: f64| (seconds * 960.0).round() as u32;
    // (tick, order at the same tick, bytes)
    let mut events: Vec<(u32, u8, Vec<u8>)> = Vec::new();
    if bank != 128 {
        events.push((0, 0, vec![0xB0 | channel, 0, bank as u8]));
    }
    events.push((0, 1, vec![0xC0 | channel, program as u8]));
    for &(start, length, key, velocity) in notes {
        events.push((tick(start), 3, vec![0x90 | channel, key, velocity]));
        events.push((tick(start + length), 2, vec![0x80 | channel, key, 0]));
    }
    // Something at the very end, so that the tail is rendered.
    events.push((tick(total_s), 4, vec![0xB0 | channel, 0x7B, 0]));
    events.sort_by_key(|e| (e.0, e.1));

    let mut track = vec![0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20]; // 500000 us a quarter
    let mut now = 0;
    for (at, _, bytes) in events {
        let mut delta = at - now;
        now = at;
        let mut groups = vec![(delta & 0x7F) as u8];
        while delta > 0x7F {
            delta >>= 7;
            groups.push((delta & 0x7F) as u8 | 0x80);
        }
        track.extend(groups.iter().rev());
        track.extend(bytes);
    }
    track.extend([0x00, 0xFF, 0x2F, 0x00]);
    let mut file = b"MThd".to_vec();
    file.extend([0, 0, 0, 6, 0, 0, 0, 1]);
    file.extend(480_u16.to_be_bytes());
    file.extend(b"MTrk");
    file.extend((track.len() as u32).to_be_bytes());
    file.extend(track);
    file
}

/// 16-bit stereo frames of a WAV file.
fn read_wav(path: &Path) -> Result<Vec<[f32; 2]>, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    let mut pos = 12;
    while pos + 8 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into()?) as usize;
        let body = pos + 8;
        if &bytes[pos..pos + 4] == b"data" {
            let data = &bytes[body..bytes.len().min(body + size)];
            let sample = |b: &[u8]| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0;
            let frames = data.as_chunks::<4>().0.iter();
            return Ok(frames.map(|f| [sample(&f[..2]), sample(&f[2..])]).collect());
        }
        pos = body + size + (size & 1);
    }
    Err("no data in the WAV file".into())
}

fn fluidsynth(
    sf2: &Path,
    out: &Path,
    name: &str,
    bank: u16,
    program: u16,
    notes: &Notes,
    total_s: f64,
) -> Result<Vec<[f32; 2]>, Box<dyn std::error::Error>> {
    let midi = out.join(format!("{name}.mid"));
    let wav = out.join(format!("{name}-fluidsynth.wav"));
    std::fs::write(&midi, midi_file(bank, program, notes, total_s))?;
    let status = Command::new("fluidsynth")
        .args(["-ni", "-q", "-g"])
        .arg(10.0_f64.powf(f64::from(GAIN_DB) / 20.0).to_string())
        .arg("-r")
        .arg(RATE.to_string())
        .args([
            "-o",
            "synth.reverb.active=0",
            "-o",
            "synth.chorus.active=0",
            "-F",
        ])
        .arg(&wav)
        .arg(sf2)
        .arg(&midi)
        .status()?;
    if !status.success() {
        return Err(format!("fluidsynth ended with {status}").into());
    }
    let mut frames = read_wav(&wav)?;
    frames.resize((total_s * f64::from(RATE)) as usize, [0.0; 2]);
    Ok(frames)
}

/// In place, the length a power of two.
fn fft(re: &mut [f64], im: &mut [f64]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let angle = -std::f64::consts::TAU / len as f64;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (sin, cos) = (angle * k as f64).sin_cos();
                let (a, b) = (start + k, start + k + len / 2);
                let (xr, xi) = (re[b] * cos - im[b] * sin, re[b] * sin + im[b] * cos);
                (re[b], im[b]) = (re[a] - xr, im[a] - xi);
                (re[a], im[a]) = (re[a] + xr, im[a] + xi);
            }
        }
        len *= 2;
    }
}

/// The centre of the spectrum in Hz (weighted by magnitude) of the first `seconds`, both
/// sides together, from Hann windows of 4096 frames.
fn centroid(frames: &[[f32; 2]], seconds: f64) -> f64 {
    const N: usize = 4096;
    let part = &frames[..((seconds * f64::from(RATE)) as usize).min(frames.len())];
    let mut sum = vec![0.0_f64; N / 2];
    for window in part.as_chunks::<N>().0 {
        let mut re: Vec<f64> = window
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let hann = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / N as f64).cos();
                f64::from(f[0] + f[1]) * 0.5 * hann
            })
            .collect();
        let mut im = vec![0.0; N];
        fft(&mut re, &mut im);
        for (bin, total) in sum.iter_mut().enumerate() {
            *total += (re[bin] * re[bin] + im[bin] * im[bin]).sqrt();
        }
    }
    let weight: f64 = sum.iter().sum();
    let hz = |bin: usize| bin as f64 * f64::from(RATE) / N as f64;
    sum.iter()
        .enumerate()
        .map(|(bin, m)| hz(bin) * m)
        .sum::<f64>()
        / weight.max(1.0e-12)
}

/// RMS in dBFS of `from..to` seconds.
fn level(frames: &[[f32; 2]], from: f64, to: f64) -> f64 {
    let at = |s: f64| ((s * f64::from(RATE)) as usize).min(frames.len());
    let part = &frames[at(from)..at(to)];
    let power: f64 = part.iter().flatten().map(|x| f64::from(x * x)).sum();
    db((power / (part.len().max(1) * 2) as f64).sqrt())
}

/// The biggest difference in dB between the two, in steps of 0.25 s, where either is over
/// -60 dBFS.
fn furthest_apart(a: &[[f32; 2]], b: &[[f32; 2]], seconds: f64) -> f64 {
    let mut worst = 0.0_f64;
    let mut from = 0.0;
    while from < seconds {
        let (la, lb) = (level(a, from, from + 0.25), level(b, from, from + 0.25));
        if la.max(lb) > -60.0 {
            worst = worst.max((la - lb).abs());
        }
        from += 0.25;
    }
    worst
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let (Some(sf2), Some(out)) = (args.get(1), args.get(2)) else {
        return Err("sf2_listen <file.sf2> <output folder> [--pack <pack folder>]".into());
    };
    let pack_dir = args
        .iter()
        .position(|a| a == "--pack")
        .and_then(|i| args.get(i + 1));
    let (sf2, out) = (Path::new(sf2), Path::new(out));
    std::fs::create_dir_all(out)?;
    let began = std::time::Instant::now();
    let file = Sf2File::open(sf2)?;
    println!(
        "opened in {:.1} ms, {} presets",
        began.elapsed().as_secs_f64() * 1000.0,
        file.presets().len()
    );
    let pack = pack_dir.map(|dir| read_pack(Path::new(dir))).transpose()?;

    let mut phrase_rows = Vec::new();
    let mut held_rows = Vec::new();
    let mut pack_rows = Vec::new();
    for (name, bank, program, phrase, pack_name) in cases() {
        let began = std::time::Instant::now();
        let preset = Arc::new(file.load(bank, program, RATE as f32)?);
        println!(
            "{name}: read in {:.1} ms, {} zones, {} samples, {:.1} MiB",
            began.elapsed().as_secs_f64() * 1000.0,
            preset.region_count(),
            preset.sample_count(),
            preset.sample_bytes() as f64 / (1024.0 * 1024.0)
        );
        let notes: Notes = phrase
            .iter()
            .map(|&(start, length, key)| (start, length, key, VELOCITY))
            .collect();
        let total = PHRASE_S + TAIL_S;

        let phrased = render_notes(&mut Sampler::new(preset.clone()), &as_heard(&notes), total);
        write_wav(&out.join(format!("{name}-meloet.wav")), &phrased)?;
        let theirs = fluidsynth(sf2, out, name, bank, program, &notes, total)?;
        phrase_rows.push(format!(
            "| {name} | {bank}:{program} {} | {:.1} | {:.1} | {:+.1} | {:.0} | {:.0} | {:+.1} % | {:.1} |",
            preset.kind(),
            level(&theirs, 0.0, PHRASE_S),
            level(&phrased, 0.0, PHRASE_S),
            level(&phrased, 0.0, PHRASE_S) - level(&theirs, 0.0, PHRASE_S),
            centroid(&theirs, PHRASE_S),
            centroid(&phrased, PHRASE_S),
            (centroid(&phrased, PHRASE_S) / centroid(&theirs, PHRASE_S) - 1.0) * 100.0,
            furthest_apart(&phrased, &theirs, total),
        ));

        // One note held for 3 s and let go: a kit plays its snare, the others C4.
        let key = if bank == 128 { 38 } else { 60 };
        let held: Notes = vec![(0.0, 3.0, key, VELOCITY)];
        let ours = render_notes(&mut Sampler::new(preset.clone()), &as_heard(&held), 5.0);
        let held_name = format!("{name}-held");
        let theirs = fluidsynth(sf2, out, &held_name, bank, program, &held, 5.0)?;
        std::fs::remove_file(out.join(format!("{held_name}-fluidsynth.wav")))?;
        std::fs::remove_file(out.join(format!("{held_name}.mid")))?;
        let spans = [
            (0.0, 0.25),
            (1.0, 1.25),
            (2.5, 2.75),
            (3.1, 3.2),
            (3.4, 3.5),
        ];
        let cells: Vec<String> = spans
            .iter()
            .map(|&(a, b)| format!("{:.1} / {:.1}", level(&theirs, a, b), level(&ours, a, b)))
            .collect();
        held_rows.push(format!("| {name} | {} |", cells.join(" | ")));

        if let (Some(pack), Some(pack_name)) = (&pack, pack_name) {
            let json = pack.instruments.iter().find(|(n, _)| n == pack_name);
            let Some((_, json)) = json else {
                println!("{pack_name} is not in the pack");
                continue;
            };
            let bank = load_instrument(json, 4)?;
            let packed = render_notes(&mut Sampler::new(bank), &as_heard(&notes), total);
            write_wav(&out.join(format!("{name}-pack.wav")), &packed)?;
            pack_rows.push(format!(
                "| {name} | {pack_name} | {:.1} | {:.1} | {:+.1} | {:.0} | {:.0} | {:.1} |",
                level(&packed, 0.0, PHRASE_S),
                level(&phrased, 0.0, PHRASE_S),
                level(&phrased, 0.0, PHRASE_S) - level(&packed, 0.0, PHRASE_S),
                centroid(&packed, PHRASE_S),
                centroid(&phrased, PHRASE_S),
                furthest_apart(&phrased, &packed, total),
            ));
        }
    }

    println!("\nThe phrase (4 s and a tail of 2 s), velocity {VELOCITY}\n");
    println!(
        "| file | preset | RMS FluidSynth dBFS | RMS meloet dBFS | difference dB | centre FluidSynth Hz | centre meloet Hz | centre | furthest apart in 0.25 s steps dB |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    phrase_rows.iter().for_each(|row| println!("{row}"));
    println!("\nOne note held 3 s (FluidSynth / meloet, RMS dBFS)\n");
    println!(
        "| file | 0–0.25 s | 1–1.25 s | 2.5–2.75 s | 0.1–0.2 s after the key | 0.4–0.5 s after the key |"
    );
    println!("|---|---|---|---|---|---|");
    held_rows.iter().for_each(|row| println!("{row}"));
    if !pack_rows.is_empty() {
        println!("\nThe pack's instrument and the same preset read from the SoundFont\n");
        println!(
            "| file | pack | RMS pack dBFS | RMS SoundFont dBFS | difference dB | centre pack Hz | centre SoundFont Hz | furthest apart in 0.25 s steps dB |"
        );
        println!("|---|---|---|---|---|---|---|---|");
        pack_rows.iter().for_each(|row| println!("{row}"));
    }
    Ok(())
}

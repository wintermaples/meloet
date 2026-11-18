//! Writes each computed instrument playing its sample phrase to a WAV file, for listening.
//!
//! `cargo run --release -p meloet-sound --example render_synths -- target/synth-wav`
//!
//! Tempo 120, two bars (4 s) and 2 s more for the sound to die away. 48 kHz, 16 bit, stereo.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

use meloet_engine::{Instrument, InstrumentEvent};
use meloet_sound::synth::{ElectroKit, PolySynth, SineSynth};

const RATE: u32 = 48_000;
const BLOCK: usize = 64;
const EIGHTH: f64 = 0.25;
const PHRASE_S: f64 = 4.0;
const TAIL_S: f64 = 2.0;
const VELOCITY: u8 = 100;

/// (start in seconds, length in seconds, key)
type Phrase = Vec<(f64, f64, u8)>;

fn eighths(keys: &[u8], length: f64) -> Phrase {
    keys.iter()
        .enumerate()
        .map(|(i, &key)| (i as f64 * EIGHTH, length, key))
        .collect()
}

fn lead_line() -> Phrase {
    let mut line = eighths(
        &[72, 76, 79, 76, 81, 79, 76, 74, 72, 74, 76, 79, 84, 79, 76],
        0.2,
    );
    line.push((15.0 * EIGHTH, 0.45, 72));
    line
}

fn bass_line() -> Phrase {
    eighths(
        &[
            36, 36, 43, 36, 48, 36, 43, 48, 36, 36, 43, 48, 36, 43, 48, 43,
        ],
        0.21,
    )
}

fn pad_chords() -> Phrase {
    let c = [48, 55, 60, 64, 67].map(|key| (0.0, 1.95, key));
    let f = [53, 57, 60, 65, 69].map(|key| (2.0, 1.95, key));
    c.into_iter().chain(f).collect()
}

fn drum_beat() -> Phrase {
    let hit = |eighth: f64, key: u8| (eighth * EIGHTH, 0.05, key);
    let mut beat = vec![hit(0.0, 49)];
    // Bar 1 and the first half of bar 2: hi-hat on every eighth, kick on 1 and 3, snare on 2 and 4.
    for eighth in 0..12 {
        let open = eighth == 7;
        beat.push(hit(f64::from(eighth), if open { 46 } else { 42 }));
    }
    for eighth in [0.0, 3.0, 4.0, 8.0, 11.0] {
        beat.push(hit(eighth, 36));
    }
    for eighth in [2.0, 6.0, 10.0] {
        beat.push(hit(eighth, 38));
    }
    // The last two beats: sixteenths down the toms.
    for (i, key) in [50, 50, 47, 47, 45, 45, 45, 45].into_iter().enumerate() {
        beat.push(hit(12.0 + i as f64 * 0.5, key));
    }
    beat.push(hit(12.0, 36));
    beat.push(hit(14.0, 36));
    beat
}

fn render(inst: &mut dyn Instrument, phrase: &Phrase) -> Vec<[f32; 2]> {
    let frame = |seconds: f64| (seconds * f64::from(RATE)).round() as usize;
    let mut events: Vec<(usize, InstrumentEvent)> = Vec::new();
    for &(start, length, note) in phrase {
        let on = InstrumentEvent::NoteOn {
            note,
            velocity: VELOCITY,
        };
        events.push((frame(start), on));
        events.push((frame(start + length), InstrumentEvent::NoteOff { note }));
    }
    events.sort_by_key(|(at, ev)| (*at, matches!(ev, InstrumentEvent::NoteOn { .. })));

    inst.prepare(RATE as f32, BLOCK);
    let total = frame(PHRASE_S + TAIL_S);
    let mut out = Vec::with_capacity(total);
    let (mut left, mut right) = ([0.0_f32; BLOCK], [0.0_f32; BLOCK]);
    let mut next = 0;
    while out.len() < total {
        while next < events.len() && events[next].0 <= out.len() {
            inst.event(events[next].1);
            next += 1;
        }
        // Stop the block at the next event, so every note starts on its own frame.
        let until = events.get(next).map_or(total, |(at, _)| (*at).min(total));
        let frames = (until - out.len()).min(BLOCK);
        inst.render(&mut left[..frames], &mut right[..frames]);
        out.extend(
            left[..frames]
                .iter()
                .zip(&right[..frames])
                .map(|(l, r)| [*l, *r]),
        );
    }
    out
}

fn write_wav(path: &Path, frames: &[[f32; 2]]) -> std::io::Result<()> {
    let data_bytes = (frames.len() * 4) as u32;
    let mut w = BufWriter::new(File::create(path)?);
    w.write_all(b"RIFF")?;
    w.write_all(&(36 + data_bytes).to_le_bytes())?;
    w.write_all(b"WAVEfmt ")?;
    w.write_all(&16u32.to_le_bytes())?;
    w.write_all(&1u16.to_le_bytes())?; // PCM
    w.write_all(&2u16.to_le_bytes())?; // channels
    w.write_all(&RATE.to_le_bytes())?;
    w.write_all(&(RATE * 4).to_le_bytes())?; // bytes per second
    w.write_all(&4u16.to_le_bytes())?; // bytes per frame
    w.write_all(&16u16.to_le_bytes())?; // bits per sample
    w.write_all(b"data")?;
    w.write_all(&data_bytes.to_le_bytes())?;
    for frame in frames {
        for sample in frame {
            let pcm = (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16;
            w.write_all(&pcm.to_le_bytes())?;
        }
    }
    w.flush()
}

fn db(x: f64) -> f64 {
    20.0 * x.max(1.0e-9).log10()
}

fn main() -> std::io::Result<()> {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/synth-wav".to_string());
    let dir = Path::new(&dir);
    std::fs::create_dir_all(dir)?;

    let mut list: Vec<(&str, Box<dyn Instrument>, Phrase)> = vec![
        ("synth-lead", Box::new(PolySynth::lead()), lead_line()),
        ("synth-bass", Box::new(PolySynth::bass()), bass_line()),
        ("warm-pad", Box::new(PolySynth::pad()), pad_chords()),
        ("sine", Box::new(SineSynth::new()), lead_line()),
        ("electro-kit", Box::new(ElectroKit::new()), drum_beat()),
    ];
    println!("file, peak, RMS of the two bars (both channels)");
    for (name, inst, phrase) in list.iter_mut() {
        let frames = render(inst.as_mut(), phrase);
        let path = dir.join(format!("{name}.wav"));
        write_wav(&path, &frames)?;
        let peak = frames
            .iter()
            .flatten()
            .fold(0.0_f64, |m, x| m.max(f64::from(x.abs())));
        let bars = &frames[..(PHRASE_S * f64::from(RATE)) as usize];
        let power: f64 = bars.iter().flatten().map(|x| f64::from(x * x)).sum();
        let rms = (power / (bars.len() * 2) as f64).sqrt();
        println!(
            "{}  peak {:.1} dBFS  RMS {:.1} dBFS",
            path.display(),
            db(peak),
            db(rms)
        );
    }
    Ok(())
}

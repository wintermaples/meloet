//! Short WAV files of the arpeggio, to judge by ear:
//! `cargo test -p meloet-sound --test arp_ear -- --ignored --nocapture`
//! (the folder comes from MELOET_EAR_DIR, else `target/arp-ear` under the repository).
//! Each file is the engine itself, rendered without a device at 120 in 4/4: a chord
//! (C4 E4 G4 B4, each key with its own velocity) held for two bars on the built-in lead
//! synth, through the arpeggio, with the metronome on.

use std::path::{Path, PathBuf};

use meloet_engine::{BlockTiming, Engine, EngineCommand, EngineConfig, InstrumentEvent, MidiIn};
use meloet_model::{Arp, ArpRate, ArpShape};
use meloet_sound::synth::PolySynth;

const RATE: u32 = 48_000;
const BLOCK: usize = 256;
const CHORD: [(u8, u8); 4] = [(60, 110), (64, 80), (67, 95), (71, 70)];

/// Five seconds. The keys go down at the start and up after `held_s`; the song plays
/// unless `still`.
fn render(set: Arp, keys: &[(u8, u8)], held_s: f64, still: bool) -> Vec<f32> {
    let mut parts = Engine::new(EngineConfig::default());
    let synth = Box::new(PolySynth::lead());
    assert!(parts.handle.swap_instrument(0, synth).is_ok());
    let send = |parts: &mut meloet_engine::EngineParts, command| {
        assert!(parts.handle.send(command).is_ok());
    };
    send(&mut parts, EngineCommand::SetArp { track: 0, arp: set });
    send(&mut parts, EngineCommand::SetMetronome { on: true });
    if !still {
        send(&mut parts, EngineCommand::Play { from: 0 });
    }
    for &(note, velocity) in keys {
        let event = InstrumentEvent::NoteOn { note, velocity };
        parts.midi_in.push(MidiIn { event, t_us: 0 }).unwrap();
    }
    let total = 5 * RATE as usize / BLOCK * BLOCK;
    let up_at = (held_s * f64::from(RATE)) as usize;
    let mut out = vec![0.0f32; total * 2];
    let mut up = false;
    for (i, block) in out.chunks_mut(BLOCK * 2).enumerate() {
        let callback_us = (i * BLOCK) as u64 * 1_000_000 / u64::from(RATE);
        if !up && i * BLOCK >= up_at {
            up = true;
            for &(note, _) in keys {
                let event = InstrumentEvent::NoteOff { note };
                let t_us = callback_us;
                parts.midi_in.push(MidiIn { event, t_us }).unwrap();
            }
        }
        let timing = BlockTiming {
            callback_us,
            playback_us: callback_us + 10_000,
        };
        parts.engine.process(block, timing);
    }
    out
}

fn write(dir: &Path, name: &str, stereo: &[f32]) {
    let data_bytes = (stereo.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data_bytes as usize);
    bytes.extend(b"RIFF");
    bytes.extend((36 + data_bytes).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(RATE.to_le_bytes());
    bytes.extend((RATE * 4).to_le_bytes());
    bytes.extend(4u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(data_bytes.to_le_bytes());
    for v in stereo {
        bytes.extend(((v.clamp(-1.0, 1.0) * 32767.0).round() as i16).to_le_bytes());
    }
    std::fs::write(dir.join(name), bytes).unwrap();
    let peak = stereo.iter().fold(0.0f32, |p, v| p.max(v.abs()));
    println!(
        "{name}: {:.1} s, peak {:.1} dB",
        stereo.len() as f32 / 2.0 / RATE as f32,
        20.0 * peak.max(1e-9).log10()
    );
}

#[test]
#[ignore = "writes WAV files to listen to"]
fn write_the_arpeggio_to_listen_to() {
    let dir = std::env::var_os("MELOET_EAR_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/arp-ear"),
        PathBuf::from,
    );
    std::fs::create_dir_all(&dir).unwrap();
    let on = Arp {
        on: true,
        ..Arp::default()
    };
    let shapes = [
        (ArpShape::Up, "up"),
        (ArpShape::Down, "down"),
        (ArpShape::UpDown, "updown"),
        (ArpShape::Random, "random"),
    ];
    for (shape, shape_name) in shapes {
        for (rate, rate_name) in [(ArpRate::Eighth, "8"), (ArpRate::Sixteenth, "16")] {
            let set = Arp { shape, rate, ..on };
            let name = format!("arp-{shape_name}-{rate_name}.wav");
            write(&dir, &name, &render(set, &CHORD, 4.0, false));
        }
    }
    let up16 = Arp {
        rate: ArpRate::Sixteenth,
        ..on
    };
    let two = Arp { octaves: 2, ..up16 };
    write(
        &dir,
        "arp-up-16-2octaves.wav",
        &render(two, &CHORD, 4.0, false),
    );
    let triplet = Arp {
        rate: ArpRate::Eighth,
        triplet: true,
        ..on
    };
    write(
        &dir,
        "arp-up-8-triplet.wav",
        &render(triplet, &CHORD, 4.0, false),
    );
    // One key: note repeat.
    write(
        &dir,
        "repeat-16.wav",
        &render(up16, &CHORD[..1], 4.0, false),
    );
    // Hold: the keys are let go after half a second and the chord goes on.
    let hold = Arp { hold: true, ..up16 };
    write(
        &dir,
        "arp-up-16-hold.wav",
        &render(hold, &CHORD, 0.5, false),
    );
    // The song stands still (no metronome sounds): from the first key on, at the tempo.
    let eighth = Arp {
        rate: ArpRate::Eighth,
        ..on
    };
    write(
        &dir,
        "arp-up-8-stopped.wav",
        &render(eighth, &CHORD, 4.0, true),
    );
}

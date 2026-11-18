//! Writes each computed instrument playing its sample phrase to a WAV file, for listening.
//!
//! `cargo run --release -p meloet-sound --example render_synths -- target/synth-wav`
//!
//! Tempo 120, two bars (4 s) and 2 s more for the sound to die away. 48 kHz, 16 bit, stereo.

mod shared;

use std::path::Path;

use meloet_engine::Instrument;
use meloet_sound::synth::{ElectroKit, PolySynth, SineSynth};
use shared::*;

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
        let (peak, rms) = loudness(&frames, PHRASE_S);
        println!(
            "{}  peak {:.1} dBFS  RMS {:.1} dBFS",
            path.display(),
            peak,
            rms
        );
    }
    Ok(())
}

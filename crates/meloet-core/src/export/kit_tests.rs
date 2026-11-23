//! The studio kit's notes as the rules tell an engine (the export's engine, which is told
//! the same ops as the playing one): a one-shot cymbal rings on after a short note ends.

use std::sync::Arc;

use meloet_model::song::{Doc, Note};
use meloet_model::{BuiltinInstrument, InstrumentRef, NewTake, edit};
use meloet_sound::sampler::SampleBank;

use super::fixture::{BLOCK, Place, RATE};
use super::render::{ExportEngine, Plugs};
use super::told::Told;
use crate::instruments::Samples;

const CRASH: u8 = 49;
const SIXTEENTH: u64 = 480;

fn wav(frames: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    let data = (frames * 2) as u32;
    bytes.extend(b"RIFF");
    bytes.extend((36 + data).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16u32.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(1u16.to_le_bytes());
    bytes.extend(RATE.to_le_bytes());
    bytes.extend((RATE * 2).to_le_bytes());
    bytes.extend(2u16.to_le_bytes());
    bytes.extend(16u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(data.to_le_bytes());
    for i in 0..frames {
        let level = 0.05 * (1.0 - i as f32 / frames as f32);
        bytes.extend(((level * 32767.0) as i16).to_le_bytes());
    }
    bytes
}

/// The cymbal on the left, a kick and hi-hats that choke each other on the right.
fn kit(place: &Place) -> Arc<SampleBank> {
    let dir = place.root.join("kit");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("crash.wav"), wav(3 * RATE as usize)).unwrap();
    std::fs::write(dir.join("short.wav"), wav(RATE as usize)).unwrap();
    let mut regions = Vec::new();
    for rr in 1..=4 {
        regions.push(format!(
            "{{\"sample\": \"crash.wav\", \"keyLo\": {CRASH}, \"keyHi\": {CRASH}, \"root\": {CRASH}, \
             \"ampVeltrack\": 0, \"oneShot\": true, \"rrIndex\": {rr}, \"rrCount\": 4, \"pan\": -1}}"
        ));
    }
    for (key, more) in [
        (36, ""),
        (42, ", \"chokeGroup\": 1"),
        (46, ", \"chokeGroup\": 2, \"chokedBy\": [1]"),
    ] {
        regions.push(format!(
            "{{\"sample\": \"short.wav\", \"keyLo\": {key}, \"keyHi\": {key}, \"root\": {key}, \
             \"ampVeltrack\": 0, \"oneShot\": true, \"pan\": 1{more}}}"
        ));
    }
    let json = format!(
        "{{\"format\": \"meloet-instrument\", \"version\": 1, \"kind\": \"studioKit\", \
         \"polyphony\": 32, \"sustainPedal\": false, \"regions\": [{}]}}",
        regions.join(", ")
    );
    std::fs::write(dir.join("instrument.json"), json).unwrap();
    meloet_sound::sampler::load_instrument(&dir.join("instrument.json"), 1).unwrap()
}

fn song(crash_ticks: u64, others: bool) -> Doc {
    let doc = Doc::new_default();
    let first = doc.tracks[0].id;
    let kit = InstrumentRef::Builtin(BuiltinInstrument::StudioKit);
    let doc = edit::set_instrument(&doc, first, kit).map_or(doc, |e| e.doc);
    let note = |start, key, length| Note {
        start,
        length,
        key,
        velocity: 100,
        from: None,
    };
    let mut notes = vec![note(0, CRASH, crash_ticks)];
    if others {
        for eighth in 0..12 {
            let hat = if eighth % 2 == 0 { 42 } else { 46 };
            notes.push(note(eighth * 2 * SIXTEENTH, 36, SIXTEENTH));
            notes.push(note(eighth * 2 * SIXTEENTH, hat, SIXTEENTH));
        }
    }
    let take = NewTake {
        origin: 0,
        length: 32 * SIXTEENTH,
        notes,
        controls: Vec::new(),
    };
    edit::add_recording(&doc, first, vec![take], 0)
        .unwrap()
        .0
        .doc
}

/// The left channel of the first 3 s.
fn left(place: &Place, bank: &Arc<SampleBank>, doc: &Doc) -> Vec<f32> {
    let studio_kit = InstrumentRef::Builtin(BuiltinInstrument::StudioKit);
    let samples = vec![(studio_kit, Samples::Ready(bank.clone()))];
    let mut engine = ExportEngine::new(RATE, BLOCK, place.store.clone(), samples);
    let mut plugs = Plugs::default();
    for op in Told::of(doc, None, &Vec::new(), &[]).ops {
        engine.tell(op, &mut plugs);
    }
    engine.play_from(0, false);
    let mut out = Vec::new();
    for _ in 0..3 * RATE as usize / BLOCK {
        out.extend(engine.render(BLOCK).chunks(2).map(|f| f[0]));
    }
    out
}

/// Loudness from 0.5 s to 1 s and from 2 s to 2.5 s.
fn tail(left: &[f32]) -> (f32, f32) {
    let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
    let s = RATE as usize;
    (rms(&left[s / 2..s]), rms(&left[2 * s..5 * s / 2]))
}

#[test]
fn a_short_crash_of_the_studio_kit_rings_on() {
    let place = Place::new("kit-tails");
    let bank = kit(&place);
    let alone = tail(&left(&place, &bank, &song(16 * SIXTEENTH, false)));
    assert!(alone.0 > 0.005 && alone.1 > 0.002, "{alone:?}");
    for (what, doc) in [
        ("a sixteenth", song(SIXTEENTH, false)),
        ("among kicks and hi-hats", song(SIXTEENTH, true)),
    ] {
        let got = tail(&left(&place, &bank, &doc));
        assert!(
            (got.0 - alone.0).abs() < alone.0 * 0.01 && (got.1 - alone.1).abs() < alone.1 * 0.01,
            "{what}: {got:?}, alone {alone:?}"
        );
    }
}

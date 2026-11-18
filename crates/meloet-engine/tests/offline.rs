//! Engine and placeholder synth without a device.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed};

use meloet_engine::{
    BlockTiming, Chord, Engine, EngineCommand, EngineConfig, EngineParts, Instrument,
    InstrumentEvent, LatencyRecord, MidiIn, PlaceholderSynth,
};

const RATE: usize = 48_000;

struct Rig {
    parts: EngineParts,
    block: usize,
}

impl Rig {
    fn new() -> Self {
        Self::with(EngineConfig::default(), 256)
    }

    fn with(config: EngineConfig, block: usize) -> Self {
        let mut rig = Rig {
            parts: Engine::new(config),
            block,
        };
        assert!(
            rig.parts
                .handle
                .swap_instrument(0, Box::new(PlaceholderSynth::new()))
                .is_ok()
        );
        rig
    }

    fn midi(&mut self, event: InstrumentEvent) {
        self.parts
            .midi_in
            .push(MidiIn { event, t_us: 0 })
            .expect("midi ring full");
    }

    fn send(&mut self, command: EngineCommand) {
        assert!(self.parts.handle.send(command).is_ok(), "command ring full");
    }

    /// Renders `frames` frames in device blocks of `self.block` and returns the left channel.
    fn run(&mut self, frames: usize) -> Vec<f32> {
        let mut left = Vec::with_capacity(frames);
        let mut out = vec![0.0f32; self.block * 2];
        while left.len() < frames {
            let n = self.block.min(frames - left.len());
            self.parts
                .engine
                .process(&mut out[..n * 2], BlockTiming::default());
            for frame in out[..n * 2].as_chunks::<2>().0 {
                assert_eq!(frame[0], frame[1], "the placeholder sound is centred");
                left.push(frame[0]);
            }
        }
        left
    }
}

fn note_on(note: u8, velocity: u8) -> InstrumentEvent {
    InstrumentEvent::NoteOn { note, velocity }
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0, |m, s| m.max(s.abs()))
}

/// Frequency from the spacing of upward zero crossings.
fn frequency(samples: &[f32]) -> f32 {
    let crossings: Vec<f32> = samples
        .windows(2)
        .enumerate()
        .filter(|(_, w)| w[0] < 0.0 && w[1] >= 0.0)
        .map(|(i, w)| i as f32 + w[0] / (w[0] - w[1]))
        .collect();
    assert!(crossings.len() > 10, "not enough signal to measure");
    let cycles = (crossings.len() - 1) as f32;
    cycles * RATE as f32 / (crossings[crossings.len() - 1] - crossings[0])
}

#[test]
fn note_on_sounds_at_the_right_pitch() {
    let mut rig = Rig::new();
    assert_eq!(peak(&rig.run(1024)), 0.0, "silent before any note");
    for (note, hz) in [(69u8, 440.0f32), (60, 261.63), (84, 1046.50)] {
        rig.midi(note_on(note, 100));
        let sound = rig.run(RATE / 2);
        assert!(peak(&sound) > 0.02, "note {note} is audible");
        assert!(peak(&sound) < 0.5, "note {note} is not loud");
        let measured = frequency(&sound[RATE / 10..]);
        assert!(
            (measured - hz).abs() < hz * 0.002,
            "note {note}: {measured} Hz, wanted {hz}"
        );
        rig.midi(InstrumentEvent::NoteOff { note });
        rig.run(RATE / 2);
    }
}

#[test]
fn louder_velocity_is_louder_and_brighter() {
    let mut level = Vec::new();
    for velocity in [30u8, 120] {
        let mut rig = Rig::new();
        rig.midi(note_on(57, velocity));
        let sound = rig.run(RATE / 4);
        // Sample-to-sample movement against level rises with the share of harmonics.
        let movement: f32 = sound.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
        let size: f32 = sound.iter().map(|s| s.abs()).sum();
        level.push((peak(&sound), movement / size));
    }
    assert!(level[1].0 > level[0].0 * 2.0, "{level:?}");
    assert!(level[1].1 > level[0].1 * 1.05, "{level:?}");
}

#[test]
fn note_off_releases_into_silence() {
    let mut rig = Rig::new();
    rig.midi(note_on(69, 100));
    let held = rig.run(RATE);
    assert!(
        peak(&held[RATE - 2400..]) > 0.01,
        "still sustaining after a second"
    );
    rig.midi(InstrumentEvent::NoteOff { note: 69 });
    let tail = rig.run(RATE / 2);
    assert!(peak(&tail[..480]) > 0.001, "the release is not a hard cut");
    assert_eq!(
        peak(&tail[RATE * 35 / 100..]),
        0.0,
        "silent 350 ms after note off"
    );
}

#[test]
fn sustain_pedal_holds_released_notes() {
    let mut rig = Rig::new();
    rig.midi(InstrumentEvent::Control {
        controller: 64,
        value: 127,
    });
    rig.midi(note_on(64, 100));
    rig.run(4800);
    rig.midi(InstrumentEvent::NoteOff { note: 64 });
    let held = rig.run(RATE / 2);
    assert!(
        peak(&held[RATE / 2 - 2400..]) > 0.01,
        "pedal keeps the note"
    );
    rig.midi(InstrumentEvent::Control {
        controller: 64,
        value: 0,
    });
    let tail = rig.run(RATE / 2);
    assert_eq!(peak(&tail[RATE * 35 / 100..]), 0.0, "pedal up lets it go");
}

#[test]
fn pitch_bend_moves_two_semitones() {
    let mut rig = Rig::new();
    rig.midi(note_on(69, 100));
    rig.run(4800);
    rig.midi(InstrumentEvent::PitchBend { value: 8191 });
    let up = frequency(&rig.run(RATE / 2)[2400..]);
    assert!((up - 493.88).abs() < 1.0, "bend up: {up} Hz");
    rig.midi(InstrumentEvent::PitchBend { value: -8192 });
    let down = frequency(&rig.run(RATE / 2)[2400..]);
    assert!((down - 392.0).abs() < 1.0, "bend down: {down} Hz");
    rig.midi(InstrumentEvent::PitchBend { value: 0 });
    let centre = frequency(&rig.run(RATE / 2)[2400..]);
    assert!((centre - 440.0).abs() < 1.0, "bend centre: {centre} Hz");
}

#[test]
fn modulation_wheel_adds_vibrato() {
    let mut rig = Rig::new();
    rig.midi(note_on(81, 100));
    rig.midi(InstrumentEvent::Control {
        controller: 1,
        value: 127,
    });
    rig.run(4800);
    // 20 ms windows over one second: the pitch swings about 30 cents (1.75 %) each way.
    let sound = rig.run(RATE);
    let pitches: Vec<f32> = sound.chunks(960).map(frequency).collect();
    let low = pitches.iter().cloned().fold(f32::MAX, f32::min);
    let high = pitches.iter().cloned().fold(0.0, f32::max);
    assert!((865.0..872.0).contains(&low), "lowest {low}");
    assert!((888.0..896.0).contains(&high), "highest {high}");
}

#[test]
fn stealing_a_voice_does_not_click() {
    let mut rig = Rig::new();
    let mut sound = Vec::new();
    for note in 40..90u8 {
        rig.midi(note_on(note, 90));
        sound.extend(rig.run(480));
    }
    let late = &sound[32 * 480..];
    let jump = late
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max);
    assert!(sound.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
    // 50 notes on 32 voices: without a fade a stolen voice would jump by its whole level.
    assert!(jump < 0.25, "largest step {jump}");
    rig.send(EngineCommand::AllNotesOff);
    let tail = rig.run(RATE / 2);
    assert_eq!(peak(&tail[RATE * 35 / 100..]), 0.0);
}

#[test]
fn meters_move_and_reading_clears_them() {
    let mut rig = Rig::new();
    let meters = rig.parts.handle.meters().clone();
    rig.midi(note_on(60, 110));
    let sound = rig.run(256 * 20);
    let track = meters.take_track_peak(0);
    let master = meters.take_master_peak();
    assert!(
        (track[0] - peak(&sound)).abs() < 1e-6 && track[0] == track[1],
        "{track:?}"
    );
    assert_eq!(track, master);
    assert_eq!(meters.take_track_peak(1), [0.0, 0.0], "empty track");
    assert_eq!(meters.take_track_peak(0), [0.0, 0.0], "cleared by the read");
    assert_eq!(meters.take_master_peak(), [0.0, 0.0]);
    assert_eq!(meters.last_frames(), 256);
    assert_eq!(meters.callbacks(), 20);
    assert_eq!(meters.sample_rate(), 48_000);
    assert!(!meters.take_clipped());

    rig.send(EngineCommand::SetTrackGain {
        track: 0,
        gain: 20.0,
    });
    let loud = rig.run(256 * 4);
    assert!(meters.take_master_peak()[0] > 1.0);
    assert!(peak(&loud) <= 1.0, "the output itself is clamped");
    assert!(meters.take_clipped());
    assert!(!meters.take_clipped(), "cleared by the read");

    rig.send(EngineCommand::SetTrackGain {
        track: 0,
        gain: 0.0,
    });
    rig.run(256);
    assert_eq!(peak(&rig.run(256)), 0.0, "gain 0 mutes the track");
}

#[test]
fn keyboard_plays_the_play_target() {
    let mut rig = Rig::new();
    let meters = rig.parts.handle.meters().clone();
    assert!(
        rig.parts
            .handle
            .swap_instrument(3, Box::new(PlaceholderSynth::new()))
            .is_ok()
    );
    rig.send(EngineCommand::SetPlayTarget { track: 3 });
    rig.midi(note_on(60, 100));
    rig.run(2048);
    assert_eq!(meters.take_track_peak(0), [0.0, 0.0]);
    assert!(meters.take_track_peak(3)[0] > 0.01);
}

#[test]
fn audition_sounds_on_its_track_until_let_go() {
    let mut rig = Rig::new();
    let meters = rig.parts.handle.meters().clone();
    assert!(
        rig.parts
            .handle
            .swap_instrument(2, Box::new(PlaceholderSynth::new()))
            .is_ok()
    );
    rig.send(EngineCommand::Audition {
        track: 2,
        keys: Chord::new(&[60]),
        velocity: 100,
    });
    rig.run(2048);
    assert_eq!(meters.take_track_peak(0), [0.0, 0.0], "not the play target");
    assert!(meters.take_track_peak(2)[0] > 0.01);
    assert!(
        rig.parts.handle.pop_latency().is_none(),
        "not a key: no key-to-sound record"
    );
    // The next audition lets go of the first; the off lets go of the second.
    rig.send(EngineCommand::Audition {
        track: 2,
        keys: Chord::new(&[64]),
        velocity: 100,
    });
    rig.run(256);
    rig.send(EngineCommand::AuditionOff);
    rig.run(RATE);
    meters.take_track_peak(2);
    assert_eq!(peak(&rig.run(256)), 0.0, "both released");
    // A track with no instrument is no trouble.
    rig.send(EngineCommand::Audition {
        track: 40,
        keys: Chord::new(&[64]),
        velocity: 100,
    });
    rig.send(EngineCommand::AuditionOff);
    rig.run(256);
}

#[test]
fn audition_sounds_a_chord_and_replaces_it() {
    let mut rig = Rig::new();
    let meters = rig.parts.handle.meters().clone();
    rig.send(EngineCommand::Audition {
        track: 0,
        keys: Chord::new(&[60, 64, 67]),
        velocity: 100,
    });
    let chord = peak(&rig.run(4096));
    rig.send(EngineCommand::AuditionOff);
    rig.run(RATE);
    rig.send(EngineCommand::Audition {
        track: 0,
        keys: Chord::new(&[60]),
        velocity: 100,
    });
    let one = peak(&rig.run(4096));
    assert!(
        chord > one * 1.5,
        "three keys are louder than one: {chord} {one}"
    );
    // Empty keys only let go.
    rig.send(EngineCommand::Audition {
        track: 0,
        keys: Chord::new(&[]),
        velocity: 100,
    });
    rig.run(RATE);
    meters.take_track_peak(0);
    assert_eq!(peak(&rig.run(256)), 0.0, "all let go");
}

#[test]
fn a_chord_keeps_sixteen_keys_without_doubles() {
    let keys: Vec<u8> = (40..70).collect();
    assert_eq!(Chord::new(&keys).keys(), &keys[..16]);
    assert_eq!(Chord::new(&[60, 60, 200, 61]).keys(), &[60, 61]);
    assert!(Chord::new(&[]).keys().is_empty());
}

#[test]
fn note_on_leaves_a_latency_record() {
    let mut rig = Rig::new();
    rig.parts
        .midi_in
        .push(MidiIn {
            event: note_on(60, 100),
            t_us: 1_000,
        })
        .unwrap();
    rig.parts
        .midi_in
        .push(MidiIn {
            event: InstrumentEvent::NoteOff { note: 60 },
            t_us: 1_500,
        })
        .unwrap();
    let mut out = [0.0f32; 512];
    rig.parts.engine.process(
        &mut out,
        BlockTiming {
            callback_us: 3_000,
            playback_us: 14_000,
        },
    );
    assert_eq!(
        rig.parts.handle.pop_latency(),
        Some(LatencyRecord {
            key_us: 1_000,
            callback_us: 3_000,
            playback_us: 14_000
        })
    );
    assert_eq!(
        rig.parts.handle.pop_latency(),
        None,
        "only NoteOn is recorded"
    );
}

/// Constant level; records the largest block it was asked for and whether it was dropped.
struct Probe {
    level: f32,
    largest: Arc<AtomicUsize>,
    dropped: Arc<AtomicBool>,
    max_frames: usize,
}

impl Instrument for Probe {
    fn prepare(&mut self, _sample_rate: f32, max_frames: usize) {
        self.max_frames = max_frames;
    }
    fn event(&mut self, _ev: InstrumentEvent) {}
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        assert_eq!(left.len(), right.len());
        assert!(left.len() <= self.max_frames);
        self.largest.fetch_max(left.len(), Relaxed);
        left.fill(self.level);
        right.fill(-self.level);
    }
    fn reset(&mut self) {}
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.dropped.store(true, Relaxed);
    }
}

#[test]
fn swapped_out_instrument_comes_back_instead_of_being_dropped() {
    let mut rig = Rig::new();
    let largest = Arc::new(AtomicUsize::new(0));
    let probe = |level: f32, dropped: &Arc<AtomicBool>| {
        Box::new(Probe {
            level,
            largest: largest.clone(),
            dropped: dropped.clone(),
            max_frames: 0,
        })
    };
    let (first_dropped, second_dropped) = (
        Arc::new(AtomicBool::new(false)),
        Arc::new(AtomicBool::new(false)),
    );

    assert!(
        rig.parts
            .handle
            .swap_instrument(0, probe(0.25, &first_dropped))
            .is_ok()
    );
    let mut out = [0.0f32; 512];
    rig.parts.engine.process(&mut out, BlockTiming::default());
    assert_eq!(
        &out[..2],
        &[0.25, -0.25],
        "the new instrument is the one rendering"
    );
    assert!(
        rig.parts.handle.take_retired().is_some(),
        "the placeholder synth came back"
    );

    assert!(
        rig.parts
            .handle
            .swap_instrument(0, probe(0.5, &second_dropped))
            .is_ok()
    );
    rig.parts.engine.process(&mut out, BlockTiming::default());
    assert_eq!(&out[510..], &[0.5, -0.5]);
    assert!(
        !first_dropped.load(Relaxed),
        "the audio side did not drop it"
    );
    let old = rig
        .parts
        .handle
        .take_retired()
        .expect("old instrument on the retired ring");
    assert!(rig.parts.handle.take_retired().is_none());
    drop(old);
    assert!(first_dropped.load(Relaxed));

    // A track that does not exist: the instrument is handed back, not dropped.
    let stray = Arc::new(AtomicBool::new(false));
    assert!(
        rig.parts
            .handle
            .swap_instrument(10_000, probe(0.1, &stray))
            .is_ok()
    );
    rig.parts.engine.process(&mut out, BlockTiming::default());
    assert!(!stray.load(Relaxed));
    assert_eq!(rig.parts.handle.drop_retired(), 1);
    assert!(stray.load(Relaxed));
    assert!(!second_dropped.load(Relaxed));
}

#[test]
fn a_block_larger_than_max_frames_is_rendered_in_pieces() {
    let small = EngineConfig {
        max_frames: 128,
        ..EngineConfig::default()
    };
    let largest = Arc::new(AtomicUsize::new(0));
    let mut rig = Rig::with(small, 1000);
    let probe = Probe {
        level: 0.0,
        largest: largest.clone(),
        dropped: Arc::default(),
        max_frames: 0,
    };
    assert!(rig.parts.handle.swap_instrument(1, Box::new(probe)).is_ok());
    rig.midi(note_on(69, 100));
    let chunked = rig.run(3000);
    assert_eq!(largest.load(Relaxed), 128);
    assert_eq!(
        rig.parts.handle.meters().last_frames(),
        1000,
        "the device block, not the piece"
    );

    // The same music through an engine that never has to split.
    let mut whole = Rig::with(EngineConfig::default(), 1000);
    whole.midi(note_on(69, 100));
    let reference = whole.run(3000);
    let worst = chunked
        .iter()
        .zip(&reference)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f32::max);
    assert!(
        worst < 1e-3,
        "pieces join without a seam (worst difference {worst})"
    );
    assert!((frequency(&chunked[480..]) - 440.0).abs() < 1.0);
}

#[test]
fn test_tone_plays_and_ends() {
    let mut rig = Rig::new();
    rig.send(EngineCommand::TestTone);
    let tone = rig.run(RATE * 6 / 10);
    let level = peak(&tone);
    assert!((0.12..0.32).contains(&level), "about -12 dBFS, got {level}");
    assert!(peak(&tone[RATE / 5..RATE / 4]) > 0.02, "second note sounds");
    assert_eq!(peak(&rig.run(RATE / 10)), 0.0, "over after 0.6 s");
    assert!(
        rig.parts.handle.meters().take_master_peak()[0] > 0.1,
        "shows on the master meter"
    );
    assert_eq!(
        rig.parts.handle.meters().take_track_peak(0),
        [0.0, 0.0],
        "not on a track"
    );
}

#[test]
fn a_thousand_blocks_with_midi_never_touch_the_allocator() {
    let mut rig = Rig::new();
    assert!(
        rig.parts
            .handle
            .swap_instrument(1, Box::new(PlaceholderSynth::new()))
            .is_ok()
    );
    let mut out = vec![0.0f32; 2048 * 2];
    let mut heard = 0.0f32;
    for block in 0..1000usize {
        let note = 36 + (block * 7 % 60) as u8;
        // More notes than voices, pedal, bend, wheel, every command, and odd block sizes.
        rig.midi(note_on(note, 40 + (block % 80) as u8));
        if block % 3 == 0 {
            rig.midi(InstrumentEvent::NoteOff {
                note: 36 + ((block / 3) * 7 % 60) as u8,
            });
        }
        if block % 50 == 0 {
            rig.midi(InstrumentEvent::Control {
                controller: 64,
                value: if block % 100 == 0 { 127 } else { 0 },
            });
            rig.midi(InstrumentEvent::Control {
                controller: 1,
                value: (block % 128) as u8,
            });
            rig.midi(InstrumentEvent::PitchBend {
                value: (block as i16 % 16) * 500 - 4000,
            });
        }
        match block % 97 {
            5 => rig.send(EngineCommand::TestTone),
            11 => rig.send(EngineCommand::SetTrackGain {
                track: 0,
                gain: 0.5,
            }),
            23 => rig.send(EngineCommand::SetPlayTarget { track: block % 2 }),
            41 => rig.send(EngineCommand::AllNotesOff),
            47 | 71 => rig.send(EngineCommand::Audition {
                track: block % 3,
                keys: Chord::new(&[40 + (block % 40) as u8, 47, 52, 59]),
                velocity: 90,
            }),
            53 | 83 => rig.send(EngineCommand::AuditionOff),
            61 | 89 => rig.send(EngineCommand::SetSilentTracks {
                tracks: [block % 2].into_iter().collect(),
            }),
            60 => {
                assert!(
                    rig.parts
                        .handle
                        .swap_instrument(block % 2, Box::new(PlaceholderSynth::new()))
                        .is_ok()
                );
                rig.parts.handle.drop_retired();
            }
            _ => {}
        }
        let frames = [64, 256, 1024, 2048, 333][block % 5];
        rig.parts
            .engine
            .process(&mut out[..frames * 2], BlockTiming::default());
        heard = heard.max(peak(&out[..frames * 2]));
        while rig.parts.handle.pop_latency().is_some() {}
    }
    assert!(heard > 0.05, "the run made sound");
    assert_eq!(
        common::audio_block_counts(),
        (0, 0),
        "(allocations, frees) inside audio blocks"
    );
}

#[test]
fn a_rebuilt_engine_shares_the_meters_and_plays_from_the_new_ring() {
    let mut rig = Rig::new();
    let meters = rig.parts.handle.meters().clone();
    let callbacks = meters.callbacks();

    // The old engine and its producer are as good as lost.
    let (engine, midi_in) = rig.parts.handle.rebuild();
    rig.parts.engine = engine;
    rig.parts.midi_in = midi_in;
    assert!(Arc::ptr_eq(rig.parts.handle.meters(), &meters));

    assert_eq!(peak(&rig.run(256)), 0.0, "no instrument yet");
    assert!(
        rig.parts
            .handle
            .swap_instrument(0, Box::new(PlaceholderSynth::new()))
            .is_ok()
    );
    rig.run(256);
    rig.midi(note_on(69, 100));
    assert!(
        peak(&rig.run(4800)) > 0.05,
        "the new ring reaches the new engine"
    );
    assert_eq!(meters.callbacks(), callbacks + 2 + 19);
}

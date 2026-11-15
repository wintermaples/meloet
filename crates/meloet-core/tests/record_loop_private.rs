//! Recording over a loop, through the real ALSA sequencer and a PipeWire of the test's own:
//! the takes and their contents follow `docs/architecture.md` 13 (spec chapter 3). Keys right
//! before and right after the lap boundary, a key held across it, a pedal held across it, a lap
//! with no note, and a key pressed just before the recording starts (during the count-in).
//! Run with `-- --ignored --nocapture`; needs `pipewire`, `wireplumber`, `pw-link` and the ALSA
//! sequencer.

#[path = "../../meloet-engine/tests/private_pw/mod.rs"]
mod private_pw;
mod record_timing;

use std::sync::Mutex;
use std::time::Duration;

use meloet_core::{Core, CoreOptions};
use meloet_midi::vkeys::VirtualKeyboard;
use meloet_midi::{MidiEvent, monotonic_us};
use meloet_protocol::{Command, TakeControlValue, TakeNotes, TransportState};
use record_timing::{Reading, reading, wait_until};

const BPM: f64 = 120.0;
const BAR: i64 = 1920;
/// Bars 2 to 4: [1920, 5760).
const LOOP_START: i64 = BAR;
const LOOP_LEN: i64 = 2 * BAR;
const VELOCITY: u8 = 25;
/// Ticks per millisecond.
const TPMS: f64 = 480.0 * BPM / 60_000.0;
/// Allowed difference between where a key should land and where it did, in ticks.
const SLACK: i64 = 3;

/// Song position counted on without wrapping: lap × loop length + tick. During the count-in
/// it is the tick itself.
fn continuous(r: &Reading) -> f64 {
    match r.transport.state {
        TransportState::Recording => {
            f64::from(r.transport.lap) * LOOP_LEN as f64 + r.transport.tick
        }
        _ => r.transport.tick,
    }
}

/// Waits until the listener hears continuous position `c`.
fn at(latest: &Mutex<Option<Reading>>, c: i64) {
    let r = reading(latest).expect("a reading");
    assert!(r.transport.ticks_per_us > 0.0, "{r:?}");
    let t = r.sent_us + (c as f64 - continuous(&r)) / r.transport.ticks_per_us;
    assert!(
        t > monotonic_us() as f64,
        "position {c} is already past: {r:?}"
    );
    wait_until(t);
}

/// Where a lap's tick lands in continuous position.
fn lap_tick(lap: i64, tick: i64) -> i64 {
    lap * LOOP_LEN + tick
}

enum Act {
    On(u8),
    Off(u8),
    Pedal(u8),
}

#[test]
#[ignore = "starts a PipeWire daemon of its own"]
fn loop_recording_makes_one_take_per_lap_with_notes() {
    let Some(pw) = private_pw::PrivatePipeWire::start() else {
        eprintln!("no pipewire program; skipped");
        return;
    };
    let (name, midi) = record_timing::private_keyboard("loop");
    let Ok(keys) = VirtualKeyboard::open(&name) else {
        eprintln!("skipped: the ALSA sequencer cannot be opened here");
        return;
    };
    let config = pw.dir().join("config-meloet");
    record_timing::write_settings(&config, private_pw::SINK);
    let core = Core::start(CoreOptions {
        config_dir: Some(config),
        measure_dir: None,
        midi: Some(midi),
        devices: true,
        audio: true,
        songs_dir: None,
    });
    let latest = record_timing::listen(&core);
    record_timing::wait_for(&core, "the output runs", Duration::from_secs(10), |s| {
        s.audio.running && s.audio.actual_frames.is_some()
    });
    record_timing::wait_for(&core, "the keyboard is seen", Duration::from_secs(3), |s| {
        s.keyboards.list.iter().any(|k| k.name == name)
    });
    core.send(Command::SetTempo { bpm: BPM });
    core.send(Command::SetCountIn { bars: 1 });
    core.send(Command::SetLoop {
        from_bar: 2,
        to_bar: 4,
    });
    let state = core.snapshot().state;
    assert!(state.song.loop_on);
    assert_eq!(
        state.song.loop_range.map(|r| (r.start, r.end)),
        Some((LOOP_START as u64, (LOOP_START + LOOP_LEN) as u64))
    );

    let ms = |n: i64| (n as f64 * TPMS).round() as i64;
    let end = LOOP_START + LOOP_LEN;
    // (continuous position, what), put in order below.
    let mut script: Vec<(i64, Act)> = vec![
        // During the count-in, 50 ms before the recording starts.
        (LOOP_START - ms(50), Act::On(60)),
        (LOOP_START + ms(100), Act::Off(60)),
        // Lap 0.
        (lap_tick(0, 2880), Act::On(62)),
        (lap_tick(0, 2880) + ms(120), Act::Off(62)),
        (lap_tick(0, 3840), Act::Pedal(127)),
        // Held across the boundary: from 25 ms before the end of lap 0 to 200 ms into lap 1.
        (end - ms(25), Act::On(64)),
        (end + ms(200), Act::Off(64)),
        // Lap 1: 25 ms after its start.
        (lap_tick(1, LOOP_START) + ms(25), Act::On(65)),
        (lap_tick(1, LOOP_START) + ms(25) + ms(100), Act::Off(65)),
        (lap_tick(1, 4320), Act::Pedal(0)),
        // Lap 2: nothing. Lap 3: one key.
        (lap_tick(3, 2880), Act::On(67)),
        (lap_tick(3, 2880) + ms(120), Act::Off(67)),
    ];

    script.sort_by_key(|(c, _)| *c);
    let asked = monotonic_us() as f64;
    core.send(Command::Record);
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while !reading(&latest)
        .is_some_and(|r| r.sent_us > asked && r.transport.state == TransportState::CountIn)
    {
        assert!(
            std::time::Instant::now() < deadline,
            "the count-in did not start"
        );
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut sent = Vec::new();
    for (c, act) in &script {
        at(&latest, *c);
        let event = match act {
            Act::On(n) => MidiEvent::NoteOn {
                note: *n,
                velocity: VELOCITY,
            },
            Act::Off(n) => MidiEvent::NoteOff {
                note: *n,
                velocity: 0,
            },
            Act::Pedal(v) => MidiEvent::Control {
                controller: 64,
                value: *v,
            },
        };
        keys.send(event).expect("send");
        sent.push((*c, event));
    }
    at(&latest, lap_tick(3, 3840));
    core.send(Command::Record);
    record_timing::wait_for(&core, "the takes are in", Duration::from_secs(3), |s| {
        s.transport.state == TransportState::Stopped && s.transport.recording.is_none()
    });

    // Read every take, the last one first (it is the one shown).
    let state = core.snapshot().state;
    let clip_id = state
        .arrange
        .selected_clip
        .expect("the new clip is selected");
    let clip_of = |s: &meloet_protocol::AppState| {
        s.song
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .find(|c| c.id == clip_id)
            .cloned()
            .expect("the clip")
    };
    let clip = clip_of(&state);
    eprintln!(
        "clip {}..{}, take {}/{}",
        clip.start, clip.end, clip.take, clip.takes
    );
    let mut takes = Vec::new();
    for step in 0..clip.takes {
        if step > 0 {
            core.send(Command::StepTake { delta: -1 });
        }
        let c = clip_of(&core.snapshot().state);
        let take = TakeNotes::decode(&core.take_notes(c.take_id).expect("notes")).unwrap();
        let base = (c.start + c.take_offset) as i64;
        let notes: Vec<(u8, i64, i64)> = take
            .notes
            .iter()
            .map(|n| (n.key, base + i64::from(n.start), i64::from(n.length)))
            .collect();
        let controls: Vec<(i64, TakeControlValue)> = take
            .controls
            .iter()
            .map(|k| (base + i64::from(k.tick), k.value))
            .collect();
        eprintln!(
            "take {}: offset {} length {} notes (key, song tick, length) {notes:?} controls {controls:?}",
            c.take, c.take_offset, take.length
        );
        takes.push((c.take, notes, controls));
    }
    core.stop();
    takes.reverse();

    // Laps 0, 1 and 3 have notes; lap 2 makes no take. The last lap with notes is shown.
    assert_eq!(clip.takes, 3, "one take per lap with notes");
    assert_eq!(clip.take, 3, "the last lap with notes is shown");
    let near = |got: i64, want: i64| (got - want).abs() <= SLACK;
    let has = |notes: &[(u8, i64, i64)], key: u8, start: i64| {
        notes
            .iter()
            .find(|n| n.0 == key && near(n.1, start))
            .copied()
    };

    let (_, lap0, lap0_controls) = &takes[0];
    let early = has(lap0, 60, LOOP_START - ms(50));
    assert!(
        early.is_some(),
        "the key before the start widens lap 0: {lap0:?}"
    );
    assert!(has(lap0, 62, 2880).is_some(), "{lap0:?}");
    let held = has(lap0, 64, end - ms(25)).expect("the held key is in the lap it was pressed in");
    assert!(
        near(held.1 + held.2, end),
        "the held key ends at the loop end: {held:?}"
    );
    assert_eq!(lap0.len(), 3, "{lap0:?}");
    assert!(
        lap0_controls.iter().any(|(t, v)| near(*t, 3840)
            && *v
                == TakeControlValue::Cc {
                    number: 64,
                    value: 127
                }),
        "{lap0_controls:?}"
    );

    let (_, lap1, lap1_controls) = &takes[1];
    assert!(has(lap1, 65, LOOP_START + ms(25)).is_some(), "{lap1:?}");
    assert!(
        !lap1.iter().any(|n| n.0 == 64),
        "the held key is not carried into lap 1: {lap1:?}"
    );
    assert_eq!(lap1.len(), 1, "{lap1:?}");
    assert!(
        lap1_controls.iter().any(|(t, v)| *t == LOOP_START
            && *v
                == TakeControlValue::Cc {
                    number: 64,
                    value: 127
                }),
        "the held pedal is written at the head of lap 1: {lap1_controls:?}"
    );
    assert!(
        lap1_controls.iter().any(|(t, v)| near(*t, 4320)
            && *v
                == TakeControlValue::Cc {
                    number: 64,
                    value: 0
                }),
        "{lap1_controls:?}"
    );

    let (_, lap3, _) = &takes[2];
    assert_eq!(lap3.len(), 1, "{lap3:?}");
    assert!(has(lap3, 67, 2880).is_some(), "{lap3:?}");
}

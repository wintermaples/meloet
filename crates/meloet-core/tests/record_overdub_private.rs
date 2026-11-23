//! 「重ねる」 over a loop, through the real ALSA sequencer and a PipeWire of the test's own:
//! every pass with notes adds them to the one take of the clip, as one step each. A key held
//! across the end of a pass goes into that pass, cut at the loop's end; a pass with no notes
//! is no step. Run with `-- --ignored --nocapture`; needs `pipewire`, `wireplumber`, `pw-link`
//! and the ALSA sequencer.

#[path = "../../meloet-engine/tests/private_pw/mod.rs"]
mod private_pw;
mod record_timing;

use std::sync::Mutex;
use std::time::Duration;

use meloet_core::{Core, CoreOptions};
use meloet_midi::vkeys::VirtualKeyboard;
use meloet_midi::{MidiEvent, monotonic_us};
use meloet_protocol::{Command, TakeNotes, TransportState};
use record_timing::{Reading, reading, wait_until};

const BPM: f64 = 120.0;
const Q: i64 = meloet_model::TICKS_PER_QUARTER as i64;
const BAR: i64 = 4 * Q;
/// Bars 2 to 4: [BAR, 3 * BAR).
const LOOP_START: i64 = BAR;
const LOOP_LEN: i64 = 2 * BAR;
const VELOCITY: u8 = 25;
/// Ticks per millisecond.
const TPMS: f64 = Q as f64 * BPM / 60_000.0;
/// Allowed difference between where a key should land and where it did, in ticks.
const SLACK: i64 = 3;

fn continuous(r: &Reading) -> f64 {
    match r.transport.state {
        TransportState::Recording => {
            f64::from(r.transport.lap) * LOOP_LEN as f64 + r.transport.tick
        }
        _ => r.transport.tick,
    }
}

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

fn lap_tick(lap: i64, tick: i64) -> i64 {
    lap * LOOP_LEN + tick
}

#[test]
#[ignore = "starts a PipeWire daemon of its own"]
fn overdub_over_a_loop_adds_each_pass_as_one_step() {
    let Some(pw) = private_pw::PrivatePipeWire::start() else {
        eprintln!("no pipewire program; skipped");
        return;
    };
    let (name, midi) = record_timing::private_keyboard("overdub");
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
        state_dir: None,
        sounds_dir: None,
        pack: None,
        updates: None,
        plugins: None,
        sf2_dir: None,
        cache_dir: None,
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
    core.send(Command::SetOverdub { on: true });
    record_timing::wait_for(
        &core,
        "「重ねる」 is on",
        Duration::from_secs(3),
        |s| s.transport.overdub && s.song.loop_on,
    );
    let steps = core.snapshot().state.song.undo_count;

    let ms = |n: i64| (n as f64 * TPMS).round() as i64;
    let end = LOOP_START + LOOP_LEN;
    let script: Vec<(i64, u8, bool)> = vec![
        (lap_tick(0, 6 * Q), 62, true),
        (lap_tick(0, 6 * Q) + ms(120), 62, false),
        // Held across the end of pass 0: from 25 ms before it to 200 ms into pass 1.
        (end - ms(25), 64, true),
        (end + ms(200), 64, false),
        (lap_tick(1, LOOP_START) + ms(300), 65, true),
        (lap_tick(1, LOOP_START) + ms(400), 65, false),
        // Pass 2: nothing. Pass 3: one key.
        (lap_tick(3, 7 * Q), 67, true),
        (lap_tick(3, 7 * Q) + ms(120), 67, false),
    ];

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
    let mut after_pass = Vec::new();
    for &(c, note, down) in &script {
        at(&latest, c);
        let event = if down {
            MidiEvent::NoteOn {
                note,
                velocity: VELOCITY,
            }
        } else {
            MidiEvent::NoteOff { note, velocity: 0 }
        };
        keys.send(event).expect("send");
        let s = core.snapshot().state;
        after_pass.push((c, s.song.undo_count - steps));
    }
    at(&latest, lap_tick(3, 8 * Q));
    core.send(Command::Record);
    record_timing::wait_for(&core, "the notes are in", Duration::from_secs(3), |s| {
        s.transport.state == TransportState::Stopped && s.transport.recording.is_none()
    });

    let state = core.snapshot().state;
    let clips: Vec<_> = state.song.tracks.iter().flat_map(|t| &t.clips).collect();
    assert_eq!(clips.len(), 1, "{clips:?}");
    let clip = clips[0];
    let take = TakeNotes::decode(&core.take_notes(clip.take_id).expect("notes")).unwrap();
    let base = clip.start as i64 + clip.take_offset;
    let notes: Vec<(u8, i64, i64)> = (take.notes.iter())
        .map(|n| (n.key, base + i64::from(n.start), i64::from(n.length)))
        .collect();
    eprintln!(
        "clip {}..{}, take {}/{}, steps {}, notes (key, song tick, length) {notes:?}, steps while playing {after_pass:?}",
        clip.start,
        clip.end,
        clip.take,
        clip.takes,
        state.song.undo_count - steps
    );
    core.stop();

    assert_eq!(clip.takes, 1, "no take is made");
    assert_eq!(state.song.undo_count - steps, 3, "passes 0, 1 and 3");
    assert_eq!(
        after_pass.last().map(|a| a.1),
        Some(2),
        "two passes were in before the last one ended"
    );
    let near = |got: i64, want: i64| (got - want).abs() <= SLACK;
    let has = |key: u8, start: i64| notes.iter().find(|n| n.0 == key && near(n.1, start));
    assert!(has(62, 6 * Q).is_some());
    let held = has(64, end - ms(25)).expect("the held key is in the pass it was pressed in");
    assert!(near(held.1 + held.2, end), "cut at the loop end: {held:?}");
    assert!(has(65, LOOP_START + ms(300)).is_some());
    assert!(has(67, 7 * Q).is_some());
    assert_eq!(notes.len(), 4, "{notes:?}");
}

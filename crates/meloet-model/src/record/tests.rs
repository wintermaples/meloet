use super::*;

/// Bars 2 and 3 in 4/4.
fn bars_2_3() -> Option<TickRange> {
    TickRange::new(1920, 3840)
}

fn on(tick: i64, lap: u32, key: u8) -> RecEvent {
    RecEvent {
        tick,
        lap,
        kind: RecEventKind::NoteOn { key, velocity: 90 },
    }
}

fn off(tick: i64, lap: u32, key: u8) -> RecEvent {
    RecEvent {
        tick,
        lap,
        kind: RecEventKind::NoteOff { key },
    }
}

fn cc(tick: i64, lap: u32, number: u8, value: u8) -> RecEvent {
    RecEvent {
        tick,
        lap,
        kind: RecEventKind::Cc { number, value },
    }
}

fn bend(tick: i64, lap: u32, v: i16) -> RecEvent {
    RecEvent {
        tick,
        lap,
        kind: RecEventKind::PitchBend(v),
    }
}

fn run(
    start: u64,
    loop_range: Option<TickRange>,
    events: &[RecEvent],
    stop: (i64, u32),
) -> Option<Recorded> {
    run_held(start, loop_range, HeldControls::default(), events, stop)
}

fn run_held(
    start: u64,
    loop_range: Option<TickRange>,
    held: HeldControls,
    events: &[RecEvent],
    stop: (i64, u32),
) -> Option<Recorded> {
    let mut r = Recorder::new(start, loop_range, held);
    for &e in events {
        r.push(e);
    }
    r.finish(stop.0, stop.1)
}

/// (start, length, key) of each note.
fn notes(t: &NewTake) -> Vec<(u64, u64, u8)> {
    t.notes.iter().map(|n| (n.start, n.length, n.key)).collect()
}

fn controls(t: &NewTake) -> Vec<(u64, ControlValue)> {
    t.controls.iter().map(|c| (c.tick, c.value)).collect()
}

fn sustain(value: u8) -> ControlValue {
    ControlValue::Cc {
        number: CC_SUSTAIN,
        value,
    }
}

#[test]
fn presses_and_releases_make_notes() {
    let r = run(
        1920,
        None,
        &[
            on(1920, 0, 60),
            on(2000, 0, 64),
            off(2100, 0, 64),
            off(2400, 0, 60),
        ],
        (3000, 0),
    )
    .unwrap();
    assert_eq!(r.active, 0);
    let [t] = &r.takes[..] else { panic!() };
    assert_eq!((t.origin, t.length), (1920, 1080));
    assert_eq!(notes(t), [(0, 480, 60), (80, 100, 64)]);
    assert_eq!(t.notes[0].velocity, 90);
}

#[test]
fn ticks_are_kept_as_they_came() {
    let r = run(0, None, &[on(101, 0, 60), off(358, 0, 60)], (1000, 0)).unwrap();
    assert_eq!(notes(&r.takes[0]), [(101, 257, 60)]);
}

#[test]
fn pressing_again_without_release_ends_the_first_note() {
    let r = run(
        0,
        None,
        &[on(100, 0, 60), on(200, 0, 60), off(300, 0, 60)],
        (1000, 0),
    )
    .unwrap();
    assert_eq!(notes(&r.takes[0]), [(100, 100, 60), (200, 100, 60)]);
}

#[test]
fn velocity_zero_is_a_release() {
    let quiet = RecEvent {
        tick: 300,
        lap: 0,
        kind: RecEventKind::NoteOn {
            key: 60,
            velocity: 0,
        },
    };
    let r = run(0, None, &[on(100, 0, 60), quiet], (1000, 0)).unwrap();
    assert_eq!(notes(&r.takes[0]), [(100, 200, 60)]);
}

#[test]
fn a_note_is_at_least_one_tick() {
    let r = run(0, None, &[on(500, 0, 60), off(500, 0, 60)], (600, 0)).unwrap();
    assert_eq!(notes(&r.takes[0]), [(500, 1, 60)]);
}

#[test]
fn keys_down_at_stop_end_at_stop() {
    let r = run(0, None, &[on(500, 0, 60)], (800, 0)).unwrap();
    assert_eq!(notes(&r.takes[0]), [(500, 300, 60)]);
    assert_eq!(r.takes[0].length, 800);
}

#[test]
fn a_note_before_the_start_moves_the_take_start() {
    let r = run(1920, None, &[on(1900, 0, 60), off(2000, 0, 60)], (2400, 0)).unwrap();
    let t = &r.takes[0];
    assert_eq!((t.origin, t.length), (1900, 500));
    assert_eq!(notes(t), [(0, 100, 60)]);
}

#[test]
fn nothing_goes_before_zero() {
    let r = run(
        0,
        None,
        &[
            on(-50, 0, 59),
            off(-10, 0, 59),
            on(-30, 0, 60),
            off(100, 0, 60),
            cc(-40, 0, CC_SUSTAIN, 127),
        ],
        (200, 0),
    )
    .unwrap();
    let t = &r.takes[0];
    assert_eq!(t.origin, 0);
    assert_eq!(notes(t), [(0, 1, 59), (0, 100, 60)]);
    assert_eq!(controls(t), [(0, sustain(127))]);
}

#[test]
fn no_notes_no_take() {
    assert_eq!(run(0, None, &[], (1000, 0)), None);
    assert_eq!(
        run(
            0,
            None,
            &[cc(10, 0, CC_SUSTAIN, 127), bend(20, 0, 100)],
            (1000, 0)
        ),
        None
    );
    assert_eq!(
        run(1920, bars_2_3(), &[cc(2000, 1, CC_SUSTAIN, 127)], (2000, 2)),
        None
    );
}

#[test]
fn controls_are_kept_in_take_ticks() {
    let r = run(
        960,
        None,
        &[
            cc(900, 0, CC_SUSTAIN, 127),
            on(1000, 0, 60),
            cc(1100, 0, 7, 80),
            bend(1200, 0, -4000),
            cc(1300, 0, CC_SUSTAIN, 0),
            off(1400, 0, 60),
        ],
        (1500, 0),
    )
    .unwrap();
    assert_eq!(
        controls(&r.takes[0]),
        [
            (0, sustain(127)),
            (
                140,
                ControlValue::Cc {
                    number: 7,
                    value: 80
                }
            ),
            (240, ControlValue::PitchBend(-4000)),
            (340, sustain(0)),
        ]
    );
}

#[test]
fn controls_held_at_start_are_written_at_the_head() {
    let held = HeldControls {
        sustain: 127,
        bend: -100,
        modulation: 5,
    };
    let r = run_held(
        960,
        None,
        held,
        &[on(900, 0, 60), off(1000, 0, 60)],
        (1200, 0),
    )
    .unwrap();
    let t = &r.takes[0];
    assert_eq!(t.origin, 900);
    assert_eq!(
        controls(t),
        [
            (0, sustain(127)),
            (0, ControlValue::PitchBend(-100)),
            (
                0,
                ControlValue::Cc {
                    number: CC_MODULATION,
                    value: 5
                }
            ),
        ]
    );
}

#[test]
fn each_pass_is_its_own_take_and_empty_passes_are_skipped() {
    let r = run(
        1920,
        bars_2_3(),
        &[
            on(2000, 0, 60),
            off(2100, 0, 60),
            cc(2000, 1, 7, 1),
            on(2500, 2, 62),
            off(2600, 2, 62),
        ],
        (2700, 2),
    )
    .unwrap();
    assert_eq!(r.takes.len(), 2);
    assert_eq!(r.active, 1);
    let (a, b) = (&r.takes[0], &r.takes[1]);
    assert_eq!((a.origin, a.length), (1920, 1920));
    assert_eq!(notes(a), [(80, 100, 60)]);
    assert_eq!((b.origin, b.length), (1920, 780));
    assert_eq!(notes(b), [(580, 100, 62)]);
}

#[test]
fn shown_take_is_the_last_pass_with_notes() {
    let r = run(
        1920,
        bars_2_3(),
        &[
            on(2000, 0, 60),
            off(2100, 0, 60),
            on(2000, 1, 62),
            off(2100, 1, 62),
            cc(2000, 2, CC_SUSTAIN, 127),
        ],
        (3000, 3),
    )
    .unwrap();
    assert_eq!(r.takes.len(), 2);
    assert_eq!(r.active, 1);
    assert_eq!(notes(&r.takes[1]), [(80, 100, 62)]);
    assert_eq!(
        r.takes[1].length, 1920,
        "a pass that ran to the end is whole"
    );
}

#[test]
fn a_key_held_over_the_loop_end_stays_in_its_pass() {
    let r = run(
        1920,
        bars_2_3(),
        &[
            on(3800, 0, 60),
            off(2000, 1, 60),
            on(2100, 1, 62),
            off(2200, 1, 62),
        ],
        (2300, 1),
    )
    .unwrap();
    assert_eq!(notes(&r.takes[0]), [(1880, 40, 60)]);
    assert_eq!(notes(&r.takes[1]), [(180, 100, 62)]);
}

#[test]
fn a_key_held_through_several_passes_and_the_stop() {
    let r = run(1920, bars_2_3(), &[on(3800, 0, 60)], (2500, 2)).unwrap();
    assert_eq!(r.takes.len(), 1);
    assert_eq!(notes(&r.takes[0]), [(1880, 40, 60)]);
    assert_eq!(r.active, 0);
}

#[test]
fn pressing_again_in_the_next_pass() {
    let r = run(
        1920,
        bars_2_3(),
        &[on(3800, 0, 60), on(2000, 1, 60), off(2100, 1, 60)],
        (2300, 1),
    )
    .unwrap();
    assert_eq!(notes(&r.takes[0]), [(1880, 40, 60)]);
    assert_eq!(notes(&r.takes[1]), [(80, 100, 60)]);
}

#[test]
fn pedal_bend_and_modulation_left_on_go_to_the_next_pass_head() {
    let r = run(
        1920,
        bars_2_3(),
        &[
            cc(3000, 0, CC_SUSTAIN, 127),
            bend(3100, 0, 300),
            cc(3200, 0, CC_MODULATION, 0),
            on(3300, 0, 60),
            off(3400, 0, 60),
            // Pass 1: the pedal comes up; the bend stays.
            on(2000, 1, 62),
            cc(2100, 1, CC_SUSTAIN, 0),
            off(2200, 1, 62),
            on(2000, 2, 64),
            off(2100, 2, 64),
        ],
        (2200, 2),
    )
    .unwrap();
    assert_eq!(r.takes.len(), 3);
    assert_eq!(
        controls(&r.takes[1]),
        [
            (0, sustain(127)),
            (0, ControlValue::PitchBend(300)),
            (180, sustain(0)),
        ]
    );
    assert_eq!(controls(&r.takes[2]), [(0, ControlValue::PitchBend(300))]);
}

#[test]
fn starting_inside_the_loop() {
    let r = run(
        2880,
        bars_2_3(),
        &[
            on(2870, 0, 59),
            off(2875, 0, 59),
            on(2900, 0, 60),
            off(3000, 0, 60),
            on(1920, 1, 62),
            off(2000, 1, 62),
        ],
        (2100, 1),
    )
    .unwrap();
    let (a, b) = (&r.takes[0], &r.takes[1]);
    assert_eq!((a.origin, a.length), (2880, 960));
    // A press just before the start is kept at the start of the pass.
    assert_eq!(notes(a), [(0, 1, 59), (20, 100, 60)]);
    assert_eq!((b.origin, b.length), (1920, 180));
    assert_eq!(notes(b), [(0, 80, 62)]);
}

#[test]
fn a_loop_ending_before_the_start_is_not_looked_at() {
    let r = run(
        4000,
        bars_2_3(),
        &[
            on(4000, 0, 60),
            off(4100, 0, 60),
            on(4200, 3, 62),
            off(4300, 3, 62),
        ],
        (4400, 3),
    )
    .unwrap();
    assert_eq!(r.takes.len(), 1);
    assert_eq!(notes(&r.takes[0]), [(0, 100, 60), (200, 100, 62)]);
}

#[test]
fn live_notes_of_the_pass_being_recorded() {
    let mut r = Recorder::new(0, None, HeldControls::default());
    for e in [on(100, 0, 60), off(200, 0, 60), on(300, 0, 62)] {
        r.push(e);
    }
    let live: Vec<_> = r
        .live_notes(450, 0)
        .iter()
        .map(|n| (n.start, n.length, n.key))
        .collect();
    assert_eq!(live, [(100, 100, 60), (300, 150, 62)]);

    let mut r = Recorder::new(1920, bars_2_3(), HeldControls::default());
    for e in [
        on(3800, 0, 60),
        on(2000, 1, 62),
        off(2050, 1, 62),
        on(2100, 1, 64),
    ] {
        r.push(e);
    }
    let live: Vec<_> = r
        .live_notes(2200, 1)
        .iter()
        .map(|n| (n.start, n.length, n.key))
        .collect();
    assert_eq!(
        live,
        [(2000, 50, 62), (2100, 100, 64)],
        "the key from pass 0 is not in pass 1"
    );
    let pass0: Vec<_> = r
        .live_notes(2200, 0)
        .iter()
        .map(|n| (n.start, n.length, n.key))
        .collect();
    assert_eq!(pass0, [(3800, 40, 60)]);
    assert!(r.live_notes(2200, 2).is_empty());
}

#[test]
fn takes_from_the_recorder_go_into_the_song() {
    let r = run(
        1920,
        bars_2_3(),
        &[
            cc(3000, 0, CC_SUSTAIN, 127),
            on(3800, 0, 60),
            on(2000, 1, 62),
            off(2100, 1, 62),
        ],
        (2500, 1),
    )
    .unwrap();
    let doc = crate::Doc::new_default();
    let (e, added) =
        crate::edit::add_recording(&doc, crate::TrackId(1), r.takes, r.active).unwrap();
    let (_, clip) = e.doc.clip(added.clip).unwrap();
    assert_eq!(
        (clip.start, clip.end, clip.takes.len(), clip.active),
        (1920, 3840, 2, 1)
    );
}

use super::*;
use crate::time::TICKS_PER_QUARTER;

const Q: u64 = TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;
/// `Q` and `BAR` for the ticks of events, which are signed.
const QI: i64 = Q as i64;
const B: i64 = BAR as i64;

/// Bars 2 and 3 in 4/4.
fn bars_2_3() -> Option<TickRange> {
    TickRange::new(BAR, 2 * BAR)
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
        BAR,
        None,
        &[
            on(B, 0, 60),
            on(B + 80, 0, 64),
            off(B + 180, 0, 64),
            off(B + QI, 0, 60),
        ],
        (B + 9 * QI / 4, 0),
    )
    .unwrap();
    assert_eq!(r.active, 0);
    let [t] = &r.takes[..] else { panic!() };
    assert_eq!((t.origin, t.length), (BAR, 9 * Q / 4));
    assert_eq!(notes(t), [(0, Q, 60), (80, 100, 64)]);
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
    let r = run(
        BAR,
        None,
        &[on(B - 20, 0, 60), off(B + 80, 0, 60)],
        (B + QI, 0),
    )
    .unwrap();
    let t = &r.takes[0];
    assert_eq!((t.origin, t.length), (BAR - 20, Q + 20));
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
        run(
            BAR,
            bars_2_3(),
            &[cc(B + 80, 1, CC_SUSTAIN, 127)],
            (B + 80, 2)
        ),
        None
    );
}

#[test]
fn controls_are_kept_in_take_ticks() {
    let r = run(
        2 * Q,
        None,
        &[
            cc(2 * QI - 60, 0, CC_SUSTAIN, 127),
            on(2 * QI + 40, 0, 60),
            cc(2 * QI + 140, 0, 7, 80),
            bend(2 * QI + 240, 0, -4000),
            cc(2 * QI + 340, 0, CC_SUSTAIN, 0),
            off(2 * QI + 440, 0, 60),
        ],
        (2 * QI + 540, 0),
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
        2 * Q,
        None,
        held,
        &[on(2 * QI - 60, 0, 60), off(2 * QI + 40, 0, 60)],
        (2 * QI + 240, 0),
    )
    .unwrap();
    let t = &r.takes[0];
    assert_eq!(t.origin, 2 * Q - 60);
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
        BAR,
        bars_2_3(),
        &[
            on(B + 80, 0, 60),
            off(B + 180, 0, 60),
            cc(B + 80, 1, 7, 1),
            on(B + 580, 2, 62),
            off(B + 680, 2, 62),
        ],
        (B + 780, 2),
    )
    .unwrap();
    assert_eq!(r.takes.len(), 2);
    assert_eq!(r.active, 1);
    let (a, b) = (&r.takes[0], &r.takes[1]);
    assert_eq!((a.origin, a.length), (BAR, BAR));
    assert_eq!(notes(a), [(80, 100, 60)]);
    assert_eq!((b.origin, b.length), (BAR, 780));
    assert_eq!(notes(b), [(580, 100, 62)]);
}

#[test]
fn shown_take_is_the_last_pass_with_notes() {
    let r = run(
        BAR,
        bars_2_3(),
        &[
            on(B + 80, 0, 60),
            off(B + 180, 0, 60),
            on(B + 80, 1, 62),
            off(B + 180, 1, 62),
            cc(B + 80, 2, CC_SUSTAIN, 127),
        ],
        (B + 1080, 3),
    )
    .unwrap();
    assert_eq!(r.takes.len(), 2);
    assert_eq!(r.active, 1);
    assert_eq!(notes(&r.takes[1]), [(80, 100, 62)]);
    assert_eq!(
        r.takes[1].length, BAR,
        "a pass that ran to the end is whole"
    );
}

#[test]
fn a_key_held_over_the_loop_end_stays_in_its_pass() {
    let r = run(
        BAR,
        bars_2_3(),
        &[
            on(2 * B - 40, 0, 60),
            off(B + 80, 1, 60),
            on(B + 180, 1, 62),
            off(B + 280, 1, 62),
        ],
        (B + 380, 1),
    )
    .unwrap();
    assert_eq!(notes(&r.takes[0]), [(BAR - 40, 40, 60)]);
    assert_eq!(notes(&r.takes[1]), [(180, 100, 62)]);
}

#[test]
fn a_key_held_through_several_passes_and_the_stop() {
    let r = run(BAR, bars_2_3(), &[on(2 * B - 40, 0, 60)], (B + 580, 2)).unwrap();
    assert_eq!(r.takes.len(), 1);
    assert_eq!(notes(&r.takes[0]), [(BAR - 40, 40, 60)]);
    assert_eq!(r.active, 0);
}

#[test]
fn pressing_again_in_the_next_pass() {
    let r = run(
        BAR,
        bars_2_3(),
        &[
            on(2 * B - 40, 0, 60),
            on(B + 80, 1, 60),
            off(B + 180, 1, 60),
        ],
        (B + 380, 1),
    )
    .unwrap();
    assert_eq!(notes(&r.takes[0]), [(BAR - 40, 40, 60)]);
    assert_eq!(notes(&r.takes[1]), [(80, 100, 60)]);
}

#[test]
fn pedal_bend_and_modulation_left_on_go_to_the_next_pass_head() {
    let r = run(
        BAR,
        bars_2_3(),
        &[
            cc(B + 1080, 0, CC_SUSTAIN, 127),
            bend(B + 1180, 0, 300),
            cc(B + 1280, 0, CC_MODULATION, 0),
            on(B + 1380, 0, 60),
            off(B + 1480, 0, 60),
            // Pass 1: the pedal comes up; the bend stays.
            on(B + 80, 1, 62),
            cc(B + 180, 1, CC_SUSTAIN, 0),
            off(B + 280, 1, 62),
            on(B + 80, 2, 64),
            off(B + 180, 2, 64),
        ],
        (B + 280, 2),
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
        BAR + 2 * Q,
        bars_2_3(),
        &[
            on(B + 2 * QI - 10, 0, 59),
            off(B + 2 * QI - 5, 0, 59),
            on(B + 2 * QI + 20, 0, 60),
            off(B + 2 * QI + 120, 0, 60),
            on(B, 1, 62),
            off(B + 80, 1, 62),
        ],
        (B + 180, 1),
    )
    .unwrap();
    let (a, b) = (&r.takes[0], &r.takes[1]);
    // A press just before the start stays where it was played; the first take reaches back to it.
    assert_eq!((a.origin, a.length), (BAR + 2 * Q - 10, 2 * Q + 10));
    assert_eq!(notes(a), [(0, 5, 59), (30, 100, 60)]);
    assert_eq!((b.origin, b.length), (BAR, 180));
    assert_eq!(notes(b), [(0, 80, 62)]);
}

#[test]
fn a_loop_ending_before_the_start_is_not_looked_at() {
    let r = run(
        2 * BAR + 160,
        bars_2_3(),
        &[
            on(2 * B + 160, 0, 60),
            off(2 * B + 260, 0, 60),
            on(2 * B + 360, 3, 62),
            off(2 * B + 460, 3, 62),
        ],
        (2 * B + 560, 3),
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

    let mut r = Recorder::new(BAR, bars_2_3(), HeldControls::default());
    for e in [
        on(2 * B - 40, 0, 60),
        on(B + 80, 1, 62),
        off(B + 130, 1, 62),
        on(B + 180, 1, 64),
    ] {
        r.push(e);
    }
    let live: Vec<_> = r
        .live_notes(B + 280, 1)
        .iter()
        .map(|n| (n.start, n.length, n.key))
        .collect();
    assert_eq!(
        live,
        [(BAR + 80, 50, 62), (BAR + 180, 100, 64)],
        "the key from pass 0 is not in pass 1"
    );
    let pass0: Vec<_> = r
        .live_notes(B + 280, 0)
        .iter()
        .map(|n| (n.start, n.length, n.key))
        .collect();
    assert_eq!(pass0, [(2 * BAR - 40, 40, 60)]);
    assert!(r.live_notes(B + 280, 2).is_empty());
}

#[test]
fn takes_from_the_recorder_go_into_the_song() {
    let r = run(
        BAR,
        bars_2_3(),
        &[
            cc(B + 1080, 0, CC_SUSTAIN, 127),
            on(2 * B - 40, 0, 60),
            on(B + 80, 1, 62),
            off(B + 180, 1, 62),
        ],
        (B + 580, 1),
    )
    .unwrap();
    let doc = crate::Doc::new_default();
    let (e, added) =
        crate::edit::add_recording(&doc, crate::TrackId(1), r.takes, r.active).unwrap();
    let (_, clip) = e.doc.clip(added.clip).unwrap();
    assert_eq!(
        (clip.start, clip.end, clip.takes.len(), clip.active),
        (BAR, 2 * BAR, 2, 1)
    );
}

#[test]
fn a_key_pressed_just_before_the_loop_start_widens_only_the_first_take() {
    let r = run(
        BAR,
        bars_2_3(),
        &[
            on(B - 40, 0, 60),
            off(B + 70, 0, 60),
            on(2 * B - 10, 0, 62),
            on(B + 30, 1, 64),
            off(B + 80, 1, 64),
        ],
        (B + 180, 1),
    )
    .unwrap();
    let (a, b) = (&r.takes[0], &r.takes[1]);
    assert_eq!((a.origin, a.length), (BAR - 40, BAR + 40));
    assert_eq!(notes(a), [(0, 110, 60), (BAR + 30, 10, 62)]);
    assert_eq!((b.origin, b.length), (BAR, 180));
    assert_eq!(notes(b), [(30, 50, 64)]);

    let mut live = Recorder::new(BAR, bars_2_3(), HeldControls::default());
    live.push(on(B - 40, 0, 60));
    let n = live.live_notes(B - 20, 0);
    assert_eq!((n[0].start, n[0].length), (BAR - 40, 20));
}

#[test]
fn the_first_loop_take_does_not_go_before_zero() {
    let r = run(
        0,
        TickRange::new(0, BAR),
        &[on(-40, 0, 60), off(100, 0, 60)],
        (200, 0),
    )
    .unwrap();
    assert_eq!(r.takes[0].origin, 0);
    assert_eq!(notes(&r.takes[0]), [(0, 100, 60)]);
}

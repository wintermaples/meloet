use meloet_model::record::RecEventKind;
use meloet_protocol::{
    ClipView, Command, StatusMessage, StepName, StepWhat, TakeNotes, TransportState,
};

use super::super::tests::{cmd, event, hear, off, on, ready, record_once, report, started, status};
use super::super::{EngineOp, RecordReport};
use super::*;
use crate::state::{Effect, Input, Model};

const Q: u64 = TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;
/// At 120: a quarter note is half a second.
const TICK_US: f64 = 500_000.0 / Q as f64;
const S: u64 = 1_000_000;

/// Ready to play at 120 in 4/4, with nothing played yet.
fn at_120() -> Model {
    let mut m = ready();
    cmd(&mut m, Command::SetTempo { bpm: 120.0 });
    m
}

fn still(run: u32, t_us: u64, kind: RecEventKind) -> Played {
    Played::Key {
        kind,
        t_us,
        run,
        at: None,
    }
}

fn moving(run: u32, tick: f64, lap: u32, kind: RecEventKind) -> Played {
    Played::Key {
        kind,
        t_us: (tick * TICK_US) as u64 + u64::from(lap) * 100 * S,
        run,
        at: Some(PlayedAt { tick, lap }),
    }
}

fn ended(run: u32, t_us: u64, at: Option<(f64, u32)>) -> Played {
    Played::RunEnded {
        run,
        t_us,
        at: at.map(|(tick, lap)| PlayedAt { tick, lap }),
    }
}

fn played(m: &mut Model, items: &[Played]) {
    played_holding(m, items, HeldNow::default());
}

fn played_holding(m: &mut Model, items: &[Played], held: HeldNow) {
    let items = items.to_vec();
    assert_eq!(m.handle(Input::Played { items, held }), []);
}

fn pedal(value: u8) -> RecEventKind {
    RecEventKind::Cc { number: 64, value }
}

/// 「さっきのを残す」 at `now_us`. Returns what the engine is told.
fn keep(m: &mut Model, now_us: u64) -> Vec<EngineOp> {
    m.set_now(now_us, None);
    ops(cmd(m, Command::KeepPlayed))
}

fn ops(effects: Vec<Effect>) -> Vec<EngineOp> {
    (effects.into_iter())
        .filter_map(|e| match e {
            Effect::Engine(op) => Some(op),
            _ => None,
        })
        .collect()
}

fn can_keep(m: &Model) -> bool {
    m.view().transport.can_keep_played
}

fn clips(m: &Model) -> Vec<ClipView> {
    m.view().song.tracks[0].clips.clone()
}

/// (key, start, length) of the shown take, at their places in the song.
fn notes(m: &Model, clip: &ClipView) -> Vec<(u8, i64, u32)> {
    let take = TakeNotes::decode(&m.take_notes(clip.take_id).unwrap()).unwrap();
    let zero = clip.start as i64 + clip.take_offset;
    (take.notes.iter())
        .map(|n| (n.key, zero + i64::from(n.start), n.length))
        .collect()
}

fn last_row(m: &Model) -> (StepName, StepWhat) {
    let n = m.view().song.undo_count;
    let row = m.history_rows(n, n).pop().expect("a row");
    (row.name, row.what)
}

#[test]
fn with_nothing_played_there_is_nothing_to_keep() {
    let mut m = at_120();
    let steps = m.view().song.undo_count;
    assert!(!can_keep(&m));
    assert_eq!(keep(&mut m, S), []);
    assert_eq!(status(&m), Some(StatusMessage::NothingToKeep));
    // Controllers alone are no phrase.
    played(&mut m, &[still(0, S, pedal(127)), still(0, 2 * S, off(60))]);
    assert!(!can_keep(&m));
    assert_eq!(keep(&mut m, 3 * S), []);
    assert_eq!(m.view().song.undo_count, steps);
    assert!(clips(&m).is_empty());
}

#[test]
fn what_was_played_while_stopped_starts_at_the_bar_of_the_play_line() {
    let mut m = at_120();
    cmd(&mut m, Command::Locate { tick: BAR + 700 });
    let steps = m.view().song.undo_count;
    // The first key is where the phrase starts, whenever it was played. The second is still
    // down when the phrase is kept, a second and a half after the first.
    played(&mut m, &[still(0, 10 * S, on(60))]);
    assert!(can_keep(&m));
    played(
        &mut m,
        &[
            still(0, 10 * S + 250_000, off(60)),
            still(0, 10 * S + 500_000 + 200, on(64)),
        ],
    );
    let told = keep(&mut m, 10 * S + 1_500_000);
    assert!(
        matches!(&told[..], [EngineOp::Sequence { .. }]),
        "only the notes: {told:?}"
    );
    let view = m.view();
    assert_eq!(view.transport.state, TransportState::Stopped);
    assert_eq!(view.song.undo_count, steps + 1);
    assert_eq!(status(&m), None);
    let clip = &clips(&m)[0];
    assert_eq!((clip.start, clip.end, clip.takes), (BAR, 2 * BAR, 1));
    // Not moved to any grid: 200 us is three quarters of a tick at this tempo.
    assert_eq!(
        notes(&m, clip),
        [
            (60, BAR as i64, (Q / 2) as u32),
            (64, (BAR + Q) as i64 + 1, 2 * Q as u32 - 1)
        ]
    );
    assert_eq!(view.arrange.selected_clip, Some(clip.id));
    assert_eq!(
        last_row(&m),
        (StepName::KeepPlayed, StepWhat::Take { number: 1 })
    );

    // Kept once: the phrase is gone, and the key let go later starts nothing.
    assert!(!can_keep(&m));
    played(&mut m, &[still(0, 12 * S, off(64))]);
    assert_eq!(keep(&mut m, 13 * S), []);
    assert_eq!(status(&m), Some(StatusMessage::NothingToKeep));
    assert_eq!(m.view().song.undo_count, steps + 1);

    cmd(&mut m, Command::Undo);
    assert!(clips(&m).is_empty());
    assert!(!can_keep(&m));
    cmd(&mut m, Command::Redo);
    assert_eq!(clips(&m).len(), 1);
}

#[test]
fn a_phrase_longer_than_a_bar_ends_on_a_bar_line() {
    let mut m = at_120();
    played(
        &mut m,
        &[
            still(0, S, on(60)),
            still(0, S + 100_000, off(60)),
            still(0, 3 * S, on(62)),
            still(0, 3 * S + 100_000, off(62)),
        ],
    );
    // Kept exactly two bars after the first key: the clip ends there, not a bar later.
    keep(&mut m, 5 * S);
    let clip = &clips(&m)[0];
    assert_eq!((clip.start, clip.end), (0, 2 * BAR));
    assert_eq!(
        notes(&m, clip)
            .iter()
            .map(|n| (n.0, n.1))
            .collect::<Vec<_>>(),
        [(60, 0), (62, BAR as i64)]
    );
}

#[test]
fn after_two_bars_with_no_key_and_no_pedal_held_the_next_key_starts_anew() {
    // Two bars at 120 are 4 seconds.
    let phrase = |gap_us: u64, pedal_held: bool| {
        let mut m = at_120();
        played(&mut m, &[still(0, S, on(60))]);
        if pedal_held {
            played(&mut m, &[still(0, S + 10, pedal(127))]);
        }
        played(
            &mut m,
            &[
                still(0, S + 500_000, off(60)),
                still(0, S + 500_000 + gap_us, on(72)),
                still(0, S + 600_000 + gap_us, off(72)),
            ],
        );
        keep(&mut m, 20 * S);
        let clip = clips(&m)[0].clone();
        (notes(&m, &clip), clip.end)
    };
    let (kept, _) = phrase(4 * S + 1, false);
    assert_eq!(kept, [(72, 0, (Q / 5) as u32)], "only the new phrase");
    let (kept, _) = phrase(4 * S, false);
    assert_eq!(kept.len(), 2, "two bars exactly is not yet past them");
    assert_eq!(kept[1].1, (9 * Q) as i64);
    let (kept, _) = phrase(9 * S, true);
    assert_eq!(kept.len(), 2, "the pedal holds the phrase together");

    // The time is that of two bars at the tempo of the play line now.
    let mut m = at_120();
    cmd(&mut m, Command::SetTempo { bpm: 60.0 });
    played(
        &mut m,
        &[
            still(0, S, on(60)),
            still(0, 2 * S, off(60)),
            still(0, 9 * S, on(72)),
            still(0, 10 * S, off(72)),
        ],
    );
    keep(&mut m, 20 * S);
    assert_eq!(notes(&m, &clips(&m)[0]).len(), 2, "7 s is less than 8 s");
}

#[test]
fn starting_and_stopping_the_play_closes_the_phrase_and_the_next_key_replaces_it() {
    let mut m = at_120();
    // Played while stopped with a key still down, then the play starts.
    played(
        &mut m,
        &[
            still(0, S, on(60)),
            ended(0, 2 * S, None),
            // Let go while the song plays: not of the phrase, which ended with its run.
            moving(1, 500.0, 0, off(60)),
            moving(1, 600.0, 0, pedal(127)),
        ],
    );
    assert!(can_keep(&m));
    keep(&mut m, 50 * S);
    let clip = clips(&m)[0].clone();
    assert_eq!((clip.start, clip.end), (0, BAR));
    assert_eq!(
        notes(&m, &clip),
        [(60, 0, 2 * Q as u32)],
        "cut where the run ended"
    );
    cmd(&mut m, Command::Undo);

    // Played while the song plays, then stopped with a key still down.
    played(
        &mut m,
        &[
            moving(1, (BAR + 10) as f64, 0, on(62)),
            moving(1, (BAR + 500) as f64 + 0.4, 0, off(62)),
            moving(1, (2 * BAR - 100) as f64, 0, on(65)),
            // A key the stop found played after its own time comes before the run's end.
            still(2, 60 * S, off(65)),
            ended(1, 59 * S, Some(((2 * BAR + 300) as f64, 0))),
        ],
    );
    keep(&mut m, 70 * S);
    let clip = clips(&m)[0].clone();
    assert_eq!((clip.start, clip.end), (BAR, 3 * BAR));
    assert_eq!(
        notes(&m, &clip),
        [
            (62, (BAR + 10) as i64, 490),
            (65, (2 * BAR - 100) as i64, 400)
        ]
    );
    // The pedal that was down before the phrase began is at its head.
    let take = TakeNotes::decode(&m.take_notes(clip.take_id).unwrap()).unwrap();
    assert_eq!(take.controls.len(), 1, "{:?}", take.controls);
    assert_eq!(take.controls[0].tick, 0);
    cmd(&mut m, Command::Undo);

    // A closed phrase waits until a key is pressed in a later run.
    played(
        &mut m,
        &[
            still(2, 80 * S, on(40)),
            still(2, 81 * S, off(40)),
            ended(2, 82 * S, None),
            moving(3, 100.0, 0, pedal(0)),
        ],
    );
    assert!(can_keep(&m), "what is not a key press replaces nothing");
    played(
        &mut m,
        &[
            moving(3, 200.0, 0, on(41)),
            moving(3, 300.0, 0, off(41)),
            ended(3, 90 * S, Some((5000.0, 0))),
        ],
    );
    keep(&mut m, 95 * S);
    assert_eq!(notes(&m, &clips(&m)[0]), [(41, 200, 100)]);
}

#[test]
fn kept_while_the_song_still_plays_held_keys_end_where_the_listener_is() {
    let mut m = at_120();
    cmd(&mut m, Command::PlayStop);
    played(&mut m, &[ended(0, S, None), moving(1, 100.0, 0, on(60))]);
    m.set_now(2 * S, None);
    m.set_heard_run(Some(HeardRun {
        run: 1,
        at: Some(PlayedAt {
            tick: 2500.4,
            lap: 0,
        }),
    }));
    let told = ops(cmd(&mut m, Command::KeepPlayed));
    assert!(
        !told
            .iter()
            .any(|op| matches!(op, EngineOp::Stop { .. } | EngineOp::Play { .. })),
        "{told:?}"
    );
    assert_eq!(m.view().transport.state, TransportState::Playing);
    let clip = clips(&m)[0].clone();
    assert_eq!((clip.start, clip.end), (0, BAR));
    assert_eq!(notes(&m, &clip), [(60, 100, 2400)]);
    cmd(&mut m, Command::Undo);

    // What is heard is of another run (the engine has moved on): the phrase ends with its
    // last event.
    played(&mut m, &[moving(1, 3000.0, 0, on(61))]);
    m.set_heard_run(Some(HeardRun { run: 2, at: None }));
    cmd(&mut m, Command::KeepPlayed);
    assert_eq!(notes(&m, &clips(&m)[0]), [(61, 3000, 1)]);
}

fn looping(m: &mut Model) {
    cmd(
        m,
        Command::SetLoop {
            from_bar: 2,
            to_bar: 3,
        },
    );
}

#[test]
fn each_time_round_the_loop_is_a_take_and_all_of_them_one_step() {
    let mut m = at_120();
    looping(&mut m);
    let steps = m.view().song.undo_count;
    let b = BAR as f64;
    played(
        &mut m,
        &[
            // Before the loop, into it, and held over its end.
            moving(1, b - 200.0, 0, on(60)),
            moving(1, b + 100.0, 0, off(60)),
            moving(1, 2.0 * b - 50.0, 0, on(62)),
            moving(1, b + 30.0, 1, off(62)),
            // Nothing in the second time round; a key in the third, still down at the end.
            moving(1, b + 400.0, 2, on(64)),
            ended(1, 900 * S, Some((b + 1000.0, 2))),
        ],
    );
    keep(&mut m, 901 * S);
    assert_eq!(m.view().song.undo_count, steps + 1);
    let clip = clips(&m)[0].clone();
    assert_eq!(
        (clip.start, clip.end, clip.takes, clip.take),
        (0, 2 * BAR, 2, 2)
    );
    assert_eq!(notes(&m, &clip), [(64, BAR as i64 + 400, 600)]);
    assert_eq!(
        last_row(&m),
        (StepName::KeepPlayed, StepWhat::Take { number: 2 })
    );
    cmd(&mut m, Command::StepTake { delta: -1 });
    let first = clips(&m)[0].clone();
    assert_eq!(
        notes(&m, &first),
        [(60, BAR as i64 - 200, 300), (62, 2 * BAR as i64 - 50, 50)],
        "the key held over the loop's end is cut there"
    );
    cmd(&mut m, Command::Undo);
    cmd(&mut m, Command::Undo);
    assert!(clips(&m).is_empty());

    // With the loop off the times round are not looked at: one take.
    cmd(&mut m, Command::ToggleLoop);
    played(
        &mut m,
        &[
            moving(3, b, 0, on(60)),
            moving(3, b + 100.0, 0, off(60)),
            ended(3, 950 * S, Some((b + 200.0, 0))),
        ],
    );
    keep(&mut m, 951 * S);
    assert_eq!(clips(&m)[0].takes, 1);
}

#[test]
fn with_overdub_on_the_phrase_goes_into_the_take_that_is_there() {
    let mut m = at_120();
    record_once(&mut m, &[(36, 0.0, 200.0)], BAR as f64);
    let before = clips(&m)[0].clone();
    assert_eq!((before.takes, before.note_count), (1, 1));
    let steps = m.view().song.undo_count;
    cmd(&mut m, Command::SetOverdub { on: true });
    played(
        &mut m,
        &[
            moving(5, 960.0, 0, on(38)),
            moving(5, 1100.0, 0, off(38)),
            ended(5, 5 * S, Some((1500.0, 0))),
        ],
    );
    keep(&mut m, 6 * S);
    let clip = clips(&m)[0].clone();
    assert_eq!((clip.id, clip.takes, clip.note_count), (before.id, 1, 2));
    assert_eq!(notes(&m, &clip), [(36, 0, 200), (38, 960, 140)]);
    assert_eq!(m.view().song.undo_count, steps + 1);
    assert_eq!(last_row(&m).0, StepName::KeepPlayed);
    cmd(&mut m, Command::Undo);
    assert_eq!(notes(&m, &clips(&m)[0]), [(36, 0, 200)]);

    // Without it the phrase is another take of the clip there.
    cmd(&mut m, Command::SetOverdub { on: false });
    cmd(&mut m, Command::Locate { tick: 0 });
    played(
        &mut m,
        &[still(6, 7 * S, on(40)), still(6, 7 * S + 100_000, off(40))],
    );
    keep(&mut m, 8 * S);
    let clip = clips(&m)[0].clone();
    assert_eq!((clip.id, clip.takes, clip.take), (before.id, 2, 2));
    assert_eq!(
        last_row(&m),
        (StepName::KeepPlayed, StepWhat::Take { number: 2 })
    );
}

#[test]
fn nothing_is_kept_while_counting_in_or_recording_and_a_recording_forgets_the_phrase() {
    let mut m = at_120();
    played(&mut m, &[still(0, S, on(60)), still(0, S + 1000, off(60))]);
    assert!(can_keep(&m));
    cmd(&mut m, Command::SetCountIn { bars: 1 });
    cmd(&mut m, Command::Record);
    assert_eq!(m.view().transport.state, TransportState::CountIn);
    assert!(!can_keep(&m), "not while counting in");
    let steps = m.view().song.undo_count;
    assert_eq!(keep(&mut m, 2 * S), []);
    assert_eq!(m.view().song.undo_count, steps);
    assert_ne!(status(&m), Some(StatusMessage::NothingToKeep));
    started(&mut m);
    hear(&mut m, 3 * S, 10.0, TransportState::Recording, 0);
    assert_eq!(keep(&mut m, 3 * S), []);
    assert_eq!(m.view().song.undo_count, steps);
    event(&mut m, on(70), 3 * S, 20.0, 0);
    event(&mut m, off(70), 3 * S, 90.0, 0);
    cmd(&mut m, Command::Record);
    report(
        &mut m,
        RecordReport::Stopped {
            t_us: 4 * S,
            tick: 500.0,
            lap: 0,
        },
    );
    assert_eq!(m.view().song.undo_count, steps + 1, "the recording");
    assert!(!can_keep(&m), "what was played before it is gone");
    assert_eq!(keep(&mut m, 5 * S), []);
    assert_eq!(status(&m), Some(StatusMessage::NothingToKeep));
}

#[test]
fn another_keys_target_and_another_song_forget_the_phrase() {
    let mut m = at_120();
    let phrase = [still(0, S, on(60)), still(0, S + 1000, off(60))];
    played(&mut m, &phrase);
    cmd(&mut m, Command::DuplicateTrack { track: 1 });
    assert_eq!(m.view().song.tracks.len(), 2);
    assert!(!can_keep(&m), "the copy is the keys target now");
    cmd(
        &mut m,
        Command::SelectTrack {
            track: 1,
            from_arm: false,
        },
    );
    assert!(!can_keep(&m), "going back does not bring it back");

    // The phrase goes to the track that was the target when it was played.
    played(&mut m, &phrase);
    assert!(can_keep(&m));
    keep(&mut m, 2 * S);
    let view = m.view();
    assert_eq!(
        (
            view.song.tracks[0].clips.len(),
            view.song.tracks[1].clips.len()
        ),
        (1, 0)
    );

    // A song with nothing to save gives way at once.
    let mut m = ready();
    played(&mut m, &phrase);
    assert!(can_keep(&m));
    cmd(&mut m, Command::NewSong);
    assert!(!can_keep(&m));
}

#[test]
fn a_phrase_holds_4096_events_and_ten_minutes_and_lets_the_oldest_go() {
    let mut m = at_120();
    // 3000 keys, each pressed and let go 10 ms apart: 6000 events.
    let items: Vec<Played> = (0..3000u64)
        .flat_map(|i| {
            let key = 30 + (i % 60) as u8;
            [
                still(0, S + i * 20_000, on(key)),
                still(0, S + i * 20_000 + 10_000, off(key)),
            ]
        })
        .collect();
    played(&mut m, &items);
    keep(&mut m, 62 * S);
    let clip = clips(&m)[0].clone();
    let kept = notes(&m, &clip);
    assert_eq!(kept.len(), MAX_EVENTS / 2);
    // The oldest key left is where the phrase starts now.
    let first = 3000 - MAX_EVENTS as u64 / 2;
    assert_eq!((kept[0].0, kept[0].1), (30 + (first % 60) as u8, 0));
    cmd(&mut m, Command::Undo);

    // A key from more than ten minutes before the newest goes, with what followed it.
    played(
        &mut m,
        &[
            still(0, 100 * S, on(50)),
            still(0, 100 * S + 1, pedal(100)),
            still(0, 101 * S, on(52)),
            still(0, 102 * S, off(50)),
            still(0, 700 * S + 1, off(52)),
        ],
    );
    keep(&mut m, 701 * S);
    let clip = clips(&m)[0].clone();
    assert_eq!(
        notes(&m, &clip).iter().map(|n| n.0).collect::<Vec<_>>(),
        [52]
    );
    let take = TakeNotes::decode(&m.take_notes(clip.take_id).unwrap()).unwrap();
    assert_eq!(
        take.controls.len(),
        1,
        "the pedal it was played under stays"
    );
}

#[test]
fn a_keyboard_that_goes_away_lets_go_of_the_keys_of_the_phrase() {
    let mut m = at_120();
    played(&mut m, &[still(0, S, on(60)), still(0, S + 10, pedal(127))]);
    m.set_now(S + 500_000, None);
    m.handle(Input::KeyboardRemoved { id: "kb1".into() });
    keep(&mut m, 3 * S);
    let clip = clips(&m)[0].clone();
    assert_eq!(notes(&m, &clip), [(60, 0, Q as u32)]);
    let take = TakeNotes::decode(&m.take_notes(clip.take_id).unwrap()).unwrap();
    assert_eq!(take.controls.len(), 2, "the pedal goes up with the keys");
}

#[test]
fn one_key_held_under_thousands_of_controller_moves_stays() {
    let mut m = at_120();
    let mut items = vec![still(0, S, on(60))];
    items.extend((0..5000u64).map(|i| {
        let kind = RecEventKind::Cc {
            number: 1,
            value: (i % 128) as u8,
        };
        still(0, S + 1000 + i * 100, kind)
    }));
    played(&mut m, &items);
    assert!(can_keep(&m));
    keep(&mut m, 2 * S);
    let clip = clips(&m)[0].clone();
    assert_eq!(notes(&m, &clip), [(60, 0, 2 * Q as u32)]);
    let take = TakeNotes::decode(&m.take_notes(clip.take_id).unwrap()).unwrap();
    assert_eq!(take.controls.len(), MAX_EVENTS - 1, "the newest moves");
}

#[test]
fn one_key_let_go_under_thousands_of_controller_moves_keeps_its_length() {
    let mut m = at_120();
    let mut items = vec![still(0, S, on(60)), still(0, S + 250_000, off(60))];
    items.extend((0..5000u64).map(|i| {
        let kind = RecEventKind::Cc {
            number: 1,
            value: (i % 128) as u8,
        };
        still(0, S + 251_000 + i * 100, kind)
    }));
    played(&mut m, &items);
    keep(&mut m, 2 * S);
    let clip = clips(&m)[0].clone();
    assert_eq!(notes(&m, &clip), [(60, 0, (Q / 2) as u32)]);
    let take = TakeNotes::decode(&m.take_notes(clip.take_id).unwrap()).unwrap();
    assert_eq!(take.controls.len(), MAX_EVENTS - 2, "the newest moves");
}

#[test]
fn what_is_held_is_read_anew_after_a_recording_took_the_keys() {
    let pedal_down = HeldNow {
        keys: 0,
        controls: HeldControls {
            sustain: 127,
            ..HeldControls::default()
        },
    };
    let mut m = at_120();
    // The pedal goes down, and comes up while a recording takes the keys: nothing says so
    // here.
    played_holding(&mut m, &[still(0, S, pedal(127))], pedal_down);
    record_once(&mut m, &[(36, 0.0, 200.0)], BAR as f64);
    cmd(&mut m, Command::Undo);
    played(
        &mut m,
        &[still(2, 20 * S, on(60)), still(2, 20 * S + 1000, off(60))],
    );
    // Nothing was held for more than two bars: a new phrase, with no pedal at its head.
    played(
        &mut m,
        &[still(2, 30 * S, on(62)), still(2, 30 * S + 1000, off(62))],
    );
    keep(&mut m, 31 * S);
    let clip = clips(&m)[0].clone();
    assert_eq!(
        notes(&m, &clip).iter().map(|n| n.0).collect::<Vec<_>>(),
        [62]
    );
    let take = TakeNotes::decode(&m.take_notes(clip.take_id).unwrap()).unwrap();
    assert_eq!(take.controls, []);
    cmd(&mut m, Command::Undo);

    // And the other way round: it went down during the recording and is still down.
    record_once(&mut m, &[(36, 0.0, 200.0)], BAR as f64);
    cmd(&mut m, Command::Undo);
    played_holding(&mut m, &[still(4, 40 * S, on(64))], pedal_down);
    keep(&mut m, 41 * S);
    let clip = clips(&m)[0].clone();
    let take = TakeNotes::decode(&m.take_notes(clip.take_id).unwrap()).unwrap();
    assert_eq!(
        take.controls.len(),
        1,
        "the pedal is at the head of the take"
    );
}

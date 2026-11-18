use std::sync::Arc;

use super::*;
use crate::edit::{NewTake, add_recording, switch_take};
use crate::song::{Clip, ClipId, ClipTake, ControlPoint, Note, Take, TakeId};

const T1: TrackId = TrackId(1);

fn note(start: u64, length: u64, key: u8) -> Note {
    Note {
        from: None,
        start,
        length,
        key,
        velocity: 80,
    }
}

fn point(tick: u64, value: ControlValue) -> ControlPoint {
    ControlPoint { tick, value }
}

fn sustain(value: u8) -> ControlValue {
    ControlValue::Cc {
        number: CC_SUSTAIN,
        value,
    }
}

fn modulation(value: u8) -> ControlValue {
    ControlValue::Cc {
        number: CC_MODULATION,
        value,
    }
}

fn new_take(origin: u64, length: u64, notes: Vec<Note>, controls: Vec<ControlPoint>) -> NewTake {
    NewTake {
        origin,
        length,
        notes,
        controls,
    }
}

/// A short form for comparing: (tick, what).
fn short(events: &[PlayEvent]) -> Vec<(u64, String)> {
    events
        .iter()
        .map(|e| {
            let what = match e.kind {
                PlayKind::NoteOn { key, .. } => format!("on {key}"),
                PlayKind::NoteOff { key } => format!("off {key}"),
                PlayKind::Control(ControlValue::Cc { number, value }) => {
                    format!("cc{number} {value}")
                }
                PlayKind::Control(ControlValue::PitchBend(v)) => format!("bend {v}"),
            };
            (e.tick, what)
        })
        .collect()
}

fn ev(list: &[(u64, &str)]) -> Vec<(u64, String)> {
    list.iter().map(|&(t, s)| (t, s.to_owned())).collect()
}

/// A doc whose one track has `clip`, with ids that do not clash.
fn doc_with(clip: Clip) -> Doc {
    let mut doc = Doc::new_default();
    Arc::make_mut(&mut doc.tracks[0]).clips.push(Arc::new(clip));
    doc.next_id = 100;
    doc
}

fn clip(start: u64, end: u64, offset: i64, take: Take) -> Clip {
    Clip {
        id: ClipId(1),
        name: "ピアノ".to_owned(),
        start,
        end,
        takes: vec![ClipTake {
            take: Arc::new(take),
            offset,
        }],
        active: 0,
    }
}

#[test]
fn notes_at_song_positions() {
    let doc = Doc::new_default();
    let take = new_take(
        1920,
        1080,
        vec![note(0, 480, 60), note(80, 100, 64)],
        vec![],
    );
    let (e, _) = add_recording(&doc, T1, vec![take], 0).unwrap();
    let events = track_events(&e.doc, T1);
    assert_eq!(
        short(&events),
        ev(&[
            (1920, "on 60"),
            (2000, "on 64"),
            (2100, "off 64"),
            (2400, "off 60")
        ])
    );
    assert_eq!(
        events[0].kind,
        PlayKind::NoteOn {
            key: 60,
            velocity: 80
        }
    );
    assert!(track_events(&e.doc, TrackId(9)).is_empty());
}

#[test]
fn the_shown_take_plays() {
    let doc = Doc::new_default();
    let a = new_take(0, 960, vec![note(0, 10, 60)], vec![]);
    let b = new_take(0, 960, vec![note(5, 10, 72)], vec![]);
    let (e, added) = add_recording(&doc, T1, vec![a, b], 1).unwrap();
    assert_eq!(
        short(&track_events(&e.doc, T1)),
        ev(&[(5, "on 72"), (15, "off 72")])
    );
    let e = switch_take(&e.doc, added.clip, 0).unwrap();
    assert_eq!(
        short(&track_events(&e.doc, T1)),
        ev(&[(0, "on 60"), (10, "off 60")])
    );
}

#[test]
fn takes_are_offset_inside_their_clip() {
    let doc = Doc::new_default();
    let (e, _) = add_recording(
        &doc,
        T1,
        vec![new_take(1000, 500, vec![note(0, 10, 60)], vec![])],
        0,
    )
    .unwrap();
    let (e, _) = add_recording(
        &e.doc,
        T1,
        vec![new_take(500, 600, vec![note(0, 10, 61)], vec![])],
        0,
    )
    .unwrap();
    assert_eq!(
        short(&track_events(&e.doc, T1)),
        ev(&[(500, "on 61"), (510, "off 61")])
    );
    let (_, clip) = e.doc.clip(ClipId(1)).unwrap();
    let e = switch_take(&e.doc, clip.id, 0).unwrap();
    assert_eq!(
        short(&track_events(&e.doc, T1)),
        ev(&[(1000, "on 60"), (1010, "off 60")])
    );
}

#[test]
fn nothing_sounds_after_the_clip_end() {
    // A take longer than its clip (as it will be once clips can be cut shorter).
    let take = Take {
        origin: None,
        id: TakeId(2),
        length: 3000,
        notes: vec![note(0, 50, 59), note(900, 200, 62), note(1100, 10, 64)].into(),
        controls: vec![
            point(
                50,
                ControlValue::Cc {
                    number: 7,
                    value: 3,
                },
            ),
            point(1500, sustain(127)),
        ]
        .into(),
    };
    let doc = doc_with(clip(1000, 2000, 0, take));
    assert_eq!(
        short(&track_events(&doc, T1)),
        ev(&[
            (1000, "on 59"),
            (1050, "off 59"),
            (1050, "cc7 3"),
            (1900, "on 62"),
            (2000, "off 62"),
        ])
    );
}

#[test]
fn same_tick_order_is_release_controls_press() {
    let take = new_take(
        0,
        960,
        vec![note(0, 100, 60), note(100, 100, 60)],
        vec![point(100, sustain(127)), point(150, sustain(0))],
    );
    let (e, _) = add_recording(&Doc::new_default(), T1, vec![take], 0).unwrap();
    assert_eq!(
        short(&track_events(&e.doc, T1)),
        ev(&[
            (0, "on 60"),
            (100, "off 60"),
            (100, "cc64 127"),
            (100, "on 60"),
            (150, "cc64 0"),
            (200, "off 60"),
        ])
    );
}

#[test]
fn controls_left_on_are_put_back_at_the_clip_end() {
    let take = new_take(
        0,
        960,
        vec![note(0, 10, 60)],
        vec![
            point(0, sustain(127)),
            point(5, ControlValue::PitchBend(200)),
            point(6, modulation(30)),
            point(
                7,
                ControlValue::Cc {
                    number: 7,
                    value: 90,
                },
            ),
        ],
    );
    let (e, _) = add_recording(&Doc::new_default(), T1, vec![take], 0).unwrap();
    let events = short(&track_events(&e.doc, T1));
    assert_eq!(
        events[events.len() - 3..],
        ev(&[(960, "cc64 0"), (960, "bend 0"), (960, "cc1 0")])
    );

    let take = new_take(
        0,
        960,
        vec![note(0, 10, 60)],
        vec![point(0, sustain(127)), point(500, sustain(0))],
    );
    let (e, _) = add_recording(&Doc::new_default(), T1, vec![take], 0).unwrap();
    assert_eq!(
        short(&track_events(&e.doc, T1)).last(),
        Some(&(500, "cc64 0".to_owned()))
    );
}

#[test]
fn back_to_back_clips() {
    let a = new_take(
        0,
        1920,
        vec![note(1800, 120, 60)],
        vec![point(100, sustain(127))],
    );
    let b = new_take(
        1920,
        960,
        vec![note(0, 10, 62)],
        vec![point(0, sustain(127))],
    );
    let (e, _) = add_recording(&Doc::new_default(), T1, vec![a], 0).unwrap();
    let (e, _) = add_recording(&e.doc, T1, vec![b], 0).unwrap();
    assert_eq!(e.doc.tracks[0].clips.len(), 2);
    let events = short(&track_events(&e.doc, T1));
    assert_eq!(
        events[2..6],
        ev(&[
            (1920, "off 60"),
            (1920, "cc64 0"),
            (1920, "cc64 127"),
            (1920, "on 62")
        ])
    );
}

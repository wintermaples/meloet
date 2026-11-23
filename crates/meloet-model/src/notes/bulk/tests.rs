use super::*;
use crate::edit::{NewTake, add_recording};
use crate::time::TICKS_PER_QUARTER;

const T1: TrackId = TrackId(1);
const Q: u64 = TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

fn note(start: u64, length: u64, key: u8, velocity: u8) -> Note {
    Note {
        from: None,
        start,
        length,
        key,
        velocity,
    }
}

/// One clip whose take starts at `BAR`, holding `notes` (from the take's tick 0).
fn one_clip(mut notes: Vec<Note>) -> (Doc, ClipId) {
    notes.sort_by_key(|n| (n.start, n.key));
    let (edit, added) = add_recording(
        &Doc::new_default(),
        T1,
        vec![NewTake {
            origin: BAR,
            length: 4 * BAR,
            notes,
            controls: Vec::new(),
        }],
        0,
    )
    .unwrap();
    (edit.doc, added.clip)
}

fn velocities(doc: &Doc, clip: ClipId) -> Vec<u8> {
    song_notes(doc.clip(clip).unwrap().1)
        .iter()
        .map(|n| n.velocity)
        .collect()
}

fn lengths(doc: &Doc, clip: ClipId) -> Vec<u64> {
    song_notes(doc.clip(clip).unwrap().1)
        .iter()
        .map(|n| n.length)
        .collect()
}

/// Notes at Q apart with these velocities.
fn with_velocities(vs: &[u8]) -> (Doc, ClipId) {
    one_clip(
        vs.iter()
            .enumerate()
            .map(|(i, &v)| note(i as u64 * Q, 10, 60, v))
            .collect(),
    )
}

#[test]
fn scaling_rounds_and_stops_at_1_and_127() {
    let table: [(u16, [u8; 5]); 4] = [
        (120, [1, 12, 77, 120, 127]),
        (50, [1, 5, 32, 50, 64]),
        (10, [1, 1, 6, 10, 13]),
        (300, [3, 30, 127, 127, 127]),
    ];
    let (doc, clip) = with_velocities(&[1, 10, 64, 100, 127]);
    for (percent, want) in table {
        let e = scale_velocity(&doc, clip, &[0, 1, 2, 3, 4], percent).unwrap();
        assert_eq!(velocities(&e.edit.doc, clip), want, "{percent} %");
        assert_eq!(e.selected, [0, 1, 2, 3, 4]);
        assert_eq!(e.edit.step, StepKind::ScaleVelocity { clip, count: 5 });
    }
    // Out of 10..=300: put at the end.
    let e = scale_velocity(&doc, clip, &[2], 1000).unwrap();
    assert_eq!(velocities(&e.edit.doc, clip)[2], 127);
    let e = scale_velocity(&doc, clip, &[3], 0).unwrap();
    assert_eq!(velocities(&e.edit.doc, clip)[3], 10);
    // Only the picked notes.
    let e = scale_velocity(&doc, clip, &[3], 50).unwrap();
    assert_eq!(velocities(&e.edit.doc, clip), [1, 10, 64, 50, 127]);
    assert_eq!(
        scale_velocity(&doc, clip, &[0, 1, 2], 100).unwrap_err(),
        EditError::Unchanged
    );
    // 127 × 120 % and 1 × 50 % stay where they are.
    assert_eq!(
        scale_velocity(&doc, clip, &[4], 120).unwrap_err(),
        EditError::Unchanged
    );
    assert_eq!(
        scale_velocity(&doc, clip, &[0], 50).unwrap_err(),
        EditError::Unchanged
    );
}

#[test]
fn a_ramp_goes_by_start_and_chords_share_a_value() {
    let notes = vec![
        note(0, 10, 60, 100),
        note(Q, 10, 60, 100),
        note(2 * Q, 10, 60, 100),
        note(2 * Q, 10, 64, 100),
        note(3 * Q, 10, 60, 100),
    ];
    let (doc, clip) = one_clip(notes);
    let all = [0, 1, 2, 3, 4];
    let e = ramp_velocity(&doc, clip, &all, 40, 110).unwrap();
    assert_eq!(velocities(&e.edit.doc, clip), [40, 63, 87, 87, 110]);
    assert_eq!(e.selected, all);
    assert_eq!(e.edit.step, StepKind::RampVelocity { clip, count: 5 });
    let e = ramp_velocity(&doc, clip, &all, 110, 40).unwrap();
    assert_eq!(velocities(&e.edit.doc, clip), [110, 87, 63, 63, 40]);
    // The ends are the picked notes': the others stay.
    let e = ramp_velocity(&doc, clip, &[1, 4], 1, 127).unwrap();
    assert_eq!(velocities(&e.edit.doc, clip), [100, 1, 100, 100, 127]);
    // All at one start: `from`.
    let e = ramp_velocity(&doc, clip, &[2, 3], 20, 120).unwrap();
    assert_eq!(velocities(&e.edit.doc, clip), [100, 100, 20, 20, 100]);
    // Out of 1..=127: put at the ends.
    let e = ramp_velocity(&doc, clip, &[0, 4], 0, 255).unwrap();
    assert_eq!(velocities(&e.edit.doc, clip), [1, 100, 100, 100, 127]);
    assert_eq!(
        ramp_velocity(&doc, clip, &all, 100, 100).unwrap_err(),
        EditError::Unchanged
    );
}

#[test]
fn a_random_spread_comes_from_the_seed_and_the_index() {
    // The numbers stay the same from version to version.
    let deltas: Vec<i64> = (0..6).map(|i| random_delta(1, i, 8)).collect();
    assert_eq!(deltas, PINNED);
    for seed in [0, 1, 77, u32::MAX] {
        for i in 0..200 {
            for amount in [1, 8, 32] {
                assert!(random_delta(seed, i, amount).abs() <= i64::from(amount));
            }
        }
    }
    let spread: std::collections::BTreeSet<i64> = (0..500).map(|i| random_delta(9, i, 2)).collect();
    assert_eq!(spread.into_iter().collect::<Vec<_>>(), [-2, -1, 0, 1, 2]);

    let (doc, clip) = with_velocities(&[64, 64, 64, 64, 64, 64]);
    let all = [0, 1, 2, 3, 4, 5];
    let e = random_velocity(&doc, clip, &all, 8, 1).unwrap();
    let want: Vec<u8> = PINNED.iter().map(|d| (64 + d) as u8).collect();
    assert_eq!(velocities(&e.edit.doc, clip), want);
    assert_eq!(e.selected, all);
    assert_eq!(e.edit.step, StepKind::RandomVelocity { clip, count: 6 });
    // The same command, the same result.
    let again = random_velocity(&doc, clip, &all, 8, 1).unwrap();
    assert_eq!(velocities(&again.edit.doc, clip), want);
    // A note keeps its number when fewer are picked.
    let e = random_velocity(&doc, clip, &[3], 8, 1).unwrap();
    assert_eq!(velocities(&e.edit.doc, clip)[3], want[3]);
    // Stops at 1 and 127.
    let (doc, clip) = with_velocities(&[1, 1, 1, 127, 127, 127]);
    let e = random_velocity(&doc, clip, &all, 32, 5).unwrap();
    assert!(
        velocities(&e.edit.doc, clip)
            .iter()
            .all(|v| (1..=127).contains(v))
    );
}

const PINNED: [i64; 6] = [1, 1, -4, -5, -5, 7];

#[test]
fn splitmix_is_the_usual_one() {
    // The first number of splitmix64 from the state 0.
    assert_eq!(splitmix64(0), 0xE220_A839_7B1D_CDAF);
}

#[test]
fn legato_reaches_the_next_picked_start() {
    let notes = vec![
        note(0, 10, 60, 100),
        note(0, 3 * Q, 64, 100),
        note(Q, 50, 62, 100),
        note(2 * Q, 10, 70, 100),
        note(3 * Q, 5, 65, 100),
    ];
    let (doc, clip) = one_clip(notes);
    // Index 3 is not picked: it is not a "next" start, and stays.
    let e = legato(&doc, clip, &[0, 1, 2, 4]).unwrap();
    assert_eq!(lengths(&e.edit.doc, clip), [Q, Q, 2 * Q, 10, 5]);
    assert_eq!(e.selected, [0, 1, 2, 4]);
    assert_eq!(e.edit.step, StepKind::Legato { clip, count: 4 });
    // All picked.
    let e = legato(&doc, clip, &[0, 1, 2, 3, 4]).unwrap();
    assert_eq!(lengths(&e.edit.doc, clip), [Q, Q, Q, Q, 5]);
    // One start only (a chord), or one note: nothing to reach.
    assert_eq!(
        legato(&doc, clip, &[0, 1]).unwrap_err(),
        EditError::Unchanged
    );
    assert_eq!(legato(&doc, clip, &[4]).unwrap_err(), EditError::Unchanged);
    // Already there.
    let e = legato(&doc, clip, &[2, 4]).unwrap();
    assert_eq!(
        legato(&e.edit.doc, clip, &[2, 4]).unwrap_err(),
        EditError::Unchanged
    );
}

#[test]
fn notes_one_tick_apart_get_one_tick() {
    let (doc, clip) = one_clip(vec![note(0, 100, 60, 100), note(1, 100, 62, 100)]);
    let e = legato(&doc, clip, &[0, 1]).unwrap();
    assert_eq!(lengths(&e.edit.doc, clip), [1, 100]);
}

#[test]
fn bad_picks() {
    let (doc, clip) = with_velocities(&[64]);
    assert_eq!(
        scale_velocity(&doc, clip, &[], 50).unwrap_err(),
        EditError::Unchanged
    );
    assert_eq!(
        legato(&doc, clip, &[3]).unwrap_err(),
        EditError::NoSuchNote(3)
    );
    assert_eq!(
        ramp_velocity(&doc, ClipId(999), &[0], 1, 2).unwrap_err(),
        EditError::NoSuchClip(ClipId(999))
    );
}

use meloet_protocol::{ClipView, Command, NoteAt, TakeNotes};

use crate::play::tests::{cmd, ready};
use crate::state::Model;

const Q: u64 = meloet_model::TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

fn clips(m: &Model) -> Vec<ClipView> {
    m.view().song.tracks[0].clips.clone()
}

fn steps(m: &Model) -> u32 {
    m.view().song.undo_count
}

fn keys_of(m: &Model, clip: &ClipView) -> Vec<(u64, u8, u64)> {
    let notes = TakeNotes::decode(&m.take_notes(clip.take_id).unwrap()).unwrap();
    let zero = (clip.start as i64 + clip.take_offset) as u64;
    notes
        .notes
        .iter()
        .map(|n| (zero + u64::from(n.start), n.key, u64::from(n.length)))
        .collect()
}

/// A C major triad (C3 E3 G3) on each of `bars` bars from `from`, a bar long.
fn chords(from: u64, bars: u64) -> Vec<NoteAt> {
    (from..from + bars)
        .flat_map(|b| {
            [48, 52, 55].map(|key| NoteAt {
                start: b * BAR,
                length: BAR,
                key,
                velocity: 100,
            })
        })
        .collect()
}

/// A clip of bars 1–4 shown in the piano roll, with one note at its start.
fn one_clip() -> (Model, ClipView) {
    let mut m = ready();
    cmd(&mut m, Command::CreateClip { track: 1, tick: 0 });
    let take = m.view().piano_roll.take;
    cmd(
        &mut m,
        Command::AddNote {
            take,
            start: 0,
            length: Q,
            key: 72,
            velocity: 90,
        },
    );
    let clip = clips(&m)[0].clone();
    (m, clip)
}

#[test]
fn the_notes_are_one_step_the_clip_grows_and_notes_there_stay() {
    let (mut m, clip) = one_clip();
    let m = &mut m;
    assert_eq!((clip.start, clip.end), (0, 4 * BAR));
    let n = steps(m);
    cmd(
        m,
        Command::AddNotesAt {
            take: Some(clip.take_id),
            notes: chords(0, 6),
        },
    );
    assert_eq!(steps(m), n + 1);
    let now = clips(m)[0].clone();
    assert_eq!((now.id, now.start, now.end), (clip.id, 0, 6 * BAR));
    let notes = keys_of(m, &now);
    assert_eq!(notes.len(), 19);
    assert!(notes.contains(&(0, 72, Q)), "the note there stays");
    assert!(notes.contains(&(5 * BAR, 55, BAR)));
    // The notes written are selected.
    let sel: u32 = (m.view().piano_roll.selected_notes.iter())
        .map(|r| r.end - r.start)
        .sum();
    assert_eq!(sel, 18);
    cmd(m, Command::Undo);
    let back = clips(m)[0].clone();
    assert_eq!((back.end, back.take_id), (4 * BAR, clip.take_id));
}

#[test]
fn nothing_is_put_from_where_the_next_clip_is_hit() {
    let (mut m, clip) = one_clip();
    let m = &mut m;
    cmd(
        m,
        Command::CreateClip {
            track: 1,
            tick: 6 * BAR + Q,
        },
    );
    assert_eq!(clips(m)[1].start, 6 * BAR);
    cmd(
        m,
        Command::SelectClip {
            clip: Some(clip.id),
        },
    );
    let n = steps(m);
    cmd(
        m,
        Command::AddNotesAt {
            take: Some(clip.take_id),
            notes: chords(0, 8),
        },
    );
    assert_eq!(steps(m), n + 1);
    let now = clips(m)[0].clone();
    assert_eq!(now.end, 6 * BAR);
    let notes = keys_of(m, &now);
    assert_eq!(notes.len(), 1 + 6 * 3);
    assert!(notes.iter().all(|&(s, _, _)| s < 6 * BAR));
}

#[test]
fn an_old_take_or_nothing_left_does_nothing() {
    let (mut m, clip) = one_clip();
    let m = &mut m;
    let n = steps(m);
    cmd(
        m,
        Command::AddNotesAt {
            take: Some(clip.take_id + 1000),
            notes: chords(0, 2),
        },
    );
    cmd(
        m,
        Command::AddNotesAt {
            take: Some(clip.take_id),
            notes: vec![NoteAt {
                start: 0,
                length: BAR,
                key: 200,
                velocity: 100,
            }],
        },
    );
    cmd(
        m,
        Command::AddNotesAt {
            take: Some(clip.take_id),
            notes: Vec::new(),
        },
    );
    assert_eq!(steps(m), n);
    // A note given twice is written once; velocity 0 is 1.
    let mut twice = chords(1, 1);
    twice.extend(chords(1, 1));
    twice[0].velocity = 0;
    cmd(
        m,
        Command::AddNotesAt {
            take: Some(clip.take_id),
            notes: twice,
        },
    );
    assert_eq!(steps(m), n + 1);
    let notes = TakeNotes::decode(&m.take_notes(clips(m)[0].take_id).unwrap()).unwrap();
    assert_eq!(notes.notes.len(), 4);
}

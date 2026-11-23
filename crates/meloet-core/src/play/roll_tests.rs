//! The piano roll: editing notes, the selection, the clipboard, the settings, the audition.

use std::collections::{HashMap, HashSet};

use meloet_model::ControlPoint;
use meloet_model::ControlValue;
use meloet_model::edit::{NewTake, add_recording};
use meloet_model::song::Track;
use meloet_model::{InstrumentRef, Take, TrackColor};
use meloet_protocol::{
    ClipSpot, Command, ControlKind, IndexRange, NoteField, SelectMode, StatusMessage, TakeNotes,
};

use super::tests::{cmd, ready, record_once, report, started, status};
use super::*;
use crate::songfile;
use crate::state::Effect;

const Q: u64 = meloet_model::TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }

    fn signed(&mut self, n: i64) -> i64 {
        self.below(2 * n as u64 + 1) as i64 - n
    }
}

fn notes_from(rng: &mut Rng, count: u64, length: u64) -> Vec<Note> {
    let mut notes: Vec<Note> = (0..count)
        .map(|_| Note {
            from: None,
            start: rng.below(length - 10),
            length: 1 + rng.below(400),
            key: 30 + rng.below(60) as u8,
            velocity: 1 + rng.below(127) as u8,
        })
        .map(|n| Note {
            length: n.length.min(length - n.start),
            ..n
        })
        .collect();
    notes.sort_by_key(|n| n.start);
    notes
}

fn controls_from(rng: &mut Rng, count: u64, length: u64) -> Vec<ControlPoint> {
    let mut points: Vec<ControlPoint> = (0..count)
        .map(|_| ControlPoint {
            tick: rng.below(length),
            value: match rng.below(3) {
                0 => ControlValue::Cc {
                    number: 64,
                    value: [0, 127][rng.below(2) as usize],
                },
                1 => ControlValue::Cc {
                    number: 1,
                    value: rng.below(128) as u8,
                },
                _ => ControlValue::PitchBend(rng.signed(8192) as i16),
            },
        })
        .collect();
    points.sort_by_key(|c| c.tick);
    points
}

/// Two tracks. Track 1: a clip of 3 takes over bars 2–6 (the takes at different places in it),
/// and a clip of 1 take over bars 8–9. Track 2: one clip.
fn song(rng: &mut Rng) -> Song {
    let mut s = Song::new_default();
    s.doc.tracks.push(Arc::new(Track {
        kind: meloet_model::TrackKind::Keys,
        id: TrackId(2),
        name: "ベース".into(),
        color: TrackColor::for_index(1),
        instrument: InstrumentRef::SINE,
        channel: Default::default(),
        sends: [meloet_model::Gain::SILENT; 2],
        muted: false,
        clips: Vec::new(),
        automation: Vec::new(),
    }));
    let take = |rng: &mut Rng, origin: u64, length: u64| NewTake {
        origin,
        length,
        notes: notes_from(rng, 6, length),
        controls: controls_from(rng, 4, length),
    };
    let add = |doc: &Doc, track: u32, takes: Vec<NewTake>| {
        add_recording(doc, TrackId(track), takes, 0).unwrap().0.doc
    };
    let doc = add(&s.doc, 1, vec![take(rng, BAR, 4 * BAR)]);
    let doc = add(&doc, 1, vec![take(rng, 2 * BAR, BAR)]);
    let doc = add(&doc, 1, vec![take(rng, BAR + Q, 2 * BAR)]);
    let doc = add(&doc, 1, vec![take(rng, 7 * BAR, BAR)]);
    let doc = add(&doc, 2, vec![take(rng, 0, 2 * BAR)]);
    s.doc = doc;
    s
}

/// The doc with `next_id` left out: undo and redo bring the rest back, never a lower `next_id`.
fn held(doc: &Doc) -> Doc {
    Doc {
        next_id: 0,
        ..doc.clone()
    }
}

/// Every id given out so far: a take id always names the same take, a clip new in a step has an
/// id never given out, and `next_id` never goes back.
#[derive(Default)]
struct Given {
    takes: HashMap<TakeId, Arc<Take>>,
    clips: HashSet<ClipId>,
    next_id: u32,
}

impl Given {
    /// `before`: the doc a new step was made on. Without it (undo, redo) clips may come back.
    fn saw(&mut self, doc: &Doc, before: Option<&Doc>, ctx: &str) {
        assert!(doc.next_id >= self.next_id, "{ctx}: next_id went back");
        self.next_id = doc.next_id;
        let takes = doc
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .flat_map(|c| c.keys())
            .flat_map(|ct| {
                std::iter::once(&ct.take).chain(ct.take.origin.as_ref().map(|o| &o.take))
            });
        for t in takes {
            assert!(t.id.0 < doc.next_id, "{ctx}: take {}", t.id.0);
            assert!(
                !self.clips.contains(&ClipId(t.id.0)),
                "{ctx}: take {}",
                t.id.0
            );
            let had = self.takes.entry(t.id).or_insert_with(|| t.clone());
            assert_eq!(had, t, "{ctx}: take id {} given twice", t.id.0);
        }
        for c in doc.tracks.iter().flat_map(|t| &t.clips) {
            let new = before.is_some_and(|b| b.clip(c.id).is_none());
            assert!(c.id.0 < doc.next_id, "{ctx}: clip {}", c.id.0);
            assert!(
                !self.takes.contains_key(&TakeId(c.id.0)),
                "{ctx}: clip {}",
                c.id.0
            );
            assert!(
                !(new && self.clips.contains(&c.id)),
                "{ctx}: clip id {} given twice",
                c.id.0
            );
            self.clips.insert(c.id);
        }
    }
}

fn fresh(song: Song) -> Play {
    let mut p = Play::new(false, 0);
    p.replace_song(song, false, &mut Vec::new());
    p
}

fn shown_take(p: &Play) -> Option<Arc<Take>> {
    p.shown().map(|c| c.active_take().take.clone())
}

fn shown_id(p: &Play) -> u32 {
    shown_take(p).unwrap().id.0
}

/// The rules every song keeps after any edit. Trims can leave notes and points of a take
/// outside its clip; only what is inside sounds.
fn check(p: &Play, takes_of: &HashMap<ClipId, usize>, ctx: &str) {
    for t in &p.song().doc.tracks {
        assert!(t.clips.is_sorted_by_key(|c| c.start), "{ctx}: clip order");
        for w in t.clips.windows(2) {
            assert!(w[0].end <= w[1].start, "{ctx}: clips overlap");
        }
        for e in track_events(&p.song().doc, t.id) {
            assert!(
                t.clips.iter().any(|c| c.start <= e.tick && e.tick <= c.end),
                "{ctx}: an event at {} outside the clips",
                e.tick
            );
        }
        for c in &t.clips {
            assert_eq!(
                Some(&c.takes.len()),
                takes_of.get(&c.id),
                "{ctx}: takes of {:?}",
                c.id
            );
            let shown = &c.active_take().take;
            assert_eq!(
                c.note_count(),
                shown.notes.iter().filter(|n| c.holds(n.start)).count(),
                "{ctx}: note count"
            );
            for ct in c.keys() {
                let take = &ct.take;
                assert!(
                    take.notes.iter().is_sorted_by_key(|n| (n.start, n.key)),
                    "{ctx}: order in {:?}",
                    take.id
                );
                match &take.origin {
                    Some(o) => {
                        assert!(o.take.origin.is_none(), "{ctx}: an origin of an origin");
                        assert!(
                            take.notes
                                .iter()
                                .filter_map(|n| n.from)
                                .all(|f| (f.get() as usize) < o.take.notes.len()),
                            "{ctx}: from inside the origin of {:?}",
                            take.id
                        );
                    }
                    None => assert!(
                        take.notes.iter().all(|n| n.from.is_none()),
                        "{ctx}: from with no origin"
                    ),
                }
                assert!(take.controls.iter().is_sorted_by_key(|c| c.tick), "{ctx}");
                for c2 in &take.controls {
                    assert!(c2.tick < take.length, "{ctx}: point inside the take");
                    if let ControlValue::Cc { number: 64, value } = c2.value {
                        assert!(value == 0 || value == 127, "{ctx}: sustain {value}");
                    }
                }
                for n in &take.notes {
                    assert!(n.length >= 1 && n.key <= 127, "{ctx}");
                    assert!((1..=127).contains(&n.velocity), "{ctx}");
                    assert!(n.end() <= take.length, "{ctx}: inside the take");
                }
            }
        }
    }
    let view = p.piano_roll_view();
    let len = shown_take(p).map_or(0, |t| t.notes.len() as u32);
    assert!(
        view.selected_notes
            .iter()
            .all(|r| r.start < r.end && r.end <= len),
        "{ctx}: selection {:?} of {len}",
        view.selected_notes
    );
    let points = shown_take(p).map_or(0, |t| t.controls.len() as u32);
    assert!(
        view.selected_controls
            .iter()
            .all(|r| r.start < r.end && r.end <= points),
        "{ctx}: selected points {:?} of {points}",
        view.selected_controls
    );
    if !p.note_sel.notes.is_empty() || !p.note_sel.controls.is_empty() {
        assert_eq!(p.note_sel.take.map(|t| t.0), view.take, "{ctx}");
    }
    // What the screen gets: the shown take, and its origin by id.
    if let Some(t) = shown_take(p) {
        let got = TakeNotes::decode(&p.take_notes(t.id.0).unwrap()).unwrap();
        assert_eq!(got.origin, t.origin.as_ref().map(|o| o.take.id.0), "{ctx}");
        assert!(
            got.notes
                .iter()
                .zip(&t.notes)
                .all(|(a, b)| a.from == b.from.map(OriginIndex::get)),
            "{ctx}"
        );
        if let Some(o) = got.origin {
            let origin = TakeNotes::decode(&p.take_notes(o).unwrap()).unwrap();
            assert!(
                got.notes
                    .iter()
                    .filter_map(|n| n.from)
                    .all(|f| (f as usize) < origin.notes.len())
            );
        }
    }
}

fn some_points(rng: &mut Rng, p: &Play) -> Vec<u32> {
    let len = shown_take(p).map_or(0, |t| t.controls.len()) as u64;
    if len == 0 || rng.below(4) == 0 {
        return match rng.below(2) {
            0 => p.note_sel.controls.clone(),
            _ => vec![len as u32 + rng.below(3) as u32],
        };
    }
    (0..1 + rng.below(3))
        .map(|_| rng.below(len) as u32)
        .collect()
}

fn some_notes(rng: &mut Rng, p: &Play) -> Vec<u32> {
    let len = shown_take(p).map_or(0, |t| t.notes.len()) as u64;
    if len == 0 || rng.below(4) == 0 {
        // Sometimes the selection, sometimes indexes past the end.
        return match rng.below(2) {
            0 => p.note_sel.notes.clone(),
            _ => vec![len as u32 + rng.below(3) as u32],
        };
    }
    (0..1 + rng.below(4))
        .map(|_| rng.below(len) as u32)
        .collect()
}

#[test]
fn random_edits_undo_and_redo_come_back_to_the_same_songs() {
    let mut counts = [0usize; 27];
    let mut made = [0usize; 27];
    let mut applied = 0usize;
    let mut saved_negative = 0usize;
    for seed in 1..=12u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let first = song(&mut rng);
        let mut p = fresh(first.clone());
        let mut takes_of: HashMap<ClipId, usize> = first
            .doc
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .map(|c| (c.id, c.takes.len()))
            .collect();
        let clips = |p: &Play| -> Vec<ClipId> {
            p.song()
                .doc
                .tracks
                .iter()
                .flat_map(|t| &t.clips)
                .map(|c| c.id)
                .collect()
        };
        p.select_clip(Some(clips(&p)[0].0));
        let mut given = Given::default();
        given.saw(&p.song().doc, None, "first");
        let mut line = vec![p.song().doc.clone()];
        let mut at = 0usize;
        let e = &mut Vec::new();
        for round in 0..400 {
            let ctx = format!("seed {seed}, round {round}");
            if clips(&p).is_empty() {
                // Cut or deleted every clip: back one step.
                p.undo(e);
                at = p.undo_count() as usize;
                assert_eq!(held(&p.song().doc), held(&line[at]), "{ctx}");
                given.saw(&p.song().doc, None, &ctx);
            }
            if p.shown().is_none() {
                // Undo took away the copy the piano roll showed.
                let all = clips(&p);
                p.select_clip(Some(all[rng.below(all.len() as u64) as usize].0));
            }
            let all = clips(&p);
            let some_clip = all[rng.below(all.len() as u64) as usize];
            // Sometimes the selection, else a few clips, now and then one that is not there.
            let group: Vec<u32> = match rng.below(4) {
                0 => p.clips_sel.iter().map(|c| c.0).collect(),
                _ => (0..1 + rng.below(3))
                    .map(|_| match rng.below(15) {
                        0 => 99_999,
                        _ => all[rng.below(all.len() as u64) as usize].0,
                    })
                    .collect(),
            };
            let steps = p.undo_count();
            let before = p.song().doc.clone();
            let take = if rng.below(20) == 0 {
                // An old id: dropped.
                shown_id(&p) + 1000
            } else {
                shown_id(&p)
            };
            let action = rng.below(29);
            counts[action.min(26) as usize] += 1;
            match action {
                0 => {
                    let c = p.shown().unwrap().clone();
                    let start = c.start + rng.below(c.end - c.start + 200);
                    let note = Note {
                        from: None,
                        start,
                        length: rng.below(2000),
                        key: rng.below(128) as u8,
                        velocity: rng.below(128) as u8,
                    };
                    p.add_note(Some(take), note, e);
                }
                1 => {
                    let picked = some_notes(&mut rng, &p);
                    let (ticks, keys) = (rng.signed(3 * BAR as i64), rng.signed(20) as i32);
                    p.move_notes(take, &picked, ticks, keys, false, e);
                }
                2 => {
                    let picked = some_notes(&mut rng, &p);
                    let (ticks, keys) = (rng.signed(BAR as i64), rng.signed(13) as i32);
                    p.move_notes(take, &picked, ticks, keys, true, e);
                }
                3 => {
                    let picked = some_notes(&mut rng, &p);
                    let edge = [ClipEdge::Start, ClipEdge::End][rng.below(2) as usize];
                    p.resize_notes(take, &picked, edge, rng.signed(1000), e);
                }
                4 => {
                    let picked = some_notes(&mut rng, &p);
                    p.delete_notes(take, &picked, e);
                }
                5 => {
                    let picked = some_notes(&mut rng, &p);
                    p.change_velocity(take, &picked, rng.signed(150) as i32, e);
                }
                6 => {
                    if rng.below(2) == 0 {
                        let picked = some_notes(&mut rng, &p);
                        p.copy_notes(take, &picked);
                    }
                    let c = p.shown().unwrap().clone();
                    p.paste_notes(c.start + rng.below(c.end - c.start), e);
                }
                7 => {
                    let picked = some_notes(&mut rng, &p);
                    p.cut_notes(take, &picked, e);
                }
                8 => {
                    p.step_take(rng.signed(2) as i32, e);
                }
                9 => drop(p.undo(e)),
                10 => drop(p.redo(e)),
                11 => {
                    if rng.below(3) == 0 {
                        let division = [0, 2, 4, 8, 16, 32][rng.below(6) as usize];
                        p.set_quantize(meloet_protocol::QuantizeSettings {
                            division,
                            triplet: rng.below(3) == 0,
                            strength: rng.below(101) as u8,
                            swing: rng.below(101) as u8,
                            align: [QuantizeAlign::Start, QuantizeAlign::StartAndEnd]
                                [rng.below(2) as usize],
                        });
                    }
                    let picked = if rng.below(3) == 0 {
                        Vec::new()
                    } else {
                        some_notes(&mut rng, &p)
                    };
                    p.quantize(take, &picked, e);
                }
                12 => {
                    let picked = if rng.below(2) == 0 {
                        Vec::new()
                    } else {
                        some_notes(&mut rng, &p)
                    };
                    p.unquantize(take, &picked, e);
                }
                13 => {
                    let picked = some_notes(&mut rng, &p);
                    let field = [
                        NoteField::Start,
                        NoteField::Key,
                        NoteField::Length,
                        NoteField::Velocity,
                    ][rng.below(4) as usize];
                    let value = match field {
                        NoteField::Start => rng.below(8 * BAR) as i64,
                        _ => rng.signed(300),
                    };
                    p.set_notes_field(take, &picked, field, value, e);
                }
                14 => {
                    let c = p.shown().unwrap().clone();
                    let tick = (c.start + rng.below(c.end - c.start + 200)).saturating_sub(100);
                    let control = match rng.below(4) {
                        0 => ControlKind::PitchBend,
                        1 => ControlKind::Cc { number: 64 },
                        2 => ControlKind::Cc { number: 1 },
                        _ => ControlKind::Cc {
                            number: rng.below(140) as u8,
                        },
                    };
                    p.add_control(take, tick, control, rng.signed(9000) as i32, e);
                }
                15 => {
                    let picked = some_points(&mut rng, &p);
                    let (ticks, value) = (rng.signed(2 * BAR as i64), rng.signed(200) as i32);
                    p.move_controls(take, &picked, ticks, value, e);
                }
                16 => {
                    let picked = some_points(&mut rng, &p);
                    p.delete_controls(take, &picked, e);
                }
                17 => {
                    let mode = [SelectMode::Replace, SelectMode::Add, SelectMode::Toggle]
                        [rng.below(3) as usize];
                    let picked = some_points(&mut rng, &p);
                    p.select_controls(take, picked, mode);
                }
                18 | 19 => {
                    let ticks = match rng.below(3) {
                        0 => rng.signed(8) * BAR as i64,
                        _ => rng.signed(6 * BAR as i64),
                    };
                    let rows = [0, 1, 0, -1, 0, 5][rng.below(6) as usize];
                    p.move_clips(&group, ticks, rows, action == 19, e);
                }
                20 => {
                    p.duplicate_clips(&group, e);
                }
                21 => {
                    let edge = [ClipEdge::Start, ClipEdge::End][rng.below(2) as usize];
                    p.trim_clip(some_clip.0, edge, rng.signed(2 * BAR as i64), e);
                }
                22 => {
                    if rng.below(2) == 0 {
                        p.copy_clips(&group);
                    }
                    let to = match rng.below(3) {
                        0 => None,
                        _ => Some(ClipSpot {
                            track: [1, 2, 1, 7][rng.below(4) as usize],
                            tick: rng.below(12 * BAR),
                        }),
                    };
                    p.paste_clips(to, e);
                }
                23 => p.cut_clips(&group, e),
                24 => p.delete_selected(e),
                25 => match rng.below(4) {
                    0 => p.select_all_clips(),
                    1 => {
                        p.select_track(rng.below(3) as u32 + 1, false);
                    }
                    _ => {
                        let mode = [SelectMode::Replace, SelectMode::Add, SelectMode::Toggle]
                            [rng.below(3) as usize];
                        p.select_clips(group.clone(), mode);
                    }
                },
                _ => match rng.below(5) {
                    0 => p.select_clip(Some(some_clip.0)),
                    1 => p.select_all_notes(),
                    2 => p.clear_note_selection(),
                    _ => {
                        let mode = [SelectMode::Replace, SelectMode::Add, SelectMode::Toggle]
                            [rng.below(3) as usize];
                        let picked = some_notes(&mut rng, &p);
                        p.select_notes(take, picked, mode);
                    }
                },
            }
            let now = p.undo_count();
            match action {
                9 => assert!(now + 1 == steps || (now == steps && at == 0), "{ctx}"),
                10 => assert!(now == steps + 1 || now == steps, "{ctx}"),
                _ if now == steps + 1 => {
                    applied += 1;
                    made[action.min(26) as usize] += 1;
                    // The clips placed are selected: moved ones keep their ids, copies are new.
                    if matches!(action, 18..=20 | 22) {
                        assert!(!p.clips_sel.is_empty(), "{ctx}");
                        let given = |c: &ClipId| group.contains(&c.0);
                        if action == 18 {
                            assert!(p.clips_sel.iter().all(given), "{ctx}");
                        } else {
                            assert!(!p.clips_sel.iter().any(given), "{ctx}");
                        }
                        for &c in &p.clips_sel {
                            let takes = p.song().doc.clip(c).unwrap().1.takes.len();
                            takes_of.insert(c, takes);
                        }
                        if let Some(c) = p.selected {
                            assert_eq!(p.piano_roll, Some(c), "{ctx}");
                        }
                    }
                    if matches!(action, 23 | 24) {
                        assert!(
                            p.clips_sel.iter().all(|&c| p.song().doc.clip(c).is_some()),
                            "{ctx}"
                        );
                    }
                    line.truncate(at + 1);
                    line.push(p.song().doc.clone());
                    given.saw(&p.song().doc, Some(&before), &ctx);
                }
                _ => assert_eq!(now, steps, "{ctx}: one command, one step at most"),
            }
            at = now as usize;
            assert_eq!(held(&p.song().doc), held(&line[at]), "{ctx}");
            given.saw(&p.song().doc, None, &ctx);
            check(&p, &takes_of, &ctx);
            if round % 50 == 49 {
                let back = songfile::decode(&songfile::encode(p.song())).unwrap();
                assert_eq!(&back, p.song(), "{ctx}: saved and opened");
                saved_negative += p
                    .song()
                    .doc
                    .tracks
                    .iter()
                    .flat_map(|t| &t.clips)
                    .flat_map(|c| c.keys())
                    .filter(|ct| ct.offset < 0)
                    .count();
            }
        }
        while p.history.undo_count() > 0 {
            p.undo(e);
        }
        assert_eq!(
            held(&p.song().doc),
            held(&first.doc),
            "seed {seed}: all undone"
        );
        while p.history.redo_count() > 0 {
            p.redo(e);
        }
        assert_eq!(
            held(&p.song().doc),
            held(line.last().unwrap()),
            "seed {seed}: all redone"
        );
    }
    assert!(counts.iter().all(|&n| n > 100), "{counts:?}");
    assert!(applied > 1500, "{applied} steps");
    // Quantize, back, the fields, and the three edits of points all made steps.
    assert!(made[11..=16].iter().all(|&n| n > 30), "{made:?}");
    // Move, copy, duplicate, trim, paste, cut and delete too; some saved songs had takes trimmed
    // at the head.
    assert!(made[18..=24].iter().all(|&n| n > 20), "{made:?}");
    assert!(saved_negative > 10, "{saved_negative}");
}

fn two_notes() -> Play {
    let mut s = Song::new_default();
    let take = NewTake {
        origin: BAR,
        length: BAR,
        notes: vec![
            Note {
                from: None,
                start: 0,
                length: 100,
                key: 60,
                velocity: 90,
            },
            Note {
                from: None,
                start: Q,
                length: 100,
                key: 64,
                velocity: 90,
            },
        ],
        controls: Vec::new(),
    };
    s.doc = add_recording(&s.doc, TrackId(1), vec![take], 0)
        .unwrap()
        .0
        .doc;
    let mut p = fresh(s);
    let clip = p.song().doc.tracks[0].clips[0].id;
    p.select_clip(Some(clip.0));
    p
}

#[test]
fn the_edited_notes_are_selected_and_undo_brings_the_selection_back() {
    let mut p = two_notes();
    let e = &mut Vec::new();
    let t0 = shown_id(&p);
    p.select_notes(t0, vec![0], SelectMode::Replace);
    p.move_notes(t0, &[1], 2 * Q as i64, 0, false, e);
    let t1 = shown_id(&p);
    assert_ne!(t1, t0);
    assert_eq!(p.note_sel.notes, [1]);
    // Copies are selected, not the originals.
    p.move_notes(t1, &[0, 1], 0, 12, true, e);
    let t2 = shown_id(&p);
    assert_eq!(p.note_sel.notes, [1, 3]);
    p.delete_notes(t2, &[0], e);
    assert!(p.note_sel.notes.is_empty());

    p.undo(e);
    assert_eq!((shown_id(&p), &p.note_sel.notes[..]), (t2, &[1, 3][..]));
    p.undo(e);
    assert_eq!((shown_id(&p), &p.note_sel.notes[..]), (t1, &[1][..]));
    p.undo(e);
    assert_eq!((shown_id(&p), &p.note_sel.notes[..]), (t0, &[0][..]));
    p.redo(e);
    assert_eq!((shown_id(&p), &p.note_sel.notes[..]), (t1, &[1][..]));

    // A note command at the old take does nothing.
    p.move_notes(t0, &[0], Q as i64, 0, false, e);
    p.delete_notes(t2, &[0], e);
    assert_eq!(p.undo_count(), 1);
    assert_eq!(shown_id(&p), t1);
}

#[test]
fn select_modes_and_select_all() {
    let mut p = two_notes();
    let t = shown_id(&p);
    p.select_notes(t, vec![1, 7], SelectMode::Replace);
    assert_eq!(p.note_sel.notes, [1]);
    p.select_notes(t, vec![0], SelectMode::Add);
    assert_eq!(p.note_sel.notes, [0, 1]);
    p.select_notes(t, vec![0, 0], SelectMode::Toggle);
    assert_eq!(p.note_sel.notes, [1]);
    p.select_notes(t + 1, vec![0], SelectMode::Replace);
    assert_eq!(p.note_sel.notes, [1], "an old take is dropped");
    p.clear_note_selection();
    assert!(p.piano_roll_view().selected_notes.is_empty());
    p.select_all_notes();
    assert_eq!(
        p.piano_roll_view().selected_notes,
        [IndexRange { start: 0, end: 2 }]
    );
    // Another clip shown: the selection goes.
    p.select_clip(None);
    assert_eq!(
        p.note_sel.notes,
        [0, 1],
        "unselecting the clip keeps the piano roll"
    );
    p.replace_song(Song::new_default(), false, &mut Vec::new());
    assert!(p.piano_roll_view().selected_notes.is_empty());
}

#[test]
fn cut_copy_and_paste() {
    let mut p = two_notes();
    let e = &mut Vec::new();
    let t = shown_id(&p);
    p.copy_notes(t, &[1, 0]);
    assert_eq!(p.piano_roll_view().clipboard_notes, 2);
    assert_eq!(p.undo_count(), 0, "copying is not a step");
    assert_eq!(p.paste_notes(2 * BAR - 200, e), None);
    // The second ends past the clip: it grows by a bar, in the same step.
    assert_eq!(p.undo_count(), 1);
    assert_eq!(shown_take(&p).unwrap().notes.len(), 4);
    assert_eq!(p.note_sel.notes, [2, 3]);
    assert_eq!(
        (p.shown().unwrap().start, p.shown().unwrap().end),
        (BAR, 3 * BAR)
    );
    let t = shown_id(&p);
    p.cut_notes(t, &[0, 2], e);
    assert_eq!(p.undo_count(), 2, "cut is one step");
    assert_eq!(shown_take(&p).unwrap().notes.len(), 2);
    assert_eq!(p.piano_roll_view().clipboard_notes, 2);
    let kept = p.clipboard.clone();
    assert_eq!(kept[0].start, 0);
    assert_eq!(kept[1].start, BAR - 200);
    p.paste_notes(BAR, e);
    assert_eq!(p.note_sel.notes, [0, 2]);
    assert_eq!(shown_take(&p).unwrap().notes.len(), 4);
    assert_eq!(p.undo_count(), 3);
}

/// The owner's report: notes copied in one track's clip could not be pasted in another track's.
#[test]
fn notes_paste_into_a_clip_of_another_track() {
    let mut p = two_notes();
    let e = &mut Vec::new();
    let t = shown_id(&p);
    p.copy_notes(t, &[0, 1]);
    let mut s = p.song().clone();
    s.doc = add_recording(
        &s.doc,
        TrackId(1),
        vec![NewTake {
            origin: 4 * BAR,
            length: BAR,
            notes: Vec::new(),
            controls: Vec::new(),
        }],
        0,
    )
    .unwrap()
    .0
    .doc;
    let mut track = Track::clone(&s.doc.tracks[0]);
    track.id = TrackId(2);
    track.clips = vec![s.doc.tracks[0].clips[1].clone()];
    let mut one = Track::clone(&s.doc.tracks[0]);
    one.clips.truncate(1);
    s.doc.tracks = vec![Arc::new(one), Arc::new(track)];
    let other = s.doc.tracks[1].clips[0].id;
    let clipboard = p.clipboard.clone();
    let mut p = fresh(s);
    p.clipboard = clipboard;
    p.select_clip(Some(other.0));
    assert_eq!(p.shown().unwrap().id, other);
    // At the song's start, before the clip: the clip grows back to bar 1.
    assert_eq!(p.paste_notes(0, e), None);
    assert_eq!(p.undo_count(), 1, "pasting and growing are one step");
    let c = p.shown().unwrap().clone();
    assert_eq!((c.start, c.end), (0, 5 * BAR));
    assert_eq!(shown_take(&p).unwrap().notes.len(), 2);
    p.undo(e);
    let c = p.shown().unwrap().clone();
    assert_eq!((c.start, c.end), (4 * BAR, 5 * BAR));
    assert!(shown_take(&p).unwrap().notes.is_empty());
    // At the clip's head, as Mod V does when the play line is outside it.
    p.paste_notes(4 * BAR, e);
    let c = p.shown().unwrap().clone();
    assert_eq!((c.start, c.end), (4 * BAR, 5 * BAR));
    assert_eq!(shown_take(&p).unwrap().notes.len(), 2);
}

#[test]
fn pasting_says_so_when_the_clip_cannot_grow() {
    let mut p = two_notes();
    let e = &mut Vec::new();
    let t = shown_id(&p);
    p.copy_notes(t, &[0, 1]);
    let mut s = p.song().clone();
    s.doc = add_recording(
        &s.doc,
        TrackId(1),
        vec![NewTake {
            origin: 3 * BAR,
            length: BAR,
            notes: Vec::new(),
            controls: Vec::new(),
        }],
        0,
    )
    .unwrap()
    .0
    .doc;
    let clipboard = p.clipboard.clone();
    let mut p = fresh(s);
    p.clipboard = clipboard;
    p.select_clip(Some(p.song().doc.tracks[0].clips[0].id.0));
    let before = p.song().doc.clone();
    assert_eq!(
        p.paste_notes(3 * BAR - 10, e),
        Some(StatusMessage::ClipsNotPasted)
    );
    assert_eq!(p.song().doc, before);
    assert_eq!(p.undo_count(), 0);
}

#[test]
fn edits_send_the_track_to_the_engine_again() {
    let mut p = two_notes();
    let mut e = Vec::new();
    let t = shown_id(&p);
    p.change_velocity(t, &[0], 10, &mut e);
    assert!(matches!(
        &e[..],
        [Effect::Engine(EngineOp::Sequence { track: 0, .. })]
    ));
    e.clear();
    p.change_velocity(shown_id(&p), &[0], 1000, &mut e);
    p.change_velocity(shown_id(&p), &[0], 1, &mut e);
    assert_eq!(e.len(), 1, "a change that changes nothing is no step");
}

fn clip_view(m: &crate::state::Model) -> meloet_protocol::ClipView {
    m.view().song.tracks[0].clips[0].clone()
}

#[test]
fn the_screen_sees_new_takes_selections_and_the_summary() {
    let m = &mut ready();
    record_once(
        m,
        &[
            (60, 0.0, 100.0),
            (64, Q as f64, (Q + 120) as f64),
            (67, (2 * Q) as f64, (2 * Q + 40) as f64),
        ],
        (5 * Q / 2) as f64,
    );
    let c = clip_view(m);
    assert_eq!(m.view().song.note_count, 3);
    cmd(
        m,
        Command::SelectNotes {
            take: c.take_id,
            notes: vec![2, 0],
            mode: SelectMode::Replace,
        },
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::Summary {
            loop_bars: None,
            nothing_selected: false,
            notes: Some(NoteSelection {
                count: 2,
                keys: vec![60, 67],
                quantized: false,
                names: vec![
                    KeyCount { key: 60, count: 1 },
                    KeyCount { key: 67, count: 1 },
                ],
            }),
        })
    );
    cmd(m, Command::SelectClip { clip: None });
    assert!(matches!(
        status(m),
        Some(StatusMessage::Summary {
            nothing_selected: false,
            notes: Some(_),
            ..
        })
    ));
    cmd(
        m,
        Command::DeleteNotes {
            take: c.take_id,
            notes: vec![1],
        },
    );
    let after = clip_view(m);
    assert_ne!(after.take_id, c.take_id);
    assert_eq!((after.take, after.takes), (c.take, c.takes));
    assert_eq!(after.note_count, 2);
    assert_eq!(m.view().song.note_count, 2);
    assert_eq!(m.view().piano_roll.take, Some(after.take_id));
    assert!(m.view().piano_roll.selected_notes.is_empty());
    let take = TakeNotes::decode(&m.take_notes(after.take_id).unwrap()).unwrap();
    assert_eq!(take.notes.len(), 2);
    assert_eq!(take.origin, None);
    assert!(
        m.take_notes(c.take_id).is_none(),
        "the old take is in no clip now"
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::Summary {
            loop_bars: None,
            nothing_selected: true,
            notes: None,
        })
    );
    cmd(m, Command::Undo);
    assert_eq!(clip_view(m).take_id, c.take_id);
    assert_eq!(
        m.view().piano_roll.selected_notes,
        [
            IndexRange { start: 0, end: 1 },
            IndexRange { start: 2, end: 3 }
        ]
    );
}

#[test]
fn nothing_changes_notes_while_recording() {
    let m = &mut ready();
    record_once(m, &[(60, 0.0, 100.0)], 400.0);
    let c = clip_view(m);
    cmd(m, Command::Locate { tick: 4 * BAR });
    cmd(m, Command::Record);
    started(m);
    let before = m.view().song;
    let take = c.take_id;
    for command in [
        Command::AddNote {
            take: Some(take),
            start: 10,
            length: 10,
            key: 60,
            velocity: 100,
        },
        Command::MoveNotes {
            take,
            notes: vec![0],
            ticks: 10,
            keys: 0,
            copy: false,
        },
        Command::ResizeNotes {
            take,
            notes: vec![0],
            edge: ClipEdge::End,
            ticks: 10,
        },
        Command::ChangeVelocity {
            take,
            notes: vec![0],
            delta: 3,
        },
        Command::DeleteNotes {
            take,
            notes: vec![0],
        },
        Command::CutNotes {
            take,
            notes: vec![0],
        },
        Command::PasteNotes { at: 0 },
        Command::Quantize {
            take,
            notes: Vec::new(),
        },
        Command::Unquantize {
            take,
            notes: Vec::new(),
        },
        Command::SetNotesField {
            take,
            notes: vec![0],
            field: NoteField::Velocity,
            value: 3,
        },
        Command::AddControl {
            take,
            tick: 10,
            control: ControlKind::PitchBend,
            value: 30,
        },
        Command::MoveControls {
            take,
            points: vec![0],
            ticks: 10,
            value: 1,
        },
        Command::DeleteControls {
            take,
            points: vec![0],
        },
    ] {
        cmd(m, command);
    }
    assert_eq!(m.view().song, before);
    // Selecting and copying still work.
    cmd(
        m,
        Command::CopyNotes {
            take,
            notes: vec![0],
        },
    );
    assert_eq!(m.view().piano_roll.clipboard_notes, 1);
    cmd(m, Command::Record);
    report(
        m,
        RecordReport::Stopped {
            t_us: 100,
            tick: 4.0 * BAR as f64,
            lap: 0,
        },
    );
    // Playing: taken, and the engine hears the track again.
    cmd(m, Command::PlayStop);
    let effects = cmd(
        m,
        Command::MoveNotes {
            take,
            notes: vec![0],
            ticks: 10,
            keys: 0,
            copy: false,
        },
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Engine(EngineOp::Sequence { track: 0, .. })))
    );
}

#[test]
fn the_locked_scale_keeps_the_write_tool_on_its_keys() {
    let m = &mut ready();
    record_once(m, &[(60, 0.0, 100.0)], 1000.0);
    let c = clip_view(m);
    cmd(
        m,
        Command::SetMusicalScale {
            root: 0,
            mode: ScaleMode::Major,
            lock: true,
            highlight: true,
        },
    );
    // The audition sounds every key (B-62); writing is what the lock keeps on its keys.
    assert!(
        cmd(
            m,
            Command::Audition {
                keys: vec![63],
                velocity: 100
            }
        )
        .iter()
        .any(|e| matches!(e, Effect::Engine(EngineOp::Audition { .. })))
    );
    cmd(
        m,
        Command::AddNote {
            take: Some(c.take_id),
            start: 200,
            length: 100,
            key: 63,
            velocity: 100,
        },
    );
    let said = Some(StatusMessage::OutOfScale {
        key: 63,
        root: 0,
        mode: ScaleMode::Major,
    });
    assert_eq!(status(m), said);
    cmd(m, Command::AuditionOff);
    assert_eq!(status(m), said, "the audition does not put the notice away");
    assert_eq!(m.view().song.undo_count, 1);
    // In the scale, and moving off it with ↑ is fine.
    cmd(
        m,
        Command::AddNote {
            take: Some(c.take_id),
            start: 200,
            length: 100,
            key: 62,
            velocity: 100,
        },
    );
    assert_eq!(m.view().song.undo_count, 2);
    let take = clip_view(m).take_id;
    cmd(
        m,
        Command::MoveNotes {
            take,
            notes: vec![1],
            ticks: 0,
            keys: 1,
            copy: false,
        },
    );
    assert_eq!(m.view().song.undo_count, 3);
    // Minor: A minor has no G#.
    cmd(
        m,
        Command::SetMusicalScale {
            root: 9,
            mode: ScaleMode::Minor,
            lock: true,
            highlight: true,
        },
    );
    let take = clip_view(m).take_id;
    cmd(
        m,
        Command::AddNote {
            take: Some(take),
            start: 300,
            length: 100,
            key: 68,
            velocity: 100,
        },
    );
    assert_eq!(m.view().song.undo_count, 3);
    // Not locked: anything.
    cmd(
        m,
        Command::SetMusicalScale {
            root: 9,
            mode: ScaleMode::Minor,
            lock: false,
            highlight: true,
        },
    );
    cmd(
        m,
        Command::AddNote {
            take: Some(take),
            start: 300,
            length: 100,
            key: 68,
            velocity: 100,
        },
    );
    assert_eq!(m.view().song.undo_count, 4);
}

#[test]
fn the_locked_scale_keeps_the_write_tool_on_its_keys_for_every_kind() {
    let m = &mut ready();
    record_once(m, &[(60, 0.0, 100.0)], 1000.0);
    for mode in ScaleMode::ALL {
        for root in [0u8, 5, 9] {
            for highlight in [true, false] {
                let set = |m: &mut crate::state::Model, lock| {
                    let (steps, file) = (m.view().song.undo_count, m.view().song.file);
                    cmd(
                        m,
                        Command::SetMusicalScale {
                            root,
                            mode,
                            lock,
                            highlight,
                        },
                    );
                    let v = m.view();
                    assert_eq!(
                        (v.song.undo_count, v.song.file),
                        (steps, file),
                        "the scale is no step and no change"
                    );
                };
                set(m, true);
                let model = roll::MusicalScale::new(root, model_scale_mode(mode), true).unwrap();
                for key in 60u8..72 {
                    let before = m.view().song.undo_count;
                    cmd(
                        m,
                        Command::AddNote {
                            take: Some(clip_view(m).take_id),
                            start: 200,
                            length: 100,
                            key,
                            velocity: 100,
                        },
                    );
                    let inside = model.contains(key);
                    let step = (key + 12 - root) % 12;
                    assert_eq!(inside, model.mode.steps().contains(&step));
                    assert_eq!(
                        m.view().song.undo_count,
                        before + u32::from(inside),
                        "{mode:?} root {root} key {key}"
                    );
                }
                // Not locked: the keys outside are written too, whatever `highlight` is.
                set(m, false);
                let outside = (60u8..72).find(|&k| !model.contains(k)).unwrap();
                let before = m.view().song.undo_count;
                cmd(
                    m,
                    Command::AddNote {
                        take: Some(clip_view(m).take_id),
                        start: 300,
                        length: 100,
                        key: outside,
                        velocity: 100,
                    },
                );
                assert_eq!(m.view().song.undo_count, before + 1);
            }
        }
    }
}

/// Writes `ui/src/components/pianoroll/scaleSteps.table.json`, the steps of every kind of scale
/// by its protocol name; `ui/tests/pianoroll.geom.test.ts` holds `SCALE_STEPS` to it. A table
/// that is not the committed one shows up in `git status`.
#[test]
fn write_the_scale_steps_for_the_screen() {
    let rows: Vec<String> = roll::ScaleMode::ALL
        .iter()
        .map(|&mode| {
            format!(
                "  {}: {}",
                serde_json::to_string(&scale_mode(mode)).unwrap(),
                serde_json::to_string(mode.steps()).unwrap()
            )
        })
        .collect();
    let table = format!("{{\n{}\n}}\n", rows.join(",\n"));
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ui/src/components/pianoroll/scaleSteps.table.json");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(table.as_str()) {
        std::fs::write(&path, &table).unwrap();
    }
    let names: Vec<ScaleMode> = roll::ScaleMode::ALL.map(scale_mode).to_vec();
    assert_eq!(
        names,
        ScaleMode::ALL,
        "the same order as the protocol's list"
    );
}

#[test]
fn writing_with_no_clip_makes_one_in_one_step() {
    let m = &mut ready();
    // Two tracks would be nicer; the keys play track 1 in a new song.
    cmd(
        m,
        Command::AddNote {
            take: Some(5),
            start: BAR + 100,
            length: Q / 2,
            key: 60,
            velocity: 100,
        },
    );
    assert_eq!(m.view().song.undo_count, 0, "no such take");
    cmd(
        m,
        Command::AddNote {
            take: None,
            start: BAR + 100,
            length: Q / 2,
            key: 60,
            velocity: 0,
        },
    );
    let v = m.view();
    assert_eq!(v.song.undo_count, 1);
    let c = &v.song.tracks[0].clips[0];
    assert_eq!((c.start, c.end, c.name.as_str()), (BAR, 5 * BAR, "ピアノ"));
    assert_eq!(v.arrange.selected_clip, Some(c.id));
    assert_eq!(v.arrange.piano_roll_clip, Some(c.id));
    assert_eq!(v.piano_roll.take, Some(c.take_id));
    assert_eq!(
        v.piano_roll.selected_notes,
        [IndexRange { start: 0, end: 1 }]
    );
    let take = TakeNotes::decode(&m.take_notes(c.take_id).unwrap()).unwrap();
    assert_eq!(take.notes[0].velocity, 1);
    assert_eq!(take.notes[0].start, 100);
    // The piano roll shows a clip now: `None` is from before, and dropped.
    cmd(
        m,
        Command::AddNote {
            take: None,
            start: 9 * BAR,
            length: Q / 2,
            key: 60,
            velocity: 100,
        },
    );
    assert_eq!(m.view().song.undo_count, 1);
    cmd(m, Command::Undo);
    let v = m.view();
    assert!(v.song.tracks[0].clips.is_empty());
    assert_eq!(v.arrange.piano_roll_clip, None);
    assert_eq!(v.piano_roll.take, None);
    cmd(m, Command::Redo);
    assert_eq!(m.view().song.tracks[0].clips.len(), 1);
}

#[test]
fn the_audition_sounds_on_the_play_target() {
    let m = &mut ready();
    let effects = cmd(
        m,
        Command::Audition {
            keys: vec![60],
            velocity: 0,
        },
    );
    assert!(effects.contains(&Effect::Engine(EngineOp::Audition {
        track: 0,
        keys: vec![60],
        velocity: 1
    })));
    let effects = cmd(m, Command::AuditionOff);
    assert!(effects.contains(&Effect::Engine(EngineOp::AuditionOff)));
    assert_eq!(m.view().song.undo_count, 0);

    // A locked scale does not keep the keys column and the notes quiet.
    cmd(
        m,
        Command::SetMusicalScale {
            root: 0,
            mode: ScaleMode::Major,
            lock: true,
            highlight: true,
        },
    );
    let effects = cmd(
        m,
        Command::Audition {
            keys: vec![61, 64],
            velocity: 90,
        },
    );
    assert!(effects.contains(&Effect::Engine(EngineOp::Audition {
        track: 0,
        keys: vec![61, 64],
        velocity: 90
    })));

    let mut silent = crate::state::Model::new(crate::settings::Settings::default());
    let effects = silent.handle(crate::state::Input::Command(Command::Audition {
        keys: vec![60],
        velocity: 100,
    }));
    assert!(effects.iter().all(|e| !matches!(e, Effect::Engine(_))));
}

#[test]
fn settings_are_kept_and_out_of_range_ones_dropped() {
    let mut p = two_notes();
    p.set_grid(32, true);
    p.set_grid(12, false);
    p.set_musical_scale(4, ScaleMode::Dorian, true, false);
    p.set_musical_scale(12, ScaleMode::Major, false, true);
    let q = |division, strength, swing| QuantizeSettings {
        division,
        triplet: false,
        strength,
        swing,
        align: QuantizeAlign::StartAndEnd,
    };
    p.set_quantize(q(8, 60, 20));
    p.set_quantize(q(8, 101, 20));
    p.set_quantize(q(5, 60, 20));
    let v = p.piano_roll_view();
    assert_eq!(
        v.grid,
        Grid {
            division: 32,
            triplet: true
        }
    );
    assert_eq!(
        v.scale,
        MusicalScale {
            root: 4,
            mode: ScaleMode::Dorian,
            lock: true,
            highlight: false
        }
    );
    assert_eq!(v.quantize, q(8, 60, 20));
    assert_eq!(p.undo_count(), 0, "settings are not steps");
}

#[test]
fn edited_takes_and_settings_come_back_from_the_file() {
    let mut p = two_notes();
    let e = &mut Vec::new();
    p.move_notes(shown_id(&p), &[0], -(Q as i64), 3, false, e);
    p.add_note(
        Some(shown_id(&p)),
        Note {
            from: None,
            start: BAR + 100,
            length: 50,
            key: 72,
            velocity: 30,
        },
        e,
    );
    p.set_grid(0, false);
    p.set_musical_scale(7, ScaleMode::MinorPentatonic, true, false);
    let back = songfile::decode(&songfile::encode(p.song())).unwrap();
    assert_eq!(&back, p.song());
    let mut again = fresh(back);
    again.select_clip(Some(p.song().doc.tracks[0].clips[0].id.0));
    assert_eq!(again.piano_roll_view().take, p.piano_roll_view().take);
    assert_eq!(again.piano_roll_view().grid, p.piano_roll_view().grid);
    assert_eq!(again.piano_roll_view().scale, p.piano_roll_view().scale);
    // Writing after opening gives new ids, never one the file already has.
    again.add_note(
        Some(shown_id(&again)),
        Note {
            from: None,
            start: BAR,
            length: 50,
            key: 72,
            velocity: 30,
        },
        e,
    );
    assert_eq!(shown_id(&again), p.song().doc.next_id);
}

#[test]
fn quantize_and_back_through_the_commands() {
    let m = &mut ready();
    record_once(
        m,
        &[(60, 10.0, 100.0), (64, (Q + 20) as f64, (Q + 120) as f64)],
        (5 * Q / 2) as f64,
    );
    let recorded = clip_view(m).take_id;
    let before = TakeNotes::decode(&m.take_notes(recorded).unwrap()).unwrap();
    cmd(
        m,
        Command::SetQuantize {
            division: 4,
            triplet: false,
            strength: 100,
            swing: 0,
            align: QuantizeAlign::Start,
        },
    );
    cmd(
        m,
        Command::Quantize {
            take: recorded,
            notes: Vec::new(),
        },
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::Quantized {
            division: 4,
            triplet: false,
            strength: 100,
            count: 2
        })
    );
    let c = clip_view(m);
    assert_ne!(c.take_id, recorded);
    assert_eq!((c.take, c.takes), (1, 1));
    let take = TakeNotes::decode(&m.take_notes(c.take_id).unwrap()).unwrap();
    assert_eq!(take.origin, Some(recorded));
    assert_eq!(
        take.notes.iter().map(|n| n.from).collect::<Vec<_>>(),
        [Some(0), Some(1)]
    );
    assert_eq!(
        take.notes
            .iter()
            .map(|n| u64::from(n.start) + c.start + c.take_offset as u64)
            .collect::<Vec<_>>(),
        [0, Q]
    );
    // The recorded take is in no clip, but the screen still gets it by id.
    assert_eq!(
        TakeNotes::decode(&m.take_notes(recorded).unwrap()).unwrap(),
        before
    );
    cmd(
        m,
        Command::SelectNotes {
            take: c.take_id,
            notes: vec![1],
            mode: SelectMode::Replace,
        },
    );
    assert!(matches!(
        status(m),
        Some(StatusMessage::Summary {
            notes: Some(NoteSelection {
                quantized: true,
                ..
            }),
            ..
        })
    ));
    // The other old id does nothing.
    let song = m.view().song;
    cmd(
        m,
        Command::Unquantize {
            take: recorded,
            notes: Vec::new(),
        },
    );
    assert_eq!(m.view().song, song);
    cmd(
        m,
        Command::Unquantize {
            take: c.take_id,
            notes: Vec::new(),
        },
    );
    let back = clip_view(m);
    let take = TakeNotes::decode(&m.take_notes(back.take_id).unwrap()).unwrap();
    assert_eq!(take.origin, None);
    assert!(take.notes.iter().all(|n| n.from.is_none()));
    assert_eq!(
        take.notes
            .iter()
            .map(|n| (n.start, n.length))
            .collect::<Vec<_>>(),
        before
            .notes
            .iter()
            .map(|n| (n.start, n.length))
            .collect::<Vec<_>>()
    );
    cmd(m, Command::Undo);
    cmd(m, Command::Undo);
    assert_eq!(clip_view(m).take_id, recorded);
    assert_eq!(m.view().song.undo_count, 1, "the recording");
}

#[test]
fn control_points_through_the_commands() {
    let m = &mut ready();
    record_once(m, &[(60, 10.0, 100.0)], 1200.0);
    let t0 = clip_view(m).take_id;
    let c = clip_view(m);
    cmd(
        m,
        Command::AddControl {
            take: t0,
            tick: c.start + 100,
            control: ControlKind::Cc { number: 64 },
            value: 80,
        },
    );
    let t1 = clip_view(m).take_id;
    assert_ne!(t1, t0);
    let take = TakeNotes::decode(&m.take_notes(t1).unwrap()).unwrap();
    assert_eq!(
        take.controls.last().map(|p| p.value),
        Some(meloet_protocol::TakeControlValue::Cc {
            number: 64,
            value: 127
        })
    );
    let added = take.controls.len() as u32 - 1;
    let v = m.view().piano_roll;
    assert_eq!(
        v.selected_controls,
        [IndexRange {
            start: added,
            end: added + 1
        }]
    );
    // Selecting notes leaves the points selected; a note edit keeps them too.
    cmd(
        m,
        Command::SelectNotes {
            take: t1,
            notes: vec![0],
            mode: SelectMode::Replace,
        },
    );
    cmd(
        m,
        Command::SetNotesField {
            take: t1,
            notes: vec![0],
            field: NoteField::Velocity,
            value: 10,
        },
    );
    let t2 = clip_view(m).take_id;
    let v = m.view().piano_roll;
    assert_eq!(v.selected_notes, [IndexRange { start: 0, end: 1 }]);
    assert_eq!(v.selected_controls.len(), 1);
    cmd(
        m,
        Command::DeleteControls {
            take: t2,
            points: vec![added],
        },
    );
    let v = m.view().piano_roll;
    assert!(v.selected_controls.is_empty());
    assert_eq!(v.selected_notes, [IndexRange { start: 0, end: 1 }]);
    assert_eq!(m.view().song.undo_count, 4);
    cmd(m, Command::Undo);
    assert_eq!(clip_view(m).take_id, t2);
    assert_eq!(m.view().piano_roll.selected_controls.len(), 1);
    cmd(
        m,
        Command::SelectControls {
            take: t2,
            points: vec![added],
            mode: SelectMode::Toggle,
        },
    );
    assert!(m.view().piano_roll.selected_controls.is_empty());
}

/// One clip over bar 2 with notes 10 ticks after each beat, its head and end trimmed by a beat:
/// only the notes of beats 2 and 3 start inside it.
fn trimmed_clip() -> (Play, u32) {
    let mut s = Song::new_default();
    let take = NewTake {
        origin: BAR,
        length: BAR,
        notes: (0..4)
            .map(|i| Note {
                from: None,
                start: 10 + Q * i,
                length: 100,
                key: 60 + i as u8,
                velocity: 90,
            })
            .collect(),
        controls: Vec::new(),
    };
    s.doc = add_recording(&s.doc, TrackId(1), vec![take], 0)
        .unwrap()
        .0
        .doc;
    let mut p = fresh(s);
    let clip = p.song().doc.tracks[0].clips[0].id.0;
    p.select_clip(Some(clip));
    (p, clip)
}

fn trim_both(p: &mut Play, clip: u32) {
    let e = &mut Vec::new();
    p.trim_clip(clip, ClipEdge::Start, Q as i64, e);
    p.trim_clip(clip, ClipEdge::End, -(Q as i64), e);
    let c = &p.clips_of(TrackId(1))[0];
    assert_eq!((c.start, c.end), (BAR + Q, 2 * BAR - Q));
}

#[test]
fn a_trimmed_clip_counts_and_selects_only_the_notes_inside() {
    let (mut p, clip) = trimmed_clip();
    assert_eq!(p.clips_of(TrackId(1))[0].note_count, 4);
    p.select_all_notes();
    assert_eq!(p.note_sel.notes, [0, 1, 2, 3]);

    trim_both(&mut p, clip);
    assert_eq!(p.clips_of(TrackId(1))[0].note_count, 2);
    assert_eq!(p.piano_roll_notes(), 2);
    assert_eq!(p.note_sel.notes, [1, 2], "the trimmed-off notes drop out");
    let t = shown_id(&p);
    p.clear_note_selection();
    p.select_all_notes();
    assert_eq!(
        p.piano_roll_view().selected_notes,
        [IndexRange { start: 1, end: 3 }]
    );
    let Some(StatusMessage::Summary {
        notes: Some(sel), ..
    }) = p.summary()
    else {
        panic!("{:?}", p.summary());
    };
    assert_eq!((sel.count, &sel.keys[..]), (2, &[61, 62][..]));
    p.select_notes(t, vec![0, 1, 3], SelectMode::Replace);
    assert_eq!(p.note_sel.notes, [1]);

    // The head back: its note counts again, and the selection stays as it was.
    p.trim_clip(clip, ClipEdge::Start, -(Q as i64), &mut Vec::new());
    assert_eq!(p.clips_of(TrackId(1))[0].note_count, 3);
    assert_eq!(p.note_sel.notes, [1]);
}

#[test]
fn quantizing_all_of_a_trimmed_clip_moves_only_the_notes_inside() {
    let (mut p, clip) = trimmed_clip();
    trim_both(&mut p, clip);
    let e = &mut Vec::new();
    let starts = |p: &Play| -> Vec<u64> {
        let c = p.shown().unwrap();
        let z = c.zero(c.active_take());
        let take = &c.active_take().take;
        take.notes
            .iter()
            .map(|n| (z + n.start as i64) as u64 - BAR)
            .collect()
    };
    let t = shown_id(&p);
    let notice = p.quantize(t, &[], e);
    assert!(
        matches!(notice, Some(StatusMessage::Quantized { count: 2, .. })),
        "{notice:?}"
    );
    assert_eq!(starts(&p), [10, Q, 2 * Q, 3 * Q + 10]);

    let t = shown_id(&p);
    p.unquantize(t, &[], e);
    assert_eq!(starts(&p), [10, Q + 10, 2 * Q + 10, 3 * Q + 10]);
    assert_eq!(
        p.steps().last(),
        Some(&StepKind::Unquantize {
            clip: ClipId(clip),
            count: 2
        })
    );
}

#[test]
fn the_start_edge_and_matching_velocities_through_the_commands() {
    let m = &mut ready();
    record_once(
        m,
        &[(60, 10.0, 100.0), (64, (Q + 20) as f64, (Q + 120) as f64)],
        (5 * Q / 2) as f64,
    );
    let take = clip_view(m).take_id;
    let before = TakeNotes::decode(&m.take_notes(take).unwrap()).unwrap();
    let steps = m.view().song.undo_count;
    cmd(
        m,
        Command::ResizeNotes {
            take,
            notes: vec![0, 1],
            edge: ClipEdge::Start,
            ticks: 20,
        },
    );
    let take = clip_view(m).take_id;
    let after = TakeNotes::decode(&m.take_notes(take).unwrap()).unwrap();
    for (a, b) in after.notes.iter().zip(&before.notes) {
        assert_eq!(a.start, b.start + 20);
        assert_eq!(a.start + a.length, b.start + b.length, "the end stays");
    }
    assert_eq!(m.view().song.undo_count, steps + 1);
    cmd(
        m,
        Command::MatchVelocity {
            take,
            notes: vec![1, 0, 1],
            value: 64,
        },
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::VelocityMatched {
            count: 2,
            value: 64
        })
    );
    assert_eq!(m.view().song.undo_count, steps + 2);
    let take = clip_view(m).take_id;
    let after = TakeNotes::decode(&m.take_notes(take).unwrap()).unwrap();
    assert!(after.notes.iter().all(|n| n.velocity == 64));
    // Already at the value: said, but no step. An old take: nothing.
    cmd(
        m,
        Command::MatchVelocity {
            take,
            notes: vec![0],
            value: 64,
        },
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::VelocityMatched {
            count: 1,
            value: 64
        })
    );
    assert_eq!(m.view().song.undo_count, steps + 2);
    cmd(
        m,
        Command::MatchVelocity {
            take: take + 1000,
            notes: vec![0],
            value: 20,
        },
    );
    assert_eq!(m.view().song.undo_count, steps + 2);
}

#[test]
fn matching_lengths_is_one_step_and_undoes() {
    let m = &mut ready();
    record_once(
        m,
        &[(60, 10.0, 100.0), (64, (Q + 20) as f64, (Q + 120) as f64)],
        (5 * Q / 2) as f64,
    );
    let take = clip_view(m).take_id;
    let before = TakeNotes::decode(&m.take_notes(take).unwrap()).unwrap();
    let steps = m.view().song.undo_count;
    cmd(
        m,
        Command::MatchLength {
            take,
            notes: vec![1, 0, 1],
            value: Q / 2,
        },
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::LengthMatched {
            count: 2,
            value: Q / 2
        })
    );
    assert_eq!(m.view().song.undo_count, steps + 1);
    let matched = clip_view(m).take_id;
    let after = TakeNotes::decode(&m.take_notes(matched).unwrap()).unwrap();
    assert!(after.notes.iter().all(|n| u64::from(n.length) == Q / 2));
    for (a, b) in after.notes.iter().zip(&before.notes) {
        assert_eq!(a.start, b.start, "the starts stay");
    }

    cmd(m, Command::Undo);
    let back = TakeNotes::decode(&m.take_notes(clip_view(m).take_id).unwrap()).unwrap();
    assert_eq!(back.notes, before.notes);
    cmd(m, Command::Redo);
    let take = clip_view(m).take_id;

    // Already at the value: said, but no step. An old take: nothing.
    cmd(
        m,
        Command::MatchLength {
            take,
            notes: vec![0],
            value: Q / 2,
        },
    );
    assert_eq!(
        status(m),
        Some(StatusMessage::LengthMatched {
            count: 1,
            value: Q / 2
        })
    );
    assert_eq!(m.view().song.undo_count, steps + 1);
    cmd(
        m,
        Command::MatchLength {
            take: take + 1000,
            notes: vec![0],
            value: Q / 4,
        },
    );
    assert_eq!(m.view().song.undo_count, steps + 1);

    // At least 1 tick; and no note goes past the end of its clip.
    cmd(
        m,
        Command::MatchLength {
            take,
            notes: vec![0],
            value: 0,
        },
    );
    let take = clip_view(m).take_id;
    let short = TakeNotes::decode(&m.take_notes(take).unwrap()).unwrap();
    assert_eq!(short.notes[0].length, 1);
    cmd(
        m,
        Command::MatchLength {
            take,
            notes: vec![0, 1],
            value: u64::MAX,
        },
    );
    let c = clip_view(m);
    let long = TakeNotes::decode(&m.take_notes(c.take_id).unwrap()).unwrap();
    let zero = (c.start as i64 + c.take_offset) as u64;
    for n in &long.notes {
        assert_eq!(zero + u64::from(n.start) + u64::from(n.length), c.end);
    }
    assert_eq!(m.view().song.undo_count, steps + 3);
}

fn clip_range(p: &Play) -> (u64, u64) {
    let c = &p.song().doc.tracks[0].clips[0];
    (c.start, c.end)
}

/// B-82: written, moved or resized out of the clip, the clip grows by bars as the same step.
#[test]
fn notes_out_of_the_clip_grow_it_in_one_step() {
    let mut p = two_notes();
    let e = &mut Vec::new();
    let t0 = shown_id(&p);
    assert_eq!(clip_range(&p), (BAR, 2 * BAR));
    let note = Note {
        from: None,
        start: 2 * BAR + 100,
        length: Q / 2,
        key: 67,
        velocity: 90,
    };
    assert_eq!(p.add_note(Some(t0), note, e), None);
    assert_eq!(clip_range(&p), (BAR, 3 * BAR));
    assert_eq!(p.undo_count(), 1);
    assert_eq!(p.note_sel.notes, [2]);
    let t1 = shown_id(&p);
    p.undo(e);
    assert_eq!((clip_range(&p), shown_id(&p)), ((BAR, 2 * BAR), t0));
    p.redo(e);
    assert_eq!((clip_range(&p), shown_id(&p)), ((BAR, 3 * BAR), t1));

    // Back past the song's start: the notes stop at 0, the clip starts at 0.
    assert_eq!(
        p.move_notes(t1, &[0, 1], -10 * BAR as i64, 0, false, e),
        None
    );
    assert_eq!(clip_range(&p), (0, 3 * BAR));
    assert_eq!(p.undo_count(), 2);
    let t2 = shown_id(&p);
    // The old take: nothing.
    assert_eq!(p.move_notes(t1, &[0], 5 * BAR as i64, 0, false, e), None);
    assert_eq!(
        p.resize_notes(t1, &[0], ClipEdge::End, 5 * BAR as i64, e),
        None
    );
    assert_eq!((p.undo_count(), shown_id(&p)), (2, t2));
    // The end of the last note, out to bar 6.
    assert_eq!(
        p.resize_notes(t2, &[2], ClipEdge::End, 3 * BAR as i64, e),
        None
    );
    assert_eq!(clip_range(&p), (0, 6 * BAR));
    p.undo(e);
    assert_eq!(clip_range(&p), (0, 3 * BAR));
    p.undo(e);
    assert_eq!((clip_range(&p), shown_id(&p)), ((BAR, 3 * BAR), t1));
}

#[test]
fn the_status_line_says_when_the_clip_cannot_grow() {
    let m = &mut ready();
    record_once(m, &[(60, 10.0, 100.0)], 1200.0);
    let c = clip_view(m);
    cmd(m, Command::DuplicateClips { clips: vec![c.id] });
    cmd(m, Command::SelectClip { clip: Some(c.id) });
    let take = clip_view(m).take_id;
    let steps = m.view().song.undo_count;
    let end = c.end;
    cmd(
        m,
        Command::AddNote {
            take: Some(take),
            start: end + 10,
            length: 10,
            key: 60,
            velocity: 100,
        },
    );
    assert_eq!(status(m), Some(StatusMessage::NotesNotWritten));
    cmd(
        m,
        Command::MoveNotes {
            take,
            notes: vec![0],
            ticks: (end + BAR) as i64,
            keys: 0,
            copy: false,
        },
    );
    assert_eq!(status(m), Some(StatusMessage::ClipsNotMoved));
    cmd(
        m,
        Command::MoveNotes {
            take,
            notes: vec![0],
            ticks: (end + BAR) as i64,
            keys: 0,
            copy: true,
        },
    );
    assert_eq!(status(m), Some(StatusMessage::ClipsNotDuplicated));
    cmd(
        m,
        Command::ResizeNotes {
            take,
            notes: vec![0],
            edge: ClipEdge::End,
            ticks: (end + BAR) as i64,
        },
    );
    assert_eq!(status(m), Some(StatusMessage::NotesNotResized));
    assert_eq!(m.view().song.undo_count, steps);
    assert_eq!(clip_view(m).end, end);
    // Inside the clip: a step, and nothing said.
    cmd(
        m,
        Command::MoveNotes {
            take,
            notes: vec![0],
            ticks: 1,
            keys: 0,
            copy: false,
        },
    );
    assert_eq!(m.view().song.undo_count, steps + 1);
    assert_ne!(status(m), Some(StatusMessage::ClipsNotMoved));
}

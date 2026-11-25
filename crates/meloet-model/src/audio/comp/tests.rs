use std::sync::Arc;

use super::*;
use crate::arrange::{ClipEdge, move_clips, repeat_clips, split_clips, trim_clip};
use crate::audio::record::{NewAudio, NewAudioTake, VocalRecording, add_vocal_recordings};
use crate::audio::{
    AudioId, AudioShape, AudioSource, AudioTake, SourceTempo, TrackKind, VocalInput,
    vocal_track_name,
};
use crate::edit::{delete_other_takes, delete_take, duplicate_track, switch_take};
use crate::sections::delete_section;
use crate::song::{ClipTake, ClipTakes, Song, TakeId};
use crate::{History, TICKS_PER_QUARTER, TrackId, played_frames, remove_audio};

const BAR: u64 = 4 * TICKS_PER_QUARTER;
const CLIP: ClipId = ClipId(10);

fn source(id: u32) -> Arc<AudioSource> {
    Arc::new(AudioSource {
        id: AudioId(id),
        uid: [id as u8; 16],
        file: format!("歌 {id}.wav"),
        frames: 960_000,
        sample_rate: 48_000,
        channels: 1,
        bits: Some(24),
        sha256: None,
    })
}

/// The default song with a vocal track (id 2) whose one clip (id 10) is bars 2 to 5 and has
/// three takes; the third is of another file.
fn song() -> Doc {
    let mut song = Song::new_default();
    let (one, other) = (source(11), source(14));
    let tempo = song.doc.timeline.tempo;
    let take = |id, source: &Arc<AudioSource>, first_frame| ClipTake {
        take: Arc::new(AudioTake {
            id: TakeId(id),
            source: source.clone(),
            first_frame,
            frames: 384_000,
            sync: Some(SourceTempo {
                tempo,
                head_ramp: false,
                points: Vec::new(),
            }),
        }),
        offset: 0,
    };
    let clip = Clip {
        id: CLIP,
        name: "歌".to_owned(),
        start: BAR,
        end: 5 * BAR,
        takes: ClipTakes::Vocal {
            takes: vec![
                take(12, &one, 0),
                take(13, &one, 384_000),
                take(15, &other, 0),
            ],
            shape: AudioShape::RECORDED,
        },
        active: 2,
        comp: Vec::new(),
    };
    let keys = Track::clone(&song.doc.tracks[0]);
    song.doc.tracks.push(Arc::new(Track {
        id: TrackId(2),
        name: vocal_track_name(crate::Language::Ja).to_owned(),
        kind: TrackKind::Vocal {
            input: VocalInput::Mono1,
        },
        clips: vec![Arc::new(clip)],
        ..keys
    }));
    song.doc.audio = Arc::new(vec![one, other]);
    song.doc.next_id = 20;
    song.doc
}

fn pieces(list: &[(u64, usize)]) -> Vec<CompPiece> {
    (list.iter())
        .map(|&(from, take)| CompPiece { from, take })
        .collect()
}

fn comp_of(doc: &Doc, clip: ClipId) -> Vec<(u64, usize)> {
    let (_, c) = doc.clip(clip).unwrap();
    c.comp.iter().map(|p| (p.from, p.take)).collect()
}

/// The song with the comp: take 1 for a bar, take 2 for two bars, take 3 for the last.
fn comped() -> Doc {
    let comp = pieces(&[(0, 0), (BAR, 1), (3 * BAR, 2)]);
    set_comp(&song(), CLIP, (BAR, 5 * BAR), &comp).unwrap().doc
}

#[test]
fn the_rules_of_a_comp() {
    assert!(comp_is_valid(&[], BAR, 0));
    assert!(comp_is_valid(&pieces(&[(0, 1)]), BAR, 2));
    assert!(comp_is_valid(&pieces(&[(0, 1), (5, 0), (9, 1)]), BAR, 2));
    // Not from 0, out of order, two at one place, two in a row of one take, past the end,
    // no such take.
    assert!(!comp_is_valid(&pieces(&[(1, 1)]), BAR, 2));
    assert!(!comp_is_valid(&pieces(&[(0, 1), (9, 0), (5, 1)]), BAR, 2));
    assert!(!comp_is_valid(&pieces(&[(0, 1), (5, 0), (5, 1)]), BAR, 2));
    assert!(!comp_is_valid(&pieces(&[(0, 1), (5, 1)]), BAR, 2));
    assert!(!comp_is_valid(&pieces(&[(0, 1), (BAR, 0)]), BAR, 2));
    assert!(!comp_is_valid(&pieces(&[(0, 2)]), BAR, 2));
}

#[test]
fn setting_and_clearing_are_steps_and_the_takes_stay() {
    let mut doc = song();
    let mut history = History::default();
    let before = doc.clone();
    let comp = pieces(&[(0, 0), (BAR, 1), (3 * BAR, 2)]);
    let edit = set_comp(&doc, CLIP, (BAR, 5 * BAR), &comp).unwrap();
    assert_eq!(edit.step, StepKind::SetComp { clip: CLIP });
    let (_, old) = before.clip(CLIP).unwrap();
    let (_, new) = edit.doc.clip(CLIP).unwrap();
    assert_eq!((&new.takes, new.active), (&old.takes, old.active));
    assert_eq!(new.comp, comp);
    history.apply(&mut doc, edit);
    let with = doc.clone();

    let edit = clear_comp(&doc, CLIP).unwrap();
    assert_eq!(edit.step, StepKind::ClearComp { clip: CLIP });
    history.apply(&mut doc, edit);
    assert_eq!(doc, before);
    assert_eq!(clear_comp(&doc, CLIP).err(), Some(EditError::Unchanged));

    assert_eq!(
        history.undo(&mut doc),
        Some(StepKind::ClearComp { clip: CLIP })
    );
    assert_eq!(doc, with);
    assert_eq!(
        history.undo(&mut doc),
        Some(StepKind::SetComp { clip: CLIP })
    );
    assert_eq!(doc, before);
    assert!(history.redo(&mut doc).is_some());
    assert_eq!(doc, with);
}

#[test]
fn the_shown_take_whole_takes_the_comp_away() {
    let before = song();
    let mut doc = before.clone();
    let mut history = History::default();
    let span = (BAR, 5 * BAR);
    let comp = pieces(&[(0, 0), (BAR, 1), (3 * BAR, 2)]);
    let edit = set_comp(&doc, CLIP, span, &comp).unwrap();
    history.apply(&mut doc, edit);
    let with = doc.clone();

    // Take 3 is the shown one: laid over the whole clip it is what plays without a comp.
    let edit = set_comp(&doc, CLIP, span, &pieces(&[(0, 2)])).unwrap();
    assert_eq!(edit.step, StepKind::SetComp { clip: CLIP });
    history.apply(&mut doc, edit);
    assert!(comp_of(&doc, CLIP).is_empty());
    assert_eq!(doc, before);
    assert_eq!(
        set_comp(&doc, CLIP, span, &pieces(&[(0, 2)])).err(),
        Some(EditError::Unchanged)
    );
    // Another take whole stays a comp.
    let other = set_comp(&with, CLIP, span, &pieces(&[(0, 1)])).unwrap().doc;
    assert_eq!(comp_of(&other, CLIP), [(0, 1)]);

    assert_eq!(
        history.undo(&mut doc),
        Some(StepKind::SetComp { clip: CLIP })
    );
    assert_eq!(doc, with);
    assert!(history.redo(&mut doc).is_some());
    assert_eq!(doc, before);
}

#[test]
fn what_is_not_a_comp_of_the_clip_does_nothing() {
    let doc = song();
    let span = (BAR, 5 * BAR);
    let good = pieces(&[(0, 0), (BAR, 1)]);
    let no = |r: Result<Edit, EditError>| assert_eq!(r.err(), Some(EditError::Unchanged));
    assert_eq!(
        set_comp(&doc, ClipId(99), span, &good).err(),
        Some(EditError::NoSuchClip(ClipId(99)))
    );
    // The clip has other edges than the screen drew.
    no(set_comp(&doc, CLIP, (BAR, 4 * BAR), &good));
    no(set_comp(&doc, CLIP, (0, 5 * BAR), &good));
    no(set_comp(&doc, CLIP, span, &[]));
    no(set_comp(&doc, CLIP, span, &pieces(&[(0, 0), (BAR, 3)])));
    no(set_comp(&doc, CLIP, span, &pieces(&[(BAR, 1)])));
    no(set_comp(&doc, CLIP, span, &pieces(&[(0, 0), (4 * BAR, 1)])));
    no(set_comp(&doc, CLIP, span, &pieces(&[(0, 0), (BAR, 0)])));
    // What the clip plays already: the shown take whole, or the comp it has.
    no(set_comp(&doc, CLIP, span, &pieces(&[(0, 2)])));
    let with = set_comp(&doc, CLIP, span, &good).unwrap().doc;
    no(set_comp(&with, CLIP, span, &good));
    // One take whole that is not the shown one is a comp.
    assert!(set_comp(&doc, CLIP, span, &pieces(&[(0, 0)])).is_ok());

    // A vocal clip of one take.
    let one = remove_audio(&doc, AudioId(11)).unwrap().doc;
    no(set_comp(&one, CLIP, span, &pieces(&[(0, 0)])));
}

#[test]
fn windows_of_a_comp() {
    let comp = pieces(&[(0, 0), (100, 1), (300, 2)]);
    let window = |shift, length| {
        (comp_window(&comp, shift, length).iter())
            .map(|p| (p.from, p.take))
            .collect::<Vec<_>>()
    };
    assert_eq!(window(0, 400), [(0, 0), (100, 1), (300, 2)]);
    // The end moved in: on a seam the piece after it is gone, a tick later it is there.
    assert_eq!(window(0, 300), [(0, 0), (100, 1)]);
    assert_eq!(window(0, 301), [(0, 0), (100, 1), (300, 2)]);
    assert_eq!(window(0, 1), [(0, 0)]);
    // The end moved out: the last piece goes on.
    assert_eq!(window(0, 9_000), [(0, 0), (100, 1), (300, 2)]);
    // The start moved in: by less than the first piece, onto a seam, past it.
    assert_eq!(window(40, 360), [(0, 0), (60, 1), (260, 2)]);
    assert_eq!(window(100, 300), [(0, 1), (200, 2)]);
    assert_eq!(window(150, 250), [(0, 1), (150, 2)]);
    assert_eq!(window(350, 50), [(0, 2)]);
    // The start moved out: the first piece reaches back.
    assert_eq!(window(-50, 450), [(0, 0), (150, 1), (350, 2)]);
    assert_eq!(comp_window(&[], 5, 10), []);
    // Pieces of one take that meet after a cut are one.
    let again = pieces(&[(0, 0), (100, 1), (200, 0)]);
    assert_eq!(comp_window(&again, 0, 100), pieces(&[(0, 0)]));
    assert_eq!(comp_window(&again, 100, 200), pieces(&[(0, 1), (100, 0)]));
}

#[test]
fn trimming_cuts_the_comp_and_moving_keeps_it() {
    let doc = comped();
    let whole = [(0, 0), (BAR, 1), (3 * BAR, 2)];
    let moved = move_clips(&doc, &[CLIP], 8 * BAR as i64, 0, false).unwrap();
    assert_eq!(comp_of(&moved.edit.doc, CLIP), whole);
    let copy = move_clips(&doc, &[CLIP], 8 * BAR as i64, 0, true).unwrap();
    assert_eq!(comp_of(&copy.edit.doc, copy.clips[0]), whole);
    let repeated = repeat_clips(&doc, &[CLIP], 2).unwrap();
    assert_eq!(repeated.clips.len(), 2);
    for id in &repeated.clips {
        assert_eq!(comp_of(&repeated.edit.doc, *id), whole);
    }

    let trim = |doc: &Doc, edge, ticks: i64| trim_clip(doc, CLIP, edge, ticks).unwrap().doc;
    // The end in by a bar and a half: the last piece is gone.
    let short = trim(&doc, ClipEdge::End, -(3 * BAR as i64) / 2);
    assert_eq!(comp_of(&short, CLIP), [(0, 0), (BAR, 1)]);
    // And out again: the piece does not come back; the last one goes on.
    let long = trim(&short, ClipEdge::End, 4 * BAR as i64);
    assert_eq!(comp_of(&long, CLIP), [(0, 0), (BAR, 1)]);
    // The start in by half a bar, then by two more: what sounds at a place stays there.
    let late = trim(&doc, ClipEdge::Start, BAR as i64 / 2);
    assert_eq!(
        comp_of(&late, CLIP),
        [(0, 0), (BAR / 2, 1), (5 * BAR / 2, 2)]
    );
    let later = trim(&late, ClipEdge::Start, 2 * BAR as i64);
    assert_eq!(comp_of(&later, CLIP), [(0, 1), (BAR / 2, 2)]);
    // The start out: the first piece reaches back.
    let early = trim(&doc, ClipEdge::Start, -(BAR as i64));
    assert_eq!(comp_of(&early, CLIP), [(0, 0), (2 * BAR, 1), (4 * BAR, 2)]);
    for d in [&short, &long, &late, &later, &early] {
        let (_, c) = d.clip(CLIP).unwrap();
        assert!(comp_is_valid(&c.comp, c.end - c.start, c.takes.len()));
    }
}

#[test]
fn a_cut_gives_each_half_its_pieces() {
    let doc = comped();
    // Inside the second piece.
    let split = split_clips(&doc, &[CLIP], 3 * BAR).unwrap();
    let (left, right) = (split.left[0], split.right[0]);
    assert_eq!(comp_of(&split.edit.doc, left), [(0, 0), (BAR, 1)]);
    assert_eq!(comp_of(&split.edit.doc, right), [(0, 1), (BAR, 2)]);
    // On a seam.
    let split = split_clips(&doc, &[CLIP], 2 * BAR).unwrap();
    assert_eq!(comp_of(&split.edit.doc, split.left[0]), [(0, 0)]);
    assert_eq!(
        comp_of(&split.edit.doc, split.right[0]),
        [(0, 1), (2 * BAR, 2)]
    );
    // A clip with no comp has none in its halves.
    let split = split_clips(&song(), &[CLIP], 2 * BAR).unwrap();
    assert_eq!(comp_of(&split.edit.doc, split.right[0]), []);
}

#[test]
fn a_section_cut_cuts_the_comp() {
    // Bars 3 and 4 go (the section from the marker at bar 3 to the one at bar 5): the clip
    // keeps bar 2 where it is, and its last bar comes right after it as another clip.
    let doc = comped();
    let doc = crate::timeline::add_marker(&doc, 2 * BAR, "A")
        .unwrap()
        .0
        .doc;
    let doc = crate::timeline::add_marker(&doc, 4 * BAR, "B")
        .unwrap()
        .0
        .doc;
    let marker = doc.markers.iter().find(|m| m.tick == 2 * BAR).unwrap().id;
    let cut = delete_section(&doc, marker).unwrap().doc;
    let clips = &cut.tracks.last().unwrap().clips;
    let comps: Vec<_> = (clips.iter())
        .map(|c| (c.start, c.end, comp_of(&cut, c.id)))
        .collect();
    assert_eq!(
        comps,
        [
            (BAR, 2 * BAR, vec![(0, 0)]),
            (2 * BAR, 3 * BAR, vec![(0, 2)]),
        ]
    );
}

#[test]
fn takes_and_the_comp() {
    let doc = comped();
    // Another take: the comp goes in the same step.
    let switched = switch_take(&doc, CLIP, 0).unwrap();
    let (_, c) = switched.doc.clip(CLIP).unwrap();
    assert_eq!((c.active, c.comp.len()), (0, 0));
    assert_eq!(delete_take(&doc, CLIP).err(), Some(EditError::Unchanged));
    assert!(delete_take(&song(), CLIP).is_ok());
    assert_eq!(
        delete_other_takes(&doc, CLIP).err(),
        Some(EditError::Unchanged)
    );
    assert!(delete_other_takes(&song(), CLIP).is_ok());
    assert_eq!(played_frames(&doc, CLIP), None);
    assert!(played_frames(&song(), CLIP).is_some());

    // The file of the third take leaves the song: a piece names it, so the comp goes.
    let gone = remove_audio(&doc, AudioId(14)).unwrap().doc;
    assert_eq!(comp_of(&gone, CLIP), []);
    // The file of the first two leaves a song whose comp names only the third.
    let third = pieces(&[(0, 2)]);
    let shown_first = switch_take(&song(), CLIP, 0).unwrap().doc;
    let only = set_comp(&shown_first, CLIP, (BAR, 5 * BAR), &third);
    let gone = remove_audio(&only.unwrap().doc, AudioId(11)).unwrap().doc;
    assert_eq!(comp_of(&gone, CLIP), [(0, 0)]);

    // A copy of the track has the comp, and shares the takes.
    let copy = duplicate_track(&doc, TrackId(2)).unwrap().edit.doc;
    let n = copy.tracks.len();
    let (a, b) = (&copy.tracks[n - 2].clips[0], &copy.tracks[n - 1].clips[0]);
    assert_eq!(a.comp, b.comp);
    assert_eq!(a.comp.len(), 3);
    assert_ne!(a.id, b.id);
    assert!(Arc::ptr_eq(
        &a.takes.vocal()[0].take,
        &b.takes.vocal()[0].take
    ));
}

fn recording(uid: u8, origin: u64) -> VocalRecording {
    VocalRecording {
        track: TrackId(2),
        audio: NewAudio {
            uid: [uid; 16],
            file: format!("歌 {uid}.wav"),
            frames: 96_000,
            sample_rate: 48_000,
            channels: 1,
            bits: Some(24),
        },
        takes: vec![NewAudioTake {
            origin,
            length: BAR,
            first_frame: 0,
            frames: 96_000,
        }],
    }
}

#[test]
fn a_recording_over_the_clip_keeps_what_it_plays() {
    // From half a bar before the clip, a bar long.
    let (edit, _) = add_vocal_recordings(&comped(), vec![recording(9, BAR / 2)], None).unwrap();
    let (_, c) = edit.doc.clip(CLIP).unwrap();
    assert_eq!((c.start, c.takes.len(), c.active), (BAR / 2, 4, 3));
    assert_eq!(
        comp_of(&edit.doc, CLIP),
        [(0, 0), (3 * BAR / 2, 1), (7 * BAR / 2, 2)]
    );
    assert!(comp_is_valid(&c.comp, c.end - c.start, c.takes.len()));
    // Without a comp there is none, as before.
    let (edit, _) = add_vocal_recordings(&song(), vec![recording(8, BAR)], None).unwrap();
    assert_eq!(comp_of(&edit.doc, CLIP), []);
}

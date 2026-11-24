use std::sync::Arc;

use meloet_model::song::{Clip, ClipTake, ClipTakes, Doc, Track};
use meloet_model::{
    AudioId, AudioShape, AudioSource, AudioTake, ClipId, TakeId, TrackColor, TrackId, TrackKind,
};
use meloet_protocol::{
    Command, CompPiece, SelectMode, SelectedParams, StepName, StretchChange, TickSpan,
};

use super::super::EngineOp;
use super::super::tests::{cmd, ready};
use crate::state::{Effect, Model};

const Q: u64 = meloet_model::TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;
const CLIP: u32 = 31;
/// The ids of the clip's takes, in their order.
const TAKES: [u32; 3] = [33, 34, 35];

/// The new song with a vocal track (id 3) whose one clip (id 31) is bars 1 to 4 and has
/// three takes of one file; the last is shown.
fn song() -> Model {
    let mut m = ready();
    let mut song = m.play().song().clone();
    let keys = Track::clone(&song.doc.tracks[0]);
    let source = Arc::new(AudioSource {
        id: AudioId(30),
        uid: [3; 16],
        file: "歌.wav".into(),
        frames: 4_800_000,
        sample_rate: 48_000,
        channels: 1,
        bits: Some(24),
        sha256: None,
    });
    let takes = (TAKES.iter().enumerate())
        .map(|(i, &id)| ClipTake {
            take: Arc::new(AudioTake {
                id: TakeId(id),
                source: source.clone(),
                first_frame: i as u64 * 400_000,
                frames: 400_000,
                sync: None,
            }),
            offset: 0,
        })
        .collect();
    song.doc.tracks.push(Arc::new(Track {
        id: TrackId(3),
        name: "歌".into(),
        color: TrackColor::for_index(2),
        kind: TrackKind::Vocal {
            input: meloet_model::VocalInput::Mono1,
        },
        clips: vec![Arc::new(Clip {
            id: ClipId(CLIP),
            name: "歌".into(),
            start: 0,
            end: 4 * BAR,
            takes: ClipTakes::Vocal {
                takes,
                shape: AudioShape::RECORDED,
            },
            active: 2,
            comp: Vec::new(),
        })],
        ..keys
    }));
    song.doc.audio = Arc::new(vec![source]);
    song.doc.next_id = 40;
    m.play_mut().replace_song(song, false, &mut Vec::new());
    m
}

fn pieces(list: &[(u64, u32)]) -> Vec<CompPiece> {
    (list.iter())
        .map(|&(from, take)| CompPiece { from, take })
        .collect()
}

fn set(m: &mut Model, span: (u64, u64), list: &[(u64, u32)]) -> Vec<Effect> {
    let span = TickSpan {
        start: span.0,
        end: span.1,
    };
    let pieces = pieces(list);
    cmd(
        m,
        Command::SetComp {
            clip: CLIP,
            span,
            pieces,
        },
    )
}

/// A clip's start and end, and its pieces as (from, take id).
type Comped = (u64, u64, Vec<(u64, u32)>);

fn comps(m: &Model) -> Vec<Comped> {
    (m.view().song.tracks[1].clips.iter())
        .map(|c| {
            let comp = c.comp.iter().map(|p| (p.from, p.take)).collect();
            (c.start, c.end, comp)
        })
        .collect()
}

fn comp(m: &Model) -> Vec<(u64, u32)> {
    comps(m).remove(0).2
}

fn steps(m: &Model) -> u32 {
    m.view().song.undo_count
}

fn doc(m: &Model) -> Doc {
    m.play().song().doc.clone()
}

fn last_step(m: &Model) -> StepName {
    let n = steps(m);
    m.history_rows(n, n).pop().unwrap().name
}

fn clips_sent(effects: &[Effect]) -> usize {
    (effects.iter())
        .filter(|e| matches!(e, Effect::Engine(EngineOp::Clips { .. })))
        .count()
}

const WHOLE: (u64, u64) = (0, 4 * BAR);
const THREE: [(u64, u32); 3] = [(0, TAKES[0]), (BAR, TAKES[1]), (3 * BAR, TAKES[2])];

#[test]
fn a_comp_is_one_step_by_take_ids_and_undo_brings_the_song_back() {
    let m = &mut song();
    let before = doc(m);
    let effects = set(m, WHOLE, &THREE);
    assert_eq!(comp(m), THREE);
    assert_eq!((steps(m), last_step(m)), (1, StepName::SetComp));
    assert_eq!(clips_sent(&effects), 1, "the engine hears of the pieces");
    let with = doc(m);
    // The takes and the shown take are what they were.
    let (old, new) = (&before.tracks[1].clips[0], &with.tracks[1].clips[0]);
    assert_eq!((&old.takes, old.active), (&new.takes, new.active));
    assert_eq!(m.view().song.tracks[1].clips[0].take_id, TAKES[2]);

    let effects = cmd(m, Command::ClearComp { clip: CLIP });
    assert_eq!(comp(m), []);
    assert_eq!((steps(m), last_step(m)), (2, StepName::ClearComp));
    assert_eq!(clips_sent(&effects), 1);
    assert_eq!(doc(m), before);
    cmd(m, Command::ClearComp { clip: CLIP });
    assert_eq!(steps(m), 2, "no comp to take away");

    cmd(m, Command::Undo);
    assert_eq!(doc(m), with);
    let effects = cmd(m, Command::Undo);
    assert_eq!(doc(m), before);
    assert_eq!(clips_sent(&effects), 1);
    cmd(m, Command::Redo);
    assert_eq!(doc(m), with);
    assert_eq!(comp(m), THREE);
}

#[test]
fn what_does_not_fit_the_clip_does_nothing() {
    let m = &mut song();
    let none = |m: &mut Model, span, list: &[(u64, u32)]| {
        let before = (doc(m), steps(m));
        let effects = set(m, span, list);
        assert_eq!((doc(m), steps(m)), before, "{list:?}");
        assert_eq!(clips_sent(&effects), 0);
    };
    // A take id of another clip or of long ago, other edges, pieces that are no comp, and
    // what sounds already.
    none(m, WHOLE, &[(0, TAKES[0]), (BAR, 99)]);
    none(m, (0, 3 * BAR), &THREE);
    none(m, (BAR, 4 * BAR), &THREE);
    none(m, WHOLE, &[]);
    none(m, WHOLE, &[(BAR, TAKES[0])]);
    none(m, WHOLE, &[(0, TAKES[0]), (4 * BAR, TAKES[1])]);
    none(m, WHOLE, &[(0, TAKES[0]), (BAR, TAKES[0])]);
    none(m, WHOLE, &[(0, TAKES[2])]);
    let span = TickSpan {
        start: 0,
        end: 4 * BAR,
    };
    let before = steps(m);
    cmd(
        m,
        Command::SetComp {
            clip: 999,
            span,
            pieces: pieces(&THREE),
        },
    );
    cmd(m, Command::ClearComp { clip: 999 });
    assert_eq!(steps(m), before);

    // While counting in or recording nothing changes.
    set(m, WHOLE, &THREE);
    let with = (doc(m), steps(m));
    cmd(m, Command::Record);
    assert!(m.play().busy());
    set(m, WHOLE, &[(0, TAKES[1])]);
    cmd(m, Command::ClearComp { clip: CLIP });
    assert_eq!((doc(m), steps(m)), with);
}

#[test]
fn takes_cuts_and_lengths_with_a_comp() {
    let m = &mut song();
    set(m, WHOLE, &THREE);
    cmd(m, Command::SelectClip { clip: Some(CLIP) });

    // 「テイクを消す」 and 「このテイク以外を消す」 do nothing; neither do 「逆にする」 and
    // 「ノーマライズ」.
    let before = (doc(m), steps(m));
    cmd(m, Command::DeleteTake { clip: CLIP });
    cmd(m, Command::DeleteOtherTakes { clip: CLIP });
    let asked = cmd(m, Command::ReverseClips { clips: vec![CLIP] });
    assert!(asked.is_empty(), "{asked:?}");
    let asked = cmd(m, Command::NormalizeClips { clips: vec![CLIP] });
    assert!(asked.is_empty(), "{asked:?}");
    assert_eq!((doc(m), steps(m)), before);

    // 「音程を寄せる」 is not for a clip that plays several takes.
    let tune_usable = |m: &Model| match m.view().params.selected {
        Some(SelectedParams::Clip(ref c)) => c.tune_usable,
        ref other => panic!("{other:?}"),
    };
    assert!(!tune_usable(m));
    let change = StretchChange::TuneStrength { percent: 50.0 };
    cmd(m, Command::SetClipStretch { clip: CLIP, change });
    assert_eq!((doc(m), steps(m)), before);

    // S at bar 2, inside the second piece: each half has its pieces, from its own start.
    cmd(m, Command::Locate { tick: 2 * BAR });
    cmd(m, Command::SplitClips);
    assert_eq!(
        comps(m),
        [
            (0, 2 * BAR, vec![(0, TAKES[0]), (BAR, TAKES[1])]),
            (2 * BAR, 4 * BAR, vec![(0, TAKES[1]), (BAR, TAKES[2])]),
        ]
    );
    cmd(m, Command::Undo);
    assert_eq!(comps(m), [(0, 4 * BAR, THREE.to_vec())]);

    // The length typed shorter: the pieces stay inside the clip.
    cmd(m, Command::SelectClip { clip: Some(CLIP) });
    let before = steps(m);
    cmd(
        m,
        Command::TrimClip {
            clip: CLIP,
            edge: meloet_protocol::ClipEdge::End,
            ticks: -(2 * BAR as i64),
        },
    );
    assert_eq!(steps(m), before + 1);
    assert_eq!(
        comps(m),
        [(0, 2 * BAR, vec![(0, TAKES[0]), (BAR, TAKES[1])])]
    );
    cmd(m, Command::Undo);

    // T: the next take whole, and the comp is gone, in one step.
    let before = steps(m);
    cmd(m, Command::StepTake { delta: 1 });
    assert_eq!((steps(m), last_step(m)), (before + 1, StepName::SwitchTake));
    assert_eq!(comp(m), []);
    assert_eq!(m.view().song.tracks[1].clips[0].take_id, TAKES[0]);
    assert!(tune_usable(m));
    cmd(m, Command::Undo);
    assert_eq!(comp(m), THREE);
    assert_eq!(m.view().song.tracks[1].clips[0].take_id, TAKES[2]);
}

#[test]
fn sections_cut_the_comp() {
    let m = &mut song();
    set(m, WHOLE, &THREE);
    let mark = |m: &mut Model, tick: u64, name: &str| {
        cmd(
            m,
            Command::AddMarker {
                name: name.into(),
                tick: Some(tick),
            },
        );
        let id = m.view().timeline.renaming.unwrap();
        cmd(m, Command::CloseMarkerName);
        id
    };
    let a = mark(m, 2 * BAR, "A");
    mark(m, 3 * BAR, "B");

    // A bar of space at bar 2, in the middle of the second piece.
    cmd(m, Command::InsertSpace { marker: a, bars: 1 });
    assert_eq!(
        comps(m),
        [
            (0, 2 * BAR, vec![(0, TAKES[0]), (BAR, TAKES[1])]),
            (3 * BAR, 5 * BAR, vec![(0, TAKES[1]), (BAR, TAKES[2])]),
        ]
    );
    cmd(m, Command::Undo);
    assert_eq!(comps(m), [(0, 4 * BAR, THREE.to_vec())]);

    // The section of bar 2 goes: the rest closes up, each part with its pieces.
    cmd(m, Command::DeleteSection { marker: a });
    assert_eq!(
        comps(m),
        [
            (0, 2 * BAR, vec![(0, TAKES[0]), (BAR, TAKES[1])]),
            (2 * BAR, 3 * BAR, vec![(0, TAKES[2])]),
        ]
    );
    cmd(m, Command::Undo);

    // The section is played twice: the copy has the pieces of the section.
    cmd(m, Command::DuplicateSection { marker: a });
    assert_eq!(
        comps(m),
        [
            (0, 2 * BAR, vec![(0, TAKES[0]), (BAR, TAKES[1])]),
            (2 * BAR, 3 * BAR, vec![(0, TAKES[1])]),
            (3 * BAR, 4 * BAR, vec![(0, TAKES[1])]),
            (4 * BAR, 5 * BAR, vec![(0, TAKES[2])]),
        ]
    );
}

#[test]
fn copies_of_the_clip_and_of_the_track_have_the_comp() {
    let m = &mut song();
    set(m, WHOLE, &THREE);
    cmd(
        m,
        Command::SelectClips {
            clips: vec![CLIP],
            mode: SelectMode::Replace,
        },
    );
    cmd(
        m,
        Command::RepeatClips {
            clips: vec![CLIP],
            count: 1,
        },
    );
    assert_eq!(
        comps(m),
        [
            (0, 4 * BAR, THREE.to_vec()),
            (4 * BAR, 8 * BAR, THREE.to_vec())
        ]
    );
    cmd(m, Command::DuplicateTrack { track: 3 });
    let copy = &m.view().song.tracks[2];
    let comp: Vec<_> = copy.clips[0]
        .comp
        .iter()
        .map(|p| (p.from, p.take))
        .collect();
    assert_eq!(comp, THREE);
}

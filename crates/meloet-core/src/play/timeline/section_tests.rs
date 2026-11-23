use meloet_model::MAX_TICK;
use meloet_protocol::{
    Command, SectionUse, SelectMode, StatusMessage, StepName, StepWhat, TimelineRef,
};

use super::super::tests::{cmd, ready, record_once, started, status};
use super::super::{EngineOp, StepKind};
use crate::state::{Effect, Model};

const Q: u64 = meloet_model::TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

fn steps(m: &Model) -> u32 {
    m.view().song.undo_count
}

fn markers(m: &Model) -> Vec<(u64, String, SectionUse)> {
    (m.view().timeline.markers.iter())
        .map(|k| (k.tick, k.name.clone(), k.section))
        .collect()
}

fn spans(m: &Model) -> Vec<(u64, u64)> {
    (m.view().song.tracks[0].clips.iter())
        .map(|c| (c.start, c.end))
        .collect()
}

fn mark(m: &mut Model, tick: u64, name: &str) -> u32 {
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
}

/// A clip over bars 0 to 3 with a note in each bar, markers at bars 0 and 2, the play line
/// at bar 1.
fn song() -> (Model, u32, u32) {
    let mut m = ready();
    let notes: Vec<(u8, f64, f64)> = (0..4)
        .map(|i| (60 + i as u8, (i * BAR) as f64, (i * BAR + Q) as f64))
        .collect();
    record_once(&mut m, &notes, (4 * BAR) as f64);
    m.set_now(200, None);
    let a = mark(&mut m, 0, "イントロ");
    let b = mark(&mut m, 2 * BAR, "A メロ");
    cmd(&mut m, Command::Locate { tick: BAR });
    (m, a, b)
}

fn sequences(effects: &[Effect]) -> Vec<Vec<u64>> {
    effects
        .iter()
        .filter_map(|e| match e {
            Effect::Engine(EngineOp::Sequence { events, .. }) => {
                Some(events.iter().map(|e| e.tick).collect())
            }
            _ => None,
        })
        .collect()
}

fn timelines(effects: &[Effect]) -> usize {
    effects
        .iter()
        .filter(|e| matches!(e, Effect::Engine(EngineOp::Timeline { .. })))
        .count()
}

#[test]
fn a_section_is_copied_right_after_it_as_one_step() {
    let (mut m, a, b) = song();
    assert_eq!(spans(&m), [(0, 4 * BAR)]);
    let clip = m.view().song.tracks[0].clips[0].id;
    cmd(
        &mut m,
        Command::SelectClips {
            clips: vec![clip],
            mode: SelectMode::Replace,
        },
    );
    cmd(
        &mut m,
        Command::SelectTimeline {
            target: Some(TimelineRef::Marker { id: a }),
        },
    );
    let before = (m.play().song().doc.clone(), steps(&m));
    let o = cmd(&mut m, Command::DuplicateSection { marker: a });
    assert_eq!(steps(&m), before.1 + 1);
    assert_eq!(
        spans(&m),
        [(0, 2 * BAR), (2 * BAR, 4 * BAR), (4 * BAR, 6 * BAR)]
    );
    assert_eq!(
        markers(&m),
        [
            (0, "イントロ".into(), SectionUse::Usable),
            (2 * BAR, "イントロ".into(), SectionUse::Usable),
            (4 * BAR, "A メロ".into(), SectionUse::Usable),
        ]
    );
    assert_eq!(
        m.play().steps().last(),
        Some(&StepKind::DuplicateSection {
            marker: meloet_model::MarkerId(a),
            bars: 2
        })
    );
    let row = m.history_rows(1, 100).pop().unwrap();
    assert_eq!(row.name, StepName::DuplicateSection);
    assert_eq!(
        row.what,
        StepWhat::Section {
            name: "イントロ".into(),
            bars: 2
        }
    );
    // The engine has the new notes: bars 0 and 1 twice, then bars 2 and 3.
    let ons: Vec<u64> = [0, BAR, 2 * BAR, 3 * BAR, 4 * BAR, 5 * BAR].to_vec();
    let sent = sequences(&o);
    assert_eq!(sent.len(), 1);
    for tick in ons {
        assert!(sent[0].contains(&tick), "{tick}");
    }
    assert_eq!(timelines(&o), 0, "the tempo and the meter did not change");
    // Nothing is selected, and the play line is where it was.
    let v = m.view();
    assert!(v.arrange.selected_clips.is_empty());
    assert_eq!(v.arrange.selected_clip, None);
    assert_eq!(v.timeline.selected, None);
    assert_eq!(v.transport.position, BAR);
    assert_ne!(status(&m), Some(StatusMessage::SongTooLong));

    // Undo: the song and the selection as they were. Redo: the copy again, nothing selected.
    let after = m.play().song().doc.clone();
    let o = cmd(&mut m, Command::Undo);
    assert_eq!(sequences(&o).len(), 1);
    let doc = m.play().song().doc.clone();
    assert_eq!(
        meloet_model::Doc {
            next_id: before.0.next_id,
            ..doc
        },
        before.0
    );
    let v = m.view();
    assert_eq!(v.arrange.selected_clips, [clip]);
    assert_eq!(v.timeline.selected, Some(TimelineRef::Marker { id: a }));
    cmd(&mut m, Command::Redo);
    assert_eq!(m.play().song().doc, after);
    assert!(m.view().arrange.selected_clips.is_empty());
    assert_eq!(m.view().timeline.selected, None);
    let _ = b;
}

#[test]
fn a_section_goes_and_what_follows_comes_up() {
    let (mut m, a, b) = song();
    let before = m.play().song().doc.clone();
    cmd(&mut m, Command::DeleteSection { marker: a });
    assert_eq!(spans(&m), [(0, 2 * BAR)]);
    assert_eq!(markers(&m), [(0, "A メロ".into(), SectionUse::Usable)]);
    assert_eq!(m.view().transport.position, BAR);
    let row = m.history_rows(1, 100).pop().unwrap();
    assert_eq!(row.name, StepName::DeleteSection);
    // The step is named by the marker it took away.
    assert_eq!(
        row.what,
        StepWhat::Section {
            name: "イントロ".into(),
            bars: 2
        }
    );
    // The marker is gone: its id does nothing any more.
    let now = (m.play().song().doc.clone(), steps(&m));
    for command in [
        Command::DeleteSection { marker: a },
        Command::DuplicateSection { marker: a },
        Command::InsertSpace { marker: a, bars: 4 },
    ] {
        let o = cmd(&mut m, command);
        assert!(o.iter().all(|e| !matches!(e, Effect::Engine(_))));
        assert_eq!((m.play().song().doc.clone(), steps(&m)), now);
    }
    cmd(&mut m, Command::Undo);
    let doc = m.play().song().doc.clone();
    assert_eq!(
        meloet_model::Doc {
            next_id: before.next_id,
            ..doc
        },
        before
    );
    // The last section, up to the song's end.
    cmd(&mut m, Command::DeleteSection { marker: b });
    assert_eq!(spans(&m), [(0, 2 * BAR)]);
    assert_eq!(markers(&m), [(0, "イントロ".into(), SectionUse::Usable)]);
}

#[test]
fn empty_bars_go_in_before_a_marker() {
    let (mut m, _, b) = song();
    cmd(
        &mut m,
        Command::AddTempoPoint {
            tick: 3 * BAR,
            bpm: 140.0,
        },
    );
    cmd(
        &mut m,
        Command::SetLoop {
            from_bar: 2,
            to_bar: 4,
        },
    );
    let o = cmd(&mut m, Command::InsertSpace { marker: b, bars: 4 });
    assert_eq!(spans(&m), [(0, 2 * BAR), (6 * BAR, 8 * BAR)]);
    assert_eq!(
        markers(&m),
        [
            (0, "イントロ".into(), SectionUse::Usable),
            (6 * BAR, "A メロ".into(), SectionUse::Usable),
        ]
    );
    let v = m.view();
    assert_eq!(v.timeline.tempo_points[0].tick, 7 * BAR);
    assert_eq!(
        v.song.loop_range.map(|l| (l.start, l.end)),
        Some((BAR, 7 * BAR))
    );
    // The engine is told the new tempo map, the new loop and the new notes.
    assert_eq!(timelines(&o), 1);
    assert_eq!(sequences(&o).len(), 1);
    assert!(o.iter().any(|e| matches!(
        e,
        Effect::Engine(EngineOp::Loop { start, end, .. }) if (*start, *end) == (BAR, 7 * BAR)
    )));
    let row = m.history_rows(1, 100).pop().unwrap();
    assert_eq!(row.name, StepName::InsertSpace);
    assert_eq!(
        row.what,
        StepWhat::Section {
            name: "A メロ".into(),
            bars: 4
        }
    );
    // Only 1 to 64 bars.
    let now = steps(&m);
    for bars in [0, 65, u32::MAX] {
        cmd(&mut m, Command::InsertSpace { marker: b, bars });
        assert_eq!(steps(&m), now);
    }
    cmd(
        &mut m,
        Command::InsertSpace {
            marker: b,
            bars: 64,
        },
    );
    assert_eq!(steps(&m), now + 1);
    assert_eq!(markers(&m)[1].0, 70 * BAR);
}

#[test]
fn a_section_that_can_not_be_used_does_nothing() {
    let (mut m, a, b) = song();
    // A marker inside a bar: the section before it and its own end off the bars.
    let off = mark(&mut m, 3 * BAR + Q, "拍");
    // The last marker at the song's end has nothing after it.
    let end = mark(&mut m, 4 * BAR, "おわり");
    assert_eq!(
        markers(&m).iter().map(|k| k.2).collect::<Vec<_>>(),
        [
            SectionUse::Usable,
            SectionUse::NotOnBars,
            SectionUse::NotOnBars,
            SectionUse::Empty
        ]
    );
    let now = (m.play().song().doc.clone(), steps(&m));
    for marker in [b, off, end] {
        for command in [
            Command::DuplicateSection { marker },
            Command::DeleteSection { marker },
            Command::InsertSpace { marker, bars: 4 },
        ] {
            cmd(&mut m, command);
            assert_eq!((m.play().song().doc.clone(), steps(&m)), now);
            assert_ne!(status(&m), Some(StatusMessage::SongTooLong));
        }
    }
    cmd(&mut m, Command::DuplicateSection { marker: a });
    assert_eq!(steps(&m), now.1 + 1);
}

#[test]
fn nothing_while_counting_in_or_recording() {
    let (mut m, a, _) = song();
    let now = (m.play().song().doc.clone(), steps(&m));
    let commands = |marker: u32| {
        [
            Command::DuplicateSection { marker },
            Command::DeleteSection { marker },
            Command::InsertSpace { marker, bars: 4 },
        ]
    };
    cmd(&mut m, Command::SetCountIn { bars: 1 });
    cmd(&mut m, Command::Record);
    for command in commands(a) {
        cmd(&mut m, command);
        assert_eq!((m.play().song().doc.clone(), steps(&m)), now);
    }
    started(&mut m);
    for command in commands(a) {
        cmd(&mut m, command);
        assert_eq!((m.play().song().doc.clone(), steps(&m)), now);
    }
}

#[test]
fn a_song_that_would_get_too_long_says_so_and_stays() {
    let (mut m, _, _) = song();
    let far = (MAX_TICK / BAR - 1) * BAR;
    let last = mark(&mut m, far, "遠く");
    mark(&mut m, far + BAR, "果て");
    assert_eq!(markers(&m)[2].2, SectionUse::Usable);
    let now = (m.play().song().doc.clone(), steps(&m));
    for command in [
        Command::InsertSpace {
            marker: last,
            bars: 1,
        },
        Command::DuplicateSection { marker: last },
    ] {
        cmd(&mut m, Command::SelectTimeline { target: None });
        let o = cmd(&mut m, command);
        assert_eq!(status(&m), Some(StatusMessage::SongTooLong));
        assert!(o.iter().all(|e| !matches!(e, Effect::Engine(_))));
        assert_eq!((m.play().song().doc.clone(), steps(&m)), now);
    }
    cmd(&mut m, Command::DeleteSection { marker: last });
    assert_eq!(steps(&m), now.1 + 1);
    assert_ne!(status(&m), Some(StatusMessage::SongTooLong));
}

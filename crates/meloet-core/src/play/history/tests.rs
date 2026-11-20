use std::collections::BTreeSet;

use meloet_model::effects::EffectKind;
use meloet_model::{ClipEdge, InsertId, MarkerId};
use meloet_protocol::{Command, SendBus as ProtocolBus, StripRef};

use super::*;
use crate::play::tests::{cmd, ready};

/// One of every kind of step. A kind added to `StepKind` and not to the list does not build.
fn every_step_kind() -> Vec<StepKind> {
    use StepKind::*;
    let (track, clip, marker) = (TrackId(1), ClipId(9), MarkerId(9));
    let strip = StripId::Track(track);
    let place = PluginPlace::Instrument(track);
    let target = ModelTarget::Insert {
        insert: InsertId(9),
        param: 0,
    };
    let tempo = meloet_model::Tempo::from_centi(12_000).unwrap();
    let meter = meloet_model::Meter::FOUR_FOUR;
    let kind = EffectKind::Compressor;
    let range = TickRange::new(0, 1920).unwrap();
    let count = 3;
    let all = vec![
        Record {
            track,
            clip,
            takes: 1,
        },
        SwitchTake {
            clip,
            from: 0,
            to: 1,
        },
        DeleteTake { clip, index: 0 },
        DeleteClips { count, cut: false },
        DeleteClips { count, cut: true },
        Tempo {
            from: tempo,
            to: tempo,
        },
        Meter {
            from: meter,
            to: meter,
        },
        AddTempoPoint { tick: 1920, tempo },
        MoveTempoPoint {
            from: 1920,
            to: 3840,
            tempo,
        },
        SetTempoPoint {
            tick: 1920,
            from: tempo,
            to: tempo,
        },
        SetTempoRamp {
            tick: 1920,
            on: true,
        },
        SetTempoRamp {
            tick: 1920,
            on: false,
        },
        DeleteTempoPoint { tick: 1920 },
        AddMeterChange { bar: 4, meter },
        MoveMeterChange { from: 4, to: 8 },
        SetMeterChange {
            bar: 4,
            from: meter,
            to: meter,
        },
        DeleteMeterChange { bar: 4 },
        AddMarker { marker },
        RenameMarker { marker },
        MoveMarker { marker },
        DeleteMarker { marker },
        SetLoop { range },
        Mute { track, muted: true },
        Mute {
            track,
            muted: false,
        },
        RenameTrack { track },
        SetTrackColor { track },
        DeleteTrack { track },
        MoveTrack { track },
        AddTrack { track },
        SetInstrument { track },
        RemoveMissing {
            tracks: 1,
            inserts: 1,
        },
        WriteNote {
            clip,
            clip_made: false,
        },
        MoveNotes {
            clip,
            count,
            copy: false,
        },
        MoveNotes {
            clip,
            count,
            copy: true,
        },
        ResizeNotes { clip, count },
        DeleteNotes {
            clip,
            count,
            cut: false,
        },
        DeleteNotes {
            clip,
            count,
            cut: true,
        },
        ChangeVelocity { clip, count },
        PasteNotes { clip, count },
        Quantize { clip, count },
        Unquantize { clip, count },
        SetNotesField {
            clip,
            count,
            field: NoteField::Key,
        },
        AddControl { clip },
        MoveControls { clip, count },
        DeleteControls { clip, count },
        MoveClips { count, copy: false },
        MoveClips { count, copy: true },
        DuplicateClips { count },
        PasteClips { count },
        TrimClip {
            clip,
            edge: ClipEdge::End,
        },
        SetGain { strip },
        SetPan { strip },
        SetSend { track, bus: Bus::A },
        SetParam {
            strip,
            slot: 0,
            kind,
            param: 0,
        },
        AddEffect {
            strip,
            slot: 0,
            kind,
        },
        RemoveEffect {
            strip,
            slot: 0,
            kind,
        },
        AddPlugin { place },
        RemovePlugin { place },
        Bypass {
            strip,
            slot: 0,
            bypassed: true,
        },
        Bypass {
            strip,
            slot: 0,
            bypassed: false,
        },
        MoveInsert {
            strip,
            from: 0,
            to: 1,
        },
        ReplaceEffect { strip, slot: 0 },
        PluginWindow { place },
        SetPluginParam { place, param: 0 },
        MuteReturn {
            bus: Bus::A,
            muted: true,
        },
        RemoveInstrument { track },
        AddLane { track, target },
        DeleteLane { track, target },
        AddAutoPoint { track, target },
        MoveAutoPoints {
            track,
            target,
            count,
        },
        DeleteAutoPoints {
            track,
            target,
            count,
        },
        WriteAutomation { track, target },
        RestLane { track, target },
        RestoreLane { track, target },
        Recovered,
        DiscardRecovered,
    ];
    for kind in &all {
        match kind {
            Record { .. }
            | SwitchTake { .. }
            | DeleteTake { .. }
            | DeleteClips { .. }
            | Tempo { .. }
            | Meter { .. }
            | AddTempoPoint { .. }
            | MoveTempoPoint { .. }
            | SetTempoPoint { .. }
            | SetTempoRamp { .. }
            | DeleteTempoPoint { .. }
            | AddMeterChange { .. }
            | MoveMeterChange { .. }
            | SetMeterChange { .. }
            | DeleteMeterChange { .. }
            | AddMarker { .. }
            | RenameMarker { .. }
            | MoveMarker { .. }
            | DeleteMarker { .. }
            | SetLoop { .. }
            | Mute { .. }
            | RenameTrack { .. }
            | SetTrackColor { .. }
            | DeleteTrack { .. }
            | MoveTrack { .. }
            | AddTrack { .. }
            | SetInstrument { .. }
            | RemoveMissing { .. }
            | WriteNote { .. }
            | MoveNotes { .. }
            | ResizeNotes { .. }
            | DeleteNotes { .. }
            | ChangeVelocity { .. }
            | PasteNotes { .. }
            | Quantize { .. }
            | Unquantize { .. }
            | SetNotesField { .. }
            | AddControl { .. }
            | MoveControls { .. }
            | DeleteControls { .. }
            | MoveClips { .. }
            | DuplicateClips { .. }
            | PasteClips { .. }
            | TrimClip { .. }
            | SetGain { .. }
            | SetPan { .. }
            | SetSend { .. }
            | SetParam { .. }
            | AddEffect { .. }
            | RemoveEffect { .. }
            | AddPlugin { .. }
            | RemovePlugin { .. }
            | Bypass { .. }
            | MoveInsert { .. }
            | ReplaceEffect { .. }
            | PluginWindow { .. }
            | SetPluginParam { .. }
            | MuteReturn { .. }
            | RemoveInstrument { .. }
            | AddLane { .. }
            | DeleteLane { .. }
            | AddAutoPoint { .. }
            | MoveAutoPoints { .. }
            | DeleteAutoPoints { .. }
            | WriteAutomation { .. }
            | RestLane { .. }
            | RestoreLane { .. }
            | Recovered
            | DiscardRecovered => {}
        }
    }
    all
}

fn kind_of(name: &StepName) -> String {
    serde_json::to_value(name).unwrap()["kind"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// A step with no name in the history band fails here (and a name no step has, too): the
/// screen's side of it is `ui/tests/bridge.protocol21.test.ts`.
#[test]
fn every_step_has_a_name_and_every_name_a_step() {
    let doc = Doc::new_default();
    let named: BTreeSet<String> = every_step_kind()
        .into_iter()
        .map(|kind| {
            let (name, _, _) = step_text(&StepRow {
                kind,
                number: 1,
                before: &doc,
                after: &doc,
            });
            kind_of(&name)
        })
        .collect();
    let names: BTreeSet<String> = StepName::every_kind().iter().map(kind_of).collect();
    assert_eq!(named, names);
}

#[test]
fn the_history_section_counts_and_the_rows_say_what_each_step_did() {
    let mut m = ready();
    assert_eq!(m.view().history, HistoryView::default());
    assert!(m.history_rows(1, 100).is_empty());
    assert_eq!(m.lane(1), None);

    cmd(&mut m, Command::SetTempo { bpm: 120.0 });
    cmd(
        &mut m,
        Command::SetGain {
            strip: StripRef::Track { id: 1 },
            db: Some(-12.0),
            commit: true,
        },
    );
    cmd(
        &mut m,
        Command::SetGain {
            strip: StripRef::Return {
                bus: ProtocolBus::A,
            },
            db: None,
            commit: true,
        },
    );
    let h = m.view().history;
    assert_eq!((h.total, h.redo, h.current), (3, 0, 3));
    let rows = m.history_rows(1, 100);
    assert_eq!(rows.len(), 3);
    assert_eq!(h.newest, rows[2].step);
    assert_eq!(rows.iter().map(|r| r.index).collect::<Vec<_>>(), [1, 2, 3]);
    let track = TrackRef {
        number: 1,
        name: "ピアノ".into(),
    };
    assert_eq!(
        (&rows[0].name, &rows[0].what, &rows[0].place),
        (
            &StepName::SetTempo,
            &StepWhat::Tempo {
                bar: 1,
                beat: 1,
                from_bpm: Some(96.0),
                to_bpm: Some(120.0)
            },
            &Some(StepPlace::Arrange)
        )
    );
    assert_eq!(
        (&rows[1].name, &rows[1].what, &rows[1].place),
        (
            &StepName::SetGain,
            &StepWhat::Gain {
                strip: StripName::Track { track },
                from_db: Some(0.0),
                to_db: Some(-12.0)
            },
            &Some(StepPlace::Mixer)
        )
    );
    assert_eq!(
        rows[2].what,
        StepWhat::Gain {
            strip: StripName::Return { bus: SendBus::A },
            from_db: Some(0.0),
            to_db: None
        }
    );

    // Undo moves 「いまここ」 only: the rows and their numbers stay.
    cmd(&mut m, Command::Undo);
    cmd(&mut m, Command::Undo);
    let undone = m.view().history;
    assert_eq!((undone.total, undone.redo, undone.current), (3, 2, 1));
    assert_eq!(undone.newest, h.newest);
    assert_eq!(m.history_rows(1, 100), rows);
    assert_eq!(m.history_rows(2, 2), rows[1..2]);
    assert_eq!(m.history_rows(0, 1), rows[..1], "rows count from 1");
    assert!(m.history_rows(4, 9).is_empty());

    // A new step takes the place of the undone ones.
    cmd(&mut m, Command::SetMeter { num: 3, den: 4 });
    let now = m.view().history;
    assert_eq!((now.total, now.redo, now.current), (2, 0, 2));
    let new = m.history_rows(1, 100);
    assert_eq!(new[0], rows[0]);
    assert_eq!(new[1].name, StepName::SetMeter);
    assert_eq!(new[1].step, now.newest);
    assert!(rows.iter().all(|r| r.step != new[1].step));

    // The jump does nothing yet; a lane is a step.
    let before = m.view();
    cmd(
        &mut m,
        Command::HistoryJump {
            index: 1,
            step: new[0].step,
        },
    );
    assert_eq!(
        (&m.view().song, &m.view().history),
        (&before.song, &before.history)
    );
    cmd(
        &mut m,
        Command::AddLane {
            track: 1,
            target: meloet_protocol::AutoTarget::Volume,
        },
    );
    let after = m.view();
    assert_eq!(after.history.total, before.history.total + 1);
    assert_eq!(after.song.tracks[0].lanes.len(), 1);
    assert_eq!(after.automation, meloet_protocol::AutomationView::default());
}

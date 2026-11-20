//! The history band: the counts of the section `history`, and the rows the screen fetches
//! (docs/architecture.md 21-12, 21-15). A row's name and values are made from the step and
//! the `Doc` before and after it, when the row is asked for.

use std::collections::HashMap;

use meloet_model::{
    AutoTarget as ModelTarget, Bus, ClipId, Doc, InstrumentRef, NoteField, PluginPlace, StepKind,
    StepRow, StripId, TempoMap, TickRange, TrackId,
};
use meloet_protocol::{
    BarSpan, EffectName, HistoryRow, HistoryView, LaneName, MeterValue, SendBus, StatusMessage,
    StepName, StepPlace, StepWhat, StripName, TrackRef,
};

use super::Play;
use super::mixer::{effect_kind, effect_name, gain_view};
use crate::added::sound_name;
use crate::state::Effect;

/// What a row says that neither the step nor the `Doc`s around it hold, by the number of the
/// step.
#[derive(Debug, Default)]
pub(super) struct StepNotes {
    /// The name of the parameter of a `StepKind::SetPluginParam`, as the plugin said it
    /// when the step was made.
    param_names: HashMap<u64, String>,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn shown(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn send_bus(bus: Bus) -> SendBus {
    match bus {
        Bus::A => SendBus::A,
        Bus::B => SendBus::B,
    }
}

fn meter_value(meter: meloet_model::Meter) -> MeterValue {
    MeterValue {
        num: meter.num(),
        den: meter.den(),
    }
}

/// The track as `doc` has it: its number is its place there, from 1.
fn track_ref(doc: &Doc, id: TrackId) -> Option<TrackRef> {
    let index = doc.tracks.iter().position(|t| t.id == id)?;
    Some(TrackRef {
        number: index as u32 + 1,
        name: doc.tracks[index].name.clone(),
    })
}

/// What a row is made from. A thing the step made is looked for in the `Doc` after it first,
/// one it took away is only in the `Doc` before.
struct Around<'a> {
    before: &'a Doc,
    after: &'a Doc,
}

impl Around<'_> {
    fn track(&self, id: TrackId) -> TrackRef {
        track_ref(self.after, id)
            .or_else(|| track_ref(self.before, id))
            .unwrap_or_else(|| TrackRef {
                number: 0,
                name: String::new(),
            })
    }

    fn clip_track(&self, clip: ClipId) -> Option<StepPlace> {
        let (track, _) = self.after.clip(clip).or_else(|| self.before.clip(clip))?;
        Some(StepPlace::Track {
            track: self.track(track.id),
        })
    }

    fn strip(&self, strip: StripId) -> StripName {
        match strip {
            StripId::Track(id) => StripName::Track {
                track: self.track(id),
            },
            StripId::Return(bus) => StripName::Return { bus: send_bus(bus) },
            StripId::Master => StripName::Master,
        }
    }

    /// Where a step on an insert of `strip` was made: the track, or the mixer for a return
    /// and the master (mock R10-History: 「エフェクトをとばす」 at "4 パッド").
    fn strip_place(&self, strip: StripId) -> Option<StepPlace> {
        Some(match strip {
            StripId::Track(id) => StepPlace::Track {
                track: self.track(id),
            },
            StripId::Return(_) | StripId::Master => StepPlace::Mixer,
        })
    }

    fn plugin_strip(&self, place: PluginPlace) -> StripId {
        match place {
            PluginPlace::Instrument(track) => StripId::Track(track),
            PluginPlace::Insert(strip, _) => strip,
        }
    }

    fn plugin_name(&self, place: PluginPlace) -> String {
        (self.after.plugin(place))
            .or_else(|| self.before.plugin(place))
            .map(|p| p.name.to_string())
            .unwrap_or_default()
    }

    fn effect(doc: &Doc, strip: StripId, slot: usize) -> Option<EffectName> {
        doc.insert(strip, slot).map(effect_name)
    }

    fn effect_what(doc: &Doc, strip: StripId, slot: usize) -> StepWhat {
        match Self::effect(doc, strip, slot) {
            Some(effect) => StepWhat::Effect { effect },
            None => StepWhat::None,
        }
    }

    fn lane_name(doc: &Doc, track: TrackId, target: ModelTarget) -> Option<LaneName> {
        let t = doc.track(track)?;
        Some(match target {
            ModelTarget::Volume => LaneName::Volume,
            ModelTarget::Pan => LaneName::Pan,
            ModelTarget::Send(bus) => LaneName::Send { bus: send_bus(bus) },
            ModelTarget::Mute => LaneName::Mute,
            ModelTarget::Instrument { .. } => match &t.instrument {
                InstrumentRef::Plugin(p) => LaneName::Plugin {
                    plugin: p.name.to_string(),
                    param: None,
                },
                _ => return None,
            },
            ModelTarget::Insert { insert, param } => {
                let insert = (t.channel.inserts.iter().flatten()).find(|i| i.id == insert)?;
                match (insert.kind(), insert.plugin_ref()) {
                    (Some(kind), _) => LaneName::Builtin {
                        effect: effect_kind(kind),
                        param,
                    },
                    (None, Some(p)) => LaneName::Plugin {
                        plugin: p.name.to_string(),
                        param: None,
                    },
                    (None, None) => return None,
                }
            }
        })
    }

    fn lane(&self, track: TrackId, target: ModelTarget, points: Option<usize>) -> StepWhat {
        let name = Self::lane_name(self.after, track, target)
            .or_else(|| Self::lane_name(self.before, track, target));
        match name {
            Some(target) => StepWhat::Lane {
                target,
                points: points.map(count),
            },
            None => StepWhat::None,
        }
    }

    fn tempo_at(doc: &Doc, tick: u64) -> Option<f64> {
        let t = &doc.timeline;
        match tick {
            0 => Some(t.tempo.bpm()),
            _ => (t.tempo_points.iter().find(|p| p.tick == tick)).map(|p| p.tempo.bpm()),
        }
    }

    fn meter_at(doc: &Doc, bar: u32) -> Option<MeterValue> {
        let t = &doc.timeline;
        match bar {
            0 => Some(meter_value(t.meter)),
            _ => (t.meter_changes.iter().find(|c| c.bar == bar)).map(|c| meter_value(c.meter)),
        }
    }

    /// The bar and the beat of `tick`, as shown.
    fn place(doc: &Doc, tick: u64) -> (u32, u32) {
        let at = TempoMap::new(&doc.timeline).bar_beat_tick(tick);
        (shown(at.bar), at.beat)
    }

    fn bars(doc: &Doc, range: TickRange) -> BarSpan {
        let map = TempoMap::new(&doc.timeline);
        let bar = |tick: u64| shown(map.meter_at(tick).bar + 1);
        BarSpan {
            from: bar(range.start()),
            to: bar(map.bar_at_or_after(range.end())),
        }
    }
}

/// The name, the values and the place of a step. Every kind of step has a name: a kind added
/// to `StepKind` does not build until it has one here.
pub(crate) fn step_text(row: &StepRow) -> (StepName, StepWhat, Option<StepPlace>) {
    let a = Around {
        before: row.before,
        after: row.after,
    };
    let notes = |n: usize| StepWhat::Notes { count: count(n) };
    let clips = |n: usize| StepWhat::Clips { count: count(n) };
    let marker_name = |doc: &Doc, id| doc.marker(id).map(|m| m.name.clone());
    let named = |name: Option<String>| match name {
        Some(name) => StepWhat::Name { name },
        None => StepWhat::None,
    };
    let piano_roll = Some(StepPlace::PianoRoll);
    let arrange = Some(StepPlace::Arrange);
    let mixer = Some(StepPlace::Mixer);
    let on_track = |track: TrackId| {
        Some(StepPlace::Track {
            track: a.track(track),
        })
    };
    match row.kind {
        StepKind::Record { track, clip, .. } => (
            StepName::Record,
            match a.after.clip(clip) {
                Some((_, c)) => StepWhat::Take {
                    number: count(c.takes.len()),
                },
                None => StepWhat::None,
            },
            on_track(track),
        ),
        StepKind::SwitchTake { clip, from, to } => (
            StepName::SwitchTake,
            StepWhat::TakeSwitch {
                from: count(from + 1),
                to: count(to + 1),
            },
            a.clip_track(clip),
        ),
        StepKind::DeleteTake { clip, index } => (
            StepName::DeleteTake,
            StepWhat::Take {
                number: count(index + 1),
            },
            a.clip_track(clip),
        ),
        StepKind::DeleteClips { count: n, cut } => (
            if cut {
                StepName::CutClips
            } else {
                StepName::DeleteClips
            },
            clips(n),
            arrange,
        ),
        StepKind::Tempo { from, to } => (
            StepName::SetTempo,
            StepWhat::Tempo {
                bar: 1,
                beat: 1,
                from_bpm: Some(from.bpm()),
                to_bpm: Some(to.bpm()),
            },
            arrange,
        ),
        StepKind::Meter { from, to } => (
            StepName::SetMeter,
            StepWhat::Meter {
                bar: 1,
                from: Some(meter_value(from)),
                to: Some(meter_value(to)),
            },
            arrange,
        ),
        StepKind::AddTempoPoint { tick, tempo } => {
            let (bar, beat) = Around::place(a.after, tick);
            (
                StepName::AddTimelineChange,
                StepWhat::Tempo {
                    bar,
                    beat,
                    from_bpm: None,
                    to_bpm: Some(tempo.bpm()),
                },
                arrange,
            )
        }
        // A drag that only changed the value reads as a value typed.
        StepKind::MoveTempoPoint { from, to, tempo } if from == to => {
            let (bar, beat) = Around::place(a.after, to);
            (
                StepName::SetTempo,
                StepWhat::Tempo {
                    bar,
                    beat,
                    from_bpm: Around::tempo_at(a.before, from),
                    to_bpm: Some(tempo.bpm()),
                },
                arrange,
            )
        }
        StepKind::MoveTempoPoint { from, to, .. } => {
            let (bar, beat) = Around::place(a.before, from);
            let (to_bar, to_beat) = Around::place(a.after, to);
            (
                StepName::MoveTimelineChange,
                StepWhat::Moved {
                    bar,
                    beat,
                    to_bar,
                    to_beat,
                },
                arrange,
            )
        }
        StepKind::SetTempoPoint { tick, from, to } => {
            let (bar, beat) = Around::place(a.after, tick);
            (
                StepName::SetTempo,
                StepWhat::Tempo {
                    bar,
                    beat,
                    from_bpm: Some(from.bpm()),
                    to_bpm: Some(to.bpm()),
                },
                arrange,
            )
        }
        StepKind::SetTempoRamp { tick, on } => {
            let (bar, beat) = Around::place(a.after, tick);
            (
                if on {
                    StepName::SetTempoRamp
                } else {
                    StepName::ClearTempoRamp
                },
                StepWhat::Place { bar, beat },
                arrange,
            )
        }
        StepKind::DeleteTempoPoint { tick } => {
            let (bar, beat) = Around::place(a.before, tick);
            (
                StepName::DeleteTimelineChange,
                StepWhat::Tempo {
                    bar,
                    beat,
                    from_bpm: Around::tempo_at(a.before, tick),
                    to_bpm: None,
                },
                arrange,
            )
        }
        StepKind::AddMeterChange { bar, meter } => (
            StepName::AddTimelineChange,
            StepWhat::Meter {
                bar: bar.saturating_add(1),
                from: None,
                to: Some(meter_value(meter)),
            },
            arrange,
        ),
        StepKind::MoveMeterChange { from, to } => (
            StepName::MoveTimelineChange,
            StepWhat::Moved {
                bar: from.saturating_add(1),
                beat: 1,
                to_bar: to.saturating_add(1),
                to_beat: 1,
            },
            arrange,
        ),
        StepKind::SetMeterChange { bar, from, to } => (
            StepName::SetMeter,
            StepWhat::Meter {
                bar: bar.saturating_add(1),
                from: Some(meter_value(from)),
                to: Some(meter_value(to)),
            },
            arrange,
        ),
        StepKind::DeleteMeterChange { bar } => (
            StepName::DeleteTimelineChange,
            StepWhat::Meter {
                bar: bar.saturating_add(1),
                from: Around::meter_at(a.before, bar),
                to: None,
            },
            arrange,
        ),
        StepKind::AddMarker { marker } => (
            StepName::AddMarker,
            named(marker_name(a.after, marker)),
            arrange,
        ),
        StepKind::RenameMarker { marker } => (
            StepName::RenameMarker,
            match (marker_name(a.before, marker), marker_name(a.after, marker)) {
                (Some(from), Some(to)) => StepWhat::Renamed { from, to },
                (_, to) => named(to),
            },
            arrange,
        ),
        StepKind::MoveMarker { marker } => (
            StepName::MoveMarker,
            named(marker_name(a.after, marker)),
            arrange,
        ),
        StepKind::DeleteMarker { marker } => (
            StepName::DeleteMarker,
            named(marker_name(a.before, marker)),
            arrange,
        ),
        StepKind::SetLoop { range } => (
            StepName::SetLoop,
            StepWhat::Bars {
                span: Around::bars(a.after, range),
            },
            arrange,
        ),
        StepKind::Mute { track, muted } => (
            if muted {
                StepName::Mute
            } else {
                StepName::Unmute
            },
            StepWhat::Strip {
                strip: a.strip(StripId::Track(track)),
            },
            mixer,
        ),
        StepKind::RenameTrack { track } => (
            StepName::RenameTrack,
            match (a.before.track(track), a.after.track(track)) {
                (Some(from), Some(to)) => StepWhat::Renamed {
                    from: from.name.clone(),
                    to: to.name.clone(),
                },
                _ => StepWhat::None,
            },
            arrange,
        ),
        StepKind::SetTrackColor { track } => (
            StepName::SetTrackColor,
            StepWhat::Track {
                track: a.track(track),
            },
            arrange,
        ),
        StepKind::DeleteTrack { track } => (
            StepName::DeleteTrack,
            StepWhat::Track {
                track: a.track(track),
            },
            arrange,
        ),
        StepKind::MoveTrack { track } => (
            StepName::MoveTrack,
            StepWhat::Track {
                track: a.track(track),
            },
            arrange,
        ),
        StepKind::AddTrack { track } => (
            StepName::AddTrack,
            StepWhat::Track {
                track: a.track(track),
            },
            arrange,
        ),
        StepKind::SetInstrument { track } => (
            StepName::SetInstrument,
            match a.after.track(track) {
                Some(t) => StepWhat::Sound {
                    sound: sound_name(&t.instrument),
                },
                None => StepWhat::None,
            },
            on_track(track),
        ),
        StepKind::RemoveInstrument { track } => (
            StepName::RemoveInstrument,
            match a.before.track(track) {
                Some(t) => StepWhat::Sound {
                    sound: sound_name(&t.instrument),
                },
                None => StepWhat::None,
            },
            on_track(track),
        ),
        StepKind::RemoveMissing { tracks, inserts } => (
            StepName::RemoveMissing,
            StepWhat::Missing { tracks, inserts },
            None,
        ),
        StepKind::WriteNote { .. } => (StepName::WriteNote, notes(1), piano_roll),
        StepKind::MoveNotes { count: n, copy, .. } => (
            if copy {
                StepName::DuplicateNotes
            } else {
                StepName::MoveNotes
            },
            notes(n),
            piano_roll,
        ),
        StepKind::ResizeNotes { count: n, .. } => (StepName::ResizeNotes, notes(n), piano_roll),
        StepKind::DeleteNotes { count: n, cut, .. } => (
            if cut {
                StepName::CutNotes
            } else {
                StepName::DeleteNotes
            },
            notes(n),
            piano_roll,
        ),
        StepKind::ChangeVelocity { count: n, .. } => {
            (StepName::ChangeVelocity, notes(n), piano_roll)
        }
        StepKind::PasteNotes { count: n, .. } => (StepName::PasteNotes, notes(n), piano_roll),
        StepKind::Quantize { count: n, .. } => (StepName::Quantize, notes(n), piano_roll),
        StepKind::Unquantize { count: n, .. } => (StepName::Unquantize, notes(n), piano_roll),
        // A value typed into a field is named as the drag that changes the same thing.
        StepKind::SetNotesField {
            count: n, field, ..
        } => (
            match field {
                NoteField::Start | NoteField::Key => StepName::MoveNotes,
                NoteField::Length => StepName::ResizeNotes,
                NoteField::Velocity => StepName::ChangeVelocity,
            },
            notes(n),
            piano_roll,
        ),
        StepKind::AddControl { .. } => (
            StepName::AddControl,
            StepWhat::Points { count: 1 },
            piano_roll,
        ),
        StepKind::MoveControls { count: n, .. } => (
            StepName::MoveControls,
            StepWhat::Points { count: count(n) },
            piano_roll,
        ),
        StepKind::DeleteControls { count: n, .. } => (
            StepName::DeleteControls,
            StepWhat::Points { count: count(n) },
            piano_roll,
        ),
        StepKind::MoveClips { count: n, copy } => (
            if copy {
                StepName::DuplicateClips
            } else {
                StepName::MoveClips
            },
            clips(n),
            arrange,
        ),
        StepKind::DuplicateClips { count: n } => (StepName::DuplicateClips, clips(n), arrange),
        StepKind::PasteClips { count: n } => (StepName::PasteClips, clips(n), arrange),
        StepKind::TrimClip { clip, .. } => (
            StepName::TrimClip,
            named(a.after.clip(clip).map(|(_, c)| c.name.clone())),
            a.clip_track(clip),
        ),
        StepKind::SetGain { strip } => {
            let gain = |doc: &Doc| doc.channel(strip).and_then(|c| gain_view(c.gain));
            (
                StepName::SetGain,
                StepWhat::Gain {
                    strip: a.strip(strip),
                    from_db: gain(a.before),
                    to_db: gain(a.after),
                },
                mixer,
            )
        }
        StepKind::SetPan { strip } => {
            let pan = |doc: &Doc| doc.channel(strip).map_or(0, |c| c.pan.get());
            (
                StepName::SetPan,
                StepWhat::Pan {
                    strip: a.strip(strip),
                    from: pan(a.before),
                    to: pan(a.after),
                },
                mixer,
            )
        }
        StepKind::SetSend { track, bus } => {
            let send = |doc: &Doc| (doc.track(track)).and_then(|t| gain_view(t.sends[bus.index()]));
            (
                StepName::SetSend,
                StepWhat::Send {
                    track: a.track(track),
                    bus: send_bus(bus),
                    from_db: send(a.before),
                    to_db: send(a.after),
                },
                mixer,
            )
        }
        StepKind::SetParam {
            strip,
            slot,
            kind,
            param,
        } => {
            let value = |doc: &Doc| doc.insert(strip, slot).and_then(|i| i.value(param));
            (
                StepName::SetParam {
                    effect: effect_kind(kind),
                    param,
                },
                match (value(a.before), value(a.after)) {
                    (Some(from), Some(to)) => StepWhat::Param { from, to },
                    _ => StepWhat::None,
                },
                a.strip_place(strip),
            )
        }
        StepKind::AddEffect { strip, kind, .. } => (
            StepName::AddEffect,
            StepWhat::Effect {
                effect: EffectName::Builtin {
                    effect: effect_kind(kind),
                },
            },
            a.strip_place(strip),
        ),
        StepKind::RemoveEffect { strip, kind, .. } => (
            StepName::RemoveEffect,
            StepWhat::Effect {
                effect: EffectName::Builtin {
                    effect: effect_kind(kind),
                },
            },
            a.strip_place(strip),
        ),
        StepKind::AddPlugin { place } => (
            StepName::AddEffect,
            StepWhat::Effect {
                effect: EffectName::Plugin {
                    name: a.plugin_name(place),
                },
            },
            a.strip_place(a.plugin_strip(place)),
        ),
        StepKind::RemovePlugin { place } => (
            StepName::RemoveEffect,
            StepWhat::Effect {
                effect: EffectName::Plugin {
                    name: a.plugin_name(place),
                },
            },
            a.strip_place(a.plugin_strip(place)),
        ),
        StepKind::Bypass {
            strip,
            slot,
            bypassed,
        } => (
            if bypassed {
                StepName::BypassEffect
            } else {
                StepName::RestoreEffect
            },
            Around::effect_what(a.after, strip, slot),
            a.strip_place(strip),
        ),
        StepKind::MoveInsert { strip, to, .. } => (
            StepName::MoveInsert,
            Around::effect_what(a.after, strip, to),
            a.strip_place(strip),
        ),
        StepKind::ReplaceEffect { strip, slot } => (
            StepName::ReplaceEffect,
            match (
                Around::effect(a.before, strip, slot),
                Around::effect(a.after, strip, slot),
            ) {
                (Some(from), Some(to)) => StepWhat::Replaced { from, to },
                _ => StepWhat::None,
            },
            a.strip_place(strip),
        ),
        StepKind::PluginWindow { place } => (
            StepName::PluginWindow {
                plugin: a.plugin_name(place),
            },
            StepWhat::None,
            Some(StepPlace::PluginWindow {
                strip: a.strip(a.plugin_strip(place)),
            }),
        ),
        StepKind::SetPluginParam { place, .. } => (
            StepName::SetPluginParam {
                plugin: a.plugin_name(place),
                param: None,
            },
            StepWhat::None,
            a.strip_place(a.plugin_strip(place)),
        ),
        StepKind::MuteReturn { bus, muted } => (
            if muted {
                StepName::Mute
            } else {
                StepName::Unmute
            },
            StepWhat::Strip {
                strip: a.strip(StripId::Return(bus)),
            },
            mixer,
        ),
        StepKind::AddLane { track, target } => (
            StepName::AddLane,
            a.lane(track, target, None),
            on_track(track),
        ),
        StepKind::DeleteLane { track, target } => (
            StepName::DeleteLane,
            a.lane(track, target, None),
            on_track(track),
        ),
        StepKind::AddAutoPoint { track, target } => (
            StepName::AddAutoPoint,
            a.lane(track, target, None),
            on_track(track),
        ),
        StepKind::MoveAutoPoints {
            track,
            target,
            count: n,
        } => (
            StepName::MoveAutoPoints,
            a.lane(track, target, Some(n)),
            on_track(track),
        ),
        StepKind::DeleteAutoPoints {
            track,
            target,
            count: n,
        } => (
            StepName::DeleteAutoPoints,
            a.lane(track, target, Some(n)),
            on_track(track),
        ),
        StepKind::WriteAutomation { track, target } => (
            StepName::WriteAutomation,
            a.lane(track, target, None),
            on_track(track),
        ),
        StepKind::RestLane { track, target } => (
            StepName::RestLane,
            a.lane(track, target, None),
            on_track(track),
        ),
        StepKind::RestoreLane { track, target } => (
            StepName::RestoreLane,
            a.lane(track, target, None),
            on_track(track),
        ),
        StepKind::Recovered => (StepName::Recovered, StepWhat::None, None),
        StepKind::DiscardRecovered => (StepName::DiscardRecovered, StepWhat::None, None),
    }
}

impl Play {
    pub fn history_view(&self) -> HistoryView {
        let (undone, redo) = (self.history.undo_count(), self.history.redo_count());
        let total = undone + redo;
        let newest = total
            .checked_sub(1)
            .and_then(|last| self.history.row(self.doc(), last));
        HistoryView {
            total: count(total),
            redo: count(redo),
            current: count(undone),
            newest: newest.map_or(0, |row| row.number),
        }
    }

    /// The rows `from ..= to` of the history band that there are, oldest first; rows count
    /// from 1.
    pub fn history_rows(&self, from: u32, to: u32) -> Vec<HistoryRow> {
        (from.max(1)..=to)
            .map_while(|index| self.history_row(index as usize))
            .collect()
    }

    /// Row `index` of the band, from 1. The status line says of a step what its row says.
    fn history_row(&self, index: usize) -> Option<HistoryRow> {
        let row = self.history.row(self.doc(), index.checked_sub(1)?)?;
        let (mut name, what, place) = step_text(&row);
        if let StepName::SetPluginParam { param, .. } = &mut name {
            *param = self.step_notes.param_names.get(&row.number).cloned();
        }
        Some(HistoryRow {
            index: count(index),
            step: row.number,
            name,
            what,
            place,
        })
    }

    /// The newest step is one of the plugin's parameter `param`, which the plugin calls
    /// `name`.
    pub(super) fn note_param_step(&mut self, name: String) {
        let last = self.history.undo_count().checked_sub(1);
        let row = last.and_then(|last| self.history.row(self.doc(), last));
        if let Some(row) = row
            && matches!(row.kind, StepKind::SetPluginParam { .. })
        {
            self.step_notes.param_names.insert(row.number, name);
        }
    }

    /// Before a new step: the steps that could be redone go, and what was noted of them.
    pub(super) fn forget_undone_steps(&mut self) {
        if self.history.redo_count() > 0 {
            // A step made later has a higher number than any made before it.
            let now = self.history.version();
            self.step_notes.param_names.retain(|&step, _| step <= now);
        }
    }

    /// Mod Z. Returns what the status line says.
    pub fn undo(&mut self, effects: &mut Vec<Effect>) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let done = self.history.undo_count();
        let Some(row) = self.history_row(done) else {
            return Some(StatusMessage::NothingToUndo);
        };
        self.travel(done - 1, effects);
        Some(StatusMessage::Undone {
            name: row.name,
            what: row.what,
        })
    }

    /// Mod Shift Z.
    pub fn redo(&mut self, effects: &mut Vec<Effect>) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let done = self.history.undo_count();
        let Some(row) = self.history_row(done + 1) else {
            return Some(StatusMessage::NothingToRedo);
        };
        self.travel(done + 1, effects);
        Some(StatusMessage::Redone {
            name: row.name,
            what: row.what,
        })
    }

    /// `Command::HistoryJump`: row `index` becomes the last step done. Nothing when the row
    /// is not the step `step` any more, or is the last step done already.
    pub fn history_jump(
        &mut self,
        index: u32,
        step: u64,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let done = self.history.undo_count();
        let to = index as usize;
        let row = self.history_row(to).filter(|row| row.step == step)?;
        if to == done {
            return None;
        }
        let steps = self.travel(to, effects);
        Some(StatusMessage::HistoryJumped {
            index,
            name: row.name,
            steps: count(steps),
            forward: to > done,
        })
    }

    /// Undoes or redoes until `to` steps are done, and then tells everything else once of
    /// the song as it is now: the engine, the plugins, what is selected. Returns how many
    /// steps that were.
    fn travel(&mut self, to: usize, effects: &mut Vec<Effect>) -> usize {
        let done = self.history.undo_count();
        let before = self.doc().clone();
        self.tl.forget_fresh();
        let crossed = (self.history.steps())
            .skip(done.min(to))
            .take(done.abs_diff(to))
            .any(|kind| matches!(kind, StepKind::Recovered | StepKind::DiscardRecovered));
        let steps = self.history.jump(&mut self.song.doc, to);
        if steps == 0 {
            return 0;
        }
        if crossed {
            // Over "なおすためのファイルから戻す" or "戻さない": between the recovered song and
            // the saved one, whose takes may have gone their own ways (a file changed from
            // outside). The screen lets go of its kept takes then too.
            self.serial += 1;
        }
        self.song.doc.name.clone_from(&before.name);
        self.after_change(&before, effects);
        // What was selected before the oldest step undone, or after the newest one redone.
        let sel = if to < done {
            let kept = self.sel_undo.len().saturating_sub(steps);
            let undone = self.sel_undo.drain(kept..).rev();
            self.sel_redo.extend(undone);
            self.sel_redo.last().map(|(before, _)| before.clone())
        } else {
            let kept = self.sel_redo.len().saturating_sub(steps);
            let redone = self.sel_redo.drain(kept..).rev();
            self.sel_undo.extend(redone);
            self.sel_undo.last().map(|(_, after)| after.clone())
        };
        if let Some(sel) = sel {
            self.put_sel(sel, effects);
        }
        steps
    }
}

#[cfg(test)]
mod jump_tests;
#[cfg(test)]
mod tests;

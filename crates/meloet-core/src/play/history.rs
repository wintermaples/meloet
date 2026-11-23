//! The history band: the counts of the section `history`, and the rows the screen fetches
//! (docs/architecture.md 21-12, 21-15). A row's name and values are made from the step and
//! the `Doc` before and after it, when the row is asked for.

use std::collections::HashMap;

use meloet_model::{
    AutoTarget as ModelTarget, Bus, ClipId, ControlValue, Doc, InstrumentRef, LaneHome, NoteField,
    PluginPlace, StepKind, StepRow, StripId, TempoMap, TickRange, TrackId,
};
use meloet_protocol::{
    BarSpan, ControlKind, EffectName, HistoryRow, HistoryView, LaneName, MeterValue, SendBus,
    StatusMessage, StepName, StepPlace, StepWhat, StripName, TrackRef,
};

use super::Play;
use super::mixer::{effect_kind, effect_name, gain_view};
use crate::added::sound_name;
use crate::state::Effect;

/// What a row says that neither the step nor the `Doc`s around it hold, by the number of the
/// step.
#[derive(Debug, Clone, Default)]
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

    /// A2: the section of `marker`, named by the marker as it was before the step (or after
    /// it, for one the step made).
    fn section(&self, marker: meloet_model::MarkerId, bars: u32) -> StepWhat {
        let name = (self.before.marker(marker)).or_else(|| self.after.marker(marker));
        match name {
            Some(m) => StepWhat::Section {
                name: m.name.clone(),
                bars,
            },
            None => StepWhat::None,
        }
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

    fn lane_name(doc: &Doc, home: LaneHome, target: ModelTarget) -> Option<LaneName> {
        let LaneHome::Track(track) = home else {
            return (target == ModelTarget::Volume).then_some(LaneName::Volume);
        };
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

    fn lane(&self, home: LaneHome, target: ModelTarget, points: Option<usize>) -> StepWhat {
        let name = Self::lane_name(self.after, home, target)
            .or_else(|| Self::lane_name(self.before, home, target));
        match name {
            Some(target) => StepWhat::Lane {
                target,
                points: points.map(count),
            },
            None => StepWhat::None,
        }
    }

    /// The shown keys take of `clip` in `doc`: its notes as (song position, length, key,
    /// velocity) and its control points as (song position, which control). Empty when the
    /// clip is not there.
    fn take_of(doc: &Doc, clip: ClipId) -> (Vec<NoteAt>, Vec<(i64, ControlKind)>) {
        let Some((_, c)) = doc.clip(clip).filter(|(_, c)| c.is_keys()) else {
            return Default::default();
        };
        let (zero, take) = (c.active_zero(), &c.active_take().take);
        let notes = (take.notes.iter())
            .map(|n| (zero + n.start as i64, n.length, n.key, n.velocity))
            .collect();
        let controls = (take.controls.iter())
            .map(|p| {
                let kind = match p.value {
                    ControlValue::Cc { number, .. } => ControlKind::Cc { number },
                    ControlValue::PitchBend(_) => ControlKind::PitchBend,
                };
                (zero + p.tick as i64, kind)
            })
            .collect();
        (notes, controls)
    }

    /// 「（何を）」 of a step on `n` notes of `clip`: the note's name for one note that the
    /// step put in, took away or changed ("C4"), else how many ("3 ノート"; spec 3, 92-3).
    fn notes_what(&self, clip: ClipId, n: usize) -> StepWhat {
        let plain = StepWhat::Notes { count: count(n) };
        if n != 1 {
            return plain;
        }
        let (before, _) = Self::take_of(self.before, clip);
        let (after, _) = Self::take_of(self.after, clip);
        let only = |a: &[NoteAt], b: &[NoteAt]| match only_in(a, b)[..] {
            [note] => Some(note.2),
            _ => None,
        };
        match only(&after, &before).or_else(|| only(&before, &after)) {
            Some(key) => StepWhat::Note { key },
            None => plain,
        }
    }

    /// "3 ノート ・ 96 → 110" when the notes that changed all had one velocity and have one
    /// now; `None` for both when they differ.
    fn velocity_what(&self, clip: ClipId, n: usize) -> StepWhat {
        let (before, _) = Self::take_of(self.before, clip);
        let (after, _) = Self::take_of(self.after, clip);
        let one = |notes: Vec<NoteAt>| match notes.split_first() {
            Some((first, rest)) if rest.iter().all(|n| n.3 == first.3) => Some(first.3),
            _ => None,
        };
        let (from, to) = (one(only_in(&before, &after)), one(only_in(&after, &before)));
        let both = from.is_some() && to.is_some();
        StepWhat::Velocity {
            count: count(n),
            from: from.filter(|_| both),
            to: to.filter(|_| both),
        }
    }

    /// "サステイン ・ 4 点": the control whose points the step put in, took away or moved.
    fn controls_what(&self, clip: ClipId, n: usize) -> StepWhat {
        let (_, before) = Self::take_of(self.before, clip);
        let (_, after) = Self::take_of(self.after, clip);
        let mut touched = only_in(&after, &before);
        touched.extend(only_in(&before, &after));
        let control = match touched.split_first() {
            Some((first, rest)) if rest.iter().all(|p| p.1 == first.1) => Some(first.1),
            _ => None,
        };
        StepWhat::ControlPoints {
            control,
            count: count(n),
        }
    }

    /// 「（何を）」 of a step on `n` clips: the clip's name for one clip ("Aメロ"), else how
    /// many ("3 クリップ").
    fn clips_what(&self, n: usize) -> StepWhat {
        let plain = StepWhat::Clips { count: count(n) };
        if n != 1 {
            return plain;
        }
        // Where each clip is: its track and its range.
        let placed = |doc: &Doc| -> Vec<(ClipId, TrackId, u64, u64)> {
            (doc.tracks.iter())
                .flat_map(|t| t.clips.iter().map(|c| (c.id, t.id, c.start, c.end)))
                .collect()
        };
        let (before, after) = (placed(self.before), placed(self.after));
        let name = |doc: &Doc, id: ClipId| doc.clip(id).map(|(_, c)| c.name.clone());
        let touched = match (&only_in(&after, &before)[..], &only_in(&before, &after)[..]) {
            // Put in or moved: the clip as it is after the step.
            ([one], _) => name(self.after, one.0),
            ([], [one]) => name(self.before, one.0),
            _ => None,
        };
        match touched {
            Some(name) => StepWhat::Name { name },
            None => plain,
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

/// A note of a take: its song position, length, key and velocity.
type NoteAt = (i64, u64, u8, u8);

/// What `a` has more of than `b`, each as many times as it has it more.
fn only_in<T: Copy + Eq + std::hash::Hash>(a: &[T], b: &[T]) -> Vec<T> {
    let mut left: HashMap<T, usize> = HashMap::new();
    for item in b {
        *left.entry(*item).or_default() += 1;
    }
    (a.iter())
        .filter(|item| match left.get_mut(item) {
            Some(n) if *n > 0 => {
                *n -= 1;
                false
            }
            _ => true,
        })
        .copied()
        .collect()
}

/// The name, the values and the place of a step. Every kind of step has a name: a kind added
/// to `StepKind` does not build until it has one here.
pub(crate) fn step_text(row: &StepRow) -> (StepName, StepWhat, Option<StepPlace>) {
    let a = Around {
        before: row.before,
        after: row.after,
    };
    let notes = |clip: ClipId, n: usize| a.notes_what(clip, n);
    let clips = |n: usize| a.clips_what(n);
    let marker_name = |doc: &Doc, id| doc.marker(id).map(|m| m.name.clone());
    let set_name = |doc: &Doc, index: usize| doc.mute_sets.get(index).map(|s| s.name.clone());
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
    let on_home = |home: LaneHome| match home {
        LaneHome::Track(track) => on_track(track),
        LaneHome::Master => Some(StepPlace::Master),
    };
    match row.kind {
        // Round 12, wave 2.
        StepKind::KeepPlayed { track, clip, .. } => (
            StepName::KeepPlayed,
            match a.after.clip(clip) {
                Some((_, c)) => StepWhat::Take {
                    number: count(c.active + 1),
                },
                None => StepWhat::None,
            },
            on_track(track),
        ),
        StepKind::PlacePattern { track, .. } => (
            StepName::PlacePattern,
            StepWhat::Track {
                track: a.track(track),
            },
            arrange,
        ),
        StepKind::RepeatClips { count: n } => (StepName::RepeatClips, clips(n), arrange),
        StepKind::ApplyPreset { strip, kind, .. } => (
            StepName::ApplyPreset,
            StepWhat::Effect {
                effect: EffectName::Builtin {
                    effect: effect_kind(kind),
                },
            },
            a.strip_place(strip),
        ),
        StepKind::LowerAll { .. } => (StepName::LowerAll, StepWhat::None, mixer),
        StepKind::Record { track, clip, .. } => (
            StepName::Record,
            match a.after.clip(clip) {
                Some((_, c)) => StepWhat::Take {
                    number: count(c.active + 1),
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
            match a.after.track(track) {
                Some(t) => StepWhat::Color {
                    track: a.track(track),
                    color: t.color.get(),
                },
                None => StepWhat::Track {
                    track: a.track(track),
                },
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
            match (a.before.track(track), a.after.track(track)) {
                (Some(from), Some(to)) => StepWhat::SoundReplaced {
                    track: a.track(track),
                    from: sound_name(&from.instrument),
                    to: sound_name(&to.instrument),
                },
                (None, Some(t)) => StepWhat::Sound {
                    sound: sound_name(&t.instrument),
                },
                _ => StepWhat::None,
            },
            on_track(track),
        ),
        StepKind::SetVocalInput { track } => {
            let input = |doc: &Doc| doc.track(track).and_then(|t| t.kind.input());
            (
                StepName::SetVocalInput,
                match (input(a.before), input(a.after)) {
                    (Some(from), Some(to)) => StepWhat::VocalInput {
                        track: a.track(track),
                        from: crate::state::vocal_input_view(from),
                        to: crate::state::vocal_input_view(to),
                    },
                    _ => StepWhat::None,
                },
                arrange,
            )
        }
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
        StepKind::WriteNote { clip, count: n, .. } => {
            (StepName::WriteNote, notes(clip, n), piano_roll)
        }
        StepKind::MoveNotes {
            clip,
            count: n,
            copy,
        } => (
            if copy {
                StepName::DuplicateNotes
            } else {
                StepName::MoveNotes
            },
            notes(clip, n),
            piano_roll,
        ),
        StepKind::ResizeNotes { clip, count: n } => {
            (StepName::ResizeNotes, notes(clip, n), piano_roll)
        }
        StepKind::DeleteNotes {
            clip,
            count: n,
            cut,
        } => (
            if cut {
                StepName::CutNotes
            } else {
                StepName::DeleteNotes
            },
            notes(clip, n),
            piano_roll,
        ),
        StepKind::ChangeVelocity { clip, count: n } => (
            StepName::ChangeVelocity,
            a.velocity_what(clip, n),
            piano_roll,
        ),
        StepKind::PasteNotes { clip, count: n } => {
            (StepName::PasteNotes, notes(clip, n), piano_roll)
        }
        // "1/16 ・ 12 ノート": the grid is the one the take was quantized with.
        StepKind::Quantize { clip, count: n } => (
            StepName::Quantize,
            match (a.after.clip(clip))
                .filter(|(_, c)| c.is_keys())
                .and_then(|(_, c)| c.active_take().take.origin.as_ref())
            {
                Some(origin) => StepWhat::Quantized {
                    division: origin.settings.grid.division(),
                    triplet: origin.settings.grid.triplet,
                    count: count(n),
                },
                None => StepWhat::Notes { count: count(n) },
            },
            piano_roll,
        ),
        StepKind::Unquantize { clip, count: n } => {
            (StepName::Unquantize, notes(clip, n), piano_roll)
        }
        // A value typed into a field is named as the drag that changes the same thing.
        StepKind::SetNotesField {
            clip,
            count: n,
            field,
        } => match field {
            NoteField::Start | NoteField::Key => (StepName::MoveNotes, notes(clip, n), piano_roll),
            NoteField::Length => (StepName::ResizeNotes, notes(clip, n), piano_roll),
            NoteField::Velocity => (
                StepName::ChangeVelocity,
                a.velocity_what(clip, n),
                piano_roll,
            ),
        },
        StepKind::AddControl { clip } => {
            (StepName::AddControl, a.controls_what(clip, 1), piano_roll)
        }
        StepKind::MoveControls { clip, count: n } => {
            (StepName::MoveControls, a.controls_what(clip, n), piano_roll)
        }
        StepKind::DeleteControls { clip, count: n } => (
            StepName::DeleteControls,
            a.controls_what(clip, n),
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
        StepKind::SplitClips { count: n } => (StepName::SplitClips, clips(n), arrange),
        StepKind::RenameClip { clip } => (
            StepName::RenameClip,
            match (a.before.clip(clip), a.after.clip(clip)) {
                (Some((_, from)), Some((_, to))) => StepWhat::Renamed {
                    from: from.name.clone(),
                    to: to.name.clone(),
                },
                _ => StepWhat::None,
            },
            arrange,
        ),
        StepKind::JoinClips { count: n, .. } => (StepName::JoinClips, clips(n), arrange),
        StepKind::CreateClip { clip } => (
            StepName::WriteClip,
            named(a.after.clip(clip).map(|(_, c)| c.name.clone())),
            arrange,
        ),
        StepKind::ImportAudio { count: n } => (StepName::ImportAudio, clips(n), arrange),
        StepKind::SetClipGain { clip } => {
            let gain = |doc: &Doc| {
                let (_, c) = doc.clip(clip)?;
                Some((c.name.clone(), c.takes.shape()?.gain.db() as f32))
            };
            (
                StepName::SetClipGain,
                match (gain(a.before), gain(a.after)) {
                    (Some((_, from_db)), Some((name, to_db))) => StepWhat::ClipGain {
                        name,
                        from_db,
                        to_db,
                    },
                    _ => StepWhat::None,
                },
                a.clip_track(clip),
            )
        }
        StepKind::SetClipsGain { count: n } => (StepName::SetClipGain, clips(n), arrange),
        StepKind::SetClipsFollow { count: n } => (StepName::SetClipFollow, clips(n), arrange),
        StepKind::SetClipsPitch { count: n } => (StepName::SetClipPitch, clips(n), arrange),
        StepKind::SetClipFade { clip } => (
            StepName::SetClipFade,
            named(a.after.clip(clip).map(|(_, c)| c.name.clone())),
            a.clip_track(clip),
        ),
        StepKind::SetClipFollow { clip } => (
            StepName::SetClipFollow,
            named(a.after.clip(clip).map(|(_, c)| c.name.clone())),
            a.clip_track(clip),
        ),
        StepKind::SetSourceTempo { clip } => (
            StepName::SetSourceTempo,
            named(a.after.clip(clip).map(|(_, c)| c.name.clone())),
            a.clip_track(clip),
        ),
        StepKind::SetClipPitch { clip } => (
            StepName::SetClipPitch,
            named(a.after.clip(clip).map(|(_, c)| c.name.clone())),
            a.clip_track(clip),
        ),
        StepKind::RemoveAudio { audio, .. } => (
            StepName::RemoveMissing,
            named(a.before.audio(audio).map(|s| s.file.clone())),
            arrange,
        ),
        StepKind::ScaleVelocity { clip, count: n } => (
            StepName::ScaleVelocity,
            a.velocity_what(clip, n),
            piano_roll,
        ),
        StepKind::RampVelocity { clip, count: n } => {
            (StepName::RampVelocity, a.velocity_what(clip, n), piano_roll)
        }
        StepKind::RandomVelocity { clip, count: n } => (
            StepName::RandomVelocity,
            a.velocity_what(clip, n),
            piano_roll,
        ),
        StepKind::Legato { clip, count: n } => (StepName::Legato, notes(clip, n), piano_roll),
        StepKind::DuplicateTrack { track } => (
            StepName::DuplicateTrack,
            StepWhat::Track {
                track: a.track(track),
            },
            arrange,
        ),
        StepKind::NormalizeClips { count: n } => (StepName::NormalizeClips, clips(n), arrange),
        StepKind::ReverseClips { count: n } => (StepName::ReverseClips, clips(n), arrange),
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
        StepKind::SaveMuteSet { index } => (
            StepName::SaveMuteSet,
            named(set_name(a.after, index)),
            mixer,
        ),
        StepKind::RecallMuteSet { index } => (
            StepName::RecallMuteSet,
            named(set_name(a.after, index)),
            mixer,
        ),
        StepKind::DeleteMuteSet { index } => (
            StepName::DeleteMuteSet,
            named(set_name(a.before, index)),
            mixer,
        ),
        StepKind::RenameMuteSet { index } => (
            StepName::RenameMuteSet,
            match (set_name(a.before, index), set_name(a.after, index)) {
                (Some(from), Some(to)) => StepWhat::Renamed { from, to },
                (_, to) => named(to),
            },
            mixer,
        ),
        StepKind::Batch { count: steps } => (
            StepName::Batch {
                count: count(steps),
            },
            StepWhat::None,
            None,
        ),
        StepKind::AddLane { home, target } => {
            (StepName::AddLane, a.lane(home, target, None), on_home(home))
        }
        StepKind::DeleteLane { home, target } => (
            StepName::DeleteLane,
            a.lane(home, target, None),
            on_home(home),
        ),
        StepKind::AddAutoPoint { home, target } => (
            StepName::AddAutoPoint,
            a.lane(home, target, None),
            on_home(home),
        ),
        StepKind::MoveAutoPoints {
            home,
            target,
            count: n,
        } => (
            StepName::MoveAutoPoints,
            a.lane(home, target, Some(n)),
            on_home(home),
        ),
        StepKind::DeleteAutoPoints {
            home,
            target,
            count: n,
        } => (
            StepName::DeleteAutoPoints,
            a.lane(home, target, Some(n)),
            on_home(home),
        ),
        StepKind::WriteAutomation { home, target } => (
            StepName::WriteAutomation,
            a.lane(home, target, None),
            on_home(home),
        ),
        StepKind::RestLane { home, target } => (
            StepName::RestLane,
            a.lane(home, target, None),
            on_home(home),
        ),
        StepKind::RestoreLane { home, target } => (
            StepName::RestoreLane,
            a.lane(home, target, None),
            on_home(home),
        ),
        StepKind::Recovered => (StepName::Recovered, StepWhat::None, None),
        StepKind::DiscardRecovered => (StepName::DiscardRecovered, StepWhat::None, None),
        // Round 12, waves 3 and 4.
        StepKind::SetArp { track } => (
            StepName::SetArp,
            StepWhat::Track {
                track: a.track(track),
            },
            on_track(track),
        ),
        StepKind::SetSoundKnob { track, knob } => {
            let value = |doc: &Doc| doc.track(track).map(|t| t.knobs.get(knob));
            (
                StepName::SetSoundKnob,
                match (value(a.before), value(a.after)) {
                    (Some(from), Some(to)) => StepWhat::SoundKnob {
                        track: a.track(track),
                        knob: super::wave3::knob_view(knob),
                        from,
                        to,
                    },
                    _ => StepWhat::Track {
                        track: a.track(track),
                    },
                },
                on_track(track),
            )
        }
        StepKind::DuplicateSection { marker, bars } => {
            (StepName::DuplicateSection, a.section(marker, bars), arrange)
        }
        StepKind::DeleteSection { marker, bars } => {
            (StepName::DeleteSection, a.section(marker, bars), arrange)
        }
        StepKind::InsertSpace { marker, bars } => {
            (StepName::InsertSpace, a.section(marker, bars), arrange)
        }
        StepKind::Bounce { track } => (
            StepName::Bounce,
            StepWhat::Track {
                track: a.track(track),
            },
            arrange,
        ),
        StepKind::AddFadeOut { range } => (
            StepName::AddFadeOut,
            StepWhat::Bars {
                span: Around::bars(a.after, range),
            },
            Some(StepPlace::Master),
        ),
        StepKind::SetComp { clip } => (
            StepName::SetComp,
            named(a.after.clip(clip).map(|(_, c)| c.name.clone())),
            a.clip_track(clip),
        ),
        StepKind::ClearComp { clip } => (
            StepName::ClearComp,
            named(a.after.clip(clip).map(|(_, c)| c.name.clone())),
            a.clip_track(clip),
        ),
        StepKind::SetClipTune { clip } => (
            StepName::SetClipTune,
            named(a.after.clip(clip).map(|(_, c)| c.name.clone())),
            a.clip_track(clip),
        ),
    }
}

/// The lane a step is of.
fn lane_of(kind: &StepKind) -> Option<(LaneHome, ModelTarget)> {
    match *kind {
        StepKind::AddLane { home, target }
        | StepKind::DeleteLane { home, target }
        | StepKind::AddAutoPoint { home, target }
        | StepKind::MoveAutoPoints { home, target, .. }
        | StepKind::DeleteAutoPoints { home, target, .. }
        | StepKind::WriteAutomation { home, target }
        | StepKind::RestLane { home, target }
        | StepKind::RestoreLane { home, target } => Some((home, target)),
        _ => None,
    }
}

/// The key of the plugin a lane moves in `doc`, and the plugin's own number for the
/// parameter.
fn lane_plugin(doc: &Doc, track: TrackId, target: ModelTarget) -> Option<(&str, u32)> {
    let t = doc.track(track)?;
    match target {
        ModelTarget::Instrument { param } => match &t.instrument {
            InstrumentRef::Plugin(p) => Some((&p.key, param)),
            _ => None,
        },
        ModelTarget::Insert { insert, param } => {
            let insert = (t.channel.inserts.iter().flatten()).find(|i| i.id == insert)?;
            Some((&insert.plugin_ref()?.key, param))
        }
        _ => None,
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
        let (mut name, mut what, place) = step_text(&row);
        if let StepName::SetPluginParam { param, .. } = &mut name {
            *param = self.step_notes.param_names.get(&row.number).cloned();
        }
        // The plugin's name for the parameter is in neither `Doc`.
        if let StepWhat::Lane { target, .. } = &mut what
            && let LaneName::Plugin { param, .. } = target
            && let Some((LaneHome::Track(track), target)) = lane_of(&row.kind)
        {
            *param = lane_plugin(row.after, track, target)
                .or_else(|| lane_plugin(row.before, track, target))
                .and_then(|(key, n)| self.auto.names.get(key, n));
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
        self.forget_copy();
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
#[cfg(test)]
mod what_tests;

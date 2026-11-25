//! Tempo points, meter changes and markers: what is selected, the name field, and the
//! commands. Each change to the song is one step, and none happens while counting in or
//! recording. Tempo points are named by their tick and meter changes by their bar; a command
//! that names a place with nothing at it does nothing.

use meloet_model::edit::{Edit, EditError};
use meloet_model::sections;
use meloet_model::timeline as edits;
use meloet_model::{Doc, MAX_TICK, MarkerId, Meter, Tempo, TickRange};
use meloet_protocol::{
    INSERT_SPACE_BARS_MAX, MarkerView, MeterChangeView, StatusMessage, TempoPointView, TimelineRef,
    TimelineView,
};

use super::{DEFAULT_LOOP_BARS, Play};
use crate::state::Effect;

/// What the screen is told of the timeline that is not a part of the song.
#[derive(Debug, Default)]
pub(super) struct Tl {
    selected: Option<TimelineRef>,
    /// The marker whose name field is open.
    renaming: Option<MarkerId>,
    /// The marker K just made, while its name field is still open and nothing else has
    /// changed the song: the name typed there goes into the step that made the marker.
    fresh: Option<MarkerId>,
    /// The row "テンポ ・ 拍子の変更" is open.
    open: bool,
}

fn has(doc: &Doc, at: TimelineRef) -> bool {
    let t = &doc.timeline;
    match at {
        TimelineRef::TempoPoint { tick } => {
            tick == 0 || t.tempo_points.iter().any(|p| p.tick == tick)
        }
        TimelineRef::MeterChange { bar } => {
            bar == 0 || t.meter_changes.iter().any(|c| c.bar == bar)
        }
        TimelineRef::Marker { id } => doc.marker(MarkerId(id)).is_some(),
    }
}

impl Tl {
    pub(super) fn selected(&self) -> Option<TimelineRef> {
        self.selected
    }

    pub(super) fn select(&mut self, at: Option<TimelineRef>) {
        self.selected = at;
    }

    /// The song changed by something else than the name field of the marker just made.
    pub(super) fn forget_fresh(&mut self) {
        self.fresh = None;
    }

    /// Lets go of what is not in `doc` any more.
    pub(super) fn drop_gone(&mut self, doc: &Doc) {
        self.selected = self.selected.filter(|&s| has(doc, s));
        if self.renaming.is_some_and(|id| doc.marker(id).is_none()) {
            self.renaming = None;
            self.fresh = None;
        }
    }
}

fn shown(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

impl Play {
    pub fn timeline_view(&self) -> TimelineView {
        let doc = self.doc();
        let t = &doc.timeline;
        TimelineView {
            tempo_bpm: t.tempo.bpm(),
            meter_num: t.meter.num(),
            meter_den: t.meter.den(),
            head_ramp: t.head_ramp,
            tempo_points: (t.tempo_points.iter())
                .map(|p| TempoPointView {
                    tick: p.tick,
                    tempo_bpm: p.tempo.bpm(),
                    ramp: p.ramp,
                })
                .collect(),
            meter_changes: (t.meter_changes.iter())
                .map(|c| MeterChangeView {
                    bar: c.bar,
                    meter_num: c.meter.num(),
                    meter_den: c.meter.den(),
                })
                .collect(),
            markers: (doc.markers.iter().zip(super::wave3::sections(doc)))
                .map(|(m, section)| MarkerView {
                    id: m.id.0,
                    tick: m.tick,
                    name: m.name.clone(),
                    section,
                })
                .collect(),
            selected: self.tl.selected,
            renaming: self.tl.renaming.map(|m| m.0),
            open: self.tl.open,
        }
    }

    pub fn set_timeline_open(&mut self, open: bool) {
        self.tl.open = open;
    }

    /// What the status line says of the selected tempo point or meter change.
    fn timeline_status(&self, at: TimelineRef) -> Option<StatusMessage> {
        let t = &self.doc().timeline;
        match at {
            TimelineRef::TempoPoint { tick } => {
                let (tempo, ramp, next) = if tick == 0 {
                    (t.tempo, t.head_ramp, t.tempo_points.first())
                } else {
                    let i = t.tempo_points.iter().position(|p| p.tick == tick)?;
                    let p = t.tempo_points[i];
                    (p.tempo, p.ramp, t.tempo_points.get(i + 1))
                };
                let map = self.map();
                let at = map.bar_beat_tick(tick);
                // A ramp between two points of one tempo is no ramp to tell of.
                Some(match next.filter(|n| ramp && n.tempo != tempo) {
                    Some(next) => {
                        let to = map.bar_beat_tick(next.tick);
                        StatusMessage::TempoRampSelected {
                            bar: shown(at.bar),
                            beat: at.beat,
                            tempo_bpm: tempo.bpm(),
                            to_bar: shown(to.bar),
                            to_beat: to.beat,
                            to_tempo_bpm: next.tempo.bpm(),
                        }
                    }
                    None => StatusMessage::TempoPointSelected {
                        bar: shown(at.bar),
                        beat: at.beat,
                        tempo_bpm: tempo.bpm(),
                    },
                })
            }
            TimelineRef::MeterChange { bar } => {
                let meter = match bar {
                    0 => t.meter,
                    _ => t.meter_changes.iter().find(|c| c.bar == bar)?.meter,
                };
                Some(StatusMessage::MeterChangeSelected {
                    bar: bar.saturating_add(1),
                    meter_num: meter.num(),
                    meter_den: meter.den(),
                })
            }
            TimelineRef::Marker { .. } => None,
        }
    }

    /// Selects `at`, as a part of the step just made when there is one: undo and redo bring
    /// the selection back with it.
    fn select_in_step(&mut self, at: TimelineRef, stepped: bool) {
        self.tl.selected = Some(at).filter(|&a| has(self.doc(), a));
        if stepped && let Some((_, after)) = self.sel_undo.last_mut() {
            after.timeline = self.tl.selected;
        }
    }

    /// Makes the step, when there is one to make, and selects `at`. An edit that would leave
    /// the song as it is still selects. Returns what the status line says of the selection.
    fn timeline_step(
        &mut self,
        edited: Result<Edit, EditError>,
        at: TimelineRef,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        match edited {
            Ok(e) => {
                self.apply(e, effects);
                self.select_in_step(at, true);
            }
            Err(EditError::Unchanged) => self.select_in_step(at, false),
            Err(_) => return None,
        }
        self.timeline_status(at)
    }

    /// The tempo typed into the transport: of the point in force where the play line is.
    /// Returns the notice to show.
    pub fn set_tempo(&mut self, bpm: f64, effects: &mut Vec<Effect>) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let tempo = Tempo::from_bpm(bpm)?;
        let line = self.play_line();
        let points = &self.doc().timeline.tempo_points;
        let said = !points.is_empty();
        // Inside a ramp this is the point the ramp starts from.
        let tick = (points.iter().rev())
            .find(|p| p.tick <= line)
            .map_or(0, |p| p.tick);
        let e = edits::set_tempo_at(self.doc(), tick, tempo).ok()?;
        self.apply(e, effects);
        let at = self.map().bar_beat_tick(tick);
        said.then_some(StatusMessage::TempoPointSet {
            bar: shown(at.bar),
            beat: at.beat,
            tempo_bpm: tempo.bpm(),
        })
    }

    /// The meter typed into the transport: of the meter change in force where the play line
    /// is. Returns the notice to show.
    pub fn set_meter(
        &mut self,
        num: u8,
        den: u8,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let meter = Meter::new(num, den)?;
        let line = self.play_line();
        let map = self.map();
        let changes = &self.doc().timeline.meter_changes;
        let said = !changes.is_empty();
        let bar = (changes.iter().rev())
            .find(|c| map.tick_of_bar(u64::from(c.bar)) <= line)
            .map_or(0, |c| c.bar);
        let e = edits::set_meter_at(self.doc(), bar, meter).ok()?;
        self.apply(e, effects);
        said.then_some(StatusMessage::MeterChangeSet {
            bar: bar.saturating_add(1),
            meter_num: num,
            meter_den: den,
        })
    }

    /// The nearest head of a beat, the later one of two as near.
    pub(super) fn snapped(&self, tick: u64) -> u64 {
        self.map().nearest_beat(tick.min(MAX_TICK)).min(MAX_TICK)
    }

    pub fn add_tempo_point(
        &mut self,
        tick: u64,
        bpm: f64,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let tempo = Tempo::from_bpm(bpm)?;
        let tick = self.snapped(tick);
        let edited = edits::add_tempo_point(self.doc(), tick, tempo);
        self.timeline_step(edited, TimelineRef::TempoPoint { tick }, effects)
    }

    pub fn move_tempo_point(
        &mut self,
        tick: u64,
        to_tick: u64,
        bpm: f64,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let tempo = Tempo::from_bpm(bpm)?;
        // A point that stays is not snapped: a meter change put in before it may have left
        // it between two beats, and there it stays.
        let to = if tick == 0 || to_tick == tick {
            tick
        } else {
            match self.snapped(to_tick) {
                // The head is not a place for a point: the first beat after it.
                0 => self.map().next_beat(1.0).tick.max(1) as u64,
                to => to,
            }
        };
        let edited = edits::move_tempo_point(self.doc(), tick, to, tempo);
        self.timeline_step(edited, TimelineRef::TempoPoint { tick: to }, effects)
    }

    pub fn set_tempo_ramp(&mut self, tick: u64, on: bool, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        if let Ok(e) = edits::set_tempo_ramp(self.doc(), tick, on) {
            self.apply(e, effects);
        }
    }

    pub fn add_meter_change(
        &mut self,
        bar: u32,
        num: u8,
        den: u8,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let edited = edits::add_meter_change(self.doc(), bar, Meter::new(num, den)?);
        self.timeline_step(edited, TimelineRef::MeterChange { bar }, effects)
    }

    pub fn move_meter_change(
        &mut self,
        bar: u32,
        to_bar: u32,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() || bar == 0 {
            return None;
        }
        let to = to_bar.max(1);
        let edited = edits::move_meter_change(self.doc(), bar, to);
        self.timeline_step(edited, TimelineRef::MeterChange { bar: to }, effects)
    }

    pub fn set_meter_change(
        &mut self,
        bar: u32,
        num: u8,
        den: u8,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let edited = edits::set_meter_at(self.doc(), bar, Meter::new(num, den)?);
        self.timeline_step(edited, TimelineRef::MeterChange { bar }, effects)
    }

    /// `None`: nothing is selected. A thing that is not there leaves the selection as it is.
    /// Returns the notice to show.
    pub fn select_timeline(&mut self, target: Option<TimelineRef>) -> Option<StatusMessage> {
        let Some(at) = target else {
            self.tl.selected = None;
            return None;
        };
        if !has(self.doc(), at) {
            return None;
        }
        self.tl.selected = Some(at);
        self.timeline_status(at)
    }

    pub fn delete_timeline(&mut self, target: TimelineRef, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let doc = self.doc();
        let edited = match target {
            TimelineRef::TempoPoint { tick } => edits::delete_tempo_point(doc, tick),
            TimelineRef::MeterChange { bar } => edits::delete_meter_change(doc, bar),
            TimelineRef::Marker { id } => edits::delete_marker(doc, MarkerId(id)),
        };
        if let Ok(e) = edited {
            self.apply(e, effects);
        }
    }

    /// K at the play line, or at `tick` (snapped to the beat). Returns the notice to show.
    pub fn add_marker(
        &mut self,
        name: &str,
        tick: Option<u64>,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let tick = self.snapped(tick.unwrap_or_else(|| self.play_line()));
        let (e, id) = edits::add_marker(self.doc(), tick, name).ok()?;
        self.apply(e, effects);
        self.select_in_step(TimelineRef::Marker { id: id.0 }, true);
        self.tl.renaming = Some(id);
        self.tl.fresh = Some(id);
        Some(StatusMessage::MarkerAdded {
            name: self.doc().marker(id)?.name.clone(),
        })
    }

    pub fn open_marker_name(&mut self, id: u32) {
        if self.busy() || self.doc().marker(MarkerId(id)).is_none() {
            return;
        }
        self.tl.selected = Some(TimelineRef::Marker { id });
        self.tl.renaming = Some(MarkerId(id));
        self.tl.fresh = None;
    }

    pub fn close_marker_name(&mut self) {
        self.tl.renaming = None;
        self.tl.fresh = None;
    }

    /// Enter in the name field. For the marker K just made, the name is a part of the step
    /// that made it.
    pub fn rename_marker(&mut self, id: u32, name: &str, effects: &mut Vec<Effect>) {
        let id = MarkerId(id);
        let fresh = self.tl.fresh == Some(id);
        if self.tl.renaming == Some(id) {
            self.close_marker_name();
        }
        if self.busy() {
            return;
        }
        let Ok(e) = edits::rename_marker(self.doc(), id, name) else {
            return;
        };
        if fresh {
            let before = self.doc().clone();
            if self.history.amend(&mut self.song.doc, e.doc.clone()) {
                self.after_change(&before, effects);
                return;
            }
        }
        self.apply(e, effects);
    }

    pub fn move_marker(&mut self, id: u32, tick: u64, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let tick = self.snapped(tick);
        let edited = edits::move_marker(self.doc(), MarkerId(id), tick);
        self.timeline_step(edited, TimelineRef::Marker { id }, effects);
    }

    /// A press on a marker: selects it and goes there.
    pub fn go_to_marker(&mut self, id: u32, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let Some(tick) = self.doc().marker(MarkerId(id)).map(|m| m.tick) else {
            return;
        };
        self.tl.selected = Some(TimelineRef::Marker { id });
        self.locate(tick, effects);
    }

    /// Alt → and Alt ←: to the first marker after the play line, or the last one before it,
    /// which is selected as one pressed is.
    pub fn step_marker(&mut self, forward: bool, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let line = self.play_line();
        let markers = &self.doc().markers;
        let to = if forward {
            markers.iter().find(|m| m.tick > line)
        } else {
            markers.iter().rev().find(|m| m.tick < line)
        };
        if let Some(id) = to.map(|m| m.id.0) {
            self.go_to_marker(id, effects);
        }
    }

    /// "ここから次のマーカーまでをループに": the range is one step, turning the loop on is none.
    pub fn loop_from_marker(&mut self, id: u32, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        let doc = self.doc();
        let Some(start) = doc.marker(MarkerId(id)).map(|m| m.tick) else {
            return;
        };
        let end = (doc.markers.iter().map(|m| m.tick))
            .find(|&tick| tick > start)
            .unwrap_or_else(|| edits::loop_end_from(doc, start, DEFAULT_LOOP_BARS));
        let Some(range) = TickRange::new(start, end) else {
            return;
        };
        self.song.session.loop_on = true;
        self.put_loop(Some(range), effects);
    }

    /// 「この区間をすぐ後ろに複製」. Returns the notice to show.
    pub fn duplicate_section(
        &mut self,
        marker: u32,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let edited = sections::duplicate_section(self.doc(), MarkerId(marker));
        self.section_step(edited, effects)
    }

    /// 「この区間を消して詰める」. Returns the notice to show.
    pub fn delete_section(
        &mut self,
        marker: u32,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let edited = sections::delete_section(self.doc(), MarkerId(marker));
        self.section_step(edited, effects)
    }

    /// 「この前に空きを入れる…」: `bars` bars before the marker. Returns the notice to show.
    pub fn insert_space(
        &mut self,
        marker: u32,
        bars: u32,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() || !(1..=INSERT_SPACE_BARS_MAX).contains(&bars) {
            return None;
        }
        let edited = sections::insert_space(self.doc(), MarkerId(marker), bars);
        self.section_step(edited, effects)
    }

    /// Makes the step of a whole section and lets go of everything selected, as a part of
    /// the step: undo brings the selection back. The play line stays where it is.
    fn section_step(
        &mut self,
        edited: Result<Edit, EditError>,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        match edited {
            Ok(e) => {
                self.apply(e, effects);
                self.clips_sel.clear();
                self.selected = None;
                self.set_selection([], []);
                self.tl.select(None);
                self.put_point_sel(None);
                let now = self.sel();
                if let Some((_, after)) = self.sel_undo.last_mut() {
                    *after = now;
                }
                None
            }
            Err(EditError::OffTheSong) => Some(StatusMessage::SongTooLong),
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod section_tests;
#[cfg(test)]
mod tests;

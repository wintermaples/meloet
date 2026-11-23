//! The shelf's 「パターン」 (round 12, P4): a drum pattern is heard once with ▶ and put in the
//! song with 「ここに置く」 (Enter).

use meloet_model::notes::{self, WriteAt};
use meloet_model::{
    BuiltinInstrument, Edit, EditError, InstrumentGroup, InstrumentRef, MAX_TICK, Meter, StepKind,
    TrackId, edit,
};
use meloet_protocol::{PatternSampleView, PatternView, SamplePlaying, StatusMessage};
use meloet_sound::phrases::Hits;

use super::seats::MAX_TRACKS;
use super::shelf::Mode;
use super::{ClipPick, Play};
use crate::patterns::{self, Pattern};
use crate::state::Effect;

const STUDIO_KIT: InstrumentRef = InstrumentRef::Builtin(BuiltinInstrument::StudioKit);

impl Play {
    /// The selected track when it is a drum track.
    fn drum_target(&self) -> Option<TrackId> {
        let track = self.doc().track(self.song.session.play_target?)?;
        let drums = track.instrument.group() == InstrumentGroup::Drums && !track.kind.is_vocal();
        drums.then_some(track.id)
    }

    /// The rows of 「パターン」: only in the shelf of a new track.
    pub(super) fn pattern_views(&self) -> Vec<PatternView> {
        match self.shelf.mode {
            Mode::NewTrack => patterns::all().iter().map(Pattern::view).collect(),
            _ => Vec::new(),
        }
    }

    pub(super) fn pattern_sample_view(&self) -> Option<PatternSampleView> {
        let playing = self.shelf.sample.as_ref().and_then(|s| {
            let p = s.pattern.as_ref()?;
            Some(PatternSampleView {
                id: p.id.clone(),
                playing: Some(SamplePlaying {
                    started_us: s.started_us,
                    length_us: p.length_us,
                }),
            })
        });
        let waiting = self.shelf.sample_waiting.as_ref().and_then(|w| {
            Some(PatternSampleView {
                id: w.pattern.clone()?,
                playing: None,
            })
        });
        playing.or(waiting)
    }

    /// `ShelfSelectPattern`.
    pub fn select_pattern(&mut self, id: Option<String>) {
        self.shelf.pattern = id
            .filter(|id| patterns::find(id).is_some())
            .filter(|_| self.shelf.mode == Mode::NewTrack);
    }

    /// The notes of `p` in seconds from its head, and its length, at the tempo of where
    /// 「ここに置く」 would put it.
    pub(super) fn pattern_hits(&self, p: &Pattern) -> (Hits, u64) {
        let map = self.map();
        let head = self.bar_head(self.play_line());
        let from = map.seconds_at(head as f64);
        let at = |ticks: u64| map.seconds_at((head + ticks) as f64) - from;
        let hits = (p.notes.iter())
            .map(|h| {
                let start = at(h.at);
                let length = at(h.at + h.length) - start;
                (start, length, h.piece.note(), h.velocity)
            })
            .collect();
        (hits, (at(p.ticks()) * 1e6).round() as u64)
    }

    /// `PlayPattern`: with the kit of the selected track when it is a drum track that the
    /// sample can play (a sound of meloet or an SF2 kit), else with the studio kit. Nothing
    /// without the kit's sound, or without an output.
    pub fn play_pattern(
        &mut self,
        id: &str,
        open: bool,
        latency_us: u64,
        effects: &mut Vec<Effect>,
    ) {
        if !open || self.shelf.mode != Mode::NewTrack || patterns::find(id).is_none() {
            return;
        }
        let available = self.sounds.available();
        let can = |i: &InstrumentRef| match i {
            InstrumentRef::Builtin(b) => available.contains(b),
            InstrumentRef::Sf2(_) => true,
            InstrumentRef::Plugin(_) => false,
        };
        let own = (self.drum_target())
            .and_then(|t| self.doc().track(t))
            .map(|t| t.instrument.clone())
            .filter(can);
        let Some(kit) = own.or(Some(STUDIO_KIT).filter(can)) else {
            return;
        };
        self.sample_when_read(kit, latency_us, Some(id.to_owned()), effects);
    }

    /// `PlacePattern`. Returns the notice to show.
    pub fn place_pattern(&mut self, id: &str, effects: &mut Vec<Effect>) -> Option<StatusMessage> {
        if self.busy() || self.shelf.mode != Mode::NewTrack {
            return None;
        }
        let p = patterns::find(id)?;
        let start = self.bar_head(self.play_line());
        let end = start + p.ticks();
        if end > MAX_TICK {
            return None;
        }
        let map = self.map();
        let bar = Meter::FOUR_FOUR.ticks_per_bar();
        let four_four =
            (0..u64::from(p.bars)).all(|b| map.meter_at(start + b * bar).meter == Meter::FOUR_FOUR);
        if !four_four {
            return Some(StatusMessage::PatternNeedsFourFour);
        }
        let (doc, track, new) = match self.drum_target() {
            Some(track) => (self.doc().clone(), track, false),
            None => {
                if self.doc().tracks.len() >= MAX_TRACKS {
                    return Some(StatusMessage::TooManyTracks);
                }
                let below = self.song.session.play_target;
                let (e, id) = edit::add_track(self.doc(), below, STUDIO_KIT, self.names).ok()?;
                (e.doc, id, true)
            }
        };
        let (made, clip) = match notes::create_clip_to(&doc, track, start, end) {
            Ok(made) => made,
            Err(EditError::Overlap) => return Some(StatusMessage::NotesNotWritten),
            Err(_) => return None,
        };
        let written =
            notes::write_notes(&made.doc, WriteAt::Clip(clip), &p.notes_at(start)).ok()?;
        let e = Edit {
            doc: written.edit.doc,
            step: StepKind::PlacePattern { track, clip },
        };
        let before = new.then(|| self.song.session.play_target.replace(track));
        self.apply_selecting(e, None, Some(ClipPick::one(clip)), effects);
        if let Some(before) = before {
            self.selected_track_changed(before);
            self.sync_target(effects);
        }
        self.locate(end, effects);
        None
    }
}

#[cfg(test)]
mod tests;

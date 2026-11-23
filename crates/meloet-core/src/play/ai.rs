//! The AI link's writes (`crate::ai`). Each is one step through `apply`, as the screen's
//! intents are, with the step names those already have. Nothing is selected or opened on
//! the screen: the AI is not the one looking at it.

use meloet_model::edit::{self, EditError};
use meloet_model::notes::{self, WriteAt};
use meloet_model::{ClipId, Note, TakeId, Tempo, TrackId, arrange, timeline as timeline_edit};

use super::{MAX_TRACKS, Play};
use crate::ai::{AiPlace, AiRefusal};
use crate::state::Effect;

/// What a write left: the clip it was about and the track added.
#[derive(Debug, Default)]
pub struct AiWrote {
    pub clip: Option<ClipId>,
    pub track: Option<TrackId>,
}

fn refusal(e: EditError) -> AiRefusal {
    match e {
        EditError::NoSuchTrack(t) => AiRefusal::NoSuchTrack(t.0),
        EditError::NoSuchClip(c) => AiRefusal::NoSuchClip(c.0),
        EditError::NoSuchNote(i) => AiRefusal::NoSuchNote(i as u32),
        EditError::NotKeys | EditError::WrongKind => AiRefusal::NotKeys,
        EditError::Overlap => AiRefusal::Overlap,
        EditError::OffTheSong | EditError::OutOfClip => AiRefusal::OffTheSong,
        EditError::BadNote => AiRefusal::BadNote,
        _ => AiRefusal::Unchanged,
    }
}

impl Play {
    pub fn counting_in(&self) -> bool {
        self.state == meloet_protocol::TransportState::CountIn
    }

    /// The id of the track of number `number` (from 1).
    fn track_numbered(&self, number: u32) -> Result<TrackId, AiRefusal> {
        let index = number
            .checked_sub(1)
            .ok_or(AiRefusal::NoSuchTrack(number))?;
        (self.doc().tracks.get(index as usize))
            .map(|t| t.id)
            .ok_or(AiRefusal::NoSuchTrack(number))
    }

    pub fn ai_write_notes(
        &mut self,
        at: AiPlace,
        notes: &[Note],
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let at = match at {
            AiPlace::Track(number) => WriteAt::Track(self.track_numbered(number)?),
            AiPlace::Clip(clip) => WriteAt::Clip(ClipId(clip)),
        };
        let written = notes::write_notes(self.doc(), at, notes).map_err(refusal)?;
        self.apply(written.edit, effects);
        Ok(AiWrote {
            clip: Some(written.clip),
            track: None,
        })
    }

    pub fn ai_delete_notes(
        &mut self,
        take: u32,
        picked: &[u32],
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let clip = (self.doc().tracks.iter())
            .flat_map(|t| t.clips.iter())
            .find(|c| c.is_keys() && c.active_take().take.id == TakeId(take))
            .map(|c| c.id)
            .ok_or(AiRefusal::NoSuchTake(take))?;
        let picked: Vec<usize> = picked.iter().map(|&i| i as usize).collect();
        let deleted = notes::delete_notes(self.doc(), clip, &picked, false).map_err(refusal)?;
        self.apply(deleted.edit, effects);
        Ok(AiWrote {
            clip: Some(clip),
            track: None,
        })
    }

    pub fn ai_create_clip(
        &mut self,
        track: u32,
        tick: u64,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let track = self.track_numbered(track)?;
        let (e, clip) = notes::create_clip(self.doc(), track, tick).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote {
            clip: Some(clip),
            track: None,
        })
    }

    pub fn ai_rename_clip(
        &mut self,
        clip: u32,
        name: &str,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let e = arrange::rename_clip(self.doc(), ClipId(clip), name).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote {
            clip: Some(ClipId(clip)),
            track: None,
        })
    }

    pub fn ai_set_tempo(
        &mut self,
        tick: u64,
        bpm: f64,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let tempo = Tempo::from_bpm(bpm).ok_or(AiRefusal::BadTempo)?;
        let tick = self.snapped(tick);
        let e = timeline_edit::add_tempo_point(self.doc(), tick, tempo).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote::default())
    }

    pub fn ai_add_marker(
        &mut self,
        tick: u64,
        name: &str,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let tick = self.snapped(tick);
        let (e, _) = timeline_edit::add_marker(self.doc(), tick, name).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote::default())
    }

    pub fn ai_add_track(
        &mut self,
        instrument: meloet_model::InstrumentRef,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        if self.doc().tracks.len() >= MAX_TRACKS {
            return Err(AiRefusal::TooManyTracks);
        }
        let below = self.doc().tracks.last().map(|t| t.id);
        let (e, id) = edit::add_track(self.doc(), below, instrument).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote {
            clip: None,
            track: Some(id),
        })
    }
}

//! The AI link's writes (`crate::ai`). Each is one step through `apply`, as the screen's
//! intents are, with the step names those already have. Nothing is selected or opened on
//! the screen: the AI is not the one looking at it.

use std::sync::Arc;

use meloet_model::automation as auto_edit;
use meloet_model::edit::{self, EditError, MAX_TICK};
use meloet_model::effects::EffectKind;
use meloet_model::history::StepKind;
use meloet_model::notes::{self, CutAt, NotesEdited, WriteAt, WriteTold};
use meloet_model::{
    AutoPoint, AutoTarget, ClipId, INSERT_SLOTS, InstrumentRef, Note, PluginRef, StripId, TakeId,
    Tempo, TempoMap, TrackId, arrange, mixer, quantize, timeline as timeline_edit,
};

use super::{MAX_TRACKS, Play};
use crate::ai::{
    AiNote, AiNoteCut, AiNoteEdit, AiNoteNot, AiNoteWhy, AiNotesDone, AiPick, AiPlace, AiRefusal,
    AiShift, MAX_TOLD,
};
use crate::state::Effect;

/// What an AI puts into an insert slot, as the song takes it.
pub(crate) enum AiPut {
    Builtin(EffectKind),
    Plugin(Arc<PluginRef>),
}

/// Which change of the points of a lane.
pub(crate) enum AiPoints {
    Put(Vec<AutoPoint>),
    Move {
        picked: Vec<u32>,
        ticks: i64,
        value: f32,
    },
    Delete(Vec<u32>),
}

/// What a write left: the clip it was about and the track added.
#[derive(Debug, Default)]
pub(crate) struct AiWrote {
    pub clip: Option<ClipId>,
    pub track: Option<TrackId>,
}

/// The notes left out and cut, the lists kept short.
#[derive(Default)]
struct Told {
    not_written: Vec<AiNoteNot>,
    not_written_count: u32,
}

impl Told {
    fn not(&mut self, index: u32, note: AiNote, why: AiNoteWhy) {
        self.not_written_count += 1;
        if self.not_written.len() < MAX_TOLD {
            self.not_written.push(AiNoteNot { index, note, why });
        }
    }
}

fn as_asked(n: &Note) -> AiNote {
    AiNote {
        start: n.start as i64,
        length: n.length,
        key: i32::from(n.key),
        velocity: n.velocity,
    }
}

/// Where the notes start and end, each as long as the song lets it be.
fn reach_of(notes: &[Note]) -> (u64, u64) {
    let first = notes.iter().map(|n| n.start).min().unwrap_or(0);
    let last = (notes.iter())
        .map(|n| n.start + n.length.min(MAX_TICK.saturating_sub(n.start)))
        .max()
        .unwrap_or(0);
    (first, last)
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
        EditError::NoSuchInsert => AiRefusal::SlotEmpty,
        EditError::SlotTaken => AiRefusal::SlotTaken,
        EditError::NoSuchPlugin => AiRefusal::NoSuchPlugin,
        EditError::NoSuchPoint => AiRefusal::NoSuchPoint,
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

    /// The clip of the track that holds `tick`.
    fn clip_holding(&self, track: TrackId, tick: u64) -> Option<ClipId> {
        let t = self.doc().track(track)?;
        (t.clips.iter())
            .find(|c| c.start <= tick && tick < c.end)
            .map(|c| c.id)
    }

    /// Why `clip` cannot grow by bars to hold `first..last`: the clip of its track it would
    /// run into.
    fn in_the_way(&self, clip: ClipId, (first, last): (u64, u64)) -> AiRefusal {
        let doc = self.doc();
        let Some((track, c)) = doc.clip(clip) else {
            return AiRefusal::Overlap;
        };
        let map = TempoMap::new(&doc.timeline);
        let start = c.start.min(map.meter_at(first).bar_tick);
        let end = c.end.max(map.bar_at_or_after(last).min(MAX_TICK));
        (track.clips.iter())
            .find(|o| o.id != clip && o.start < end && start < o.end)
            .map_or(AiRefusal::Overlap, |o| AiRefusal::OverlapsClip {
                clip: o.id.0,
                start: o.start,
                end: o.end,
            })
    }

    /// Refuses, naming the clip, when a clip of `track` holds `tick` or starts after it
    /// before `end`.
    fn ai_free(&self, track: TrackId, tick: u64, end: u64) -> Result<(), AiRefusal> {
        let t = (self.doc().track(track)).ok_or(AiRefusal::NoSuchTrack(track.0))?;
        if t.kind.is_vocal() {
            return Err(AiRefusal::NotKeys);
        }
        let in_the_way = (t.clips.iter()).find(|c| c.start < end.max(tick + 1) && tick < c.end);
        match in_the_way {
            Some(c) => Err(AiRefusal::OverlapsClip {
                clip: c.id.0,
                start: c.start,
                end: c.end,
            }),
            None => Ok(()),
        }
    }

    /// Applies a write of notes as its one step, and says what became of `asked` (the
    /// notes given to the model, each with the index it is told by).
    fn ai_notes_written(
        &mut self,
        (edited, wrote): (NotesEdited, WriteTold),
        was: Option<(u64, u64)>,
        asked: &[(u32, AiNote)],
        mut told: Told,
        effects: &mut Vec<Effect>,
    ) -> (AiWrote, AiNotesDone) {
        for &i in &wrote.dropped {
            let (index, note) = asked[i];
            told.not(index, note, AiNoteWhy::PastClipEnd);
        }
        let cut = (wrote.cut.iter().take(MAX_TOLD))
            .map(|c| AiNoteCut {
                index: asked[c.index].0,
                note: asked[c.index].1,
                length: c.length,
                at_clip_end: c.at == CutAt::ClipEnd,
            })
            .collect();
        let clip = edited.clip;
        self.apply(edited.edit, effects);
        let doc = self.doc();
        let track = (doc.tracks.iter())
            .position(|t| t.clips.iter().any(|c| c.id == clip))
            .map_or(0, |i| i as u32 + 1);
        let now = (doc.clip(clip)).map_or((0, 0), |(_, c)| (c.start, c.end));
        let done = AiNotesDone {
            clip: clip.0,
            track,
            clip_made: wrote.clip_made,
            was: was.filter(|_| !wrote.clip_made),
            now,
            written: (asked.len() - wrote.dropped.len()) as u32,
            not_written: told.not_written,
            not_written_count: told.not_written_count,
            cut,
            cut_count: wrote.cut.len() as u32,
        };
        let wrote = AiWrote {
            clip: Some(clip),
            track: None,
        };
        (wrote, done)
    }

    pub(crate) fn ai_write_notes(
        &mut self,
        at: AiPlace,
        notes: &[Note],
        effects: &mut Vec<Effect>,
    ) -> Result<(AiWrote, AiNotesDone), AiRefusal> {
        let reach = reach_of(notes);
        let (at, target) = match at {
            AiPlace::Track(number) => {
                let track = self.track_numbered(number)?;
                (WriteAt::Track(track), self.clip_holding(track, reach.0))
            }
            AiPlace::Clip(clip) => (WriteAt::Clip(ClipId(clip)), Some(ClipId(clip))),
        };
        let was = target
            .and_then(|c| self.doc().clip(c))
            .map(|(_, c)| (c.start, c.end));
        let written = notes::write_notes_told(self.doc(), at, notes).map_err(|e| match e {
            EditError::Overlap => target.map_or(AiRefusal::Overlap, |c| self.in_the_way(c, reach)),
            e => refusal(e),
        })?;
        let asked: Vec<(u32, AiNote)> = (notes.iter().enumerate())
            .map(|(i, n)| (i as u32, as_asked(n)))
            .collect();
        Ok(self.ai_notes_written(written, was, &asked, Told::default(), effects))
    }

    /// Many notes as one step; `labels` are the indexes they are told by (else their place
    /// in `notes`). A note that cannot be in the song is left out and told; a clip made for
    /// the notes is as long as they need, to the end of a bar. Nothing is written when a
    /// clip of the track is in the way.
    pub(crate) fn ai_put_notes(
        &mut self,
        at: AiPlace,
        notes: &[AiNote],
        labels: Option<&[u32]>,
        effects: &mut Vec<Effect>,
    ) -> Result<(AiWrote, AiNotesDone), AiRefusal> {
        let mut told = Told::default();
        let mut asked = Vec::with_capacity(notes.len());
        let mut fits = Vec::with_capacity(notes.len());
        for (i, n) in notes.iter().enumerate() {
            if n.length == 0 || !(1..=127).contains(&n.velocity) {
                return Err(AiRefusal::BadNote);
            }
            let index = labels.map_or(i as u32, |l| l[i]);
            let why = if n.start < 0 {
                Some(AiNoteWhy::BeforeSongStart)
            } else if n.start as u64 >= MAX_TICK {
                Some(AiNoteWhy::PastSongEnd)
            } else if !(0..=127).contains(&n.key) {
                Some(AiNoteWhy::KeyOutOfRange)
            } else {
                None
            };
            match why {
                Some(why) => told.not(index, *n, why),
                None => {
                    asked.push((index, *n));
                    fits.push(Note {
                        start: n.start as u64,
                        length: n.length,
                        key: n.key as u8,
                        velocity: n.velocity,
                        from: None,
                    });
                }
            }
        }
        if fits.is_empty() {
            return Err((told.not_written.first())
                .map_or(AiRefusal::Unchanged, |n| AiRefusal::NoNoteFits(n.why)));
        }
        let reach = reach_of(&fits);
        let (track, target) = match at {
            AiPlace::Clip(clip) => (None, Some(ClipId(clip))),
            AiPlace::Track(number) => {
                let track = self.track_numbered(number)?;
                (Some(track), self.clip_holding(track, reach.0))
            }
        };
        let doc = self.doc();
        let (written, was) = match (target, track) {
            (Some(clip), _) => {
                let was = doc.clip(clip).map(|(_, c)| (c.start, c.end));
                let written = notes::write_notes_told(doc, WriteAt::Clip(clip), &fits);
                let written = written.map_err(|e| match e {
                    EditError::Overlap => self.in_the_way(clip, reach),
                    e => refusal(e),
                })?;
                (written, was)
            }
            (None, Some(track)) => {
                let end = TempoMap::new(&doc.timeline)
                    .bar_at_or_after(reach.1)
                    .min(MAX_TICK);
                self.ai_free(track, reach.0, end)?;
                let (made, clip) =
                    notes::create_clip_to(doc, track, reach.0, end).map_err(refusal)?;
                let (mut edited, mut wrote) =
                    notes::write_notes_told(&made.doc, WriteAt::Clip(clip), &fits)
                        .map_err(refusal)?;
                edited.edit.step = StepKind::WriteNote {
                    clip,
                    clip_made: true,
                    count: fits.len(),
                };
                wrote.clip_made = true;
                ((edited, wrote), None)
            }
            (None, None) => return Err(AiRefusal::Unchanged),
        };
        Ok(self.ai_notes_written(written, was, &asked, told, effects))
    }

    /// Notes of the take `clip` shows, as they sound, moved and transposed into `to`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn ai_copy_notes(
        &mut self,
        clip: u32,
        take: Option<u32>,
        pick: &AiPick,
        to: AiPlace,
        shift: AiShift,
        transpose: i32,
        effects: &mut Vec<Effect>,
    ) -> Result<(AiWrote, AiNotesDone), AiRefusal> {
        let doc = self.doc();
        let (track, c) = doc.clip(ClipId(clip)).ok_or(AiRefusal::NoSuchClip(clip))?;
        if !c.is_keys() {
            return Err(AiRefusal::NotKeys);
        }
        let shown = c.active_take();
        if let Some(take) = take.filter(|&t| shown.take.id != TakeId(t)) {
            return Err(AiRefusal::NoSuchTake(take));
        }
        let zero = c.zero(shown);
        let (start, end) = (c.start as i64, c.end as i64);
        let all: Vec<&Note> = shown.take.notes.iter().collect();
        let mut picked: Vec<u32> = match pick {
            AiPick::Notes(indexes) => indexes.clone(),
            _ => (0..all.len() as u32).collect(),
        };
        picked.sort_unstable();
        picked.dedup();
        if let Some(&i) = picked.iter().find(|&&i| i as usize >= all.len()) {
            return Err(AiRefusal::NoSuchNote(i));
        }
        let mut sounding = Vec::new();
        // Asked for by index, but trimmed off.
        let mut silent = Vec::new();
        for &i in &picked {
            let n = all[i as usize];
            let at = zero + n.start as i64;
            let source = AiNote {
                start: at,
                length: n.length,
                key: i32::from(n.key),
                velocity: n.velocity,
            };
            if at < start || at >= end {
                if matches!(pick, AiPick::Notes(_)) {
                    silent.push((i, source));
                }
                continue;
            }
            if let AiPick::Range { from, to } = *pick
                && !(from as i64..to as i64).contains(&at)
            {
                continue;
            }
            let length = n.length.min((end - at) as u64);
            sounding.push((i, AiNote { length, ..source }));
        }
        let first = sounding.iter().map(|(_, n)| n.start).min();
        let delta = match shift {
            AiShift::Ticks(ticks) => ticks,
            AiShift::To(tick) => {
                let from = match *pick {
                    AiPick::Range { from, .. } => from as i64,
                    _ => start,
                };
                (tick as i64).saturating_sub(from)
            }
            AiShift::Bars(bars) => {
                let map = TempoMap::new(&doc.timeline);
                let bar = map.meter_at(first.unwrap_or(start) as u64).bar as i64;
                let to = bar.saturating_add(bars);
                if to < 0 {
                    return Err(AiRefusal::NoNoteFits(AiNoteWhy::BeforeSongStart));
                }
                map.tick_of_bar(to as u64).min(MAX_TICK) as i64 - map.tick_of_bar(bar as u64) as i64
            }
        };
        let same_place = match to {
            AiPlace::Clip(to) => to == clip,
            AiPlace::Track(number) => self.track_numbered(number)? == track.id,
        };
        if same_place && delta == 0 && transpose == 0 {
            return Err(AiRefusal::Unchanged);
        }
        let labels: Vec<u32> = sounding.iter().map(|(i, _)| *i).collect();
        let notes: Vec<AiNote> = (sounding.iter())
            .map(|(_, n)| AiNote {
                start: n.start.saturating_add(delta),
                key: n.key.saturating_add(transpose),
                ..*n
            })
            .collect();
        if notes.is_empty() {
            return Err(match silent.first() {
                Some(_) => AiRefusal::NoNoteFits(AiNoteWhy::OutsideSourceClip),
                None => AiRefusal::Unchanged,
            });
        }
        let (wrote, mut done) = self.ai_put_notes(to, &notes, Some(&labels), effects)?;
        done.not_written_count += silent.len() as u32;
        let room = MAX_TOLD.saturating_sub(done.not_written.len());
        let silent = silent
            .into_iter()
            .take(room)
            .map(|(index, note)| AiNoteNot {
                index,
                note,
                why: AiNoteWhy::OutsideSourceClip,
            });
        done.not_written.extend(silent);
        Ok((wrote, done))
    }

    /// The keys clip that shows `take` now.
    fn clip_showing(&self, take: u32) -> Result<ClipId, AiRefusal> {
        (self.doc().tracks.iter())
            .flat_map(|t| t.clips.iter())
            .find(|c| c.is_keys() && c.active_take().take.id == TakeId(take))
            .map(|c| c.id)
            .ok_or(AiRefusal::NoSuchTake(take))
    }

    pub(crate) fn ai_delete_notes(
        &mut self,
        take: u32,
        picked: &[u32],
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let clip = self.clip_showing(take)?;
        let picked: Vec<usize> = picked.iter().map(|&i| i as usize).collect();
        let deleted = notes::delete_notes(self.doc(), clip, &picked, false).map_err(refusal)?;
        self.apply(deleted.edit, effects);
        Ok(AiWrote {
            clip: Some(clip),
            track: None,
        })
    }

    pub(crate) fn ai_create_clip(
        &mut self,
        track: u32,
        tick: u64,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let track = self.track_numbered(track)?;
        self.ai_free(track, tick, tick)?;
        let (e, clip) = notes::create_clip(self.doc(), track, tick).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote {
            clip: Some(clip),
            track: None,
        })
    }

    /// A clip from the head of the bar of `tick` to the first head of a bar at `end` or
    /// after it.
    pub(crate) fn ai_create_clip_to(
        &mut self,
        track: u32,
        tick: u64,
        end: u64,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let track = self.track_numbered(track)?;
        if tick >= MAX_TICK || end > MAX_TICK {
            return Err(AiRefusal::OffTheSong);
        }
        if end <= tick {
            return Err(AiRefusal::Unchanged);
        }
        let end = TempoMap::new(&self.doc().timeline)
            .bar_at_or_after(end)
            .min(MAX_TICK);
        self.ai_free(track, tick, end)?;
        let (e, clip) = notes::create_clip_to(self.doc(), track, tick, end).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote {
            clip: Some(clip),
            track: None,
        })
    }

    pub(crate) fn ai_rename_clip(
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

    pub(crate) fn ai_set_tempo(
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

    pub(crate) fn ai_add_marker(
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

    pub(crate) fn ai_edit_notes(
        &mut self,
        take: u32,
        picked: &[u32],
        how: &AiNoteEdit,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let clip = self.clip_showing(take)?;
        let picked: Vec<usize> = picked.iter().map(|&i| i as usize).collect();
        let doc = self.doc();
        let edited = match *how {
            AiNoteEdit::Move { ticks, keys, copy } => {
                notes::move_notes(doc, clip, &picked, ticks, keys, copy)
            }
            AiNoteEdit::Resize { edge, ticks } => {
                notes::resize_notes(doc, clip, &picked, edge, ticks)
            }
            AiNoteEdit::Velocity { delta } => notes::change_velocity(doc, clip, &picked, delta),
            AiNoteEdit::Set { field, value } => {
                notes::set_notes_field(doc, clip, &picked, field, value)
            }
            AiNoteEdit::Quantize(settings) => {
                quantize::quantize(doc, clip, &picked, settings).map(|(e, _)| e)
            }
            AiNoteEdit::Unquantize => quantize::unquantize(doc, clip, &picked),
        };
        self.apply(edited.map_err(refusal)?.edit, effects);
        Ok(AiWrote {
            clip: Some(clip),
            track: None,
        })
    }

    pub(crate) fn ai_delete_clips(
        &mut self,
        clips: &[u32],
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let ids: Vec<ClipId> = clips.iter().map(|&c| ClipId(c)).collect();
        let e = edit::delete_clips(self.doc(), &ids, false).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote::default())
    }

    pub(crate) fn ai_switch_take(
        &mut self,
        clip: u32,
        index: u32,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let e = match edit::switch_take(self.doc(), ClipId(clip), index as usize) {
            Err(EditError::NoSuchTake(_)) => return Err(AiRefusal::NoSuchTake(index)),
            e => e.map_err(refusal)?,
        };
        self.apply(e, effects);
        Ok(AiWrote {
            clip: Some(ClipId(clip)),
            track: None,
        })
    }

    pub(crate) fn ai_set_instrument(
        &mut self,
        track: TrackId,
        instrument: InstrumentRef,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let t = (self.doc().track(track)).ok_or(AiRefusal::NoSuchTrack(track.0))?;
        if t.kind.is_vocal() {
            return Err(AiRefusal::NotKeys);
        }
        let e = edit::set_instrument(self.doc(), track, instrument).map_err(refusal)?;
        self.apply(e, effects);
        Ok(AiWrote::default())
    }

    pub(crate) fn ai_put_effect(
        &mut self,
        strip: StripId,
        slot: usize,
        put: AiPut,
        replace: bool,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let doc = self.doc();
        if doc.channel(strip).is_none() {
            return Err(match strip {
                StripId::Track(t) => AiRefusal::NoSuchTrack(t.0),
                _ => AiRefusal::Unchanged,
            });
        }
        if slot >= INSERT_SLOTS {
            return Err(AiRefusal::SlotEmpty);
        }
        match (doc.insert(strip, slot).is_some(), replace) {
            (true, false) => return Err(AiRefusal::SlotTaken),
            (false, true) => return Err(AiRefusal::SlotEmpty),
            _ => {}
        }
        let e = match (put, replace) {
            (AiPut::Builtin(kind), false) => mixer::add_effect(doc, strip, slot, kind),
            (AiPut::Plugin(plugin), false) => mixer::add_plugin_effect(doc, strip, slot, plugin),
            (AiPut::Builtin(kind), true) => mixer::replace_effect(doc, strip, slot, kind),
            (AiPut::Plugin(plugin), true) => mixer::replace_plugin_effect(doc, strip, slot, plugin),
        };
        self.apply(e.map_err(refusal)?, effects);
        Ok(AiWrote::default())
    }

    pub(crate) fn ai_points(
        &mut self,
        track: TrackId,
        target: AutoTarget,
        change: AiPoints,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let t = (self.doc().track(track)).ok_or(AiRefusal::NoSuchTrack(track.0))?;
        let lane = t.lane(target).ok_or(AiRefusal::NoSuchLane)?.id;
        let picked = |p: &[u32]| p.iter().map(|&i| i as usize).collect::<Vec<_>>();
        let doc = self.doc();
        let point = |e: EditError| match e {
            EditError::NoSuchPoint => AiRefusal::NoSuchPoint,
            e => refusal(e),
        };
        let e = match change {
            AiPoints::Put(points) => auto_edit::put_points(doc, track, target, &points)
                .map(|(e, _)| e)
                .map_err(refusal)?,
            AiPoints::Move {
                picked: p,
                ticks,
                value,
            } => auto_edit::move_points(doc, lane, &picked(&p), ticks, value)
                .map(|(e, _, _)| e)
                .map_err(point)?,
            AiPoints::Delete(p) => auto_edit::delete_points(doc, lane, &picked(&p))
                .map(|(e, _)| e)
                .map_err(point)?,
        };
        self.apply(e, effects);
        Ok(AiWrote::default())
    }

    pub(crate) fn ai_add_track(
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

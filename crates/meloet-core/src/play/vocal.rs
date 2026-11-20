//! Vocal tracks: their ●, their monitor, their voice in the engine, and what a recording of
//! the input keeps until it is in the song (spec 3, 23-A and 24-C; docs/architecture.md
//! 22-6, 22-7).

use std::path::Path;

use meloet_model::song::Clip;
use meloet_model::{
    ClipId, NewAudio, TempoMap, TrackId, TrackKind, VocalInput, VocalRecording, edit,
};
use meloet_protocol::{
    AudioClipState, AudioClipView, RecordFormat, StatusMessage, StretchView, VocalRecordingView,
    VocalRef,
};
use sha2::{Digest, Sha256};

use super::{EngineOp, Play, Rec};
use crate::state::{Effect, vocal_input_view};
use crate::vocalrec::{self, Marks, NoteFile, RecNote, Written};

/// `ClipView::audio`: what a vocal clip has, from its shown take; `None` for a keys clip.
///
/// `state`: whether the file of the shown take is there (`Play::audio_state`). `resampled`:
/// its rate is not the one the audio runs at.
pub(super) fn audio_clip_view(
    clip: &Clip,
    state: AudioClipState,
    resampled: bool,
) -> Option<AudioClipView> {
    let take = &clip.active_audio()?.take;
    let shape = clip.takes.shape()?;
    Some(AudioClipView {
        audio: take.source.id.0,
        file: take.source.file.clone(),
        sample_rate: take.source.sample_rate,
        bits: take.source.bits,
        channels: take.source.channels,
        resampled,
        first_frame: take.first_frame,
        frames: take.frames,
        fade_in: shape.fade_in,
        fade_out: shape.fade_out,
        gain_db: shape.gain.db() as f32,
        state,
        stretch: StretchView {
            follow: shape.follow,
            source_bpm: take.sync.as_ref().map(|s| s.tempo.bpm()),
            semitones: shape.semitones,
            cents: shape.cents,
            keep_formant: shape.keep_formant,
        },
    })
}

/// The PC's input as the rules see it: open, with this many channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenInput {
    pub channels: u16,
    pub sample_rate: u32,
}

/// What a vocal track sounds in the engine (`EngineOp::Voice`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoiceSpec {
    pub input: VocalInput,
    /// The ● and the monitor are on, and the input has the channels.
    pub on: bool,
}

/// A file a recording asks the disk writer for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VocalFileAsk {
    /// The file's name, as `vocalrec::file_name` makes it.
    pub name: String,
    /// The channels of the input that go into it.
    pub channels: Vec<u8>,
}

/// One vocal track of a recording going on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct VocalRec {
    pub track: TrackId,
    pub input: VocalInput,
    pub file: String,
    /// The number the file gets in the song's audio table.
    pub audio: u32,
    pub take: u32,
    pub clip: Option<ClipId>,
}

/// What a recording keeps of its vocal tracks.
#[derive(Debug, Clone, PartialEq, Default)]
pub(super) struct VocalPart {
    pub tracks: Vec<VocalRec>,
    pub sample_rate: u32,
    pub format: RecordFormat,
    /// The engine's frame the song rolled at, the tick it was at, and the files' frame
    /// that was sung then.
    pub roll: Option<(u64, f64, Option<f64>)>,
    /// The engine's frame each further lap began with, and the files' frame sung then.
    pub lap_frames: Vec<(u64, Option<f64>)>,
    /// What the disk writer wrote, once it has stopped.
    pub written: Option<Written>,
    /// The disk writer made the files.
    pub started: bool,
    /// What the note on the disk says now.
    pub noted: Option<Marks>,
}

impl VocalPart {
    /// The marks of the files, in the files' own frames. A lap whose mark the disk writer
    /// did not give is counted on from the roll in the engine's frames.
    fn marks(&self, from: u64, loop_range: Option<(u64, u64)>) -> Marks {
        let roll = (self.roll).and_then(|(frame, tick, file)| Some((frame, tick, file?)));
        Marks {
            from,
            loop_range,
            roll: roll.map(|(_, tick, file)| (file, tick)),
            laps: match roll {
                Some((rolled, _, at)) => (self.lap_frames.iter())
                    .map(|(frame, file)| file.unwrap_or(at + (*frame as f64 - rolled as f64)))
                    .collect(),
                None => Vec::new(),
            },
        }
    }
}

impl Play {
    /// The vocal tracks, in the order of the song.
    fn vocal_tracks(&self) -> impl Iterator<Item = (TrackId, VocalInput)> + '_ {
        (self.doc().tracks.iter()).filter_map(|t| match t.kind {
            TrackKind::Vocal { input } => Some((t.id, input)),
            TrackKind::Keys => None,
        })
    }

    fn vocal_input(&self, track: TrackId) -> Option<VocalInput> {
        self.doc().track(track)?.kind.input()
    }

    /// A vocal track is selected or has its ● on: the input is wanted open.
    pub fn wants_input(&self) -> bool {
        let session = &self.song.session;
        self.vocal_tracks()
            .any(|(id, _)| session.play_target == Some(id) || session.armed_vocals.contains(&id))
    }

    /// The vocal tracks whose input is metered: selected, or with the ● on. With the
    /// channels of the input each reads.
    pub fn metered_vocals(&self) -> Vec<(TrackId, VocalInput)> {
        let session = &self.song.session;
        self.vocal_tracks()
            .filter(|(id, _)| session.play_target == Some(*id) || session.armed_vocals.contains(id))
            .collect()
    }

    /// The selected track is a vocal track.
    pub fn vocal_selected(&self) -> bool {
        (self.song.session.play_target).is_some_and(|t| self.vocal_input(t).is_some())
    }

    pub(super) fn vocal_ref(&self, track: TrackId) -> Option<VocalRef> {
        Some(VocalRef {
            track: self.track_ref(track)?,
            input: vocal_input_view(self.vocal_input(track)?),
        })
    }

    /// Selecting a track: the keys stay with the keys track selected before a vocal one, and
    /// a vocal track that is selected gets its ● (spec 3, 24-C).
    pub(super) fn selected_track_changed(&mut self, before: Option<TrackId>) {
        let now = self.song.session.play_target;
        match now.filter(|t| self.vocal_input(*t).is_some()) {
            Some(id) => {
                if before.is_some_and(|t| self.vocal_input(t).is_none()) {
                    self.song.session.last_keys = before;
                }
                self.song.session.armed_vocals.insert(id);
            }
            // A keys track is selected: the keys are its own.
            None => self.song.session.last_keys = None,
        }
    }

    /// The session names only tracks that are in the song.
    pub(super) fn drop_gone_vocals(&mut self) {
        let doc = self.song.doc.clone();
        let vocal = |t: &TrackId| doc.track(*t).is_some_and(|t| t.kind.is_vocal());
        let session = &mut self.song.session;
        session.armed_vocals.retain(vocal);
        session.monitored.retain(vocal);
        session.last_keys =
            (session.last_keys).filter(|t| doc.track(*t).is_some_and(|t| !t.kind.is_vocal()));
    }

    /// 「＋ 歌のトラック」. Returns the notice to show.
    pub fn add_vocal_track(
        &mut self,
        has_input: bool,
        effects: &mut Vec<Effect>,
    ) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        if self.doc().tracks.len() >= super::MAX_TRACKS {
            return None;
        }
        let (e, id) = edit::add_vocal_track(self.doc(), None).ok()?;
        let before = self.song.session.play_target;
        self.apply(e, effects);
        self.song.session.play_target = Some(id);
        self.selected_track_changed(before);
        match has_input {
            true => Some(StatusMessage::VocalTrackAdded {
                track: self.track_ref(id)?,
                input: vocal_input_view(VocalInput::default()),
            }),
            false => Some(StatusMessage::NoInput),
        }
    }

    pub fn set_vocal_input(&mut self, track: u32, input: VocalInput, effects: &mut Vec<Effect>) {
        if self.busy() {
            return;
        }
        if let Ok(e) = edit::set_vocal_input(self.doc(), TrackId(track), input) {
            self.apply(e, effects);
        }
    }

    /// The ● of a vocal track. Returns the notice to show.
    pub fn toggle_vocal_arm(&mut self, track: u32, has_input: bool) -> Option<StatusMessage> {
        let id = TrackId(track);
        if self.busy() || self.vocal_input(id).is_none() {
            return None;
        }
        let armed = &mut self.song.session.armed_vocals;
        if !armed.remove(&id) {
            armed.insert(id);
            return (!has_input).then_some(StatusMessage::NoInput);
        }
        None
    }

    /// 「モニター」. Returns whether it is on now; `None` when nothing changed.
    pub fn toggle_monitor(&mut self, track: u32) -> Option<bool> {
        let id = TrackId(track);
        self.vocal_input(id)?;
        let monitored = &mut self.song.session.monitored;
        if monitored.remove(&id) {
            return Some(false);
        }
        monitored.insert(id);
        Some(true)
    }

    /// Tells the engine which seats sound the input, and whether they are heard now.
    pub fn sync_voices(&mut self, input: Option<OpenInput>, effects: &mut Vec<Effect>) {
        let session = &self.song.session;
        let mut wanted: Vec<(usize, VoiceSpec)> = (self.vocal_tracks())
            .filter_map(|(id, kind)| {
                let on = input.is_some_and(|i| kind.fits(i.channels))
                    && session.armed_vocals.contains(&id)
                    && session.monitored.contains(&id);
                Some((self.seats.of(id)?, VoiceSpec { input: kind, on }))
            })
            .collect();
        wanted.sort_by_key(|(seat, _)| *seat);
        if wanted == self.voices_sent {
            return;
        }
        for (seat, _) in &self.voices_sent {
            if !wanted.iter().any(|(s, _)| s == seat) {
                effects.push(Effect::Engine(EngineOp::Voice {
                    seat: *seat,
                    voice: None,
                }));
            }
        }
        for (seat, voice) in &wanted {
            if !self.voices_sent.contains(&(*seat, *voice)) {
                effects.push(Effect::Engine(EngineOp::Voice {
                    seat: *seat,
                    voice: Some(*voice),
                }));
            }
        }
        self.voices_sent = wanted;
    }

    /// Some vocal track is heard through meloet now.
    pub fn monitoring(&self) -> bool {
        self.voices_sent.iter().any(|(_, v)| v.on)
    }

    // ---- recording ----

    /// The vocal tracks a recording from `from` goes to, and the files to ask for: the armed
    /// ones the input has the channels for. The second: an armed track has no input.
    pub(super) fn vocals_to_record(
        &self,
        input: Option<OpenInput>,
        from: u64,
        loop_range: Option<meloet_model::TickRange>,
    ) -> (Vec<VocalRec>, bool) {
        let doc = self.doc();
        let armed: Vec<_> = (self.vocal_tracks())
            .filter(|(id, _)| self.song.session.armed_vocals.contains(id))
            .collect();
        let mut next_id = doc.next_id;
        let tracks: Vec<VocalRec> = armed
            .iter()
            .filter(|(id, kind)| {
                input.is_some_and(|i| kind.fits(i.channels)) && self.seats.of(*id).is_some()
            })
            .filter_map(|(id, kind)| {
                let t = doc.track(*id)?;
                let span = loop_range.or_else(|| meloet_model::TickRange::new(from, from + 1))?;
                let clip = (t.clips.iter()).find(|c| c.range().overlaps(span));
                let audio = next_id;
                let file = vocalrec::file_name(&t.name, audio);
                next_id = next_id.saturating_add(1);
                Some(VocalRec {
                    track: *id,
                    input: *kind,
                    file,
                    audio,
                    take: clip.map_or(1, |c| c.takes.len() as u32 + 1),
                    clip: clip.map(|c| c.id),
                })
            })
            .collect();
        let missing = tracks.len() < armed.len();
        (tracks, missing)
    }

    pub(super) fn vocal_file_asks(tracks: &[VocalRec]) -> Vec<VocalFileAsk> {
        (tracks.iter())
            .map(|v| VocalFileAsk {
                name: v.file.clone(),
                channels: v.input.channels().to_vec(),
            })
            .collect()
    }

    /// The disk writer made the files, under these names (another name where one was taken).
    pub fn vocals_started(
        &mut self,
        names: Vec<String>,
        song_file: Option<&Path>,
        effects: &mut Vec<Effect>,
    ) {
        let Some(rec) = &mut self.rec else {
            return;
        };
        for (v, name) in rec.vocal.tracks.iter_mut().zip(names) {
            v.file = name;
        }
        rec.vocal.started = true;
        self.note_vocals(song_file, effects);
    }

    /// The disk writer could not make the files: the recording goes on without the vocals.
    pub fn vocals_not_started(&mut self) {
        if let Some(rec) = &mut self.rec {
            rec.vocal.tracks.clear();
        }
    }

    /// The disk writer stopped and says what it wrote: a recording that waited for it goes
    /// into the song now.
    pub fn vocals_written(&mut self, written: Written, effects: &mut Vec<Effect>) {
        let Some(rec) = &mut self.rec else {
            return;
        };
        rec.vocal.written = Some(written);
        if let Some((tick, lap)) = rec.stopped {
            self.finish(tick, lap, effects);
        }
    }

    /// What the note about the recording going on says of where the files belong.
    #[cfg(test)]
    pub fn noted_vocals(&self) -> Option<Marks> {
        self.rec.as_ref()?.vocal.noted.clone()
    }

    /// Files of the input are being written.
    pub fn recording_vocals(&self) -> bool {
        (self.rec.as_ref()).is_some_and(|r| !r.vocal.tracks.is_empty() && r.vocal.written.is_none())
    }

    /// The selected track is a vocal track with no input to record from.
    pub fn selected_vocal(&self) -> Option<TrackId> {
        (self.song.session.play_target).filter(|t| self.vocal_input(*t).is_some())
    }

    /// Writes the note about the recording again when it knows more of where the files
    /// belong.
    pub fn note_vocals(&mut self, song_file: Option<&Path>, effects: &mut Vec<Effect>) {
        let Some(rec) = &mut self.rec else {
            return;
        };
        if rec.vocal.tracks.is_empty() || !rec.vocal.started || rec.stopped.is_some() {
            return;
        }
        let loop_range = rec.loop_range.map(|r| (r.start(), r.end()));
        let marks = rec.vocal.marks(rec.from, loop_range);
        if rec.vocal.noted.as_ref() == Some(&marks) {
            return;
        }
        rec.vocal.noted = Some(marks.clone());
        let doc = &self.song.doc;
        let files = (rec.vocal.tracks.iter())
            .map(|v| NoteFile {
                file: v.file.clone(),
                track: v.track.0,
                track_name: doc
                    .track(v.track)
                    .map(|t| t.name.clone())
                    .unwrap_or_default(),
                channels: v.input.channels().len() as u8,
            })
            .collect();
        let note = RecNote::new(
            song_file,
            rec.vocal.sample_rate,
            rec.vocal.format,
            files,
            marks,
        );
        effects.push(Effect::WriteRecNote(Box::new(note)));
    }

    /// What the vocal tracks of `rec` recorded, stopped at `tick` of `lap`: the recordings
    /// to put into the song, and the files that hold nothing to keep.
    pub(super) fn vocal_recordings(
        &self,
        rec: &Rec,
        tick: f64,
        lap: u32,
    ) -> (Vec<VocalRecording>, Vec<String>) {
        let part = &rec.vocal;
        let mut unused: Vec<String> = Vec::new();
        let rolled = part.roll.is_some_and(|(.., file)| file.is_some());
        let Some(written) = (part.written.as_ref()).filter(|w| w.frames > 0 && rolled) else {
            unused.extend(part.tracks.iter().map(|v| v.file.clone()));
            return (Vec::new(), unused);
        };
        let map = TempoMap::new(&self.doc().timeline);
        let loop_range = rec.loop_range.map(|r| (r.start(), r.end()));
        let marks = part.marks(rec.from, loop_range);
        let lap = if loop_range.is_some() { lap } else { 0 };
        let takes = vocalrec::takes_from(
            &marks,
            &map,
            written.sample_rate,
            written.frames,
            Some((tick, lap)),
        );
        if takes.is_empty() {
            unused.extend(part.tracks.iter().map(|v| v.file.clone()));
            return (Vec::new(), unused);
        }
        let recordings = (part.tracks.iter())
            .map(|v| VocalRecording {
                track: v.track,
                audio: NewAudio {
                    uid: self.audio_uid(&v.file, rec.id),
                    file: v.file.clone(),
                    frames: written.frames,
                    sample_rate: written.sample_rate,
                    channels: v.input.channels().len() as u8,
                    bits: Some(part.format.bits()),
                },
                takes: takes.clone(),
            })
            .collect();
        (recordings, unused)
    }

    /// A number for an audio file that no other file of any song gets.
    fn audio_uid(&self, file: &str, recording: u32) -> [u8; 16] {
        let mut hash = Sha256::new();
        hash.update(b"meloet audio");
        hash.update(self.uid_seed.to_le_bytes());
        hash.update(self.now_us.to_le_bytes());
        hash.update(recording.to_le_bytes());
        hash.update(file.as_bytes());
        let mut uid = [0u8; 16];
        uid.copy_from_slice(&hash.finalize()[..16]);
        uid
    }

    /// What tells one run of meloet from another in [`Play::audio_uid`].
    pub fn set_uid_seed(&mut self, seed: u64) {
        self.uid_seed = seed;
    }

    pub(super) fn recording_vocals_view(&self) -> Vec<VocalRecordingView> {
        let Some(rec) = &self.rec else {
            return Vec::new();
        };
        // The frame of the files sung at the start of the lap going on.
        let sung = match rec.vocal.lap_frames.last() {
            Some((_, frame)) => *frame,
            None => rec.vocal.roll.and_then(|(_, _, frame)| frame),
        };
        (rec.vocal.tracks.iter())
            .map(|v| VocalRecordingView {
                track: v.track.0,
                from: rec.from,
                take: v.take + rec.lap,
                clip: v.clip.map(|c| c.0),
                audio: v.audio,
                sample_rate: rec.vocal.sample_rate,
                first_frame: sung.map(|f| f.max(0.0).round() as u64),
            })
            .collect()
    }

    /// "● 録音中 ・ 1 ピアノ（鍵盤）と 6 歌（入口 1）に録っています".
    pub(super) fn vocal_recording_status(&self, rec: &Rec) -> Option<StatusMessage> {
        let vocals: Vec<VocalRef> = (rec.vocal.tracks.iter())
            .filter_map(|v| self.vocal_ref(v.track))
            .collect();
        if vocals.is_empty() {
            return None;
        }
        let monitored = &self.song.session.monitored;
        Some(StatusMessage::RecordingVocals {
            keys: rec.track.and_then(|t| self.track_ref(t)),
            monitor: rec
                .vocal
                .tracks
                .iter()
                .any(|v| monitored.contains(&v.track)),
            vocals,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use meloet_model::song::{ClipTake, ClipTakes, TakeId};
    use meloet_model::{AudioId, AudioShape, AudioSource, AudioTake, ClipGain, SourceTempo, Tempo};

    use super::*;

    #[test]
    fn a_vocal_clip_shows_its_file_its_fades_and_its_volume() {
        let source = Arc::new(AudioSource {
            id: AudioId(31),
            uid: [3; 16],
            file: "ドラム.mp3".to_owned(),
            frames: 441_000,
            sample_rate: 44_100,
            channels: 2,
            bits: None,
            sha256: Some([1; 32]),
        });
        let take = |sync| ClipTake {
            take: Arc::new(AudioTake {
                id: TakeId(40),
                source: source.clone(),
                first_frame: 12_000,
                frames: 400_000,
                sync,
            }),
            offset: -1_920,
        };
        let clip = |takes, shape| Clip {
            id: ClipId(9),
            name: "ドラム".to_owned(),
            start: 7_680,
            end: 15_360,
            takes: ClipTakes::Vocal { takes, shape },
            active: 0,
        };
        let shape = AudioShape {
            fade_in: 240,
            fade_out: 960,
            gain: ClipGain::from_db(-3.0).unwrap(),
            follow: false,
            semitones: 2,
            cents: -30,
            ..AudioShape::RECORDED
        };
        let ready = AudioClipState::Ready;
        let view = audio_clip_view(&clip(vec![take(None)], shape), ready, false).unwrap();
        assert_eq!(
            view,
            AudioClipView {
                audio: 31,
                file: "ドラム.mp3".to_owned(),
                sample_rate: 44_100,
                bits: None,
                channels: 2,
                resampled: false,
                first_frame: 12_000,
                frames: 400_000,
                fade_in: 240,
                fade_out: 960,
                gain_db: -3.0,
                state: AudioClipState::Ready,
                stretch: StretchView {
                    follow: false,
                    source_bpm: None,
                    semitones: 2,
                    cents: -30,
                    keep_formant: true,
                },
            }
        );
        let sync = Some(SourceTempo {
            tempo: Tempo::from_centi(12_000).unwrap(),
            head_ramp: false,
            points: Vec::new(),
        });
        let recorded = clip(vec![take(sync)], AudioShape::RECORDED);
        let recorded = audio_clip_view(&recorded, AudioClipState::Missing, true).unwrap();
        assert_eq!(recorded.stretch.source_bpm, Some(120.0));
        assert!(recorded.stretch.follow);
        assert_eq!(
            (recorded.state, recorded.resampled),
            (AudioClipState::Missing, true)
        );

        let keys = Clip {
            takes: ClipTakes::Keys(Vec::new()),
            ..clip(Vec::new(), shape)
        };
        assert_eq!(audio_clip_view(&keys, ready, false), None);
    }
}

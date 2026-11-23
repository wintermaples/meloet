//! Song, tracks and instrument references. Pure data: no I/O, no threads.

pub mod arp;
pub mod arrange;
pub mod audio;
pub mod automation;
pub mod chunked;
pub mod controls;
pub mod drums;
pub mod edit;
pub mod effects;
pub mod history;
#[cfg(test)]
mod id_book;
pub mod instrument;
pub mod knobs;
pub mod mixer;
pub mod mute_sets;
pub mod notes;
pub mod playback;
pub mod presets;
pub mod quantize;
pub mod record;
pub mod roll;
pub mod sections;
pub mod song;
pub mod tempo_map;
pub mod time;
pub mod timeline;

pub use arp::{ARP_GATE_PERCENT, ARP_KEYS_MAX, ARP_OCTAVES_MAX, Arp, ArpRate, ArpShape};
pub use arrange::{ClipEdge, ClipEdited, ClipsSplit};
pub use audio::clips::{
    FadeEnd, ImportPlan, ImportedAudio, ImportedClip, fit_fades, import_audio, remove_audio,
    set_clip_fade, set_clip_gain, set_clips_shape,
};
pub use audio::fade::{clip_fade_gain, fade_gain};
pub use audio::follow::{FileClock, tempo_piece};
pub use audio::record::{NewAudio, NewAudioTake, VocalRecording, add_vocal_recordings};
pub use audio::reverse::{
    ClipsReversed, NORMALIZE_TO_DB, PlayedFrames, Reversed, normalize_clips, normalize_gain,
    played_frames, reverse_clips,
};
pub use audio::stretch::{
    TempoGuess, guess_source_tempo, pick_guess, set_clip_cents, set_clip_follow,
    set_clip_keep_formant, set_clip_semitones, set_clip_tune_scale, set_clip_tune_strength,
    set_source_tempo, source_tempo_guesses,
};
pub use audio::{
    AudioId, AudioShape, AudioSource, AudioTake, ClipGain, CompPiece, SourceTempo,
    TUNE_STRENGTH_MAX, TrackKind, Tune, VOCAL_TRACK_NAME, VocalInput,
};
pub use automation::{AutoPoint, AutoTarget, Lane, LaneHome, LaneId};
pub use chunked::ChunkedVec;
pub use edit::{Edit, EditError, MAX_TICK, NewTake, RecordingAdded};
pub use history::{History, StepKind, StepRow};
pub use instrument::{
    BuiltinInstrument, GmFamily, InstrumentGroup, InstrumentRef, Kept, PluginRef, SF2_DRUM_BANK,
    Sf2Ref,
};
pub use knobs::{SOUND_KNOB_DEFAULTS, SoundKnob, SoundKnobs};
pub use mixer::{
    Bus, Channel, Gain, INSERT_SLOTS, Insert, InsertId, Inserts, Pan, PluginChange, PluginPlace,
    Return, Sends, StripId,
};
pub use mute_sets::MuteSet;
pub use notes::{NoteField, NotesEdited, WriteAt};
pub use roll::{Grid, MusicalScale, QuantizeAlign, QuantizeSettings, RollSettings, ScaleMode};
pub use song::{
    Clip, ClipId, ClipTake, ClipTakes, ControlPoint, ControlValue, Doc, Marker, MarkerId, Note,
    OriginIndex, Session, Song, Take, TakeId, TakeOrigin, Track,
};
pub use tempo_map::{Beat, MeterAt, MeterChange, TempoMap, TempoPoint, Timeline, TimelineError};
pub use time::{Meter, TICKS_PER_QUARTER, Tempo, TickRange};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(pub u32);

/// One of the five track colors (`color.track.1` to `color.track.5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TrackColor(u8);

impl TrackColor {
    pub const COUNT: u8 = 5;

    pub fn new(n: u8) -> Option<Self> {
        (1..=Self::COUNT).contains(&n).then_some(Self(n))
    }

    /// Color for the track at `index` (0-based): the 6th track gets color 1 again.
    pub fn for_index(index: usize) -> Self {
        Self((index % usize::from(Self::COUNT)) as u8 + 1)
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_song() {
        let song = Song::new_default();
        let doc = &song.doc;
        assert_eq!(doc.name, "はじめての曲");
        assert_eq!(doc.timeline.tempo.centi(), 9600);
        assert_eq!(doc.timeline.tempo.bpm(), 96.0);
        assert_eq!(doc.timeline.meter, Meter::new(4, 4).unwrap());
        assert!(doc.timeline.tempo_points.is_empty());
        assert!(doc.timeline.meter_changes.is_empty());
        assert!(doc.markers.is_empty());
        assert_eq!(doc.tracks.len(), 1);
        let t = &doc.tracks[0];
        assert_eq!(t.id, TrackId(1));
        assert_eq!(t.name, "ピアノ");
        assert_eq!(t.color.get(), 1);
        assert_eq!(t.instrument, InstrumentRef::GRAND_PIANO);
        assert_eq!(t.channel, Channel::default());
        assert_eq!(t.sends, [Gain::from_db(-15.0), Gain::SILENT]);
        assert!(t.clips.is_empty());
        assert_eq!(song.session.play_target, Some(TrackId(1)));
        assert_eq!(song.keys_target(), Some(TrackId(1)));
        assert_eq!(t.kind, TrackKind::Keys);
        assert!(doc.audio.is_empty());
        assert_eq!(doc.loop_range, None);
        assert!(!song.session.loop_on);
    }

    #[test]
    fn color_wraps_on_sixth_track() {
        let colors: Vec<u8> = (0..11).map(|i| TrackColor::for_index(i).get()).collect();
        assert_eq!(colors, [1, 2, 3, 4, 5, 1, 2, 3, 4, 5, 1]);
    }

    #[test]
    fn color_range() {
        assert_eq!(TrackColor::new(0), None);
        assert_eq!(TrackColor::new(1).map(TrackColor::get), Some(1));
        assert_eq!(TrackColor::new(5).map(TrackColor::get), Some(5));
        assert_eq!(TrackColor::new(6), None);
    }
}

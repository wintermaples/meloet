//! Song, tracks and instrument references. Pure data: no I/O, no threads.

pub mod arrange;
pub mod chunked;
pub mod controls;
pub mod edit;
pub mod history;
pub mod notes;
pub mod playback;
pub mod quantize;
pub mod record;
pub mod roll;
pub mod song;
pub mod time;

pub use arrange::{ClipEdge, ClipEdited};
pub use chunked::ChunkedVec;
pub use edit::{Edit, EditError, MAX_TICK, NewTake, RecordingAdded};
pub use history::{History, StepKind};
pub use notes::{NoteField, NotesEdited, WriteAt};
pub use roll::{Grid, MusicalScale, QuantizeAlign, QuantizeSettings, RollSettings, ScaleMode};
pub use song::{
    Clip, ClipId, ClipTake, ControlPoint, ControlValue, Doc, Note, OriginIndex, Session, Song,
    Take, TakeId, TakeOrigin, Track,
};
pub use time::{Meter, TICKS_PER_QUARTER, Tempo, TickRange};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct TempoChange {
    pub tick: u64,
    pub tempo_bpm: f64,
    pub meter: Meter,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstrumentRef {
    Placeholder,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_song() {
        let song = Song::new_default();
        let doc = &song.doc;
        assert_eq!(doc.name, "はじめての曲");
        assert_eq!(doc.tempo.centi(), 9600);
        assert_eq!(doc.tempo.bpm(), 96.0);
        assert_eq!(doc.meter, Meter::new(4, 4).unwrap());
        assert!(doc.tempo_changes.is_empty());
        assert_eq!(doc.tracks.len(), 1);
        let t = &doc.tracks[0];
        assert_eq!(t.id, TrackId(1));
        assert_eq!(t.name, "ピアノ");
        assert_eq!(t.color.get(), 1);
        assert_eq!(t.instrument, InstrumentRef::Placeholder);
        assert_eq!(t.gain_db, 0.0);
        assert!(t.clips.is_empty());
        assert_eq!(song.session.play_target, TrackId(1));
        assert_eq!(song.session.armed, Some(TrackId(1)));
        assert_eq!(song.session.loop_range, None);
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

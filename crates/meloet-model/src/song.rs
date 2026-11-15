//! The song: what undo covers (`Doc`) and what it does not (`Session`).

use std::sync::Arc;

use crate::roll::RollSettings;
use crate::time::{Meter, Tempo, TickRange};
use crate::{InstrumentRef, TempoChange, TrackColor, TrackId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClipId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TakeId(pub u32);

/// Positions in a take count from the take's tick 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Note {
    pub start: u64,
    /// 1 or more.
    pub length: u64,
    pub key: u8,
    pub velocity: u8,
}

impl Note {
    pub fn end(&self) -> u64 {
        self.start + self.length
    }
}

pub const CC_MODULATION: u8 = 1;
pub const CC_SUSTAIN: u8 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlValue {
    Cc {
        number: u8,
        value: u8,
    },
    /// -8192 to 8191, 0 is the middle.
    PitchBend(i16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlPoint {
    pub tick: u64,
    pub value: ControlValue,
}

/// One recorded pass. Never changed once made: the screen keeps drawings by `TakeId`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Take {
    pub id: TakeId,
    pub length: u64,
    /// In order of start.
    pub notes: Vec<Note>,
    /// In order of tick.
    pub controls: Vec<ControlPoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipTake {
    pub take: Arc<Take>,
    /// The take's tick 0 is at `clip.start + offset` in the song.
    pub offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clip {
    pub id: ClipId,
    pub name: String,
    pub start: u64,
    pub end: u64,
    /// Never empty.
    pub takes: Vec<ClipTake>,
    /// The take shown and played.
    pub active: usize,
}

impl Clip {
    pub fn range(&self) -> TickRange {
        TickRange::new(self.start, self.end).expect("a clip is never empty")
    }

    pub fn active_take(&self) -> &ClipTake {
        &self.takes[self.active]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub color: TrackColor,
    pub instrument: InstrumentRef,
    pub gain_db: f64,
    /// In order of start, never overlapping.
    pub clips: Vec<Arc<Clip>>,
}

/// What undo covers. Cheap to clone: tracks, clips and takes are shared.
#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    pub name: String,
    pub tempo: Tempo,
    pub meter: Meter,
    pub tempo_changes: Vec<TempoChange>,
    pub tracks: Vec<Arc<Track>>,
    /// The next clip or take id to give out.
    pub next_id: u32,
}

impl Doc {
    pub fn new_default() -> Doc {
        Doc {
            name: "はじめての曲".to_owned(),
            tempo: Tempo::from_centi(9_600).expect("96.00 is in range"),
            meter: Meter::FOUR_FOUR,
            tempo_changes: Vec::new(),
            tracks: vec![Arc::new(Track {
                id: TrackId(1),
                name: "ピアノ".to_owned(),
                color: TrackColor::for_index(0),
                instrument: InstrumentRef::Placeholder,
                gain_db: 0.0,
                clips: Vec::new(),
            })],
            next_id: 1,
        }
    }

    pub fn track(&self, id: TrackId) -> Option<&Arc<Track>> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn clip(&self, id: ClipId) -> Option<(&Arc<Track>, &Arc<Clip>)> {
        self.tracks
            .iter()
            .find_map(|t| t.clips.iter().find(|c| c.id == id).map(|c| (t, c)))
    }
}

/// What undo leaves alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// Where the keys sound.
    pub play_target: TrackId,
    /// The track a recording goes to.
    pub armed: Option<TrackId>,
    pub loop_range: Option<TickRange>,
    pub loop_on: bool,
    pub roll: RollSettings,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Song {
    pub doc: Doc,
    pub session: Session,
}

impl Song {
    pub fn new_default() -> Song {
        Song {
            doc: Doc::new_default(),
            session: Session {
                play_target: TrackId(1),
                armed: Some(TrackId(1)),
                loop_range: None,
                loop_on: false,
                roll: RollSettings::default(),
            },
        }
    }
}

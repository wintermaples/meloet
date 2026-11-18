//! The song: what undo covers (`Doc`) and what it does not (`Session`).

use std::collections::BTreeSet;
use std::num::NonZeroU32;
use std::sync::Arc;

use crate::chunked::ChunkedVec;
use crate::roll::{QuantizeSettings, RollSettings};
use crate::time::{Meter, Tempo, TickRange};
use crate::{InstrumentRef, TempoChange, TrackColor, TrackId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClipId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TakeId(pub u32);

/// Positions in a take count from the take's tick 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Note {
    pub start: u64,
    /// 1 or more.
    pub length: u64,
    pub key: u8,
    pub velocity: u8,
    /// The note of the take's origin this one came from. `None` for a note written, copied or
    /// pasted, and in a take with no origin.
    pub from: Option<OriginIndex>,
}

/// The index of a note in the origin take. Kept as index + 1, so that `Option<OriginIndex>`
/// takes 4 bytes and a note 24.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OriginIndex(NonZeroU32);

impl OriginIndex {
    /// `None` for `u32::MAX`, the one index that does not fit.
    pub fn new(index: u32) -> Option<OriginIndex> {
        index
            .checked_add(1)
            .and_then(NonZeroU32::new)
            .map(OriginIndex)
    }

    pub fn get(self) -> u32 {
        self.0.get() - 1
    }
}

impl Note {
    pub fn end(&self) -> u64 {
        self.start + self.length
    }
}

pub const CC_MODULATION: u8 = 1;
pub const CC_SUSTAIN: u8 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlValue {
    Cc {
        number: u8,
        value: u8,
    },
    /// -8192 to 8191, 0 is the middle.
    PitchBend(i16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ControlPoint {
    pub tick: u64,
    pub value: ControlValue,
}

/// One recorded pass. Never changed once made: the screen keeps drawings by `TakeId`. A take
/// made by changing another shares the chunks of notes and points it did not change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Take {
    pub id: TakeId,
    pub length: u64,
    /// In order of start.
    pub notes: ChunkedVec<Note>,
    /// In order of tick.
    pub controls: ChunkedVec<ControlPoint>,
    /// Set on a quantized take, and kept by later edits until every note is back where it was.
    pub origin: Option<TakeOrigin>,
}

/// The recorded take a quantized take was made from. Its tick 0 is at the tick 0 of the take
/// that holds it, and it never has an origin itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakeOrigin {
    pub take: Arc<Take>,
    /// What the last quantize used.
    pub settings: QuantizeSettings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipTake {
    pub take: Arc<Take>,
    /// The take's tick 0 is at `clip.start + offset` in the song. Below 0 when the head of the
    /// take is trimmed off; the take's tick 0 can then be before the song's.
    pub offset: i64,
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

    /// Where the take's tick 0 is in the song.
    pub fn zero(&self, take: &ClipTake) -> i64 {
        self.start as i64 + take.offset
    }

    /// Whether tick `tick` of the shown take is inside the clip, [start, end). A note shows,
    /// sounds and counts only when its start is; the others stay in the take, trimmed off.
    pub fn holds(&self, tick: u64) -> bool {
        let at = i128::from(self.zero(self.active_take())) + i128::from(tick);
        (i128::from(self.start)..i128::from(self.end)).contains(&at)
    }

    /// The notes of the shown take that start inside the clip.
    pub fn note_count(&self) -> usize {
        let shown = self.active_take();
        let zero = i128::from(self.zero(shown));
        let before = |at: u64| {
            let at = i128::from(at) - zero;
            shown
                .take
                .notes
                .partition_point(|n| i128::from(n.start) < at)
        };
        before(self.end) - before(self.start)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub color: TrackColor,
    pub instrument: InstrumentRef,
    pub gain_db: f64,
    /// Its clips do not play. Keys played on it still sound.
    pub muted: bool,
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
                muted: false,
                clips: Vec::new(),
            })],
            next_id: 1,
        }
    }

    pub fn track(&self, id: TrackId) -> Option<&Arc<Track>> {
        self.tracks.iter().find(|t| t.id == id)
    }

    /// Whether the clips of `track` play: not muted, and soloed when any track is.
    pub fn plays(&self, track: &Track, soloed: &BTreeSet<TrackId>) -> bool {
        let any_solo = self.tracks.iter().any(|t| soloed.contains(&t.id));
        !track.muted && (!any_solo || soloed.contains(&track.id))
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
    /// While any track of the song is here, only these play (a muted one still does not).
    pub soloed: BTreeSet<TrackId>,
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
                soloed: BTreeSet::new(),
            },
        }
    }
}

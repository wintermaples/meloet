//! The song: what undo covers (`Doc`) and what it does not (`Session`).

use std::collections::BTreeSet;
use std::num::NonZeroU32;
use std::sync::Arc;

use crate::arp::Arp;
use crate::audio::{AudioId, AudioShape, AudioSource, AudioTake, CompPiece, TrackKind};
use crate::automation::{AutoTarget, Lane};
use crate::chunked::ChunkedVec;
use crate::knobs::SoundKnobs;
use crate::mixer::{self, Channel, Return, Sends};
use crate::mute_sets::MuteSet;
use crate::roll::{QuantizeSettings, RollSettings};
use crate::tempo_map::Timeline;
use crate::time::{Meter, Tempo, TickRange};
use crate::{InstrumentRef, TrackColor, TrackId};

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

/// A take in a clip: `ClipTake` of a keys clip, `ClipTake<AudioTake>` of a vocal clip.
#[derive(Debug, PartialEq, Eq)]
pub struct ClipTake<T = Take> {
    pub take: Arc<T>,
    /// The take's tick 0 is at `clip.start + offset` in the song. Below 0 when the head of the
    /// take is trimmed off; the take's tick 0 can then be before the song's.
    pub offset: i64,
}

impl<T> Clone for ClipTake<T> {
    fn clone(&self) -> Self {
        ClipTake {
            take: self.take.clone(),
            offset: self.offset,
        }
    }
}

/// The takes of a clip, all of one kind: a clip on a keys track has keys takes, a clip on a
/// vocal track audio takes and the values only audio has. Never empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipTakes {
    Keys(Vec<ClipTake>),
    Vocal {
        takes: Vec<ClipTake<AudioTake>>,
        shape: AudioShape,
    },
}

impl ClipTakes {
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        match self {
            ClipTakes::Keys(takes) => takes.len(),
            ClipTakes::Vocal { takes, .. } => takes.len(),
        }
    }

    /// The keys takes; none for a vocal clip.
    pub fn keys(&self) -> &[ClipTake] {
        match self {
            ClipTakes::Keys(takes) => takes,
            ClipTakes::Vocal { .. } => &[],
        }
    }

    /// The keys takes, to change them; none for a vocal clip.
    pub fn keys_mut(&mut self) -> Option<&mut Vec<ClipTake>> {
        match self {
            ClipTakes::Keys(takes) => Some(takes),
            ClipTakes::Vocal { .. } => None,
        }
    }

    /// The audio takes; none for a keys clip.
    pub fn vocal(&self) -> &[ClipTake<AudioTake>] {
        match self {
            ClipTakes::Keys(_) => &[],
            ClipTakes::Vocal { takes, .. } => takes,
        }
    }

    pub fn shape(&self) -> Option<&AudioShape> {
        match self {
            ClipTakes::Keys(_) => None,
            ClipTakes::Vocal { shape, .. } => Some(shape),
        }
    }

    pub fn id(&self, index: usize) -> Option<TakeId> {
        match self {
            ClipTakes::Keys(takes) => takes.get(index).map(|t| t.take.id),
            ClipTakes::Vocal { takes, .. } => takes.get(index).map(|t| t.take.id),
        }
    }

    pub fn offset(&self, index: usize) -> Option<i64> {
        match self {
            ClipTakes::Keys(takes) => takes.get(index).map(|t| t.offset),
            ClipTakes::Vocal { takes, .. } => takes.get(index).map(|t| t.offset),
        }
    }

    /// The same takes, each with the offset `f` makes of its own.
    pub fn with_offsets(&self, f: impl Fn(i64) -> i64) -> ClipTakes {
        fn each<T>(takes: &[ClipTake<T>], f: impl Fn(i64) -> i64) -> Vec<ClipTake<T>> {
            (takes.iter())
                .map(|ct| ClipTake {
                    take: ct.take.clone(),
                    offset: f(ct.offset),
                })
                .collect()
        }
        match self {
            ClipTakes::Keys(takes) => ClipTakes::Keys(each(takes, f)),
            ClipTakes::Vocal { takes, shape } => ClipTakes::Vocal {
                takes: each(takes, f),
                shape: *shape,
            },
        }
    }

    /// The same takes without the one at `index`, which there must be.
    pub fn without(&self, index: usize) -> ClipTakes {
        let mut new = self.clone();
        match &mut new {
            ClipTakes::Keys(takes) => drop(takes.remove(index)),
            ClipTakes::Vocal { takes, .. } => drop(takes.remove(index)),
        }
        new
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clip {
    pub id: ClipId,
    pub name: String,
    pub start: u64,
    pub end: u64,
    pub takes: ClipTakes,
    /// The take shown and played.
    pub active: usize,
    /// 「いいとこ取り」 of a vocal clip (round 12, A8), in the order of `from`: which take
    /// sounds where. Empty: `active` sounds all along (always for a keys clip).
    pub comp: Vec<CompPiece>,
}

impl Clip {
    pub fn range(&self) -> TickRange {
        TickRange::new(self.start, self.end).expect("a clip is never empty")
    }

    pub fn is_keys(&self) -> bool {
        matches!(self.takes, ClipTakes::Keys(_))
    }

    /// The keys takes; none for a vocal clip.
    pub fn keys(&self) -> &[ClipTake] {
        self.takes.keys()
    }

    /// The shown take of a keys clip. Only for a keys clip ([`Clip::is_keys`]): what edits
    /// notes turns a vocal clip away before it gets here (`EditError::NotKeys`).
    pub fn active_take(&self) -> &ClipTake {
        &self.takes.keys()[self.active]
    }

    /// The shown take of a vocal clip; `None` for a keys clip.
    pub fn active_audio(&self) -> Option<&ClipTake<AudioTake>> {
        self.takes.vocal().get(self.active)
    }

    /// Whether take `index` sounds somewhere in the clip: the shown take, or with
    /// 「いいとこ取り」 the take of a piece.
    pub fn sounds(&self, index: usize) -> bool {
        match self.comp.is_empty() {
            true => index == self.active,
            false => self.comp.iter().any(|p| p.take == index),
        }
    }

    pub fn active_take_id(&self) -> TakeId {
        self.takes.id(self.active).expect("a clip is never empty")
    }

    /// Where the take's tick 0 is in the song.
    pub fn zero<T>(&self, take: &ClipTake<T>) -> i64 {
        self.start as i64 + take.offset
    }

    /// Where the shown take's tick 0 is in the song.
    pub fn active_zero(&self) -> i64 {
        let offset = self.takes.offset(self.active);
        self.start as i64 + offset.expect("a clip is never empty")
    }

    /// Whether tick `tick` of the shown take is inside the clip, [start, end). A note shows,
    /// sounds and counts only when its start is; the others stay in the take, trimmed off.
    pub fn holds(&self, tick: u64) -> bool {
        let at = i128::from(self.active_zero()) + i128::from(tick);
        (i128::from(self.start)..i128::from(self.end)).contains(&at)
    }

    /// The notes of the shown take that start inside the clip; 0 for a vocal clip.
    pub fn note_count(&self) -> usize {
        let Some(shown) = self.keys().get(self.active) else {
            return 0;
        };
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
    pub kind: TrackKind,
    pub instrument: InstrumentRef,
    /// The fader, the pan and the inserts.
    pub channel: Channel,
    pub sends: Sends,
    /// Its clips do not play. Keys played on it still sound.
    pub muted: bool,
    /// In order of start, never overlapping.
    pub clips: Vec<Arc<Clip>>,
    /// The automation lanes, in the order they were added; one for a target at most.
    pub automation: Vec<Arc<Lane>>,
    /// 「アルペジオ」 (round 12, E2).
    pub arp: Arp,
    /// The knobs of a built-in sound (round 12, E3). Another sound starts them again.
    pub knobs: SoundKnobs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MarkerId(pub u32);

/// A name at a place of the song. Markers may share a tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    pub id: MarkerId,
    pub tick: u64,
    pub name: String,
}

/// What undo covers. Cheap to clone: tracks, clips and takes are shared.
#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    pub name: String,
    /// The tempo and meter at the head, and their changes. Always well formed
    /// (`Timeline::check`).
    pub timeline: Arc<Timeline>,
    /// In the order of tick, those at one tick in the order of id.
    pub markers: Arc<Vec<Marker>>,
    /// In the order they were made.
    pub mute_sets: Arc<Vec<MuteSet>>,
    /// The loop's range; whether the loop is on is in the `Session`.
    pub loop_range: Option<TickRange>,
    pub tracks: Vec<Arc<Track>>,
    /// Return A and return B.
    pub returns: [Arc<Return>; 2],
    pub master: Arc<Channel>,
    /// The master's automation lanes (round 12, M5): of its volume only.
    pub master_lanes: Vec<Arc<Lane>>,
    /// The audio table: every audio file a take of the song plays, in the order of id.
    pub audio: Arc<Vec<Arc<AudioSource>>>,
    /// The next clip, take, marker, insert or audio id to give out.
    pub next_id: u32,
}

impl Doc {
    /// [`Doc::new_in`] Japanese.
    pub fn new_default() -> Doc {
        Doc::new_in(crate::Language::Ja)
    }

    /// The song a person starts from, its names in `language`.
    pub fn new_in(language: crate::Language) -> Doc {
        Doc {
            name: crate::first_song_name(language).to_owned(),
            timeline: Arc::new(Timeline::new(
                Tempo::from_centi(9_600).expect("96.00 is in range"),
                Meter::FOUR_FOUR,
            )),
            markers: Arc::new(Vec::new()),
            mute_sets: Arc::default(),
            loop_range: None,
            tracks: vec![Arc::new(Track {
                id: TrackId(1),
                name: InstrumentRef::GRAND_PIANO.track_name(language).to_owned(),
                color: TrackColor::for_index(0),
                kind: TrackKind::Keys,
                instrument: InstrumentRef::GRAND_PIANO,
                channel: Channel::default(),
                sends: mixer::new_track_sends(&InstrumentRef::GRAND_PIANO),
                muted: false,
                clips: Vec::new(),
                automation: Vec::new(),
                arp: Arp::default(),
                knobs: SoundKnobs::default(),
            })],
            returns: mixer::new_song_returns(),
            master: mixer::new_song_master(),
            master_lanes: Vec::new(),
            audio: Arc::default(),
            next_id: mixer::NEW_SONG_NEXT_ID,
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

    pub fn marker(&self, id: MarkerId) -> Option<&Marker> {
        self.markers.iter().find(|m| m.id == id)
    }

    pub fn audio(&self, id: AudioId) -> Option<&Arc<AudioSource>> {
        self.audio.iter().find(|a| a.id == id)
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
    /// The selected track. When it is a keys track, where the keys sound and where their
    /// recording goes ([`Song::keys_target`]). `None` only in a song with no tracks.
    pub play_target: Option<TrackId>,
    /// The keys track that was selected last before a vocal track was (B-94-1). Not saved.
    pub last_keys: Option<TrackId>,
    /// The vocal tracks whose ● is on (spec 3, 24-C): R records to all of them, and to
    /// the keys target. Not saved.
    pub armed_vocals: BTreeSet<TrackId>,
    /// The vocal tracks whose 「モニター」 is on. It sounds only while the track's ● is on.
    /// Not saved: a song opens with every monitor off.
    pub monitored: BTreeSet<TrackId>,
    /// The range is in the `Doc`: changing it is a step.
    pub loop_on: bool,
    pub roll: RollSettings,
    /// While any track of the song is here, only these play (a muted one still does not).
    pub soloed: BTreeSet<TrackId>,
    /// The folded automation lanes (▸ of a lane's header). Saved with the song.
    pub folded_lanes: Vec<(TrackId, AutoTarget)>,
    /// The master's lane is folded. Saved with the song.
    pub master_lane_folded: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Song {
    pub doc: Doc,
    pub session: Session,
}

impl Song {
    /// The keys track the keys sound on and R records the keys to: the selected track, or
    /// while a vocal track is selected the keys track selected last (B-94-1). `None` when
    /// that leaves none.
    pub fn keys_target(&self) -> Option<TrackId> {
        let keys = |id: TrackId| {
            let track = self.doc.track(id)?;
            (!track.kind.is_vocal()).then_some(id)
        };
        (self.session.play_target.and_then(keys)).or(self.session.last_keys.and_then(keys))
    }

    /// [`Song::new_in`] Japanese.
    pub fn new_default() -> Song {
        Song::new_in(crate::Language::Ja)
    }

    pub fn new_in(language: crate::Language) -> Song {
        Song {
            doc: Doc::new_in(language),
            session: Session {
                play_target: Some(TrackId(1)),
                last_keys: None,
                armed_vocals: BTreeSet::new(),
                monitored: BTreeSet::new(),
                loop_on: false,
                roll: RollSettings::default(),
                soloed: BTreeSet::new(),
                folded_lanes: Vec::new(),
                master_lane_folded: false,
            },
        }
    }
}

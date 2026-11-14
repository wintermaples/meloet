//! Song, tracks and instrument references. Pure data: no I/O, no threads.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TrackId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Meter {
    pub num: u8,
    pub den: u8,
}

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

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub color: TrackColor,
    pub instrument: InstrumentRef,
    pub gain_db: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Song {
    pub name: String,
    pub tempo_bpm: f64,
    pub meter: Meter,
    pub tempo_changes: Vec<TempoChange>,
    pub tracks: Vec<Track>,
    pub play_target: TrackId,
}

impl Song {
    pub fn new_default() -> Self {
        let first = TrackId(1);
        Self {
            name: "はじめての曲".to_owned(),
            tempo_bpm: 96.0,
            meter: Meter { num: 4, den: 4 },
            tempo_changes: Vec::new(),
            tracks: vec![Track {
                id: first,
                name: "ピアノ".to_owned(),
                color: TrackColor::for_index(0),
                instrument: InstrumentRef::Placeholder,
                gain_db: 0.0,
            }],
            play_target: first,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_song() {
        let song = Song::new_default();
        assert_eq!(song.name, "はじめての曲");
        assert_eq!(song.tempo_bpm, 96.0);
        assert_eq!(song.meter, Meter { num: 4, den: 4 });
        assert!(song.tempo_changes.is_empty());
        assert_eq!(song.tracks.len(), 1);
        let t = &song.tracks[0];
        assert_eq!(t.id, TrackId(1));
        assert_eq!(t.name, "ピアノ");
        assert_eq!(t.color.get(), 1);
        assert_eq!(t.instrument, InstrumentRef::Placeholder);
        assert_eq!(t.gain_db, 0.0);
        assert_eq!(song.play_target, TrackId(1));
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

//! The drum sounds a kit has and the keys they sit on (General MIDI's drum note map).
//!
//! The studio kit, the electro kit and the brush kit share this table. The names shown on screen
//! belong to the UI; here a sound has only an English id.

/// One sound of a kit, in the order the drum piano roll shows its rows, top to bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DrumPiece {
    Crash,
    Ride,
    OpenHiHat,
    PedalHiHat,
    ClosedHiHat,
    HighTom,
    MidTom,
    LowTom,
    Clap,
    Snare,
    SideStick,
    Kick,
}

impl DrumPiece {
    pub const COUNT: usize = 12;

    /// Top row first.
    pub const ROWS: [DrumPiece; Self::COUNT] = [
        DrumPiece::Crash,
        DrumPiece::Ride,
        DrumPiece::OpenHiHat,
        DrumPiece::PedalHiHat,
        DrumPiece::ClosedHiHat,
        DrumPiece::HighTom,
        DrumPiece::MidTom,
        DrumPiece::LowTom,
        DrumPiece::Clap,
        DrumPiece::Snare,
        DrumPiece::SideStick,
        DrumPiece::Kick,
    ];

    /// The row this sound has in the drum piano roll, 0 at the top.
    pub fn row(self) -> usize {
        self as usize
    }

    pub fn note(self) -> u8 {
        match self {
            DrumPiece::Kick => 36,
            DrumPiece::SideStick => 37,
            DrumPiece::Snare => 38,
            DrumPiece::Clap => 39,
            DrumPiece::ClosedHiHat => 42,
            DrumPiece::PedalHiHat => 44,
            DrumPiece::LowTom => 45,
            DrumPiece::OpenHiHat => 46,
            DrumPiece::MidTom => 47,
            DrumPiece::Crash => 49,
            DrumPiece::HighTom => 50,
            DrumPiece::Ride => 51,
        }
    }

    /// `None` for a key no kit sound sits on: such a note stays in the song but makes no sound.
    pub fn from_note(note: u8) -> Option<DrumPiece> {
        Self::ROWS.into_iter().find(|p| p.note() == note)
    }

    pub fn id(self) -> &'static str {
        match self {
            DrumPiece::Crash => "crash",
            DrumPiece::Ride => "ride",
            DrumPiece::OpenHiHat => "openHiHat",
            DrumPiece::PedalHiHat => "pedalHiHat",
            DrumPiece::ClosedHiHat => "closedHiHat",
            DrumPiece::HighTom => "highTom",
            DrumPiece::MidTom => "midTom",
            DrumPiece::LowTom => "lowTom",
            DrumPiece::Clap => "clap",
            DrumPiece::Snare => "snare",
            DrumPiece::SideStick => "sideStick",
            DrumPiece::Kick => "kick",
        }
    }

    /// Hitting the closed or the pedal hi-hat stops an open hi-hat that is still ringing (they
    /// are one instrument and cannot sound together).
    pub fn stops_open_hi_hat(self) -> bool {
        matches!(self, DrumPiece::ClosedHiHat | DrumPiece::PedalHiHat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_in_enum_order_and_keys_are_distinct() {
        for (i, piece) in DrumPiece::ROWS.into_iter().enumerate() {
            assert_eq!(piece.row(), i);
            assert_eq!(DrumPiece::from_note(piece.note()), Some(piece));
        }
        let mut notes = DrumPiece::ROWS.map(DrumPiece::note);
        notes.sort_unstable();
        assert_eq!(notes, [36, 37, 38, 39, 42, 44, 45, 46, 47, 49, 50, 51]);
        let mut ids = DrumPiece::ROWS.map(DrumPiece::id);
        ids.sort_unstable();
        assert!(ids.windows(2).all(|w| w[0] != w[1]));
    }

    #[test]
    fn keys_outside_the_kit() {
        for note in [0, 35, 40, 41, 43, 48, 52, 127] {
            assert_eq!(DrumPiece::from_note(note), None);
        }
    }

    #[test]
    fn only_the_two_short_hi_hats_stop_the_open_one() {
        let stoppers: Vec<_> = DrumPiece::ROWS
            .into_iter()
            .filter(|p| p.stops_open_hi_hat())
            .collect();
        assert_eq!(stoppers, [DrumPiece::PedalHiHat, DrumPiece::ClosedHiHat]);
    }
}

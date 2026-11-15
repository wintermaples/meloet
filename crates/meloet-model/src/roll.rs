//! The piano roll's settings: grid, scale and quantize. Kept in the `Session`, so undo leaves
//! them alone, and saved with the song.

/// Steps to a whole note the grid and the quantize settings take; 0 is one bar.
pub const DIVISIONS: [u16; 6] = [0, 2, 4, 8, 16, 32];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid {
    division: u16,
    pub triplet: bool,
}

impl Grid {
    pub fn new(division: u16, triplet: bool) -> Option<Grid> {
        DIVISIONS
            .contains(&division)
            .then_some(Grid { division, triplet })
    }

    pub fn division(self) -> u16 {
        self.division
    }
}

impl Default for Grid {
    fn default() -> Grid {
        Grid {
            division: 16,
            triplet: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaleMode {
    Major,
    /// The natural minor.
    Minor,
}

impl ScaleMode {
    /// Steps above the root, in semitones.
    fn steps(self) -> [u8; 7] {
        match self {
            ScaleMode::Major => [0, 2, 4, 5, 7, 9, 11],
            ScaleMode::Minor => [0, 2, 3, 5, 7, 8, 10],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MusicalScale {
    /// 0..=11 is C..B.
    root: u8,
    pub mode: ScaleMode,
    /// Only keys of the scale can be written.
    pub lock: bool,
}

impl MusicalScale {
    pub fn new(root: u8, mode: ScaleMode, lock: bool) -> Option<MusicalScale> {
        (root < 12).then_some(MusicalScale { root, mode, lock })
    }

    pub fn root(self) -> u8 {
        self.root
    }

    pub fn contains(self, key: u8) -> bool {
        let step = (key % 12 + 12 - self.root) % 12;
        self.mode.steps().contains(&step)
    }
}

impl Default for MusicalScale {
    fn default() -> MusicalScale {
        MusicalScale {
            root: 0,
            mode: ScaleMode::Major,
            lock: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantizeAlign {
    Start,
    StartAndEnd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantizeSettings {
    pub grid: Grid,
    /// Percent, 0..=100.
    strength: u8,
    /// Percent, 0..=100.
    swing: u8,
    pub align: QuantizeAlign,
}

impl QuantizeSettings {
    pub fn new(grid: Grid, strength: u8, swing: u8, align: QuantizeAlign) -> Option<Self> {
        (strength <= 100 && swing <= 100).then_some(QuantizeSettings {
            grid,
            strength,
            swing,
            align,
        })
    }

    pub fn strength(self) -> u8 {
        self.strength
    }

    pub fn swing(self) -> u8 {
        self.swing
    }
}

impl Default for QuantizeSettings {
    fn default() -> QuantizeSettings {
        QuantizeSettings {
            grid: Grid::default(),
            strength: 100,
            swing: 0,
            align: QuantizeAlign::Start,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RollSettings {
    pub grid: Grid,
    pub scale: MusicalScale,
    pub quantize: QuantizeSettings,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales() {
        let c_major = MusicalScale::default();
        let keys: Vec<u8> = (60..72).filter(|&k| c_major.contains(k)).collect();
        assert_eq!(keys, [60, 62, 64, 65, 67, 69, 71]);
        assert!(!c_major.contains(63), "D# is outside C major");
        let a_minor = MusicalScale::new(9, ScaleMode::Minor, true).unwrap();
        let keys: Vec<u8> = (0..12).filter(|&k| a_minor.contains(k)).collect();
        assert_eq!(
            keys,
            [0, 2, 4, 5, 7, 9, 11],
            "A minor has the keys of C major"
        );
        let d_major = MusicalScale::new(2, ScaleMode::Major, false).unwrap();
        let keys: Vec<u8> = (0..12).filter(|&k| d_major.contains(k)).collect();
        assert_eq!(keys, [1, 2, 4, 6, 7, 9, 11]);
        assert!(MusicalScale::new(12, ScaleMode::Major, false).is_none());
        assert!(d_major.contains(127) == d_major.contains(127 % 12));
    }

    #[test]
    fn grid_divisions() {
        for d in [0, 2, 4, 8, 16, 32] {
            assert!(Grid::new(d, true).is_some());
        }
        for d in [1, 3, 6, 12, 64, 128] {
            assert!(Grid::new(d, false).is_none());
        }
        assert!(QuantizeSettings::new(Grid::default(), 101, 0, QuantizeAlign::Start).is_none());
        assert!(QuantizeSettings::new(Grid::default(), 100, 101, QuantizeAlign::Start).is_none());
    }
}

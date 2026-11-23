//! The arpeggio of a keys track (round 12, E2): the keys held on it play one at a time, in
//! the order of `shape`, every `rate`. Only the settings live here; they are part of the song.
//! The default is off, and a track with it off sounds as before.

/// The settings of 「アルペジオ」 of a track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Arp {
    pub on: bool,
    pub shape: ArpShape,
    pub rate: ArpRate,
    /// 「三連符」: each step 2/3 as long.
    pub triplet: bool,
    /// 「オクターブ」, 1..=`ARP_OCTAVES_MAX`.
    pub octaves: u8,
    /// 「指を離しても続ける」.
    pub hold: bool,
}

impl Default for Arp {
    fn default() -> Self {
        Arp {
            on: false,
            shape: ArpShape::Up,
            rate: ArpRate::Sixteenth,
            triplet: false,
            octaves: 1,
            hold: false,
        }
    }
}

impl Arp {
    pub fn is_valid(&self) -> bool {
        (1..=ARP_OCTAVES_MAX).contains(&self.octaves)
    }
}

/// 「形」.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArpShape {
    /// 「上へ」.
    Up,
    /// 「下へ」.
    Down,
    /// 「上下」.
    UpDown,
    /// 「ばらばら」.
    Random,
}

impl ArpShape {
    pub const ALL: [ArpShape; 4] = [
        ArpShape::Up,
        ArpShape::Down,
        ArpShape::UpDown,
        ArpShape::Random,
    ];
}

/// 「速さ」: one step of the arpeggio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArpRate {
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
}

impl ArpRate {
    pub const ALL: [ArpRate; 4] = [
        ArpRate::Quarter,
        ArpRate::Eighth,
        ArpRate::Sixteenth,
        ArpRate::ThirtySecond,
    ];

    /// How many steps to a whole note (4 is a quarter).
    pub fn division(self) -> u16 {
        match self {
            ArpRate::Quarter => 4,
            ArpRate::Eighth => 8,
            ArpRate::Sixteenth => 16,
            ArpRate::ThirtySecond => 32,
        }
    }

    pub fn from_division(division: u16) -> Option<ArpRate> {
        ArpRate::ALL.into_iter().find(|r| r.division() == division)
    }
}

/// The most octaves an arpeggio goes up.
pub const ARP_OCTAVES_MAX: u8 = 3;
/// The most keys held that the arpeggio plays; the keys pressed after them are left out.
pub const ARP_KEYS_MAX: usize = 16;
/// How long each note sounds, in percent of a step.
pub const ARP_GATE_PERCENT: u32 = 80;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_off_and_valid() {
        let arp = Arp::default();
        assert!(!arp.on && arp.is_valid());
        assert_eq!(
            (arp.shape, arp.rate, arp.octaves),
            (ArpShape::Up, ArpRate::Sixteenth, 1)
        );
    }

    #[test]
    fn rates_by_division() {
        for rate in ArpRate::ALL {
            assert_eq!(ArpRate::from_division(rate.division()), Some(rate));
        }
        assert_eq!(ArpRate::from_division(12), None);
        assert!(
            !Arp {
                octaves: 0,
                ..Arp::default()
            }
            .is_valid()
        );
        assert!(
            !Arp {
                octaves: 4,
                ..Arp::default()
            }
            .is_valid()
        );
    }
}

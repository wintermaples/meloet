//! The four knobs of a built-in sound (round 12, E3): 「明るさ」「立ち上がり」「のび」「ゆれ」,
//! each 0..=100 %. Part of the song, per track. At the default values a sound is as it was
//! before the knobs (to the sample). A lane moves knob `n` as `AutoTarget::Instrument { param:
//! n }` of a built-in sound.

/// A knob, in the order of the rows of the parameters panel; its index is its `param`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundKnob {
    Brightness,
    Attack,
    Release,
    Vibrato,
}

impl SoundKnob {
    pub const ALL: [SoundKnob; 4] = [
        SoundKnob::Brightness,
        SoundKnob::Attack,
        SoundKnob::Release,
        SoundKnob::Vibrato,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn from_index(index: u32) -> Option<SoundKnob> {
        SoundKnob::ALL.get(usize::try_from(index).ok()?).copied()
    }

    /// Where the knob starts, in percent.
    pub fn default_value(self) -> f32 {
        SOUND_KNOB_DEFAULTS[self.index()]
    }
}

/// 「明るさ」 50, 「立ち上がり」 50, 「のび」 50, 「ゆれ」 0.
pub const SOUND_KNOB_DEFAULTS: [f32; 4] = [50.0, 50.0, 50.0, 0.0];

/// The values of the four knobs, in percent, each 0..=100.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoundKnobs([f32; 4]);

impl Default for SoundKnobs {
    fn default() -> Self {
        SoundKnobs(SOUND_KNOB_DEFAULTS)
    }
}

impl SoundKnobs {
    pub fn get(&self, knob: SoundKnob) -> f32 {
        self.0[knob.index()]
    }

    /// With `knob` at `value`, put into 0..=100; `None` for a value that is not a number.
    pub fn with(self, knob: SoundKnob, value: f32) -> Option<SoundKnobs> {
        if value.is_nan() {
            return None;
        }
        let mut values = self.0;
        values[knob.index()] = value.clamp(0.0, 100.0);
        Some(SoundKnobs(values))
    }

    pub fn values(&self) -> [f32; 4] {
        self.0
    }

    pub fn is_default(&self) -> bool {
        self.0 == SOUND_KNOB_DEFAULTS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knobs_by_index_and_in_range() {
        for (i, knob) in SoundKnob::ALL.into_iter().enumerate() {
            assert_eq!(SoundKnob::from_index(i as u32), Some(knob));
            assert_eq!(knob.index(), i);
        }
        assert_eq!(SoundKnob::from_index(4), None);
        let knobs = SoundKnobs::default();
        assert!(knobs.is_default());
        assert_eq!(knobs.get(SoundKnob::Vibrato), 0.0);
        let k = knobs.with(SoundKnob::Brightness, 140.0).unwrap();
        assert_eq!(k.get(SoundKnob::Brightness), 100.0);
        assert!(!k.is_default());
        assert_eq!(knobs.with(SoundKnob::Attack, f32::NAN), None);
        assert_eq!(
            knobs
                .with(SoundKnob::Attack, -3.0)
                .unwrap()
                .get(SoundKnob::Attack),
            0.0
        );
    }
}

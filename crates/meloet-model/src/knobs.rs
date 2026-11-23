//! The four knobs of a built-in sound (round 12, E3): 「明るさ」「立ち上がり」「のび」「ゆれ」,
//! each 0..=100 %. Part of the song, per track. At the default values a sound is as it was
//! before the knobs (to the sample). A lane moves knob `n` as `AutoTarget::Instrument { param:
//! n }` of a built-in sound.

use std::sync::Arc;

use crate::TrackId;
use crate::edit::{Edit, EditError, track_index};
use crate::history::StepKind;
use crate::instrument::{InstrumentGroup, InstrumentRef};
use crate::song::Doc;

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

/// Whether `knob` does something to `instrument`: only to a sound that comes with meloet,
/// and of a drum kit only 「明るさ」 and 「のび」.
pub fn knob_usable(instrument: &InstrumentRef, knob: SoundKnob) -> bool {
    match instrument {
        InstrumentRef::Builtin(b) if b.group() == InstrumentGroup::Drums => {
            matches!(knob, SoundKnob::Brightness | SoundKnob::Release)
        }
        InstrumentRef::Builtin(_) => true,
        InstrumentRef::Sf2(_) | InstrumentRef::Plugin(_) => false,
    }
}

/// 「音のつまみを変える」: `knob` of the track at `value` percent, put into 0..=100.
/// `Unchanged` for a value the track has, one that is not a number, or a knob its sound
/// does not use.
pub fn set_sound_knob(
    doc: &Doc,
    track: TrackId,
    knob: SoundKnob,
    value: f32,
) -> Result<Edit, EditError> {
    let ti = track_index(doc, track)?;
    let t = &doc.tracks[ti];
    if !knob_usable(&t.instrument, knob) {
        return Err(EditError::Unchanged);
    }
    let knobs = t.knobs.with(knob, value).ok_or(EditError::Unchanged)?;
    if knobs == t.knobs {
        return Err(EditError::Unchanged);
    }
    let mut new = doc.clone();
    Arc::make_mut(&mut new.tracks[ti]).knobs = knobs;
    Ok(Edit {
        doc: new,
        step: StepKind::SetSoundKnob { track, knob },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BuiltinInstrument;

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

    const T1: TrackId = TrackId(1);

    fn with_sound(instrument: InstrumentRef) -> Doc {
        crate::edit::set_instrument(&Doc::new_default(), T1, instrument)
            .unwrap()
            .doc
    }

    #[test]
    fn a_knob_is_one_step_and_only_for_what_the_sound_uses() {
        let doc = Doc::new_default();
        let e = set_sound_knob(&doc, T1, SoundKnob::Brightness, 130.0).unwrap();
        assert_eq!(
            e.step,
            StepKind::SetSoundKnob {
                track: T1,
                knob: SoundKnob::Brightness
            }
        );
        assert_eq!(e.doc.tracks[0].knobs.get(SoundKnob::Brightness), 100.0);
        assert_eq!(
            set_sound_knob(&e.doc, T1, SoundKnob::Brightness, 100.0),
            Err(EditError::Unchanged)
        );
        assert_eq!(
            set_sound_knob(&doc, T1, SoundKnob::Attack, f32::NAN),
            Err(EditError::Unchanged)
        );
        assert!(set_sound_knob(&doc, TrackId(9), SoundKnob::Attack, 3.0).is_err());

        let kit = with_sound(InstrumentRef::Builtin(BuiltinInstrument::StudioKit));
        let usable = SoundKnob::ALL.map(|k| knob_usable(&kit.tracks[0].instrument, k));
        assert_eq!(usable, [true, false, true, false]);
        assert!(set_sound_knob(&kit, T1, SoundKnob::Release, 20.0).is_ok());
        assert_eq!(
            set_sound_knob(&kit, T1, SoundKnob::Vibrato, 20.0),
            Err(EditError::Unchanged)
        );
        let sine = InstrumentRef::SINE;
        assert!(SoundKnob::ALL.iter().all(|&k| knob_usable(&sine, k)));
    }

    #[test]
    fn another_sound_starts_the_knobs_again_and_takes_their_lanes() {
        use crate::automation::{AutoTarget, add_lane};
        let doc = Doc::new_default();
        let doc = set_sound_knob(&doc, T1, SoundKnob::Vibrato, 40.0)
            .unwrap()
            .doc;
        let vibrato = AutoTarget::Instrument { param: 3 };
        let (e, _) = add_lane(&doc, T1, vibrato, false).unwrap();
        let strings = InstrumentRef::Builtin(BuiltinInstrument::Strings);
        let after = crate::edit::set_instrument(&e.doc, T1, strings)
            .unwrap()
            .doc;
        assert!(after.tracks[0].knobs.is_default());
        assert!(after.tracks[0].lane(vibrato).is_none());
        after.check_lanes().unwrap();

        let kit = with_sound(InstrumentRef::Builtin(BuiltinInstrument::ElectroKit));
        assert!(add_lane(&kit, T1, AutoTarget::Instrument { param: 2 }, false).is_ok());
        assert!(add_lane(&kit, T1, AutoTarget::Instrument { param: 1 }, false).is_err());
    }
}

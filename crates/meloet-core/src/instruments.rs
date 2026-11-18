//! The instruments of the model as the screen names them.

use meloet_engine::{Instrument, PlaceholderSynth};
use meloet_model::{BuiltinInstrument, InstrumentRef};
use meloet_protocol::InstrumentKind;

/// The instruments whose sound is in meloet, in the order of the shelf: the rows the shelf
/// shows.
pub(crate) fn available() -> Vec<BuiltinInstrument> {
    #[cfg(test)]
    if tests::EVERY_SOUND.get() {
        return BuiltinInstrument::all().collect();
    }
    vec![BuiltinInstrument::Sine]
}

/// The one place where instruments are made. Call it outside the audio thread; the engine's
/// handle prepares what it gets. An instrument whose sound is not in meloet yet sounds as the
/// sine wave, so that a track with it is never silent.
pub(crate) fn make(instrument: &InstrumentRef) -> Box<dyn Instrument> {
    match instrument {
        InstrumentRef::Builtin(_) => Box::new(PlaceholderSynth::new()),
    }
}

pub(crate) fn kind_of(instrument: BuiltinInstrument) -> InstrumentKind {
    use BuiltinInstrument as B;
    use InstrumentKind as K;
    match instrument {
        B::GrandPiano => K::GrandPiano,
        B::ElectricPiano => K::ElectricPiano,
        B::Organ => K::Organ,
        B::FingerBass => K::FingerBass,
        B::SynthBass => K::SynthBass,
        B::StudioKit => K::StudioKit,
        B::ElectroKit => K::ElectroKit,
        B::Strings => K::Strings,
        B::WarmPad => K::WarmPad,
        B::SynthLead => K::SynthLead,
        B::Sine => K::Sine,
    }
}

pub(crate) fn builtin_of(kind: InstrumentKind) -> BuiltinInstrument {
    use BuiltinInstrument as B;
    use InstrumentKind as K;
    match kind {
        K::GrandPiano => B::GrandPiano,
        K::ElectricPiano => B::ElectricPiano,
        K::Organ => B::Organ,
        K::FingerBass => B::FingerBass,
        K::SynthBass => B::SynthBass,
        K::StudioKit => B::StudioKit,
        K::ElectroKit => B::ElectroKit,
        K::Strings => B::Strings,
        K::WarmPad => B::WarmPad,
        K::SynthLead => B::SynthLead,
        K::Sine => B::Sine,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use meloet_model::InstrumentGroup as ModelGroup;
    use meloet_protocol::InstrumentGroup;

    use super::*;

    thread_local! {
        /// For the tests of this thread, every instrument counts as being in meloet.
        pub(crate) static EVERY_SOUND: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }

    /// The drum rows of the protocol (and so of the screen) are the model's: order, ids, keys.
    #[test]
    fn the_drum_rows_agree() {
        let model: Vec<(&str, u8)> = meloet_model::drums::DrumPiece::ROWS
            .iter()
            .map(|p| (p.id(), p.note()))
            .collect();
        assert_eq!(model, meloet_protocol::DRUM_ROWS);
    }

    /// The model's table and the protocol's say the same: order, names and groups.
    #[test]
    fn the_model_and_the_protocol_agree() {
        let model: Vec<BuiltinInstrument> = BuiltinInstrument::all().collect();
        assert_eq!(model.len(), InstrumentKind::ALL.len());
        for (b, k) in model.into_iter().zip(InstrumentKind::ALL) {
            assert_eq!(kind_of(b), k);
            assert_eq!(builtin_of(k), b);
            assert_eq!(serde_json::to_value(k).unwrap(), b.name());
            let group = match b.group() {
                ModelGroup::PianoKeys => InstrumentGroup::PianoKeys,
                ModelGroup::Bass => InstrumentGroup::Bass,
                ModelGroup::Drums => InstrumentGroup::Drums,
                ModelGroup::PadStrings => InstrumentGroup::PadStrings,
                ModelGroup::Synth => InstrumentGroup::Synth,
            };
            assert_eq!(k.group(), group);
        }
    }
}

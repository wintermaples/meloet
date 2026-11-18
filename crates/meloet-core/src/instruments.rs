//! The instruments of the model as the screen names them.

use meloet_model::BuiltinInstrument;
use meloet_protocol::InstrumentKind;

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
mod tests {
    use meloet_model::InstrumentGroup as ModelGroup;
    use meloet_protocol::InstrumentGroup;

    use super::*;

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

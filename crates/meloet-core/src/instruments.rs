//! The instruments of the model as the screen names them, which of them this PC can sound,
//! and the one place where they are made.

use std::sync::Arc;

use meloet_engine::{Instrument, InstrumentEvent, PlaceholderSynth};
use meloet_model::{BuiltinInstrument, InstrumentRef};
use meloet_protocol::InstrumentKind;
use meloet_sound::sampler::{SampleBank, Sampler};
use meloet_sound::synth::{ElectroKit, PolySynth};

/// Whether the instrument sounds with samples of the sound pack. The others are computed and
/// are always there.
pub(crate) fn sampled(instrument: BuiltinInstrument) -> bool {
    use BuiltinInstrument as B;
    match instrument {
        B::GrandPiano | B::ElectricPiano | B::Organ | B::FingerBass | B::StudioKit | B::Strings => {
            true
        }
        B::SynthBass | B::ElectroKit | B::WarmPad | B::SynthLead | B::Sine => false,
    }
}

/// What the rules know about the samples: which sampled instruments the pack on this PC has,
/// and which of those are in memory or on their way there. The samples themselves are with
/// the core thread ([`crate::sounds::Library`]).
#[derive(Debug, Default)]
pub(crate) struct Sounds {
    pub installed: Vec<BuiltinInstrument>,
    pub loaded: Vec<BuiltinInstrument>,
    pub loading: Vec<BuiltinInstrument>,
}

impl Sounds {
    /// The instruments that can sound on this PC, in the order of the shelf: the rows the
    /// shelf shows.
    pub fn available(&self) -> Vec<BuiltinInstrument> {
        #[cfg(test)]
        if tests::EVERY_SOUND.get() {
            return BuiltinInstrument::all().collect();
        }
        BuiltinInstrument::all()
            .filter(|i| !sampled(*i) || self.installed.contains(i))
            .collect()
    }

    /// Whether an instrument made now sounds: it is computed, its samples are in memory, or
    /// the pack does not have it (it then sounds as the sine wave).
    pub fn ready(&self, instrument: BuiltinInstrument) -> bool {
        !self.installed.contains(&instrument) || self.loaded.contains(&instrument)
    }
}

/// The samples of an instrument, as far as they are there when it is made.
pub(crate) enum Samples {
    Ready(Arc<SampleBank>),
    /// The pack has them, and they are not in memory (yet).
    NotRead,
    NotInstalled,
}

/// Sounds nothing: on a seat until the samples of its instrument are read.
struct Silent;

impl Instrument for Silent {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn event(&mut self, _ev: InstrumentEvent) {}
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        left.fill(0.0);
        right.fill(0.0);
    }
    fn reset(&mut self) {}
}

/// The one place where instruments are made. Call it outside the audio thread; the engine's
/// handle prepares what it gets. `samples`: those of this instrument. A sampled instrument
/// the pack does not have (a song from a PC that has it) sounds as the sine wave, so that
/// its track is not silent and nobody takes its notes for lost.
pub(crate) fn make(instrument: &InstrumentRef, samples: Samples) -> Box<dyn Instrument> {
    use BuiltinInstrument as B;
    let InstrumentRef::Builtin(builtin) = instrument;
    match (builtin, samples) {
        (B::SynthLead, _) => Box::new(PolySynth::lead()),
        (B::SynthBass, _) => Box::new(PolySynth::bass()),
        (B::WarmPad, _) => Box::new(PolySynth::pad()),
        (B::ElectroKit, _) => Box::new(ElectroKit::new()),
        (B::Sine, _) => Box::new(PlaceholderSynth::new()),
        (_, Samples::Ready(bank)) => Box::new(Sampler::new(bank)),
        (_, Samples::NotRead) => Box::new(Silent),
        (_, Samples::NotInstalled) => Box::new(PlaceholderSynth::new()),
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

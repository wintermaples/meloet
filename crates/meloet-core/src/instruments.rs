//! The instruments of the model as the screen names them, which of them this PC can sound,
//! and the one place where they are made.

use std::sync::Arc;

use meloet_engine::{Instrument, InstrumentEvent, PlaceholderSynth};
use meloet_model::{BuiltinInstrument, InstrumentRef};
use meloet_protocol::InstrumentKind;
use meloet_sound::knobs::Knobbed;
use meloet_sound::sampler::{SampleBank, Sampler};
use meloet_sound::synth::{ElectroKit, PolySynth};

use crate::sf2store::Sf2Key;

/// Whether the instrument sounds with samples of the sound pack. The others are computed and
/// are always there.
pub(crate) fn sampled(instrument: BuiltinInstrument) -> bool {
    use BuiltinInstrument as B;
    !matches!(
        instrument,
        B::SynthBass | B::ElectroKit | B::WarmPad | B::SynthLead | B::Sine
    )
}

/// What the rules know about the samples: which sampled instruments the pack on this PC has,
/// and which of those are in memory or on their way there. The samples themselves are with
/// the core thread ([`crate::sounds::Library`]).
#[derive(Debug, Default)]
pub(crate) struct Sounds {
    pub installed: Vec<BuiltinInstrument>,
    pub loaded: Vec<BuiltinInstrument>,
    pub loading: Vec<BuiltinInstrument>,
    /// The presets of the SF2 files that were added, and which of them are in memory or on
    /// their way there.
    pub sf2_listed: Vec<Sf2Key>,
    pub sf2_loaded: Vec<Sf2Key>,
    pub sf2_loading: Vec<Sf2Key>,
    /// The sampled instruments the catalog of the sound pack has: the ones not here yet can
    /// be fetched (`ShelfView::not_yet`).
    pub fetchable: Vec<BuiltinInstrument>,
    /// The last fetch found the pack broken (spec 3, 45-A: only then a new song starts with
    /// the sine wave).
    pub broken: bool,
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

    /// The same for an SF2 preset: its samples are in memory, or no added file has it.
    pub fn sf2_ready(&self, key: &Sf2Key) -> bool {
        !self.sf2_listed.contains(key) || self.sf2_loaded.contains(key)
    }

    /// The rows of the shelf's own instruments. Without the sound pack and without a way to
    /// fetch it only the sine wave is offered (spec 3, 85), though the other computed sounds
    /// still sound in a song. While it can be fetched, the computed ones are there and the
    /// rest waits in [`Sounds::not_yet`] (45-A).
    pub fn shelf_rows(&self) -> Vec<BuiltinInstrument> {
        let available = self.available();
        if available.iter().any(|i| sampled(*i)) || !self.not_yet().is_empty() {
            available
        } else {
            vec![BuiltinInstrument::Sine]
        }
    }

    /// The instruments the pack can bring and that are not here, in the order of the shelf.
    pub fn not_yet(&self) -> Vec<BuiltinInstrument> {
        let available = self.available();
        BuiltinInstrument::all()
            .filter(|i| self.fetchable.contains(i) && !available.contains(i))
            .collect()
    }
}

/// The samples of an instrument, as far as they are there when it is made.
#[derive(Clone)]
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
///
/// An SF2 preset goes the same way with the samples of its file. A plugin is not made here:
/// until it is loaded and put on the seat (`EngineOp::Plug`) its seat is silent. A plugin
/// this PC does not have never comes here; the rules ask for the sine wave in its place.
/// A sound that comes with meloet has its knobs (round 12, E3); an SF2 preset has none.
pub(crate) fn make(instrument: &InstrumentRef, samples: Samples) -> Box<dyn Instrument> {
    use BuiltinInstrument as B;
    let builtin = match instrument {
        InstrumentRef::Builtin(b) => *b,
        InstrumentRef::Plugin(_) => return Box::new(Silent),
        InstrumentRef::Sf2(_) => {
            return match samples {
                Samples::Ready(bank) => Box::new(Sampler::new(bank)),
                Samples::NotRead => Box::new(Silent),
                Samples::NotInstalled => Box::new(PlaceholderSynth::new()),
            };
        }
    };
    match (builtin, samples) {
        (B::SynthLead, _) => Box::new(Knobbed::new(PolySynth::lead())),
        (B::SynthBass, _) => Box::new(Knobbed::new(PolySynth::bass())),
        (B::WarmPad, _) => Box::new(Knobbed::new(PolySynth::pad())),
        (B::ElectroKit, _) => Box::new(Knobbed::new(ElectroKit::new())),
        (B::Sine, _) => Box::new(Knobbed::new(PlaceholderSynth::new())),
        (_, Samples::Ready(bank)) => Box::new(Knobbed::new(Sampler::new(bank))),
        (_, Samples::NotRead) => Box::new(Silent),
        (_, Samples::NotInstalled) => Box::new(Knobbed::new(PlaceholderSynth::new())),
    }
}

pub(crate) fn kind_of(instrument: BuiltinInstrument) -> InstrumentKind {
    use BuiltinInstrument as B;
    use InstrumentKind as K;
    match instrument {
        B::GrandPiano => K::GrandPiano,
        B::ElectricPiano => K::ElectricPiano,
        B::OrganDrawbar => K::OrganDrawbar,
        B::OrganPercussive => K::OrganPercussive,
        B::OrganRock => K::OrganRock,
        B::Organ => K::Organ,
        B::OrganQuiet => K::OrganQuiet,
        B::OrganChurch => K::OrganChurch,
        B::OrganReed => K::OrganReed,
        B::Vibraphone => K::Vibraphone,
        B::Marimba => K::Marimba,
        B::GuitarNylon => K::GuitarNylon,
        B::GuitarSteel => K::GuitarSteel,
        B::GuitarElectric => K::GuitarElectric,
        B::GuitarDistortion => K::GuitarDistortion,
        B::FingerBass => K::FingerBass,
        B::PickBass => K::PickBass,
        B::WoodBass => K::WoodBass,
        B::SynthBass => K::SynthBass,
        B::StudioKit => K::StudioKit,
        B::ElectroKit => K::ElectroKit,
        B::BrushKit => K::BrushKit,
        B::Strings => K::Strings,
        B::Pizzicato => K::Pizzicato,
        B::WarmPad => K::WarmPad,
        B::Trumpet => K::Trumpet,
        B::BrassSection => K::BrassSection,
        B::AltoSax => K::AltoSax,
        B::Flute => K::Flute,
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
        K::OrganDrawbar => B::OrganDrawbar,
        K::OrganPercussive => B::OrganPercussive,
        K::OrganRock => B::OrganRock,
        K::Organ => B::Organ,
        K::OrganQuiet => B::OrganQuiet,
        K::OrganChurch => B::OrganChurch,
        K::OrganReed => B::OrganReed,
        K::Vibraphone => B::Vibraphone,
        K::Marimba => B::Marimba,
        K::GuitarNylon => B::GuitarNylon,
        K::GuitarSteel => B::GuitarSteel,
        K::GuitarElectric => B::GuitarElectric,
        K::GuitarDistortion => B::GuitarDistortion,
        K::FingerBass => B::FingerBass,
        K::PickBass => B::PickBass,
        K::WoodBass => B::WoodBass,
        K::SynthBass => B::SynthBass,
        K::StudioKit => B::StudioKit,
        K::ElectroKit => B::ElectroKit,
        K::BrushKit => B::BrushKit,
        K::Strings => B::Strings,
        K::Pizzicato => B::Pizzicato,
        K::WarmPad => B::WarmPad,
        K::Trumpet => B::Trumpet,
        K::BrassSection => B::BrassSection,
        K::AltoSax => B::AltoSax,
        K::Flute => B::Flute,
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
        // The three kits are the drums group, so they all have these 12 rows.
        let kits: Vec<InstrumentKind> = InstrumentKind::ALL
            .into_iter()
            .filter(|k| k.group() == InstrumentGroup::Drums)
            .collect();
        assert_eq!(
            kits,
            [
                InstrumentKind::StudioKit,
                InstrumentKind::ElectroKit,
                InstrumentKind::BrushKit
            ]
        );
        assert_eq!(meloet_protocol::DRUM_ROWS.len(), 12);
    }

    /// Five instruments are computed; the other 26 come from the sound pack.
    #[test]
    fn which_instruments_are_sampled() {
        use BuiltinInstrument as B;
        let computed: Vec<B> = B::all().filter(|i| !sampled(*i)).collect();
        assert_eq!(
            computed,
            [
                B::SynthBass,
                B::ElectroKit,
                B::WarmPad,
                B::SynthLead,
                B::Sine
            ]
        );
        assert_eq!(B::all().filter(|i| sampled(*i)).count(), 26);
        assert!(sampled(B::BrushKit) && sampled(B::Organ) && sampled(B::Flute));
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
                ModelGroup::Guitar => InstrumentGroup::Guitar,
                ModelGroup::Bass => InstrumentGroup::Bass,
                ModelGroup::Drums => InstrumentGroup::Drums,
                ModelGroup::PadStrings => InstrumentGroup::PadStrings,
                ModelGroup::Wind => InstrumentGroup::Wind,
                ModelGroup::Synth => InstrumentGroup::Synth,
            };
            assert_eq!(k.group(), group);
        }
    }
}

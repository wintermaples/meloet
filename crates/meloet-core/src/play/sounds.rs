//! The samples the song and the shelf need in memory, and the sample (▶) of the shelf.
//!
//! An instrument whose samples are not read yet is on its seat all the same, silent; when
//! they are read, every seat with that instrument gets one made anew, which sounds. The
//! sounds of meloet and the presets of SF2 files go the same way.

use meloet_model::{BuiltinInstrument, InstrumentGroup, InstrumentRef};
use meloet_protocol::{InstrumentKind, SampleView, StatusMessage};

use super::plugs::Spot;
use super::seats::AUDITION_SEAT;
use super::shelf::Mode;
use super::{EngineOp, Play};
use crate::added::sound_name;
use crate::instruments;
use crate::sf2store::Sf2Key;
use crate::state::Effect;

/// The two bars of a sample phrase at tempo 120: what "0:02 / 0:04" counts to.
pub(super) const SAMPLE_US: u64 = (meloet_sound::phrases::PHRASE_S * 1e6) as u64;
/// What the last notes are given to die away in before the sample counts as over.
const SAMPLE_TAIL_US: u64 = (meloet_sound::phrases::TAIL_S * 1e6) as u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Sample {
    pub(super) instrument: InstrumentRef,
    /// When its first sound reaches the ear.
    pub(super) started_us: u64,
}

/// ▶ on an instrument that is still being read: it plays once it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SampleWaiting {
    pub(super) instrument: InstrumentRef,
    pub(super) latency_us: u64,
}

fn builtin(instrument: &InstrumentRef) -> Option<BuiltinInstrument> {
    match instrument {
        InstrumentRef::Builtin(b) => Some(*b),
        InstrumentRef::Plugin(_) | InstrumentRef::Sf2(_) => None,
    }
}

fn sf2_key(instrument: &InstrumentRef) -> Option<Sf2Key> {
    match instrument {
        InstrumentRef::Sf2(s) => Some(Sf2Key::of(s)),
        InstrumentRef::Builtin(_) | InstrumentRef::Plugin(_) => None,
    }
}

impl Play {
    /// The sampled instruments the sound pack of this PC has. Told once, at the start.
    pub fn set_installed_sounds(&mut self, installed: Vec<BuiltinInstrument>) {
        self.sounds.installed = installed;
    }

    /// The sounds of meloet that can sound on this PC.
    pub fn available_sounds(&self) -> Vec<BuiltinInstrument> {
        self.sounds.available()
    }

    /// None of the sounds that need the sound pack can sound: the pack is not found.
    pub fn pack_missing(&self) -> bool {
        !(self.sounds.available().into_iter()).any(instruments::sampled)
    }

    /// The presets the added SF2 files have. Told at the start.
    pub fn set_sf2_presets(&mut self, listed: Vec<Sf2Key>) {
        self.sounds.sf2_listed = listed;
    }

    /// An SF2 file was added, maybe in place of one of the same name: what was read from
    /// that one is let go, and the tracks that name the file sound with the new one.
    pub fn sf2_added(&mut self, file: &str, presets: Vec<Sf2Key>, effects: &mut Vec<Effect>) {
        let sounds = &mut self.sounds;
        sounds.sf2_listed.retain(|k| &*k.file != file);
        sounds.sf2_listed.extend(presets);
        for list in [&mut sounds.sf2_loaded, &mut sounds.sf2_loading] {
            list.retain(|k| {
                let keep = &*k.file != file;
                if !keep {
                    effects.push(Effect::DropSf2(k.clone()));
                }
                keep
            });
        }
        self.remake(
            |i| matches!(i, InstrumentRef::Sf2(s) if &*s.file == file),
            effects,
        );
    }

    /// The instruments whose samples are to be in memory: the ones of the song's tracks, the
    /// row that is auditioned and the one before it, and the sample that plays.
    fn wanted_instruments(&self) -> impl Iterator<Item = &InstrumentRef> {
        let shelf = &self.shelf;
        let tracks = self.doc().tracks.iter().map(|t| &t.instrument);
        let rows = shelf.trial.iter().chain(&shelf.before);
        let samples = (shelf.sample.iter().map(|s| &s.instrument))
            .chain(shelf.sample_waiting.iter().map(|w| &w.instrument));
        tracks.chain(rows).chain(samples)
    }

    fn wanted_sounds(&self) -> Vec<BuiltinInstrument> {
        let mut wanted = Vec::new();
        for b in self.wanted_instruments().filter_map(builtin) {
            if self.sounds.installed.contains(&b) && !wanted.contains(&b) {
                wanted.push(b);
            }
        }
        wanted
    }

    fn wanted_sf2(&self) -> Vec<Sf2Key> {
        let mut wanted = Vec::new();
        for key in self.wanted_instruments().filter_map(sf2_key) {
            if self.sounds.sf2_listed.contains(&key) && !wanted.contains(&key) {
                wanted.push(key);
            }
        }
        wanted
    }

    /// Whether an instrument made now for a seat sounds. A plugin does when the one at
    /// `spot` is loaded; one this PC does not have counts as sounding (as the sine wave).
    pub(super) fn sounds_now(&self, instrument: &InstrumentRef, spot: Spot) -> bool {
        match instrument {
            InstrumentRef::Builtin(b) => self.sounds.ready(*b),
            InstrumentRef::Sf2(s) => self.sounds.sf2_ready(&Sf2Key::of(s)),
            InstrumentRef::Plugin(_) => !self.plug_loading(spot),
        }
    }

    /// The instrument of the track the keys play, while it is still being read: a key played
    /// now is silent. Not while a row is auditioned; the shelf says it then.
    pub fn target_loading(&self) -> Option<InstrumentRef> {
        if self.auditioning() {
            return None;
        }
        let track = self.doc().track(self.song.session.play_target?)?;
        let seat = self.seats.of(track.id)?;
        Some(track.instrument.clone()).filter(|i| !self.sounds_now(i, Spot::Seat(seat)))
    }

    /// Asks for the samples that are wanted and not there, and lets go of the ones nothing
    /// wants any more. After every input.
    pub fn sync_sounds(&mut self, effects: &mut Vec<Effect>) {
        let wanted = self.wanted_sounds();
        let wanted_sf2 = self.wanted_sf2();
        let sounds = &mut self.sounds;
        for list in [&mut sounds.loaded, &mut sounds.loading] {
            list.retain(|b| {
                let keep = wanted.contains(b);
                if !keep {
                    effects.push(Effect::DropSound(*b));
                }
                keep
            });
        }
        for b in wanted {
            if !sounds.loaded.contains(&b) && !sounds.loading.contains(&b) {
                sounds.loading.push(b);
                effects.push(Effect::LoadSound(b));
            }
        }
        for list in [&mut sounds.sf2_loaded, &mut sounds.sf2_loading] {
            list.retain(|key| {
                let keep = wanted_sf2.contains(key);
                if !keep {
                    effects.push(Effect::DropSf2(key.clone()));
                }
                keep
            });
        }
        for key in wanted_sf2 {
            if !sounds.sf2_loaded.contains(&key) && !sounds.sf2_loading.contains(&key) {
                sounds.sf2_loading.push(key.clone());
                effects.push(Effect::LoadSf2(key));
            }
        }
    }

    /// The answer to [`Effect::LoadSound`]: the samples are in memory.
    pub fn sound_loaded(&mut self, instrument: BuiltinInstrument, effects: &mut Vec<Effect>) {
        let sounds = &mut self.sounds;
        if sounds.loaded.contains(&instrument) {
            return;
        }
        if !sounds.loading.contains(&instrument) {
            // Nothing wants them any more: they were let go of while they were read.
            effects.push(Effect::DropSound(instrument));
            return;
        }
        sounds.loading.retain(|b| *b != instrument);
        sounds.loaded.push(instrument);
        let made = InstrumentRef::Builtin(instrument);
        self.remake(|i| *i == made, effects);
        self.start_waiting_sample(|i| *i == made, effects);
    }

    /// The other answer to [`Effect::LoadSound`]: the pack counts as not having the
    /// instrument from now on. It leaves the shelf, and tracks with it sound as the sine wave.
    pub fn sound_unreadable(&mut self, instrument: BuiltinInstrument, effects: &mut Vec<Effect>) {
        let sounds = &mut self.sounds;
        if !sounds.installed.contains(&instrument) {
            return;
        }
        for list in [
            &mut sounds.installed,
            &mut sounds.loaded,
            &mut sounds.loading,
        ] {
            list.retain(|b| *b != instrument);
        }
        let gone = InstrumentRef::Builtin(instrument);
        self.gone_from_shelf(|i| *i == gone, effects);
    }

    /// The answer to [`Effect::LoadSf2`]: the samples of the preset are in memory.
    pub fn sf2_loaded(&mut self, key: &Sf2Key, effects: &mut Vec<Effect>) {
        let sounds = &mut self.sounds;
        if sounds.sf2_loaded.contains(key) {
            return;
        }
        if !sounds.sf2_loading.contains(key) {
            effects.push(Effect::DropSf2(key.clone()));
            return;
        }
        sounds.sf2_loading.retain(|k| k != key);
        sounds.sf2_loaded.push(key.clone());
        let is = |i: &InstrumentRef| sf2_key(i).as_ref() == Some(key);
        self.remake(is, effects);
        self.start_waiting_sample(is, effects);
    }

    /// The other answer to [`Effect::LoadSf2`]: the file counts as not having the preset
    /// from now on.
    pub fn sf2_unreadable(&mut self, key: &Sf2Key, effects: &mut Vec<Effect>) {
        let sounds = &mut self.sounds;
        if !sounds.sf2_listed.contains(key) {
            return;
        }
        for list in [
            &mut sounds.sf2_listed,
            &mut sounds.sf2_loaded,
            &mut sounds.sf2_loading,
        ] {
            list.retain(|k| k != key);
        }
        self.gone_from_shelf(|i| sf2_key(i).as_ref() == Some(key), effects);
    }

    /// An instrument turned out not to read: the shelf lets go of it, and the seats that
    /// hold it get the sine wave in its place.
    fn gone_from_shelf(&mut self, is: impl Fn(&InstrumentRef) -> bool, effects: &mut Vec<Effect>) {
        let shelf = &mut self.shelf;
        shelf.before.take_if(|b| is(b));
        shelf.sample_waiting.take_if(|w| is(&w.instrument));
        if shelf.trial.as_ref().is_some_and(&is) {
            self.end_audition(effects);
        }
        self.remake(is, effects);
    }

    fn start_waiting_sample(
        &mut self,
        is: impl Fn(&InstrumentRef) -> bool,
        effects: &mut Vec<Effect>,
    ) {
        if let Some(waiting) = self.shelf.sample_waiting.take_if(|w| is(&w.instrument)) {
            self.start_sample(waiting.instrument, waiting.latency_us, effects);
        }
    }

    /// Every seat that holds an instrument `is` says yes to gets one made now, in place of
    /// the one made before its samples were there (or before they turned out not to read).
    fn remake(&mut self, is: impl Fn(&InstrumentRef) -> bool, effects: &mut Vec<Effect>) {
        let trial = self.shelf.trial.clone();
        let trial_seat = match (self.shelf.mode, &trial) {
            (Mode::Replace(track), Some(_)) => self.seats.of(track),
            _ => None,
        };
        let plugin_tried = matches!(trial, Some(InstrumentRef::Plugin(_)));
        let mut ops = Vec::new();
        for (seat, _, own) in self.seats.taken() {
            if !is(own) {
                continue;
            }
            let made = EngineOp::Instrument {
                seat,
                instrument: own.clone(),
            };
            match &trial {
                // A plugin that is tried cannot step off and on again: the track's own
                // instrument is made anew when the trial ends.
                Some(_) if trial_seat == Some(seat) && plugin_tried => {}
                // The track's own one is set aside under a trial: the trial steps off for
                // the new one and on again.
                Some(tried) if trial_seat == Some(seat) => ops.extend([
                    EngineOp::EndTrial { seat, keep: false },
                    made,
                    EngineOp::TryInstrument {
                        seat,
                        instrument: tried.clone(),
                    },
                ]),
                _ => ops.push(made),
            }
        }
        if let Some(tried) = trial.filter(|t| is(t)) {
            match (self.shelf.mode, trial_seat) {
                (Mode::NewTrack, _) => ops.push(EngineOp::Instrument {
                    seat: AUDITION_SEAT,
                    instrument: tried,
                }),
                // When it is the track's own too, it was made anew just above.
                (Mode::Replace(_), Some(seat)) if self.seats.instrument(seat) != Some(&tried) => {
                    ops.push(EngineOp::TryInstrument {
                        seat,
                        instrument: tried,
                    });
                }
                (Mode::Replace(_), _) | (Mode::InsertEffect(..), _) => {}
            }
        }
        effects.extend(ops.into_iter().map(Effect::Engine));
    }

    /// `PlaySample`. `open`: an output is open; nothing plays without one. `latency_us`: how
    /// long a sound takes from the engine to the ear.
    pub fn play_sample(
        &mut self,
        instrument: InstrumentKind,
        open: bool,
        latency_us: u64,
        effects: &mut Vec<Effect>,
    ) {
        let instrument = instruments::builtin_of(instrument);
        if self.sounds.available().contains(&instrument) {
            let group = instrument.group();
            let instrument = InstrumentRef::Builtin(instrument);
            self.play_sample_of(instrument, group, open, latency_us, effects);
        }
    }

    /// `PlaySampleOf`: the sample of any sound the shelf has. `group`: the phrase a plugin
    /// plays; the others play the phrase of their own group.
    pub fn play_sample_of(
        &mut self,
        instrument: InstrumentRef,
        group: InstrumentGroup,
        open: bool,
        latency_us: u64,
        effects: &mut Vec<Effect>,
    ) {
        if !open {
            return;
        }
        if let InstrumentRef::Plugin(plugin) = &instrument {
            // An instance of its own: it is loaded first.
            self.stop_sample(effects);
            self.sample_plugin(plugin, group, effects);
            if self.plug_loading(Spot::Sample) {
                self.shelf.sample_waiting = Some(SampleWaiting {
                    instrument,
                    latency_us,
                });
            }
        } else if self.sounds_now(&instrument, Spot::Sample) {
            self.shelf.sample_waiting = None;
            self.start_sample(instrument, latency_us, effects);
        } else {
            self.stop_sample(effects);
            self.shelf.sample_waiting = Some(SampleWaiting {
                instrument,
                latency_us,
            });
        }
    }

    /// The engine lets a sample that is playing fade out under the new one.
    fn start_sample(
        &mut self,
        instrument: InstrumentRef,
        latency_us: u64,
        effects: &mut Vec<Effect>,
    ) {
        self.drop_plug(Spot::Sample, effects);
        effects.push(Effect::Engine(EngineOp::PlaySample {
            instrument: instrument.clone(),
        }));
        self.shelf.sample = Some(Sample {
            instrument,
            started_us: self.now_us + latency_us,
        });
    }

    /// `StopSample`, and whatever else ends a sample: the shelf closes, the output goes away.
    pub fn stop_sample(&mut self, effects: &mut Vec<Effect>) {
        self.shelf.sample_waiting = None;
        self.drop_plug(Spot::Sample, effects);
        if self.shelf.sample.take().is_some() {
            effects.push(Effect::Engine(EngineOp::StopSample));
        }
    }

    /// Ends the sample when its two bars and its tail are over. After every input.
    pub fn sync_sample(&mut self, effects: &mut Vec<Effect>) {
        let over = |s: &Sample| self.now_us >= s.started_us + SAMPLE_US + SAMPLE_TAIL_US;
        if self.shelf.sample.as_ref().is_some_and(over) {
            self.stop_sample(effects);
        }
    }

    /// A sample plays: its end needs the time to be told ([`crate::state::Input::Tick`]).
    pub fn sample_playing(&self) -> bool {
        self.shelf.sample.is_some()
    }

    /// The sample of a sound of meloet, as the screen had it before 「あとから入れたもの」.
    pub(super) fn sample_view(&self) -> Option<SampleView> {
        let s = self.shelf.sample.as_ref()?;
        Some(SampleView {
            instrument: instruments::kind_of(builtin(&s.instrument)?),
            started_us: s.started_us,
            length_us: SAMPLE_US,
        })
    }

    /// The status line while a sample plays and no row is auditioned.
    pub fn sample_status(&self) -> Option<StatusMessage> {
        let playing = &self.shelf.sample.as_ref()?.instrument;
        Some(match builtin(playing) {
            Some(b) => StatusMessage::SamplePlaying {
                instrument: instruments::kind_of(b),
            },
            None => StatusMessage::SamplePlayingSound {
                sound: sound_name(playing),
            },
        })
    }
}

#[cfg(test)]
mod tests;

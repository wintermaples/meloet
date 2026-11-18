//! The samples the song and the shelf need in memory, and the sample (▶) of the shelf.
//!
//! An instrument whose samples are not read yet is on its seat all the same, silent; when
//! they are read, every seat with that instrument gets one made anew, which sounds.

use meloet_model::{BuiltinInstrument, InstrumentRef};
use meloet_protocol::{InstrumentKind, SampleView, StatusMessage};

use super::seats::AUDITION_SEAT;
use super::shelf::Mode;
use super::{EngineOp, Play};
use crate::instruments;
use crate::state::Effect;

/// The two bars of a sample phrase at tempo 120: what "0:02 / 0:04" counts to.
const SAMPLE_US: u64 = (meloet_sound::phrases::PHRASE_S * 1e6) as u64;
/// What the last notes are given to die away in before the sample counts as over.
const SAMPLE_TAIL_US: u64 = (meloet_sound::phrases::TAIL_S * 1e6) as u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Sample {
    instrument: BuiltinInstrument,
    /// When its first sound reaches the ear.
    started_us: u64,
}

/// ▶ on an instrument whose samples are still being read: it plays once they are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SampleWaiting {
    instrument: BuiltinInstrument,
    latency_us: u64,
}

fn builtin(instrument: &InstrumentRef) -> BuiltinInstrument {
    let InstrumentRef::Builtin(b) = instrument;
    *b
}

impl Play {
    /// The sampled instruments the sound pack of this PC has. Told once, at the start.
    pub fn set_installed_sounds(&mut self, installed: Vec<BuiltinInstrument>) {
        self.sounds.installed = installed;
    }

    /// The instruments whose samples are to be in memory: the ones of the song's tracks, the
    /// row that is auditioned and the one before it, and the sample that plays.
    fn wanted_sounds(&self) -> Vec<BuiltinInstrument> {
        let shelf = &self.shelf;
        let tracks = self.doc().tracks.iter().map(|t| &t.instrument);
        let rows = shelf.trial.iter().chain(&shelf.before);
        let samples = (shelf.sample.map(|s| s.instrument).into_iter())
            .chain(shelf.sample_waiting.map(|w| w.instrument));
        let mut wanted = Vec::new();
        for b in tracks.chain(rows).map(builtin).chain(samples) {
            if self.sounds.installed.contains(&b) && !wanted.contains(&b) {
                wanted.push(b);
            }
        }
        wanted
    }

    /// The instrument of the track the keys play, while its samples are still being read: a
    /// key played now is silent. Not while a row is auditioned; the shelf says it then.
    pub fn target_loading(&self) -> Option<BuiltinInstrument> {
        if self.auditioning() {
            return None;
        }
        let track = self.doc().track(self.song.session.play_target?)?;
        Some(builtin(&track.instrument)).filter(|b| !self.sounds.ready(*b))
    }

    /// Asks for the samples that are wanted and not there, and lets go of the ones nothing
    /// wants any more. After every input.
    pub fn sync_sounds(&mut self, effects: &mut Vec<Effect>) {
        let wanted = self.wanted_sounds();
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
        self.remake(instrument, effects);
        if let Some(waiting) = self
            .shelf
            .sample_waiting
            .take_if(|w| w.instrument == instrument)
        {
            self.start_sample(instrument, waiting.latency_us, effects);
        }
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
        let shelf = &mut self.shelf;
        shelf.before.take_if(|b| *b == gone);
        shelf.sample_waiting.take_if(|w| w.instrument == instrument);
        if shelf.trial.as_ref() == Some(&gone) {
            self.end_audition(effects);
        }
        self.remake(instrument, effects);
    }

    /// Every seat that holds `instrument` gets one made now, in place of the one made before
    /// its samples were there (or before they turned out not to read).
    fn remake(&mut self, instrument: BuiltinInstrument, effects: &mut Vec<Effect>) {
        let made = InstrumentRef::Builtin(instrument);
        let trial = self.shelf.trial.clone();
        let trial_seat = match (self.shelf.mode, &trial) {
            (Mode::Replace(track), Some(_)) => self.seats.of(track),
            _ => None,
        };
        let mut ops = Vec::new();
        for (seat, _, own) in self.seats.taken() {
            if *own != made {
                continue;
            }
            let own = EngineOp::Instrument {
                seat,
                instrument: made.clone(),
            };
            match &trial {
                // The track's own one is set aside under a trial: the trial steps off for
                // the new one and on again.
                Some(tried) if trial_seat == Some(seat) => ops.extend([
                    EngineOp::EndTrial { seat, keep: false },
                    own,
                    EngineOp::TryInstrument {
                        seat,
                        instrument: tried.clone(),
                    },
                ]),
                _ => ops.push(own),
            }
        }
        if trial.as_ref() == Some(&made) {
            match (self.shelf.mode, trial_seat) {
                (Mode::NewTrack, _) => ops.push(EngineOp::Instrument {
                    seat: AUDITION_SEAT,
                    instrument: made,
                }),
                // When it is the track's own too, it was made anew just above.
                (Mode::Replace(_), Some(seat)) if self.seats.instrument(seat) != Some(&made) => {
                    ops.push(EngineOp::TryInstrument {
                        seat,
                        instrument: made,
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
        if !open || !self.sounds.available().contains(&instrument) {
            return;
        }
        if self.sounds.ready(instrument) {
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
        instrument: BuiltinInstrument,
        latency_us: u64,
        effects: &mut Vec<Effect>,
    ) {
        effects.push(Effect::Engine(EngineOp::PlaySample {
            instrument: InstrumentRef::Builtin(instrument),
        }));
        self.shelf.sample = Some(Sample {
            instrument,
            started_us: self.now_us + latency_us,
        });
    }

    /// `StopSample`, and whatever else ends a sample: the shelf closes, the output goes away.
    pub fn stop_sample(&mut self, effects: &mut Vec<Effect>) {
        self.shelf.sample_waiting = None;
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

    pub(super) fn sample_view(&self) -> Option<SampleView> {
        self.shelf.sample.map(|s| SampleView {
            instrument: instruments::kind_of(s.instrument),
            started_us: s.started_us,
            length_us: SAMPLE_US,
        })
    }

    /// The status line while a sample plays and no row is auditioned.
    pub fn sample_status(&self) -> Option<StatusMessage> {
        let instrument = instruments::kind_of(self.shelf.sample?.instrument);
        Some(StatusMessage::SamplePlaying { instrument })
    }
}

#[cfg(test)]
mod tests;

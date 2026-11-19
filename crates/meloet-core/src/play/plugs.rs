//! The plugins the engine holds for the song and the shelf: which is where, and how far each
//! has come (docs/architecture.md 20-10).
//!
//! A plugin is loaded off the core thread ([`Effect::LoadPlugin`]) under a ticket. Until the
//! answer comes its place in the engine is silent (an instrument) or empty (an effect); then
//! `EngineOp::Plug` puts it there. Nothing here unloads: a plugin is let go of when the
//! engine hands it back, which it does when something else takes its place.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use meloet_model::{InstrumentGroup, InstrumentRef, PluginPlace, PluginRef, StripId, TrackId};
use meloet_protocol::{InsertRef, PluginSlotRef, PluginState};

use super::mixer::{strip_id, strip_ref};
use super::seats::AUDITION_SEAT;
use super::shelf::Mode;
use super::sounds::Sample;
use super::{EngineOp, Play, StripSeat};
use crate::added::sound_ref;
use crate::state::Effect;

/// The sample rate of the engine before any stream told another.
const START_RATE: u32 = 48_000;

/// A place in the engine a plugin can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Spot {
    /// The seat's own instrument. The audition seat is one of these.
    Seat(usize),
    /// The instrument tried on the seat, over its own.
    Trial(usize),
    Insert(StripSeat, usize),
    /// The seat of the shelf's sample (▶).
    Sample,
}

/// The state a plugin is loaded with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlugState {
    /// As the plugin starts.
    Fresh,
    Bytes(Arc<[u8]>),
    /// What the plugin of this ticket says its state is now; when it cannot say, what was
    /// last known of it.
    Of(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Loading,
    Ready,
    /// Its process answers no more.
    Stopped,
    /// Stopped, and being put into a new process.
    Reloading,
    /// It did not load.
    Failed,
}

#[derive(Debug, Clone)]
struct Plug {
    spot: Spot,
    ticket: u64,
    /// As it was asked for: the key, and the state it was given.
    plugin: Arc<PluginRef>,
    phase: Phase,
    /// It is in the engine. A plugin that became ready under a trial waits until the trial
    /// is over.
    placed: bool,
}

#[derive(Debug)]
pub(super) struct Plugs {
    list: Vec<Plug>,
    next: u64,
    /// Every plugin this PC has, by its exact key, with the bundle it is loaded from.
    known: HashMap<Arc<str>, PathBuf>,
    rate: u32,
    /// The tickets of the song's plugins that stopped, in the order they did.
    stopped: Vec<u64>,
    /// The phrase the sample plays once its plugin is loaded.
    sample_group: InstrumentGroup,
}

impl Default for Plugs {
    fn default() -> Plugs {
        Plugs {
            list: Vec::new(),
            next: 1,
            known: HashMap::new(),
            rate: START_RATE,
            stopped: Vec::new(),
            sample_group: InstrumentGroup::Synth,
        }
    }
}

impl Plugs {
    fn at(&self, spot: Spot) -> Option<&Plug> {
        self.list.iter().find(|p| p.spot == spot)
    }

    fn at_mut(&mut self, spot: Spot) -> Option<&mut Plug> {
        self.list.iter_mut().find(|p| p.spot == spot)
    }

    fn ticket(&mut self) -> u64 {
        self.next += 1;
        self.next - 1
    }
}

/// What came of asking for a plugin at a spot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Want {
    /// It is there already, or on its way.
    Kept,
    /// It is being loaded now; what was at the spot is no plugin of the book any more.
    Loading,
    /// This PC does not have it.
    Missing,
}

impl Play {
    /// The plugins this PC has. Told at the start and whenever the scan has news.
    pub fn set_known_plugins(&mut self, known: Vec<(Arc<str>, PathBuf)>) {
        self.plugs.known = known.into_iter().collect();
    }

    pub(super) fn has_plugin(&self, key: &str) -> bool {
        self.plugs.known.contains_key(key)
    }

    /// What the engine is given for a seat's instrument: the sine wave in place of a plugin
    /// this PC does not have, so that its track is not silent.
    pub(crate) fn engine_instrument(&self, instrument: &InstrumentRef) -> InstrumentRef {
        match instrument {
            InstrumentRef::Plugin(p) if !self.has_plugin(&p.key) => InstrumentRef::SINE,
            other => other.clone(),
        }
    }

    /// Forgets the plugin at the spot. One that is loaded and not in the engine is let go of
    /// here; any other goes when the engine hands it back, or when its load answers.
    pub(super) fn drop_plug(&mut self, spot: Spot, effects: &mut Vec<Effect>) {
        let plugs = &mut self.plugs;
        if let Some(at) = plugs.list.iter().position(|p| p.spot == spot) {
            let gone = plugs.list.remove(at);
            plugs.stopped.retain(|t| *t != gone.ticket);
            if gone.phase != Phase::Loading && !gone.placed {
                effects.push(Effect::DropPlugin {
                    ticket: gone.ticket,
                });
            }
        }
    }

    /// Makes `plugin` the one at `spot`: kept when it is there with the same state, given
    /// the new state when only that differs, else loaded anew. `from`: load it with the
    /// state the plugin of that ticket has now.
    pub(super) fn want_plug(
        &mut self,
        spot: Spot,
        plugin: &Arc<PluginRef>,
        from: Option<u64>,
        effects: &mut Vec<Effect>,
    ) -> Want {
        if let Some(have) = self.plugs.at_mut(spot)
            && have.plugin.key == plugin.key
            && from.is_none()
        {
            if have.plugin.state == plugin.state {
                have.plugin = plugin.clone();
                return Want::Kept;
            }
            if have.phase == Phase::Ready && !plugin.state.is_empty() {
                have.plugin = plugin.clone();
                effects.push(Effect::SetPluginState {
                    ticket: have.ticket,
                    state: plugin.state.clone(),
                });
                return Want::Kept;
            }
        }
        self.drop_plug(spot, effects);
        let Some(bundle) = self.plugs.known.get(&*plugin.key).cloned() else {
            return Want::Missing;
        };
        let ticket = self.plugs.ticket();
        self.plugs.list.push(Plug {
            spot,
            ticket,
            plugin: plugin.clone(),
            phase: Phase::Loading,
            placed: false,
        });
        effects.push(Effect::LoadPlugin {
            ticket,
            key: plugin.key.clone(),
            bundle,
            state: match from {
                Some(ticket) => PlugState::Of(ticket),
                None if plugin.state.is_empty() => PlugState::Fresh,
                None => PlugState::Bytes(plugin.state.clone()),
            },
            sample_rate: self.plugs.rate,
        });
        Want::Loading
    }

    /// The seat's own instrument is `instrument` from now on: the engine is told, and a
    /// plugin is loaded. Not for a seat with a trial on it.
    pub(super) fn seat_instrument(
        &mut self,
        seat: usize,
        instrument: &InstrumentRef,
        effects: &mut Vec<Effect>,
    ) {
        let given = match instrument {
            InstrumentRef::Plugin(p) => match self.want_plug(Spot::Seat(seat), p, None, effects) {
                Want::Kept => return,
                Want::Loading => instrument.clone(),
                Want::Missing => InstrumentRef::SINE,
            },
            other => {
                self.drop_plug(Spot::Seat(seat), effects);
                other.clone()
            }
        };
        effects.push(Effect::Engine(EngineOp::Instrument {
            seat,
            instrument: given,
        }));
    }

    /// The seat on which a row is tried over the track's own instrument.
    fn trial_seat(&self) -> Option<usize> {
        match (self.shelf.mode, &self.shelf.trial) {
            (Mode::Replace(track), Some(_)) => self.seats.of(track),
            _ => None,
        }
    }

    /// A seat was emptied in the engine: nothing of it is in the book any more.
    pub(super) fn seat_plugs_gone(&mut self, seat: usize, effects: &mut Vec<Effect>) {
        let gone: Vec<Spot> = (self.plugs.list.iter())
            .map(|p| p.spot)
            .filter(|spot| match *spot {
                Spot::Seat(s) | Spot::Trial(s) => s == seat,
                Spot::Insert(strip, _) => strip == StripSeat::Track(seat),
                Spot::Sample => false,
            })
            .collect();
        for spot in gone {
            self.drop_plug(spot, effects);
        }
    }

    /// The plugin at `from`, if any, counts as the one at `to`, where the engine has it now.
    pub(super) fn move_plug(&mut self, from: Spot, to: Spot, effects: &mut Vec<Effect>) {
        if self.plugs.at(from).is_some() {
            self.drop_plug(to, effects);
        }
        if let Some(plug) = self.plugs.at_mut(from) {
            plug.spot = to;
        }
    }

    /// A trial on `seat` ended and the seat's own instrument is back: its plugin, when it
    /// became ready meanwhile, goes on the seat now.
    pub(super) fn place_waiting(&mut self, seat: usize, effects: &mut Vec<Effect>) {
        if let Some(plug) = self.plugs.at_mut(Spot::Seat(seat))
            && plug.phase == Phase::Ready
            && !plug.placed
        {
            plug.placed = true;
            effects.push(Effect::Engine(EngineOp::Plug {
                ticket: plug.ticket,
                spot: plug.spot,
            }));
        }
    }

    pub(super) fn plug_loading(&self, spot: Spot) -> bool {
        (self.plugs.at(spot)).is_some_and(|p| matches!(p.phase, Phase::Loading | Phase::Reloading))
    }

    pub(super) fn plug_ticket(&self, spot: Spot) -> Option<u64> {
        self.plugs.at(spot).map(|p| p.ticket)
    }

    /// The answer to [`Effect::LoadPlugin`] and [`Effect::ReloadPlugin`]: the plugin is
    /// ready for the engine.
    pub fn plugin_loaded(&mut self, ticket: u64, effects: &mut Vec<Effect>) {
        let trial_seat = self.trial_seat();
        let plugs = &mut self.plugs;
        let Some(plug) = plugs.list.iter_mut().find(|p| p.ticket == ticket) else {
            // Nothing waits for it any more.
            effects.push(Effect::DropPlugin { ticket });
            return;
        };
        plug.phase = Phase::Ready;
        plugs.stopped.retain(|t| *t != ticket);
        let spot = plug.spot;
        match spot {
            // Under a trial the engine would take it for the tried one.
            Spot::Seat(seat) if trial_seat == Some(seat) => plug.placed = false,
            Spot::Sample => {
                plug.placed = true;
                let group = plugs.sample_group;
                effects.push(Effect::Engine(EngineOp::PlugSample { ticket, group }));
                if let Some(waiting) = self.shelf.sample_waiting.take() {
                    self.shelf.sample = Some(Sample {
                        instrument: waiting.instrument,
                        started_us: self.now_us + waiting.latency_us,
                    });
                }
            }
            _ => {
                plug.placed = true;
                effects.push(Effect::Engine(EngineOp::Plug { ticket, spot }));
            }
        }
    }

    /// The row that is tried could not be loaded, or stopped: the shelf says so under it and
    /// nothing is tried any more.
    fn trial_failed(&mut self, effects: &mut Vec<Effect>) {
        self.shelf.unreadable = self.shelf.trial.as_ref().map(sound_ref);
        self.end_audition(effects);
    }

    fn is_trial_spot(spot: Spot) -> bool {
        matches!(spot, Spot::Seat(AUDITION_SEAT) | Spot::Trial(_))
    }

    /// The other answer to [`Effect::LoadPlugin`] and [`Effect::ReloadPlugin`].
    pub fn plugin_unreadable(&mut self, ticket: u64, effects: &mut Vec<Effect>) {
        let Some(plug) = self.plugs.list.iter_mut().find(|p| p.ticket == ticket) else {
            return;
        };
        let spot = plug.spot;
        match plug.phase {
            // Still as it was: stopped.
            Phase::Reloading => plug.phase = Phase::Stopped,
            _ if Self::is_trial_spot(spot) => self.trial_failed(effects),
            _ if spot == Spot::Sample => self.stop_sample(effects),
            _ => plug.phase = Phase::Failed,
        }
    }

    /// The process of the plugin answers no more.
    pub fn plugin_stopped(&mut self, ticket: u64, effects: &mut Vec<Effect>) {
        let Some(plug) = self.plugs.list.iter_mut().find(|p| p.ticket == ticket) else {
            return;
        };
        let spot = plug.spot;
        if Self::is_trial_spot(spot) {
            self.trial_failed(effects);
        } else if spot == Spot::Sample {
            self.stop_sample(effects);
        } else if plug.phase != Phase::Stopped {
            plug.phase = Phase::Stopped;
            self.plugs.stopped.retain(|t| *t != ticket);
            self.plugs.stopped.push(ticket);
        }
    }

    /// Loads every plugin of the book again under a new ticket. `live`: with the state each
    /// has now (the old ones still run); else with the state it was asked for with.
    fn load_again(&mut self, live: bool, effects: &mut Vec<Effect>) {
        let plugs = &mut self.plugs;
        for at in 0..plugs.list.len() {
            let old = plugs.list[at].clone();
            let state = match old.phase {
                Phase::Ready if live => PlugState::Of(old.ticket),
                Phase::Ready | Phase::Loading if old.plugin.state.is_empty() => PlugState::Fresh,
                Phase::Ready | Phase::Loading => PlugState::Bytes(old.plugin.state.clone()),
                Phase::Stopped | Phase::Reloading | Phase::Failed => continue,
            };
            let Some(bundle) = plugs.known.get(&*old.plugin.key).cloned() else {
                continue;
            };
            let ticket = plugs.ticket();
            if old.phase == Phase::Ready && !old.placed {
                effects.push(Effect::DropPlugin { ticket: old.ticket });
            }
            let plug = &mut plugs.list[at];
            plug.ticket = ticket;
            plug.phase = Phase::Loading;
            effects.push(Effect::LoadPlugin {
                ticket,
                key: old.plugin.key.clone(),
                bundle,
                state,
                sample_rate: plugs.rate,
            });
        }
    }

    /// The stream runs at `rate`: a plugin is made for one rate, so every one is loaded
    /// again when it is another than before.
    pub fn set_plug_rate(&mut self, rate: u32, effects: &mut Vec<Effect>) {
        if rate != self.plugs.rate && rate > 0 {
            self.plugs.rate = rate;
            self.load_again(true, effects);
        }
    }

    /// The engine was made anew, without the plugins the old one held.
    pub fn plugs_lost(&mut self, effects: &mut Vec<Effect>) {
        for plug in &mut self.plugs.list {
            plug.placed = false;
        }
        self.load_again(false, effects);
    }

    /// After every input: a plugin of the song that this PC did not have when its track was
    /// seated, and has now (the scan found it), is loaded.
    pub fn sync_plugs(&mut self, effects: &mut Vec<Effect>) {
        let trial_seat = self.trial_seat();
        let waiting: Vec<(usize, Arc<PluginRef>)> = (self.doc().tracks.iter())
            .filter_map(|t| match &t.instrument {
                InstrumentRef::Plugin(p) => Some((self.seats.of(t.id)?, p.clone())),
                _ => None,
            })
            .filter(|(seat, p)| {
                self.plugs.at(Spot::Seat(*seat)).is_none() && self.has_plugin(&p.key)
            })
            .collect();
        for (seat, plugin) in waiting {
            self.want_plug(Spot::Seat(seat), &plugin, None, effects);
            // The seat sounds as the sine wave until now. Under a trial the engine would
            // take a new instrument for the tried one; the sine wave then stays set aside.
            if trial_seat != Some(seat) {
                effects.push(Effect::Engine(EngineOp::Instrument {
                    seat,
                    instrument: InstrumentRef::Plugin(plugin),
                }));
            }
        }
    }

    fn strip_of(&self, strip: StripSeat) -> Option<StripId> {
        Some(match strip {
            StripSeat::Track(seat) => StripId::Track(self.track_on(seat)?),
            StripSeat::Return(bus) => StripId::Return(bus),
            StripSeat::Master => StripId::Master,
        })
    }

    fn track_on(&self, seat: usize) -> Option<TrackId> {
        (self.seats.taken()).find_map(|(s, track, _)| (s == seat).then_some(track))
    }

    pub(super) fn strip_seat(&self, strip: StripId) -> Option<StripSeat> {
        Some(match strip {
            StripId::Track(track) => StripSeat::Track(self.seats.of(track)?),
            StripId::Return(bus) => StripSeat::Return(bus),
            StripId::Master => StripSeat::Master,
        })
    }

    fn place_of(&self, spot: Spot) -> Option<PluginPlace> {
        match spot {
            Spot::Seat(seat) => Some(PluginPlace::Instrument(self.track_on(seat)?)),
            Spot::Insert(strip, slot) => Some(PluginPlace::Insert(self.strip_of(strip)?, slot)),
            Spot::Trial(_) | Spot::Sample => None,
        }
    }

    fn spot_of(&self, place: PluginPlace) -> Option<Spot> {
        Some(match place {
            PluginPlace::Instrument(track) => Spot::Seat(self.seats.of(track)?),
            PluginPlace::Insert(strip, slot) => Spot::Insert(self.strip_seat(strip)?, slot),
        })
    }

    /// How the plugin at a place of the song is, for the screen.
    pub fn plugin_state(&self, place: PluginPlace) -> PluginState {
        let key = self.doc().plugin(place).map(|p| &p.key);
        let plug = self.spot_of(place).and_then(|spot| self.plugs.at(spot));
        match plug.filter(|p| Some(&p.plugin.key) == key) {
            Some(p) => match p.phase {
                Phase::Ready => PluginState::Ready,
                Phase::Loading | Phase::Reloading => PluginState::Loading,
                Phase::Stopped | Phase::Failed => PluginState::Stopped,
            },
            None if key.is_some_and(|key| self.has_plugin(key)) => PluginState::Loading,
            None => PluginState::Missing,
        }
    }

    /// The plugins of the song that sound, by the ticket each was loaded under: the ones
    /// whose state a save asks for.
    pub fn plugin_tickets(&self) -> Vec<(PluginPlace, u64)> {
        (self.plugs.list.iter())
            .filter(|p| p.phase == Phase::Ready && p.placed)
            .filter_map(|p| Some((self.place_of(p.spot)?, p.ticket)))
            .collect()
    }

    fn slot_ref(place: PluginPlace) -> PluginSlotRef {
        match place {
            PluginPlace::Instrument(track) => PluginSlotRef::Instrument { track: track.0 },
            PluginPlace::Insert(strip, slot) => PluginSlotRef::Insert {
                insert: InsertRef {
                    strip: strip_ref(strip),
                    slot: slot as u8,
                },
            },
        }
    }

    fn slot_place(slot: PluginSlotRef) -> PluginPlace {
        match slot {
            PluginSlotRef::Instrument { track } => PluginPlace::Instrument(TrackId(track)),
            PluginSlotRef::Insert { insert } => {
                PluginPlace::Insert(strip_id(insert.strip), usize::from(insert.slot))
            }
        }
    }

    /// The plugin of the song that stopped last and was not loaded again or removed since,
    /// with its name.
    pub fn stopped_plugin(&self) -> Option<(PluginSlotRef, String)> {
        (self.plugs.stopped.iter().rev()).find_map(|ticket| {
            let plug = self.plugs.list.iter().find(|p| p.ticket == *ticket)?;
            let place = self.place_of(plug.spot)?;
            Some((Self::slot_ref(place), plug.plugin.name.to_string()))
        })
    }

    /// Whether the plugin in `slot` stopped, or did not load: 「もう一度読みこむ」 and 「外す」
    /// are for these.
    pub fn plugin_is_down(&self, slot: PluginSlotRef) -> bool {
        (self.spot_of(Self::slot_place(slot)))
            .and_then(|spot| self.plugs.at(spot))
            .is_some_and(|p| matches!(p.phase, Phase::Stopped | Phase::Failed))
    }

    /// 「もう一度読みこむ」: a stopped plugin goes into a new process with what was remembered
    /// of it; one that never loaded is loaded as the song has it.
    pub fn reload_plugin(&mut self, slot: PluginSlotRef, effects: &mut Vec<Effect>) {
        let Some(spot) = self.spot_of(Self::slot_place(slot)) else {
            return;
        };
        let Some(plug) = self.plugs.at_mut(spot) else {
            return;
        };
        match plug.phase {
            Phase::Stopped => {
                plug.phase = Phase::Reloading;
                let ticket = plug.ticket;
                effects.push(Effect::ReloadPlugin { ticket });
            }
            Phase::Failed => {
                let plugin = plug.plugin.clone();
                self.drop_plug(spot, effects);
                self.want_plug(spot, &plugin, None, effects);
            }
            Phase::Loading | Phase::Ready | Phase::Reloading => {}
        }
    }

    /// ▶ on a plugin's row: an instance of its own plays the phrase of `group` once it is
    /// loaded, with the sound the row has when it is the one tried now.
    pub(super) fn sample_plugin(
        &mut self,
        plugin: &Arc<PluginRef>,
        group: InstrumentGroup,
        effects: &mut Vec<Effect>,
    ) {
        let tried = (self.plugs.list.iter()).find(|p| {
            Self::is_trial_spot(p.spot) && p.phase == Phase::Ready && p.plugin.key == plugin.key
        });
        let from = tried.map(|p| p.ticket);
        self.plugs.sample_group = group;
        self.drop_plug(Spot::Sample, effects);
        self.want_plug(Spot::Sample, plugin, from, effects);
    }
}

#[cfg(test)]
mod tests;

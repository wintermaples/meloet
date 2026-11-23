//! The whole state of a plugin of the song, read and put in by the AI link
//! (docs/architecture.md 24-14).
//!
//! A read asks the plugin and keeps its answer until the AI link takes it. A put goes to the
//! plugin first; only when the plugin has taken the state does the song get it, as one step
//! (the step of a visit to the plugin's window: what the history band already names). A
//! plugin that refuses the state gets the song's state back. One whose process stops while it
//! takes the state is loaded again with what was known of it before, and so are the other
//! plugins of that process, which stopped with it.

use std::collections::HashMap;
use std::sync::Arc;

use meloet_model::{InstrumentRef, PluginChange, PluginPlace};

use super::{Phase, Spot, Want};
use crate::ai::{AiOutcome, AiPluginState, AiPutEnd, AiRefusal, AiState, AiStatePut};
use crate::play::{EngineOp, Play};
use crate::plugins::hosts::HostError;
use crate::state::{Effect, PluginAsk, StateAsk};

/// How long a state a plugin said for the AI link waits for it to take it.
const KEEP_US: u64 = 1_000_000;
/// How long a state asked for the AI link is waited for before it is asked again.
const ASK_AGAIN_US: u64 = 2_000_000;
/// How long after a put stopped a plugin's process its other plugins, told as stopped, are
/// loaded again too.
const REVIVE_US: u64 = 2_000_000;
/// How many ends of puts are kept for the AI link to learn.
const ENDS_KEPT: usize = 32;

/// A state a plugin said (`None`: it could not), and when it came.
type Said = (Option<Arc<[u8]>>, u64);

#[derive(Debug, Default)]
pub(super) struct AiStates {
    /// What plugins said for the AI link, by their tickets, and when it came.
    said: HashMap<u64, Said>,
    /// The plugins asked for their state for the AI link, and when.
    asked: Vec<(u64, u64)>,
    putting: Vec<Putting>,
    /// How puts ended, under the AI link's tickets, until the rules hand them on.
    ended: Vec<(u64, AiOutcome)>,
    /// How the last puts ended, under the AI link's tickets, the newest last.
    ends: Vec<(u64, AiStatePut)>,
    /// The key of the plugins that are loaded again when told as stopped, and until when.
    revive: Option<(Arc<str>, u64)>,
    /// Plugins a put stopped after an undo or a redo gave them another state, which their
    /// process never took: they are loaded anew with the song's state, not with what `Hosts`
    /// remembers.
    from_song: Vec<u64>,
}

impl AiStates {
    /// A state is on its way into the plugin of this ticket.
    pub(super) fn putting_into(&self, ticket: u64) -> bool {
        self.putting.iter().any(|p| p.ticket == ticket)
    }
}

/// A state on its way into a plugin.
#[derive(Debug)]
struct Putting {
    ticket: u64,
    /// The AI link's ticket.
    ai: u64,
    /// `Plug::puts` when it was sent: a state put back after it (undo, redo) overtakes it.
    puts: u32,
    place: PluginPlace,
}

impl Play {
    /// The ticket of the plugin at `place`, when it is loaded and sounds.
    fn ai_plug(&self, place: PluginPlace) -> Result<u64, AiRefusal> {
        self.doc().plugin(place).ok_or(AiRefusal::NoSuchPlugin)?;
        (self.plugin_tickets().into_iter())
            .find(|(p, _)| *p == place)
            .map(|(_, ticket)| ticket)
            .ok_or(AiRefusal::PluginNotLoaded)
    }

    /// The AI link reads the whole state of the plugin at `place`: what the plugin said since
    /// it was asked, or the plugin is asked now.
    pub(crate) fn ai_plugin_state(
        &mut self,
        place: PluginPlace,
        effects: &mut Vec<Effect>,
    ) -> Result<AiState, AiRefusal> {
        let ticket = self.ai_plug(place)?;
        let now = self.now_us;
        let ai = &mut self.plugs.ai;
        ai.said
            .retain(|_, (_, at)| now.saturating_sub(*at) <= KEEP_US);
        if let Some((state, _)) = ai.said.remove(&ticket) {
            let bytes = state.ok_or(AiRefusal::PluginNotLoaded)?;
            let plugin = self.doc().plugin(place).ok_or(AiRefusal::NoSuchPlugin)?;
            return Ok(AiState::Got(AiPluginState {
                plugin: plugin.clone(),
                bytes,
            }));
        }
        ai.asked
            .retain(|(_, at)| now.saturating_sub(*at) < ASK_AGAIN_US);
        if !ai.asked.iter().any(|(t, _)| *t == ticket) {
            ai.asked.push((ticket, now));
            effects.push(Effect::AskPlugin {
                ticket,
                ask: PluginAsk::State(StateAsk::Ai),
            });
        }
        Ok(AiState::Waiting)
    }

    /// The plugin said its state for the AI link.
    pub(in crate::play) fn ai_state_said(&mut self, ticket: u64, state: Option<Arc<[u8]>>) {
        let now = self.now_us;
        let ai = &mut self.plugs.ai;
        ai.asked.retain(|(t, _)| *t != ticket);
        ai.said
            .retain(|_, (_, at)| now.saturating_sub(*at) <= KEEP_US);
        ai.said.insert(ticket, (state, now));
    }

    /// The AI link puts a whole state into the plugin at `place`, under its ticket `ai`. The
    /// step comes when the plugin has taken it.
    pub(crate) fn ai_put_plugin_state(
        &mut self,
        place: PluginPlace,
        state: Arc<[u8]>,
        ai: u64,
        effects: &mut Vec<Effect>,
    ) -> Result<(), AiRefusal> {
        let ticket = self.ai_plug(place)?;
        if self.plugs.ai.putting.iter().any(|p| p.ticket == ticket) {
            return Err(AiRefusal::PluginBusy);
        }
        let plug = self.plugs.of(ticket).ok_or(AiRefusal::PluginNotLoaded)?;
        if state.is_empty() {
            return Err(AiRefusal::Cannot("the state is empty".into()));
        }
        if plug.plugin.state == state {
            return Err(AiRefusal::Unchanged);
        }
        // As before the parameters panel changes a plugin: a drag is over, what the window
        // changed until now is a step of its own, and a plugin the song has "as it starts"
        // says what that is, for undo.
        self.cancel_plugin_drags(effects);
        self.want_base(ticket, effects);
        let visiting = self.end_visit(ticket, effects);
        let puts = self.plugs.of(ticket).map_or(0, |plug| plug.puts);
        self.plugs.ai.putting.push(Putting {
            ticket,
            ai,
            puts,
            place,
        });
        effects.push(Effect::AskPlugin {
            ticket,
            ask: PluginAsk::PutState { state },
        });
        // A visit under way goes on from the state put in.
        if visiting {
            self.start_visit(ticket, effects);
        }
        Ok(())
    }

    /// The plugin took a state the AI link put into it, refused it, or its process is gone.
    pub(in crate::play) fn ai_state_put(
        &mut self,
        ticket: u64,
        put: Result<Arc<[u8]>, HostError>,
        effects: &mut Vec<Effect>,
    ) {
        let putting = &mut self.plugs.ai.putting;
        let Some(at) = putting.iter().position(|p| p.ticket == ticket) else {
            return;
        };
        let going = putting.remove(at);
        let end = match put {
            Ok(now) => self.ai_state_taken(&going, now, effects),
            Err(HostError::Refused(why)) => {
                self.put_song_state_back(ticket, effects);
                AiPutEnd::Refused(why)
            }
            Err(HostError::Gone) => {
                let restored = (self.plugs.of(ticket)).is_some_and(|p| p.puts != going.puts);
                self.revive(ticket, restored, effects);
                AiPutEnd::Stopped
            }
            Err(other) => AiPutEnd::Failed(other.to_string()),
        };
        let outcome = match &end {
            AiPutEnd::Step(_) | AiPutEnd::Same => AiOutcome::Done(None),
            AiPutEnd::Refused(why) => AiOutcome::Failed(format!(
                "the plugin refused the state ({why}); it has the song's state again, and no \
                 step was made"
            )),
            AiPutEnd::Stopped => AiOutcome::Failed(
                "the plugin's process stopped while it took the state (the state may be \
                 broken); it is loaded again with the song's state, and no step was made"
                    .into(),
            ),
            AiPutEnd::Overtaken => AiOutcome::Failed(
                "an undo or a redo changed the plugin meanwhile, or it was loaded again with the \
                 song's state; no step was made"
                    .into(),
            ),
            AiPutEnd::Gone => AiOutcome::Failed(
                "the plugin was taken out of the song meanwhile; no step was made".into(),
            ),
            AiPutEnd::Failed(why) => AiOutcome::Failed(format!(
                "the plugin could not take the state ({why}); no step was made"
            )),
        };
        self.plugs.ai.ended.push((going.ai, outcome));
        let undo_count = self.undo_count();
        let ends = &mut self.plugs.ai.ends;
        if ends.len() == ENDS_KEPT {
            ends.remove(0);
        }
        ends.push((going.ai, AiStatePut { end, undo_count }));
    }

    /// How the AI link's put of `ai` ended, or `Waiting` while the plugin takes it.
    pub(crate) fn ai_state_put_end(&self, ai: u64) -> Result<AiState, AiRefusal> {
        let ai_states = &self.plugs.ai;
        if ai_states.putting.iter().any(|p| p.ai == ai) {
            return Ok(AiState::Waiting);
        }
        (ai_states.ends.iter())
            .find(|(t, _)| *t == ai)
            .map(|(_, put)| AiState::Put(put.clone()))
            .ok_or_else(|| AiRefusal::Cannot("that put of a state is not known any more".into()))
    }

    /// The step of a state the plugin took: what the plugin says its state is since, as for
    /// every step of a plugin. A plugin that passed over what it was given says the state it
    /// had, which is no step.
    fn ai_state_taken(
        &mut self,
        going: &Putting,
        now: Arc<[u8]>,
        effects: &mut Vec<Effect>,
    ) -> AiPutEnd {
        let ticket = going.ticket;
        let Some(plug) = self.plugs.of(ticket) else {
            // Loaded anew meanwhile (another sample rate), with the song's state; or taken out.
            return match self.doc().plugin(going.place) {
                Some(_) => AiPutEnd::Overtaken,
                None => AiPutEnd::Gone,
            };
        };
        if plug.puts != going.puts {
            return AiPutEnd::Overtaken;
        }
        let spot = plug.spot;
        let same = plug.plugin.state == now
            || (plug.plugin.state.is_empty() && plug.base.as_ref() == Some(&now));
        let edit = self.plugin_state_edit(ticket, now, PluginChange::Window);
        let end = match edit {
            Some(edit) => {
                if let Some(place) = self.place_of(spot)
                    && let Some(now) = edit.doc.plugin(place).cloned()
                    && let Some(plug) = self.plugs.of_mut(ticket)
                {
                    plug.plugin = now;
                }
                let steps = self.undo_count();
                self.apply_own(edit, effects);
                match self.undo_count() {
                    now if now > steps => {
                        AiPutEnd::Step(self.history_rows(now, now).pop().map(Box::new))
                    }
                    _ => AiPutEnd::Same,
                }
            }
            None if same => AiPutEnd::Same,
            None => {
                self.put_song_state_back(ticket, effects);
                return AiPutEnd::Gone;
            }
        };
        // Its parameters may all have other values now.
        self.table_stale(ticket);
        self.lanes_anew(spot, effects);
        end
    }

    /// The plugin gets the song's state again: it may have taken part of one it refused.
    fn put_song_state_back(&mut self, ticket: u64, effects: &mut Vec<Effect>) {
        let Some(plug) = self
            .plugs
            .of_mut(ticket)
            .filter(|p| p.phase == Phase::Ready)
        else {
            return;
        };
        let state = match plug.plugin.state.is_empty() {
            true => plug.base.clone(),
            false => Some(plug.plugin.state.clone()),
        };
        let Some(state) = state else {
            // What it started with is not known: it starts anew.
            let spot = plug.spot;
            self.load_from_song(spot, effects);
            return;
        };
        plug.puts += 1;
        let spot = plug.spot;
        effects.push(Effect::SetPluginState { ticket, state });
        self.table_stale(ticket);
        self.visit_overtaken(ticket, effects);
        self.lanes_anew(spot, effects);
    }

    /// The process of the plugin stopped while it took a state, or before: it is loaded
    /// again, and so are the plugins of the song with its key that are told as stopped soon
    /// after (they shared its process), with what was known of them (`Hosts` keeps the state
    /// from before). `from_song`: an undo or a redo gave it another state meanwhile.
    fn revive(&mut self, ticket: u64, from_song: bool, effects: &mut Vec<Effect>) {
        let Some(plug) = self.plugs.of(ticket) else {
            return;
        };
        let (key, spot) = (plug.plugin.key.clone(), plug.spot);
        if from_song {
            self.plugs.ai.from_song.push(ticket);
        }
        self.plugs.ai.revive = Some((key, self.now_us + REVIVE_US));
        self.reload_at(spot, effects);
    }

    /// A plugin of the song was told as stopped.
    pub(super) fn revive_stopped(&mut self, ticket: u64, effects: &mut Vec<Effect>) {
        let Some((key, until)) = &self.plugs.ai.revive else {
            return;
        };
        if self.now_us > *until {
            self.plugs.ai.revive = None;
            return;
        }
        let spot = (self.plugs.of(ticket))
            .filter(|p| p.phase == Phase::Stopped && p.plugin.key == *key)
            .map(|p| p.spot);
        if let Some(spot) = spot {
            self.reload_at(spot, effects);
        }
    }

    /// The stopped plugin at `spot` goes into a new process.
    fn reload_at(&mut self, spot: Spot, effects: &mut Vec<Effect>) {
        let Some(plug) = (self.plugs.at_mut(spot)).filter(|p| p.phase == Phase::Stopped) else {
            return;
        };
        let ticket = plug.ticket;
        let from_song = &mut self.plugs.ai.from_song;
        if let Some(at) = from_song.iter().position(|t| *t == ticket) {
            from_song.remove(at);
            self.load_from_song(spot, effects);
            return;
        }
        if let Some(plug) = self.plugs.at_mut(spot) {
            plug.phase = Phase::Reloading;
        }
        effects.push(Effect::ReloadPlugin { ticket });
    }

    /// The plugin at `spot` is let go of and loaded anew with the state the song has.
    fn load_from_song(&mut self, spot: Spot, effects: &mut Vec<Effect>) {
        let plugin = (self.place_of(spot)).and_then(|place| self.doc().plugin(place).cloned());
        let Some(plugin) = plugin else {
            return;
        };
        self.drop_plug(spot, effects);
        if self.want_plug(spot, &plugin, None, effects) == Want::Loading
            && let Spot::Seat(seat) = spot
            && self.trial_seat() != Some(seat)
        {
            // Silent until it is loaded.
            effects.push(Effect::Engine(EngineOp::Instrument {
                seat,
                instrument: InstrumentRef::Plugin(plugin),
            }));
        }
    }

    /// How the puts of the AI link ended since this was last asked, under its tickets.
    pub(crate) fn take_ai_puts_ended(&mut self) -> Vec<(u64, AiOutcome)> {
        std::mem::take(&mut self.plugs.ai.ended)
    }
}

#[cfg(test)]
mod tests;

//! The plugin windows: which are open, which are being opened, and the visits a person
//! pays to them (docs/architecture.md 20-13).
//!
//! A window belongs to a loaded plugin and is named here by the ticket of its load, so it
//! stays with its plugin wherever that goes: an insert that is moved, a row of the shelf
//! that becomes a track. When the plugin is gone, stopped or loaded anew, so is its window.
//!
//! A visit (spec 3, Y15) is the time the keyboard is in a plugin's window. The plugin's
//! state is asked for as the visit starts and as it ends; when the two differ, the song
//! gets the new state as one step. The plugin has that state already, so the step puts
//! nothing into it; taking the step back does (`Effect::SetPluginState`).
//!
//! The answers come later than the asks, and the person may do something in between. What
//! keeps the song and the plugin the same then: asks of one plugin are answered in order;
//! a state put back into the plugin makes every answer asked before it void
//! ([`Play::visit_overtaken`]); and the parameters panel cuts a visit before it changes
//! the plugin ([`Play::cut_visit`]).

use std::sync::Arc;

use meloet_model::PluginChange;
use meloet_protocol::{PluginSlotRef, PluginWindowView, StatusMessage, WindowsView};

use super::Play;
use super::plugs::Plug;
use crate::state::{Effect, PluginAsk, StateAsk};

/// A window that is not up this long after it was asked for is said to be opening.
pub(super) const SLOW_US: u64 = 200_000;

#[derive(Debug)]
struct Open {
    ticket: u64,
    width: u32,
    height: u32,
    larger_than_screen: bool,
}

#[derive(Debug)]
struct Visit {
    ticket: u64,
    serial: u64,
    /// The plugin's state as the visit started; `None` until the plugin has said it.
    before: Option<Arc<[u8]>>,
    /// The visit is over and the plugin's state is asked for.
    left: bool,
    /// The plugin could not say its state as the visit ended (its process stopped), and
    /// has not been seen stopped since.
    unanswered: bool,
    /// How many times a hand changed the plugin in the window, as the plugin process said
    /// it; `None` until it does. Only a visit that ends with the person leaving the window
    /// hears it: with `None` the state's bytes alone say whether the visit is a step.
    hands: Option<u32>,
    /// The hands of `hands` that wrote into lanes: their steps are made apart.
    wrote: u32,
}

#[derive(Debug, Default)]
pub(super) struct Windows {
    /// In the order they came up.
    open: Vec<Open>,
    /// Asked for and not up yet: the ticket, and when it was asked for.
    opening: Vec<(u64, u64)>,
    /// The tickets of the windows the keyboard is in.
    inside: Vec<u64>,
    visits: Vec<Visit>,
    next_visit: u64,
    /// A window that was asked for did not come up: what the status line is to say of it.
    failed: Option<StatusMessage>,
}

impl Play {
    /// The plugin a window command names: one of the song that sounds, or with `None` the
    /// one the shelf auditions.
    fn window_plug(&self, slot: Option<PluginSlotRef>) -> Option<&Plug> {
        let spot = match slot {
            Some(slot) => self.spot_of(Self::slot_place(slot))?,
            None => self.trial_spot()?,
        };
        self.plugs.at(spot).filter(|plug| plug.sounds())
    }

    fn window_slot(&self, plug: &Plug) -> Option<PluginSlotRef> {
        self.place_of(plug.spot).map(Self::slot_ref)
    }

    /// `OpenPluginWindow`: the window opens, or comes to the front when it is open.
    pub fn open_plugin_window(&mut self, slot: Option<PluginSlotRef>, effects: &mut Vec<Effect>) {
        let Some(plug) = self.window_plug(slot).filter(|plug| self.plug_opens(plug)) else {
            return;
        };
        let (ticket, title) = (plug.ticket, plug.plugin.name.to_string());
        let wins = &mut self.wins;
        let known = wins.open.iter().any(|w| w.ticket == ticket)
            || wins.opening.iter().any(|(t, _)| *t == ticket);
        if !known {
            wins.opening.push((ticket, self.now_us));
        }
        effects.push(Effect::AskPlugin {
            ticket,
            ask: PluginAsk::OpenWindow { title },
        });
    }

    /// `ClosePluginWindow`. What was changed in the window is a step first, as when the
    /// person leaves it.
    pub fn close_plugin_window(&mut self, slot: Option<PluginSlotRef>, effects: &mut Vec<Effect>) {
        let Some(ticket) = self.window_plug(slot).map(|plug| plug.ticket) else {
            return;
        };
        self.window_left(ticket, effects);
        let wins = &mut self.wins;
        wins.open.retain(|w| w.ticket != ticket);
        wins.opening.retain(|(t, _)| *t != ticket);
        effects.push(Effect::AskPlugin {
            ticket,
            ask: PluginAsk::CloseWindow,
        });
    }

    /// `RaisePluginWindows`: every open window comes to the front, in the order of the list.
    pub fn raise_plugin_windows(&mut self, effects: &mut Vec<Effect>) {
        for open in &self.wins.open {
            if let Some(plug) = self.plugs.of(open.ticket) {
                effects.push(Effect::AskPlugin {
                    ticket: open.ticket,
                    ask: PluginAsk::OpenWindow {
                        title: plug.plugin.name.to_string(),
                    },
                });
            }
        }
    }

    /// The window asked for is on the screen. Returns what the status line says of it.
    pub(super) fn window_opened(
        &mut self,
        ticket: u64,
        (width, height): (u32, u32),
        larger_than_screen: bool,
    ) -> Option<StatusMessage> {
        let plug = self.plugs.of(ticket).filter(|plug| plug.sounds())?;
        let (name, slot) = (plug.plugin.name.to_string(), self.window_slot(plug));
        let wins = &mut self.wins;
        let asked = wins.opening.len();
        wins.opening.retain(|(t, _)| *t != ticket);
        if wins.opening.len() == asked {
            return None;
        }
        wins.open.retain(|w| w.ticket != ticket);
        wins.open.push(Open {
            ticket,
            width,
            height,
            larger_than_screen,
        });
        Some(match larger_than_screen {
            true => StatusMessage::PluginWindowLarge {
                name,
                slot,
                width,
                height,
            },
            false if wins.open.len() == 1 => StatusMessage::PluginWindowOpen { name },
            false => StatusMessage::PluginWindowsOpen {
                count: wins.open.len() as u32,
            },
        })
    }

    /// The window asked for did not come up; one that was not asked for is not said.
    pub(super) fn window_failed(&mut self, ticket: u64) {
        let asked = self.wins.opening.len();
        self.wins.opening.retain(|(t, _)| *t != ticket);
        if self.wins.opening.len() < asked
            && let Some(plug) = self.plugs.of(ticket)
        {
            self.wins.failed = Some(StatusMessage::PluginWindowFailed {
                name: plug.plugin.name.to_string(),
            });
        }
    }

    /// What the status line says of a window that did not come up, once.
    pub fn take_window_failed(&mut self) -> Option<StatusMessage> {
        self.wins.failed.take()
    }

    pub(super) fn window_resized(&mut self, ticket: u64, size: (u32, u32), larger: bool) {
        if let Some(open) = self.wins.open.iter_mut().find(|w| w.ticket == ticket) {
            (open.width, open.height) = size;
            open.larger_than_screen = larger;
        }
    }

    /// The person closed the window, or the plugin did.
    pub(super) fn window_closed(&mut self, ticket: u64, effects: &mut Vec<Effect>) {
        self.window_left(ticket, effects);
        self.wins.open.retain(|w| w.ticket != ticket);
        self.wins.opening.retain(|(t, _)| *t != ticket);
    }

    pub(super) fn window_entered(&mut self, ticket: u64, effects: &mut Vec<Effect>) {
        if !self.wins.inside.contains(&ticket) {
            self.wins.inside.push(ticket);
        }
        self.start_visit(ticket, effects);
    }

    pub(super) fn window_left(&mut self, ticket: u64, effects: &mut Vec<Effect>) {
        self.wins.inside.retain(|t| *t != ticket);
        if self.dragging(ticket) {
            // The drag cut the visit as it started, and the person has been at the panel
            // since: the news that they left the window is late. What the plugin has now
            // is the drag's.
            self.wins.visits.retain(|v| v.ticket != ticket || v.left);
        } else {
            self.end_visit(ticket, effects);
        }
    }

    /// A visit starts when the keyboard is in the plugin's window and none is under way.
    /// Only a plugin of the song is visited: what is changed in one the shelf auditions is
    /// no step.
    pub(super) fn start_visit(&mut self, ticket: u64, effects: &mut Vec<Effect>) {
        let of_song = (self.plugs.of(ticket))
            .is_some_and(|plug| plug.sounds() && self.place_of(plug.spot).is_some());
        let wins = &mut self.wins;
        let visited = wins.visits.iter().any(|v| v.ticket == ticket && !v.left);
        if !of_song || visited || !wins.inside.contains(&ticket) {
            return;
        }
        wins.next_visit += 1;
        let serial = wins.next_visit;
        wins.visits.push(Visit {
            ticket,
            serial,
            before: None,
            left: false,
            unanswered: false,
            hands: None,
            wrote: 0,
        });
        effects.push(Effect::AskPlugin {
            ticket,
            ask: PluginAsk::State(StateAsk::Entered(serial)),
        });
    }

    /// Ends the visit under way, if there is one: the plugin's state is asked for, and the
    /// answer makes the step. Returns whether there was one.
    pub(super) fn end_visit(&mut self, ticket: u64, effects: &mut Vec<Effect>) -> bool {
        let visits = &mut self.wins.visits;
        let Some(visit) = visits.iter_mut().find(|v| v.ticket == ticket && !v.left) else {
            return false;
        };
        visit.left = true;
        effects.push(Effect::AskPlugin {
            ticket,
            ask: PluginAsk::State(StateAsk::Left(visit.serial)),
        });
        true
    }

    /// Before the parameters panel changes the plugin: what the window changed until now
    /// is a step of its own, and the visit goes on as a new one.
    pub(super) fn cut_visit(&mut self, ticket: u64, effects: &mut Vec<Effect>) {
        if self.end_visit(ticket, effects) {
            self.start_visit(ticket, effects);
        }
    }

    /// The panel changed the plugin under a visit, and this is the plugin's state since:
    /// what the visit compares with at its end, so that only what the window changes from
    /// here on is its step.
    pub(super) fn visit_goes_on_from(&mut self, ticket: u64, state: &Arc<[u8]>) {
        let visits = &mut self.wins.visits;
        if let Some(visit) = visits.iter_mut().find(|v| v.ticket == ticket && !v.left) {
            visit.before = Some(state.clone());
        }
    }

    /// The visit under way, if there is one, starts anew: what it compares with at its end
    /// is the plugin as it is now.
    pub(super) fn visit_anew(&mut self, ticket: u64, effects: &mut Vec<Effect>) {
        let visits = &mut self.wins.visits;
        let before = visits.len();
        visits.retain(|v| v.ticket != ticket || v.left);
        if visits.len() != before {
            self.start_visit(ticket, effects);
        }
    }

    /// A state was put back into the plugin (a step back, or forward again): what a visit
    /// that is under way compares with is the plugin as it is from now on, and one that
    /// just ended is no step.
    pub(super) fn visit_overtaken(&mut self, ticket: u64, effects: &mut Vec<Effect>) {
        self.wins.visits.retain(|v| v.ticket != ticket);
        self.start_visit(ticket, effects);
    }

    /// The plugin said its state for a visit.
    pub(super) fn visit_state(
        &mut self,
        ticket: u64,
        ask: StateAsk,
        state: Option<Arc<[u8]>>,
        effects: &mut Vec<Effect>,
    ) {
        let visits = &mut self.wins.visits;
        match ask {
            StateAsk::Entered(serial) => {
                let Some(at) = visits.iter().position(|v| v.serial == serial) else {
                    return;
                };
                match state {
                    Some(state) => {
                        visits[at].before = Some(state.clone());
                        self.base_said(ticket, &state);
                    }
                    // Nothing to compare with: this visit cannot be a step.
                    None if !visits[at].left => drop(visits.remove(at)),
                    None => {}
                }
            }
            StateAsk::Left(serial) => {
                let Some(at) = visits.iter().position(|v| v.serial == serial) else {
                    return;
                };
                let Some(after) = state else {
                    // Its process stopped. Loaded again, the plugin comes back with what
                    // was changed in its window: the visit ends then.
                    (visits[at].left, visits[at].unanswered) = (false, true);
                    return;
                };
                let visit = visits.remove(at);
                // With no hand in the window, what differs is what a lane moved.
                let changed = visit.before.is_some_and(|before| before != after);
                if changed && visit.hands.is_none_or(|hands| hands > visit.wrote) {
                    self.plugin_changed(ticket, after, PluginChange::Window, effects);
                } else {
                    let rests = self.take_window_rests(ticket, effects);
                    self.apply_resting(None, &rests, true, effects);
                }
            }
            StateAsk::Base | StateAsk::Param { .. } => {}
        }
    }

    /// After every input: the windows of plugins that are gone, stopped, or loaded under
    /// another ticket are gone with them; no news comes of those. A visit to a plugin whose
    /// process stopped waits: the process that is started again for it gives the plugin
    /// what was changed in its window, and the visit ends then, with its step.
    pub fn sync_windows(&mut self, effects: &mut Vec<Effect>) {
        let plugs = &self.plugs;
        let there = |ticket: u64| plugs.of(ticket).is_some_and(Plug::sounds);
        let wins = &mut self.wins;
        wins.open.retain(|w| there(w.ticket));
        wins.opening.retain(|(t, _)| there(*t));
        wins.inside.retain(|t| there(*t));
        wins.visits.retain(|v| plugs.of(v.ticket).is_some());
        let mut back = Vec::new();
        for visit in &mut wins.visits {
            let sounds = there(visit.ticket);
            if !sounds {
                visit.unanswered = false;
            } else if !visit.left && !visit.unanswered && !wins.inside.contains(&visit.ticket) {
                back.push(visit.ticket);
            }
        }
        for ticket in back {
            self.end_visit(ticket, effects);
        }
        self.drop_gone_touches(effects);
    }

    /// The hands of the visit under way, or of the last one that waits for its state.
    pub(super) fn visit_hands(&mut self, ticket: u64, count: u32) {
        let visits = &mut self.wins.visits;
        if let Some(visit) = visits.iter_mut().rev().find(|v| v.ticket == ticket) {
            visit.hands = Some(visit.hands.unwrap_or(0).saturating_add(count));
        }
    }

    /// A hand in the window of the visit under way wrote into a lane.
    pub(super) fn visit_wrote(&mut self, ticket: u64) {
        let visits = &mut self.wins.visits;
        if let Some(visit) = visits.iter_mut().find(|v| v.ticket == ticket && !v.left) {
            visit.wrote += 1;
        }
    }

    /// Whether a visit to a plugin's window is under way.
    #[cfg(test)]
    pub(crate) fn visiting(&self) -> bool {
        self.wins.visits.iter().any(|v| !v.left)
    }

    pub fn windows_view(&self) -> WindowsView {
        let list = (self.wins.open.iter())
            .filter_map(|open| {
                let plug = self.plugs.of(open.ticket)?;
                Some(PluginWindowView {
                    slot: self.window_slot(plug),
                    name: plug.plugin.name.to_string(),
                    width: open.width,
                    height: open.height,
                    larger_than_screen: open.larger_than_screen,
                })
            })
            .collect();
        WindowsView { list }
    }

    /// The status line while a window that was asked for takes long to come up.
    pub fn window_status(&self) -> Option<StatusMessage> {
        let slow = |(_, asked): &&(u64, u64)| self.now_us >= asked + SLOW_US;
        let (ticket, _) = self.wins.opening.iter().find(slow)?;
        let plug = self.plugs.of(*ticket)?;
        Some(StatusMessage::PluginWindowOpening {
            name: plug.plugin.name.to_string(),
            slot: self.window_slot(plug),
        })
    }

    /// When the next window that is being opened counts as slow.
    pub(super) fn windows_wake_us(&self) -> Option<u64> {
        (self.wins.opening.iter())
            .map(|(_, asked)| asked + SLOW_US)
            .filter(|at| *at > self.now_us)
            .min()
    }
}

#[cfg(test)]
pub(super) mod tests;

//! Tells the engine what the mixer is to sound like: the song's values with the previews of
//! the controls being dragged on top. Only what differs from what the engine was told last
//! is sent, strip by strip (docs/architecture.md 19-11).

use std::sync::Arc;

use meloet_model::mixer::{insert_moves, move_slot, moved_slot, new_track_sends};
use meloet_model::{
    Bus, Channel, Gain, INSERT_SLOTS, Insert, Inserts, Pan, PluginPlace, PluginRef, Sends, StripId,
};

use super::mixer::Preview;
use super::plugs::{Spot, Want};
use super::seats::AUDITION_SEAT;
use super::shelf::Mode;
use super::{EngineOp, Play, StripSeat};
use crate::state::Effect;

/// What the engine was told about one strip. A new one is the engine's strip before it is
/// told anything.
#[derive(Debug, Clone, PartialEq)]
struct Told {
    gain: Gain,
    pan: Pan,
    sends: Sends,
    inserts: Inserts,
    /// The slots it was told to pass by. A mark of the slot, apart from what it holds.
    bypassed: [bool; INSERT_SLOTS],
}

impl Told {
    fn new() -> Told {
        Told {
            gain: Gain::UNITY,
            pan: Pan::CENTER,
            sends: [Gain::SILENT; 2],
            inserts: Inserts::default(),
            bypassed: [false; INSERT_SLOTS],
        }
    }
}

#[derive(Debug)]
pub(super) struct MixTold {
    /// By seat; the audition seat is the last.
    tracks: Vec<Told>,
    returns: [Told; 2],
    master: Told,
}

impl MixTold {
    pub fn new() -> MixTold {
        MixTold {
            tracks: vec![Told::new(); AUDITION_SEAT + 1],
            returns: [Told::new(), Told::new()],
            master: Told::new(),
        }
    }

    /// The engine emptied the seat (`EngineOp::ClearSeat`): its strip is a new one.
    pub fn seat_cleared(&mut self, seat: usize) {
        if let Some(told) = self.tracks.get_mut(seat) {
            *told = Told::new();
        }
    }

    fn strip(&self, strip: StripSeat) -> Option<&Told> {
        match strip {
            StripSeat::Track(seat) => self.tracks.get(seat),
            StripSeat::Return(bus) => Some(&self.returns[bus.index()]),
            StripSeat::Master => Some(&self.master),
        }
    }

    fn strip_mut(&mut self, strip: StripSeat) -> Option<&mut Told> {
        match strip {
            StripSeat::Track(seat) => self.tracks.get_mut(seat),
            StripSeat::Return(bus) => Some(&mut self.returns[bus.index()]),
            StripSeat::Master => Some(&mut self.master),
        }
    }
}

/// One strip as it is to sound.
struct Wanted<'a> {
    seat: StripSeat,
    /// What the previews name it by; `None` for the audition seat.
    id: Option<StripId>,
    channel: &'a Channel,
    sends: Sends,
    /// A muted return: its fader counts as −∞.
    muted: bool,
}

/// An insert slot and the plugin the song has in it now; `None` when it held one before and
/// holds none now.
type PlugAsk = (StripSeat, usize, Option<Arc<PluginRef>>);

fn sync_strip(
    told: &mut Told,
    want: Wanted,
    previews: &[Preview],
    plugs: &mut Vec<PlugAsk>,
    effects: &mut Vec<Effect>,
) {
    let mine = |strip: &StripId| Some(*strip) == want.id;
    let mut gain = want.channel.gain;
    let mut pan = want.channel.pan;
    let mut sends = want.sends;
    for p in previews {
        match p {
            Preview::Gain(strip, g) if mine(strip) => gain = *g,
            Preview::Pan(strip, p) if mine(strip) => pan = *p,
            Preview::Send(track, bus, g) if mine(&StripId::Track(*track)) => {
                sends[bus.index()] = *g;
            }
            _ => {}
        }
    }
    if want.muted {
        gain = Gain::SILENT;
    }
    let strip = want.seat;
    if (gain, pan) != (told.gain, told.pan) {
        (told.gain, told.pan) = (gain, pan);
        effects.push(Effect::Engine(EngineOp::Strip { strip, gain, pan }));
    }
    if let (StripSeat::Track(seat), true) = (strip, sends != told.sends) {
        told.sends = sends;
        effects.push(Effect::Engine(EngineOp::Sends { seat, sends }));
    }
    let told_bypassed = &mut told.bypassed;
    for (slot, (told, insert)) in (told.inserts.iter_mut())
        .zip(&want.channel.inserts)
        .enumerate()
    {
        let insert = insert.as_ref();
        let bypassed = insert.is_some_and(|i| i.bypassed);
        if told_bypassed[slot] != bypassed {
            told_bypassed[slot] = bypassed;
            effects.push(Effect::Engine(EngineOp::Bypass {
                strip,
                slot,
                bypassed,
            }));
        }
        if let Some(plugin) = insert.and_then(Insert::plugin_ref) {
            // A plugin is not made here: the book loads it and puts it into the slot, which
            // is empty until then. An effect of meloet that was there goes now.
            if told.as_ref().is_some_and(|t| t.kind().is_some()) {
                effects.push(Effect::Engine(EngineOp::Insert {
                    strip,
                    slot,
                    insert: None,
                }));
            }
            if told.as_ref() != insert {
                told.clone_from(&insert.cloned());
            }
            plugs.push((strip, slot, Some(plugin.clone())));
            continue;
        }
        if told.as_ref().is_some_and(|t| t.kind().is_none()) {
            plugs.push((strip, slot, None));
        }
        let previewed = |param: u32, value: f32| {
            let hit = previews.iter().find_map(|p| match p {
                Preview::Param {
                    strip: s,
                    slot: at,
                    kind,
                    param: n,
                    value,
                } if mine(s)
                    && *at == slot
                    && Some(*kind) == insert.and_then(Insert::kind)
                    && *n == param =>
                {
                    Some(*value)
                }
                _ => None,
            });
            hit.unwrap_or(value)
        };
        match (told.as_mut(), insert) {
            (Some(told), Some(insert)) if told.kind().is_some() && told.kind() == insert.kind() => {
                // The effect of the slot is this insert's from now on, as it is when the
                // song is another one: a later move finds it by the id.
                told.id = insert.id;
                for (param, value) in insert.values().iter().enumerate() {
                    let param = param as u32;
                    let value = previewed(param, *value);
                    if told.value(param) != Some(value) {
                        told.set(param, value);
                        effects.push(Effect::Engine(EngineOp::Param {
                            strip,
                            slot,
                            param,
                            value,
                        }));
                    }
                }
            }
            (None, None) => {}
            // Another effect, one more or one less: only then is an effect made or put away.
            _ => {
                let mut insert = insert.cloned();
                if let Some(insert) = insert.as_mut() {
                    for param in 0..insert.values().len() as u32 {
                        let value = previewed(param, insert.values()[param as usize]);
                        insert.set(param, value);
                    }
                }
                told.clone_from(&insert);
                effects.push(Effect::Engine(EngineOp::Insert {
                    strip,
                    slot,
                    insert,
                }));
            }
        }
    }
}

impl Play {
    /// Tells the engine what changed in the mixer since it was last told. After every input
    /// and every change of the song; when nothing changed it sends nothing.
    pub fn sync_mix(&mut self, effects: &mut Vec<Effect>) {
        let doc = &self.song.doc;
        let previews = self.mixer.previews();
        let told = &mut self.mix_told;
        let asks = &mut Vec::new();
        for t in &doc.tracks {
            let Some(seat) = self.seats.of(t.id) else {
                continue;
            };
            let want = Wanted {
                seat: StripSeat::Track(seat),
                id: Some(StripId::Track(t.id)),
                channel: &t.channel,
                sends: t.sends,
                muted: false,
            };
            sync_strip(&mut told.tracks[seat], want, previews, asks, effects);
        }
        // An instrument tried for a new track sounds as the track will: with the sends a
        // track of its group starts with.
        let plain = Channel::default();
        let audition = Wanted {
            seat: StripSeat::Track(AUDITION_SEAT),
            id: None,
            channel: &plain,
            sends: match (self.shelf.mode, &self.shelf.trial) {
                (Mode::NewTrack, Some(instrument)) => new_track_sends(instrument),
                _ => [Gain::SILENT; 2],
            },
            muted: false,
        };
        sync_strip(
            &mut told.tracks[AUDITION_SEAT],
            audition,
            previews,
            asks,
            effects,
        );
        for bus in Bus::ALL {
            let r = &doc.returns[bus.index()];
            let want = Wanted {
                seat: StripSeat::Return(bus),
                id: Some(StripId::Return(bus)),
                channel: &r.channel,
                sends: [Gain::SILENT; 2],
                muted: r.muted,
            };
            sync_strip(
                &mut told.returns[bus.index()],
                want,
                previews,
                asks,
                effects,
            );
        }
        let master = Wanted {
            seat: StripSeat::Master,
            id: Some(StripId::Master),
            channel: &doc.master,
            sends: [Gain::SILENT; 2],
            muted: false,
        };
        sync_strip(&mut told.master, master, previews, asks, effects);
        for (strip, slot, plugin) in std::mem::take(asks) {
            let spot = Spot::Insert(strip, slot);
            let Some(plugin) = plugin else {
                self.drop_plug(spot, effects);
                continue;
            };
            let held = self.plug_ticket(spot).is_some();
            // Another plugin was in the slot, and the engine still has it.
            if self.want_plug(spot, &plugin, None, effects) != Want::Kept && held {
                effects.push(Effect::Engine(EngineOp::Insert {
                    strip,
                    slot,
                    insert: None,
                }));
            }
        }
    }

    /// The song is another `Doc` of its history (a step, one taken back, a jump over many):
    /// an insert the engine was told of that is in another slot of its strip now, told by
    /// its `InsertId`, is moved there as it sounds, and whatever names a slot of the strip
    /// follows. [`Play::sync_mix`] then finds it where it is, and makes nothing anew for it.
    pub(super) fn inserts_follow(&mut self, effects: &mut Vec<Effect>) {
        let doc = &self.song.doc;
        let tracks = doc
            .tracks
            .iter()
            .map(|t| (StripId::Track(t.id), &t.channel));
        let returns = Bus::ALL.map(|bus| (StripId::Return(bus), &doc.returns[bus.index()].channel));
        let master: &Channel = &doc.master;
        let mut moved = Vec::new();
        for (strip, channel) in tracks.chain(returns).chain([(StripId::Master, master)]) {
            let seat = self.strip_seat(strip);
            let Some((seat, told)) = seat.and_then(|s| Some((s, self.mix_told.strip(s)?))) else {
                continue;
            };
            for (from, to) in insert_moves(&told.inserts, &channel.inserts) {
                moved.push((strip, seat, from, to));
            }
        }
        for (strip, seat, from, to) in moved {
            let follow = |place: PluginPlace| match place {
                PluginPlace::Insert(s, slot) if s == strip => {
                    PluginPlace::Insert(s, moved_slot(slot, from, to))
                }
                other => other,
            };
            self.selection_follows(follow);
            self.shelf_follows(strip, from, to);
            if let Some(told) = self.mix_told.strip_mut(seat) {
                move_slot(&mut told.inserts, from, to);
                move_slot(&mut told.bypassed, from, to);
            }
            self.plugs_follow(seat, from, to);
            effects.push(Effect::Engine(EngineOp::MoveInsert {
                strip: seat,
                from,
                to,
            }));
        }
    }

    /// The seat's track is gone: the engine empties the seat, strip and all.
    pub(super) fn clear_seat(&mut self, seat: usize, effects: &mut Vec<Effect>) {
        self.mix_told.seat_cleared(seat);
        self.seat_plugs_gone(seat, effects);
        effects.push(Effect::Engine(EngineOp::ClearSeat { seat }));
    }
}

#[cfg(test)]
mod tests;

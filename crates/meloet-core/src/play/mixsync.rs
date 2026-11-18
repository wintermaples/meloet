//! Tells the engine what the mixer is to sound like: the song's values with the previews of
//! the controls being dragged on top. Only what differs from what the engine was told last
//! is sent, strip by strip (docs/architecture.md 19-11).

use meloet_model::mixer::new_track_sends;
use meloet_model::{Bus, Channel, Gain, Insert, Inserts, Pan, Sends, StripId};

use super::mixer::Preview;
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
}

impl Told {
    fn new() -> Told {
        Told {
            gain: Gain::UNITY,
            pan: Pan::CENTER,
            sends: [Gain::SILENT; 2],
            inserts: Inserts::default(),
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

fn sync_strip(told: &mut Told, want: Wanted, previews: &[Preview], effects: &mut Vec<Effect>) {
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
    for (slot, (told, insert)) in (told.inserts.iter_mut())
        .zip(&want.channel.inserts)
        .enumerate()
    {
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
                    && Some(*kind) == insert.as_ref().map(Insert::kind)
                    && *n == param =>
                {
                    Some(*value)
                }
                _ => None,
            });
            hit.unwrap_or(value)
        };
        match (told.as_mut(), insert) {
            (Some(told), Some(insert)) if told.kind() == insert.kind() => {
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
                let mut insert = insert.clone();
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
            sync_strip(&mut told.tracks[seat], want, previews, effects);
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
        sync_strip(&mut told.tracks[AUDITION_SEAT], audition, previews, effects);
        for bus in Bus::ALL {
            let r = &doc.returns[bus.index()];
            let want = Wanted {
                seat: StripSeat::Return(bus),
                id: Some(StripId::Return(bus)),
                channel: &r.channel,
                sends: [Gain::SILENT; 2],
                muted: r.muted,
            };
            sync_strip(&mut told.returns[bus.index()], want, previews, effects);
        }
        let master = Wanted {
            seat: StripSeat::Master,
            id: Some(StripId::Master),
            channel: &doc.master,
            sends: [Gain::SILENT; 2],
            muted: false,
        };
        sync_strip(&mut told.master, master, previews, effects);
    }

    /// The seat's track is gone: the engine empties the seat, strip and all.
    pub(super) fn clear_seat(&mut self, seat: usize, effects: &mut Vec<Effect>) {
        self.mix_told.seat_cleared(seat);
        effects.push(Effect::Engine(EngineOp::ClearSeat { seat }));
    }
}

#[cfg(test)]
mod tests;

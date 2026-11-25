//! What the engine is told of the automation (docs/architecture.md 21-7): for each seat, the
//! lanes of its track that are not resting, in one set; and the lanes a person has hold of.
//!
//! The same one way for every change of the song (a step, undo, redo, a jump in the
//! history, another song) and for a new stream: the lanes a seat was last sent are kept by
//! their heads (what, which points, which slot), and a seat whose heads differ gets its
//! whole set again. The points are copied only then.

use std::collections::HashMap;

use meloet_model::effects::EffectKind;
use meloet_model::{AutoTarget, InstrumentRef, Lane, LaneHome, Track};

use super::{EngineOp, Play, StripSeat};
use crate::state::Effect;

/// One lane of a track as the engine plays it.
#[derive(Debug, Clone, PartialEq)]
pub struct SeatLane {
    pub target: AutoTarget,
    /// The value stays at the point before until the next one.
    pub step: bool,
    /// Tick and place (0..=1), ticks rising.
    pub points: Vec<(u64, f32)>,
    /// For a parameter of an insert: the slot it is in now.
    pub slot: usize,
    /// For a parameter of an insert that comes with meloet: its kind, whose table gives the
    /// values. `None` for a plugin.
    pub effect: Option<EffectKind>,
}

/// What tells two sets of lanes apart without their points: the lane's number changes
/// whenever its points do.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Head {
    target: AutoTarget,
    step: bool,
    lane: u32,
    slot: usize,
    effect: Option<EffectKind>,
}

#[derive(Debug, Default)]
pub(crate) struct AutoSync {
    /// The heads each seat was last sent.
    sent: HashMap<usize, Vec<Head>>,
    /// The lane of the master's fader as it was last sent: its number, when it plays.
    master_sent: Option<u32>,
    /// The lanes a person has hold of ([`Play::hold_lane`]).
    held: Vec<(LaneHome, AutoTarget)>,
}

/// The lane of the master's fader, when it plays (not resting, with points).
fn master_lane(doc: &meloet_model::Doc) -> Option<&Lane> {
    (doc.lane_of(LaneHome::Master, AutoTarget::Volume))
        .map(|l| &**l)
        .filter(|l| l.moves())
}

fn master_lanes(lane: Option<&Lane>) -> Vec<SeatLane> {
    (lane.into_iter())
        .map(|l| SeatLane {
            target: AutoTarget::Volume,
            step: l.step,
            points: l.points.iter().map(|p| (p.tick, p.value)).collect(),
            slot: 0,
            effect: None,
        })
        .collect()
}

/// The lanes of a track that play (not resting, with points): target, step, the lane's
/// number.
fn heads_of(track: &Track) -> Vec<(AutoTarget, bool, u32)> {
    (track.automation.iter())
        .filter(|l| l.moves())
        .map(|l| (l.target, l.step, l.id.0))
        .collect()
}

/// The points of the lane of `target` of a track: tick and place.
fn points_of(track: &Track, target: AutoTarget) -> Vec<(u64, f32)> {
    (track.lane(target))
        .map(|l| l.points.iter().map(|p| (p.tick, p.value)).collect())
        .unwrap_or_default()
}

/// The heads of the lanes of `track` the engine can play: a lane of an insert whose effect
/// is not on the track (a plugin not found) is left out.
fn heads(track: &Track) -> Vec<Head> {
    heads_of(track)
        .into_iter()
        .filter_map(|(target, step, lane)| {
            let (slot, effect) = match target {
                AutoTarget::Insert { insert, .. } => {
                    let inserts = track.channel.inserts.iter().enumerate();
                    inserts
                        .filter_map(|(slot, i)| Some((slot, i.as_ref()?)))
                        .find(|(_, i)| i.id == insert)
                        .map(|(slot, i)| (slot, i.kind()))?
                }
                _ => (0, None),
            };
            Some(Head {
                target,
                step,
                lane,
                slot,
                effect,
            })
        })
        .collect()
}

fn seat_lanes(track: &Track, heads: &[Head]) -> Vec<SeatLane> {
    (heads.iter())
        .map(|h| SeatLane {
            target: h.target,
            step: h.step,
            points: points_of(track, h.target),
            slot: h.slot,
            effect: h.effect,
        })
        .collect()
}

impl Play {
    /// Tells the engine the lanes of every seat whose lanes changed since it was last told;
    /// with `all`, of every seat (a new stream, another song). Call after the seats and the
    /// mixer are synced.
    pub(crate) fn send_automation(&mut self, all: bool, effects: &mut Vec<Effect>) {
        let doc = &self.song.doc;
        let mut now: HashMap<usize, (&Track, Vec<Head>)> = HashMap::new();
        for track in &doc.tracks {
            if let Some(seat) = self.seats.of(track.id) {
                let mut heads = heads(track);
                // A plugin this PC does not have sounds as the sine wave, whose knobs its
                // lanes are not for.
                if let InstrumentRef::Plugin(p) = &track.instrument
                    && !self.has_plugin(&p.key)
                {
                    heads.retain(|h| !matches!(h.target, AutoTarget::Instrument { .. }));
                }
                now.insert(seat, (track, heads));
            }
        }
        if all {
            self.autosync.sent.clear();
            self.autosync
                .held
                .retain(|(home, _)| doc.lanes(*home).is_some());
        }
        // The engine outlives a stream and a song, and is told of lanes only while a stream
        // runs: a new stream, or another song, is told even that the master or a seat plays
        // no lane.
        let master = master_lane(doc);
        if all || master.map(|l| l.id.0) != self.autosync.master_sent {
            self.autosync.master_sent = master.map(|l| l.id.0);
            effects.push(Effect::Engine(EngineOp::MasterAutomation {
                lanes: master_lanes(master),
            }));
        }
        let gone: Vec<usize> = (self.autosync.sent.keys())
            .filter(|seat| !now.contains_key(seat))
            .copied()
            .collect();
        for seat in gone {
            if self
                .autosync
                .sent
                .remove(&seat)
                .is_some_and(|h| !h.is_empty())
            {
                effects.push(Effect::Engine(EngineOp::Automation {
                    seat,
                    lanes: Vec::new(),
                }));
            }
        }
        let mut seats: Vec<usize> = now.keys().copied().collect();
        seats.sort_unstable();
        for seat in seats {
            let (track, heads) = &now[&seat];
            let before = self.autosync.sent.get(&seat);
            if before.map_or(!all && heads.is_empty(), |b| b == heads) {
                continue;
            }
            // An effect that comes with meloet keeps the value its lane left it at: the
            // hand's value goes back to it when its lane stops playing.
            for gone in before.into_iter().flatten() {
                let (AutoTarget::Insert { insert, param }, Some(_)) = (gone.target, gone.effect)
                else {
                    continue;
                };
                let still = heads.iter().any(|h| h.target == gone.target);
                let at = (track.channel.inserts.iter().enumerate())
                    .find(|(_, i)| i.as_ref().is_some_and(|i| i.id == insert));
                if let (false, Some((slot, Some(i)))) = (still, at)
                    && let Some(value) = i.value(param)
                {
                    effects.push(Effect::Engine(EngineOp::Param {
                        strip: StripSeat::Track(seat),
                        slot,
                        param,
                        value,
                    }));
                }
            }
            let lanes = seat_lanes(track, heads);
            effects.push(Effect::Engine(EngineOp::Automation { seat, lanes }));
            self.autosync.sent.insert(seat, heads.clone());
        }
        if all {
            for &(home, target) in &self.autosync.held {
                if let Some(op) = self.hold_op(home, target, true) {
                    effects.push(Effect::Engine(op));
                }
            }
        }
    }

    /// The lanes of `seat` go to the engine again as they are, which then gives its plugins
    /// their values anew: after a plugin's state was put back, or it was loaded again
    /// (docs/architecture.md 21-10). Lanes that changed since they were last told are left
    /// to [`Play::send_automation`].
    pub(crate) fn resend_automation(&mut self, seat: usize, effects: &mut Vec<Effect>) {
        let doc = &self.song.doc;
        let seats = &self.seats;
        let Some(track) = doc.tracks.iter().find(|t| seats.of(t.id) == Some(seat)) else {
            return;
        };
        let heads = heads(track);
        if heads.is_empty() || self.autosync.sent.get(&seat) != Some(&heads) {
            return;
        }
        let lanes = seat_lanes(track, &heads);
        effects.push(Effect::Engine(EngineOp::Automation { seat, lanes }));
    }

    /// A person has hold of what the lane of `target` of `home` moves (`on`), or let go of
    /// it: until then the lane plays nothing and the value is the hand's, as the preview
    /// sets it (docs/architecture.md 21-9, 21-11). Kept for the track, or the master, across
    /// changes of its lanes, and told again to a new stream.
    pub fn hold_lane(
        &mut self,
        home: impl Into<LaneHome>,
        target: AutoTarget,
        on: bool,
        effects: &mut Vec<Effect>,
    ) {
        let home = home.into();
        let held = &mut self.autosync.held;
        let was = held.iter().position(|h| *h == (home, target));
        match (was, on) {
            (None, true) => held.push((home, target)),
            (Some(at), false) => {
                held.swap_remove(at);
            }
            _ => return,
        }
        if let Some(op) = self.hold_op(home, target, on) {
            effects.push(Effect::Engine(op));
        }
    }

    fn hold_op(&self, home: LaneHome, target: AutoTarget, on: bool) -> Option<EngineOp> {
        match home {
            LaneHome::Track(track) => {
                let seat = self.seats.of(track)?;
                Some(EngineOp::HoldLane { seat, target, on })
            }
            LaneHome::Master => Some(EngineOp::HoldMasterLane { on }),
        }
    }

    /// The lanes held now.
    pub fn held_lanes(&self) -> &[(LaneHome, AutoTarget)] {
        &self.autosync.held
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::play::tests::{cmd, ready};

    fn ops(effects: &[Effect]) -> Vec<&EngineOp> {
        (effects.iter())
            .filter_map(|e| match e {
                Effect::Engine(op @ (EngineOp::HoldLane { .. } | EngineOp::Automation { .. })) => {
                    Some(op)
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_held_lane_is_told_once_and_again_to_a_new_stream() {
        let mut play = Play::new(false, 1);
        let track = play.doc().tracks[0].id;
        let seat = play.seats.of(track).unwrap();
        let mut effects = Vec::new();
        play.hold_lane(track, AutoTarget::Volume, true, &mut effects);
        play.hold_lane(track, AutoTarget::Volume, true, &mut effects);
        assert_eq!(
            ops(&effects),
            [&EngineOp::HoldLane {
                seat,
                target: AutoTarget::Volume,
                on: true
            }]
        );
        effects.clear();
        play.stream_opened(&mut effects);
        assert!(ops(&effects).contains(&&EngineOp::HoldLane {
            seat,
            target: AutoTarget::Volume,
            on: true
        }));
        effects.clear();
        play.hold_lane(track, AutoTarget::Volume, false, &mut effects);
        assert_eq!(ops(&effects).len(), 1);
        assert!(play.held_lanes().is_empty());
    }

    #[test]
    fn a_song_without_lanes_tells_a_new_stream_so_and_then_nothing() {
        let mut play = Play::new(false, 1);
        let seat = play.seats.of(play.doc().tracks[0].id).unwrap();
        let mut effects = Vec::new();
        play.stream_opened(&mut effects);
        assert_eq!(
            ops(&effects),
            [&EngineOp::Automation {
                seat,
                lanes: Vec::new()
            }]
        );
        effects.clear();
        play.send_automation(false, &mut effects);
        assert!(ops(&effects).is_empty());
    }

    /// A volume lane with points on track 1.
    fn with_volume_lane(m: &mut crate::state::Model) {
        use meloet_protocol::{AutoTarget, Command, LaneHome};
        let home = LaneHome::Track { track: 1 };
        let target = AutoTarget::Volume;
        cmd(m, Command::AddLane { home, target });
        let mut effects = Vec::new();
        for tick in [0, 4 * meloet_model::TICKS_PER_QUARTER] {
            // The lane's number changes with its points.
            let view = m.view();
            let lane = (view.song.tracks[0].lanes.iter())
                .find(|l| l.target == target)
                .expect("the lane")
                .lane_id;
            let value = Some(0.0);
            effects = cmd(m, Command::AddAutoPoint { lane, tick, value });
        }
        assert_eq!(told(&effects), [(0, 1)], "the lane plays on seat 0");
    }

    /// The seats told of their lanes, and how many lanes each.
    fn told(effects: &[Effect]) -> Vec<(usize, usize)> {
        (ops(effects).into_iter())
            .filter_map(|op| match op {
                EngineOp::Automation { seat, lanes } => Some((*seat, lanes.len())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn another_song_leaves_no_lane_of_the_one_before_on_its_seats() {
        use meloet_protocol::{Command, SongAnswer};
        let m = &mut ready();
        with_volume_lane(m);
        cmd(m, Command::NewSong);
        let dialog = (m.view().workspace.song_dialog)
            .expect("the song has changes")
            .id;
        let answer = SongAnswer::DontSave;
        let effects = cmd(m, Command::AnswerSongDialog { dialog, answer });
        let play = m.play();
        let mut seats: Vec<usize> = (play.doc().tracks.iter())
            .filter_map(|t| play.seats.of(t.id))
            .collect();
        seats.sort_unstable();
        assert!(seats.contains(&0), "the seat of the lane before");
        let none: Vec<(usize, usize)> = seats.iter().map(|&seat| (seat, 0)).collect();
        assert_eq!(told(&effects), none);
    }

    #[test]
    fn a_new_stream_hears_of_a_lane_deleted_while_none_ran() {
        use meloet_protocol::{AutoTarget, Command, LaneHome};
        let m = &mut ready();
        with_volume_lane(m);
        // What the engine is told of while no stream runs is lost.
        let home = LaneHome::Track { track: 1 };
        let target = AutoTarget::Volume;
        cmd(m, Command::DeleteLane { home, target });
        let mut effects = Vec::new();
        m.play_mut().stream_opened(&mut effects);
        assert!(told(&effects).contains(&(0, 0)));
    }
}

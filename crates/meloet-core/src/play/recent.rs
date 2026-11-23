//! 「さっきのを残す」: what was played on the keys target track while not recording is held
//! here as one phrase, and becomes a clip when asked for.
//!
//! The engine says every key played outside a recording with the run of the transport it
//! was played in ([`Played`]). A phrase is the keys of one run, from a key press on:
//!
//! - an event of another run, or the end of the phrase's run, closes the phrase; the next
//!   key press after that starts a new phrase in place of it;
//! - in a run that stands still, a key press that comes after no key and no pedal was held
//!   for the time of two bars starts a new phrase too;
//! - what is not a key press and has no open phrase of its run to go to is dropped. The
//!   controller values are followed all the same, for the head of the next phrase.

use std::collections::VecDeque;

use meloet_model::edit::{self, Edit, EditError, RecordingAdded};
use meloet_model::record::{HeldControls, RecEvent, RecEventKind, Recorder, overdub_recording};
use meloet_model::song::{CC_MODULATION, CC_SUSTAIN};
use meloet_model::{StepKind, TICKS_PER_QUARTER, TickRange, TrackId};
use meloet_protocol::StatusMessage;

use super::{ClipPick, Play};
use crate::state::Effect;

/// The most events a phrase holds; the oldest go first.
const MAX_EVENTS: usize = 4096;
/// The longest a phrase reaches back from its newest event.
const MAX_AGE_US: u64 = 10 * 60 * 1_000_000;
/// The pedal counts as down from here.
const PEDAL_DOWN: u8 = 64;

/// Where the listener was in a run that plays.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayedAt {
    pub tick: f64,
    /// How often the loop had gone round in the run.
    pub lap: u32,
}

/// What the engine says of the keys played outside a recording, in the order it happened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Played {
    /// `at`: `None` in a run that stands still.
    Key {
        kind: RecEventKind,
        t_us: u64,
        run: u32,
        at: Option<PlayedAt>,
    },
    /// `at`: where the listener was then, when the run played and the place is known.
    RunEnded {
        run: u32,
        t_us: u64,
        at: Option<PlayedAt>,
    },
}

/// What is held on the keyboards now, as the thread that reads them knows it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HeldNow {
    /// The keys that are down, one bit each.
    pub keys: u128,
    pub controls: HeldControls,
}

/// The run the listener is in now, read with the time of each input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeardRun {
    pub run: u32,
    /// `None` while the run stands still.
    pub at: Option<PlayedAt>,
}

#[derive(Debug, Clone, Copy)]
struct Ev {
    kind: RecEventKind,
    t_us: u64,
    /// Not looked at in a phrase that stands still.
    at: PlayedAt,
}

#[derive(Debug)]
struct Phrase {
    run: u32,
    playing: bool,
    track: TrackId,
    /// The loop the run went round.
    loop_range: Option<TickRange>,
    /// The controller values just before the first event.
    head: HeldControls,
    /// Starts with a key press.
    events: VecDeque<Ev>,
    /// Key presses in `events`.
    notes: usize,
    closed: bool,
    /// When and where the run ended, once the engine said so.
    end: Option<(u64, Option<PlayedAt>)>,
}

/// What the keys track is and how it loops when a key comes in.
#[derive(Debug, Clone, Copy)]
pub(super) struct Around {
    pub track: Option<TrackId>,
    pub loop_range: Option<TickRange>,
    /// The time of two bars where the play line is.
    pub two_bars_us: u64,
}

#[derive(Debug, Default)]
pub(super) struct Recent {
    phrase: Option<Phrase>,
    /// `down` and `controls` were followed without a gap since they were last read from the
    /// keyboard's own record.
    synced: bool,
    /// The keys that are down, one bit each.
    down: u128,
    controls: HeldControls,
    /// Since when no key and no pedal is held; `None` while one is.
    idle_since: Option<u64>,
}

fn is_press(kind: RecEventKind) -> bool {
    matches!(kind, RecEventKind::NoteOn { velocity, .. } if velocity > 0)
}

fn follow(controls: &mut HeldControls, kind: RecEventKind) {
    match kind {
        RecEventKind::Cc { number, value } if number == CC_SUSTAIN => controls.sustain = value,
        RecEventKind::Cc { number, value } if number == CC_MODULATION => {
            controls.modulation = value;
        }
        RecEventKind::PitchBend(v) => controls.bend = v,
        _ => {}
    }
}

impl Phrase {
    /// Lets go of the oldest key press and of what follows it up to the next one.
    fn drop_oldest(&mut self) {
        while let Some(ev) = self.events.pop_front() {
            if is_press(ev.kind) {
                self.notes -= 1;
            }
            follow(&mut self.head, ev.kind);
            if self.events.front().is_none_or(|e| is_press(e.kind)) {
                break;
            }
        }
    }

    fn push(&mut self, ev: Ev) {
        if is_press(ev.kind) {
            self.notes += 1;
        }
        self.events.push_back(ev);
        while self.events.len() > MAX_EVENTS {
            if self.notes > 1 {
                self.drop_oldest();
            } else {
                // One key held under a long run of controller moves: the key stays, and
                // the oldest of the moves goes.
                self.events.remove(1);
            }
        }
        while (self.events.front()).is_some_and(|e| e.t_us + MAX_AGE_US < ev.t_us) {
            self.drop_oldest();
        }
    }
}

impl Recent {
    /// Whether a phrase with a key press waits for `track`.
    pub fn has_notes(&self, track: Option<TrackId>) -> bool {
        (self.phrase.as_ref()).is_some_and(|p| p.notes > 0 && Some(p.track) == track)
    }

    pub fn forget(&mut self) {
        self.phrase = None;
    }

    /// Keys went by that were not said here (a recording took them, no stream ran): what is
    /// held is read anew with the next keys.
    pub fn lost_track(&mut self) {
        self.synced = false;
    }

    fn sync(&mut self, held: HeldNow, t_us: u64) {
        if std::mem::replace(&mut self.synced, true) {
            return;
        }
        self.down = held.keys;
        self.controls = held.controls;
        let is_held = self.down != 0 || self.controls.sustain >= PEDAL_DOWN;
        self.idle_since = (!is_held).then(|| self.idle_since.unwrap_or(t_us));
    }

    /// The keys go to another track now, or to none.
    pub fn follow_target(&mut self, track: Option<TrackId>) {
        if (self.phrase.as_ref()).is_some_and(|p| Some(p.track) != track) {
            self.phrase = None;
        }
    }

    pub fn take(&mut self, played: Played, held: HeldNow, around: Around) {
        self.follow_target(around.track);
        let (Played::Key { t_us, .. } | Played::RunEnded { t_us, .. }) = played;
        self.sync(held, t_us);
        match played {
            Played::Key {
                kind,
                t_us,
                run,
                at,
            } => self.key(kind, t_us, run, at, around),
            Played::RunEnded { run, t_us, at } => {
                if let Some(p) = (self.phrase.as_mut()).filter(|p| p.run == run && p.end.is_none())
                {
                    p.closed = true;
                    p.end = Some((t_us, at));
                }
            }
        }
    }

    fn key(
        &mut self,
        kind: RecEventKind,
        t_us: u64,
        run: u32,
        at: Option<PlayedAt>,
        around: Around,
    ) {
        let press = is_press(kind);
        let head = self.controls;
        let idle_since = self.idle_since;
        self.hold(kind, t_us);
        let ev = Ev {
            kind,
            t_us,
            at: at.unwrap_or(PlayedAt { tick: 0.0, lap: 0 }),
        };
        if let Some(p) = &mut self.phrase {
            if p.run != run {
                p.closed = true;
            }
            let rested = !p.playing
                && idle_since.is_some_and(|t| t_us.saturating_sub(t) > around.two_bars_us);
            if !p.closed && !(press && rested) {
                p.push(ev);
                if p.events.is_empty() {
                    self.phrase = None;
                }
                return;
            }
        }
        // A closed phrase stays until the next key press takes its place.
        let Some(track) = around.track.filter(|_| press) else {
            return;
        };
        self.phrase = Some(Phrase {
            run,
            playing: at.is_some(),
            track,
            loop_range: around.loop_range.filter(|_| at.is_some()),
            head,
            events: VecDeque::from([ev]),
            notes: 1,
            closed: false,
            end: None,
        });
    }

    /// Follows which keys and controllers are held.
    fn hold(&mut self, kind: RecEventKind, t_us: u64) {
        match kind {
            RecEventKind::NoteOn { key, velocity } if key <= 127 => {
                self.down &= !(1 << key);
                self.down |= u128::from(velocity > 0) << key;
            }
            RecEventKind::NoteOff { key } if key <= 127 => self.down &= !(1 << key),
            _ => follow(&mut self.controls, kind),
        }
        let held = self.down != 0 || self.controls.sustain >= PEDAL_DOWN;
        self.idle_since = match self.idle_since {
            _ if held => None,
            None => Some(t_us),
            since => since,
        };
    }

    /// The keyboard is gone: every key and the pedal are let go at `t_us`, where `heard` is.
    pub fn keys_lost(&mut self, t_us: u64, heard: Option<HeardRun>, around: Around) {
        let (run, at) = match (&self.phrase, heard) {
            (_, Some(h)) => (h.run, h.at),
            (Some(p), None) if !p.playing => (p.run, None),
            // Nowhere to put them: the phrase ends where its last event is.
            _ => {
                self.down = 0;
                self.controls = HeldControls::default();
                self.idle_since.get_or_insert(t_us);
                return;
            }
        };
        for key in 0..128u8 {
            if self.down & (1 << key) != 0 {
                self.key(RecEventKind::NoteOff { key }, t_us, run, at, around);
            }
        }
        if self.controls.sustain != 0 {
            let up = RecEventKind::Cc {
                number: CC_SUSTAIN,
                value: 0,
            };
            self.key(up, t_us, run, at, around);
        }
        self.controls = HeldControls::default();
    }
}

/// A phrase as the recorder takes it: its events at their ticks, and where it ends.
struct Placed {
    start: u64,
    loop_range: Option<TickRange>,
    events: Vec<RecEvent>,
    /// Where the keys still held are let go.
    end: (i64, u32),
}

impl Phrase {
    /// `still`: for a phrase that stands still, the tick its first key goes to and the
    /// ticks in a microsecond. `now_us` and `heard`: where an open phrase ends.
    fn place(&self, still: (u64, f64), now_us: u64, heard: Option<HeardRun>) -> Placed {
        let looped = self.loop_range.is_some();
        let first_us = self.events.front().map_or(0, |e| e.t_us);
        let (bar, per_us) = still;
        let tick_at =
            |t_us: u64| bar as i64 + (t_us.saturating_sub(first_us) as f64 * per_us).round() as i64;
        let place = |at: PlayedAt| (at.tick.round() as i64, if looped { at.lap } else { 0 });
        let events: Vec<RecEvent> = (self.events.iter())
            .map(|e| {
                let (tick, lap) = if self.playing {
                    place(e.at)
                } else {
                    (tick_at(e.t_us), 0)
                };
                RecEvent {
                    tick,
                    lap,
                    kind: e.kind,
                }
            })
            .collect();
        let last = events.last().map_or((0, 0), |e| (e.tick, e.lap));
        let end = if self.playing {
            let said = match self.end {
                Some((_, at)) => at,
                None if self.closed => None,
                None => heard.filter(|h| h.run == self.run).and_then(|h| h.at),
            };
            said.map(place)
        } else {
            let end_us = match self.end {
                Some((t_us, _)) => Some(t_us),
                None if self.closed => None,
                None => Some(now_us),
            };
            end_us.map(|t| (tick_at(t), 0))
        };
        // Never before the last event: `(lap, tick)` only goes up.
        let end = end
            .filter(|e| (e.1, e.0) >= (last.1, last.0))
            .unwrap_or(last);
        Placed {
            start: events.first().map_or(0, |e| e.tick.max(0) as u64),
            loop_range: self.loop_range,
            events,
            end,
        }
    }
}

impl Play {
    fn around(&self) -> Around {
        let line = self.play_line();
        let map = self.map();
        let bar = map.meter_at(line).meter.ticks_per_bar();
        let per_us = map.tempo_at(line as f64) * TICKS_PER_QUARTER as f64 / 60e6;
        Around {
            track: self.song.keys_target(),
            loop_range: self.loop_on(),
            two_bars_us: (2.0 * bar as f64 / per_us).round() as u64,
        }
    }

    /// The run the listener is in, with the time of each input ([`Play::set_now`]).
    pub fn set_heard_run(&mut self, run: Option<HeardRun>) {
        self.heard_run = run;
    }

    /// What the engine says was played outside a recording. `held`: what is held now, after
    /// all of it.
    pub fn played(&mut self, items: &[Played], mut held: HeldNow) {
        let around = self.around();
        // The keys held on a track with an arpeggio are not what is said as played (its
        // notes are, each with its own end): none of them counts as down.
        let doc = self.doc();
        if (around.track.and_then(|t| doc.track(t))).is_some_and(|t| t.arp.on) {
            held.keys = 0;
        }
        for &item in items {
            self.recent.take(item, held, around);
        }
    }

    /// The keyboard input stopped: what is held in the phrase is let go here.
    pub fn played_keys_lost(&mut self) {
        let around = self.around();
        self.recent.keys_lost(self.now_us, self.heard_run, around);
    }

    pub(super) fn recent_follow_target(&mut self) {
        self.recent.follow_target(self.song.keys_target());
    }

    pub(super) fn can_keep_played(&self) -> bool {
        !self.busy() && self.recent.has_notes(self.song.keys_target())
    }

    /// 「さっきのを残す」: the phrase becomes a clip on the keys target track, as a recording
    /// does, and is forgotten. Returns the notice to show.
    pub fn keep_played(&mut self, effects: &mut Vec<Effect>) -> Option<StatusMessage> {
        if self.busy() {
            return None;
        }
        let track = self.song.keys_target()?;
        self.recent_follow_target();
        let nothing = Some(StatusMessage::NothingToKeep);
        let Some(phrase) = self.recent.phrase.take().filter(|p| p.notes > 0) else {
            return nothing;
        };
        let map = self.map();
        // A phrase played while the song stood still starts at the head of the play line's
        // bar and runs at the tempo there, whatever the map does after it.
        let line = self.play_line();
        let per_us = map.tempo_at(line as f64) * TICKS_PER_QUARTER as f64 / 60e6;
        let mut placed = phrase.place((self.bar_head(line), per_us), self.now_us, self.heard_run);

        let start = self.bar_head(placed.start);
        // As the recorder reads it: a loop that ends before the phrase starts was not gone
        // round.
        let loop_range = placed.loop_range.filter(|r| r.end() > start);
        if loop_range.is_none() {
            placed.events.iter_mut().for_each(|e| e.lap = 0);
            placed.end.1 = 0;
        }
        let mut recorder = Recorder::new(start, loop_range, phrase.head);
        for &event in &placed.events {
            recorder.push(event);
        }
        let (end, lap) = placed.end;
        let pass_start = match loop_range {
            Some(r) if lap > 0 => r.start(),
            _ => start,
        };
        for key in 0..128u8 {
            recorder.push(RecEvent {
                tick: end,
                lap,
                kind: RecEventKind::NoteOff { key },
            });
        }
        // The clip ends on a bar line, past the last thing played in its last pass.
        let reach = (placed.events.iter())
            .filter(|e| e.lap == lap)
            .map(|e| e.tick + 1)
            .fold(end, i64::max)
            .max(pass_start as i64 + 1);
        let stop = map.bar_at_or_after(reach as u64) as i64;
        let Some(recorded) = recorder.finish(stop, lap) else {
            return nothing;
        };
        let added = if self.overdub {
            let mut done: Option<(Edit, RecordingAdded)> = None;
            for take in recorded.takes {
                let on = done.as_ref().map_or(self.doc(), |(e, _)| &e.doc);
                if let Ok(d) = overdub_recording(on, track, take) {
                    done = Some(d);
                }
            }
            done.ok_or(EditError::NoTakes)
        } else {
            edit::add_recording(self.doc(), track, recorded.takes, recorded.active)
        };
        let (mut e, added) = added.ok()?;
        if let StepKind::Record { track, clip, takes } = e.step {
            e.step = StepKind::KeepPlayed { track, clip, takes };
        }
        self.apply_selecting(e, None, Some(ClipPick::one(added.clip)), effects);
        None
    }
}

#[cfg(test)]
mod tests;

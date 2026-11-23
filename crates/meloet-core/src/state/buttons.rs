//! The buttons (Mackie Control) and the fader of a keyboard. They send the same commands as
//! the keys and buttons of the screen, so what is a step and what is not stays the same.

use meloet_midi::Button;
use meloet_model::mixer::Gain;
use meloet_protocol::{Command, StripRef};

use super::{Effect, Model};

/// How long the fader stays still before its level becomes one step.
pub(super) const FADER_REST_US: u64 = 500_000;

/// Half of one of the fader's 127 steps, in fader length.
const HALF_STEP: f32 = 0.5 / 127.0;

/// Fader length the song's level may be off from what the fader set, and still be it
/// (the level is kept in 0.01 dB steps).
const SAME_LEVEL: f32 = 1e-4;

/// The keyboard's fader follows the selected track's fader only after it has reached it
/// (passed or met the level the track is at), so that the level never jumps.
#[derive(Debug, Default)]
pub(super) struct Fader {
    /// The track it has been moved for. Another track selected starts over.
    track: Option<u32>,
    /// Where the fader was last, 0..=1.
    last: Option<f32>,
    caught: bool,
    /// The length of the level it set last.
    set: Option<f32>,
    pending: Option<Pending>,
}

/// A level heard but not in the song yet.
#[derive(Debug, Clone, Copy)]
struct Pending {
    track: u32,
    db: Option<f32>,
    at_us: u64,
}

impl Fader {
    /// Whether the fader at `pos` moves `track`, whose level in the song is at `level` (both as
    /// fader lengths, 0..=1).
    fn follows(&mut self, track: u32, level: f32, pos: f32) -> bool {
        if self.track != Some(track) {
            *self = Fader {
                track: Some(track),
                ..Fader::default()
            };
        }
        // The level was changed some other way (the screen, undo): reach it again.
        if self.caught
            && self.pending.is_none()
            && self.set.is_some_and(|set| (set - level).abs() > SAME_LEVEL)
        {
            self.caught = false;
        }
        if !self.caught {
            let crossed = self
                .last
                .is_some_and(|last| (last - level) * (pos - level) <= 0.0);
            self.caught = crossed || (pos - level).abs() <= HALF_STEP;
        }
        self.last = Some(pos);
        self.caught
    }
}

impl Model {
    /// The command a button stands for now, if any.
    pub(super) fn button_command(&self, button: Button) -> Option<Command> {
        let stopped = self.play.is_stopped();
        match button {
            Button::Play => stopped.then_some(Command::PlayStop),
            Button::Stop => (!stopped).then_some(Command::PlayStop),
            Button::Record => Some(Command::Record),
            Button::Rewind | Button::Left => Some(Command::Locate {
                tick: self.play.bar_step(false),
            }),
            Button::FastForward | Button::Right => Some(Command::Locate {
                tick: self.play.bar_step(true),
            }),
            Button::Cycle | Button::Middle => Some(Command::ToggleLoop),
            Button::Up | Button::Down => {
                let track = self.play.track_beside(button == Button::Down)?;
                Some(Command::SelectTrack {
                    track,
                    from_arm: false,
                })
            }
        }
    }

    /// The fader of a keyboard at `value` (0..=127): the selected track's level as the
    /// mixer's fader at that length, heard at once and one step when the fader rests.
    pub(super) fn keyboard_fader(&mut self, value: u8, effects: &mut Vec<Effect>) {
        let Some((track, gain)) = self.play.selected_gain() else {
            return;
        };
        if self.fader.pending.is_some_and(|p| p.track != track) {
            self.commit_fader(effects);
        }
        let pos = f32::from(value.min(127)) / 127.0;
        if !self.fader.follows(track, gain.position(), pos) {
            return;
        }
        let level = Gain::from_position(pos);
        let db = level.db().map(|db| db as f32);
        self.fader.set = Some(level.position());
        self.fader.pending = Some(Pending {
            track,
            db,
            at_us: self.now_us.saturating_add(FADER_REST_US),
        });
        let strip = StripRef::Track { id: track };
        self.command(
            Command::SetGain {
                strip,
                db,
                commit: false,
            },
            effects,
        );
    }

    /// Makes the level the fader set one step once the fader has rested.
    pub(super) fn fader_due(&mut self, effects: &mut Vec<Effect>) {
        if self.fader.pending.is_some_and(|p| p.at_us <= self.now_us) {
            self.commit_fader(effects);
        }
    }

    fn commit_fader(&mut self, effects: &mut Vec<Effect>) {
        if let Some(p) = self.fader.pending.take() {
            let strip = StripRef::Track { id: p.track };
            self.command(
                Command::SetGain {
                    strip,
                    db: p.db,
                    commit: true,
                },
                effects,
            );
        }
    }

    pub(super) fn fader_wake_us(&self) -> Option<u64> {
        self.fader.pending.map(|p| p.at_us)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fader_waits_until_it_passes_the_level() {
        let mut f = Fader::default();
        let level = 0.8;
        assert!(!f.follows(1, level, 0.2));
        assert!(!f.follows(1, level, 0.5));
        assert!(f.follows(1, level, 0.85), "passed 0.8 on the way up");
        assert!(f.follows(1, level, 0.1), "then it follows");

        let mut f = Fader::default();
        assert!(!f.follows(1, 0.3, 0.9));
        assert!(f.follows(1, 0.3, 0.2), "passed on the way down");

        let mut f = Fader::default();
        assert!(f.follows(1, 0.5, 64.0 / 127.0), "within half a step of it");
    }

    #[test]
    fn another_track_starts_the_wait_over() {
        let mut f = Fader::default();
        assert!(f.follows(1, 0.5, 0.5));
        assert!(!f.follows(2, 0.9, 0.6));
        assert!(!f.follows(2, 0.9, 0.7));
        assert!(f.follows(2, 0.9, 1.0));
    }

    #[test]
    fn a_level_changed_elsewhere_must_be_reached_again() {
        let mut f = Fader::default();
        assert!(f.follows(1, 0.5, 0.5));
        f.set = Some(0.5);
        assert!(!f.follows(1, 0.2, 0.6), "the screen set 0.2");
        assert!(f.follows(1, 0.2, 0.1));
    }
}

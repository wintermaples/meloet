//! R2 「キーで弾く」: the keys of the PC keyboard (`Command::TypedKey`), played on the core
//! thread as the MIDI thread plays a keyboard's (`play_key`): into the engine through a ring
//! of their own ([`meloet_engine::EngineHandle::typed_in`], read with the MIDI ring in the
//! order the keys were played), into the held keys of the meters, and to the screen at once.
//! They do not go through the rules: no view is made for a key, an AI-link batch does not hold
//! it, and the notice on the status line stays.

use std::sync::atomic::Ordering::Relaxed;

use meloet_midi::MidiEvent;
use meloet_protocol::{TYPED_KEY_VELOCITY, TYPED_KEYBOARD_ID};

use super::{Input, Worker, fixed, monotonic_us, play_key};

/// The oldest a key's time may be when the core thread takes it. Older times are taken as now:
/// the engine's clock keeps the last 512 blocks (0.34 s at 32 frames), and a recorded key whose
/// time it no longer keeps would be lost.
pub(super) const MAX_AGE_US: u64 = 250_000;

/// What the PC keyboard holds down, and the time of its last event.
#[derive(Default)]
pub(super) struct TypedHands {
    held: u128,
    last_us: u64,
    /// The rules had 「キーで弾く」 on after the last input.
    on: bool,
}

impl TypedHands {
    /// The event a key makes at `fixed` velocity (0: not fixed), or `None` when it changes
    /// nothing: a key held already, a key let go that is not held, a note above 127.
    pub(super) fn key(&mut self, note: u8, down: bool, fixed_velocity: u8) -> Option<MidiEvent> {
        let bit = 1u128.checked_shl(u32::from(note))?;
        if down == (self.held & bit != 0) {
            return None;
        }
        self.held ^= bit;
        Some(if down {
            fixed(
                MidiEvent::NoteOn {
                    note,
                    velocity: TYPED_KEY_VELOCITY,
                },
                fixed_velocity,
            )
        } else {
            MidiEvent::NoteOff { note, velocity: 0 }
        })
    }

    /// The time a key is played at: the screen's, but never later than now, never older than
    /// [`MAX_AGE_US`] (then now), and never before the last one, so the ring stays in order.
    pub(super) fn time(&mut self, t_us: u64, now_us: u64) -> u64 {
        let t = if t_us.saturating_add(MAX_AGE_US) < now_us {
            now_us
        } else {
            t_us.min(now_us)
        };
        self.last_us = t.max(self.last_us);
        self.last_us
    }

    /// A NoteOff for every key held, lowest first.
    pub(super) fn let_go(&mut self) -> Vec<MidiEvent> {
        let held = std::mem::take(&mut self.held);
        (0..128u8)
            .filter(|&n| held & (1 << n) != 0)
            .map(|note| MidiEvent::NoteOff { note, velocity: 0 })
            .collect()
    }

    /// Everything was let go elsewhere (`Effect::AllNotesOff`).
    pub(super) fn forget(&mut self) {
        self.held = 0;
    }
}

impl Worker {
    pub(super) fn typed_key(&mut self, note: u8, down: bool, t_us: u64) {
        if !self.model.typed_keys() {
            return;
        }
        let fixed_velocity = self.shared.fixed_velocity.load(Relaxed);
        let Some(event) = self.typed.key(note, down, fixed_velocity) else {
            return;
        };
        let t_us = self.typed.time(t_us, monotonic_us());
        self.play_typed(event, t_us);
        if down {
            // The MIDI thread says its keyboard again at its next key.
            self.shared.typed_played.store(true, Relaxed);
            if !self.model.typed_is_active() {
                self.handle(Input::KeyboardPlayed {
                    id: TYPED_KEYBOARD_ID.into(),
                });
            }
        }
    }

    /// After every input: when the rules turned 「キーで弾く」 off, every key it holds is let go.
    pub(super) fn follow_typed(&mut self) {
        let on = self.model.typed_keys();
        if std::mem::replace(&mut self.typed.on, on) && !on {
            let t_us = self.typed.time(monotonic_us(), monotonic_us());
            for event in self.typed.let_go() {
                self.play_typed(event, t_us);
            }
            // The keyboard shown is a MIDI one again: the MIDI thread must say which it plays.
            self.shared.typed_played.store(true, Relaxed);
        }
    }

    fn play_typed(&mut self, event: MidiEvent, t_us: u64) {
        let ring = self.audio.handle.typed_in();
        play_key(event, t_us, &self.shared, ring, &self.hub, &self.links.tx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on(note: u8, velocity: u8) -> Option<MidiEvent> {
        Some(MidiEvent::NoteOn { note, velocity })
    }

    fn off(note: u8) -> Option<MidiEvent> {
        Some(MidiEvent::NoteOff { note, velocity: 0 })
    }

    #[test]
    fn a_key_sounds_once_until_it_is_let_go_at_the_typed_or_the_fixed_velocity() {
        let mut hands = TypedHands::default();
        assert_eq!(hands.key(60, true, 0), on(60, TYPED_KEY_VELOCITY));
        assert_eq!(hands.key(60, true, 0), None, "held already");
        assert_eq!(hands.key(61, false, 0), None, "not held");
        assert_eq!(hands.key(127, true, 30), on(127, 30), "the fixed velocity");
        assert_eq!(hands.key(128, true, 0), None);
        assert_eq!(hands.key(255, false, 0), None);
        assert_eq!(hands.key(60, false, 0), off(60));
        assert_eq!(hands.key(0, true, 0), on(0, TYPED_KEY_VELOCITY));
        assert_eq!(
            hands.let_go(),
            [off(0).unwrap(), off(127).unwrap()],
            "lowest first"
        );
        assert_eq!(hands.let_go(), []);
        assert_eq!(hands.key(0, false, 0), None, "let go already");
        assert_eq!(hands.key(5, true, 0), on(5, TYPED_KEY_VELOCITY));
        hands.forget();
        assert_eq!(hands.let_go(), []);
    }

    #[test]
    fn the_time_of_a_key_is_never_later_than_now_too_old_or_before_the_last() {
        let mut hands = TypedHands::default();
        let now = 10_000_000;
        assert_eq!(
            hands.time(now - 3_000, now),
            now - 3_000,
            "the screen's time"
        );
        assert_eq!(
            hands.time(now + 5_000, now + 1_000),
            now + 1_000,
            "not later than now"
        );
        assert_eq!(
            hands.time(now - 1_000, now + 2_000),
            now + 1_000,
            "not before the last"
        );
        let later = now + 1_000_000;
        assert_eq!(hands.time(0, later), later, "no time: now");
        assert_eq!(
            hands.time(later - MAX_AGE_US - 1, later),
            later,
            "too old: now"
        );
        assert_eq!(
            hands.time(later + 10 - MAX_AGE_US, later + 10),
            later,
            "as old as may be, but not before the last"
        );
    }
}

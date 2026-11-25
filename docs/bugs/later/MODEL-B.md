# MODEL-B — worth a look later (not bugs)

- crates/meloet-model/src/automation.rs:836 — `add_fade_out` wakes a master lane that rests; its old points before the fade's start then move the master volume where the hand's fader was heard until now. Written as meant in docs/architecture.md 25-32, but the sound before the fade changes; worth asking whether waking should also drop or flatten those points.
- crates/meloet-model/src/audio/follow.rs:65 — `until` writes a ramp between two equal tempos differently from a steady stretch with the same seconds, so `is_the_songs` says no after a ramp like that is turned off, and the clip is made again through the stretcher at a ratio of 1.

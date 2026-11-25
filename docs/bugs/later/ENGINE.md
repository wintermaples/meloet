# ENGINE — worth a look later (not bugs)

- crates/meloet-engine/src/automation.rs:300-302, 373-375 — one `turn` serves the instrument's lanes and every plugin slot's lanes of a track, and moves by `PLUGIN_POINTS` places in the list of all lanes rather than past the lanes it served; turn-taking over more than 128 lanes is only rough.
- crates/meloet-engine/src/clipplay.rs:126-133 — `set_reader` while a track's clips sound sets the gate to 0 at once (no 10 ms fade); only matters if a ring is replaced while playing.
- crates/meloet-engine/src/clock.rs:224-231 — `segment_at` reads up to 512 slots on the audio thread for each key older than every stretch (right after a stream opens); with a full MIDI ring that is many atomic reads in one block.
- crates/meloet-engine/src/engine.rs:2849-2855 — for a track that reads ahead, a mute-lane change past the loop's end is not scheduled (`frames_to` gives `None` there), so it lands at the next cut, up to one block late.
- crates/meloet-engine/src/synth.rs:359-373 — a stolen voice that starts its waiting note inside a block makes no sound for that frame and keeps its vibrato ratio at 1.0 until the block ends.

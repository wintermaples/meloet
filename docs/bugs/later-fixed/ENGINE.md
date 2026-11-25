# ENGINE — fixed in the second pass (2026-11-25)

- crates/meloet-engine/src/engine.rs:2260-2267 — `SetLoop` seeks every lane whose `ahead_wrapped` is set, also while stopped, and `stop()` (2397-2431) does not clear the flag: a stop in the last L frames before the loop's end (a track with latency L) followed by a loop change while stopped chases the song's pedal, wheel and bend onto that instrument while it stands still (keys played then sustain until play starts). Narrow window.
- crates/meloet-engine/src/engine.rs:2144-2150 — `AllNotesOff` (sent when a keyboard is pulled out) releases the song's sounding notes on every track while the song plays, not only what the keyboards held on the play target; a design question.
- crates/meloet-engine/src/engine.rs:1776-1783 — a callback with 0 frames stores a block length of 0, so the next callback always counts as late (`late_callbacks`).

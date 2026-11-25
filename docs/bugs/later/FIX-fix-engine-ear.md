# Later: FIX-fix-engine-ear

- crates/meloet-engine/src/engine.rs:2833-2835 (`LapChanged::t_us`) — the time the lap's first frame reaches the DAC, not when its song is heard (the master's insert latency is not added). Nothing in core reads it now.
- crates/meloet-engine/src/engine.rs:2662-2666 (`LatencyRecord`) — the key → sound numbers the settings show do not include the master's insert latency.
- crates/meloet-engine/src/engine.rs:2421-2455 (`stop`) — after a stop, the play line runs on to where the last block reached the DAC (now plus the master's insert latency) and then goes back to the stop position: it went back before ENGINE-001 too, by the output delay.
- crates/meloet-engine/src/engine.rs:1816 (with 2180) — the master's insert latency is read after the commands, so inside `read_commands` of the block where it changed the old value is used: a beat clicked at a tempo swap in that block waits the old delay (with an old 0 it sounds at once and is not moved), and `run_place`'s fallback `segment_now` counts back with the old delay. One block only.

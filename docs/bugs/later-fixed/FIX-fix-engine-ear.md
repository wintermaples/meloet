# FIX-fix-engine-ear — fixed in the second pass (2026-11-25)

- crates/meloet-engine/src/engine.rs:2476-2480 (`record_here`) — R pressed while playing, within the output delay (plus the master's insert latency since ENGINE-001) after the play started: the clock has no playing stretch there yet, so `from` falls back to `t.tick()`, which is ahead of what was heard by up to that much. `run_place` counts back from `segment_now(0)` in the same case.
- crates/meloet-engine/src/clock.rs:312 (`heard_time_of_frame`) — a frame older than every stretch kept gets no master latency, while `time_of_frame` counts from the oldest stretch there.

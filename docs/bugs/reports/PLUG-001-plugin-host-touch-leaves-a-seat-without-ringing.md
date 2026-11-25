---
id: PLUG-001
title: Plugin host: touch() leaves a seat without ringing the bell, so a block asked meanwhile can make meloet stop the whole host
area: PLUG
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# PLUG-001: Plugin host: touch() leaves a seat without ringing the bell, so a block asked meanwhile can make meloet stop the whole host

## Where

crates/meloet-plughost/src/rt.rs:394-399 (`Handle::touch`: takes the seat, no `self.bell.ring()` after)
crates/meloet-plughost/src/rt.rs:561-575 (`run`: a seat another thread holds is passed over, then the thread sleeps on the bell for up to `IDLE_NS`, line 34)
crates/meloet-plughost/src/control.rs:241 (`take_hold` calls `audio.touch(slot, id, true)`)
crates/meloet-plughost/src/control.rs:251 (`let_go` calls `audio.touch(slot, id, false)`)
crates/meloet-plugin/src/ipc/ends.rs:228-243 (`await_overdue`: nobody took the request and `progress` did not move, so the slot and the whole mapping are stopped)

## What happens

When the plugin host's main thread is inside `Handle::touch` (it holds the slot's seat for a moment) at the moment the engine asks that slot for a block, the audio thread that wakes for the block finds the seat busy, passes it over, finds nothing else to do and goes back to sleep on the bell. `touch` then leaves the seat without ringing the bell, so nothing wakes the audio thread again until its 100 ms time limit (`IDLE_NS`) or until some other slot is asked.

In a host with one plugin (the usual case: one instance of a plugin per process) nothing else rings. The engine's deadline passes (silent or dry block), and at the next block's deadline `await_overdue` sees: request not taken, `progress` unchanged, no thread inside a plugin. It marks the slot and the shared header as stopped. meloet then kills the process and shows the plugin as stopped (「（名前）が止まりました…」); every plugin of that host is silent until 「もう一度読みこむ」.

It is timing-dependent and rare per event, but it can happen any time the person takes hold of or lets go of a parameter in a plugin's window while the engine runs blocks.

## What should happen

The main thread rings the bell every time it leaves a seat, so a block asked while it was there is not passed over. This is the rule the audio thread relies on: the comment in `run` (rt.rs:562-563) says "the main thread rings the bell when it leaves one", and every other seat user does it: `install` (rt.rs:347), `remove` (354), `set_param` (366), `params_taken` (375), `outputs` (415), `forget_params` (425). A busy or late host must not be taken for a dead one (docs/architecture.md 20-2, 23-14).

## How to trigger

Read from the code, not run.
1. Load one VST3 or LV2 plugin with a window (one plugin in its host process) and let the engine run blocks (playing, or the output just open).
2. Open its window and grab a knob without moving it (VST3 `beginEdit` / LV2 `ui:touch` grabbed: `Said::ParamBegin` alone, no `set_param` that would ring), or release a knob after the last move (`Said::ParamEnd`), or let an LV2 parameter that has no touch go quiet for 0.5 s (`LET_GO`).
3. If the engine submits that slot's block while `touch` holds the seat, and the woken audio thread looks at the seat in that moment, the host is stopped two blocks later.

## Cause

`Handle::touch` is the only main-thread method that takes a seat and does not call `self.bell.ring()` afterwards. The audio thread reads the bell count before it walks the seats (`let seen = bell.count()`), so a ring made by the engine before that walk does not wake it again; only a later ring does. With no ring, the thread sleeps for `IDLE_NS` (100 ms), longer than two engine blocks, and the engine's watch in `await_overdue` cannot tell this from a frozen process.

## Suggested fix

Ring the bell at the end of `Handle::touch`, as the sibling methods do (after the `Taken` guard is dropped):

```rust
pub fn touch(&mut self, slot: u32, id: u32, on: bool) -> Result<(), String> {
    if let Some(run) = self.seat(slot)?.run() {
        run.touch(id, on);
    }
    // A block asked for while the main thread was at the seat was passed over.
    self.bell.ring();
    Ok(())
}
```

No protocol or file-shape change.

## Verification

Confirmed by reading (no test run; it is a race between two threads of the host and the engine, and I found no small test that forces the order). `Handle::touch` (`rt.rs:394-399`) is the only main-thread method that takes a seat and does not ring the bell after; `install`, `remove`, `set_param`, `params_taken`, `outputs`, `forget_params` all do, with the same comment. The path to a stopped host holds: the engine's `submit` rings once (`ends.rs:292-296`); the audio thread wakes, reads `seen`, finds the seat busy and sleeps up to `IDLE_NS` (100 ms); the engine does not ring again while the block is `pending` (`submit` returns at once); the first `wait` stores `seen_progress`; at the next block `await_overdue` sees `taken != seq`, `progress` unchanged (only `take_request` and `finish` raise it) and no thread inside a plugin, and marks the mapping stopped.

Correction: the window is narrower than the report's steps suggest. The main thread holds the seat in `touch` only for a push into a small `Vec`, so the audio thread must look at the seat in those few instructions. It is rare per touch; but a host with one plugin has no other ring to save it, and the result is a stopped plugin, so severity `medium` stands (I would not argue with `low` by frequency).

Fix plan: `crates/meloet-plughost/src/rt.rs` `Handle::touch`: `self.bell.ring();` after the seat guard is dropped, with the same comment as the siblings. No test can watch the race itself; a unit test that `touch` raises the bell count is enough.

## Fix

- `crates/meloet-plughost/src/rt.rs` `Handle::touch`: rings the bell after it leaves the seat, as every other seat user does.
- Test: `rt::tests::taking_hold_of_a_parameter_rings_the_bell_after` (meloet-plughost): the bell is read as an audio thread reads it before it looks at the seats; after `touch` (take hold, let go) it must have moved, so a thread that found the seat taken does not sleep until `IDLE_NS`. The race itself has no test: the main thread's moment inside `touch` cannot be held from a test without new machinery.

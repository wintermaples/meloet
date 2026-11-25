---
id: EDGE-001
title: The shell's own zoom subscription counts as the screen's greeting, so the plugin scan and the held song start before the page loads
area: EDGE
kind: bug
severity: medium
status: confirmed
found: 2026-11-25
---

# EDGE-001: The shell's own zoom subscription counts as the screen's greeting, so the plugin scan and the held song start before the page loads

## Where

- crates/meloet-app/src/main.rs:251-276 (`follow_scale` calls `core.subscribe(...)`)
- crates/meloet-app/src/main.rs:557-560 (`follow_scale` runs in the Tauri `setup` hook)
- crates/meloet-core/src/service.rs:888-891 (`Core::subscribe` always sends `Msg::ScreenReady`)
- crates/meloet-core/src/service.rs:2844-2849 (`ScreenReady` releases the held scan jobs and gives `Input::ScreenUp`)

## What happens

At start, the shell subscribes to the core for itself (to follow the zoom for the smallest window size). `Core::subscribe` treats every subscription as "the screen's page is up": it sends `Msg::ScreenReady`. So `ScreenReady` reaches the core thread in Tauri's `setup` hook, before the WebView has even started to load the page, and long before the page's own greeting (`hello` -> `subscribe`).

What waits for the screen therefore does not wait at all:
- the plugin scan jobs held by `PluginSide::held` are given to the scanner at once (hundreds of child processes start while WebKit loads its libraries);
- `Input::ScreenUp` runs `songs_screen_up` (a song handed over by the PC, `meloet song.meloet`, is opened at once), `pack_screen_up` (the catalog check and a resumed fetch) and `updates_screen_up`.
The 10 s fallback (`SCAN_WAITS_FOR_SCREEN`) never matters, because the release always comes at once.

## What should happen

The scan, the given song and the other "screen up" work start after the page's greeting. docs/architecture.md:850: "The scan starts after the screen's hello (`subscribe`), or after 10 s ... Why: under VS Code's CodeLLDB, concurrent spawns left meloet stopped with no window, and spawns overlapping WebKit's library loading showed as pauses; the window also comes up sooner." CLAUDE.md ("Things about this PC"): "Fixed by starting children one at a time and starting the scan after the screen's greeting (2026-11-19). If either is removed, it happens again." docs/architecture.md:2614: "A path that comes before the screen's greeting is kept until `Input::ScreenUp`." Commit aa16851 says the same; `follow_scale` (commit e9cb85d, 2026-11-15) was older and was not changed with it.

## How to trigger

Read from the code, not run. Start the real app (any start; with plugins on the PC and no saved scan, or `meloet some.meloet`). In Tauri 2.12.5 `setup` runs on `RuntimeRunEvent::Ready` right after the config windows are built (`tauri-2.12.5/src/app.rs:1441-1445, 2688-2701`), before the main loop has loaded any page. `setup` -> `follow_scale` -> `core.subscribe` -> `Msg::ScreenReady` -> `plugins.release()` and `Input::ScreenUp`. A log line or a breakpoint in `PluginSide::release` would show it fires before the page's `hello` command.

## Cause

`Core::subscribe` (service.rs:888) does two things: it adds the sinks and it announces the screen. The shell's internal subscription in `follow_scale` only wants the patches, but it gets the announcement too. It is the first `subscribe` of the run, so the `ScreenReady` it sends is the one that counts (`release` and the `screen_up` flags act once).

## Suggested fix

Smallest change: let the shell subscribe without announcing the screen, for example a separate core method (`Core::listen(messages)` that only calls `hub.subscribe`, or `subscribe` with a flag) used by `follow_scale`, while the `subscribe` command of the page keeps sending `ScreenReady`. Alternatively send `ScreenReady` from the `hello` command instead of from `subscribe`. No protocol, song-file or wording change. Needs a core change (crates/meloet-core/src/service.rs), which is outside this area.

## Verification

Confirmed by reading (the real app was not started). Tauri 2.12.5 (`~/.cargo/registry/src/*/tauri-2.12.5/src/app.rs`, the version in `Cargo.lock`): the builder's `setup` closure is called from `fn setup` (`app.rs:2688-2701`), which first builds the windows of the config and then calls the closure, on `RuntimeRunEvent::Ready` (`app.rs:1442-1443`), on the main thread, before the event loop has run any page load. In meloet's closure (`main.rs:557-560`) `follow_scale` calls `core.subscribe(..)` (`main.rs:254`), and `Core::subscribe` (`service.rs:888-891`) always sends `Msg::ScreenReady`; the core thread then calls `plugins.release()` and `handle(Input::ScreenUp)` (`service.rs:2844-2849`). The page's own `subscribe` command (`main.rs:105`) can only come after the page loaded. So the shell's subscription is always the first, and the scan, the song handed over by the PC, the pack check and the update check never wait for the page; `SCAN_WAITS_FOR_SCREEN` never matters. Architecture 20-10 and the CodeLLDB note in CLAUDE.md say the scan starts after the screen's greeting. Children are still started one at a time (`launch::spawn`), so I do not claim the CodeLLDB hang is back; the pauses while WebKit loads and the later window are what 20-10 names. Severity `medium` stands.

Corrections: none.

Fix plan: `crates/meloet-core/src/service.rs`: a second method (for example `Core::listen(messages)`) that only calls `self.hub.subscribe(messages, <a telemetry sink that says false>)` and sends nothing; `crates/meloet-app/src/main.rs` `follow_scale` uses it. The page's `subscribe` command keeps `Core::subscribe`. Test in `service/tests.rs` or `plug_tests.rs`: with `wait_for_screen`, `listen` does not release the held scan and `subscribe` does. After the fix, check by hand once that a start under VS Code's 「meloet（デバッグ）」 still shows the window.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->

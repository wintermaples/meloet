# UI-BRIDGE — fixed in the second pass (2026-11-25)

- ui/src/gl/Surface.svelte:72-76 — the `$effect` reads `current.state`, so `engine.setScale` runs after every patch of any section; `setScale` calls `touch(null)`, which marks every region dirty, so each patch re-measures every slot box (`getBoundingClientRect`) and rebuilds every picture even when the scale did not change. Only a cost; the picture comparison stops the redraw.
- ui/src/bridge/fakeAudio.ts:137 — `fakeAudioClip` gives `takes: []`, while Rust always lists the shown take in `AudioClipView.takes` (protocol doc "the shown one too"). No screen code depends on it today; tests on the fake would miss a part that looks the shown take up in `takes`.
- ui/src/lib/HScrollbar.svelte:3-5 — the header comment says the handle is gray.8 on hover and gray.9 while moving; the CSS (and 7-readings "Fill or ink, per place") uses accent and accent.ink. Stale comment.

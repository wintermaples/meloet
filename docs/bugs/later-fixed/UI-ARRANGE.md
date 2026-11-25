# UI-ARRANGE — fixed in the second pass (2026-11-25)

- ui/src/components/automation/AutoLane.svelte:226-233 — the `$effect` of the rec border calls `placeRec()` → `playTick()`, which reads `current.telemetry`; while a lane is written the effect runs again on every telemetry message (about 60 a second), removes and adds its `everyGlFrame` each time and resets `drawnWidth`, so the width is written again even when it did not change.
- ui/src/components/automation/WriteLine.svelte:35-52 — when the loop turns while writing, `sample()` takes no more values (`tick > last.tick` is false), so the line inside the border stops changing for the rest of the pass (screen only; core places the values itself).
- ui/src/components/arrange/wavePeaks.svelte.ts:145-147 — `trim()` drops other songs' entries but keeps `failed` / `tries`; audio ids start again per song, so a new song's first peaks can wait on the old song's back-off (up to 8 s).
- ui/src/components/Arrange.svelte:1031-1041 — the vocal recording label's full width (`labelFullPx`) is measured once per tail; when a later lap's take number gains a digit, the label is cut one character short (the keys label measures again on a new take, line 928).
- ui/src/components/Arrange.svelte:405-415, 449-465 — the moving picture of a clip move is not made again for the target row: clips moved onto the 160 row keep the old height, and in a group over several rows every row is shifted by the grabbed clip's px, so with lanes between tracks the other clips are drawn off their rows while dragging.
- ui/src/components/TrackHeads.svelte:141-148 — a wheel turn during a head reorder drag scrolls the rows under the lifted head (`wheelRows` blocks only clip drags and box selects), so the drop line no longer matches the pointer.

## not true when looked at again (2026-11-25)

- ui/src/components/pianoroll/Roll.svelte:894-901 — the piano roll pages with the play line during note drags too (the same trap as UI-ARRANGE-004, outside this area).

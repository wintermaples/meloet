# UI-ROLL — worth a look later (not bugs)

- ui/src/components/pianoroll/Roll.svelte:595-596 — every value step of a velocity drag changes `rollDragKey`, so the whole pianoRoll region (the note area too) is drawn again, though only the velocity lane changes.
- ui/src/components/pianoroll/CcLanes.svelte:275-281 — `placeLive` reads `getBoundingClientRect` twice on every changed pointer move of a CC drag (a forced layout per move); `Roll.svelte:334-337` `at(e)` does the same on every hover move of the note area.
- ui/src/components/pianoroll/Roll.svelte:894-910 — during the count-in `frame()` runs and the key column lights the notes under the recording start (`shownTick` holds `recording.from`), though nothing sounds; spec E1 and readings say notes are not lit in the count-in, but the key column "lights as before" (unclear which wins).
- ui/src/components/pianoroll/paint.ts:165-229 — drum hit squares have no hover look; spec 2 lists normal / selected / hover for "Drum hit square".
- ui/src/components/pianoroll/ChordRow.svelte:66 — ▶ of a progression plays at the song's head tempo (`timeline.tempoBpm`); with tempo changes it is not the tempo where the chords would be placed (the shelf patterns use that one, readings P4).

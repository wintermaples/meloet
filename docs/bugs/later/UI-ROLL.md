# UI-ROLL — worth a look later (not bugs)

- ui/src/components/pianoroll/sounding.ts:137 — `soundingNotes` walks the take from index 0 on every frame while playing (about 10 000 steps a frame near the end of a 10 000-note take); chapter 7 asks per-frame work to follow what changes, not what has piled up.
- ui/src/components/pianoroll/Roll.svelte:595-596 — every value step of a velocity drag changes `rollDragKey`, so the whole pianoRoll region (the note area too) is drawn again, though only the velocity lane changes.
- ui/src/components/pianoroll/CcLanes.svelte:275-281 — `placeLive` reads `getBoundingClientRect` twice on every changed pointer move of a CC drag (a forced layout per move); `Roll.svelte:334-337` `at(e)` does the same on every hover move of the note area.
- ui/src/components/pianoroll/Roll.svelte:894-910 — during the count-in `frame()` runs and the key column lights the notes under the recording start (`shownTick` holds `recording.from`), though nothing sounds; spec E1 and readings say notes are not lit in the count-in, but the key column "lights as before" (unclear which wins).
- ui/src/components/pianoroll/shown.svelte.ts:299-306 — a rejected `takeNotes` is not caught; the previous take (maybe of another clip) stays shown until the next state change.
- ui/src/components/pianoroll/paint.ts:353-369 — on drum rows the velocity lane draws bars (and `drag.ts:258-283` lets you drag them) for notes on keys the kit does not have, which the note area does not show (spec 3: such notes "do not show in the piano roll").
- ui/src/components/pianoroll/paint.ts:165-229 — drum hit squares have no hover look; spec 2 lists normal / selected / hover for "Drum hit square".
- ui/src/components/pianoroll/NoteInspector.svelte:135-140 — `bulkDrafts` are set again whenever `bulkSpecs` is made again (a new pianoRoll section or take), so text being typed in the change-all rows is lost if a diff arrives meanwhile (an AI link edit, a selection from elsewhere).
- ui/src/components/pianoroll/Roll.svelte:978-979 — `labelW` / `markerW` keep one entry per beat label and marker ever shown; they grow while scrolling and are never trimmed.
- ui/src/components/pianoroll/ChordRow.svelte:66 — ▶ of a progression plays at the song's head tempo (`timeline.tempoBpm`); with tempo changes it is not the tempo where the chords would be placed (the shelf patterns use that one, readings P4).

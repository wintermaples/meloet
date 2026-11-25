# UI-ROLL — fixed in the second pass (2026-11-25)

- ui/src/components/pianoroll/sounding.ts:137 — `soundingNotes` walks the take from index 0 on every frame while playing (about 10 000 steps a frame near the end of a 10 000-note take); chapter 7 asks per-frame work to follow what changes, not what has piled up.
- ui/src/components/pianoroll/shown.svelte.ts:299-306 — a rejected `takeNotes` is not caught; the previous take (maybe of another clip) stays shown until the next state change.
- ui/src/components/pianoroll/paint.ts:353-369 — on drum rows the velocity lane draws bars (and `drag.ts:258-283` lets you drag them) for notes on keys the kit does not have, which the note area does not show (spec 3: such notes "do not show in the piano roll").
- ui/src/components/pianoroll/NoteInspector.svelte:135-140 — `bulkDrafts` are set again whenever `bulkSpecs` is made again (a new pianoRoll section or take), so text being typed in the change-all rows is lost if a diff arrives meanwhile (an AI link edit, a selection from elsewhere).
- ui/src/components/pianoroll/Roll.svelte:978-979 — `labelW` / `markerW` keep one entry per beat label and marker ever shown; they grow while scrolling and are never trimmed.

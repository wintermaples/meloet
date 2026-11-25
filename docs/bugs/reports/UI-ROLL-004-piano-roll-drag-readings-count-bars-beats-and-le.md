---
id: UI-ROLL-004
title: Piano roll drag readings count bars, beats and lengths in the song's first time signature
area: UI-ROLL
kind: bug
severity: low
status: reported
found: 2026-11-25
---

# UI-ROLL-004: Piano roll drag readings count bars, beats and lengths in the song's first time signature

## Where

- `ui/src/components/pianoroll/Roll.svelte:75` — `meter` is `{ num: timeline.meterNum, den: timeline.meterDen }`, the time signature at the song's start only.
- `ui/src/components/pianoroll/Roll.svelte:285` and `:1258` — that `meter` is what `RollDrag` and `CcLanes` (`LaneMouse`) read for their texts.
- `ui/src/components/pianoroll/drag.ts:404` — draw: `positionText(d.start, meter)`, `lengthText(d.length, meter)`.
- `ui/src/components/pianoroll/drag.ts:408-409` — move: `signedLength(d.ticks, meter)`.
- `ui/src/components/pianoroll/drag.ts:415` — resize: `lengthText(…, meter)`.
- `ui/src/components/pianoroll/cc.ts:380` and `:383` — CC points: `positionText(…, meter)`.

## What happens

In a song with a time signature change, the 「その場の読み値」 and the status line show the wrong bar and beat while a note is drawn or a CC point is added or moved after the change, and lengths and moves are counted in beats of the first time signature. Example: 4/4 at the start, 3/4 from bar 5 (bar 5 starts at tick 30720). Drawing at bar 7 beat 1 (tick 42240) reads 「6.3.0000」; after the release the note list and 「選んだノート」 show 「7.1.0000」 for the same note. In a 6/8 part of a 4/4 song, a note one eighth long reads 「長さ 0.0960」 while the note list says 「1.0000」.

## What should happen

Spec 3, "Rules for round 3": "The value being moved (position, pitch, length, velocity, CC) is shown both in the 「その場の読み値」 near the cursor and on the status line." Everywhere else positions follow the tempo map and lengths use the time signature at that place: the note list and 「選んだノート」 (`NoteListPanel.svelte:36`, `noteFields.ts:217`, `notes.ts:88`), the arrange drag reading (`arrangeDrag.ts:290`), the marker and tempo readings (`MarkerRow.svelte:302`, `TempoBand.svelte:78`).

## How to trigger

Read from the code, not run: in a 4/4 song add a 3/4 change at bar 5; open a clip that reaches bar 7 in the piano roll; choose 「書く」 and press at bar 7 beat 1, holding the button; read the reading.

## Cause

`positionText(tick, bars)` with a plain time signature (`barsMap(meter)` → one meter for the whole song) counts bars of that meter from tick 0, and `lengthText` / `signedLength` take the beat of that meter. The piano roll passes the head's meter, not the tempo map it already has (`map`, `Roll.svelte:77`).

## Suggested fix

Give `RollDrag` and `LaneMouse` the tempo map for their readings: `positionText(tick, map)`, and lengths in the meter at the note (`meterAtOf(map, tick)`), as the note list does. No protocol change, no new wording.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->

---
id: UI-ROLL-008
title: Piano roll drags, the note panels and their buttons stay usable while recording or counting in, but core ignores them
area: UI-ROLL
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-ROLL-008: Piano roll drags, the note panels and their buttons stay usable while recording or counting in, but core ignores them

## Where

- `ui/src/components/pianoroll/NoteInspector.svelte:46`, `:95`, `:180`, `:264-265` — `live`, `specOf`, `toolUsable`, `canQuantize`, `canUnquantize` look only at `pr.take === take.id`, not at the transport.
- `ui/src/components/pianoroll/NoteListPanel.svelte:134-139` and `ui/src/components/pianoroll/CcListPanel.svelte:117-122` — fields open and send `setNotesField` / `moveControls` at any time.
- `ui/src/components/pianoroll/Roll.svelte:280` and `ui/src/components/pianoroll/CcLanes.svelte:85` — drags and the draw tool are blocked only for the clip being recorded (`liveMode`), not during the count-in or a recording of another clip.
- Compare `ui/src/components/pianoroll/rollMenu.ts:32` with `Roll.svelte:1103` (the right-click menu disables edit rows while recording or counting in), `ui/src/components/menu/menus.ts:230` (「もとの位置へ戻す」 needs `!busy`), and `ui/src/components/Arrange.svelte:1381-1385` (no clip drag starts while busy).
- `crates/meloet-core/src/play.rs:2349-2354` (`aimed` returns None while busy) and `:2444` (`add_note` returns while busy).

## What happens

During the count-in, and while recording a clip other than the one shown (on another track, or a new clip on the same track), everything in the piano roll that edits notes looks usable: the buttons of 「選んだノート」 (「かける」, 「もとの位置へ戻す」, 「そろえる」, 「割合」「だんだん」「ばらつき」 「かける」, 「のばす」), its fields, the fields of 「ノートの一覧」 and 「CC の一覧」, and drags and drawing in the note area, the velocity lane and the CC lanes. A drag even shows the notes moving. On release or Enter, core does nothing (busy), nothing is said on the status line, and the notes jump back.

## What should happen

Readings A-7, "Right-click during recording and count-in": "Rows that change the song (cut, paste, duplicate, delete, quantize, back to the original position) are disabled." The menu bar disables 「もとの位置へ戻す」 while busy, and the arrange starts no clip drag while busy. The rest of the piano roll should follow the same rule: the panel's buttons and fields in their disabled look, and no note or point drag or draw while busy.

## How to trigger

Read from the code, not run: with two keyboard tracks, open a clip of track 1 in the piano roll, select track 2 and press R. During the count-in or the recording, press 「かける」 under 「クオンタイズ」, or drag a note of the shown clip and let go.

## Cause

These parts never read `transport.state` for count-in or recording; only the right-click menu (`busy`) and the clip being recorded (`liveMode`) are handled.

## Suggested fix

Add the busy check used elsewhere (`transport.state === "recording" || transport.state === "countIn"`, as `songBusy` in `keys/attach.ts`) to `live` / `toolUsable` / `canQuantize` / `canUnquantize` and to opening the list fields, and do not start move, resize, draw, erase, velocity or CC point drags while busy (selecting by a press or a box can stay, as copy and select all stay usable in the right-click menu). No protocol change; the disabled looks already exist.

## Verification

Confirmed by reading (not run). Rust: `aimed` returns `None` while `busy()` (`crates/meloet-core/src/play.rs:2349-2354`), and the note intents go through it; `add_note` returns while busy. Screen: `Roll.svelte:280` blocks drags only in `liveMode` (135: the shown clip is the one being recorded), so during the count-in and while another clip is recorded, drags, drawing, the panel buttons and the list fields all work on the screen and are dropped by core with nothing said. The right-click menu (`rollMenu.ts:32`) and the menu bar already use `busy`. 7-readings A-7 "Right-click during recording and count-in" gives the rule (rows that change the song are disabled).

One root with UI-ARRANGE-009, UI-ARRANGE-013 and UI-PARITY-010 (parts that look usable while recording or counting in though Rust refuses). Also reported as UI-PARITY-005 (the drag half of this report, seen from the parity side: every arrange drag only chooses while busy; merged into this report and deleted). From it, more places: `ui/src/components/pianoroll/Roll.svelte:346-360` (`down`), `drag.ts:191-250` (`RollDrag.down` starts move, resize, velocity, erase and draw with no busy check), `CcLanes.svelte:137-145` and `cc.ts:255-270` (CC points); the siblings that only choose while busy: `Arrange.svelte:1376-1385`, `TempoBand.svelte:99, 166`, `MarkerRow.svelte:65`, `CompBand.svelte:90`, `AudioClipHandles.svelte:174, 268`. While busy, `down` should send only the selection it would send now and start no drag.

Fix plan (as suggested): one `busy` for the piano roll (`transport.state === "recording" || transport.state === "countIn"`, the rule of `songBusy` in `ui/src/keys/attach.ts`), used in: `NoteInspector.svelte` (`live`, `toolUsable`, `canQuantize`, `canUnquantize`: the disabled looks exist), `NoteListPanel.svelte` / `CcListPanel.svelte` (do not open a field), `Roll.svelte` / `CcLanes.svelte` / `drag.ts` / `cc.ts` (no move, resize, draw, erase, velocity or CC point drag starts; selecting by press or box stays), and the piano roll ruler's loop drag (the twin of UI-ARRANGE-013). No protocol or wording change.

## Fix

No protocol or wording change; the disabled looks that exist are used.

- `ui/src/components/pianoroll/drag.ts`, `ui/src/components/pianoroll/cc.ts`: `busy()` in `RollInput` / `LaneInput`; while busy a press sends only the selection it sends now (note, edge, CC point; a box still selects) and no move, resize, draw, erase, velocity or CC point drag starts.
- `ui/src/components/pianoroll/Roll.svelte`, `ui/src/components/pianoroll/CcLanes.svelte`: give `busy` (recording or counting in); the piano roll ruler starts no loop drag and sends no locate while busy (the twin of UI-ARRANGE-013).
- `ui/src/components/pianoroll/NoteInspector.svelte`: `editable` (`live` and not `songBusy`) for the fields, the change-all rows, 「割合」「だんだん」「ばらつき」, 「のばす」, 「かける」 and 「もとの位置へ戻す」; the values stay shown, the field buttons are disabled.
- `ui/src/components/pianoroll/NoteListPanel.svelte`, `ui/src/components/pianoroll/CcListPanel.svelte`: no field opens while busy (rows still select).
- Tests: `ui/tests/pianoroll.drag.test.ts` "while recording or counting in (UI-ROLL-008)" (4 tests); `ui/tests/pianoroll.cc.test.ts` "… while recording, a press on a point only selects it …"; `ui/tests/pianoroll.panels.test.ts` "while recording or counting in, the panels do not edit (UI-ROLL-008)" (2 tests); `ui/tests/pianoroll.r5-notes.test.ts` "while recording or counting in, no field opens (UI-ROLL-008)"; `ui/tests/pianoroll.hold-mount.test.ts` "while counting in or recording (UI-ROLL-008, UI-ARRANGE-013)" (2 tests).

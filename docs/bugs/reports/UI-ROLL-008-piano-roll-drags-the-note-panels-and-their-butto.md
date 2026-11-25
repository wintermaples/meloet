---
id: UI-ROLL-008
title: Piano roll drags, the note panels and their buttons stay usable while recording or counting in, but core ignores them
area: UI-ROLL
kind: parity
severity: low
status: reported
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

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->

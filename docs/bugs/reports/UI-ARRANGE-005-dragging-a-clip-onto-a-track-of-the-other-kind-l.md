---
id: UI-ARRANGE-005
title: Dragging a clip onto a track of the other kind looks allowed; Rust refuses it with nothing said
area: UI-ARRANGE
kind: parity
severity: medium
status: fixed
found: 2026-11-25
---

# UI-ARRANGE-005: Dragging a clip onto a track of the other kind looks allowed; Rust refuses it with nothing said

## Where

ui/src/components/arrangeDrag.ts:149-158 (`groupPlaceable`: checks only "inside the tracks, not before 0, no overlap")
ui/src/components/arrangeDrag.ts:260-265 (`clipDragCursor`: `move` / `copy` while `ok`)
crates/meloet-model/src/arrange.rs:235-236 (`place`: `if doc.tracks[to].kind.is_vocal() == c.is_keys() { return Err(EditError::WrongKind) }`)
crates/meloet-core/src/play.rs:1257-1261 (`apply_placed`: any error other than `Overlap` → `None`, no status text)

## What happens

A keyboard clip dragged up or down onto a vocal track (or an audio clip onto a keyboard track) is shown as a normal move: the moving picture sits on that track, the cursor is `move` (`copy` with Alt), and the reading says the new position. On release `moveClips` is sent, the model answers `WrongKind`, core drops it without a status text, and the clip goes back to where it was. The person is not told why. The same happens with 「貼りつけ」 on an empty place of a track of the other kind (`Arrange.svelte:1653`: the row is usable whenever there is a clipboard).

## What should happen

Architecture 22-15: "Moving or pasting a clip to a track of the other kind returns `EditError::WrongKind`." Architecture 17 / B-71: where a move cannot go ("outside the tracks"), "nothing happens and nothing is said (the screen shows a 'not allowed' cursor)". The screen should treat a row of the other kind the same: `ok: false`, the `not-allowed` cursor, nothing sent.

## How to trigger

Read from the code, not run. Make a keyboard track with a clip and a vocal track under it. Drag the keyboard clip down onto the vocal track's row and release: the picture shows it there with the move cursor, then it goes back, with nothing on the status line.

## Cause

`groupPlaceable` does not compare the clip's kind (`clip.audio !== null`) with the target track's kind (`track.kind.kind`), while the model refuses that pair.

## Suggested fix

In `groupPlaceable`, return false when a clip would land on a track of the other kind (`(clip.audio !== null) !== (track.kind.kind === "vocal")`), so `ok` is false and the cursor is `not-allowed`. Optionally make the empty-place 「貼りつけ」 row unusable on a track of the other kind when the clipboard holds only clips of the other kind (that needs to know the clipboard's kinds; today only `clipboardClips` is sent, so this half needs a protocol field and a lead decision). No wording change for the drag half.

## Verification

Confirmed by reading (not run), the drag half. `groupPlaceable` (`arrangeDrag.ts:149-158`) checks only "a track is there, not before 0, no overlap"; the model's `place` returns `EditError::WrongKind` for a keyboard clip on a vocal track and the other way round (`crates/meloet-model/src/arrange.rs:235-236`), and `apply_placed` (`crates/meloet-core/src/play.rs:1257-1261`) turns every error but `Overlap` into "nothing, no text". So the screen shows `move` / `copy` and a place that Rust will refuse. Architecture 17 gives the rule for places a move cannot go: the "not allowed" cursor, nothing sent.

Fix plan: in `ui/src/components/arrangeDrag.ts` `groupPlaceable`, return false when `(clip.audio !== null) !== (track.kind.kind === "vocal")`; add a case to the arrange drag tests. No protocol or wording change.

The paste half (「貼りつけ」 on an empty place of a track of the other kind, `Arrange.svelte:1653`) is real too but is left out of this report's fix: the screen only knows `clipboardClips` (a count), so disabling the row needs a new protocol field. It goes to `docs/bugs/later/VERIFY-UI.md`.

## Fix

Drag half only. `ui/src/components/arrangeDrag.ts` `groupPlaceable`: false when a clip would land on a track of the other kind (`(clip.audio !== null) !== (track.kind.kind === "vocal")`), so `ok` is false, the cursor is `not-allowed` and nothing is sent. The paste half stays in `docs/bugs/later/VERIFY-UI.md`.
Test: `ui/tests/arrange.group.test.ts` "a keyboard clip on a vocal track, or an audio clip on a keyboard track, is 「できない」 …".

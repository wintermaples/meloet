---
id: UI-ARRANGE-001
title: Automation lane: the kept look of a point drag is applied to the next lane's points and throws during render
area: UI-ARRANGE
kind: bug
severity: high
status: confirmed
found: 2026-11-25
---

# UI-ARRANGE-001: Automation lane: the kept look of a point drag is applied to the next lane's points and throws during render

## Where

ui/src/components/automation/AutoLane.svelte:37-38 (`points` / `pts`: empty while the points of the lane's new `laneId` have not arrived)
ui/src/components/automation/AutoLane.svelte:74-80 (`pending` is cleared only by a `$effect`; `shownDrag` uses `pending.drag` with no check of `pending.laneId`)
ui/src/components/automation/AutoLane.svelte:81-91 (`rects`: `from: d.points.map((i) => pts[i]!)`)
ui/src/components/automation/AutoLane.svelte:153-165 (`up()`: keeps `pending` after `moveAutoPoints`)
ui/src/components/automation/autoGeom.ts:214-217 (`draggedPoints`: `pts[i]!.tick` with indexes of the old lane)

## What happens

After a point drag in an automation lane (or the master's lane) is released, `pending` keeps the drag so the points do not jump back. `MoveAutoPoints` makes a new `LaneId` (architecture 21-5: "new id whenever the points change"), so the state patch that answers the move gives the lane a new `laneId` whose points are not fetched yet (`LaneStore.get` is asked only when something reads them; nothing fetches ahead). In that flush `pts` is `[]`, but `shownDrag` is still `pending.drag` (with `moved: true`), so `A.draggedPoints([], drag)` reads `pts[i]!.tick` of `undefined` and throws a `TypeError`.

The throw happens inside the `use:glSlot` update of the lane (a render effect), which Svelte 5.57 runs before user `$effect`s, so the `$effect` that would clear `pending` (line 74) has not run yet. There is no `<svelte:boundary>` in the app, so the error leaves the batch: `Batch.#process` catches it, calls `reset_all(root)` (every effect of the app tree is marked clean) and throws it again (svelte 5.57.3, `ui/node_modules/svelte/src/internal/client/reactivity/batch.js:366-378`, `reset_all` at 1330; `error-handling.js` `invoke_error_boundary` rethrows with no boundary). Results, read from the code:

- an uncaught `TypeError` in the console after every point drag in the real app;
- the parts later in the tree than this lane, and every `$effect` of that batch (for example `AutoClock`'s position refresh, the mixer, the history band count), do not take that state until something dirties them again;
- when the new points arrive (`autoState` `version` goes up), the lane draws again; if `pending` is still set (the clearing `$effect` was marked clean and nothing has written `pending`, `lane` or `current.rev` since, which is the case when `send()` resolved before the patch came), the stale `pending.drag` is laid over the **new** points: the moved points are shown moved a second time (by `dTick` / `dValue` again), with accent frames at the moved places, until another state change makes the clearing `$effect` run. When `send()` resolves after the patch, its `.then` writes `pending` and the clearing runs at once, so only the throw is seen.

The fake bridge does nothing for `moveAutoPoints` (`ui/src/bridge/fake.ts:3249-3252`, no new lane id), and no test drags a point of a mounted lane, so tests do not see it.

The same throw can come during a drag (not only after it) if the lane's `laneId` changes while the point is held (for example Mod Z while holding a point, or an edit from the AI link): `dragPoints` (`autoGeom.ts:203`) and `moveReading` (`AutoLane.svelte:101`) read `pts[d.grab]!` of the new list.

## What should happen

The kept look should show the moved points until the answer's points are there, then go, with no error (the comment at `AutoLane.svelte:68`: "The look kept after release ... so the point does not jump back"). The sibling `CompBand.svelte:54` does it right: it uses `pending` only while `pending.clip === clip.id`.

## How to trigger

Read from the code, not run. In the real app (not the fake scenes): add a volume lane to a track, add two points, drag one point sideways or up and down, release. When the answer arrives, the console shows `TypeError: Cannot read properties of undefined (reading 'tick')` from `draggedPoints`; depending on which comes first (the command's answer or the patch), the lane line may then show the point moved twice as far as dragged until anything else changes the state.

## Cause

`shownDrag = drag ?? pending?.drag ?? null` (line 79) does not check that `pending.laneId` is still `lane.laneId`, and `draggedPoints` / `rects` index `pts` with the indexes of the old lane. The clearing `$effect` runs after the render effect that already threw.

## Suggested fix

Apply the kept drag to the points it was made on, not to `pts`: while `pending.laneId !== lane.laneId`, draw `draggedPoints` over the points of `pending.laneId` (`peekLanePoints` still has them; `LaneStore.keepOnly` keeps dropped lanes), and drop `pending` once the new lane's points have arrived (`points?.id === lane.laneId`). At the least, use `pending.drag` only while `pending.laneId === lane.laneId` (as `CompBand` does) and make `draggedPoints` / `dragPoints` skip indexes that are not in `pts`. No protocol or wording change.

## Verification

Confirmed, with a test that was run (2026-11-25, scratch worktree, not kept): a vitest test mounted `AutoLane.svelte` (svelte 5.57.3, jsdom) with a lane of three points (the points given through a mocked `peekLanePoints`), sent pointerdown / pointermove / pointerup on the middle point (`moveAutoPoints` was sent), then gave the lane prop the same lane with a new `laneId` whose points are not there, and called `flushSync()`. Result: `flushSync` threw `TypeError: Cannot read properties of undefined (reading 'tick')` with `draggedPoints` in the stack. In that run `pending.laneId !== lane.laneId` already held, so the clearing `$effect` would have dropped `pending`; the throw came first.

Why the order is so (read in `ui/node_modules/svelte`, 5.57.3): `use:glSlot={{ kind: "rects", rects }}` reads `rects` inside a `render_effect` (`src/internal/client/dom/elements/actions.js:22`), and render effects are run before user `$effect`s. With no `<svelte:boundary>` in `ui/src` (grep: none), the error leaves `Batch.#process`, which calls `reset_all(root)` and throws again (`reactivity/batch.js:366-378`).

Not run, read only: the "moved a second time" picture after the new points arrive (it needs `send()` to resolve before the patch; the logic in the report is right as far as I can read it), and the throw during a held drag when the `laneId` changes (`dragPoints`, `moveReading`: the same `pts[i]!` on an empty or shorter list).

Rust side checked: `auto::move_points` returns a new `LaneId` (`crates/meloet-model/src/automation.rs:425-429`), so every real move goes this way. The fake bridge does nothing for `moveAutoPoints`, which is why no test saw it.

Severity stays high: an uncaught error on every point move in the real app, and the batch is thrown away (other parts of the screen miss that state).

Fix plan (agreed, the smallest): in `ui/src/components/automation/AutoLane.svelte` make the kept look safe at the source: `const shownDrag = $derived(drag ?? (pending && pending.laneId === lane.laneId ? pending.drag : null))`, and drop a held `drag` when `lane.laneId` is no longer the one it was started on (remember the lane id in the drag, or cancel it in the same derived). In `ui/src/components/automation/autoGeom.ts` make `draggedPoints` and `dragPoints` return the input unchanged when an index is not in `pts` (so a wrong index can never throw in render). With only this, the new lane's points are not here yet, so the lane is empty for the frames until they arrive (as it already is after an add or a delete). If the lead wants no blink, draw the old lane's points (`peekLanePoints(pending.laneId)`) with the drag until `points?.id === lane.laneId`; that is the report's first choice and also needs no protocol change. Add a test of the mounted lane with a new `laneId` after a move (the form above), and make the fake bridge give a new lane id on `moveAutoPoints` so scenes go the real way.

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->

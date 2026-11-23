<!-- The line inside the rec border while writing with 「● 書く」 (mock F6-AutoRec). The core puts the written values in as one step on release
     (docs/architecture.md 21-9), so until then the screen collects (tick, value) from the hand's position (handValue.ts) and the position heard now,
     and draws only inside the border. Outside the border it stays the lane's line (AutoLane, one canvas of arrangeLanes). It changes at 60 Hz, so it is drawn in the
     same small region as the tip of the clip being recorded (recordingClip). The box goes from the start of writing to the right end of the lane, and its top and bottom edges line up with the lane (WebKit that paints on the CPU, chapter 7).
     The picture is redrawn only when the right edge of the border moves by a screen pixel, or when the hand's value changes.
     On release, the core's step (a new laneId) comes, and when its points arrive, the collected values are dropped. When cancelled (it ends with the same laneId), they are dropped at once. -->
<script lang="ts">
  import type { LaneView, TrackView } from "../../bridge";
  import { everyGlFrame, glSlot, type GlRect } from "../../gl/slot";
  import { current } from "../../state/app.svelte";
  import { tickToX, type View } from "../arrangeGeom";
  import { metrics } from "../metrics.svelte";
  import * as A from "./autoGeom";
  import { playTick, pointsOf } from "./autoState.svelte";
  import { handAt } from "./handValue";
  import { sameTarget } from "./laneValue";

  interface Props {
    track: TrackView;
    lane: LaneView;
    top: number;
    view: View;
  }
  let { track, lane, top, view }: Props = $props();

  const writing = $derived(current.state!.automation.writing.find((w) => w.lane.track === track.id && sameTarget(w.lane.target, lane.target)) ?? null);
  const playing = $derived(current.state!.transport.state !== "stopped");
  const points = $derived(pointsOf(lane.laneId));
  const committed = $derived(points && points.id === lane.laneId ? A.pointList(points) : []);

  /** The collected (tick, value). laneId is the lane at the start of writing, from is the tick at the start of writing. done: writing has ended (waiting for the core's step). */
  let buf = $state.raw<{ laneId: number; from: number; pts: A.Pt[]; range: A.LaneRange; done: boolean } | null>(null);
  let now = $state(0);

  function sample(): void {
    const w = writing;
    if (!w) return;
    const hand = handAt(track.id, lane.target);
    if (hand === null) return;
    const tick = Math.max(w.fromTick, playTick());
    const b = buf;
    if (!b || b.done || b.from !== w.fromTick) {
      buf = { laneId: lane.laneId, from: w.fromTick, pts: [{ tick: w.fromTick, value: hand }], range: A.laneRange(committed, lane.step), done: false };
      now = tick;
      return;
    }
    const last = b.pts[b.pts.length - 1]!;
    const nowPx = Math.round(tickToX(view, tick) * metrics.dpr);
    if (last.value === hand && nowPx === Math.round(tickToX(view, now) * metrics.dpr)) return;
    if (tick > last.tick) buf = { ...b, pts: [...b.pts, { tick, value: hand }] };
    now = tick;
  }

  $effect(() => {
    if (!writing || !playing) return;
    return everyGlFrame(sample);
  });

  // After writing ends, keep them until the points of the core's step arrive. If the lane stays the same (there was no step), drop them at once.
  $effect(() => {
    const b = buf;
    if (!b) return;
    if (writing && !b.done) return;
    if (!b.done) {
      buf = lane.laneId === b.laneId ? null : { ...b, done: true };
      return;
    }
    if (lane.laneId !== b.laneId && points && points.id === lane.laneId) buf = null;
  });

  const dims = $derived<A.LaneDims>({
    h: metrics.px("size.lane"),
    pad: metrics.px("space.1"),
    hair: metrics.px("line.hair"),
    point: metrics.px("part.cc.point"),
    range: buf?.range,
  });
  const x0 = $derived(buf ? Math.max(0, tickToX(view, buf.from)) : 0);

  /** Only line points that are at least 2 points' width apart sideways (the spacing in mock F6-AutoRec). The newest one is kept. */
  function thinned(pts: readonly A.Pt[], minPx: number): A.Pt[] {
    const out: A.Pt[] = [];
    let lastX = -Infinity;
    pts.forEach((p, i) => {
      const x = tickToX(view, p.tick);
      if (x - lastX >= minPx || i === pts.length - 1) {
        out.push(p);
        lastX = x;
      }
    });
    return out;
  }

  const rects = $derived.by((): GlRect[] => {
    const b = buf;
    if (!b) return [];
    const d = dims;
    const right = tickToX(view, Math.max(b.from, now)) - x0;
    const innerW = right - 2 * d.hair;
    if (innerW <= 0) return [];
    // A background that hides the lane's line inside the border (the border line stays as AutoLane's HTML).
    const out: GlRect[] = [{ x: d.hair, y: d.hair, w: innerW, h: d.h - 2 * d.hair - d.hair, color: "color.gray.2" }];
    const v: View = { pxPerTick: view.pxPerTick, scrollX: view.scrollX + x0 + d.hair, width: innerW - d.hair };
    const look: A.LaneLook = { resting: false, armed: false, selected: () => false, hovered: -1, from: [], flat: null };
    for (const r of A.laneRects(v, thinned(b.pts, 2 * d.point), lane.step, d, look)) out.push({ ...r, x: r.x + d.hair });
    return out;
  });
</script>

{#if buf}
  <div
    class="write-line"
    style:left="{x0}px"
    style:top="{top}px"
    style:width="{Math.max(0, view.width - x0)}px"
    style:height="{dims.h}px"
    use:glSlot={{ kind: "rects", rects }}
    data-test="auto-write-line"
  ></div>
{/if}

<style>
  .write-line {
    position: absolute;
  }
</style>

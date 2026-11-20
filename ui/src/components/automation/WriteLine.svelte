<!-- 「● 書く」で書いている間の、rec の枠の中の線（モック F6-AutoRec）。core は書いた値を離したときに 1 手で入れる
     （docs/architecture.md 21-9）ので、それまでは画面が、手の位置（handValue.ts）と耳に届いている位置から（ティック、値）をため、
     枠の中だけを描く。枠の外は帯の線のまま（AutoLane、arrangeLanes の 1 枚）。60 Hz で変わるので、録っているクリップの先と同じ
     小さな区画（recordingClip）に描く。箱は書きはじめから帯の右はしまでで、帯と上下の端がそろう（CPU で描く WebKit、7 章）。
     絵は、枠の右はしが画面の画素で動いたときか、手の値が変わったときだけ書きなおす。
     離したら core の手（新しい laneId）が来て、その点が届いたら、ためた物を捨てる。やめた（同じ laneId のまま終わった）ときは、すぐ捨てる。 -->
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

  /** ためた（ティック、値）。laneId は書きはじめの帯、from は書きはじめのティック。done: 書きおわった（core の手を待つ）。 */
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

  // 書きおわったら、core の手の点が届くまで残す。同じ帯のまま（手がなかった）なら、すぐ捨てる。
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

  /** 線の点は、横に点 2 つぶん以上はなれた物だけ（モック F6-AutoRec の間）。いちばん新しい物は残す。 */
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
    // 枠の中の帯の線をかくす地（枠の線は AutoLane の HTML のまま見せる）。
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

<!-- The time side of an open automation lane in the arrange (spec 2 "Automation point", spec 3 (Y16), mock R10-AutoEdit, F6-AutoRec, F6-AutoRest).
     Lines and points are WebGL (one canvas of the arrange's arrangeLanes region; no new region); the reading and the border of the part being written are HTML.
     Pressing and releasing on an empty place adds a point (with the value on the line at that place, snapped to the start of a beat). Holding a point and moving it changes the value up and down, and the position sideways.
     Holding and moving from an empty place chooses by a box. A point is removed by pressing it twice, or by choosing it and pressing Delete (laneKeys.ts).
     While dragging, only the screen moves first, and one intent is sent on release. Esc cancels (cancelDrag in pianoroll/view.svelte.ts). -->
<script lang="ts">
  import { onDestroy, onMount, untrack } from "svelte";
  import { peekLanePoints, type LaneView } from "../../bridge";
  import { everyGlFrame, glSlot } from "../../gl/slot";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text";
  import type { TempoMap } from "../../time/tempoMap";
  import { tickToX, type View } from "../arrangeGeom";
  import { metrics } from "../metrics.svelte";
  import { builtinOf } from "../params/selected";
  import { dragReading, onDragCancel } from "../pianoroll/view.svelte";
  import { positionText as placeText, ticksPerBar, ticksPerBeat } from "../time";
  import { pickLane } from "../timeline/pick";
  import * as A from "./autoGeom";
  import { playTick, pointsOf } from "./autoState.svelte";
  import { laneMenu } from "./laneMenu";
  import { bareText, handPosition, hasLaneRef, isOnTrack, positionText, valueAt, type LaneOwner } from "./laneValue";

  interface Props {
    track: LaneOwner;
    lane: LaneView;
    view: View;
    map: TempoMap;
    busy: boolean;
  }
  let { track, lane, view, map, busy }: Props = $props();

  const automation = $derived(current.state!.automation);
  const transport = $derived(current.state!.transport);
  const still = () => metrics.px("space.1");

  const points = $derived(pointsOf(lane.laneId));
  const pts = $derived(points && points.id === lane.laneId ? A.pointList(points) : []);
  /** The look kept after release and sending the intent, until the answer arrives (so the point does not jump back to the original place for a moment). */
  let pending = $state.raw<{ drag: A.PointDrag; laneId: number; rev: number | null } | null>(null);
  /** The kept look with the points it was made on: the answer gives the lane a new id (architecture 21-5), whose points come a moment later. */
  const kept = $derived.by(() => {
    const p = pending;
    if (!p) return null;
    if (p.laneId === lane.laneId) return { drag: p.drag, pts };
    const old = points ? undefined : peekLanePoints(p.laneId);
    return old ? { drag: p.drag, pts: A.pointList(old) } : null;
  });
  /** The points drawn: the lane's, or the ones the kept look was made on while the new ones are on their way. */
  const drawn = $derived(kept?.pts ?? pts);
  /** The lane's vertical range (spec 3 (92-6, 21-B)). It comes from the points that arrived, so when a point is moved outside, it widens again when the answer comes after release. */
  const range = $derived(A.laneRange(drawn, lane.step));
  const dims: A.LaneDims = $derived({
    h: metrics.px("size.lane"),
    pad: metrics.px("space.1"),
    hair: metrics.px("line.hair"),
    point: metrics.px("part.cc.point"),
    range,
  });
  /** 「上 −3.0 dB」「下 −9.0 dB」 at the top right and bottom right. Not shown for a lane that shows the whole range, or one whose value text the screen cannot make (plugins). */
  const ends = $derived.by(() => {
    if (range === A.FULL_RANGE) return null;
    const e = A.rangeEnds(drawn, range);
    const hi = positionText(lane.name, e.hi);
    const lo = positionText(lane.name, e.lo);
    return hi === null || lo === null ? null : { hi, lo };
  });
  const resting = $derived(lane.resting || hasLaneRef(automation.held, track.id, lane.target));
  const armed = $derived(hasLaneRef(automation.armed, track.id, lane.target));
  const selected = $derived(automation.selected?.laneId === lane.laneId ? A.indexSet(automation.selected.points) : new Set<number>());
  const values = $derived.by(() => {
    const sel = builtinOf(current.state!.params);
    if (!sel || lane.target.kind !== "insert" || sel.insert.strip.kind !== "track" || sel.insert.strip.id !== track.id) return null;
    return track.slots[sel.insert.slot]?.id === lane.target.insert ? sel.values : null;
  });

  // ---- Dragging ----

  let drag = $state.raw<A.PointDrag | null>(null);
  /** The lane id the held point was pressed on: its indexes mean nothing in the points of another id. */
  let dragLane = $state(0);
  let box = $state.raw<{ x0: number; y0: number; x: number; y: number; moved: boolean; shift: boolean } | null>(null);
  let hovered = $state(-1);
  let reading = $state<{ text: string; x: number; y: number } | null>(null);

  $effect(() => {
    const p = pending;
    if (p && (p.laneId === lane.laneId ? p.rev !== null && current.rev >= p.rev : !kept)) pending = null;
  });

  const shownDrag = $derived((dragLane === lane.laneId ? drag : null) ?? kept?.drag ?? null);
  const shown = $derived(A.draggedPoints(drawn, shownDrag));
  const rects = $derived.by(() => {
    const d = shownDrag?.moved ? shownDrag : null;
    return A.laneRects(view, shown.pts, lane.step, dims, {
      resting,
      armed,
      selected: (i) => (d ? shown.moving[i]! : selected.has(i)),
      hovered: d ? -1 : hovered,
      from: d ? d.points.flatMap((i) => drawn[i] ?? []) : [],
      flat: drawn.length === 0 ? handPosition(track, lane.target, values) : null,
    });
  });

  let node = $state<HTMLElement>();
  const at = (e: MouseEvent) => {
    const r = node!.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };
  const meterAt = (tick: number) => map.meterAt(Math.max(0, tick));
  /** The reading in place; the status line shows the same text (spec 2 "Reading in place"). */
  function read(next: { text: string; x: number; y: number } | null): void {
    if (next || reading) dragReading.text = next?.text ?? null;
    reading = next;
  }

  function moveReading(d: A.PointDrag, p: { x: number; y: number }): void {
    const grab = pts[d.grab]!;
    const to = Math.min(1, Math.max(0, grab.value + d.dValue));
    const m = meterAt(grab.tick);
    const delta = A.deltaText(d.dTick, ticksPerBar(m), ticksPerBeat(m));
    const from = bareText(lane.name, grab.value);
    const toText = positionText(lane.name, to);
    read({ text: from !== null && toText !== null ? t("lane.readMove", { delta, from, to: toText }) : delta, x: p.x, y: p.y });
  }
  function addReading(p: { x: number; y: number }, shift: boolean): void {
    const tick = A.addTick(view, map, p.x, shift);
    const v = points ? valueAt(points, tick) : null;
    const value = v === null ? null : positionText(lane.name, v);
    const position = placeText(tick, map);
    read({ text: value === null ? position : t("lane.readAdd", { position, value }), x: p.x, y: p.y });
  }

  function down(e: PointerEvent): void {
    if (e.button !== 0) return;
    e.stopPropagation();
    const p = at(e);
    const i = A.pointAt(view, pts, dims, p.x, p.y, dims.point);
    pickLane();
    node!.setPointerCapture?.(e.pointerId);
    if (i < 0) {
      box = { x0: p.x, y0: p.y, x: p.x, y: p.y, moved: false, shift: e.shiftKey };
      if (!busy) addReading(p, e.shiftKey);
      return;
    }
    const chosen = selected.has(i) ? [...selected].sort((a, b) => a - b) : [i];
    if (!selected.has(i)) send({ type: "selectAutoPoints", lane: lane.laneId, points: [i] });
    if (busy) return;
    pending = null;
    dragLane = lane.laneId;
    drag = { points: chosen, grab: i, downX: p.x, downY: p.y, moved: false, dTick: 0, dValue: 0 };
  }
  function move(e: PointerEvent): void {
    const p = at(e);
    const d = drag;
    if (d && dragLane !== lane.laneId) {
      cancel();
      return;
    }
    if (d) {
      const next = A.dragPoints(d, pts, view, dims, map, p.x, p.y, still(), e.shiftKey);
      if (next !== d) drag = next;
      if (next.moved) moveReading(next, p);
      return;
    }
    const b = box;
    if (b) {
      const moved = b.moved || Math.hypot(p.x - b.x0, p.y - b.y0) >= still();
      box = { ...b, x: p.x, y: p.y, moved };
      if (moved) read(null);
      return;
    }
    hovered = A.pointAt(view, pts, dims, p.x, p.y, dims.point);
  }
  function up(e: PointerEvent): void {
    const p = at(e);
    read(null);
    const d = drag;
    if (d) {
      drag = null;
      if (dragLane !== lane.laneId || !d.moved || (d.dTick === 0 && d.dValue === 0)) return;
      const made = { drag: d, laneId: lane.laneId, rev: null as number | null };
      pending = made;
      void send({ type: "moveAutoPoints", lane: lane.laneId, points: d.points, dTick: d.dTick, dValue: d.dValue }).then((rev) => {
        if (pending === made) pending = { ...made, rev: rev ?? -Infinity };
      });
      return;
    }
    const b = box;
    if (!b) return;
    box = null;
    if (b.moved) {
      send({ type: "selectAutoPoints", lane: lane.laneId, points: A.pointsInBox(view, pts, dims, b.x0, b.y0, p.x, p.y) });
    } else if (!busy) {
      send({ type: "addAutoPoint", lane: lane.laneId, tick: A.addTick(view, map, p.x, b.shift), value: null });
    }
  }
  function cancel(): boolean {
    if (!drag && !box) return false;
    drag = null;
    box = null;
    read(null);
    return true;
  }
  onMount(() => onDragCancel(cancel));
  onDestroy(() => read(null));

  function double(e: MouseEvent): void {
    e.stopPropagation();
    if (busy) return;
    const p = at(e);
    const i = A.pointAt(view, pts, dims, p.x, p.y, dims.point);
    if (i >= 0) send({ type: "deleteAutoPoints", lane: lane.laneId, points: [i] });
  }
  function context(e: MouseEvent): void {
    e.preventDefault();
    e.stopPropagation();
    if (drag || box) return;
    laneMenu(e, track, lane, busy);
  }

  const boxFrame = $derived(
    box?.moved ? { x: Math.min(box.x0, box.x), y: Math.min(box.y0, box.y), w: Math.abs(box.x - box.x0), h: Math.abs(box.y - box.y0) } : null,
  );

  let readingW = $state(0);
  let readingH = $state(0);
  const readingBox = $derived.by(() => {
    if (!reading) return null;
    const off = metrics.px("part.reading.offset");
    return { left: Math.max(0, reading.x - off - readingW), top: reading.y - off - readingH };
  });

  // ---- The rec border of the part being written (from the start of writing to the play line. The line moves every frame, so only the right edge is written per frame) ----

  const writing = $derived(automation.writing.find((w) => isOnTrack(w.lane, track.id) && JSON.stringify(w.lane.target) === JSON.stringify(lane.target)) ?? null);
  let recFrame = $state<HTMLElement | null>(null);
  let drawnWidth = NaN;
  function placeRec(): void {
    const w = writing;
    if (!w || !recFrame) return;
    const x0 = tickToX(view, w.fromTick);
    // The meters come about 60 times a second: read untracked, so the effect below does not start again for each.
    const x1 = tickToX(view, Math.max(w.fromTick, untrack(playTick)));
    const width = Math.round((x1 - Math.max(x0, 0)) * metrics.dpr) / metrics.dpr;
    if (width === drawnWidth) return;
    drawnWidth = width;
    recFrame.style.width = `${Math.max(0, width)}px`;
  }
  $effect(() => {
    void view;
    void recFrame;
    drawnWidth = NaN;
    placeRec();
    if (!writing || transport.state === "stopped") return;
    return everyGlFrame(placeRec);
  });
</script>

<div
  class="auto-lane"
  role="presentation"
  bind:this={node}
  use:glSlot={{ kind: "rects", rects }}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={cancel}
  onpointerleave={() => !drag && (hovered = -1)}
  ondblclick={double}
  oncontextmenu={context}
  data-test="auto-lane"
>
  {#if ends}
    <span class="m-num m-lbl end top" data-test="auto-range-top">{t("lane.rangeTop", { value: ends.hi })}</span>
    <span class="m-num m-lbl end bottom" data-test="auto-range-bottom">{t("lane.rangeBottom", { value: ends.lo })}</span>
  {/if}
  {#if writing}
    <div class="rec-frame" bind:this={recFrame} style:left="{Math.max(0, tickToX(view, writing.fromTick))}px" data-test="auto-writing"></div>
  {/if}
  {#if boxFrame}
    <div class="box-frame" style:left="{boxFrame.x}px" style:top="{boxFrame.y}px" style:width="{boxFrame.w}px" style:height="{boxFrame.h}px" data-test="auto-box"></div>
  {/if}
  {#if readingBox && reading}
    <div class="m-num reading" style:left="{readingBox.left}px" style:top="{readingBox.top}px" bind:clientWidth={readingW} bind:clientHeight={readingH} data-test="auto-reading">
      {reading.text}
    </div>
  {/if}
</div>

<style>
  /* The open lane (size.lane, background gray.2. Mock R10-AutoEdit). */
  .auto-lane {
    /* Lanes scrolled out of view are not painted or laid out (songs with 64 tracks. The height is fixed). */
    content-visibility: auto;
    position: relative;
    height: var(--size-lane);
    background: var(--color-gray-2);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    touch-action: none;
  }
  /* The part being written over (spec 3 (Y16): a line.hair border in the rec color). */
  .rec-frame {
    position: absolute;
    top: 0;
    bottom: 0;
    border: var(--line-hair) solid var(--color-rec);
    pointer-events: none;
  }
  .end {
    position: absolute;
    right: var(--space-2);
    white-space: nowrap;
    pointer-events: none;
  }
  .end.top {
    top: 0;
  }
  .end.bottom {
    bottom: 0;
  }
  .box-frame {
    position: absolute;
    z-index: 3;
    border: var(--line-hair) solid var(--color-accent-ink);
    pointer-events: none;
  }
  .reading {
    position: absolute;
    z-index: 10;
    height: var(--part-reading-height);
    padding: 0 var(--part-reading-pad-x);
    display: flex;
    align-items: center;
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-8);
    color: var(--color-gray-10);
    white-space: nowrap;
    pointer-events: none;
  }
</style>

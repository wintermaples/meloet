<!-- CC lanes (the 「サステイン ・ ピッチベンド ・ モジュレーション」 row of the band table in spec 3 and the CC lanes in "Rules for round 3 (piano roll and clips)", mock R3-Lanes).
     Below the velocity lane, only the open ones, in the order sustain, pitch bend, modulation, other CCs. size.lane each.
     If the note area goes below 10 rows, the ones opened earliest are folded first.
     Each lane is drawn in the WebGL pianoRoll region (only when the points or the visible range change).
     The lines and points that move while dragging, and the point before it is placed, are a one-lane surface in the pianoRollLive region (liveHost in Roll.svelte). -->
<script lang="ts">
  import { onDestroy, onMount, untrack } from "svelte";
  import type { Command } from "../../bridge";
  import { glSlot, type GlSlot } from "../../gl/slot";
  import { platformOf } from "../../keys/keys";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { metrics } from "../metrics.svelte";
  import type { Meter } from "../time";
  import { mapOf } from "../../time/bars";
  import { fitLanes, LANE_ORDER, LaneMouse, laneControl, type LaneBox, type LaneDrag } from "./cc";
  import { cursorCss } from "./cursor";
  import type { CursorKind, Mods } from "./drag";
  import * as G from "./geom";
  import { paintLane, paintLaneLive, type LanePicture } from "./paint";
  import { ccLanes, dragReading, onDragCancel, prView, type LaneTag } from "./view.svelte";

  interface Props {
    g: G.RollGeom;
    meter: Meter;
    step: number;
    /** The shown take (null when what is being recorded is shown). */
    take: G.Shown | null;
    /** The height of the note area (CSS px). */
    areaH: number;
    /** The box of the pianoRollLive region (its top left is the note area's top left). */
    liveHost: HTMLElement | undefined;
    onreading?: (at: { x: number; y: number } | null) => void;
  }
  let { g, meter, step, take, areaH, liveHost, onreading }: Props = $props();

  const pr = $derived(current.state!.pianoRoll);
  const scale = $derived(metrics.scale);
  const laneH = $derived(metrics.px("size.lane"));
  /** The row height (the vertical zoom step; B-63). */
  const rowH = $derived(g.rowH);
  const pad = $derived(metrics.px("mock.pianoroll.cc-pad"));
  const point = $derived(metrics.px("part.cc.point"));

  const tags = $derived(LANE_ORDER.filter((tag) => ccLanes.open.includes(tag)));

  $effect(() => {
    const open = ccLanes.open;
    if (!(areaH > 0) || open.length < 2) return;
    const keep = fitLanes(open, areaH, laneH, rowH);
    if (keep.length !== open.length) ccLanes.open = keep;
  });

  function nameOf(tag: LaneTag): string {
    switch (tag) {
      case "sustain":
        return t("pr.sustain");
      case "pitchBend":
        return t("pr.pitchBend");
      case "modulation":
        return t("pr.modulation");
      case "other":
        return ccLanes.other === null ? t("pr.otherCc") : t("pr.ccNumber", { number: String(ccLanes.other) });
    }
  }
  const fold = (tag: LaneTag) => (ccLanes.open = ccLanes.open.filter((x) => x !== tag));

  const selectedRanges = $derived(take && pr.take === take.take.id ? pr.selectedControls : []);
  const selFlags = $derived(take ? G.selectionFlags(take.take.controlTicks.length, selectedRanges) : null);

  // ---- Mouse ----

  const heights = $state<Record<LaneTag, number>>({ sustain: 0, pitchBend: 0, modulation: 0, other: 0 });
  const bodies: Partial<Record<LaneTag, HTMLElement>> = {};
  /** The lane the mouse is over or pressing now. */
  let active = $state<LaneTag | null>(null);
  let dragVersion = $state(0);
  let eraseHover = $state(-1);
  let pending = $state.raw<{ tag: LaneTag; drag: LaneDrag } | null>(null);
  let pendingReplied = $state(false);
  let lastSent: Promise<unknown> | null = null;

  const boxOf = (tag: LaneTag | null): LaneBox => ({ height: tag ? heights[tag] : 0, pad });
  const mouse = new LaneMouse({
    shown: () => take,
    control: () => (active ? laneControl(active, ccLanes.other) : null),
    geom: () => g,
    box: () => boxOf(active),
    step: () => step,
    meter: () => meter,
    tool: () => prView.tool,
    selected: () => selectedRanges,
    reachPx: () => metrics.px("space.1"),
    stillPx: () => metrics.px("space.1"),
    send: (c: Command) => {
      lastSent = send(c);
    },
  });

  const shownDrag = $derived.by(() => {
    void dragVersion;
    if (mouse.drag && active) return { tag: active, drag: mouse.drag };
    return pending;
  });

  $effect(() => {
    if (!pending || !pendingReplied) return;
    if (pr.take === (take?.take.id ?? null)) pending = null;
  });

  const platform = platformOf(navigator);
  const modsOf = (e: MouseEvent): Mods => ({ shift: e.shiftKey, alt: e.altKey, mod: platform === "mac" ? e.metaKey : e.ctrlKey });
  let cursor = $state<CursorKind>("default");
  const cursorStyle = $derived(cursorCss(cursor, scale));

  function at(e: MouseEvent, tag: LaneTag): { x: number; y: number } {
    const r = bodies[tag]!.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  }

  function changed(e: MouseEvent | null): void {
    dragVersion += 1;
    dragReading.text = mouse.reading();
    onreading?.(dragReading.text && e ? { x: e.clientX, y: e.clientY } : null);
    placeLive();
  }

  /** The cursor at p in the active lane. */
  function cursorAt(p: { x: number; y: number }): CursorKind {
    const d = mouse.drag;
    if (d) return d.kind === "move" ? "move" : d.kind === "add" ? "pencil" : d.kind === "erase" ? "eraser" : "default";
    if (prView.tool === "erase") return "eraser";
    if (mouse.over(p.x, p.y)) return "move";
    return prView.tool === "draw" && take ? "pencil" : "default";
  }

  function down(e: PointerEvent, tag: LaneTag): void {
    if (e.button !== 0) return;
    prView.focus = "pianoRoll";
    prView.picked = "controls";
    pending = null;
    active = tag;
    const p = at(e, tag);
    mouse.down(p.x, p.y, modsOf(e));
    eraseHover = -1;
    if (mouse.drag) (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    cursor = cursorAt(p);
    changed(e);
  }

  function move(e: PointerEvent, tag: LaneTag): void {
    if (mouse.drag) {
      const p = at(e, active!);
      if (mouse.move(p.x, p.y, modsOf(e))) changed(e);
      else if (dragReading.text) onreading?.({ x: e.clientX, y: e.clientY });
      return;
    }
    active = tag;
    const p = at(e, tag);
    if (mouse.hover(p.x, p.y)) eraseHover = mouse.eraseHover;
    cursor = cursorAt(p);
  }

  function up(): void {
    if (!mouse.drag || !active) return;
    const was = { tag: active, drag: mouse.drag };
    lastSent = null;
    const command = mouse.up();
    if (command && lastSent && command.type !== "selectControls") {
      pending = was;
      pendingReplied = false;
      void (lastSent as Promise<unknown>).then(() => (pendingReplied = true));
    }
    changed(null);
  }

  function leave(): void {
    if (mouse.drag) return;
    if (eraseHover >= 0) eraseHover = mouse.eraseHover = -1;
  }

  onMount(() =>
    onDragCancel(() => {
      if (!mouse.cancel()) return false;
      changed(null);
      return true;
    }),
  );
  onDestroy(() => {
    if (mouse.cancel()) {
      dragReading.text = null;
      onreading?.(null);
    }
  });

  // ---- WebGL ----

  const pictures = new Map<LaneTag, LanePicture>();
  const handles = new Map<LaneTag, { update(s: GlSlot): void }>();
  const slotOf = (tag: LaneTag): GlSlot => ({
    kind: "paint",
    paint: (out, box, ctx) => {
      const p = pictures.get(tag);
      if (p) paintLane(out, box, p, ctx.dpr);
    },
  });

  function laneSlot(node: HTMLElement, tag: LaneTag) {
    const h = glSlot(node, slotOf(tag));
    handles.set(tag, h);
    return {
      destroy() {
        h.destroy();
        if (handles.get(tag) === h) handles.delete(tag);
      },
    };
  }

  /** The shape of the drag that matters for the large picture (the amount while moving is drawn only by the moving surface). */
  function dragPart(d: LaneDrag): string {
    switch (d.kind) {
      case "move":
        return `m${d.press.moved ? 1 : 0}`;
      case "box":
        return `b${d.hits.length}:${d.hits.reduce((a, b) => a + b, 0)}`;
      case "erase":
        return `e${d.hits.size}`;
      case "add":
        return "a";
    }
  }
  const serials = new WeakMap<LaneDrag, number>();
  let serial = 0;
  const dragKey = $derived.by(() => {
    const s = shownDrag;
    if (!s) return "";
    if (!serials.has(s.drag)) serials.set(s.drag, ++serial);
    return `${s.tag}:${serials.get(s.drag)}:${dragPart(s.drag)}`;
  });

  function pictureOf(tag: LaneTag, d: LaneDrag | null): LanePicture {
    return {
      g,
      meter,
      step,
      map: mapOf(current.state!.timeline),
      barStep: pr.grid.division === 0,
      control: laneControl(tag, ccLanes.other),
      shown: take,
      lane: boxOf(tag),
      point,
      selected: selFlags,
      drag: d,
      eraseHover: tag === active ? eraseHover : -1,
    };
  }

  $effect(() => {
    void dragKey;
    void heights.sustain;
    void heights.pitchBend;
    void heights.modulation;
    void heights.other;
    const s = untrack(() => shownDrag);
    pictures.clear();
    for (const tag of tags) pictures.set(tag, pictureOf(tag, s && s.tag === tag ? s.drag : null));
    untrack(() => {
      for (const [tag, h] of handles) h.update(slotOf(tag));
    });
  });

  // The moving surface: one lane, the one being dragged.
  let liveTop = $state(0);
  let liveH = $state(0);
  function placeLive(): void {
    const s = shownDrag;
    const body = s ? bodies[s.tag] : undefined;
    if (!body || !liveHost) return;
    const r = body.getBoundingClientRect();
    liveTop = r.top - liveHost.getBoundingClientRect().top;
    liveH = r.height;
  }
  const live = $derived.by(() => {
    void dragVersion;
    const s = shownDrag;
    if (!s || !liveHost) return null;
    if (s.drag.kind === "add" || (s.drag.kind === "move" && s.drag.press.moved)) return s;
    return null;
  });
  let livePicture: LanePicture | null = null;
  let liveHandle: { update(s: GlSlot): void } | null = null;
  const liveSlot: GlSlot = { kind: "paint", paint: (out, box, ctx) => livePicture && paintLaneLive(out, box, livePicture, ctx.dpr) };
  function liveSlotAction(node: HTMLElement) {
    liveHost?.append(node);
    const h = glSlot(node, liveSlot);
    liveHandle = h;
    return {
      destroy() {
        h.destroy();
        node.remove();
        if (liveHandle === h) liveHandle = null;
      },
    };
  }
  $effect(() => {
    const s = live;
    void dragVersion;
    livePicture = s ? pictureOf(s.tag, s.drag) : null;
    untrack(() => liveHandle?.update(liveSlot));
  });

  // The box for choosing
  const boxFrame = $derived.by(() => {
    void dragVersion;
    const d = mouse.drag;
    if (!d || d.kind !== "box" || !d.press.moved || !active) return null;
    return { tag: active, x: Math.min(d.press.x, d.x), y: Math.min(d.press.y, d.y), w: Math.abs(d.x - d.press.x), h: Math.abs(d.y - d.press.y) };
  });
</script>

{#each tags as tag (tag)}
  <div class="lane" data-test="pr-lane-{tag}">
    <button class="m-plain head" onclick={() => fold(tag)} data-test="pr-lane-fold-{tag}">
      <span class="m-hi">▾ {nameOf(tag)}</span>
    </button>
    <div
      class="body"
      role="presentation"
      bind:this={bodies[tag]}
      bind:clientHeight={heights[tag]}
      style:cursor={tag === active ? cursorStyle : null}
      onpointerdown={(e) => down(e, tag)}
      onpointermove={(e) => move(e, tag)}
      onpointerup={up}
      onpointercancel={() => mouse.cancel() && changed(null)}
      onpointerleave={leave}
      use:laneSlot={tag}
    >
      {#if boxFrame && boxFrame.tag === tag}
        <div class="box-frame" style:left="{boxFrame.x}px" style:top="{boxFrame.y}px" style:width="{boxFrame.w}px" style:height="{boxFrame.h}px"></div>
      {/if}
    </div>
  </div>
{/each}
{#if live}
  <div class="live" style:top="{liveTop}px" style:height="{liveH}px" use:liveSlotAction></div>
{/if}

<style>
  .lane {
    height: var(--size-lane);
    flex-shrink: 0;
    display: flex;
    border-top: var(--line-hair) solid var(--color-gray-6);
  }
  .head {
    width: var(--pr-keys, var(--size-col-keys));
    flex-shrink: 0;
    display: flex;
    align-items: flex-start;
    padding: var(--space-1) var(--space-2);
    background: var(--color-gray-3);
    border-right: var(--line-hair) solid var(--color-gray-6);
    white-space: nowrap;
    overflow: hidden;
  }
  .body {
    flex: 1;
    min-width: 0;
    position: relative;
    overflow: hidden;
    touch-action: none;
  }
  .box-frame {
    position: absolute;
    border: var(--line-hair) solid var(--color-accent);
    pointer-events: none;
    z-index: 2;
  }
  .live {
    position: absolute;
    left: 0;
    right: 0;
  }
</style>

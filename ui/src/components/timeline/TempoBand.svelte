<!-- アレンジの「テンポ ・ 拍子の変更」の行の、時間の側（仕様書 3 章 Y17 の 18-C、モック F6-Tempo ・ Opt6-Tempo ・ F6-AutoRec）。
     たたんだとき: 再生の線がいる所のテンポと拍子。ひらいたとき: 上の段に拍子の旗、下の段にテンポの階段の線と点とだんだん。
     線と点は WebGL（アレンジの arrangeLanes の区画の 1 枚。区画はふやさない）、文字と旗は HTML。
     ドラッグ中は画面だけが先に動き、離したときに意図を 1 つ送る。Esc でやめる。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { Command, TimelineRef, TimelineView } from "../../bridge";
  import { glSlot } from "../../gl/slot";
  import { send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import type { TempoMap } from "../../time/tempoMap";
  import { tickToX, type View } from "../arrangeGeom";
  import { tempoText } from "../format";
  import { openContextMenu, type MenuEntry } from "../menu/contextMenu.svelte";
  import { metrics } from "../metrics.svelte";
  import { onDragCancel } from "../pianoroll/view.svelte";
  import { meterText, positionText, stepMeter, stepTempo, type Meter } from "../time";
  import { readMeter, readTempo, type ClampNote } from "../valueClamp";
  import * as T from "./tempoGeom";
  import { pickTimeline } from "./pick";
  import ValueInput from "./ValueInput.svelte";

  interface Props {
    view: View;
    map: TempoMap;
    timeline: TimelineView;
    /** たたんだときに出す、再生の線がいる所のテンポと拍子。 */
    now: string;
    /** 録音中かカウントイン中: えらぶだけ（core もそれしか受けない）。 */
    busy: boolean;
  }
  let { view, map, timeline, now, busy }: Props = $props();

  const laneH = $derived(metrics.px("size.lane"));
  const dims: T.BandDims = $derived({ hair: metrics.px("line.hair"), point: metrics.px("part.cc.point") });
  const still = () => metrics.px("space.1");

  const points = $derived(T.tempoPoints(timeline));
  const scale = $derived(T.tempoScale(points, laneH, metrics.px("space.3"), metrics.scale / 100));
  const flagList = $derived(T.flags(timeline, map));
  const selected = $derived(timeline.selected);

  // ---- テンポの点 ----

  let pointDrag = $state<T.PointDrag | null>(null);
  let hovered = $state(-1);
  const shownPoints = $derived(T.draggedPoints(points, pointDrag));
  const isSelected = (ref: TimelineRef) => T.sameRef(selected, ref);
  const rects = $derived(
    T.tempoRects(
      view,
      shownPoints,
      scale,
      dims,
      (p, i) => ({
        selected: pointDrag?.moved ? p.tick === pointDrag.toTick : isSelected({ kind: "tempoPoint", tick: p.tick }),
        hovered: i === hovered,
      }),
      pointDrag?.moved ? { tick: pointDrag.tick, bpm: pointDrag.fromBpm } : null,
    ),
  );
  const labels = $derived(T.tempoLabels(view, shownPoints, scale, dims.point, dims));
  const ramps = $derived(T.rampSpans(view, shownPoints));

  let lane = $state<HTMLElement>();
  let meterRow = $state<HTMLElement>();
  let band = $state<HTMLElement>();
  const at = (e: MouseEvent, el: HTMLElement | undefined) => {
    const r = el!.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };
  const pointUnder = (p: { x: number; y: number }) => T.pointAt(view, points, scale, p.x, p.y, dims.point);

  /** その場の読み値（位置）。 */
  let reading = $state<{ text: string; x: number; y: number } | null>(null);
  function readAt(e: PointerEvent, tick: number): void {
    const r = band!.getBoundingClientRect();
    reading = { text: positionText(tick, map), x: e.clientX - r.left, y: e.clientY - r.top };
  }
  let readingW = $state(0);
  let readingH = $state(0);
  const readingBox = $derived.by(() => {
    if (!reading) return null;
    const off = metrics.px("part.reading.offset");
    return { left: reading.x - off - readingW, top: reading.y - off - readingH };
  });

  function laneDown(e: PointerEvent): void {
    if (e.button !== 0) return;
    const p = at(e, lane);
    const i = pointUnder(p);
    if (i < 0) {
      if (selected) send({ type: "selectTimeline", target: null });
      return;
    }
    const pt = points[i]!;
    pickTimeline();
    if (!isSelected({ kind: "tempoPoint", tick: pt.tick })) send({ type: "selectTimeline", target: { kind: "tempoPoint", tick: pt.tick } });
    if (busy) return;
    pointDrag = { tick: pt.tick, fromBpm: pt.bpm, toTick: pt.tick, bpm: pt.bpm, downX: p.x, downY: p.y, moved: false };
    lane!.setPointerCapture?.(e.pointerId);
  }
  function laneMove(e: PointerEvent): void {
    const p = at(e, lane);
    const d = pointDrag;
    if (!d) {
      hovered = pointUnder(p);
      return;
    }
    const next = T.dragPoint(d, view, scale, map, p.x, p.y, still(), e.shiftKey);
    if (next !== d) {
      pointDrag = next;
      readAt(e, next.toTick);
    }
  }
  function laneUp(): void {
    const d = pointDrag;
    pointDrag = null;
    reading = null;
    if (!d?.moved || (d.toTick === d.tick && d.bpm === d.fromBpm)) return;
    send({ type: "moveTempoPoint", tick: d.tick, toTick: d.toTick, bpm: d.bpm });
  }
  function laneLeave(): void {
    if (!pointDrag) hovered = -1;
  }

  /** 2 回押す: 点の上なら値を打ちこむ、ほかの所なら、その拍の頭に点を足す。 */
  let editing = $state<{ kind: "tempo"; tick: number } | { kind: "meter"; bar: number } | null>(null);
  function laneDouble(e: MouseEvent): void {
    if (busy) return;
    const p = at(e, lane);
    const i = pointUnder(p);
    if (i >= 0) {
      editing = { kind: "tempo", tick: points[i]!.tick };
      return;
    }
    const add = T.addAt(view, map, p.x, "tempo") as { tick: number; bpm: number };
    send({ type: "addTempoPoint", tick: add.tick, bpm: add.bpm });
  }

  function laneMenu(e: MouseEvent): void {
    e.preventDefault();
    if (pointDrag) return;
    const i = pointUnder(at(e, lane));
    if (i < 0) return;
    const pt = points[i]!;
    const target: TimelineRef = { kind: "tempoPoint", tick: pt.tick };
    pickTimeline();
    if (!isSelected(target)) send({ type: "selectTimeline", target });
    const entries: MenuEntry[] = [
      { kind: "row", label: "menu.tempoRamp", usable: !busy, checked: pt.ramp, run: () => send({ type: "setTempoRamp", tick: pt.tick, on: !pt.ramp }) },
      { kind: "separator" },
      { kind: "row", label: "menu.delete", key: t("key.delete"), usable: !busy && pt.tick !== 0, run: () => send({ type: "deleteTimeline", target }) },
    ];
    openContextMenu(e.clientX, e.clientY, entries);
  }

  // ---- 拍子の旗 ----

  let flagDrag = $state<T.FlagDrag | null>(null);
  function flagDown(e: PointerEvent, f: T.Flag): void {
    if (e.button !== 0) return;
    e.stopPropagation();
    const ref: TimelineRef = { kind: "meterChange", bar: f.bar };
    pickTimeline();
    if (!isSelected(ref)) send({ type: "selectTimeline", target: ref });
    if (busy) return;
    flagDrag = { bar: f.bar, toBar: f.bar, downX: at(e, meterRow).x, moved: false };
    meterRow!.setPointerCapture?.(e.pointerId);
  }
  function rowDown(e: PointerEvent): void {
    if (e.button === 0 && selected) send({ type: "selectTimeline", target: null });
  }
  function rowMove(e: PointerEvent): void {
    const d = flagDrag;
    if (!d) return;
    const next = T.dragFlag(d, view, map, at(e, meterRow).x, still());
    if (next !== d) {
      flagDrag = next;
      if (next.moved) readAt(e, map.tickOfBar(next.toBar));
    }
  }
  function rowUp(): void {
    const d = flagDrag;
    flagDrag = null;
    reading = null;
    if (d?.moved && d.toBar !== d.bar) send({ type: "moveMeterChange", bar: d.bar, toBar: d.toBar });
  }
  function rowDouble(e: MouseEvent): void {
    if (busy) return;
    const add = T.addAt(view, map, at(e, meterRow).x, "meter") as { bar: number; num: number; den: number };
    send({ type: "addMeterChange", bar: add.bar, num: add.num, den: add.den });
  }
  function flagMenu(e: MouseEvent, f: T.Flag): void {
    e.preventDefault();
    e.stopPropagation();
    if (flagDrag) return;
    const target: TimelineRef = { kind: "meterChange", bar: f.bar };
    pickTimeline();
    if (!isSelected(target)) send({ type: "selectTimeline", target });
    // 仕様書 3 章 92-17。曲の頭の旗には「消す」がない（92-2）。
    const entries: MenuEntry[] = [{ kind: "row", label: "menu.typeMeter", usable: !busy, run: () => (editing = { kind: "meter", bar: f.bar }) }];
    if (f.bar !== 0) entries.push({ kind: "separator" }, { kind: "row", label: "menu.delete", key: t("key.delete"), usable: !busy, run: () => send({ type: "deleteTimeline", target }) });
    openContextMenu(e.clientX, e.clientY, entries);
  }

  /** 旗を描く所: ドラッグ中は動いた旗をその小節の頭に、もとの所に gray.7 の枠。 */
  const flagsShown = $derived.by(() => {
    const d = flagDrag?.moved ? flagDrag : null;
    return flagList
      .filter((f) => !d || d.bar === 0 || f.bar !== d.toBar || f.bar === d.bar)
      .map((f) => {
        const tick = d && f.bar === d.bar ? map.tickOfBar(d.toBar) : f.tick;
        return { ...f, x: tickToX(view, tick), moving: d !== null && f.bar === d.bar };
      })
      .filter((f) => f.x > -view.width && f.x < view.width);
  });
  const flagFrom = $derived(flagDrag?.moved && flagDrag.bar !== 0 ? tickToX(view, map.tickOfBar(flagDrag.bar)) : null);
  let movingFlagW = $state(0);

  // ---- 打ちこむ ----

  function commitClamped(command: Command, clamp: ClampNote | null): void {
    send(command);
    if (clamp) send({ type: "clampValue", ...clamp });
  }
  const editPoint = $derived.by(() => {
    const e = editing;
    return e?.kind === "tempo" ? (points.find((p) => p.tick === e.tick) ?? null) : null;
  });
  const editFlag = $derived.by(() => {
    const e = editing;
    return e?.kind === "meter" ? (flagList.find((f) => f.bar === e.bar) ?? null) : null;
  });
  function closeEdit(): void {
    editing = null;
  }

  // ---- やめる ----

  function cancel(): boolean {
    if (!pointDrag && !flagDrag) return false;
    pointDrag = null;
    flagDrag = null;
    reading = null;
    return true;
  }
  onMount(() => onDragCancel(cancel));
</script>

<div class="band" class:open={timeline.open} bind:this={band} data-test="tempo-band">
  {#if timeline.open}
    <div
      class="meter-row"
      role="presentation"
      bind:this={meterRow}
      onpointerdown={rowDown}
      onpointermove={rowMove}
      onpointerup={rowUp}
      onpointercancel={cancel}
      ondblclick={rowDouble}
      oncontextmenu={(e) => e.preventDefault()}
      data-test="meter-row"
    >
      {#if flagFrom !== null}
        <div class="flag-from" style:left="{flagFrom}px" style:width="{movingFlagW}px"></div>
      {/if}
      {#each flagsShown as f (f.bar)}
        {#if f.moving}
          <span
            class="m-num flag selected"
            role="presentation"
            style:left="{f.x}px"
            bind:clientWidth={movingFlagW}
            data-test="meter-flag">{f.num}/{f.den}</span
          >
        {:else}
          <span
            class="m-num flag"
            class:selected={isSelected({ kind: "meterChange", bar: f.bar })}
            role="presentation"
            style:left="{f.x}px"
            onpointerdown={(e) => flagDown(e, f)}
            ondblclick={(e) => {
              e.stopPropagation();
              if (!busy) editing = { kind: "meter", bar: f.bar };
            }}
            oncontextmenu={(e) => flagMenu(e, f)}
            data-test="meter-flag">{f.num}/{f.den}</span
          >
        {/if}
      {/each}
      {#if editFlag}
        <div class="edit" style:left="{tickToX(view, editFlag.tick)}px">
          <ValueInput
            value={meterText(editFlag)}
            parse={readMeter}
            step={(r, d) => meterText(stepMeter(r.value, d))}
            commit={(r: { value: Meter; clamp: ClampNote | null }) =>
              commitClamped({ type: "setMeterChange", bar: editFlag.bar, num: r.value.num, den: r.value.den }, r.clamp)}
            reject={() => send({ type: "rejectValue", field: "meterNum", kept: String(editFlag.num), range: null })}
            close={closeEdit}
            label={t("transport.timeSignature")}
            chars={5}
            test="meter-flag-field"
          />
        </div>
      {/if}
    </div>
    <div
      class="tempo-row"
      role="presentation"
      bind:this={lane}
      use:glSlot={{ kind: "rects", rects }}
      onpointerdown={laneDown}
      onpointermove={laneMove}
      onpointerup={laneUp}
      onpointercancel={cancel}
      onpointerleave={laneLeave}
      ondblclick={laneDouble}
      oncontextmenu={laneMenu}
      data-test="tempo-row"
    >
      {#each ramps as r (r.tick)}
        <span class="ramp m-lbl" style:left="{(r.x0 + r.x1) / 2}px">{t("timeline.ramp")}</span>
      {/each}
      {#each labels as l (l.tick)}
        {@const on = pointDrag?.moved ? l.tick === pointDrag.toTick : isSelected({ kind: "tempoPoint", tick: l.tick })}
        <span class="m-num value" class:selected={on} style:left="{l.x}px" style:top="{l.y}px">{tempoText(l.bpm)}</span>
      {/each}
      {#if editPoint}
        <div class="edit" style:left="{tickToX(view, editPoint.tick) + dims.point}px" style:top="{T.bpmToY(scale, editPoint.bpm)}px">
          <ValueInput
            value={tempoText(editPoint.bpm)}
            parse={readTempo}
            step={(r, d) => tempoText(stepTempo(r.value, d))}
            commit={(r: { value: number; clamp: ClampNote | null }) =>
              commitClamped({ type: "moveTempoPoint", tick: editPoint.tick, toTick: editPoint.tick, bpm: r.value }, r.clamp)}
            reject={() => send({ type: "rejectValue", field: "tempo", kept: tempoText(editPoint.bpm), range: null })}
            close={closeEdit}
            label={t("transport.tempo")}
            chars={6}
            test="tempo-point-field"
          />
        </div>
      {/if}
    </div>
  {:else}
    <span class="m-num m-lbl now" style:left="{-view.scrollX}px" data-test="tempo-now">{now}</span>
  {/if}
  {#if readingBox && reading}
    <div class="m-num reading" style:left="{readingBox.left}px" style:top="{readingBox.top}px" bind:clientWidth={readingW} bind:clientHeight={readingH} data-test="tempo-reading">
      {reading.text}
    </div>
  {/if}
</div>

<style>
  .band {
    position: relative;
    flex-shrink: 0;
    height: var(--size-row);
    background: var(--color-gray-3);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    touch-action: none;
  }
  /* ひらいた帯（たたんで 24、ひらいて 72 = 拍子の段 size.row ＋ テンポの段 size.lane。モック F6-Tempo の地は gray.2）。 */
  .band.open {
    height: auto;
    background: var(--color-gray-2);
  }
  .meter-row {
    position: relative;
    height: var(--size-row);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    box-sizing: border-box;
  }
  .tempo-row {
    position: relative;
    height: var(--size-lane);
  }
  .now {
    position: absolute;
    top: var(--space-1);
    margin-left: var(--space-1);
  }
  .flag {
    position: absolute;
    z-index: 1;
    top: var(--space-1);
    padding-left: var(--space-1);
    border-left: var(--line-mark) solid var(--color-gray-9);
    color: var(--color-gray-10);
    white-space: nowrap;
    cursor: default;
  }
  .flag.selected {
    border-left-color: var(--color-accent);
    background: var(--color-gray-4);
  }
  .flag-from {
    position: absolute;
    z-index: 1;
    top: var(--space-1);
    height: var(--text-s-line-height);
    box-sizing: border-box;
    border: var(--line-hair) solid var(--color-gray-7);
    pointer-events: none;
  }
  .value {
    position: absolute;
    z-index: 1;
    color: var(--color-gray-10);
    white-space: nowrap;
    pointer-events: none;
  }
  .value.selected {
    color: var(--color-accent);
  }
  .ramp {
    position: absolute;
    z-index: 1;
    top: var(--line-mark);
    transform: translateX(-50%);
    pointer-events: none;
  }
  .edit {
    position: absolute;
    z-index: 5;
    top: 0;
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

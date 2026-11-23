<!-- アレンジのマーカーの行（仕様書 2 章「マーカーの名札」、3 章 Y18、モック R10-Markers ・ F6-Tempo）。
     名札は左に line.mark の線と名前。えらんだ名札は線が accent で地が gray.4。押すとその位置へ、2 回押すと名前の枠、
     押したまま横へ動かすと拍に吸いつく（離して 1 つ送る。もとの所に gray.7 の枠、その場の読み値は位置。Esc でやめる）。 -->
<script lang="ts">
  import { onMount, tick as nextTick } from "svelte";
  import type { MarkerView, TimelineView } from "../../bridge";
  import { send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { nearestBeatTick } from "../../time/bars";
  import type { TempoMap } from "../../time/tempoMap";
  import { tickToX, xToTick, type View } from "../arrangeGeom";
  import { openContextMenu } from "../menu/contextMenu.svelte";
  import { metrics } from "../metrics.svelte";
  import { onDragCancel } from "../pianoroll/view.svelte";
  import { positionText } from "../time";
  import { pickTimeline } from "./pick";

  interface Props {
    view: View;
    map: TempoMap;
    timeline: TimelineView;
    /** 録音中かカウントイン中: 押すと、えらんでそこへ行くだけ（core もそれしか受けない）。 */
    busy: boolean;
  }
  let { view, map, timeline, busy }: Props = $props();

  const selectedId = $derived(timeline.selected?.kind === "marker" ? timeline.selected.id : null);

  let row = $state<HTMLElement>();
  const xOf = (e: MouseEvent) => e.clientX - row!.getBoundingClientRect().left;

  interface Drag {
    id: number;
    tick: number;
    toTick: number;
    downX: number;
    moved: boolean;
    width: number;
  }
  let drag = $state<Drag | null>(null);
  let reading = $state<{ x: number; y: number } | null>(null);
  let readingW = $state(0);
  let readingH = $state(0);

  const tags = $derived(
    timeline.markers
      .map((m) => ({ ...m, x: tickToX(view, drag?.moved && drag.id === m.id ? drag.toTick : m.tick) }))
      .filter((m) => m.x > -view.width && m.x < view.width),
  );

  function down(e: PointerEvent, m: MarkerView): void {
    if (e.button !== 0) return;
    e.stopPropagation();
    pickTimeline();
    const el = e.currentTarget as HTMLElement;
    drag = { id: m.id, tick: m.tick, toTick: m.tick, downX: xOf(e), moved: false, width: el.offsetWidth };
    row!.setPointerCapture?.(e.pointerId);
  }
  function move(e: PointerEvent): void {
    const d = drag;
    if (!d) return;
    const x = xOf(e);
    const moved = !busy && (d.moved || Math.abs(x - d.downX) >= metrics.px("space.1"));
    if (!moved) return;
    const at = d.tick + (x - d.downX) / view.pxPerTick;
    const toTick = e.shiftKey ? Math.max(0, Math.round(at)) : nearestBeatTick(map, at);
    if (moved !== d.moved || toTick !== d.toTick) drag = { ...d, moved, toTick };
    const r = row!.getBoundingClientRect();
    reading = { x: e.clientX - r.left, y: e.clientY - r.top };
  }
  function up(): void {
    const d = drag;
    drag = null;
    reading = null;
    if (!d) return;
    if (!d.moved) send({ type: "goToMarker", id: d.id });
    else if (d.toTick !== d.tick) send({ type: "moveMarker", id: d.id, tick: d.toTick });
  }
  function cancel(): boolean {
    if (!drag) return false;
    drag = null;
    reading = null;
    return true;
  }
  onMount(() => onDragCancel(cancel));

  function rowDown(e: PointerEvent): void {
    if (e.button === 0 && timeline.selected?.kind === "marker") send({ type: "selectTimeline", target: null });
  }

  // An empty place of the row (spec 3 Y18, round 10): a marker there, named as K names it. Rust snaps it to the beat.
  const tickAt = (e: MouseEvent) => Math.max(0, Math.round(xToTick(view, xOf(e))));
  function addAt(tick: number): void {
    pickTimeline();
    send({ type: "addMarker", name: t("marker.newName", { count: timeline.markers.length + 1 }), tick });
  }
  function rowDouble(e: MouseEvent): void {
    if (e.button === 0 && !busy && !drag) addAt(tickAt(e));
  }
  function rowMenu(e: MouseEvent): void {
    e.preventDefault();
    if (drag || e.target instanceof HTMLInputElement) return;
    const tick = tickAt(e);
    openContextMenu(e.clientX, e.clientY, [{ kind: "row", label: "menu.addMarkerHere", usable: !busy, run: () => addAt(tick) }]);
  }

  function menu(e: MouseEvent, m: MarkerView): void {
    e.preventDefault();
    e.stopPropagation();
    if (drag) return;
    pickTimeline();
    if (selectedId !== m.id) send({ type: "selectTimeline", target: { kind: "marker", id: m.id } });
    openContextMenu(e.clientX, e.clientY, [
      { kind: "row", label: "menu.loopFromMarker", usable: !busy, run: () => send({ type: "loopFromMarker", id: m.id }) },
      { kind: "row", label: "menu.renameMarker", key: t("key.renameTrack"), usable: !busy, run: () => send({ type: "openMarkerName", id: m.id }) },
      { kind: "separator" },
      { kind: "row", label: "menu.delete", key: t("key.delete"), usable: !busy, run: () => send({ type: "deleteTimeline", target: { kind: "marker", id: m.id } }) },
    ]);
  }

  // ---- 名前の枠（core が持つ renaming。打っている途中の文字は画面だけの物） ----

  let draft = $state("");
  let field = $state<HTMLInputElement>();
  let sizerW = $state(0);
  let openFor: number | null = null;
  $effect(() => {
    const id = timeline.renaming;
    if (id === openFor) return;
    openFor = id;
    const m = timeline.markers.find((x) => x.id === id);
    if (!m) return;
    draft = m.name;
    void nextTick().then(() => {
      field?.focus();
      field?.select();
    });
  });
  function fieldKey(e: KeyboardEvent, id: number): void {
    if (e.isComposing) return;
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      send({ type: "renameMarker", id, name: draft });
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      send({ type: "closeMarkerName" });
    }
  }
  function fieldBlur(id: number): void {
    if (timeline.renaming === id) send({ type: "closeMarkerName" });
  }

  const readingBox = $derived.by(() => {
    if (!reading || !drag?.moved) return null;
    const off = metrics.px("part.reading.offset");
    return { left: reading.x - off - readingW, top: reading.y - off - readingH };
  });
</script>

<div
  class="markers"
  role="presentation"
  bind:this={row}
  onpointerdown={rowDown}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={cancel}
  ondblclick={rowDouble}
  oncontextmenu={rowMenu}
  data-test="marker-row"
>
  {#if drag?.moved}
    <div class="from" style:left="{tickToX(view, drag.tick)}px" style:width="{drag.width}px"></div>
  {/if}
  {#each tags as m (m.id)}
    {#if timeline.renaming === m.id}
      <div class="naming" style:left="{m.x}px">
        <span class="sizer" aria-hidden="true" bind:clientWidth={sizerW}>{draft}</span>
        <input
          class="name-field"
          type="text"
          spellcheck="false"
          autocomplete="off"
          aria-label={t("arrange.marker")}
          data-test="marker-name-field"
          style:width="{sizerW}px"
          bind:this={field}
          bind:value={draft}
          onpointerdown={(e) => e.stopPropagation()}
          ondblclick={(e) => e.stopPropagation()}
          onkeydown={(e) => fieldKey(e, m.id)}
          onblur={() => fieldBlur(m.id)}
        />
      </div>
    {:else}
      <span
        class="tag"
        class:selected={selectedId === m.id || drag?.id === m.id}
        role="presentation"
        style:left="{m.x}px"
        onpointerdown={(e) => down(e, m)}
        ondblclick={(e) => {
          e.stopPropagation();
          if (!busy) send({ type: "openMarkerName", id: m.id });
        }}
        oncontextmenu={(e) => menu(e, m)}
        data-test="marker-tag">{m.name}</span
      >
    {/if}
  {/each}
  {#if readingBox && drag}
    <div class="m-num reading" style:left="{readingBox.left}px" style:top="{readingBox.top}px" bind:clientWidth={readingW} bind:clientHeight={readingH} data-test="marker-reading">
      {positionText(drag.toTick, map)}
    </div>
  {/if}
</div>

<style>
  .markers {
    position: relative;
    height: var(--size-row);
    flex-shrink: 0;
    background: var(--color-gray-3);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    touch-action: none;
  }
  .tag {
    position: absolute;
    z-index: 2;
    top: var(--space-1);
    padding: 0 var(--space-1);
    border-left: var(--line-mark) solid var(--color-gray-9);
    color: var(--color-gray-10);
    white-space: nowrap;
    cursor: default;
  }
  .tag.selected {
    border-left-color: var(--color-accent);
    background: var(--color-gray-4);
  }
  .from {
    position: absolute;
    z-index: 1;
    top: var(--space-1);
    height: var(--text-s-line-height);
    box-sizing: border-box;
    border: var(--line-hair) solid var(--color-gray-7);
    pointer-events: none;
  }
  /* 名前の枠（トラックの名前の枠と同じ: 地 gray.0、枠 gray.10、太さ 600）。左の線は accent。 */
  .naming {
    position: absolute;
    z-index: 3;
    top: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    border-left: var(--line-mark) solid var(--color-accent);
  }
  .sizer {
    position: absolute;
    visibility: hidden;
    white-space: pre;
    padding: 0 var(--space-1);
    font-weight: var(--text-s-weight-600);
    border: var(--line-hair) solid transparent;
  }
  .name-field {
    box-sizing: content-box;
    min-width: var(--space-6);
    height: var(--part-toggle-size);
    padding: 0 var(--space-1);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-10);
    border-radius: var(--radius-1);
    color: var(--color-gray-10);
    font: inherit;
    font-weight: var(--text-s-weight-600);
    outline: none;
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

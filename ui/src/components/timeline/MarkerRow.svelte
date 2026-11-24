<!-- The marker row of the arrange (spec 2 "Marker tag", spec 3 (Y18), mock R10-Markers, F6-Tempo).
     A tag has a line.mark line on the left and the name. A chosen tag has an accent line and a gray.4 background. Pressing goes to its position, pressing twice opens the name box,
     and holding and moving sideways snaps to beats (one intent on release; a gray.7 border at the original place, the reading in place is the position; Esc cancels). -->
<script lang="ts">
  import { onDestroy, onMount, tick as nextTick } from "svelte";
  import { INSERT_SPACE_BARS_DEFAULT, INSERT_SPACE_BARS_MAX, type MarkerView, type TimelineView } from "../../bridge";
  import { send } from "../../state/app.svelte";
  import { t } from "../../text";
  import { nearestBeatTick } from "../../time/bars";
  import type { TempoMap } from "../../time/tempoMap";
  import { tickToX, xToTick, type View } from "../arrangeGeom";
  import { openContextMenu } from "../menu/contextMenu.svelte";
  import { metrics } from "../metrics.svelte";
  import { dragReading, onDragCancel } from "../pianoroll/view.svelte";
  import { positionText } from "../time";
  import { readBars } from "../valueClamp";
  import { pickTimeline } from "./pick";
  import { closeSpaceBox, openSpaceBox, spaceBox } from "./spaceBox.svelte";

  interface Props {
    view: View;
    map: TempoMap;
    timeline: TimelineView;
    /** While recording or during count-in: pressing only chooses and goes there (the core accepts only that). */
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
    // The rows of a whole section (round 12 A2): only when both ends of the section are heads of bars.
    const section = !busy && m.section === "usable";
    const hint = busy || m.section === "usable" ? undefined : t(m.section === "empty" ? "timeline.sectionEmpty" : "timeline.sectionNotOnBars");
    openContextMenu(e.clientX, e.clientY, [
      { kind: "row", label: "menu.loopFromMarker", usable: !busy, run: () => send({ type: "loopFromMarker", id: m.id }) },
      { kind: "row", label: "menu.duplicateSection", usable: section, hint, run: () => send({ type: "duplicateSection", marker: m.id }) },
      { kind: "row", label: "menu.deleteSection", usable: section, hint, run: () => send({ type: "deleteSection", marker: m.id }) },
      { kind: "row", label: "menu.insertSpace", usable: section, hint, run: () => openSpaceBox(m.id) },
      { kind: "separator" },
      { kind: "row", label: "menu.renameMarker", key: t("key.renameTrack"), usable: !busy, run: () => send({ type: "openMarkerName", id: m.id }) },
      { kind: "separator" },
      { kind: "row", label: "menu.delete", key: t("key.delete"), usable: !busy, run: () => send({ type: "deleteTimeline", target: { kind: "marker", id: m.id } }) },
    ]);
  }

  // ---- The name box (the core keeps renaming; the text being typed is only on the screen) ----

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

  // ---- The box of 「この前に空きを入れる…」 (round 12 A2): digits at the marker, the look of the repeat box ----

  let spaceW = $state(0);
  const spacing = $derived.by(() => {
    const m = timeline.markers.find((x) => x.id === spaceBox.marker);
    if (!m || m.section !== "usable") return null;
    return { id: m.id, x: Math.min(Math.max(tickToX(view, m.tick), 0), Math.max(0, view.width - spaceW)) };
  });
  let spaceDraft = $state("");
  let spaceField = $state<HTMLInputElement>();
  let spaceFor: number | null = null;
  const spaceRead = $derived(readBars(spaceDraft, INSERT_SPACE_BARS_MAX));
  $effect(() => {
    const id = spaceBox.marker;
    if (id === spaceFor) return;
    spaceFor = id;
    if (id === null) return;
    spaceDraft = String(INSERT_SPACE_BARS_DEFAULT);
    void nextTick().then(() => {
      spaceField?.focus();
      spaceField?.select();
    });
  });
  // Recording starting, or the marker or its section going away, closes the box.
  $effect(() => {
    if (spaceBox.marker !== null && (busy || !spacing)) closeSpaceBox();
  });
  // While the box is open, the status line says what Enter does.
  $effect(() => {
    if (!spacing) return;
    dragReading.text = spaceRead === null ? null : t("timeline.readInsertSpace", { count: spaceRead.value });
    return () => {
      dragReading.text = null;
    };
  });
  onDestroy(closeSpaceBox);
  function spaceKey(e: KeyboardEvent, id: number): void {
    if (e.isComposing) return;
    if (e.key === "Enter") {
      // A box that is not a whole number stays open.
      if (spaceRead !== null) {
        closeSpaceBox();
        send({ type: "insertSpace", marker: id, bars: spaceRead.value });
        if (spaceRead.clamp) send({ type: "clampValue", ...spaceRead.clamp });
      }
    } else if (e.key === "Escape") {
      closeSpaceBox();
    } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      const by = (e.key === "ArrowUp" ? 1 : -1) * (e.shiftKey ? 10 : 1);
      spaceDraft = String(Math.min(INSERT_SPACE_BARS_MAX, Math.max(1, (spaceRead?.value ?? INSERT_SPACE_BARS_DEFAULT) + by)));
    } else {
      return;
    }
    e.preventDefault();
    e.stopPropagation();
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
        class:m-typable={!busy}
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
  {#if spacing}
    {@const id = spacing.id}
    <div class="space-box" role="presentation" style:left="{spacing.x}px" bind:clientWidth={spaceW} onpointerdown={(e) => e.stopPropagation()} ondblclick={(e) => e.stopPropagation()}>
      <input
        class="space-field"
        class:broken={spaceRead === null}
        type="text"
        inputmode="numeric"
        spellcheck="false"
        autocomplete="off"
        aria-label={t("timeline.insertSpaceBars")}
        aria-invalid={spaceRead === null}
        data-test="insert-space-field"
        bind:this={spaceField}
        bind:value={spaceDraft}
        onkeydown={(e) => spaceKey(e, id)}
        onblur={closeSpaceBox}
      />
    </div>
  {/if}
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
    border-bottom: var(--line-hair) solid var(--color-line-thin);
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
    border-left-color: var(--color-accent-ink);
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
  /* The name box (the same as the track name box: background gray.0, border gray.10, weight 600). The left line is accent. */
  .naming {
    position: absolute;
    z-index: 3;
    top: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    border-left: var(--line-mark) solid var(--color-accent-ink);
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
  /* The box of 「この前に空きを入れる…」: the look of the repeat box of the arrange, two digits wide. */
  .space-box {
    position: absolute;
    z-index: 4;
    top: 0;
    bottom: 0;
    display: flex;
    align-items: center;
  }
  .space-field {
    box-sizing: border-box;
    width: calc(2ch + 2 * var(--space-1) + 2 * var(--line-hair));
    height: var(--part-toggle-size);
    padding: 0 var(--space-1);
    border: var(--line-hair) solid var(--color-gray-10);
    border-radius: var(--radius-1);
    background: var(--color-gray-0);
    color: var(--color-gray-10);
    font: inherit;
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    font-weight: var(--text-s-weight-600);
    outline: none;
  }
  .space-field.broken {
    border-color: var(--color-rec);
    color: var(--color-rec);
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

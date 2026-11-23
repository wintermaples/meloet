<!-- The parameter list of a plugin (spec 3, the 「パラメータ」 rule; mocks F-ParamFew, F-ParamMany, F-ParamSearch, F-ParamNoWindow).
     ParamTableBook fetches the names and groups only once; changed values come in paramValues. Rows and headings are 24 high,
     and only the part in view is made (even with 2855 rows). Only the screen keeps which groups are folded. This part lives
     as long as one plugin in one slot; if a new table (after an undo, ...) has the same groups in order, the folding and the
     scroll stay. Until the new table comes, the old one is shown (an empty list would send the scroll back to the top). -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { paramTable, type ParamTable, type SelectedPlugin } from "../../bridge";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import Button from "../Button.svelte";
  import { openContextMenu } from "../menu/contextMenu.svelte";
  import { metrics } from "../metrics.svelte";
  import { paramUi } from "./paramUi.svelte";
  import PluginParamRow from "./PluginParamRow.svelte";
  import {
    foldedAtFirst,
    hiddenKnobs,
    matchCount,
    pluginEntries,
    sameGroups,
    stepLabels,
    stepOf,
    stepPluginRow,
    tableCount,
    valueOfStep,
    type PluginEntry,
  } from "./pluginRows";

  let { sel, plugin, usable }: { sel: SelectedPlugin; plugin: string; usable: boolean } = $props();

  let table = $state.raw<ParamTable | null>(null);
  /** Goes up each time values come (the rows read the values in the table again). */
  let tick = $state(0);
  let folded = $state.raw<Set<string>>(new Set());
  let expanded = $state.raw<Set<string>>(new Set());

  onMount(() => {
    const book = paramTable();
    const take = () => {
      const now = book.table();
      if (now && now !== table) {
        const kept = table !== null && sameGroups(table, now);
        table = now;
        if (!kept) {
          folded = foldedAtFirst(now);
          expanded = new Set();
        }
      }
      tick += 1;
    };
    take();
    return book.listen(take);
  });

  // Groups opened with 「… ほか N」 are reset when the search text changes.
  $effect(() => {
    void paramUi.query;
    untrack(() => (expanded = new Set()));
  });

  const entries = $derived(
    table ? pluginEntries(table, sel.automated, { query: paramUi.query, folded, expanded }) : [],
  );
  const total = $derived(table ? tableCount(table) : sel.count);
  $effect(() => {
    paramUi.matches = table ? matchCount(table, paramUi.query) : 0;
  });
  const searching = $derived(paramUi.query.trim() !== "");
  const knobs = $derived(table ? hiddenKnobs(table) : null);

  // ---- Make only the part in view (every row is 24 high) ----

  let viewport = $state<HTMLDivElement>();
  let scrollTop = $state(0);
  let viewH = $state(0);
  const rowPx = $derived(metrics.px("part.param.height"));
  const OVERSCAN = 8;
  const range = $derived.by(() => {
    if (viewH <= 0) return { from: 0, to: Math.min(entries.length, 64) };
    const first = Math.floor(scrollTop / rowPx);
    const last = Math.ceil((scrollTop + viewH) / rowPx);
    return { from: Math.max(0, first - OVERSCAN), to: Math.min(entries.length, last + OVERSCAN) };
  });
  const visible = $derived(entries.slice(range.from, range.to));

  $effect(() => {
    const v = viewport;
    if (!v) return;
    const read = () => {
      scrollTop = v.scrollTop;
      viewH = v.clientHeight;
    };
    read();
    v.addEventListener("scroll", read, { passive: true });
    const observer = new ResizeObserver(read);
    observer.observe(v);
    return () => {
      v.removeEventListener("scroll", read);
      observer.disconnect();
    };
  });

  /** Keep the chosen row in view (on ↑ ↓). */
  function reveal(id: number): void {
    const at = entries.findIndex((e) => e.kind === "row" && e.row.id === id);
    const v = viewport;
    if (!v || at < 0) return;
    const top = at * rowPx;
    if (top < v.scrollTop) v.scrollTop = top;
    else if (top + rowPx > v.scrollTop + v.clientHeight) v.scrollTop = top + rowPx - v.clientHeight;
  }

  let list = $state<HTMLElement>();
  function pick(id: number): void {
    if (list && !list.contains(document.activeElement)) list.focus({ preventScroll: true });
    if (sel.row !== id) send({ type: "selectParam", param: id });
  }
  function toggle(group: string): void {
    const next = new Set(folded);
    if (!next.delete(group)) next.add(group);
    folded = next;
  }
  function more(group: string): void {
    expanded = new Set([...expanded, group]);
  }

  function enter(): void {
    const id = sel.row;
    const row = id === null ? undefined : paramTable().row(id);
    if (!row || !usable || !table) return;
    const kind = row.kind;
    if (kind.kind === "smooth") paramUi.typing = row.id;
    else if (kind.kind === "toggle") send({ type: "setPluginParam", slot: sel.slot, table: table.table, param: row.id, value: (row.value ?? 0) >= 0.5 ? 0 : 1, commit: true });
    else {
      const el = list?.querySelector<HTMLElement>(`[data-param="${row.id}"]`);
      const box = el?.getBoundingClientRect();
      const at = row.value === null ? null : stepOf(kind, row.value);
      const tableId = table.table;
      openContextMenu(
        box?.right ?? 0,
        box?.bottom ?? 0,
        stepLabels(kind).map((text, i) => ({
          kind: "row" as const,
          label: "params.search" as const,
          text,
          checked: i === at,
          usable: true,
          run: () => send({ type: "setPluginParam", slot: sel.slot, table: tableId, param: row.id, value: valueOfStep(kind, i), commit: true }),
        })),
      );
    }
  }

  function onkeydown(ev: KeyboardEvent): void {
    if (ev.ctrlKey || ev.metaKey || ev.altKey || ev.shiftKey || ev.isComposing) return;
    if (ev.key === "Enter") {
      ev.preventDefault();
      ev.stopPropagation();
      enter();
      return;
    }
    if (ev.key !== "ArrowUp" && ev.key !== "ArrowDown") return;
    ev.preventDefault();
    ev.stopPropagation();
    const next = stepPluginRow(entries, sel.row, ev.key === "ArrowDown" ? 1 : -1);
    if (next !== null) {
      pick(next);
      reveal(next);
    }
  }

  const automated = $derived(new Set(sel.automated));

  // Bands go only under tracks (the marks of return and master inserts are disabled; docs/history/design-notes.ja.md A-19, item 7).
  const markTrack = $derived.by((): number | null => {
    const s = sel.slot;
    if (s.kind === "instrument") return s.track;
    return s.insert.strip.kind === "track" ? s.insert.strip.id : null;
  });
  const busy = $derived(current.state!.transport.state === "recording" || current.state!.transport.state === "countIn");
  /** Parameters with a band (the mark is accent even before core's `automated` counts the band). */
  const laneParams = $derived.by(() => {
    const s = sel.slot;
    const track = current.state!.song.tracks.find((tr) => tr.id === markTrack);
    const out = new Set<number>();
    if (!track) return out;
    const id = s.kind === "insert" ? track.slots[s.insert.slot]?.id : undefined;
    for (const l of track.lanes) {
      if (s.kind === "instrument" && l.target.kind === "instrument") out.add(l.target.param);
      if (s.kind === "insert" && l.target.kind === "insert" && l.target.insert === id) out.add(l.target.param);
    }
    return out;
  });
  function addLane(param: number): void {
    const s = sel.slot;
    const track = markTrack;
    if (track === null) return;
    if (s.kind === "instrument") {
      send({ type: "addLane", track, target: { kind: "instrument", param } });
      return;
    }
    const id = current.state!.song.tracks.find((tr) => tr.id === track)?.slots[s.insert.slot]?.id;
    if (id !== undefined) send({ type: "addLane", track, target: { kind: "insert", insert: id, param } });
  }
  const key = (e: PluginEntry) => e.id;
</script>

<ScrollArea bind:viewport data-test="params-list">
  <div class="list" role="presentation" tabindex="-1" bind:this={list} {onkeydown}>
    <div style:height="{range.from * rowPx}px"></div>
    {#each visible as e (key(e))}
      {#if e.kind === "heading"}
        <button class="m-plain group m-focus-in" onclick={() => toggle(e.group)} data-group={e.group} data-test="params-group">
          <span class={e.folded ? "m-lbl" : "m-hi"}>{e.folded ? "▸" : "▾"}</span>
          <span class="m-hi group-name">{e.name}</span>
          <span class="m-num m-lbl">{e.count}</span>
        </button>
      {:else if e.kind === "more"}
        <button class="m-plain more m-lbl" onclick={() => more(e.group)} data-test="params-more">{t("params.more", { count: e.count })}</button>
      {:else}
        <PluginParamRow
          slot={sel.slot}
          table={sel.table!}
          {plugin}
          row={e.row}
          {tick}
          selected={sel.row === e.row.id}
          automated={automated.has(e.row.id) || laneParams.has(e.row.id)}
          {usable}
          onselect={() => pick(e.row.id)}
          markUsable={markTrack !== null && !busy}
          add={() => addLane(e.row.id)}
        />
      {/if}
    {/each}
    <div style:height="{(entries.length - range.to) * rowPx}px"></div>
    {#if table && searching && entries.length === 0}
      <div class="none" data-test="params-none">
        <span>{t("msg.searchNoMatch", { query: paramUi.query.trim() })}</span>
        <Button onclick={() => (paramUi.query = "")} test="params-clear">{t("button.clearSearch")}</Button>
      </div>
    {:else if table && searching}
      <div class="hint m-lbl" data-test="params-hint">
        <span>{t("params.searchHint")}</span><span class="m-num">{t("key.esc")}</span>
      </div>
    {:else if table && total > 40}
      <div class="hint m-lbl" data-test="params-hint"><span>{t("params.foldedHint")}</span></div>
    {/if}
    {#if knobs && !searching}
      <div class="hint knobs m-lbl" data-test="params-knobs"><span>{t("params.keyboardKnobsHidden", knobs)}</span></div>
    {/if}
  </div>
</ScrollArea>

<style>
  .list {
    outline: none;
  }
  .group,
  .more {
    width: 100%;
    height: var(--part-paramgroup-height);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    white-space: nowrap;
  }
  .group {
    background: var(--color-gray-4);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  .group:hover,
  .more:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .more {
    padding-left: calc(var(--space-3) + var(--space-4));
    border-bottom: var(--line-hair) solid var(--color-line-row);
  }
  .group-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .knobs {
    white-space: normal;
  }
  .none {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    white-space: normal;
  }
  .hint {
    display: flex;
    justify-content: space-between;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
  }
</style>

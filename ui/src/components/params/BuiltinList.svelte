<!-- The parameter list of a built-in effect (mocks F4-Mixer, R6-Delay). Those with a heading are in groups; only rows left by the search box.
     Above it, 「プリセット ▾」 (round 12 M1): the name of the preset the values are, on in the accent, or 「プリセット ▾」; it opens PresetMenu.
     ↑ ↓ choose a row. Enter: the typing box for a smooth value, the list of steps for a stepped value, a toggle for on/off (spec 3 (86-6)). -->
<script lang="ts">
  import { untrack } from "svelte";
  import type { SelectedInsert } from "../../bridge";
  import { EFFECT_PARAMS } from "../../bridge/types";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { send } from "../../state/app.svelte";
  import { t } from "../../text";
  import Button from "../Button.svelte";
  import { openContextMenu } from "../menu/contextMenu.svelte";
  import ParamRow from "./ParamRow.svelte";
  import PresetMenu from "./PresetMenu.svelte";
  import { arrowToChosenRow } from "./rowKeys";
  import { presetName } from "./presetName";
  import { allRows, rowCount, searchRows, stepRow, valueText } from "./paramRows";
  import { paramUi } from "./paramUi.svelte";
  import { current } from "../../state/app.svelte";
  import type { AutoTarget } from "../../bridge";
  import { livePosition } from "../automation/autoState.svelte";
  import { laneMoves, laneOf, paramAt } from "../automation/laneValue";

  let { sel }: { sel: SelectedInsert } = $props();

  let folded = $state<string[]>([]);
  let presetsOpen = $state(false);
  const insertKey = $derived(`${JSON.stringify(sel.insert)}:${sel.effect}`);
  $effect(() => {
    void insertKey;
    untrack(() => {
      folded = [];
      presetsOpen = false;
    });
  });
  const presetText = $derived(sel.preset === null ? t("preset.button") : t("preset.named", { name: presetName(sel.effect, sel.preset) }));

  const groups = $derived(allRows(sel.effect, sel.values));
  const found = $derived(searchRows(groups, paramUi.query));
  const shownCount = $derived(rowCount(found));
  $effect(() => {
    paramUi.matches = shownCount;
  });
  const visibleRows = $derived(found.flatMap((g) => (g.group !== null && folded.includes(g.group) ? [] : g.rows)));

  let list = $state<HTMLElement>();

  // ---- Automation (bands go only under tracks; the marks of returns and the master are disabled) ----

  const track = $derived.by(() => {
    const strip = sel.insert.strip;
    return strip.kind === "track" ? (current.state!.song.tracks.find((tr) => tr.id === strip.id) ?? null) : null;
  });
  const insertId = $derived(track?.slots[sel.insert.slot]?.id ?? null);
  const busy = $derived(current.state!.transport.state === "recording" || current.state!.transport.state === "countIn");
  const targetOf = (param: number): AutoTarget | null => (insertId === null ? null : { kind: "insert", insert: insertId, param });
  function markOf(param: number) {
    const target = targetOf(param);
    const tr = track;
    return {
      on: tr !== null && target !== null && laneOf(tr, target) !== undefined,
      usable: tr !== null && target !== null && !busy,
      add: () => tr && target && send({ type: "addLane", home: { kind: "track", track: tr.id }, target }),
    };
  }
  /** The value the band moves (the bar position in the params panel → value). */
  function autoValue(param: number): number | null {
    const target = targetOf(param);
    const tr = track;
    if (!tr || !target || !laneMoves(laneOf(tr, target), current.state!.automation.held, tr.id)) return null;
    const pos = livePosition(tr.id, target);
    const spec = EFFECT_PARAMS[sel.effect][param];
    return pos === null || !spec ? null : paramAt(spec, pos);
  }

  function pick(param: number): void {
    if (list && !list.contains(document.activeElement)) list.focus({ preventScroll: true });
    if (sel.row !== param) send({ type: "selectParam", param });
  }
  function toggleGroup(group: string): void {
    folded = folded.includes(group) ? folded.filter((g) => g !== group) : [...folded, group];
  }

  function enter(): void {
    const row = visibleRows.find((r) => sel.row !== null && r.covers.includes(sel.row));
    if (!row) return;
    const spec = EFFECT_PARAMS[sel.effect][row.param]!;
    const value = sel.values[row.param] ?? spec.default;
    if (spec.shape === "continuous") paramUi.typing = row.param;
    else if (spec.shape === "switch") send({ type: "setParam", insert: sel.insert, effect: sel.effect, param: row.param, value: value >= 0.5 ? 0 : 1, commit: true });
    else {
      const el = list?.querySelector<HTMLElement>(`[data-param="${row.param}"]`);
      const box = el?.getBoundingClientRect();
      openContextMenu(
        box?.right ?? 0,
        box?.bottom ?? 0,
        spec.steps.map((_, i) => ({
          kind: "row" as const,
          label: "params.search" as const,
          text: valueText(sel.effect, spec, i),
          checked: i === Math.round(value),
          usable: true,
          run: () => send({ type: "setParam", insert: sel.insert, effect: sel.effect, param: row.param, value: i, commit: true }),
        })),
      );
    }
  }

  // Spec 4 "In the parameters: choose the row above / below" (↑ ↓), "move the value" (← →) and "type a value" (Enter). Only while the focus is in the panel.
  function onkeydown(ev: KeyboardEvent): void {
    if (arrowToChosenRow(ev, list, visibleRows.find((r) => sel.row !== null && r.covers.includes(sel.row))?.param ?? null)) return;
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
    const next = stepRow(visibleRows, sel.row, ev.key === "ArrowDown" ? 1 : -1);
    if (next === null) return;
    pick(next);
    list?.querySelector(`[data-param="${next}"]`)?.scrollIntoView?.({ block: "nearest" });
  }
</script>

<div class="presets">
  <span class="m-spacer"></span>
  <Button on={sel.preset !== null} onclick={() => (presetsOpen = !presetsOpen)} test="preset-button">{presetText}</Button>
</div>
<ScrollArea data-test="params-list">
  <div class="list" role="presentation" tabindex="-1" bind:this={list} {onkeydown}>
    {#if presetsOpen}
      <PresetMenu {sel} onclose={() => (presetsOpen = false)} />
    {/if}
    {#each found as g, gi (g.group ?? `-${gi}`)}
      {#if g.group !== null}
        {@const isFolded = folded.includes(g.group)}
        <button class="m-plain group m-focus-in" onclick={() => toggleGroup(g.group!)} data-group={g.group}>
          <span class={isFolded ? "m-lbl" : "m-hi"}>{isFolded ? "▸" : "▾"}</span>
          <span class="m-hi group-name">{g.name}</span>
          <span class="m-num m-lbl">{g.rows.length}</span>
        </button>
      {/if}
      {#if g.group === null || !folded.includes(g.group)}
        {#each g.rows as r (r.param)}
          <ParamRow
            insert={sel.insert}
            effect={sel.effect}
            param={r.param}
            name={r.name}
            value={sel.values[r.param] ?? 0}
            selected={sel.row !== null && r.covers.includes(sel.row)}
            onselect={() => pick(r.param)}
            mark={markOf(r.param)}
            auto={autoValue(r.param)}
          />
        {/each}
      {/if}
    {/each}
    {#if shownCount === 0 && paramUi.query.trim() !== ""}
      <div class="none" data-test="params-none">
        <span>{t("msg.searchNoMatch", { query: paramUi.query.trim() })}</span>
        <Button onclick={() => (paramUi.query = "")} test="params-clear">{t("button.clearSearch")}</Button>
      </div>
    {/if}
  </div>
</ScrollArea>

<style>
  .list {
    outline: none;
  }
  /* 「プリセット ▾」 (round 12 M1), on the right of a row above the list. */
  .presets {
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  .group {
    width: 100%;
    height: var(--part-paramgroup-height);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    white-space: nowrap;
  }
  .group:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .group-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .none {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    white-space: normal;
  }
</style>

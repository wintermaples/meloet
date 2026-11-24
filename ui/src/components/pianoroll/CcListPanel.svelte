<!-- The content of the open 「CC の一覧」 (spec 3 "Rules added in version 5 (after round 3)", mock R5-Lists). The heading (「▾ CC の一覧 17」) is drawn by the band part (PianoRoll.svelte).
     A row of column headings (position, value), and per kind a heading (pressing folds it) and rows of that kind's points (in position order). Headings and points are rows of the same height;
     as in the note list, only the visible rows are made, and they scroll vertically with the same scroll bar.
     Pressing a row selects that point (Shift adds; selectControls). Pressing a field twice, or Enter on a row, turns that field into an 「入力の枠」 (the same typing as the note list).
     Only position and value can be typed. Deciding sends one moveControls. -->
<script lang="ts">
  import { mapOf, meterAtOf } from "../../time/bars";
  import { tick, untrack } from "svelte";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text";
  import { metrics } from "../metrics.svelte";
  import {
    CC_FIELD_ORDER,
    ccGroups,
    ccLines,
    ccRow,
    commitControl,
    controlFieldSpec,
    neighborPoint,
    nextCcField,
    type CcFieldName,
    type CcGroup,
    type CcRow,
  } from "./ccFields";
  import NoteFieldInput from "./NoteFieldInput.svelte";
  import { clampCommand, rejectCommand, type FieldRead } from "./noteFields";
  import { inRanges } from "./notes";
  import { shown } from "./shown.svelte";
  import { ccList } from "./view.svelte";

  const app = $derived(current.state!);
  /** Positions are counted following time signature changes (lengths in beats of the time signature at that place). */
  const meter = $derived(mapOf(app.timeline));
  const take = $derived(shown.take);
  const clip = $derived(shown.clip);
  const count = $derived(take?.controlTicks.length ?? 0);
  const selected = $derived(take && app.pianoRoll.take === take.id ? app.pianoRoll.selectedControls : []);
  const rowH = $derived(metrics.px("size.row.s"));

  const groups = $derived(take ? ccGroups(take) : []);
  const layout = $derived(ccLines(groups, ccList.folded, count));

  function toggle(g: CcGroup): void {
    if (ccList.folded.includes(g.key)) {
      ccList.folded = ccList.folded.filter((k) => k !== g.key);
      return;
    }
    ccList.folded = [...ccList.folded, g.key];
    if (editing && layout.lineOf[editing.at] === -1) editing = null;
  }

  let viewport = $state<HTMLDivElement>();
  let top = $state(0);
  let viewH = $state(0);

  $effect(() => {
    const v = viewport;
    if (!v) return;
    const read = () => {
      top = v.scrollTop;
      viewH = v.clientHeight;
    };
    const observer = new ResizeObserver(read);
    observer.observe(v);
    v.addEventListener("scroll", read, { passive: true });
    untrack(read);
    return () => {
      observer.disconnect();
      v.removeEventListener("scroll", read);
    };
  });

  type Line = { line: number; head: CcGroup | null; folded: boolean; row: CcRow | null; selected: boolean };

  const lines = $derived.by((): Line[] => {
    if (!take || !clip || rowH <= 0) return [];
    const { lines } = layout;
    const first = Math.max(0, Math.floor(top / rowH));
    const last = Math.min(lines.length, Math.ceil((top + viewH) / rowH) + 1);
    const out: Line[] = [];
    for (let l = first; l < last; l++) {
      const v = lines[l]!;
      if (v < 0) {
        const g = groups[-1 - v]!;
        out.push({ line: l, head: g, folded: ccList.folded.includes(g.key), row: null, selected: false });
      } else out.push({ line: l, head: null, folded: false, row: ccRow(take, clip, v, meter), selected: inRanges(selected, v) });
    }
    return out;
  });

  function pick(e: MouseEvent, index: number): void {
    if (!take) return;
    send({ type: "selectControls", take: take.id, points: [index], mode: e.shiftKey ? "add" : "replace" });
  }

  // ---- Typing in fields ----

  const LABELS: Record<CcFieldName, string> = { position: t("note.position"), value: t("ccList.value") };

  /** at: the row number in the list (= the index in the take). token: a number per opened box (so the blur of the previous box does not close the next one). */
  let editing = $state<{ at: number; name: CcFieldName; draft: string; token: number } | null>(null);
  let tokens = 0;

  const specAt = (at: number, name: CcFieldName) => (take && clip ? controlFieldSpec(name, take, clip, at, meter) : null);
  const editSpec = $derived(editing ? specAt(editing.at, editing.name) : null);

  function reveal(at: number): void {
    const v = viewport;
    const line = layout.lineOf[at] ?? -1;
    if (!v || rowH <= 0 || line < 0) return;
    if (line * rowH < v.scrollTop) v.scrollTop = line * rowH;
    else if ((line + 1) * rowH > v.scrollTop + v.clientHeight) v.scrollTop = (line + 1) * rowH - v.clientHeight;
    top = v.scrollTop;
  }

  function open(at: number, name: CcFieldName): void {
    const spec = specAt(at, name);
    if (!spec) return;
    editing = { at, name, draft: spec.initial, token: ++tokens };
    reveal(at);
  }

  async function close(refocus: boolean): Promise<void> {
    const at = editing?.at;
    editing = null;
    if (!refocus || at === undefined) return;
    await tick();
    viewport?.querySelector<HTMLElement>(`[data-at="${at}"]`)?.focus();
  }

  function finish(value: number, tab: -1 | 0 | 1, clamped: FieldRead["clamped"]): void {
    const e = editing;
    const spec = editSpec;
    if (!e || !spec || !take) return;
    const command = commitControl(e.name, spec, value, take, e.at);
    if (command) send(command);
    if (clamped) send(clampCommand(clamped));
    if (tab === 0) {
      void close(true);
      return;
    }
    const next = nextCcField(e.name, tab < 0);
    const at = next.row === 0 ? e.at : neighborPoint(layout.lines, layout.lineOf[e.at] ?? -1, next.row);
    if (at === null || (layout.lineOf[at] ?? -1) < 0) void close(true);
    else open(at, next.name);
  }

  function reject(): void {
    if (editSpec) send(rejectCommand(editSpec, editSpec.initial));
  }

  function cancel(token: number): void {
    if (editing?.token === token) void close(true);
  }

  function onRowDouble(ev: MouseEvent, at: number): void {
    const name = (ev.target as HTMLElement | null)?.closest<HTMLElement>("[data-field]")?.dataset.field;
    if (name && (CC_FIELD_ORDER as readonly string[]).includes(name)) open(at, name as CcFieldName);
  }

  function onRowKey(ev: KeyboardEvent, at: number): void {
    if (ev.key !== "Enter" || ev.shiftKey || ev.ctrlKey || ev.metaKey || ev.altKey || ev.isComposing) return;
    ev.preventDefault();
    ev.stopPropagation();
    open(at, CC_FIELD_ORDER[0]!);
  }
</script>

{#snippet cell(row: CcRow, name: CcFieldName)}
  <span class="r m-typable" class:m-hi={name === "value"} data-field={name}>{row[name]}</span>
{/snippet}

{#snippet input(name: CcFieldName)}
  {#if editing && editSpec}
    {@const e = editing}
    <span class="cell">
      <NoteFieldInput
        spec={editSpec}
        bind:draft={e.draft}
        label={LABELS[name]}
        test="cc-list-field"
        right
        onfinish={finish}
        oncancel={() => cancel(e.token)}
        onreject={reject}
      />
    </span>
  {/if}
{/snippet}

<div class="list" data-testid="pr-cc-list">
  <div class="cols head m-lbl">
    <span class="r">{t("note.position")}</span>
    <span class="r">{t("ccList.value")}</span>
  </div>
  <ScrollArea bind:viewport data-test="cc-list-rows">
    <div class="rows" style:height="{layout.lines.length * rowH}px">
      {#each lines as l (l.line)}
        {#if l.head}
          {@const g = l.head}
          <button
            class="m-plain group m-focus-in"
            style:transform="translateY({l.line * rowH}px)"
            aria-expanded={!l.folded}
            data-test="cc-list-group"
            data-group={g.key}
            onclick={() => toggle(g)}
          >
            <span class={l.folded ? "m-lbl" : "m-hi"}>{l.folded ? "▸" : "▾"}</span>
            <span class="m-hi name">{g.name}</span>
            <span class="m-num m-lbl">{g.points.length}</span>
          </button>
        {:else if l.row}
          {@const row = l.row}
          {#if editing && editing.at === row.index}
            <div class="m-num cols row editing" class:selected={l.selected} style:transform="translateY({l.line * rowH}px)" data-point={row.index}>
              {#if editing.name === "position"}{@render input("position")}{:else}{@render cell(row, "position")}{/if}
              {#if editing.name === "value"}{@render input("value")}{:else}{@render cell(row, "value")}{/if}
            </div>
          {:else}
            <button
              class="m-plain m-num cols row m-focus-in"
              class:selected={l.selected}
              style:transform="translateY({l.line * rowH}px)"
              aria-pressed={l.selected}
              data-point={row.index}
              data-at={row.index}
              onclick={(e) => pick(e, row.index)}
              onkeydown={(e) => onRowKey(e, row.index)}
              ondblclick={(e) => onRowDouble(e, row.index)}
            >
              {@render cell(row, "position")}
              {@render cell(row, "value")}
            </button>
          {/if}
        {/if}
      {/each}
    </div>
  </ScrollArea>
</div>

<style>
  .list {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--color-gray-3);
    overflow: hidden;
  }
  /* Columns are position 76 and value (the rest). Both right-aligned. */
  .cols {
    display: grid;
    grid-template-columns: var(--mock-notelist-position) minmax(0, 1fr);
    align-items: center;
    height: var(--size-row-s);
    flex-shrink: 0;
    white-space: nowrap;
  }
  .cols > span {
    padding: 0 var(--mock-notelist-cell-pad);
    overflow: hidden;
  }
  .cols > .r {
    text-align: right;
  }
  /* The field being typed in: the input box fills the field. */
  .cols > .cell {
    height: 100%;
    padding: 0;
  }
  .head {
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  .rows {
    position: relative;
  }
  /* Per-kind heading (R5-Lists): background gray.2, ▾ ▸, name, number of points. */
  .group {
    position: absolute;
    left: 0;
    top: 0;
    width: 100%;
    height: var(--size-row-s);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    background: var(--color-gray-2);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    white-space: nowrap;
  }
  .group .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .group:hover,
  .group:active {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  /* List rows (spec 2): the same as the note list. */
  .row {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    width: 100%;
    color: var(--color-gray-9);
    background: var(--color-gray-3);
    border-bottom: var(--line-hair) solid var(--color-line-row);
    border-left: var(--line-mark) solid var(--color-gray-3);
  }
  .row.selected {
    background: var(--color-gray-4);
    border-left-color: var(--color-accent-ink);
  }
  button.row:hover {
    color: var(--color-gray-10);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  button.row:active {
    background: var(--color-gray-2);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
</style>

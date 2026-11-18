<!-- ひらいた「CC の一覧」の中身（仕様書 3 章「第 5 版で足した決まり」、モック R5-Lists）。見出し（▾ CC の一覧 17）は帯の部品（PianoRoll.svelte）が描く。
     列の見出しの行（位置 ・ 種類 ・ 値）と、テイクのすべての点の行（位置の順）。ノートの一覧と同じく、見えている行だけを作り、同じスクロールの棒で縦に送る。
     行を押すとその点を選ぶ（Shift で足す。selectControls）。欄を 2 回押すか、行で Enter を押すと、その欄が「入力の枠」（ノートの一覧と同じ打ちこみ）。
     打ちこめるのは位置と値だけ。決めると moveControls を 1 つ。 -->
<script lang="ts">
  import { tick, untrack } from "svelte";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { textWidth } from "../fit";
  import { metrics } from "../metrics.svelte";
  import { CC_MAX, isOnOff, PITCH_BEND_MIN, switchText, valueText } from "./cc";
  import { CC_FIELD_ORDER, ccRow, commitControl, controlFieldSpec, controlOf, nextCcField, type CcFieldName, type CcRow } from "./ccFields";
  import NoteFieldInput from "./NoteFieldInput.svelte";
  import { clampCommand, rejectCommand, type FieldRead } from "./noteFields";
  import { inRanges } from "./notes";
  import { shown } from "./shown.svelte";

  const app = $derived(current.state!);
  const meter = $derived({ num: app.song.meterNum, den: app.song.meterDen });
  const take = $derived(shown.take);
  const clip = $derived(shown.clip);
  const count = $derived(take?.controlTicks.length ?? 0);
  const selected = $derived(take && app.pianoRoll.take === take.id ? app.pianoRoll.selectedControls : []);
  const rowH = $derived(metrics.px("size.row.s"));

  /** 値の列の、いちばん長い書きかたの幅（CSS px）。どの行も同じ幅にそろえる。 */
  const valueMin = $derived.by(() => {
    if (!take) return 0;
    let longest = valueText({ kind: "cc", number: 0 }, CC_MAX);
    for (let i = 0; i < count; i++) {
      const c = controlOf(take, i);
      if (isOnOff(c)) {
        longest = switchText(CC_MAX);
        break;
      }
      if (c.kind === "pitchBend") longest = valueText(c, PITCH_BEND_MIN);
    }
    return metrics.scaled(textWidth(longest)) + 2 * metrics.px("mock.notelist.cell-pad");
  });

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

  const rows = $derived.by((): (CcRow & { selected: boolean })[] => {
    if (!take || !clip || rowH <= 0) return [];
    const first = Math.max(0, Math.floor(top / rowH));
    const last = Math.min(count, Math.ceil((top + viewH) / rowH) + 1);
    const out = [];
    for (let i = first; i < last; i++) out.push({ ...ccRow(take, clip, i, meter), selected: inRanges(selected, i) });
    return out;
  });

  function pick(e: MouseEvent, index: number): void {
    if (!take) return;
    send({ type: "selectControls", take: take.id, points: [index], mode: e.shiftKey ? "add" : "replace" });
  }

  // ---- 欄の打ちこみ ----

  const LABELS: Record<CcFieldName, string> = { position: t("note.position"), value: t("ccList.value") };

  /** at: 一覧の何行目か（= テイクの中の番号）。token: ひらいた枠ごとの番号（前の枠の blur で、次の枠をとじないため）。 */
  let editing = $state<{ at: number; name: CcFieldName; draft: string; token: number } | null>(null);
  let tokens = 0;

  const specAt = (at: number, name: CcFieldName) => (take && clip ? controlFieldSpec(name, take, clip, at, meter) : null);
  const editSpec = $derived(editing ? specAt(editing.at, editing.name) : null);

  function reveal(at: number): void {
    const v = viewport;
    if (!v || rowH <= 0) return;
    if (at * rowH < v.scrollTop) v.scrollTop = at * rowH;
    else if ((at + 1) * rowH > v.scrollTop + v.clientHeight) v.scrollTop = (at + 1) * rowH - v.clientHeight;
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
    const at = e.at + next.row;
    if (at < 0 || at >= count) void close(true);
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
  <span class="r" class:m-hi={name === "value"} data-field={name}>{row[name]}</span>
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

<div class="list" data-testid="pr-cc-list" style:--cc-value-min="{Math.ceil(valueMin)}px">
  <div class="cols head m-lbl">
    <span class="r">{t("note.position")}</span>
    <span>{t("ccList.kind")}</span>
    <span class="r">{t("ccList.value")}</span>
  </div>
  <ScrollArea bind:viewport data-test="cc-list-rows">
    <div class="rows" style:height="{count * rowH}px">
      {#each rows as row (row.index)}
        {#if editing && editing.at === row.index}
          <div class="m-num cols row editing" class:selected={row.selected} style:transform="translateY({row.index * rowH}px)" data-point={row.index}>
            {#if editing.name === "position"}{@render input("position")}{:else}{@render cell(row, "position")}{/if}
            <span class="kind">{row.kind}</span>
            {#if editing.name === "value"}{@render input("value")}{:else}{@render cell(row, "value")}{/if}
          </div>
        {:else}
          <button
            class="m-plain m-num cols row m-focus-in"
            class:selected={row.selected}
            style:transform="translateY({row.index * rowH}px)"
            aria-pressed={row.selected}
            data-point={row.index}
            data-at={row.index}
            onclick={(e) => pick(e, row.index)}
            onkeydown={(e) => onRowKey(e, row.index)}
            ondblclick={(e) => onRowDouble(e, row.index)}
          >
            {@render cell(row, "position")}
            <span class="kind">{row.kind}</span>
            {@render cell(row, "value")}
          </button>
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
  /* 欄の幅はモック R5-Lists（位置 76 ・ 種類 96 ・ 値は残り）。欄の幅 216 では「踏む（127）」が入らないので、
     値の列はいちばん長い書きかたの幅を先にとり（--cc-value-min）、種類のほうを「…」で縮める。 */
  .cols {
    display: grid;
    grid-template-columns: var(--mock-notelist-position) minmax(0, var(--mock-cclist-kind)) minmax(var(--cc-value-min), 1fr);
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
  /* 種類は日本語の字（Mono にしない）、gray.10。 */
  .kind {
    font-family: var(--font-sans);
    color: var(--color-gray-10);
    text-overflow: ellipsis;
  }
  /* 打っている欄: 入力の枠が欄いっぱい。 */
  .cols > .cell {
    height: 100%;
    padding: 0;
  }
  .head {
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .rows {
    position: relative;
  }
  /* 一覧の行（仕様書 2 章）: ノートの一覧と同じ。 */
  .row {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    width: 100%;
    color: var(--color-gray-9);
    background: var(--color-gray-3);
    border-bottom: var(--line-hair) solid var(--color-gray-4);
    border-left: var(--line-mark) solid var(--color-gray-3);
  }
  .row.selected {
    background: var(--color-gray-4);
    border-left-color: var(--color-accent);
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

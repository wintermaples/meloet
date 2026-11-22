<!-- ピアノロールの左の「ノートの一覧」の中身（モック A-PianoRoll・F-WidePR の左の欄）。見出し（▾ ノートの一覧 23）とたたんだ縦の見出しは、
     帯の部品（PianoRoll.svelte）が描く。これは、ひらいたときの中身だけを、親の箱いっぱいに描く。見せているテイクは `shown`（shown.svelte.ts）。
     欄の見出しの行（位置 ・ 音 ・ 長さ ・ Vel）と、ノートごとの行（仕様書 2 章「一覧の行」、高さ size.row.s）。ノートが何万あっても重くならないよう、
     見えている行だけを作る。行を押すとそのノートを選ぶ（Shift で足す）。選ぶのは Rust（selectNotes を送るだけ）。
     欄を 2 回押すか、行で Enter を押すと、その欄が「入力の枠」になる（仕様書 3 章、モック R3-Lanes）。決めると setNotesField（その行のノート 1 つ）。
     Tab で右の欄へ、右はしなら次の行の最初の欄へ（Shift Tab は逆）。打っている行は、一覧の何行目かで覚える（決めるとテイクの id が変わるため）。
     ドラムのトラックでは「長さ」の列を出さず、位置 68 ・ 音（楽器の名前、のこり全部。入りきらなければ「…」、乗せると全部）・ Vel 40（仕様書 3 章）。
     キットにない鍵のノートは音名で。 -->
<script lang="ts">
  import { mapOf, meterAtOf } from "../../time/bars";
  import { tick, untrack } from "svelte";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { tipWhenCut } from "../../lib/tip.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { noteLengthText, notePositionText } from "../format";
  import { metrics } from "../metrics.svelte";
  import { drumName, keyLabel, rollShowsDrums } from "./drums";
  import NoteFieldInput from "./NoteFieldInput.svelte";
  import {
    clampCommand,
    commitCommand,
    DRUM_FIELD_ORDER,
    FIELD_ORDER,
    nextField,
    noteFieldSpec,
    rejectCommand,
    type FieldName,
    type FieldRead,
  } from "./noteFields";
  import { clipNoteIndexes, inRanges } from "./notes";
  import { shown } from "./shown.svelte";

  const app = $derived(current.state!);
  /** 位置は拍子の変更をたどって数える（長さは、その所の拍子の拍で）。 */
  const meter = $derived(mapOf(app.timeline));
  const take = $derived(shown.take);
  const clip = $derived(shown.clip);
  /** 一覧に出すノートの、テイクの中の番号（はじまりの順）。 */
  const indexes = $derived(take && clip ? clipNoteIndexes(clip, take) : new Uint32Array(0));
  const selected = $derived(take && app.pianoRoll.take === take.id ? app.pianoRoll.selectedNotes : []);
  const rowH = $derived(metrics.px("size.row.s"));
  const drums = $derived(rollShowsDrums(app, clip, shown.track));
  const fields = $derived(drums ? DRUM_FIELD_ORDER : FIELD_ORDER);

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

  interface Row {
    /** 一覧の中の何行目か。 */
    at: number;
    /** テイクの中の番号。 */
    index: number;
    position: string;
    key: string;
    /** 音が楽器の名前か（ドラムの、キットにある鍵）。 */
    named: boolean;
    length: string;
    velocity: number;
    selected: boolean;
  }

  const rows = $derived.by((): Row[] => {
    if (!take || !clip || rowH <= 0) return [];
    const first = Math.max(0, Math.floor(top / rowH));
    const last = Math.min(indexes.length, Math.ceil((top + viewH) / rowH) + 1);
    const at = clip.start + clip.takeOffset;
    const out: Row[] = [];
    for (let r = first; r < last; r++) {
      const i = indexes[r]!;
      out.push({
        at: r,
        index: i,
        position: notePositionText(at + take.starts[i]!, meter),
        key: keyLabel(take.keys[i]!, drums),
        named: !!drums && drumName(take.keys[i]!, drums) !== null,
        length: noteLengthText(take.lengths[i]!, meterAtOf(meter, at + take.starts[i]!)),
        velocity: take.velocities[i]!,
        selected: inRanges(selected, i),
      });
    }
    return out;
  });

  function pick(e: MouseEvent, index: number): void {
    if (!take) return;
    send({ type: "selectNotes", take: take.id, notes: [index], mode: e.shiftKey ? "add" : "replace" });
  }

  // ---- 欄の打ちこみ ----

  const LABELS: Record<FieldName, string> = {
    position: t("note.position"),
    key: t("note.key"),
    length: t("note.length"),
    velocity: t("noteList.velocity"),
  };

  /** at: 一覧の何行目か。token: ひらいた枠ごとの番号（前の枠の blur で、次の枠をとじないため）。 */
  let editing = $state<{ at: number; name: FieldName; draft: string; token: number } | null>(null);
  let tokens = 0;

  const specAt = (at: number, name: FieldName) =>
    take && clip && at >= 0 && at < indexes.length ? noteFieldSpec(name, take, clip, [indexes[at]!], meter, drums) : null;
  const editSpec = $derived(editing ? specAt(editing.at, editing.name) : null);

  /** 打っている行が見えるように送る。 */
  function reveal(at: number): void {
    const v = viewport;
    if (!v || rowH <= 0) return;
    if (at * rowH < v.scrollTop) v.scrollTop = at * rowH;
    else if ((at + 1) * rowH > v.scrollTop + v.clientHeight) v.scrollTop = (at + 1) * rowH - v.clientHeight;
    top = v.scrollTop;
  }

  function open(at: number, name: FieldName): void {
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
    const command = commitCommand(spec, value, take.id, [indexes[e.at]!]);
    if (command) send(command);
    if (clamped) send(clampCommand(clamped));
    if (tab === 0) {
      void close(true);
      return;
    }
    const next = nextField(e.name, tab < 0, fields);
    const at = e.at + next.row;
    if (at < 0 || at >= indexes.length) void close(true);
    else open(at, next.name);
  }

  function reject(): void {
    if (editSpec && editing) send(rejectCommand(editSpec, editSpec.initial));
  }

  function cancel(token: number): void {
    if (editing?.token === token) void close(true);
  }

  function onRowDouble(ev: MouseEvent, at: number): void {
    const name = (ev.target as HTMLElement | null)?.closest<HTMLElement>("[data-field]")?.dataset.field;
    if (name && (fields as readonly string[]).includes(name)) open(at, name as FieldName);
  }

  function onRowKey(ev: KeyboardEvent, at: number): void {
    if (ev.key !== "Enter" || ev.shiftKey || ev.ctrlKey || ev.metaKey || ev.altKey || ev.isComposing) return;
    ev.preventDefault();
    ev.stopPropagation();
    open(at, fields[0]!);
  }
</script>

{#snippet cell(row: Row, name: FieldName)}
  {#if name === "key" && row.named}
    <span class="m-hi name" data-field={name} use:tipWhenCut>{row.key}</span>
  {:else}
    <span class:r={name !== "key"} class:m-hi={name === "key" || name === "velocity"} data-field={name}>{row[name]}</span>
  {/if}
{/snippet}

<div class="list" data-testid="pr-note-list">
  <div class="cols head m-lbl" class:drums>
    <span class="r">{t("note.position")}</span>
    <span>{t("note.key")}</span>
    {#if !drums}<span class="r">{t("note.length")}</span>{/if}
    <span class="r">{t("noteList.velocity")}</span>
  </div>
  <ScrollArea bind:viewport data-test="note-list-rows">
    <div class="rows" style:height="{indexes.length * rowH}px">
      {#each rows as row (row.index)}
        {#if editing && editSpec && editing.at === row.at}
          {@const e = editing}
          <div
            class="m-num cols row editing"
            class:drums
            class:selected={row.selected}
            style:transform="translateY({row.at * rowH}px)"
            data-note={row.index}
          >
            {#each fields as name (name)}
              {#if name === e.name}
                <span class="cell">
                  <NoteFieldInput
                    spec={editSpec}
                    bind:draft={e.draft}
                    label={LABELS[name]}
                    test="note-list-field"
                    right={name !== "key"}
                    onfinish={finish}
                    oncancel={() => cancel(e.token)}
                    onreject={reject}
                  />
                </span>
              {:else}
                {@render cell(row, name)}
              {/if}
            {/each}
          </div>
        {:else}
          <button
            class="m-plain m-num cols row m-focus-in"
            class:drums
            class:selected={row.selected}
            style:transform="translateY({row.at * rowH}px)"
            aria-pressed={row.selected}
            data-note={row.index}
            data-at={row.at}
            onclick={(e) => pick(e, row.index)}
            onkeydown={(e) => onRowKey(e, row.at)}
            ondblclick={(e) => onRowDouble(e, row.at)}
          >
            {@render cell(row, "position")}
            {@render cell(row, "key")}
            {#if !drums}
              {@render cell(row, "length")}
            {/if}
            {@render cell(row, "velocity")}
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
  /* 欄の幅はモック A-PianoRoll（76 ・ 40 ・ 52 ・ 残り）。 */
  .cols {
    display: grid;
    grid-template-columns: var(--mock-notelist-position) var(--mock-notelist-key) var(--mock-notelist-length) 1fr;
    align-items: center;
    height: var(--size-row-s);
    flex-shrink: 0;
    white-space: nowrap;
  }
  /* ドラム（仕様書 3 章）: 位置 68 ・ 音（のこり全部）・ Vel 40。 */
  .cols.drums {
    grid-template-columns: var(--part-drum-list-position) 1fr var(--part-drum-list-velocity);
  }
  .cols > span {
    padding: 0 var(--mock-notelist-cell-pad);
    overflow: hidden;
  }
  /* 楽器の名前（モック R7-Drum: 一覧の中でも書体は本文のまま）。入りきらなければ「…」。 */
  .cols > .name {
    font-family: var(--font-sans);
    text-overflow: ellipsis;
  }
  .cols > .r {
    text-align: right;
  }
  /* 打っている欄: 入力の枠が欄いっぱい（モック R3-Lanes）。 */
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
  /* 一覧の行（仕様書 2 章）: ふつうは地 gray.3、左のふちは地と同じ色。選んだ行は左 line.mark の accent、地 gray.4。 */
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
  /* 乗った（枠 gray.8 ・ 文字 gray.10）・押した（bg gray.2 ・ 枠 gray.8）。枠は左のふちの内がわに描くので、
     選んだ行の左 line.mark の accent とまざらない（docs/history/design-notes.ja.md A-6）。 */
  button.row:hover {
    color: var(--color-gray-10);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  button.row:active {
    background: var(--color-gray-2);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
</style>

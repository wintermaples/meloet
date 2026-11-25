<!-- The content of 「ノートの一覧」 on the left of the piano roll (the left panel of mock A-PianoRoll, F-WidePR). The heading (「▾ ノートの一覧 23」) and the folded vertical heading are
     drawn by the band part (PianoRoll.svelte). This draws only the open content, filling the parent box. The shown take is `shown` (shown.svelte.ts).
     A row of field headings (position, note, length, Vel), and a row per note (spec 2 "List row", height size.row.s). So that tens of thousands of notes are not heavy,
     only the visible rows are made. Pressing a row selects that note (Shift adds). Rust selects (this only sends selectNotes).
     Pressing a field twice, or Enter on a row, turns that field into an 「入力の枠」 (spec 3, mock R3-Lanes). Deciding sends setNotesField (the one note of that row).
     Tab goes to the field on the right, and from the right end to the first field of the next row (Shift Tab the other way). The row being typed in is remembered by its row number in the list (deciding changes the take id).
     On a drum track, the 「長さ」 column is not shown: position 68, note (the drum piece name, all the rest; 「…」 if it does not fit, all on hover), Vel 40 (spec 3).
     Notes of keys not in the kit use note names. -->
<script lang="ts">
  import { mapOf, meterAtOf } from "../../time/bars";
  import { tick, untrack } from "svelte";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { tipWhenCut } from "../../lib/tip.svelte";
  import { songBusy } from "../../keys/attach";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text";
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
  /** Positions are counted following time signature changes (lengths in beats of the time signature at that place). */
  const meter = $derived(mapOf(app.timeline));
  const take = $derived(shown.take);
  const clip = $derived(shown.clip);
  /** The indexes in the take of the notes shown in the list (in start order). */
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
    /** The row number in the list. */
    at: number;
    /** The index in the take. */
    index: number;
    position: string;
    key: string;
    /** Whether the note is a drum piece name (for drums, a key the kit has). */
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

  // ---- Typing in fields ----

  const LABELS: Record<FieldName, string> = {
    position: t("note.position"),
    key: t("note.key"),
    length: t("note.length"),
    velocity: t("noteList.velocity"),
  };

  /** at: the row number in the list. token: a number per opened box (so the blur of the previous box does not close the next one). */
  let editing = $state<{ at: number; name: FieldName; draft: string; token: number } | null>(null);
  let tokens = 0;

  const specAt = (at: number, name: FieldName) =>
    take && clip && at >= 0 && at < indexes.length ? noteFieldSpec(name, take, clip, [indexes[at]!], meter, drums) : null;
  const editSpec = $derived(editing ? specAt(editing.at, editing.name) : null);

  /** Scrolls so the row being typed in is visible. */
  function reveal(at: number): void {
    const v = viewport;
    if (!v || rowH <= 0) return;
    if (at * rowH < v.scrollTop) v.scrollTop = at * rowH;
    else if ((at + 1) * rowH > v.scrollTop + v.clientHeight) v.scrollTop = (at + 1) * rowH - v.clientHeight;
    top = v.scrollTop;
  }

  function open(at: number, name: FieldName): void {
    after = null;
    // While recording or counting in Rust takes no edit.
    const spec = songBusy(app) ? null : specAt(at, name);
    if (!spec) return;
    editing = { at, name, draft: spec.initial, token: ++tokens };
    reveal(at);
  }

  /**
   * A Tab after a value that can change the order of the notes: the next field opens once Rust's answer and its take are shown,
   * on the row of the edited note (Rust selects it), with its text from that take. old: the take id before; row: the step from that row.
   */
  let after = $state.raw<{ name: FieldName; row: -1 | 0 | 1; old: number; at: number; rev: number | null } | null>(null);
  $effect(() => {
    const a = after;
    if (!a || a.rev === null || current.rev < a.rev || !take || take.id !== app.pianoRoll.take) return;
    untrack(() => {
      after = null;
      let row = a.at;
      if (take.id !== a.old) {
        const edited = app.pianoRoll.selectedNotes[0]?.start;
        row = edited === undefined ? -1 : indexes.indexOf(edited);
      }
      const at = row < 0 ? -1 : row + a.row;
      if (at >= 0 && at < indexes.length) open(at, a.name);
    });
  });

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
    const sent = command ? send(command) : null;
    if (clamped) send(clampCommand(clamped));
    if (tab === 0) {
      void close(true);
      return;
    }
    const next = nextField(e.name, tab < 0, fields);
    if (sent && (e.name === "position" || e.name === "key")) {
      void close(true);
      const a = { name: next.name, row: next.row, old: take.id, at: e.at, rev: null };
      after = a;
      void sent.then((rev) => {
        if (after === a) after = rev === null ? null : { ...a, rev };
      });
      return;
    }
    const at = e.at + next.row;
    if (at < 0 || at >= indexes.length) void close(true);
    else open(at, next.name);
  }

  function reject(): void {
    if (editSpec && editing) send(rejectCommand(editSpec, editSpec.initial));
  }

  function cancel(token: number, refocus: boolean): void {
    if (editing?.token === token) void close(refocus);
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
    <span class="m-hi name m-typable" data-field={name} use:tipWhenCut>{row.key}</span>
  {:else}
    <span class="m-typable" class:r={name !== "key"} class:m-hi={name === "key" || name === "velocity"} data-field={name}>{row[name]}</span>
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
                    oncancel={(refocus) => cancel(e.token, refocus)}
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
  /* Field widths are from mock A-PianoRoll (76, 40, 52, the rest). */
  .cols {
    display: grid;
    grid-template-columns: var(--mock-notelist-position) var(--mock-notelist-key) var(--mock-notelist-length) 1fr;
    align-items: center;
    height: var(--size-row-s);
    flex-shrink: 0;
    white-space: nowrap;
  }
  /* Drums (spec 3): position 68, note (all the rest), Vel 40. */
  .cols.drums {
    grid-template-columns: var(--part-drum-list-position) 1fr var(--part-drum-list-velocity);
  }
  .cols > span {
    padding: 0 var(--mock-notelist-cell-pad);
    overflow: hidden;
  }
  /* Drum piece names (mock R7-Drum: the body face even inside the list). 「…」 if it does not fit. */
  .cols > .name {
    font-family: var(--font-sans);
    text-overflow: ellipsis;
  }
  .cols > .r {
    text-align: right;
  }
  /* The field being typed in: the input box fills the field (mock R3-Lanes). */
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
  /* List rows (spec 2): normally background gray.3, with the left edge the same color as the background. A selected row has a line.mark accent on the left and background gray.4. */
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
  /* Hover (border gray.8, text gray.10), pressed (bg gray.2, border gray.8). The border is drawn inside the left edge, so it does not
     mix with the line.mark accent on the left of a selected row (docs/history/design-notes.ja.md A-6). */
  button.row:hover {
    color: var(--color-gray-10);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  button.row:active {
    background: var(--color-gray-2);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
</style>

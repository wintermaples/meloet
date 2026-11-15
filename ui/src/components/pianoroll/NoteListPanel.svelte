<!-- ピアノロールの左の「ノートの一覧」の中身（モック A-PianoRoll・F-WidePR の左の欄）。見出し（▾ ノートの一覧 23）とたたんだ縦の見出しは、
     帯の部品（PianoRoll.svelte）が描く。これは、ひらいたときの中身だけを、親の箱いっぱいに描く。見せているテイクは `shown`（shown.svelte.ts）。
     欄の見出しの行（位置 ・ 音 ・ 長さ ・ Vel）と、ノートごとの行（仕様書 2 章「一覧の行」、高さ size.row.s）。ノートが何万あっても重くならないよう、
     見えている行だけを作る。行を押すとそのノートを選ぶ（Shift で足す）。選ぶのは Rust（selectNotes を送るだけ）。 -->
<script lang="ts">
  import { untrack } from "svelte";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { noteLengthText, notePositionText } from "../format";
  import { noteName } from "../meterFormat";
  import { metrics } from "../metrics.svelte";
  import { clipNoteIndexes, inRanges } from "./notes";
  import { shown } from "./shown.svelte";

  const app = $derived(current.state!);
  const meter = $derived({ num: app.song.meterNum, den: app.song.meterDen });
  const take = $derived(shown.take);
  const clip = $derived(shown.clip);
  /** 一覧に出すノートの、テイクの中の番号（はじまりの順）。 */
  const indexes = $derived(take && clip ? clipNoteIndexes(clip, take) : new Uint32Array(0));
  const selected = $derived(take && app.pianoRoll.take === take.id ? app.pianoRoll.selectedNotes : []);
  const rowH = $derived(metrics.px("size.row.s"));

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
        key: noteName(take.keys[i]!),
        length: noteLengthText(take.lengths[i]!, meter),
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
</script>

<div class="list" data-testid="pr-note-list">
  <div class="cols head m-lbl">
    <span class="r">{t("note.position")}</span>
    <span>{t("note.key")}</span>
    <span class="r">{t("note.length")}</span>
    <span class="r">{t("noteList.velocity")}</span>
  </div>
  <ScrollArea bind:viewport data-test="note-list-rows">
    <div class="rows" style:height="{indexes.length * rowH}px">
      {#each rows as row (row.index)}
        <button
          class="m-plain m-num cols row"
          class:selected={row.selected}
          style:transform="translateY({row.at * rowH}px)"
          aria-pressed={row.selected}
          data-note={row.index}
          onclick={(e) => pick(e, row.index)}
        >
          <span class="r">{row.position}</span>
          <span class="m-hi">{row.key}</span>
          <span class="r">{row.length}</span>
          <span class="r m-hi">{row.velocity}</span>
        </button>
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
  .cols > span {
    padding: 0 var(--mock-notelist-cell-pad);
    overflow: hidden;
  }
  .cols > .r {
    text-align: right;
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
</style>

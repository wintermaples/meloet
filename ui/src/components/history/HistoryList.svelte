<!-- 取り消しの履歴の帯の行（モック F-History、R10-History、仕様書 2 章「履歴の行 ・ 乗った ・ えらんだ」、3 章 Y19）。
     新しい手が上。見えている行だけを作り、行は bridge の historyRowBook が見えている所だけ取りにいってためる。
     行を押すとえらぶ。乗った行とえらんだ行は地 gray.4 で右に「ここまで戻す Enter」。押すか Enter で HistoryJump。
     キー（↑ ↓ Home Enter Esc）は帯にフォーカスがあるとき keys/attach.ts から来る。 -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { historyRowBook } from "../../bridge";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { metrics } from "../metrics.svelte";
  import { historyJumpCommand, historyUi, pickRow, selectedRow } from "./historyUi.svelte";
  import { historyRowText, stepPlaceText } from "./stepText";

  /** 見えている行の前後に、先に取りにいく行の数。 */
  const AHEAD = 16;

  const app = $derived(current.state!);
  const history = $derived(app.history);
  const total = $derived(history.total);
  const rowH = $derived(metrics.px("size.row"));
  const book = historyRowBook();

  let version = $state(0);
  onMount(() => {
    // えらんだ行は、帯をひらくたびに捨てる（ひらいた所はいまの手）。
    historyUi.selected = null;
    return book.listen(() => version++);
  });

  let viewport = $state<HTMLDivElement>();
  let top = $state(0);
  let viewH = $state(0);
  let hover = $state<number | null>(null);
  let focused = $state(false);

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

  /** 見えている行の番号の範囲（上の行が新しい）。 */
  const shown = $derived.by(() => {
    if (rowH <= 0 || total === 0) return { first: 0, last: -1 };
    const first = Math.max(0, Math.floor(top / rowH));
    const last = Math.min(total - 1, Math.ceil((top + viewH) / rowH));
    return { first, last };
  });

  $effect(() => {
    const { first, last } = shown;
    if (last < first || !current.connected) return;
    book.want(total - last - AHEAD, total - first + AHEAD);
  });

  const selected = $derived(selectedRow(history));

  interface Line {
    line: number;
    index: number;
    text: string | null;
    place: string | null;
  }
  const lines = $derived.by((): Line[] => {
    void version;
    const out: Line[] = [];
    const ctx = { song: app.song, library: app.library };
    for (let line = shown.first; line <= shown.last; line++) {
      const index = total - line;
      const row = book.row(index);
      out.push({ line, index, text: row ? historyRowText(row, ctx) : null, place: row ? stepPlaceText(row.place, ctx) : null });
    }
    return out;
  });

  /** 見えている所のいちばん下の行より前の手の数（「… 6 手目より前は、下へスクロール」）。 */
  const before = $derived(rowH > 0 ? total - Math.floor((top + viewH) / rowH) : 0);

  /** その行が見えるように送る。 */
  function reveal(index: number): void {
    const v = viewport;
    if (!v || rowH <= 0) return;
    const y = (total - index) * rowH;
    if (y < v.scrollTop) v.scrollTop = y;
    else if (y + rowH > v.scrollTop + v.clientHeight) v.scrollTop = y + rowH - v.clientHeight;
    // scroll の知らせを待たずに、同じコマで行を作る。
    top = v.scrollTop;
  }

  // ひらいたときは、いまの手が見えるように。
  let revealed = false;
  $effect(() => {
    if (revealed || !viewport || viewH <= 0) return;
    revealed = true;
    const now = untrack(() => history.current);
    if (now >= 1) reveal(now);
  });

  // キーでえらんだ行は、見える所まで送る。
  $effect(() => {
    const s = selected;
    if (s !== null && historyUi.byKeys) untrack(() => reveal(s));
  });

  let list = $state<HTMLDivElement>();

  /** 行を押すとえらぶ。フォーカスは行ではなく帯に置く（行は見えなくなると消えるので）。 */
  function press(e: PointerEvent, index: number): void {
    if (e.button !== 0) return;
    e.preventDefault();
    list?.focus();
    pickRow(index);
  }

  function jump(index: number): void {
    const c = historyJumpCommand(app, index, book);
    if (c) send(c);
  }
</script>

<div
  bind:this={list}
  class="list"
  role="listbox"
  tabindex="0"
  aria-label={t("band.history")}
  aria-activedescendant={selected !== null ? `history-row-${selected}` : undefined}
  data-history-keys
  data-test="history-list"
  onfocusin={() => (focused = true)}
  onfocusout={() => (focused = false)}
>
  <ScrollArea bind:viewport data-test="history-rows">
    <div class="rows" style:height="{total * rowH}px">
      {#each lines as l (l.index)}
        {@const now = l.index === history.current}
        {@const undone = l.index > history.current}
        {@const lit = !now && (l.index === hover || l.index === selected)}
        <div
          class="row"
          class:now
          class:undone
          class:lit
          class:ring={focused && historyUi.byKeys && l.index === selected}
          id="history-row-{l.index}"
          role="option"
          tabindex="-1"
          aria-selected={l.index === selected}
          style:transform="translateY({l.line * rowH}px)"
          data-index={l.index}
          onpointerenter={() => (hover = l.index)}
          onpointerleave={() => {
            if (hover === l.index) hover = null;
          }}
          onpointerdown={(e) => press(e, l.index)}
        >
          <span class="num m-num m-lbl">{l.index}</span>
          <span class="name" class:m-hi={now}>{l.text ?? ""}</span>
          <span class="m-lbl">{l.place ?? ""}</span>
          <span class="m-spacer"></span>
          {#if lit}
            <button
              type="button"
              class="m-btn"
              data-test="history-jump"
              onclick={(e) => {
                e.stopPropagation();
                pickRow(l.index);
                jump(l.index);
              }}>{t("history.jumpHere")} <span class="m-num m-lbl">{t("key.enter")}</span></button
            >
          {:else if now}
            <span class="here">{t("history.current")}</span>
          {:else if undone}
            <span class="m-lbl">{t("history.redoable")}</span>
          {/if}
        </div>
      {/each}
    </div>
  </ScrollArea>
  <div class="more m-lbl" data-test="history-more">
    {#if before >= 1 && total > 0}{t("history.more", { index: before })}{/if}
  </div>
</div>

<style>
  .list {
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--color-gray-1);
    outline: none;
  }
  .rows {
    position: relative;
  }
  /* 行は高さ size.row、左右と欄のあいだは space.3、下に gray.4 の line.hair。左の line.mark は、いまの手だけ accent。 */
  .row {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    box-sizing: border-box;
    height: var(--size-row);
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-gray-4);
    border-left: var(--line-mark) solid var(--color-gray-1);
    background: var(--color-gray-1);
    color: var(--color-gray-9);
    white-space: nowrap;
    cursor: default;
  }
  .row.undone {
    color: var(--color-gray-8);
  }
  .row.now {
    border-left-color: var(--color-accent);
    background: var(--color-gray-4);
  }
  .row.lit {
    background: var(--color-gray-4);
  }
  .row.ring {
    outline: var(--line-hair) solid var(--color-gray-10);
    outline-offset: calc(-1 * var(--line-hair));
  }
  .num {
    width: var(--mock-history-number);
    flex-shrink: 0;
    text-align: right;
  }
  .name {
    width: var(--mock-history-name);
    flex-shrink: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .here {
    color: var(--color-accent);
  }
  .more {
    flex-shrink: 0;
    margin-top: auto;
    height: var(--size-row);
    display: flex;
    align-items: center;
    padding: 0 var(--space-3);
  }
</style>

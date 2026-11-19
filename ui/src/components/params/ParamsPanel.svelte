<!-- パラメータの欄（右の欄。仕様書 1 章「パラメータ」: 見出し 24、名前と形式と居場所 48、さがす枠 32、一覧。モック F4-Mixer ・ R6-Delay）。
     えらんだ子は core が持つ（AppState.params.selected）。さがす文字とたたんだ見出しは画面だけが持ち、えらぶ子が変わるとはじめに戻す。
     いま出すのは最初から入っているエフェクトだけ（プラグインをえらんでいるときは見出しだけ）。
     最初から入っているエフェクトは窓を持たず、名前の右は形式の印ではなく「最初から入っている」。「この子は窓を持ちません。」の行は出さない（仕様書 3 章 Y8）。
     何もえらんでいないときは見出しだけ（仕様書にもモックにも形がない）。 -->
<script lang="ts">
  import { untrack } from "svelte";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import Button from "../Button.svelte";
  import { stripName } from "../mixer/stripName";
  import ParamRow from "./ParamRow.svelte";
  import { allRows, rowCount, searchRows, stepRow } from "./paramRows";
  import { builtinOf } from "./selected";

  let { onfold }: { onfold: () => void } = $props();

  const app = $derived(current.state!);
  const sel = $derived(builtinOf(app.params));

  let query = $state("");
  let folded = $state<string[]>([]);
  const insertKey = $derived(sel ? `${JSON.stringify(sel.insert)}:${sel.effect}` : "");
  $effect(() => {
    void insertKey;
    untrack(() => {
      query = "";
      folded = [];
    });
  });

  const groups = $derived(sel ? allRows(sel.effect, sel.values) : []);
  const found = $derived(searchRows(groups, query));
  const total = $derived(rowCount(groups));
  const shownCount = $derived(rowCount(found));
  const visibleRows = $derived(found.flatMap((g) => (g.group !== null && folded.includes(g.group) ? [] : g.rows)));

  function pick(param: number): void {
    if (sel && sel.row !== param) send({ type: "selectParam", param });
  }
  function toggleGroup(group: string): void {
    folded = folded.includes(group) ? folded.filter((g) => g !== group) : [...folded, group];
  }

  // 仕様書 4 章「パラメータの中: 行を上下にえらぶ」（↑ ↓）。欄の中にフォーカスがあるときだけ。
  function onkeydown(ev: KeyboardEvent): void {
    if (ev.key !== "ArrowUp" && ev.key !== "ArrowDown") return;
    if (ev.ctrlKey || ev.metaKey || ev.altKey || ev.shiftKey || ev.isComposing || !sel) return;
    ev.preventDefault();
    ev.stopPropagation();
    const next = stepRow(visibleRows, sel.row, ev.key === "ArrowDown" ? 1 : -1);
    if (next !== null) pick(next);
  }
</script>

<aside class="panel" data-band="params" data-test="params">
  <button class="m-plain head m-focus-in" onclick={onfold} data-test="rail-fold">
    <span class="m-hi">▾</span>
    <span class="m-name">{t("band.params")}</span>
    <span class="m-spacer"></span>
    <span class="m-num m-lbl">{t("key.params")}</span>
  </button>
  {#if sel}
    <div class="who">
      <div class="line">
        <span class="m-name" data-test="params-name">{t(`effect.${sel.effect}`)}</span>
        <span class="m-lbl">{t("params.builtin")}</span>
      </div>
      <span class="m-lbl where" data-test="params-where"
        >{t("params.where", { strip: stripName(app.song, sel.insert.strip), slot: sel.insert.slot + 1 })}</span
      >
    </div>
    <label class="bar">
      <span class="m-lbl">{t("params.search")}</span>
      <input class="search" type="text" placeholder={t("params.searchLabel")} bind:value={query} data-test="params-search" />
      <span class="m-num m-lbl" data-test="params-count">{t("params.count", { shown: shownCount, total })}</span>
    </label>
    <ScrollArea data-test="params-list">
      <div role="presentation" {onkeydown}>
        {#each found as g, gi (g.group ?? `-${gi}`)}
          {#if g.group !== null}
            {@const isFolded = folded.includes(g.group)}
            <button class="m-plain group m-focus-in" onclick={() => toggleGroup(g.group!)} data-group={g.group}>
              <span class="m-hi">{isFolded ? "▸" : "▾"}</span>
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
              />
            {/each}
          {/if}
        {/each}
        {#if shownCount === 0 && query.trim() !== ""}
          <div class="none" data-test="params-none">
            <span>{t("msg.searchNoMatch", { query: query.trim() })}</span>
            <Button onclick={() => (query = "")} test="params-clear">{t("button.clearSearch")}</Button>
          </div>
        {/if}
      </div>
    </ScrollArea>
  {/if}
</aside>

<style>
  .panel {
    width: var(--size-shelf);
    flex-shrink: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-gray-3);
    border-left: var(--line-hair) solid var(--color-gray-6);
  }
  .head,
  .group {
    width: 100%;
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    white-space: nowrap;
  }
  .head:hover,
  .group:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .head:active {
    background: var(--color-gray-2);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .group {
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .group-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* 名前と形式と居場所（高さ 48。モック F4-Mixer: 左右 12、下に gray.5 の線）。 */
  .who {
    height: calc(2 * var(--size-row));
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    white-space: nowrap;
    overflow: hidden;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .where {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .bar {
    height: var(--mock-shelf-bar);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .search {
    flex: 1;
    min-width: 0;
    height: var(--part-input-height);
    padding: 0 var(--part-input-pad-x);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    color: var(--color-gray-10);
    font: inherit;
    outline: none;
  }
  .search::placeholder {
    color: var(--color-gray-7);
  }
  .search:focus {
    border-color: var(--color-gray-8);
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

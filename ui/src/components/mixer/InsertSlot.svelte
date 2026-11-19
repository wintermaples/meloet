<!-- インサートの枠（仕様書 2 章「インサートの枠」「インサートの枠 ・ 足した状態」「インサートの枠のオンの印」: 高さ 24、空きは「空き ＋」、えらんだ＝枠 accent）。
     入っている枠の左にオンの印（12×12。オンは accent の点、とばすと空で名前 gray.7）。押すと ToggleBypass。止まった枠と空きの枠には印なし。
     名前の場所は印のぶんせまく、入りきらない名前は「…」。乗ると名前の全部（とばしている枠は「（とばしている）」をそえて）。
     窓がひらいているプラグインは、右はしに窓の印。止まった物は赤い枠と「止」、見つからない物は名前を gray.8。
     入っている枠を押すと、その子をえらぶ（SelectInsert）。右クリックは、えらんでから一覧（insertMenu）。えらんでいる枠は Delete で外す。
     空きを押すと、棚が「（ストリップ）にエフェクトを入れる」でひらく。録音中 ・ カウントイン中も使える（A-14）。
     並べかえのドラッグは Strip が持つ（lift: 動かしている枠の上下のずれ、line: 入る所の線）。 -->
<script lang="ts">
  import type { InsertRef, InsertView } from "../../bridge";
  import { tipOf } from "../../lib/tip.svelte";
  import { send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { openContextMenu } from "../menu/contextMenu.svelte";
  import { insertName } from "../sounds";
  import { openShelf } from "../tracks.svelte";
  import WindowMark from "../WindowMark.svelte";
  import { insertMenu } from "./insertMenu";

  interface Props {
    insert: InsertRef;
    view: InsertView | null;
    selected: boolean;
    windowOpen?: boolean;
    lift?: number | null;
    line?: "above" | "below" | null;
    ondown?: (e: PointerEvent) => void;
  }
  let { insert, view, selected, windowOpen = false, lift = null, line = null, ondown }: Props = $props();

  const pluginState = $derived(view?.effect.kind === "plugin" ? view.effect.state : null);
  const marked = $derived(view !== null && pluginState !== "stopped");
  const name = $derived(view ? insertName(view) : "");

  let nameEl = $state<HTMLElement>();
  const tipText = () => {
    if (!view) return null;
    if (view.bypassed) return `${name}${t("mixer.bypassedTip")}`;
    return nameEl && nameEl.scrollWidth > nameEl.clientWidth ? name : null;
  };

  // えらんでいるプラグインの枠をもう一度押すと、窓を前に出す（仕様書 3 章「パラメータ」）。最初から入っている物は何もしない。
  function press(): void {
    if (view === null) send(openShelf({ kind: "insertEffect", strip: insert.strip, slot: insert.slot }));
    else if (!selected || view.effect.kind === "plugin") send({ type: "selectInsert", insert });
  }

  function context(e: MouseEvent): void {
    e.preventDefault();
    if (view === null) return;
    if (!selected) send({ type: "selectInsert", insert });
    openContextMenu(e.clientX, e.clientY, insertMenu(insert, view, send));
  }

  function key(e: KeyboardEvent): void {
    if (e.key !== "Delete" || e.shiftKey || e.ctrlKey || e.metaKey || e.altKey || view === null) return;
    e.preventDefault();
    // ほかの「消す」（えらんだクリップやノート）に渡さない。
    e.stopPropagation();
    if (selected) send({ type: "removeEffect", insert });
  }
</script>

<div
  class="slot"
  class:filled={view !== null}
  class:selected
  class:stopped={pluginState === "stopped"}
  class:missing={pluginState === "missing"}
  class:bypassed={view?.bypassed === true}
  class:lifted={lift !== null}
  style:transform={lift !== null ? `translateY(${lift}px)` : null}
  role="presentation"
  data-test="mixer-insert"
  data-slot={insert.slot}
  onpointerdown={(e) => view !== null && ondown?.(e)}
  onclick={press}
  onkeydown={key}
  oncontextmenu={context}
>
  {#if marked}
    <button
      class="m-plain mark"
      class:on={!view!.bypassed}
      role="switch"
      aria-checked={!view!.bypassed}
      aria-label={t("params.bypass")}
      data-test="mixer-bypass"
      onpointerdown={(e) => e.stopPropagation()}
      onclick={(e) => {
        e.stopPropagation();
        send({ type: "toggleBypass", insert });
      }}><span></span></button
    >
  {/if}
  <button class="m-plain press" use:tipOf={tipText}>
    {#if view !== null}
      <span class="name" bind:this={nameEl}>{name}</span>
      {#if pluginState === "stopped"}<span>{t("mixer.stoppedMark")}</span>{/if}
      {#if windowOpen}<WindowMark />{/if}
    {:else}
      <span class="name">{t("mixer.emptySlot")}</span><span>{t("mixer.emptySlotAdd")}</span>
    {/if}
  </button>
  {#if line}<div class="line {line}" data-test="mixer-insert-line"></div>{/if}
</div>

<style>
  .slot {
    position: relative;
    height: var(--part-insert-height);
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: 0 0 0 var(--space-1);
    border: var(--line-hair) solid var(--color-gray-5);
    background: var(--color-gray-1);
    color: var(--color-gray-8);
    white-space: nowrap;
  }
  .slot.filled {
    border-color: var(--color-gray-7);
    color: var(--color-gray-10);
  }
  .slot:hover {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .slot:active {
    background: var(--color-gray-2);
    border-color: var(--color-gray-8);
  }
  .slot.selected,
  .slot.selected:hover {
    border-color: var(--color-accent);
  }
  .slot.missing {
    color: var(--color-gray-8);
  }
  .slot.bypassed .name {
    color: var(--color-gray-7);
  }
  .slot.stopped,
  .slot.stopped:hover {
    border-color: var(--color-rec);
    color: var(--color-rec);
  }
  /* 並べかえのドラッグ中の枠は accent の line.hair でつまみ上げる（仕様書 2 章「インサートを並べかえる印」）。 */
  .slot.lifted {
    z-index: 2;
    outline: var(--line-hair) solid var(--color-accent);
    outline-offset: calc(-1 * var(--line-hair));
  }
  .press {
    flex: 1;
    min-width: 0;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-1);
    padding: 0 var(--space-1) 0 0;
    color: inherit;
    overflow: hidden;
  }
  .slot:not(.filled) .press {
    padding-left: 0;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    text-align: left;
  }
  /* オンの印（2 章の「選ぶ印」12×12）。オンは accent の点、とばすと空。 */
  .mark {
    flex-shrink: 0;
    box-sizing: border-box;
    width: var(--part-choice-size);
    height: var(--part-choice-size);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: var(--line-hair) solid var(--color-gray-8);
  }
  .mark span {
    width: var(--part-choice-dot);
    height: var(--part-choice-dot);
  }
  .mark.on {
    border-color: var(--color-accent);
  }
  .mark.on span {
    background: var(--color-accent);
  }
  /* 入る所: accent の line.mark の横線（トラックの並べかえと同じ）。枠と枠の間に重ねる。 */
  .line {
    position: absolute;
    left: calc(-1 * var(--line-hair));
    right: calc(-1 * var(--line-hair));
    height: var(--line-mark);
    background: var(--color-accent);
    pointer-events: none;
    z-index: 3;
  }
  .line.above {
    top: calc(-1 * (var(--space-1) / 4 + var(--line-mark) / 2 + var(--line-hair)));
  }
  .line.below {
    bottom: calc(-1 * (var(--space-1) / 4 + var(--line-mark) / 2 + var(--line-hair)));
  }
</style>

<!-- インサートの枠（仕様書 2 章「インサートの枠」: 高さ 24、空きは「空き ＋」、えらんだ＝枠 accent）。
     入っている枠を押すと、その子をえらぶ（SelectInsert）。右クリックは、えらんでから一覧（「外す」。B-86）。えらんでいる枠は Delete でも外す。
     空きを押すと、棚が「（ストリップ）にエフェクトを入れる」でひらく。録音中 ・ カウントイン中も使える（A-14）。 -->
<script lang="ts">
  import type { EffectKind, InsertRef } from "../../bridge";
  import { send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { openContextMenu } from "../menu/contextMenu.svelte";
  import { openShelf } from "../tracks.svelte";
  import { insertMenu } from "./insertMenu";

  let { insert, effect, selected }: { insert: InsertRef; effect: EffectKind | null; selected: boolean } = $props();

  function press(): void {
    if (effect === null) send(openShelf({ kind: "insertEffect", strip: insert.strip, slot: insert.slot }));
    else if (!selected) send({ type: "selectInsert", insert });
  }

  function context(e: MouseEvent): void {
    e.preventDefault();
    if (effect === null) return;
    if (!selected) send({ type: "selectInsert", insert });
    openContextMenu(e.clientX, e.clientY, insertMenu(insert, send));
  }

  function key(e: KeyboardEvent): void {
    if (e.key !== "Delete" || e.shiftKey || e.ctrlKey || e.metaKey || e.altKey || effect === null) return;
    e.preventDefault();
    // ほかの「消す」（えらんだクリップやノート）に渡さない。
    e.stopPropagation();
    if (selected) send({ type: "removeEffect", insert });
  }
</script>

<button
  class="m-plain slot"
  class:filled={effect !== null}
  class:selected
  data-test="mixer-insert"
  data-slot={insert.slot}
  onclick={press}
  oncontextmenu={context}
  onkeydown={key}
>
  {#if effect !== null}
    <span class="name">{t(`effect.${effect}`)}</span>
  {:else}
    <span class="name">{t("mixer.emptySlot")}</span><span>{t("mixer.emptySlotAdd")}</span>
  {/if}
</button>

<style>
  .slot {
    height: var(--part-insert-height);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-1);
    padding: 0 var(--space-1);
    border: var(--line-hair) solid var(--color-gray-5);
    background: var(--color-gray-1);
    color: var(--color-gray-8);
    white-space: nowrap;
    overflow: hidden;
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
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>

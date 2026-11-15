<!-- アレンジの上の案内（置き場はアレンジの時間の面、position: relative の中）。
     「音が出ました」: 仕様書 6 章「音が出た」、モック F-Connect3。「からっぽのアレンジ」: モック F-Empty。 -->
<script lang="ts">
  import { current } from "../state/app.svelte";
  import { t } from "../text/ja";
  import { textParts } from "./format";

  const hint = $derived(current.state?.workspace.hint ?? null);
  /**
   * 2 文目より後は、録音ボタンが使えるときだけ出す（仕様書 6 章、3 章「案内の文は、いま使えない部品を押すようには言わない」）。
   * F-Empty の 3 文目（トラックを増やすには…）は、トラックを追加がまだ使えないので出さない。
   */
  const recordUsable = $derived(current.state?.transport.canRecord ?? false);
  const keys = $derived(textParts("arrange.soundCameOutKeys", { recordKey: t("key.record"), playKey: t("key.playStop") }));
</script>

{#if hint === "soundCameOut"}
  <div class="hint" data-test="sound-hint">
    <span class="strong">{t("msg.soundCameOut")}</span>{#if recordUsable}{" "}{t("msg.soundCameOutRecord")}{" "}<span
        class="dim"
        >{#each keys as part, i (i)}{#if part.hole !== null}<span class="m-num">{part.text}</span>{:else}{part.text}{/if}{/each}</span
      >{/if}
  </div>
{:else if hint === "emptyArrange"}
  <div class="hint" data-test="empty-hint">
    <span class="strong">{t("arrange.emptyTitle")}</span>
    {t("arrange.emptyBody")}
  </div>
{/if}

<style>
  .hint {
    position: absolute;
    left: var(--mock-arrange-note-left);
    top: var(--mock-arrange-note-top);
    z-index: 1;
    box-sizing: border-box;
    width: var(--mock-sound-hint-width);
    padding: var(--space-3);
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-7);
    color: var(--color-gray-9);
    font-family: var(--font-sans);
    font-size: var(--text-s-size);
    line-height: var(--text-s-line-height);
    font-weight: var(--text-s-weight-400);
    white-space: normal;
  }
  .strong {
    color: var(--color-gray-10);
  }
  .dim {
    color: var(--color-gray-8);
  }
</style>

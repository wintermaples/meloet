<!-- 「音が出ました」（仕様書 6 章「音が出た」、モック F-Connect3）。置き場はアレンジの時間の面（position: relative）の中。 -->
<script lang="ts">
  import { current } from "../state/app.svelte";
  import { t } from "../text/ja";

  /** 2 文目は、録音ボタンが使えるときだけ出す（仕様書 6 章、3 章「案内の文は、いま使えない部品を押すようには言わない」）。 */
  let { recordUsable = false }: { recordUsable?: boolean } = $props();

  const show = $derived(current.state?.workspace.hint === "soundCameOut");
</script>

{#if show}
  <div class="hint" data-test="sound-hint">
    <span class="strong">{t("msg.soundCameOut")}</span>{#if recordUsable}{t("msg.soundCameOutRecord")}{/if}
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
</style>

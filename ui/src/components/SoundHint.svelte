<!-- 「音が出ました」（仕様書 6 章「音が出た」、モック F-Connect3）。置き場はアレンジの時間の面（position: relative）の中。 -->
<script lang="ts">
  import { current } from "../state/app.svelte";
  import { t } from "../text/ja";

  const show = $derived(current.state?.workspace.hint === "soundCameOut");
  const parts = $derived.by(() => {
    const text = t("msg.soundCameOut");
    const i = text.indexOf("。") + 1;
    return [text.slice(0, i), text.slice(i)] as const;
  });
</script>

{#if show}
  <div class="hint" data-test="sound-hint"><span class="strong">{parts[0]}</span>{parts[1]}</div>
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

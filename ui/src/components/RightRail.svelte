<!-- 右はし: ひらいた欄（棚かパラメータ。前に出ている物だけ）と、たたんだ物の縦の見出し。
     モック F-Empty（縦の見出し）、A-Mixer・F-ParamFew（ひらいた欄の見出しの行）。 -->
<script lang="ts">
  import type { Band } from "../bridge";
  import { current, send } from "../state/app.svelte";
  import { t } from "../text/ja";

  const layout = $derived(current.state!.workspace.layout);
  const front = $derived(layout.rightFront);

  const RIGHT: Band[] = ["shelf", "params"];
  const tabs = $derived(RIGHT.filter((b) => b !== front));
  const toggle = (band: Band) => send({ type: "toggleBand", band });
</script>

{#snippet title(band: Band)}
  <span class="m-name">{band === "shelf" ? t("band.shelf") : t("band.params")}</span>
{/snippet}

{#if front}
  <aside class="panel" data-band={front}>
    <div class="head">
      <span class="m-hi">▾</span>
      {@render title(front)}
      <span class="m-spacer"></span>
      <span class="m-num m-lbl">{front === "shelf" ? t("key.shelf") : t("key.params")}</span>
    </div>
  </aside>
{/if}

<nav class="tabs">
  {#each tabs as band, i (band)}
    {#if i > 0}<div class="rule"></div>{/if}
    <button class="m-plain tab" onclick={() => toggle(band)} data-band={band}>
      <span class="m-lbl">▸</span>
      <span class="vt">{@render title(band)}</span>
      {#if band === "shelf"}
        <span class="vt m-lbl">{t("band.shelfCount", { instruments: 0, effects: 0 })}</span>
      {/if}
      <span class="m-num m-lbl">{band === "shelf" ? t("key.shelf") : t("key.params")}</span>
    </button>
  {/each}
</nav>

<style>
  .panel {
    width: var(--size-shelf);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-gray-3);
    border-left: var(--line-hair) solid var(--color-gray-6);
  }
  .head {
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    white-space: nowrap;
  }
  .tabs {
    width: var(--size-tab-v);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-gray-4);
    border-left: var(--line-hair) solid var(--color-gray-6);
  }
  .tab {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) 0;
  }
  .tab:hover .m-name,
  .tab:hover .m-lbl {
    color: var(--color-gray-10);
  }
  .rule {
    height: var(--line-hair);
    flex-shrink: 0;
    background: var(--color-gray-6);
  }
  .vt {
    writing-mode: vertical-rl;
    white-space: nowrap;
  }
</style>

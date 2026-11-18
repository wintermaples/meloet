<!-- 右はし: ひらいた欄（棚かパラメータ。前に出ている物だけ）と、たたんだ物の縦の見出し。
     モック F-Empty（縦の見出し）、A-Mixer・F-ParamFew（ひらいた欄の見出しの行）。 -->
<script lang="ts">
  import type { Band } from "../bridge";
  import { current, send } from "../state/app.svelte";
  import { t } from "../text/ja";
  import { metrics } from "./metrics.svelte";

  const layout = $derived(current.state!.workspace.layout);
  const front = $derived(layout.rightFront);

  const RIGHT: Band[] = ["shelf", "params"];
  const tabs = $derived(RIGHT.filter((b) => b !== front));
  const toggle = (band: Band) => send({ type: "toggleBand", band });

  // When the window is too low for every vertical heading, the shelf counts go first.
  let nav = $state<HTMLElement>();
  let countsFit = $state(true);
  let heightWithCounts = 0;

  function fitCounts() {
    if (!nav) return;
    if (countsFit) {
      heightWithCounts = nav.scrollHeight;
      if (heightWithCounts > nav.clientHeight) countsFit = false;
    } else if (heightWithCounts <= nav.clientHeight) {
      countsFit = true;
    }
  }

  $effect(() => {
    void metrics.scale;
    countsFit = true;
  });

  $effect(() => {
    void tabs;
    if (!nav) return;
    const observer = new ResizeObserver(fitCounts);
    observer.observe(nav);
    for (const tab of nav.children) observer.observe(tab);
    return () => observer.disconnect();
  });
</script>

{#snippet title(band: Band)}
  <span class="m-name">{band === "shelf" ? t("band.shelf") : t("band.params")}</span>
{/snippet}

{#if front}
  <aside class="panel" data-band={front}>
    <button class="m-plain head m-focus-in" onclick={() => toggle(front)} data-test="rail-fold">
      <span class="m-hi">▾</span>
      {@render title(front)}
      <span class="m-spacer"></span>
      <span class="m-num m-lbl">{front === "shelf" ? t("key.shelf") : t("key.params")}</span>
    </button>
  </aside>
{/if}

<nav class="tabs" bind:this={nav}>
  {#each tabs as band, i (band)}
    {#if i > 0}<div class="rule"></div>{/if}
    <button class="m-plain tab m-focus-in" onclick={() => toggle(band)} data-band={band}>
      <span class="m-lbl">▸</span>
      <span class="vt">{@render title(band)}</span>
      {#if band === "shelf" && countsFit}
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
  .tabs {
    width: var(--size-tab-v);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-gray-4);
    border-left: var(--line-hair) solid var(--color-gray-6);
    overflow: hidden;
  }
  .tab {
    flex-shrink: 0;
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
  /* 押した（仕様書 2 章: bg gray.2 ・ 枠 gray.8。枠は内がわに描く）。ひらいた欄の見出し（押すとたたむ。B-74）は、乗ったときも枠。 */
  .head:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .tab:active,
  .head:active {
    background: var(--color-gray-2);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
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

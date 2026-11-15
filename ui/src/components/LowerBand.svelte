<!-- ひらいた下の帯（ピアノロール、ミキサー、取り消しの履歴）。ピアノロールは中身まで（pianoroll/PianoRoll.svelte）。
     ほかは見出しの行だけで、中身は空。見出しの行は A-Mixer・F-History の帯のいちばん上の行。 -->
<script lang="ts">
  import type { Band } from "../bridge";
  import { t, type TextKey } from "../text/ja";
  import { metrics } from "./metrics.svelte";
  import PianoRoll from "./pianoroll/PianoRoll.svelte";

  let { band, height }: { band: Band; height: number } = $props();

  const NAMES: Partial<Record<Band, TextKey>> = {
    pianoRoll: "band.pianoRoll",
    mixer: "band.mixer",
    history: "band.history",
  };
  const name = $derived(NAMES[band]);
</script>

{#if band === "pianoRoll"}
  <PianoRoll {height} />
{:else}
  <section class="band" style:height="{metrics.scaled(height)}px" data-band={band}>
    <div class="head">
      <span class="m-hi">▾</span>
      {#if name}<span class="m-name">{t(name)}</span>{/if}
    </div>
    <div class="body"></div>
  </section>
{/if}

<style>
  .band {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--color-gray-2);
  }
  .head {
    height: var(--size-toolbar);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    border-bottom: var(--line-hair) solid var(--color-gray-6);
    white-space: nowrap;
  }
  .body {
    flex: 1;
    min-height: 0;
  }
</style>

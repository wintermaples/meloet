<!-- ひらいた下の帯（ピアノロール、ミキサー、取り消しの履歴）。ピアノロールは中身まで（pianoroll/PianoRoll.svelte）。
     ミキサーは、道具の列（音の流れの文、メーターの単位。モック F4-Mixer）とストリップ（mixer/MixerStrips.svelte）。取り消しの履歴は history/HistoryHead.svelte と history/HistoryList.svelte。見出しの行は A-Mixer・F-History の帯のいちばん上の行。「▾ 名前」を押すと、その帯をたたむ（B-74）。 -->
<script lang="ts">
  import type { Band } from "../bridge";
  import { term } from "../lib/tip.svelte";
  import { send } from "../state/app.svelte";
  import { t, type TextKey } from "../text/ja";
  import HistoryHead from "./history/HistoryHead.svelte";
  import HistoryList from "./history/HistoryList.svelte";
  import { metrics } from "./metrics.svelte";
  import MixerStrips from "./mixer/MixerStrips.svelte";
  import PianoRoll from "./pianoroll/PianoRoll.svelte";

  let { band, height }: { band: Band; height: number } = $props();

  const NAMES: Partial<Record<Band, TextKey>> = {
    pianoRoll: "band.pianoRoll",
    mixer: "band.mixer",
    history: "band.history",
  };
  const name = $derived(NAMES[band]);
  /** 用語の説明がある帯の名前（仕様書 5 章。「取り消しの履歴」は 5 章にない）。 */
  const TERMS: Partial<Record<Band, string>> = { mixer: "ミキサー" };
</script>

{#if band === "pianoRoll"}
  <PianoRoll {height} />
{:else}
  <section class="band" style:height="{metrics.scaled(height)}px" data-band={band}>
    <div class="head">
      <button class="m-plain fold m-focus-in" onclick={() => send({ type: "toggleBand", band })} data-test="band-fold">
        <span class="m-hi">▾</span>
        {#if name}
          {#if TERMS[band]}<span class="m-name" use:term={TERMS[band]!}>{t(name)}</span>{:else}<span class="m-name">{t(name)}</span>{/if}
        {/if}
      </button>
      {#if band === "mixer"}
        <span class="m-lbl">{t("mixer.flow")}</span>
        <span class="m-spacer"></span>
        <span class="m-lbl">{t("mixer.meterUnit")} <span class="m-num" use:term={"dB"}>{t("mixer.db")}</span></span>
      {:else if band === "history"}
        <HistoryHead />
      {/if}
    </div>
    <div class="body">
      {#if band === "mixer"}<MixerStrips />{:else if band === "history"}<HistoryList />{/if}
    </div>
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
  /* 押せる所は帯の左はしから。状態は仕様書 2 章のボタン（枠は内がわに描く）。 */
  .fold {
    align-self: stretch;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    margin: 0 calc(-1 * var(--space-2)) 0 calc(-1 * var(--space-3));
    padding: 0 var(--space-2) 0 var(--space-3);
  }
  .fold:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .fold:active {
    background: var(--color-gray-2);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .body {
    flex: 1;
    min-height: 0;
  }
</style>

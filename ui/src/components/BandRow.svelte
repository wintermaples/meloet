<!-- 帯札の行（仕様書 2 章「帯札」、3 章。モック F-Empty・A-PianoRoll の下から 2 行目）。 -->
<script lang="ts">
  import type { Band } from "../bridge";
  import { current, send } from "../state/app.svelte";
  import { t } from "../text/ja";
  import { bandCountsShown, bandTabWidth, textWidth } from "./fit";
  import { dbNumber } from "./format";
  import { metrics } from "./metrics.svelte";

  type Tab = { band: Band | "arrange"; name: string; count: string; key: string; open: boolean };

  const app = $derived(current.state!);
  const lower = $derived(app.workspace.layout.lower.map((l) => l.band));

  const tabs: Tab[] = $derived([
    { band: "arrange", name: t("band.arrange"), count: t("band.trackCount", { count: app.song.tracks.length }), key: t("key.arrange"), open: lower.length === 0 },
    { band: "pianoRoll", name: t("band.pianoRoll"), count: t("band.noteCount", { count: app.song.noteCount }), key: t("key.pianoRoll"), open: lower.includes("pianoRoll") },
    // マスターのフェーダーの値。今回は 0.0 dB のまま（docs/progress.md）。
    { band: "mixer", name: t("band.mixer"), count: t("band.masterDb", { db: t("common.db", { value: dbNumber(0) }) }), key: t("key.mixer"), open: lower.includes("mixer") },
    { band: "history", name: t("band.history"), count: t("band.stepCount", { count: app.song.undoCount }), key: t("key.history"), open: lower.includes("history") },
  ]);

  const backHint = $derived(app.workspace.layout.showBackHint ? t("msg.backToArrange") : null);

  let width = $state(0);
  const shown = $derived(
    bandCountsShown(
      width,
      metrics.scale,
      tabs.map((tab) => bandTabWidth(tab.name, tab.count, tab.key)),
      backHint ? textWidth(backHint) + 12 * 2 : 0,
    ),
  );

  function press(tab: Tab) {
    if (tab.band === "arrange") send({ type: "goArrange" });
    else send({ type: "toggleBand", band: tab.band });
  }
</script>

<nav class="row" bind:clientWidth={width}>
  {#each tabs as tab, i (tab.band)}
    <button class="m-plain tab" class:open={tab.open} onclick={() => press(tab)} data-band={tab.band}>
      <span class={tab.open ? "m-hi" : "m-lbl"}>{tab.open ? "▾" : "▸"}</span>
      <span class="m-name">{tab.name}</span>
      {#if shown[i]}<span class="m-lbl">{tab.count}</span>{/if}
      <span class="m-num m-lbl">{tab.key}</span>
    </button>
  {/each}
  <span class="m-spacer"></span>
  {#if backHint}<span class="m-lbl hint">{backHint}</span>{/if}
</nav>

<style>
  .row {
    height: var(--size-bandrow);
    flex-shrink: 0;
    display: flex;
    align-items: stretch;
    background: var(--color-gray-4);
    border-top: var(--line-hair) solid var(--color-gray-6);
    white-space: nowrap;
    overflow: hidden;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex-shrink: 0;
    padding: 0 var(--space-3);
    border-top: var(--line-mark) solid transparent;
    border-right: var(--line-hair) solid var(--color-gray-6);
  }
  .tab:hover .m-name {
    color: var(--color-gray-10);
  }
  .tab.open {
    background: var(--color-gray-1);
    border-top-color: var(--color-accent);
  }
  .hint {
    align-self: center;
    padding-right: var(--space-3);
  }
</style>

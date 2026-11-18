<!-- 帯札の行（仕様書 2 章「帯札」、3 章。モック F-Empty・A-PianoRoll の下から 2 行目）。 -->
<script lang="ts">
  import type { Band } from "../bridge";
  import { space } from "../design/tokens";
  import { term } from "../lib/tip.svelte";
  import { current, send } from "../state/app.svelte";
  import { t } from "../text/ja";
  import { bandCountsShown, bandTabWidth, textWidth } from "./fit";
  import { dbNumber, termParts } from "./format";
  import { metrics } from "./metrics.svelte";

  /** term は名前の用語の説明、countTerm は件数の文の中の用語（仕様書 5 章にある物だけ）。 */
  type Tab = { band: Band | "arrange"; name: string; count: string; key: string; open: boolean; term?: string; countTerm?: string };

  const app = $derived(current.state!);
  const lower = $derived(app.workspace.layout.lower.map((l) => l.band));

  const tabs: Tab[] = $derived([
    { band: "arrange", name: t("band.arrange"), count: t("band.trackCount", { count: app.song.tracks.length }), key: t("key.arrange"), open: lower.length === 0, term: "アレンジ" },
    {
      band: "pianoRoll",
      name: t("band.pianoRoll"),
      count: t("band.noteCount", { count: app.song.noteCount }),
      key: t("key.pianoRoll"),
      open: lower.includes("pianoRoll"),
      term: "ピアノロール",
      countTerm: "ノート",
    },
    // マスターのフェーダーの値。今回は 0.0 dB のまま（docs/progress.md）。
    {
      band: "mixer",
      name: t("band.mixer"),
      count: t("band.masterDb", { db: t("common.db", { value: dbNumber(0) }) }),
      key: t("key.mixer"),
      open: lower.includes("mixer"),
      term: "ミキサー",
    },
    { band: "history", name: t("band.history"), count: t("band.stepCount", { undo: app.song.undoCount, total: app.song.undoCount + app.song.redoCount }), key: t("key.history"), open: lower.includes("history") },
  ]);

  const backHint = $derived(app.workspace.layout.showBackHint ? t("msg.backToArrange") : null);

  let width = $state(0);
  const shown = $derived(
    bandCountsShown(
      width,
      metrics.scale,
      tabs.map((tab) => bandTabWidth(tab.name, tab.count, tab.key)),
      backHint ? textWidth(backHint) + space["space.3"][0] * 2 : 0,
    ),
  );

  function press(tab: Tab) {
    if (tab.band === "arrange") send({ type: "goArrange" });
    else send({ type: "toggleBand", band: tab.band });
  }
</script>

<nav class="row" bind:clientWidth={width}>
  {#each tabs as tab, i (tab.band)}
    <button class="m-plain tab m-focus-in" class:open={tab.open} onclick={() => press(tab)} data-band={tab.band}>
      <span class={tab.open ? "m-hi" : "m-lbl"}>{tab.open ? "▾" : "▸"}</span>
      {#if tab.term}<span class="m-name" use:term={tab.term}>{tab.name}</span>{:else}<span class="m-name">{tab.name}</span>{/if}
      {#if shown[i]}
        <span class="m-lbl"
          >{#if tab.countTerm}{#each termParts(tab.count, tab.countTerm) as piece, j (j)}{#if piece.term}<span use:term={tab.countTerm!}
                  >{piece.text}</span
                >{:else}{piece.text}{/if}{/each}{:else}{tab.count}{/if}</span
        >
      {/if}
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
  /* 押した（仕様書 2 章: bg gray.2 ・ 枠 gray.8。枠は内がわに描く）。 */
  .tab:active,
  .tab.open:active {
    background: var(--color-gray-2);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .hint {
    align-self: center;
    padding-right: var(--space-3);
  }
</style>

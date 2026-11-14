<!-- アレンジ（モック F-Empty・F-Connect1）。ちぢんだときは全体図（A-PianoRoll の上の帯）。
     小節の線と全体図は WebGL が描く。children は、アレンジの上に重ねる物（出口えらび、案内）。 -->
<script lang="ts">
  import type { Snippet } from "svelte";
  import { glSlot } from "../gl/slot";
  import { term } from "../lib/tip.svelte";
  import { current } from "../state/app.svelte";
  import { t } from "../text/ja";
  import { dbNumber, tempoText } from "./format";
  import { metrics } from "./metrics.svelte";

  let { children }: { children?: Snippet } = $props();

  const song = $derived(current.state!.song);
  const layout = $derived(current.state!.workspace.layout);
  const height = $derived(metrics.scaled(layout.arrangeHeight));

  let timeWidth = $state(0);
  const barPx = $derived(metrics.px("mock.arrange.bar"));
  const bars = $derived(Math.max(1, Math.ceil(timeWidth / barPx)));
</script>

{#if layout.arrangeOverview}
  <section class="overview" style:height="{height}px">
    <div class="overview-label"><span class="m-lbl">{t("arrange.overview")}</span></div>
    <div class="overview-slot" use:glSlot={{ kind: "overview" }}></div>
    <div class="layer">{@render children?.()}</div>
  </section>
{:else}
  <section class="arrange" style:height="{height}px">
    <div class="head">
      <div class="row between">
        <span class="m-name">{t("arrange.title")}</span>
        <span class="off" use:term={"グリッド"}>{t("arrange.grid")} <span class="m-num">1/16</span></span>
      </div>
      <div class="row"><span class="m-lbl" use:term={"マーカー"}>{t("arrange.marker")}</span></div>
      <button class="m-plain row tempo-head" aria-disabled="true" tabindex="-1">
        <span>▸</span>
        <span>{t("arrange.tempoChanges")}</span>
        <span class="m-spacer"></span>
        <span class="m-num">{t("arrange.changeCount", { count: song.tempoChanges })}</span>
      </button>
      {#each song.tracks as track (track.number)}
        <div class="track-head">
          <div class="track-color" style:background="var(--color-track-{((track.color - 1) % 5) + 1})"></div>
          <div class="track-body">
            <div class="line">
              <span class="m-num m-lbl track-number">{track.number}</span>
              <span class="m-name track-name">{track.name}</span>
              <button class="tog" aria-disabled="true" tabindex="-1" aria-label={t("arrange.mute")} use:term={"ミュート"}>{t("arrange.muteShort")}</button>
              <button class="tog" aria-disabled="true" tabindex="-1" aria-label={t("arrange.solo")} use:term={"ソロ"}>{t("arrange.soloShort")}</button>
              <button class="tog" aria-disabled="true" tabindex="-1">{t("arrange.armShort")}</button>
            </div>
            <div class="line gap2">
              <span class="m-lbl instrument" use:term={"仮の音"}>{t("instrument.placeholder")}</span>
              <span class="m-num">{t("common.db", { value: dbNumber(track.gainDb) })}</span>
            </div>
            <div class="meter" use:glSlot={{ kind: "meter", source: { track: track.number }, gap: metrics.px("line.hair") }}></div>
          </div>
        </div>
      {/each}
      <button class="m-plain add" aria-disabled="true" tabindex="-1">
        {t("arrange.addTrack")}<span>{t("arrange.addTrackKinds")}</span>
      </button>
    </div>

    <div class="time" bind:clientWidth={timeWidth}>
      <div class="ruler">
        {#each { length: bars }, i (i)}
          <span class="m-num m-lbl bar-number" style:left="{i * barPx}px">{i + 1}</span>
        {/each}
      </div>
      <div class="ruler"></div>
      <div class="ruler">
        <span class="m-num m-lbl bar-number">
          {t("arrange.tempoAndSignature", { tempo: tempoText(song.tempoBpm), signature: `${song.meterNum}/${song.meterDen}` })}
        </span>
      </div>
      <div class="lanes" use:glSlot={{ kind: "arrangeGrid", barPx, firstBarX: 0 }}>
        {#each song.tracks as track (track.number)}
          <div class="lane"></div>
        {/each}
      </div>
      <div class="layer">{@render children?.()}</div>
    </div>
  </section>
{/if}

<style>
  .arrange,
  .overview {
    flex-shrink: 0;
    display: flex;
    overflow: hidden;
    position: relative;
  }
  .layer {
    position: absolute;
    inset: 0;
    z-index: 20;
    pointer-events: none;
  }
  .layer > :global(*) {
    pointer-events: auto;
  }

  .overview {
    background: var(--color-gray-3);
    border-bottom: var(--line-hair) solid var(--color-gray-6);
  }
  .overview-label {
    width: var(--mock-overview-label);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    padding: 0 var(--space-3);
    overflow: hidden;
    border-right: var(--line-hair) solid var(--color-gray-6);
  }
  .overview-slot {
    flex: 1;
    min-width: 0;
  }

  .head {
    width: var(--size-col-head);
    flex-shrink: 0;
    background: var(--color-gray-3);
    border-right: var(--line-hair) solid var(--color-gray-6);
    overflow: hidden;
  }
  .row {
    height: var(--size-row);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-2);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    white-space: nowrap;
  }
  .row.between {
    justify-content: space-between;
  }
  .off {
    color: var(--color-gray-7);
    white-space: nowrap;
  }
  .tempo-head {
    width: 100%;
    background: var(--color-gray-4);
    color: var(--color-gray-7);
  }

  .track-head {
    height: var(--size-track);
    display: flex;
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .track-color {
    width: var(--line-track);
    flex-shrink: 0;
  }
  .track-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-2) 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  .line.gap2 {
    gap: var(--space-2);
  }
  .track-number {
    width: var(--mock-track-number);
    flex-shrink: 0;
  }
  .track-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .instrument {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tog {
    width: var(--part-toggle-size);
    height: var(--part-toggle-size);
    flex-shrink: 0;
    padding: 0;
    border: var(--line-hair) solid var(--color-gray-5);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-gray-7);
    font: inherit;
    text-align: center;
    cursor: default;
  }
  .meter {
    height: calc(var(--part-meter-horizontal) * 2 + var(--line-hair));
  }
  .add {
    width: 100%;
    height: var(--mock-arrange-add-track);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-2);
    color: var(--color-gray-7);
    cursor: default;
  }

  .time {
    flex: 1;
    min-width: 0;
    position: relative;
    display: flex;
    flex-direction: column;
    background: var(--color-gray-1);
  }
  .ruler {
    height: var(--size-row);
    flex-shrink: 0;
    position: relative;
    background: var(--color-gray-3);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .bar-number {
    position: absolute;
    top: var(--space-1);
    margin-left: var(--space-1);
  }
  .lanes {
    flex: 1;
    min-height: 0;
  }
  .lane {
    height: var(--size-track);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
</style>

<!-- 再生まわりの帯（モック F-Connect1・F-Empty・Main の上の帯）。再生と録音はまだ使えない。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import { glSlot } from "../gl/slot";
  import { term } from "../lib/tip.svelte";
  import { current, onTelemetry } from "../state/app.svelte";
  import { t } from "../text/ja";
  import { transportParts } from "./fit";
  import { PeakHold, peakText, tempoText } from "./format";
  import { metrics } from "./metrics.svelte";

  const song = $derived(current.state!.song);
  let width = $state(0);
  const parts = $derived(transportParts(width, metrics.scale));

  const load = $derived(Math.round((current.telemetry?.loadPermille ?? 0) / 10));

  // マスターの dB の数字: 直近 1 秒のいちばん大きい値。音が止まると計器は送られてこないので、時計でも見なおす。
  const hold = new PeakHold(1000);
  let masterPeak = $state(0);
  onMount(() => {
    const off = onTelemetry((tm) => {
      hold.push(Math.max(tm.masterPeak[0], tm.masterPeak[1]), performance.now());
      masterPeak = hold.max(performance.now());
    });
    const timer = setInterval(() => (masterPeak = hold.max(performance.now())), 250);
    return () => {
      off();
      clearInterval(timer);
    };
  });
</script>

<header class="transport" bind:clientWidth={width}>
  {#if parts.songName}
    <div class="song">
      <span class="m-name ellipsis">{song.name}</span>
      <span class="m-lbl ellipsis">{t("transport.songNew")}</span>
    </div>
  {/if}

  <div class="buttons">
    <button class="m-btn" aria-disabled="true" aria-label={t("transport.toStart")} tabindex="-1">
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 0h2v10H0zM10 0v10L3 5z" fill="currentColor" /></svg>
    </button>
    <button class="m-btn" aria-disabled="true" aria-label={t("transport.play")} tabindex="-1">
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 0l8 5-8 5z" fill="currentColor" /></svg>
    </button>
    <button class="m-btn" aria-disabled="true" aria-label={t("transport.stop")} tabindex="-1">
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1h8v8H1z" fill="currentColor" /></svg>
    </button>
    <button class="m-btn" aria-disabled="true" aria-label={t("transport.record")} tabindex="-1">
      <span class="rec"></span>
    </button>
    <button class="m-btn" aria-disabled="true" tabindex="-1" use:term={"ループ"}>{t("transport.loop")}</button>
  </div>

  <div class="position">
    <div class="col">
      {#if parts.positionLabels}<span class="m-lbl" use:term={"小節 . 拍 . ティック"}>{t("transport.position")}</span>{/if}
      <span class="m-readout">1. 1. 000</span>
    </div>
    <div class="col">
      {#if parts.positionLabels}<span class="m-lbl">{t("transport.time")}</span>{/if}
      <span class="m-readout">00:00:00.000</span>
    </div>
  </div>

  <div class="col">
    <span class="m-lbl" use:term={"テンポ"}>{t("transport.tempo")}</span>
    <span class="m-readout">{tempoText(song.tempoBpm)}</span>
  </div>
  <div class="col">
    <span class="m-lbl" use:term={"拍子"}>{t("transport.timeSignature")}</span>
    <span class="m-readout">{song.meterNum}/{song.meterDen}</span>
  </div>

  <button class="m-btn" aria-disabled="true" tabindex="-1" use:term={"メトロノーム"}>{t("transport.metronome")}</button>
  {#if parts.countIn}
    <button class="m-btn" aria-disabled="true" tabindex="-1" use:term={"カウントイン"}>
      {t("transport.countIn")}<span class="m-num">{t("transport.countInBars", { bars: 1 })}</span>
    </button>
  {/if}

  <div class="m-spacer"></div>

  {#if parts.load}
    <div class="col end">
      <span class="m-lbl">{t("transport.cpu")}</span>
      <span class="m-num">{t("transport.cpuValue", { percent: load })}</span>
    </div>
  {/if}

  <div class="master">
    <div class="master-head">
      <span class="m-lbl" use:term={"マスター"}>{t("transport.master")}</span>
      <span class="m-num master-db">{peakText(masterPeak)}</span>
    </div>
    <div class="meter" use:glSlot={{ kind: "meter", source: "master", gap: metrics.px("space.1") }}></div>
  </div>
</header>

<style>
  .transport {
    height: var(--size-transport);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 0 var(--space-3);
    background: var(--color-gray-3);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    white-space: nowrap;
    overflow: hidden;
  }
  .song {
    width: var(--mock-transport-name);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .ellipsis {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .buttons {
    display: flex;
    gap: var(--space-1);
  }
  .rec {
    width: var(--part-rec-dot);
    height: var(--part-rec-dot);
    border-radius: var(--radius-round);
    border: var(--line-mark) solid currentColor;
  }
  .position {
    height: var(--mock-transport-readout);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: 0 var(--space-3);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-5);
  }
  .col {
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
  }
  .col.end {
    align-items: flex-end;
  }
  .master {
    width: var(--mock-transport-master);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .master-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .master-db {
    font-size: var(--text-m-size);
    line-height: var(--text-m-line-height);
    font-weight: var(--text-m-weight-400);
  }
  .meter {
    height: calc(var(--part-meter-horizontal) * 2 + var(--space-1));
  }
</style>

<!-- 画面の並び（仕様書 3 章）: 上から、再生まわり、入力計、アレンジ、ひらいた下の帯、帯札の行、状態行。
     右はしに、ひらいた欄と縦の見出し。重なりの順は、画面の部品（WebGL の面は、それぞれの箱の中）< 案内・出口えらび（20）
     < ダイアログ（30）< 用語の説明。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import Arrange from "./components/Arrange.svelte";
  import BandRow from "./components/BandRow.svelte";
  import InputMeter from "./components/InputMeter.svelte";
  import LowerBand from "./components/LowerBand.svelte";
  import { metrics, watchDevicePixelRatio } from "./components/metrics.svelte";
  import OutputChooser from "./components/OutputChooser.svelte";
  import RightRail from "./components/RightRail.svelte";
  import SettingsDialog from "./components/SettingsDialog.svelte";
  import SoundHint from "./components/SoundHint.svelte";
  import StatusLine from "./components/StatusLine.svelte";
  import Transport from "./components/Transport.svelte";
  import { writeCssVariables } from "./design/tokens";
  import Surface from "./gl/Surface.svelte";
  import { attachKeys } from "./keys/attach";
  import Tip from "./lib/Tip.svelte";
  import { current, send, start } from "./state/app.svelte";

  const state = $derived(current.state);

  $effect(() => writeCssVariables(metrics.scale, metrics.dpr));

  const sendViewport = () => {
    if (current.connected) send({ type: "setViewport", width: window.innerWidth, height: window.innerHeight });
  };
  $effect(() => {
    if (current.connected) sendViewport();
  });

  onMount(() => {
    start();
    const stopDpr = watchDevicePixelRatio();
    const stopKeys = attachKeys({ state: () => current.state, send });
    window.addEventListener("resize", sendViewport);
    return () => {
      stopDpr();
      stopKeys();
      window.removeEventListener("resize", sendViewport);
    };
  });
</script>

{#if state}
  {@const layout = state.workspace.layout}
  <div class="app">
    <Transport />
    <div class="meter" style:height={layout.meter === "full" ? "var(--size-meter-full)" : "var(--size-meter-slim)"}>
      <InputMeter />
    </div>
    <div class="middle">
      <div class="work">
        <Arrange>
          {#if state.workspace.chooser}<OutputChooser />{/if}
          {#if state.workspace.hint === "soundCameOut"}<SoundHint />{/if}
        </Arrange>
        {#each layout.lower as lower (lower.band)}
          <LowerBand band={lower.band} height={lower.height} />
        {/each}
      </div>
      <RightRail />
    </div>
    <BandRow />
    <StatusLine />
  </div>
  {#if state.workspace.settings}
    <div class="dialog">
      <SettingsDialog />
    </div>
  {/if}
{:else if current.failure}
  <pre class="failure">{current.failure}</pre>
{/if}
<Tip />
<Surface />

<style>
  .app {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .meter {
    flex-shrink: 0;
    overflow: hidden;
  }
  .middle {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .work {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--color-gray-1);
  }
  .dialog {
    position: fixed;
    inset: 0;
    z-index: 30;
    pointer-events: none;
  }
  .failure {
    margin: var(--space-4);
    color: var(--color-gray-10);
    white-space: pre-wrap;
  }
</style>

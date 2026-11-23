<!-- The order of the screen (spec 3), from the top: menu bar (docs/history/design-notes.ja.md B-59), transport, input meter,
     arrange, the open lower bands, band tab row, status line. On the right, the open panels and their vertical headings.
     Stacking order: the parts of the screen (WebGL surfaces inside their own boxes) < hints and the output chooser (20)
     < dialogs (30) < lists (menu bar, context menu; 40) < term tips. -->
<script lang="ts">
  import { onMount } from "svelte";
  import AboutDialog from "./components/about/AboutDialog.svelte";
  import { about } from "./components/about/about.svelte";
  import KeyListDialog from "./components/keylist/KeyListDialog.svelte";
  import { keyList } from "./components/keylist/keyList.svelte";
  import Arrange from "./components/Arrange.svelte";
  import AutoClock from "./components/automation/AutoClock.svelte";
  import BandRow from "./components/BandRow.svelte";
  import ExportDialog from "./components/export/ExportDialog.svelte";
  import FileDropNotice from "./components/FileDropNotice.svelte";
  import InputMeter from "./components/InputMeter.svelte";
  import LatencyDialog from "./components/LatencyDialog.svelte";
  import LowerBand from "./components/LowerBand.svelte";
  import ContextMenu from "./components/menu/ContextMenu.svelte";
  import MenuBar from "./components/menu/MenuBar.svelte";
  import { metrics, watchDevicePixelRatio } from "./components/metrics.svelte";
  import OutputChooser from "./components/OutputChooser.svelte";
  import ScanStoppedDialog from "./components/plugins/ScanStoppedDialog.svelte";
  import RecoveryHint from "./components/RecoveryHint.svelte";
  import RightRail from "./components/RightRail.svelte";
  import SettingsDialog from "./components/SettingsDialog.svelte";
  import SongDialog from "./components/SongDialog.svelte";
  import SoundHint from "./components/SoundHint.svelte";
  import StatusLine from "./components/StatusLine.svelte";
  import Transport from "./components/Transport.svelte";
  import { palettes } from "./design/themes";
  import { writeCssVariables } from "./design/tokens";
  import Surface from "./gl/Surface.svelte";
  import { attachKeys } from "./keys/attach";
  import Tip from "./lib/Tip.svelte";
  import { current, send, start } from "./state/app.svelte";

  const state = $derived(current.state);

  // Derived first, so the variables are written again only when one of these changes, not with every state.
  const scale = $derived(metrics.scale);
  const theme = $derived(metrics.theme);
  $effect(() => writeCssVariables(scale, metrics.dpr, palettes[theme]));

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
    <MenuBar />
    <Transport />
    <div class="meter" style:height={layout.meter === "full" ? "var(--size-meter-full)" : "var(--size-meter-slim)"}>
      <InputMeter />
    </div>
    <div class="middle">
      <div class="work">
        <Arrange>
          {#if state.workspace.chooser}<OutputChooser />{/if}
          {#if state.workspace.hint}<SoundHint />{/if}
          {#if state.workspace.recovery}<RecoveryHint />{/if}
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
  <AutoClock />
  {#if state.workspace.songDialog}
    <div class="dialog">
      <SongDialog />
    </div>
  {/if}
  {#if state.workspace.latencyDialog}
    <!-- The latency dialog goes over the sound settings (in mock F7-Latency the sound settings are not seen). -->
    <div class="dialog">
      <LatencyDialog />
    </div>
  {:else if state.workspace.settings}
    <div class="dialog">
      <SettingsDialog />
    </div>
  {/if}
  {#if state.export.dialog}
    <div class="dialog">
      <ExportDialog />
    </div>
  {/if}
  {#if about.open}
    <div class="dialog">
      <AboutDialog />
    </div>
  {/if}
  {#if keyList.open}
    <div class="dialog">
      <KeyListDialog />
    </div>
  {/if}
  {#if state.plugins.stoppedOpen}
    <div class="dialog">
      <ScanStoppedDialog />
    </div>
  {/if}
  {#if state.workspace.fileDrop}<FileDropNotice drop={state.workspace.fileDrop} />{/if}
{:else if current.failure}
  <pre class="failure">{current.failure}</pre>
{/if}
<ContextMenu />
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

<!-- The dialog 「書き出しました」 when an export is done (round 12 E9). Built like 「meloet について」 (spec 2 "Dialog"),
     but core holds it: it is open while `export.run` is `done`, and 「閉じる」, Esc and Enter send `closeExport`. -->
<script lang="ts">
  import type { ExportRunView } from "../../bridge";
  import { movableDialog } from "../../lib/dialogMove";
  import { send } from "../../state/app.svelte";
  import { t } from "../../text";
  import Button from "../Button.svelte";
  import { metrics } from "../metrics.svelte";
  import { exportedDetails, exportedLoudness, exportedName, perTrack, wavePath } from "./exported";

  let { run }: { run: Extract<ExportRunView, { state: "done" }> } = $props();

  const many = $derived(perTrack(run));
  const loudness = $derived(exportedLoudness(run));
  const close = (): void => void send({ type: "closeExport" });
</script>

<div class="place">
  <div class="lead"></div>
  <div class="dialog" role="dialog" aria-label={t("exported.title")} data-test="exported-dialog" style:color-scheme={metrics.colorScheme} use:movableDialog>
    <div class="head" data-dialog-head>
      <span class="title">{t("exported.title")}</span>
      <span class="spacer"></span>
      <button type="button" class="close" onclick={close} data-test="exported-close-head">
        {t("button.close")}<span class="mono">{t("key.esc")}</span>
      </button>
    </div>
    <div class="body">
      <div class="name" data-test="exported-name">{exportedName(run)}</div>
      <div class="label" data-test="exported-details">{exportedDetails(run)}</div>
      {#if !many}
        <svg class="wave" viewBox="0 0 {run.wave.length} 255" preserveAspectRatio="none" aria-hidden="true" data-test="exported-wave">
          <path d={wavePath(run.wave)} />
        </svg>
      {/if}
      {#if loudness !== null}
        <div class="label gap" data-test="exported-loudness">{loudness}</div>
      {/if}
      <div class="label" class:gap={loudness === null} data-test="exported-place">{run.place}</div>
    </div>
    <div class="foot">
      <span class="spacer"></span>
      <Button disabled={many} test="exported-play" onclick={() => void send({ type: "playExported" })}>{t("exported.play")}</Button>
      <Button test="exported-folder" onclick={() => void send({ type: "showExported" })}>{t("button.openFolder")}</Button>
      <Button primary test="exported-close" onclick={close}>{t("button.close")}<span class="mono">{t("key.enter")}</span></Button>
    </div>
  </div>
  <div class="lead"></div>
</div>

<style>
  .place {
    position: absolute;
    inset: 0;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: var(--part-dialog-margin) 0;
  }
  .lead {
    flex: 1 1 0;
  }
  .dialog {
    flex-shrink: 0;
    pointer-events: auto;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    width: var(--part-dialog-width);
    max-height: 100%;
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-8);
    border-radius: var(--radius-1);
    color: var(--color-gray-9);
    font-family: var(--font-sans);
    font-size: var(--text-s-size);
    line-height: var(--text-s-line-height);
    font-weight: var(--text-s-weight-400);
    white-space: nowrap;
  }
  .head {
    flex-shrink: 0;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    height: var(--part-dialog-head);
    padding: 0 var(--space-4);
    border-bottom: var(--line-hair) solid var(--color-gray-6);
    cursor: move;
  }
  .head:hover {
    background: var(--color-gray-4);
  }
  .title {
    color: var(--color-gray-10);
    font-weight: var(--text-s-weight-600);
  }
  .close {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: 0;
    border: 0;
    background: none;
    color: var(--color-gray-8);
    font: inherit;
    cursor: pointer;
  }
  .close:hover {
    color: var(--color-gray-10);
  }
  .spacer {
    flex: 1;
    min-width: 0;
  }
  .mono {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  .body {
    display: flex;
    flex-direction: column;
    padding: var(--space-3) var(--space-4);
  }
  .body > div {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .name {
    color: var(--color-gray-10);
    font-size: var(--text-m-size);
    line-height: var(--text-m-line-height);
  }
  .label {
    color: var(--color-gray-8);
  }
  .gap {
    margin-top: var(--space-2);
  }
  .wave {
    display: block;
    width: 100%;
    height: var(--size-overview);
    margin-top: var(--space-2);
    background: var(--color-meter-ground);
    fill: var(--color-meter-ok);
  }
  .foot {
    flex-shrink: 0;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--part-dialog-foot);
    padding: 0 var(--space-4);
    border-top: var(--line-hair) solid var(--color-gray-6);
  }
</style>

<!-- The 「スキャンで止まったプラグイン」 dialog (spec 2 "Dialog", "Row of a plugin stopped in a scan"; spec 3 Y13; spec 6; mock F5-ScanStopped).
     Open, close and try again are intents to core. Esc is Rust's back (close). -->
<script lang="ts">
  import type { StoppedPluginView } from "../../bridge";
  import { movableDialog } from "../../lib/dialogMove";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import Button from "../Button.svelte";
  import { formatText, placesReveal, reasonText } from "./scan";

  const plugins = $derived(current.state?.plugins ?? null);

  function retry(p: StoppedPluginView): void {
    if (!p.retrying) send({ type: "retryStoppedPlugin", path: p.path, id: p.id });
  }

  function openPlaces(): void {
    placesReveal.pending = true;
    send({ type: "openPluginPlaces" });
  }

  const close = () => send({ type: "closeScanStopped" });
</script>

{#if plugins?.stoppedOpen}
  <div class="place">
    <div class="lead"></div>
    <div class="dialog" role="dialog" aria-label={t("plugin.stoppedTitle")} data-test="scan-stopped-dialog" use:movableDialog>
      <div class="head" data-dialog-head>
        <span class="title">{t("plugin.stoppedTitle")}</span>
        <span class="m-spacer"></span>
        <button type="button" class="close" data-test="scan-stopped-close-head" onclick={close}>
          {t("button.close")}<span class="m-num">{t("key.esc")}</span>
        </button>
      </div>
      <ScrollArea data-test="scan-stopped-body">
        <p class="intro">{t("plugin.stoppedList", { count: plugins.stopped.length })}</p>
        {#each plugins.stopped as p (`${p.path}\n${p.id ?? ""}`)}
          <div class="row" data-test="scan-stopped-row">
            <div class="top">
              <span class="name">{p.name}</span>
              <span class="m-num format">{formatText(p.format)}</span>
              <span class="m-lbl">{reasonText(p.reason)}</span>
              <span class="m-spacer"></span>
              <Button test="scan-stopped-retry" onclick={() => retry(p)}>{t(p.retrying ? "plugin.retrying" : "button.retry")}</Button>
            </div>
            <span class="m-num m-lbl path">{p.shownPath}</span>
          </div>
        {/each}
      </ScrollArea>
      <div class="foot">
        <Button test="scan-stopped-places" onclick={openPlaces}>{t("button.pluginFolders")}</Button>
        <Button primary test="scan-stopped-close" onclick={close}>{t("button.close")}</Button>
      </div>
    </div>
    <div class="lead"></div>
  </div>
{/if}

<style>
  /* The height follows the content, up to a gap of 24 above and below the window. Past that, only the content between scrolls. */
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
    width: var(--mock-scan-stopped-width);
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
  /* Hovering or moving the heading row: bg gray.4 (spec 2 "Dialog heading row (moving)"). */
  .head:hover {
    background: var(--color-gray-4);
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
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
  .intro {
    margin: 0;
    padding: var(--space-3) var(--space-4);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    white-space: normal;
  }
  .row {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-4);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  .top {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-gray-10);
    font-weight: var(--text-s-weight-600);
  }
  /* Format mark (`.fm` in mock F5-ScanStopped: 4 left and right, border gray.7, text gray.8). */
  .format {
    flex-shrink: 0;
    padding: 0 var(--space-1);
    border: var(--line-hair) solid var(--color-gray-7);
    border-radius: var(--radius-1);
    color: var(--color-gray-8);
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .foot {
    flex-shrink: 0;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-2);
    height: var(--part-dialog-foot);
    padding: 0 var(--space-4);
    border-top: var(--line-hair) solid var(--color-gray-6);
  }
</style>

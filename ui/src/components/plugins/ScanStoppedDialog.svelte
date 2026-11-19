<!-- ダイアログ「スキャンで止まったプラグイン」（仕様書 2 章「ダイアログ」「スキャンで止まった物の行」、3 章 Y13、6 章、モック F5-ScanStopped）。
     ひらく ・ 閉じる ・ もう一度ためすは core への意図。Esc は Rust の back（閉じる）。 -->
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
  /* 高さは中身に合わせ、窓の上下 24 ずつのあきまで。それをこえたら、間の中身だけスクロールする。 */
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
  /* 見出しの行に乗る ・ 動かしている: 地 gray.4（仕様書 2 章「ダイアログの見出しの行（動かす）」）。 */
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
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    white-space: normal;
  }
  .row {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-4);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
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
  /* 形式の印（モック F5-ScanStopped の `.fm`: 左右 4、枠 gray.7、文字 gray.8）。 */
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

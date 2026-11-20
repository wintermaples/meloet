<!-- 遅れを測る（仕様書 3 章 25-B ・ 33-A、6 章「遅れを測っている」「遅れを測れない」、モック F7-Latency ・ R11-Fit）。
     音の設定 ▸ くわしい設定の「遅れを測る…」からひらく（WorkspaceView.latencyDialog）。カチッという音を 5 回鳴らして平均を出し、
     「この数を使う」で、その PC の設定にする（曲ではなく PC につく）。PC が言う数は AudioView.latency。 -->
<script lang="ts">
  import { LATENCY_CLICKS, type LatencyMeasure } from "../bridge";
  import { movableDialog } from "../lib/dialogMove";
  import { current, send } from "../state/app.svelte";
  import { t } from "../text/ja";
  import Button from "./Button.svelte";
  import { msText, textParts } from "./format";

  const app = $derived(current.state);
  const measure = $derived<LatencyMeasure | null>(app?.workspace.latencyDialog?.measure ?? null);
  const latency = $derived(app?.audio.latency ?? null);
  const dash = t("meter.playTargetNone");
  const ms = (v: number | null) => t("latency.ms", { ms: v === null ? dash : msText(v) });

  /** 測った数の行: 測っている間は、そこまでの平均と「（まだ 3 回の平均）」。終わったら、その数。 */
  const measured = $derived.by((): { value: number | null; partial: number | null } => {
    if (measure?.state === "measuring") return { value: measure.meanMs, partial: measure.meanMs === null ? null : measure.done };
    if (measure?.state === "done") return { value: measure.measuredMs, partial: null };
    return { value: latency?.measuredMs ?? null, partial: null };
  });
  /** 「測っています 3 / 5」は、棒をはさんで 2 つに分ける（モック F7-Latency）。 */
  const progress = $derived.by(() => {
    if (measure?.state !== "measuring") return null;
    const parts = textParts("latency.measuring", { done: measure.done, total: measure.of });
    const at = parts.findIndex((p) => p.hole === "done");
    return {
      label: parts.slice(0, at).map((p) => p.text).join("").trim(),
      count: parts.slice(at).map((p) => p.text).join(""),
      fraction: measure.of > 0 ? Math.min(1, measure.done / measure.of) : 0,
    };
  });
  /** 測れなかったわけ。ぐらつく（unsteady）ときの文は、仕様書にもモックにもない。 */
  const failure = $derived.by(() => {
    if (measure?.state !== "failed") return null;
    if (measure.reason === "tooQuiet") return t("latency.failed");
    if (measure.reason === "noInput") return t("msg.noAudioInput");
    return null;
  });

  const close = () => send({ type: "closeLatencyMeasure" });
  /** やめる（Esc）: 測っている間は測るのをやめ、ほかはダイアログを閉じる。 */
  const cancel = () => send(measure?.state === "measuring" ? { type: "stopLatencyMeasure" } : { type: "closeLatencyMeasure" });
</script>

{#if app && measure && latency}
  <div class="place">
    <div class="lead"></div>
    <div class="dialog" role="dialog" aria-label={t("latency.title")} data-test="latency-dialog" use:movableDialog>
      <div class="head" data-dialog-head>
        <span class="title">{t("latency.title")}</span>
        <span class="spacer"></span>
        <button type="button" class="close" data-test="latency-close" onclick={close}>
          {t("settings.close")}<span class="mono">{t("key.esc")}</span>
        </button>
      </div>
      <div class="body">
        <span class="wrap">{t("latency.how", { count: LATENCY_CLICKS })}</span>
        {#if progress}
          <div class="progress" data-test="latency-progress">
            <span class="label">{progress.label}</span>
            <div class="bar"><div class="fill" style:transform="scaleX({progress.fraction})"></div></div>
            <span class="mono strong">{progress.count}</span>
          </div>
        {/if}
        <div class="table">
          <div class="row">
            <span class="strong name">{t("latency.reported")}</span>
            <span class="mono strong" data-test="latency-reported">{ms(latency.reportedMs)}</span>
          </div>
          <div class="row">
            <span class="strong name">{t("latency.measured")}</span>
            <span class="mono strong" data-test="latency-measured">{ms(measured.value)}</span>
            {#if measured.partial !== null}<span class="label">{t("latency.partialMean", { count: measured.partial })}</span>{/if}
          </div>
        </div>
        {#if failure}
          <span class="wrap strong" data-test="latency-failed">{failure}</span>
        {:else}
          <span class="wrap label">{t("latency.quietHint")}</span>
        {/if}
      </div>
      <div class="foot">
        <Button test="latency-cancel" onclick={cancel}>{t("button.cancel")}<span class="mono label">{t("key.esc")}</span></Button>
        {#if measure.state === "idle"}
          <Button primary test="latency-start" onclick={() => send({ type: "startLatencyMeasure" })}>{t("button.measure")}</Button>
        {:else if measure.state === "failed"}
          <Button primary test="latency-again" onclick={() => send({ type: "startLatencyMeasure" })}>{t("button.again")}</Button>
        {:else}
          <Button primary test="latency-use" disabled={measure.state !== "done"} onclick={() => send({ type: "useMeasuredLatency" })}
            >{t("button.useThisValue")}</Button
          >
        {/if}
      </div>
    </div>
    <div class="lead"></div>
  </div>
{/if}

<style>
  /* 窓の上下のまん中（音の設定と同じ）。 */
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
    width: var(--mock-latency-width);
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
  .close:focus-visible {
    outline: var(--line-hair) solid var(--color-gray-10);
    outline-offset: var(--line-hair);
  }
  .spacer {
    flex: 1;
  }
  .body {
    min-height: 0;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-4);
  }
  .wrap {
    white-space: normal;
  }
  .label {
    color: var(--color-gray-8);
  }
  .strong {
    color: var(--color-gray-10);
  }
  .mono {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  /* 測っている棒（モック F7-Latency: 地 gray.4、accent）。 */
  .progress {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .bar {
    flex: 1;
    height: var(--mock-latency-progress);
    background: var(--color-gray-4);
  }
  .fill {
    height: 100%;
    background: var(--color-accent);
    transform-origin: left;
  }
  .table {
    display: flex;
    flex-direction: column;
    border: var(--line-hair) solid var(--color-gray-5);
  }
  .row {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-height: var(--size-toolbar);
    padding: var(--space-1) 0;
    border-bottom: var(--line-hair) solid var(--color-gray-4);
  }
  .row:last-child {
    border-bottom: 0;
  }
  .name {
    flex-shrink: 0;
    width: var(--mock-settings-adv-label);
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

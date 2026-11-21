<!-- 書き出し（仕様書 3 章 34-B、2 章「ダイアログ」、6 章「書き出しの前」「書き出しで音が割れる」、モック F7-Export ・ F-Export ・ R11-Fit）。
     範囲、形、保存する所、書き出す前のたしかめ、「くわしく」。中身は Rust の export.dialog の写しで、画面は意図だけを送る。
     窓に入らないときは、見出しと下のボタンを残して中をスクロールする（R11-Fit）。 -->
<script lang="ts">
  import type { ExportChange, ExportDialogView, ExportFormat, ExportRange, ExportTail } from "../../bridge";
  import { movableDialog } from "../../lib/dialogMove";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { current, send } from "../../state/app.svelte";
  import { exportShown } from "../../state/exportShown.svelte";
  import { LIST_COMMA, t, type TextKey } from "../../text/ja";
  import { mapOf } from "../../time/bars";
  import Button from "../Button.svelte";
  import { openContextMenu, type MenuEntry } from "../menu/contextMenu.svelte";
  import { stripName } from "../mixer/stripName";
  import { cutHead } from "./cutHead";
  import { bitsOf, checkLines, clips, detailsSummary, rangeLabel, spanText, tailText } from "./exportText";

  const RANGES: ExportRange[] = ["song", "loop", "tracks"];
  const FORMAT_NAMES: Record<ExportFormat, TextKey> = { wav: "export.formatWav", flac: "export.formatFlac" };
  const TAILS: ExportTail[] = ["untilSilent", "off"];

  const app = $derived(current.state);
  const dialog = $derived<ExportDialogView | null>(exportShown.current?.dialog ?? null);
  const bars = $derived(app ? mapOf(app.timeline) : null);

  const usable = (d: ExportDialogView, r: ExportRange) => (r === "loop" ? d.loopRange !== null : r === "tracks" ? d.tracks > 0 : true);
  const set = (change: ExportChange) => send({ type: "setExport", change });
  const close = () => send({ type: "closeExport" });
  const start = () => send({ type: "startExport" });

  const stopped = $derived(
    app && dialog
      ? dialog.stoppedPlugins
          .map((p) =>
            t("export.stoppedPlugin", {
              plugin: p.name,
              strip: stripName(app.song, p.slot.kind === "instrument" ? { kind: "track", id: p.slot.track } : p.slot.insert.strip, app.library),
            }),
          )
          .join(LIST_COMMA)
      : "",
  );

  /** ▾ を押したら、その下に一覧（音の設定のくわしい設定と同じ形）。 */
  function choose(e: MouseEvent, entries: MenuEntry[]): void {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    openContextMenu(r.left, r.bottom, entries);
  }
  const rateRows = (d: ExportDialogView): MenuEntry[] =>
    d.sampleRateChoices.map((hz) => ({
      kind: "row",
      label: "settings.rateHz",
      text: t("settings.rateHz", { rate: hz }),
      usable: true,
      checked: hz === d.sampleRate,
      run: () => set({ kind: "sampleRate", hz }),
    }));
  const bitsRows = (d: ExportDialogView): MenuEntry[] =>
    d.bitsChoices.map((bits) => ({
      kind: "row",
      label: "settings.bits",
      text: t("settings.bits", { bits: bitsOf(bits) }),
      usable: true,
      checked: bits === d.bits,
      run: () => set({ kind: "bits", bits }),
    }));
  const tailRows = (d: ExportDialogView): MenuEntry[] =>
    TAILS.map((tail) => ({
      kind: "row",
      label: tail === "untilSilent" ? "export.tailUntilSilent" : "export.tailOff",
      text: tailText(tail),
      usable: true,
      checked: tail === d.tail,
      run: () => set({ kind: "tail", tail }),
    }));
</script>

{#snippet dropdown(label: string, test: string, open: (e: MouseEvent) => void)}
  <button type="button" class="drop" aria-haspopup="menu" data-test={test} onclick={open}>{label}<span class="label">▾</span></button>
{/snippet}

{#if app && dialog && bars}
  <div class="place">
    <div class="lead"></div>
    <div class="dialog" role="dialog" aria-label={t("export.title")} data-test="export-dialog" use:movableDialog>
      <div class="head" data-dialog-head>
        <span class="title">{t("export.title")}</span>
        <span class="spacer"></span>
        <button type="button" class="close" data-test="export-close" onclick={close}>
          {t("settings.close")}<span class="mono">{t("key.esc")}</span>
        </button>
      </div>

      <ScrollArea data-test="export-body">
        <section>
          <span class="label">{t("export.range")}</span>
          <div class="choices">
            {#each RANGES as r (r)}
              <Button test="export-range-{r}" on={dialog.range === r} disabled={!usable(dialog, r)} onclick={() => set({ kind: "range", range: r })}
                >{rangeLabel(r, dialog, bars)}</Button
              >
            {/each}
          </div>
          <span class="label" data-test="export-span">{spanText(dialog, bars)}</span>
        </section>

        <section>
          <span class="label">{t("export.format")}</span>
          <div class="choices">
            {#each dialog.formats as f (f)}
              <Button test="export-format-{f}" on={dialog.format === f} onclick={() => set({ kind: "format", format: f })}>{t(FORMAT_NAMES[f])}</Button>
            {/each}
          </div>
        </section>

        <section>
          <span class="label">{t("export.place")}</span>
          <div class="line">
            <span class="field mono strong" data-test="export-place" use:cutHead>
              <span class="path"><span class="path-text" data-cut-text>{dialog.place}</span></span>
            </span>
            <Button test="export-choose" onclick={() => send({ type: "chooseExportPlace" })}>{t("export.choosePlace")}</Button>
          </div>
        </section>

        <section data-test="export-check">
          <span class="label">{t("export.check")}</span>
          {#each checkLines(dialog.check) as line, i (i)}
            <span class="label" class:wrap={line.wrap} class:strong={line.wrap} data-test="export-check-line">
              {#each line.parts as part, j (j)}{#if part.hole !== null}<span class="mono strong">{part.text}</span>{:else}{part.text}{/if}{/each}
            </span>
          {/each}
          {#if stopped}
            <span class="stopped" data-test="export-stopped">
              <span class="label">{t("export.stoppedPlugins")}</span>
              <span class="rec">{stopped}</span>
              <span class="label">{t("export.stoppedSkipped")}</span>
            </span>
          {:else}
            <span class="label" data-test="export-stopped">{t("export.noStoppedPlugins")}</span>
          {/if}
        </section>

        <button
          type="button"
          class="adv"
          aria-expanded={dialog.advancedOpen}
          data-test="export-details"
          onclick={() => set({ kind: "advancedOpen", open: !dialog.advancedOpen })}
        >
          <span class="label">{dialog.advancedOpen ? "▾" : "▸"}</span>
          <span class="strong grow">{t("button.details")}</span>
          {#if !dialog.advancedOpen}<span class="label" data-test="export-details-summary">{detailsSummary(dialog)}</span>{/if}
        </button>
        {#if dialog.advancedOpen}
          <div class="adv-body" data-test="export-adv">
            <div class="row">
              <span class="strong name">{t("export.sampleRate")}</span>
              {@render dropdown(t("settings.rateHz", { rate: dialog.sampleRate }), "export-rate", (e) => choose(e, rateRows(dialog)))}
            </div>
            <div class="row">
              <span class="strong name">{t("export.bits")}</span>
              {@render dropdown(t("settings.bits", { bits: bitsOf(dialog.bits) }), "export-bits", (e) => choose(e, bitsRows(dialog)))}
            </div>
            <div class="row">
              <span class="strong name">{t("export.tail")}</span>
              {@render dropdown(tailText(dialog.tail), "export-tail", (e) => choose(e, tailRows(dialog)))}
            </div>
            <div class="row">
              <button
                type="button"
                class="pick"
                role="checkbox"
                aria-checked={dialog.realtime}
                disabled={dialog.realtimeForced}
                data-test="export-realtime"
                onclick={() => set({ kind: "realtime", on: !dialog.realtime })}
              >
                <span class="mark" class:on={dialog.realtime}><span class="dot"></span></span>
                <span>{t("export.realtime")}</span>
              </button>
            </div>
          </div>
        {/if}
      </ScrollArea>

      <div class="foot">
        <Button test="export-cancel" onclick={close}>{t("button.cancel")}<span class="mono label">{t("key.esc")}</span></Button>
        <Button primary test="export-start" disabled={!dialog.canExport} onclick={start}
          >{clips(dialog.check) ? t("button.exportAnyway") : t("export.start")}<span class="mono label">{t("key.enter")}</span></Button
        >
      </div>
    </div>
    <div class="lead"></div>
  </div>
{/if}

<style>
  /* 窓の上下のまん中（仕様書 3 章「ダイアログを動かす」: ひらくたびに、いまの窓のまん中から）。 */
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
  .spacer,
  .grow {
    flex: 1;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-3) var(--space-4);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .choices {
    display: flex;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  /* 保存する所（モック F7-Export: 地 gray.0、枠 gray.6、内 4 ・ 8）。 */
  .field {
    flex: 1;
    min-width: 0;
    display: flex;
    overflow: hidden;
    padding: var(--space-1) var(--space-2);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-6);
  }
  /* 入りきらない道は、頭を「…」で省いて終わり（ファイルの名前）を見せる（cutHead が data-cut をつける）。 */
  .field:global([data-cut])::before {
    content: "…";
    flex-shrink: 0;
  }
  .path {
    flex: 1;
    min-width: 0;
    display: flex;
    overflow: hidden;
  }
  .field:global([data-cut]) .path {
    justify-content: flex-end;
  }
  .path-text {
    flex-shrink: 0;
  }
  .label {
    color: var(--color-gray-8);
  }
  .strong {
    color: var(--color-gray-10);
  }
  .wrap {
    white-space: normal;
  }
  .rec {
    color: var(--color-rec);
  }
  .mono {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  /* 止まっているプラグイン（モック F-Export: 名前は rec、そのあとに「この枠をとばした音で書き出します」）。 */
  .stopped {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    overflow: hidden;
  }
  /* 「くわしく」の行（モック F7-Export の hd: 高さ 24、左右 12、地 gray.4）。 */
  .adv {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    height: var(--size-row);
    padding: 0 var(--space-3);
    border: 0;
    background: var(--color-gray-4);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .adv:focus-visible,
  .drop:focus-visible,
  .pick:focus-visible,
  .close:focus-visible {
    outline: var(--line-hair) solid var(--color-gray-10);
    outline-offset: var(--line-hair);
  }
  /* ひらいた「くわしく」は、音の設定のくわしい設定と同じ行の形（名前 120、高さ 32 から、下に gray.4 の線）。 */
  .adv-body {
    display: flex;
    flex-direction: column;
    padding: var(--space-1) var(--space-4) var(--space-2);
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
  .drop {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--part-button-height);
    padding: 0 var(--part-button-pad-x);
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-gray-10);
    font: inherit;
    cursor: pointer;
  }
  .drop:hover {
    border-color: var(--color-gray-8);
  }
  /* 実時間で書き出す: 選ぶ印（仕様書 2 章「選ぶ印（設定の中）」12×12 の枠、えらぶと 6×6 の accent）。 */
  .pick {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--part-button-height);
    padding: 0;
    border: 0;
    background: none;
    color: var(--color-gray-10);
    font: inherit;
    cursor: pointer;
  }
  .mark {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--part-choice-size);
    height: var(--part-choice-size);
    border: var(--line-hair) solid var(--color-gray-8);
  }
  .mark.on {
    border-color: var(--color-accent);
  }
  .dot {
    width: var(--part-choice-dot);
    height: var(--part-choice-dot);
  }
  .mark.on .dot {
    background: var(--color-accent);
  }
  .pick:disabled {
    color: var(--color-gray-7);
    cursor: default;
  }
  .pick:disabled .mark {
    border-color: var(--color-gray-5);
  }
  .pick:disabled .mark.on .dot {
    background: var(--color-gray-7);
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

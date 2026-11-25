<!-- Export (spec 3 34-B, 41-A, 42-C; spec 2 "Dialog"; spec 6 "Before export"; mock F7-Export, F-Export, F8-ExportCheck, F8-ExportClip, R11-Fit; round 12 F1).
     Range, format, loudness, where to save, the check before export (while counting, 「書き出す」 can be pressed once a number has come; soloed tracks are told here), 「くわしく」.
     When 「書き出す」 finds its files there already, Rust keeps the dialog and asks in the place of the check, with 「このまま書き出す」 (protocol 39).
     The content is a copy of Rust's export.dialog; the screen only sends intents.
     When it does not fit the window, the heading and the bottom buttons stay and the inside scrolls (R11-Fit). -->
<script lang="ts">
  import type { ExportChange, ExportDialogView, ExportFormat, ExportLoudness, ExportMidiTracks, ExportRange, ExportTail } from "../../bridge";
  import { movableDialog } from "../../lib/dialogMove";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { current, send } from "../../state/app.svelte";
  import { exportShown } from "../../state/exportShown.svelte";
  import { LIST_COMMA, t, type TextKey } from "../../text";
  import { mapOf } from "../../time/bars";
  import Button from "../Button.svelte";
  import { openContextMenu, type MenuEntry } from "../menu/contextMenu.svelte";
  import { stripName } from "../mixer/stripName";
  import { cutHead } from "./cutHead";
  import {
    bitsOf,
    checkLines,
    clips,
    detailsSummary,
    loudnessTarget,
    midiProblemText,
    midiTakenText,
    midiVocalLines,
    rangeLabel,
    replaceText,
    soloText,
    spanText,
    tailText,
  } from "./exportText";

  const RANGES: ExportRange[] = ["song", "loop", "tracks"];
  const FORMAT_NAMES: Record<ExportFormat, TextKey> = { wav: "export.formatWav", flac: "export.formatFlac", mp3: "export.formatMp3", midi: "export.formatMidi" };
  const MIDI_TRACKS: [ExportMidiTracks, TextKey][] = [
    ["all", "export.midiAll"],
    ["selected", "export.midiSelected"],
    ["numbers", "export.midiNumbers"],
  ];
  const LOUDNESS_NAMES: [ExportLoudness, TextKey][] = [
    ["asIs", "export.loudnessAsIs"],
    ["match", "export.loudnessMatch"],
  ];
  const TAILS: ExportTail[] = ["untilSilent", "off"];
  const TEXT_FIELDS: ["title" | "artist", TextKey][] = [
    ["title", "export.songTitle"],
    ["artist", "export.artist"],
  ];

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

  const solo = $derived(dialog ? soloText(dialog.solo) : null);
  const midi = $derived(dialog?.format === "midi");
  /** Nothing is measured for MIDI, so it never clips. */
  const clipping = $derived(dialog !== null && !midi && clips(dialog.check));
  const replacing = $derived(dialog?.replace ? replaceText(dialog.replace) : null);

  /** Pressing ▾ opens a list under it (the same form as the advanced sound settings). */
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
  const kbpsRows = (d: ExportDialogView): MenuEntry[] =>
    d.mp3KbpsChoices.map((kbps) => ({
      kind: "row",
      label: "export.kbps",
      text: t("export.kbps", { kbps }),
      usable: true,
      checked: kbps === d.mp3Kbps,
      run: () => set({ kind: "mp3Kbps", kbps }),
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

  // 「題名」, 「作者」 (round 12 F2) and the numbers of 「番号で」: what is typed stays on the screen and is sent once, when the box is left or Enter is pressed. Esc puts back what Rust has.
  type TextField = "title" | "artist" | "midiNumbers";
  let drafts = $state<Record<TextField, string | null>>({ title: null, artist: null, midiNumbers: null });
  function commitText(field: TextField): void {
    const typed = drafts[field];
    drafts[field] = null;
    if (typed === null || !dialog || typed === dialog[field]) return;
    set(
      field === "title" ? { kind: "title", title: typed } : field === "artist" ? { kind: "artist", artist: typed } : { kind: "midiNumbers", text: typed },
    );
  }
  function textKey(e: KeyboardEvent, field: TextField): void {
    if (e.isComposing) return;
    if (e.key !== "Enter" && e.key !== "Escape") return;
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") drafts[field] = null;
    (e.currentTarget as HTMLInputElement).blur();
  }
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

        {#if midi}
          <!-- 「入れるトラック」 (spec 3 "MIDI export", spec 7 A-31). -->
          <section data-test="export-midi">
            <span class="label">{t("export.midiTracks")}</span>
            <div class="choices">
              {#each MIDI_TRACKS as [choice, name] (choice)}
                <Button
                  test="export-midi-{choice}"
                  on={dialog.midiTracks === choice}
                  disabled={choice === "selected" && !dialog.midiSelectedUsable}
                  onclick={() => set({ kind: "midiTracks", tracks: choice })}>{t(name)}</Button
                >
              {/each}
            </div>
            {#if dialog.midiTracks === "numbers"}
              <input
                class="text numbers"
                type="text"
                aria-label={t("export.midiNumbers")}
                placeholder={t("export.midiNumbersHint")}
                data-test="export-midi-text"
                value={drafts.midiNumbers ?? dialog.midiNumbers}
                oninput={(e) => (drafts.midiNumbers = e.currentTarget.value)}
                onkeydown={(e) => textKey(e, "midiNumbers")}
                onblur={() => commitText("midiNumbers")}
              />
            {/if}
            {#if midiTakenText(dialog.midiTaken)}
              <span class="label wrap" data-test="export-midi-taken">{midiTakenText(dialog.midiTaken)}</span>
            {/if}
            {#each midiVocalLines(dialog.midiVocal) as line, i (i)}
              <span class="label wrap" data-test="export-midi-vocal">{line}</span>
            {/each}
            {#if dialog.midiProblem}
              <span class="rec wrap" data-test="export-midi-problem">{midiProblemText(dialog.midiProblem)}</span>
            {/if}
          </section>
        {:else}
          <!-- 「音の大きさ」 (round 12 F1): 「公開先に合わせる」 cannot be chosen for 「トラックごと」 nor for a song near silence. -->
          <section data-test="export-loudness">
            <span class="label">{t("export.loudness")}</span>
            <div class="line">
              <div class="choices">
                {#each LOUDNESS_NAMES as [l, name] (l)}
                  <Button
                    test="export-loudness-{l}"
                    on={dialog.loudness === l}
                    disabled={l === "match" && !dialog.canMatch}
                    onclick={() => set({ kind: "loudness", loudness: l })}>{t(name)}</Button
                  >
                {/each}
              </div>
              <span class="mono" data-test="export-loudness-target">{loudnessTarget()}</span>
            </div>
          </section>
        {/if}

        <section>
          <span class="label">{t("export.place")}</span>
          <div class="line">
            <span class="field mono strong" data-test="export-place" use:cutHead>
              <span class="path"><span class="path-text" data-cut-text>{dialog.place}</span></span>
            </span>
            <Button test="export-choose" onclick={() => send({ type: "chooseExportPlace" })}>{t("export.choosePlace")}</Button>
          </div>
        </section>

        {#if midi && replacing}
          <!-- MIDI has no check: the question is where the check would be. -->
          <section data-test="export-replace-midi">
            <span class="rec wrap" data-test="export-replace">{replacing}</span>
          </section>
        {/if}

        {#if !midi}
          <section data-test="export-check">
            <span class="label">{t("export.check")}</span>
            {#each checkLines(dialog.check, dialog) as line, i (i)}
              {#if line.kind === "counting"}
                <!-- 「たしかめています ・ 160 の棒 ・ 38 %」 (mock F8-ExportCheck). The % text is in a box outside the flow, with its width set by the text 「100 %」. -->
                <span class="counting" data-test="export-check-line">
                  <span class="label">{t("export.checkChecking")}</span>
                  <span class="bar"><span class="fill" style:transform="scaleX({Math.min(1, Math.max(0, line.percent / 100))})"></span></span>
                  <span class="mono strong percent" data-size={t("export.runningPercent", { percent: 100 })} data-test="export-check-percent"
                    ><span class="in">{t("export.runningPercent", { percent: line.percent })}</span></span
                  >
                </span>
              {:else}
                <span class={line.tone === "rec" ? "rec wrap" : line.tone === "hint" ? "label wrap" : "label"} data-test="export-check-line">
                  {#each line.parts as part, j (j)}{#if part.hole !== null}<span
                        class="mono"
                        class:strong={line.tone === "plain"}
                        class:rec={line.tone === "over"}
                        class:apart={j > 0 && !line.parts[j - 1]!.text.endsWith(" ")}>{part.text}</span
                      >{:else}{part.text}{/if}{/each}
                </span>
              {/if}
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
            {#if solo}
              <span class="label wrap" data-test="export-solo">{solo}</span>
            {/if}
            {#if replacing}
              <span class="rec wrap" data-test="export-replace">{replacing}</span>
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
              {#if dialog.format === "mp3"}
                <div class="row">
                  <span class="strong name">{t("export.bitrate")}</span>
                  {@render dropdown(t("export.kbps", { kbps: dialog.mp3Kbps }), "export-kbps", (e) => choose(e, kbpsRows(dialog)))}
                </div>
              {:else}
                <div class="row">
                  <span class="strong name">{t("export.bits")}</span>
                  {@render dropdown(t("settings.bits", { bits: bitsOf(dialog.bits) }), "export-bits", (e) => choose(e, bitsRows(dialog)))}
                </div>
              {/if}
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
              <!-- 「曲の情報」 (round 12 F2): a wav holds no picture, so its row is there but cannot be used. -->
              <div class="sub label" data-test="export-song-info">{t("export.songInfo")}</div>
              {#each TEXT_FIELDS as [field, name] (field)}
                <div class="row">
                  <span class="strong name">{t(name)}</span>
                  <input
                    class="text"
                    type="text"
                    aria-label={t(name)}
                    data-test="export-{field}"
                    value={drafts[field] ?? dialog[field]}
                    oninput={(e) => (drafts[field] = e.currentTarget.value)}
                    onkeydown={(e) => textKey(e, field)}
                    onblur={() => commitText(field)}
                  />
                </div>
              {/each}
              <div class="row" class:off={!dialog.pictureUsable} data-test="export-picture-row">
                <span class="strong name">{t("export.picture")}</span>
                <span class="grow picture" class:label={dialog.picture === null} data-test="export-picture">{dialog.picture ?? t("export.noPicture")}</span>
                {#if dialog.picture !== null}
                  <Button test="export-remove-picture" disabled={!dialog.pictureUsable} onclick={() => set({ kind: "removePicture" })}
                    >{t("export.removePicture")}</Button
                  >
                {/if}
                <Button test="export-choose-picture" disabled={!dialog.pictureUsable} onclick={() => set({ kind: "choosePicture" })}
                  >{t("export.choosePicture")}</Button
                >
              </div>
            </div>
          {/if}
        {/if}
      </ScrollArea>

      <div class="foot">
        <Button test="export-cancel" onclick={close}>{t("button.cancel")}<span class="mono label">{t("key.esc")}</span></Button>
        <Button primary={!clipping && !replacing} test="export-start" disabled={!dialog.canExport} onclick={start}
          >{clipping || replacing ? t("button.exportAnyway") : t("export.start")}<span class="mono label">{t("key.enter")}</span></Button
        >
      </div>
    </div>
    <div class="lead"></div>
  </div>
{/if}

<style>
  /* Centred vertically in the window (spec 3, moving dialogs: each time it opens, from the centre of the current window). */
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
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  .choices {
    display: flex;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  /* Where to save (mock F7-Export: bg gray.0, border gray.6, padding 4 and 8). */
  .field {
    flex: 1;
    min-width: 0;
    display: flex;
    overflow: hidden;
    padding: var(--space-1) var(--space-2);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-6);
  }
  /* A path that does not fit is cut at the head with 「…」 to show the end (the file name) (cutHead sets data-cut). */
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
  .apart {
    margin-left: var(--space-1);
  }
  /* The check bar row (mock F8-ExportCheck: gap 8, the bar is 160×4, bg gray.4 with accent). */
  .counting {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .bar {
    flex-shrink: 0;
    width: var(--part-exporting-bar);
    height: var(--mock-exporting-bar);
    background: var(--color-gray-4);
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--color-accent-ink);
    transform-origin: left;
  }
  .percent {
    position: relative;
  }
  .percent::before {
    content: attr(data-size);
    visibility: hidden;
  }
  .percent > .in {
    position: absolute;
    top: 0;
    left: 0;
    white-space: nowrap;
  }
  .mono {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  /* Stopped plugins (mock F-Export: the name in rec, then 「この枠をとばした音で書き出します」). */
  .stopped {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    overflow: hidden;
  }
  /* The 「くわしく」 row (hd in mock F7-Export: height 24, 12 left and right, bg gray.4). */
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
  /* The open 「くわしく」 uses the same row form as the advanced sound settings (name 120, at least 32 high, a line.row line below). */
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
    border-bottom: var(--line-hair) solid var(--color-line-row);
  }
  .row:last-child {
    border-bottom: 0;
  }
  /* 「曲の情報」 over its rows, as the name of a part of the dialog. */
  .sub {
    padding: var(--space-3) 0 var(--space-1);
  }
  .text {
    flex: 1;
    min-width: 0;
    height: var(--part-input-height);
    padding: 0 var(--part-input-pad-x);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    color: var(--color-gray-10);
    font: inherit;
    outline: none;
  }
  .numbers {
    flex: none;
  }
  .numbers::placeholder {
    color: var(--color-gray-7);
  }
  .text:focus {
    border-color: var(--color-gray-8);
  }
  .picture {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .row.off .name,
  .row.off .picture {
    color: var(--color-gray-7);
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
  /* Export in real time: choice mark (spec 2 "Choice mark (in settings)", a 12×12 box; chosen is a 6×6 accent). */
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
    border-color: var(--color-accent-ink);
  }
  .dot {
    width: var(--part-choice-dot);
    height: var(--part-choice-dot);
  }
  .mark.on .dot {
    background: var(--color-accent-ink);
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

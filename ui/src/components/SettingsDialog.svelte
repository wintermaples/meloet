<!-- 音の設定（モック F-Settings のおもて、F7-SettingsAdv の「くわしい設定」。仕様書 2 章「ダイアログ」、3 章 33-A）。
     くわしい設定は、入口 ・ バッファ ・ サンプルレート ・ 録る形 ・ 遅れの合わせ ・ プラグインの場所。バッファとサンプルレートは「頼む数」で、
     右に「動いている」数。ちがったら、その下に「頼んだ 128 ではなく、256 で動いています…」。F-SettingsAdv の「音のしくみ」と 64〜1024 のならびは、ない。
     ▾ の一覧は右クリックの一覧と同じ部品（ContextMenu）。 -->
<script lang="ts">
  import { RECORD_FORMATS, type EndpointView, type VocalInput } from "../bridge";
  import { space, type Scale } from "../design/tokens";
  import { keyboardLayout } from "../gl/keyboard";
  import { glRegion, glSlot } from "../gl/slot";
  import { movableDialog } from "../lib/dialogMove";
  import ScrollArea from "../lib/ScrollArea.svelte";
  import { term } from "../lib/tip.svelte";
  import { current, send } from "../state/app.svelte";
  import { t } from "../text/ja";
  import Button from "./Button.svelte";
  import EndpointRow from "./EndpointRow.svelte";
  import UnusableEndpoints from "./UnusableEndpoints.svelte";
  import { endpointGroups, type EndpointLook } from "./endpoint";
  import { msText } from "./format";
  import { openContextMenu, type MenuEntry } from "./menu/contextMenu.svelte";
  import PluginPlaces from "./plugins/PluginPlaces.svelte";
  import ValueField from "./params/ValueField.svelte";

  const FIRST_NOTE = 36;
  const LAST_NOTE = 84;
  const WHITE_KEYS = keyboardLayout(FIRST_NOTE, LAST_NOTE).whiteCount;
  const CHANNELS: Record<number, VocalInput> = { 1: "mono1", 2: "stereo12" };

  const app = $derived(current.state);
  const settings = $derived(app?.workspace.settings ?? null);
  const advanced = $derived(settings?.advancedOpen ?? false);
  const scale = $derived((app?.workspace.scale ?? 100) as Scale);
  const meterGap = $derived(space["space.1"][scale === 100 ? 0 : scale === 125 ? 1 : 2]);

  const groups = $derived(endpointGroups(app?.outputs.list ?? []));
  const notUsed: EndpointLook = { name: t("settings.notUsed"), side: null, link: null };
  const inputGroups = $derived(endpointGroups(app?.inputs.list ?? [], "input"));
  const inputs = $derived([...inputGroups.usable, { id: "", look: notUsed }]);
  const dash = t("meter.playTargetNone");

  function testTone(): void {
    if (settings?.draftOutput) send({ type: "testTone", output: settings.draftOutput });
  }

  // ---- くわしい設定 ----

  /** 「入口 1 ・ 2（USB マイク）」。チャンネルの数がわからない入口は名前だけ。 */
  function inputName(input: EndpointView): string {
    const channels = input.channels === null ? undefined : CHANNELS[Math.min(input.channels, 2)];
    return channels ? t("settings.inputChoice", { channels: t(`vocal.inputShort.${channels}`), name: input.name }) : input.name;
  }
  const chosenInput = $derived(app?.inputs.list.find((i) => i.id === app.inputs.chosen) ?? null);
  const audio = $derived(app?.audio ?? null);
  /** バッファの「動いている」数と、そのひとかたまりの長さ（モック F7-SettingsAdv の「動いている 256 ・ 5.3 ms」）。 */
  const running = $derived.by(() => {
    if (!audio?.running || audio.actualFrames === null) return null;
    const ms = audio.sampleRate ? msText((audio.actualFrames / audio.sampleRate) * 1000) : null;
    return ms === null ? t("settings.running", { value: audio.actualFrames }) : t("settings.runningLatency", { frames: audio.actualFrames, ms });
  });
  const bufferDiffers = $derived(!!audio?.running && audio.actualFrames !== null && audio.actualFrames !== audio.requestedFrames);
  const rateDiffers = $derived(!!audio?.running && audio.sampleRate !== null && audio.sampleRate !== audio.requestedSampleRate);
  const latencyText = $derived.by(() => {
    const l = audio?.latency;
    if (!l) return "";
    const ms = l.usedMs === null ? dash : msText(l.usedMs);
    if (l.used === "measured") return t("settings.latencyMeasured", { ms });
    if (l.used === "manual") return t("latency.ms", { ms });
    return t("settings.latencyReported", { ms });
  });

  /** 「手で決める…」: 遅れの合わせの行の数が、その場で入力の枠になる（仕様書 3 章 94-19。0〜500 ms は core がおさえる）。 */
  let typingLatency = $state(false);
  /** 打った字を ms に。空なら手で決めた数をやめる（null）。読めなければ undefined。 */
  function parseLatency(text: string): number | null | undefined {
    const s = text.normalize("NFKC").replace(/\s+/g, "").replace(/ms$/i, "");
    if (s === "") return null;
    if (!/^(\d+(\.\d*)?|\.\d+)$/.test(s)) return undefined;
    return Number(s);
  }
  function submitLatency(text: string): void {
    typingLatency = false;
    const ms = parseLatency(text);
    if (ms !== undefined) send({ type: "settingsSetManualLatency", ms });
  }

  /** ▾ を押したら、その下に一覧（右クリックの一覧と同じ形）。 */
  function choose(e: MouseEvent, entries: MenuEntry[]): void {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    openContextMenu(r.left, r.bottom, entries);
  }
  function inputRows(): MenuEntry[] {
    const usable = inputGroups.usable.flatMap((u) => app!.inputs.list.filter((i) => i.id === u.id));
    return [
      ...usable.map(
        (i): MenuEntry => ({
          kind: "row",
          label: "settings.input",
          text: inputName(i),
          usable: true,
          checked: i.id === app!.inputs.chosen,
          run: () => send({ type: "settingsSelectInput", input: i.id }),
        }),
      ),
      { kind: "row", label: "settings.notUsed", usable: true, checked: app!.inputs.chosen === null, run: () => send({ type: "settingsSelectInput", input: null }) },
    ];
  }
  function bufferRows(): MenuEntry[] {
    return audio!.bufferChoices.map((frames) => ({
      kind: "row",
      label: "settings.buffer",
      text: String(frames),
      usable: true,
      checked: frames === audio!.requestedFrames,
      run: () => send({ type: "settingsSetBuffer", frames }),
    }));
  }
  function rateRows(): MenuEntry[] {
    return audio!.sampleRateChoices.map((hz) => ({
      kind: "row",
      label: "settings.rateHz",
      text: t("settings.rateHz", { rate: hz }),
      usable: true,
      checked: hz === audio!.requestedSampleRate,
      run: () => send({ type: "settingsSetSampleRate", hz }),
    }));
  }
  function formatRows(): MenuEntry[] {
    return RECORD_FORMATS.map((f) => ({
      kind: "row",
      label: "settings.bits",
      text: t("settings.bits", { bits: f.bits }),
      usable: true,
      checked: f.format === audio!.recordFormat,
      run: () => send({ type: "settingsSetRecordFormat", format: f.format }),
    }));
  }
  const bits = $derived(RECORD_FORMATS.find((f) => f.format === audio?.recordFormat)?.bits ?? 24);
</script>

{#snippet dropdown(label: string, test: string, open: (e: MouseEvent) => void)}
  <button type="button" class="drop" aria-haspopup="menu" data-test={test} onclick={open}>{label}<span class="label">▾</span></button>
{/snippet}

{#snippet keyboardRows()}
  {#each app?.keyboards.list ?? [] as k (k.id)}
    <EndpointRow
      look={{ name: k.name, side: t("friendly.keyboard"), link: "connected" }}
      selected={k.id === app?.keyboards.active}
      disabled={false}
      still
      size="settings"
    />
  {/each}
{/snippet}

{#if app && settings}
  <div class="place">
    <div class="lead"></div>
    <div class="dialog" role="dialog" aria-label={advanced ? t("settings.advTitle") : t("settings.title")} data-test="settings-dialog" use:movableDialog>
      <div class="head" data-dialog-head>
        <span class="title">{advanced ? t("settings.advTitle") : t("settings.title")}</span>
        <span class="spacer"></span>
        <button type="button" class="close" data-test="settings-close" onclick={() => send({ type: "settingsCancel" })}>
          {t("settings.close")}<span class="mono">{t("key.esc")}</span>
        </button>
      </div>
      {#if advanced}
        <!-- 2 段目の見出し（仕様書 2 章「ダイアログの 2 段目へ進む行」の行き先。モック F7-SettingsAdv）。 -->
        <div class="crumb">
          <button type="button" class="back" data-test="settings-advanced" onclick={() => send({ type: "settingsToggleAdvanced" })}>{t("settings.advBack")}</button>
          <span class="title">{t("settings.advanced")}</span>
        </div>
      {/if}

      <ScrollArea data-gl-scroll data-test="settings-body">
        <!-- スクロールする中身ぜんたいが 1 つの区画（メーターと鍵盤で 1 枚）。くわしい設定で slot がなくなっても区画は持つ。 -->
        <div use:glRegion={"settings"}>
          {#if !advanced}
            <section>
              <span class="label">{t("settings.outputs")}</span>
              {#each groups.usable as output (output.id)}
                <EndpointRow
                  look={output.look}
                  selected={settings.draftOutput === output.id}
                  disabled={false}
                  size="settings"
                  onpick={() => send({ type: "settingsSelectOutput", output: output.id })}
                />
              {/each}
              <div class="line">
                <Button test="settings-test-tone" disabled={!settings.draftOutput} onclick={testTone}>
                  {t("endpoint.testTone")}
                </Button>
                <div class="level" use:glSlot={{ kind: "meter", source: "master", gap: meterGap }}></div>
              </div>
              <UnusableEndpoints
                rows={groups.unusable}
                heading={t("msg.unusableHeading")}
                size="settings"
                note={t("msg.unusableHeadingSettings")}
              />
            </section>

            <section>
              <span class="label">{t("settings.voiceInput")}</span>
              {#each inputs as input (input.id)}
                <EndpointRow look={input.look} selected={input.id === (app.inputs.chosen ?? "")} disabled size="settings" />
              {/each}
              <div class="line disabled">
                <span>{t("settings.level")}</span>
                <div class="level off"></div>
              </div>
              <UnusableEndpoints rows={inputGroups.unusable} heading={t("msg.unusableInputHeading")} size="settings" />
            </section>

            <section>
              <span class="label">{t("settings.keyboard")}</span>
              {#if app.keyboards.list.length > 0}
                {@render keyboardRows()}
              {:else}
                <div class="line"><span class="label">{t("meter.noKeyboard")}</span></div>
              {/if}
              <div class="line">
                <span class="label">{t("settings.keysLight")}</span>
                <div
                  class="keys"
                  style:--white-keys={WHITE_KEYS}
                  use:glSlot={{ kind: "keyboard", size: "slim", firstNote: FIRST_NOTE, lastNote: LAST_NOTE }}
                ></div>
              </div>
            </section>

            <section>
              <span class="label" use:term={"遅れ（レイテンシ）"}>{t("settings.latency")}</span>
              <div class="line">
                <span class="label field">{t("settings.keyToSound")}</span>
                {#if app.audio.latencyMs !== null}
                  <span class="mono strong" data-test="latency">{t("settings.aboutMs", { ms: msText(app.audio.latencyMs) })}</span>
                {/if}
                <span class="label">{t("settings.latencyHint")}</span>
              </div>
              <div class="line disabled">
                <span class="field">{t("settings.recordedPosition")}</span>
                <span>{t("settings.autoFixOn")}</span>
                <span>{t("settings.autoFixHint")}</span>
              </div>
            </section>
            <button
              type="button"
              class="adv"
              data-test="settings-advanced"
              onclick={() => send({ type: "settingsToggleAdvanced" })}
            >
              <span class="label">▸</span>
              <span class="strong">{t("settings.advanced")}</span>
              <span class="label">
                {t("settings.advancedSummary", {
                  system: app.audio.system,
                  rate: app.audio.sampleRate ?? dash,
                  buffer: app.audio.actualFrames ?? dash,
                })}
              </span>
              <span class="spacer"></span>
              <span class="label">{t("settings.advancedHint")}</span>
            </button>
          {:else}
            <div class="adv-body" data-test="settings-adv">
              <div class="row">
                <span class="strong name">{t("settings.input")}</span>
                {@render dropdown(chosenInput ? inputName(chosenInput) : t("settings.notUsed"), "settings-input", (e) => choose(e, inputRows()))}
              </div>
              <div class="row" class:joined={bufferDiffers}>
                <span class="strong name" use:term={"バッファ"}>{t("settings.buffer")}</span>
                {@render dropdown(String(app.audio.requestedFrames), "settings-buffer", (e) => choose(e, bufferRows()))}
                {#if running}<span class="label" data-test="buffer-actual">{running}</span>{/if}
              </div>
              {#if bufferDiffers}
                <div class="note" data-test="buffer-differs">
                  {t("settings.notAsRequested", { requested: app.audio.requestedFrames, actual: app.audio.actualFrames! })}
                </div>
              {/if}
              <div class="row" class:joined={rateDiffers}>
                <span class="strong name" use:term={"サンプルレート"}>{t("settings.sampleRate")}</span>
                {@render dropdown(t("settings.rateHz", { rate: app.audio.requestedSampleRate }), "settings-rate", (e) => choose(e, rateRows()))}
                {#if app.audio.running && app.audio.sampleRate !== null}
                  <span class="label" data-test="rate-actual">{t("settings.running", { value: app.audio.sampleRate })}</span>
                {/if}
              </div>
              {#if rateDiffers}
                <div class="note" data-test="rate-differs">
                  {t("settings.notAsRequested", { requested: app.audio.requestedSampleRate, actual: app.audio.sampleRate! })}
                </div>
              {/if}
              <div class="row">
                <span class="strong name">{t("settings.recordFormat")}</span>
                {@render dropdown(t("settings.bits", { bits }), "settings-format", (e) => choose(e, formatRows()))}
              </div>
              <div class="row">
                <span class="strong name" use:term={"遅れの補正"}>{t("settings.latencyAlign")}</span>
                {#if typingLatency}
                  <span class="latency-field">
                    <ValueField
                      initial={app.audio.latency.usedMs === null ? "" : msText(app.audio.latency.usedMs)}
                      label={t("settings.latencyAlign")}
                      unit={t("settings.latencyUnit")}
                      onsubmit={submitLatency}
                      oncancel={() => (typingLatency = false)}
                    />
                  </span>
                {:else}
                  <span class="mono strong" data-test="latency-used">{latencyText}</span>
                {/if}
                <Button test="settings-measure" onclick={() => send({ type: "openLatencyMeasure" })}>{t("button.measureLatency")}</Button>
                <Button test="settings-manual" on={typingLatency} onclick={() => (typingLatency = true)}>{t("button.manualLatency")}</Button>
              </div>
              <div class="row">
                <span class="strong name">{t("plugin.places")}</span>
                <Button test="settings-places" on={app.plugins.placesOpen} onclick={() => send({ type: "togglePluginPlaces" })}>{t("button.openPlaces")}</Button>
              </div>
              {#if app.plugins.placesOpen}
                <PluginPlaces plugins={app.plugins} />
              {/if}
            </div>
          {/if}
        </div>
      </ScrollArea>

      <div class="foot">
        {#if advanced}
          <Button primary test="settings-done" onclick={() => send({ type: "settingsCancel" })}>{t("button.close")}</Button>
        {:else}
          <Button test="settings-cancel" onclick={() => send({ type: "settingsCancel" })}>{t("button.cancel")}</Button>
          <Button primary test="settings-apply" onclick={() => send({ type: "settingsApply" })}>{t("settings.ok")}</Button>
        {/if}
      </div>
    </div>
    <div class="lead"></div>
  </div>
{/if}

<style>
  /* 窓の上下のまん中（仕様書 3 章）。高さは中身に合わせ、窓の上下 24 ずつのあきまで。それをこえたら、中身をスクロールする。 */
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
    color-scheme: dark;
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
  section {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-3) var(--space-4);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--size-row);
  }
  .line.disabled {
    color: var(--color-gray-7);
  }
  .field {
    flex-shrink: 0;
    width: var(--mock-settings-label);
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
  .spacer {
    flex: 1;
  }
  .level {
    width: var(--mock-settings-meter);
    height: calc(var(--part-meter-horizontal) * 2 + var(--space-1));
  }
  .level.off {
    height: var(--part-meter-horizontal);
    background: var(--color-gray-0);
  }
  .keys {
    flex-shrink: 0;
    width: calc(var(--mock-input-meter-key-slim-w) * var(--white-keys) + var(--line-hair));
    height: var(--mock-input-meter-keys-slim-h);
  }
  .adv {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    width: 100%;
    height: var(--size-toolbar);
    padding: 0 var(--space-3);
    border: 0;
    background: var(--color-gray-4);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .adv:focus-visible,
  .back:focus-visible,
  .drop:focus-visible,
  .close:focus-visible {
    outline: var(--line-hair) solid var(--color-gray-10);
    outline-offset: var(--line-hair);
  }
  /* くわしい設定（モック F7-SettingsAdv）: 名前 120 の行が並ぶ。行は高さ 32 から、下に gray.4 の線。 */
  .crumb {
    flex-shrink: 0;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--part-dialognext-height);
    padding: 0 var(--space-4);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .back {
    padding: 0;
    border: 0;
    background: none;
    color: var(--color-gray-8);
    font: inherit;
    cursor: pointer;
  }
  .back:hover {
    color: var(--color-gray-10);
  }
  .adv-body {
    display: flex;
    flex-direction: column;
    padding: var(--space-1) var(--space-4) var(--space-2);
  }
  .row {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--space-2);
    min-height: var(--size-toolbar);
    padding: var(--space-1) 0;
    border-bottom: var(--line-hair) solid var(--color-gray-4);
  }
  .row.joined {
    border-bottom: 0;
  }
  /* 「手で決める…」の枠（「8.0」と「ms」）。枠の幅は欄の打ちこむ枠と同じ。 */
  .latency-field {
    display: flex;
  }
  .latency-field :global(input) {
    flex: none;
    width: var(--mock-inspector-bulk-field);
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
  .note {
    padding: var(--space-1) 0 var(--space-2) calc(var(--mock-settings-adv-label) + var(--space-2));
    border-bottom: var(--line-hair) solid var(--color-gray-4);
    color: var(--color-gray-8);
    white-space: normal;
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

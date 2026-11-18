<!-- 音の設定（モック F-Settings のおもて、F-SettingsAdvFixed の奥。仕様書 2 章「ダイアログ」）。 -->
<script lang="ts">
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

  const SYSTEMS = [
    { name: "PipeWire", key: "settings.systemPipeWire" },
    { name: "JACK", key: "settings.systemJack" },
    { name: "ALSA", key: "settings.systemAlsa" },
  ] as const;
  const FIRST_NOTE = 36;
  const LAST_NOTE = 84;
  const WHITE_KEYS = keyboardLayout(FIRST_NOTE, LAST_NOTE).whiteCount;
  const RATES = [44100, 48000, 96000];
  const BUFFERS = [64, 128, 256, 512, 1024];
  const AROUND = /(約 [\d.]+ ms)/;

  const app = $derived(current.state);
  const settings = $derived(app?.workspace.settings ?? null);
  const advanced = $derived(settings?.advancedOpen ?? false);
  const scale = $derived((app?.workspace.scale ?? 100) as Scale);
  const meterGap = $derived(space["space.1"][scale === 100 ? 0 : scale === 125 ? 1 : 2]);

  const groups = $derived(endpointGroups(app?.outputs.list ?? []));
  const outputs = $derived(advanced ? groups.usable.filter((o) => o.id === settings?.draftOutput) : groups.usable);
  const notUsed: EndpointLook = { name: t("settings.notUsed"), side: null, link: null };
  const inputGroups = $derived(endpointGroups(app?.inputs.list ?? []));
  const inputs = $derived.by(() => {
    if (!app) return [];
    const rows = [...inputGroups.usable, { id: "", look: notUsed }];
    if (!advanced) return rows;
    return rows.filter((r) => r.id === (app.inputs.chosen ?? ""));
  });
  const keyboards = $derived.by(() => {
    if (!app) return [];
    const list = app.keyboards.list;
    return advanced ? list.filter((k) => k.id === app.keyboards.active) : list;
  });

  const withCurrent = (list: number[], value: number | null) =>
    value === null || list.includes(value) ? list : [...list, value].sort((a, b) => a - b);
  const rates = $derived(withCurrent(RATES, app?.audio.sampleRate ?? null));
  const buffers = $derived(withCurrent(BUFFERS, app?.audio.requestedFrames ?? null));
  const dash = "—";

  const fixed = $derived.by(() => {
    const m = app?.status.message;
    if (m?.id !== "bufferFixed") return null;
    const values = {
      actual: m.actual,
      requested: m.requested,
      latencyMs: msText(m.latencyMs),
      latencyMsAtRequested: msText(m.latencyMsAtRequested),
    };
    // 見出しはモックの文。本文は仕様書 6 章の文で、見出しと同じ 1 文目をのぞき、モックのとおり 3 つに分ける。
    const sentences = t("msg.bufferFixedDetail", values).match(/[^。]*。/g) ?? [];
    return {
      title: t("settings.bufferFixedTitle", { actual: m.actual }),
      paragraphs: [sentences.slice(1, 4), sentences.slice(4, 6), sentences.slice(6)].map((s) => s.join("")),
    };
  });

  function testTone(): void {
    if (settings?.draftOutput) send({ type: "testTone", output: settings.draftOutput });
  }
</script>

{#snippet keyboardRows()}
  {#each keyboards as k (k.id)}
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
  <div class="place" class:advanced>
    <div class="lead"></div>
    <div class="dialog" role="dialog" aria-label={t("settings.title")} data-test="settings-dialog" use:movableDialog>
      <div class="head" data-dialog-head>
        <span class="title">{t("settings.title")}</span>
        <span class="spacer"></span>
        <button type="button" class="close" data-test="settings-close" onclick={() => send({ type: "settingsCancel" })}>
          {t("settings.close")}<span class="mono">{t("key.esc")}</span>
        </button>
      </div>

      <ScrollArea data-gl-scroll data-test="settings-body">
        <section>
          <span class="label">{t("settings.outputs")}</span>
          {#each outputs as output (output.id)}
            <EndpointRow
              look={output.look}
              selected={settings.draftOutput === output.id}
              disabled={false}
              size="settings"
              onpick={() => send({ type: "settingsSelectOutput", output: output.id })}
            />
          {/each}
          {#if !advanced}
            <div class="line">
              <Button test="settings-test-tone" disabled={!settings.draftOutput} onclick={testTone}>
                {t("endpoint.testTone")}
              </Button>
              <div class="level" use:glRegion={"settingsMeter"} use:glSlot={{ kind: "meter", source: "master", gap: meterGap }}></div>
            </div>
            <UnusableEndpoints
              rows={groups.unusable}
              heading={t("msg.unusableHeading")}
              size="settings"
              note={t("msg.unusableHeadingSettings")}
            />
          {/if}
        </section>

        <section>
          <span class="label">{t("settings.voiceInput")}</span>
          {#each inputs as input (input.id)}
            <EndpointRow look={input.look} selected={input.id === (app.inputs.chosen ?? "")} disabled size="settings" />
          {/each}
          {#if !advanced}
            <div class="line disabled">
              <span>{t("settings.level")}</span>
              <div class="level off"></div>
            </div>
            <UnusableEndpoints rows={inputGroups.unusable} heading={t("msg.unusableInputHeading")} size="settings" />
          {/if}
        </section>

        <section>
          <span class="label">{t("settings.keyboard")}</span>
          {#if keyboards.length > 0}
            {@render keyboardRows()}
          {:else}
            <div class="line"><span class="label">{t("meter.noKeyboard")}</span></div>
          {/if}
          {#if !advanced}
            <div class="line">
              <span class="label">{t("settings.keysLight")}</span>
              <div
                class="keys"
                style:--white-keys={WHITE_KEYS}
                use:glRegion={"settingsKeys"}
                use:glSlot={{ kind: "keyboard", size: "slim", firstNote: FIRST_NOTE, lastNote: LAST_NOTE }}
              ></div>
            </div>
          {/if}
        </section>

        {#if !advanced}
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
          <div class="adv-open">
            <button
              type="button"
              class="adv"
              data-test="settings-advanced"
              onclick={() => send({ type: "settingsToggleAdvanced" })}
            >
              <span class="strong">▾</span>
              <span class="strong">{t("settings.advanced")}</span>
              <span class="spacer"></span>
              <span class="label">{t("settings.advancedOpenHint")}</span>
            </button>
            <div class="adv-body">
              <div class="line">
                <span class="label field">{t("settings.system")}</span>
                <div class="seg">
                  {#each SYSTEMS as s (s.name)}
                    {#if s.name === app.audio.system}
                      <Button on>{t(s.key)}</Button>
                    {:else}
                      <Button disabled>{t(s.key)}</Button>
                    {/if}
                  {/each}
                </div>
              </div>
              <div class="line">
                <span class="label field">{t("settings.outputDevice")}</span>
                <span class="mono strong value">{app.audio.outputDevice ?? ""}</span>
              </div>
              <div class="line">
                <span class="label field" use:term={"MIDI"}>{t("settings.midiInput")}</span>
                <span class="mono strong value">{app.audio.midiInput ?? ""}</span>
              </div>
              <div class="line">
                <span class="label field" use:term={"サンプルレート"}>{t("settings.sampleRate")}</span>
                <div class="seg">
                  {#each rates as r (r)}
                    {#if r === app.audio.sampleRate}
                      <Button on>{r}</Button>
                    {:else}
                      <Button disabled>{r}</Button>
                    {/if}
                  {/each}
                </div>
                <span class="label">{t("settings.hz")}</span>
              </div>
              <div class="line">
                <span class="label field" use:term={"バッファ"}>{t("settings.bufferRequested")}</span>
                <div class="seg">
                  {#each buffers as b (b)}
                    {#if b === app.audio.requestedFrames}
                      <Button on>{b}</Button>
                    {:else}
                      <Button disabled>{b}</Button>
                    {/if}
                  {/each}
                </div>
                <span class="label">{t("settings.samples")}</span>
              </div>
              <div class="line">
                <span class="label field">{t("settings.bufferActual")}</span>
                <span class="mono strong big" data-test="buffer-actual">{app.audio.actualFrames ?? dash}</span>
                <span class="label">
                  {app.audio.actualFrames !== null && app.audio.actualFrames !== app.audio.requestedFrames
                    ? t("settings.bufferDiffers")
                    : t("settings.samples")}
                </span>
              </div>
              {#if fixed}
                <div class="fixed" data-test="buffer-fixed">
                  <span class="strong bold">{fixed.title}</span>
                  {#each fixed.paragraphs as p, i (i)}
                    <span class:label={i === 2}>
                      {#each p.split(AROUND) as piece, j (j)}
                        {#if AROUND.test(piece)}<span class="mono strong">{piece}</span>{:else}{piece}{/if}
                      {/each}
                    </span>
                  {/each}
                </div>
              {/if}
              <div class="line">
                <span class="label field" use:term={"遅れの補正"}>{t("settings.latencyComp")}</span>
                <div class="seg">
                  <Button disabled>{t("settings.auto")}</Button>
                  <Button disabled>{t("settings.manual")}</Button>
                </div>
                <Button disabled>{t("settings.remeasure")}</Button>
              </div>
            </div>
          </div>
        {/if}
      </ScrollArea>

      <div class="foot">
        <Button test="settings-cancel" onclick={() => send({ type: "settingsCancel" })}>{t("button.cancel")}</Button>
        <Button primary test="settings-apply" onclick={() => send({ type: "settingsApply" })}>{t("settings.ok")}</Button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* 上はモックの位置。窓が低ければ、上下 24 ずつのあきまで上へ寄せ、それでも入らなければ中身をスクロールする。 */
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
    flex: 0 1 calc(var(--mock-settings-top) - var(--part-dialog-margin));
  }
  .place.advanced .lead {
    flex-basis: calc(var(--mock-settings-top-advanced) - var(--part-dialog-margin));
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
  .bold {
    font-weight: var(--text-s-weight-600);
  }
  .mono {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  .big {
    font-size: var(--text-m-size);
    line-height: var(--text-m-line-height);
  }
  .value {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
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
  .close:focus-visible {
    outline: var(--line-hair) solid var(--color-gray-10);
    outline-offset: var(--line-hair);
  }
  .adv-open {
    display: flex;
    flex-direction: column;
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .adv-body {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-4) var(--space-3);
    background: var(--color-gray-2);
  }
  .seg {
    display: flex;
  }
  .fixed {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    margin: var(--space-1) 0;
    padding: var(--space-2);
    border: var(--line-hair) solid var(--color-gray-8);
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

<!-- 入力計（仕様書 3 章の表の入力計の行。モック F-Connect1・2・3、F-Empty、F-MeterLongName、Opt-Meter 1-B）。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import { keyIsDown } from "../bridge";
  import type { Scale } from "../design/tokens";
  import { keyboardLayout } from "../gl/keyboard";
  import { glSlot } from "../gl/slot";
  import { term, tip } from "../lib/tip.svelte";
  import { current, onEvent } from "../state/app.svelte";
  import { t } from "../text/ja";
  import { heldText, pad3, signed4, SUSTAIN_ON } from "./meterFormat";
  import { metrics } from "./metrics.svelte";
  import { charCount, meterFit, nameWidthPx, sentences, type MeterFit, type NameWidth } from "./meterFit";

  const FIRST_NOTE = 36;
  const LAST_NOTE = 84;
  const WHITE_KEYS = keyboardLayout(FIRST_NOTE, LAST_NOTE).whiteCount;
  const TRACK = "\u0001";
  const INSTRUMENT = "\u0002";
  const TRACK_LABEL = "\u0003";

  const app = $derived(current.state);
  const slim = $derived(app?.workspace.layout.meter === "slim");
  const scale = $derived((app?.workspace.scale ?? 100) as Scale);
  // 鍵盤が 2 台以上なら、名前は最後に弾いた鍵盤。うしろに残りの台数（仕様書 3 章）。
  // 入力そのものが止まっている間は、鍵盤が抜けたときと同じ出しかた。
  const keyboard = $derived(
    app?.keyboards.inputStopped ? null : (app?.keyboards.list.find((k) => k.id === app.keyboards.active) ?? null),
  );
  const name = $derived(keyboard?.name ?? t("meter.noKeyboard"));
  const others = $derived(keyboard ? app!.keyboards.list.length - 1 : 0);
  const more = $derived(others > 0 ? t("meter.otherKeyboards", { count: others }) : "");
  const allNames = $derived(app?.keyboards.list.map((k) => k.name).join(" ・ ") ?? "");
  const right = $derived.by(() => {
    if (!app) return null;
    if (app.keyboards.inputStopped) return { kind: "inputStopped" as const };
    if (!keyboard) {
      if (app.keyboards.lost !== null) return { kind: "lost" as const };
      return { kind: "missing" as const };
    }
    if (app.outputs.chosen === null) return { kind: "noOutput" as const };
    const track = app.song.tracks.find((x) => x.number === app.song.playTarget);
    return track ? { kind: "target" as const, track } : null;
  });

  // 押している鍵は計器で受け、鍵の on/off は計器を待たずに当てる。
  let held = $state<number[]>([]);
  let velocity = $state(0);
  $effect(() => {
    const tel = current.telemetry;
    if (!tel) return;
    const notes: number[] = [];
    for (let n = 0; n < 128; n++) if (keyIsDown(tel, n)) notes.push(n);
    held = notes;
    velocity = tel.lastVelocity;
  });
  onMount(() =>
    onEvent((e) => {
      if (e.type !== "note") return;
      if (e.on && e.velocity > 0) {
        if (!held.includes(e.note)) held = [...held, e.note];
        velocity = e.velocity;
      } else {
        held = held.filter((n) => n !== e.note);
      }
    }),
  );

  const sustain = $derived(current.telemetry?.sustain ?? 0);
  const bend = $derived(current.telemetry?.pitchBend ?? 0);
  const modulation = $derived(current.telemetry?.modulation ?? 0);
  const live = $derived(held.length > 0);

  /**
   * 鳴る先の値を、トラックの部分と音源の部分（音源の名前と、そのまわりの「 ・ 」）に分ける。
   * 細いときは音源の名前を出さない（モック A-PianoRoll・F-History の「弾くと鳴る先 1 ピアノ」）。
   */
  function targetPieces(trackName: string): { text: string; instrument: boolean }[] {
    const track = t("track.label", { number: TRACK, name: trackName });
    const filled = t("meter.playTargetWithInstrument", { track: TRACK_LABEL, instrument: INSTRUMENT });
    return filled
      .split(new RegExp(`(${TRACK_LABEL})`))
      .flatMap((p) =>
        p === TRACK_LABEL
          ? track.split(new RegExp(`(${TRACK})`)).map((text) => ({ text, instrument: false }))
          : [{ text: p, instrument: true }],
      )
      .filter((p) => p.text !== "");
  }

  /** 右はしの案内の文。tone: 1 文目と残りの色（「鍵盤がない」はモック F-Connect1 で色を分けている）。 */
  const message = $derived.by(() => {
    if (right?.kind === "missing") return { parts: sentences(t("msg.keyboardMissing")), first: "strong", rest: "dim" };
    if (right?.kind === "lost")
      return { parts: sentences(t("msg.keyboardLost", { keyboard: app!.keyboards.lost! })), first: "strong", rest: "strong" };
    if (right?.kind === "inputStopped")
      return { parts: sentences(t("msg.keyboardInputStopped")), first: "strong", rest: "strong" };
    if (right?.kind === "noOutput") return { parts: sentences(t("msg.noOutputMeter")), first: "accent", rest: "accent" };
    return null;
  });

  // 横の入れかた: 見えない写し（ruler）で、ちぢめない幅を測る。
  let row = $state<HTMLElement>();
  let ruler = $state<HTMLElement>();
  let fit = $state<MeterFit>({
    nameWidth: "size.meter.name",
    pitchBend: true,
    modulation: true,
    narrowKeys: false,
    instrument: true,
    targetLabel: true,
    sustain: true,
    sentences: Infinity,
  });
  let nameNatural = $state(0);

  function measure(): void {
    if (!row || !ruler) return;
    const style = getComputedStyle(row);
    const gap = parseFloat(style.columnGap) || 0;
    const available = row.clientWidth - (parseFloat(style.paddingLeft) || 0) - (parseFloat(style.paddingRight) || 0);
    const width = (part: string) => ruler!.querySelector<HTMLElement>(`[data-part="${part}"]`)?.offsetWidth ?? 0;
    const nameW = width("name");
    const bendW = keyboard ? width("bend") + gap : 0;
    const modW = keyboard ? width("mod") + gap : 0;
    const keyW = (n: string) => metrics.px(`mock.input-meter.key-${n}-w`);
    // ラベルと値は縦にならぶので、鳴る先の幅は広いほう。
    const labelW = width("target-label");
    const valueW = width("target-value");
    let instrumentW = 0;
    for (const el of ruler.querySelectorAll<HTMLElement>('[data-part="target-instrument"]')) instrumentW += el.offsetWidth;
    const trackW = valueW - instrumentW;
    nameNatural = nameW;
    fit = meterFit({
      meter: slim ? "slim" : "full",
      scale,
      nameChars: charCount(name + (more ? ` ${more}` : "")),
      available,
      rest: ruler.scrollWidth - nameW - bendW - modW,
      name: nameW,
      pitchBend: bendW,
      modulation: modW,
      narrowKeys: (keyW("full") - keyW("slim")) * WHITE_KEYS,
      instrument: Math.max(labelW, valueW) - Math.max(labelW, trackW),
      targetLabel: Math.max(0, labelW - trackW),
      sustain: keyboard ? width("sustain") + gap : 0,
      sentences: message ? message.parts.map((_, i) => width(`msg-${i}`)) : undefined,
    });
  }

  onMount(() => {
    const observer = new ResizeObserver(() => measure());
    if (row) observer.observe(row);
    if (ruler) observer.observe(ruler);
    return () => observer.disconnect();
  });
  $effect(() => {
    // 状態が変わったら測りなおす（ResizeObserver のない所でも、決まりの通りに出す）。
    void [slim, scale, name, more, keyboard, right, message];
    measure();
  });

  const cssVar = (w: NameWidth) => `var(--${w.replaceAll(".", "-")})`;
  const truncated = $derived(nameNatural > 0 && nameNatural > nameWidthPx(fit.nameWidth, scale));
  const nameTip = $derived(others > 0 ? allNames : truncated ? name : null);
</script>

{#snippet body(measuring: boolean)}
  <div class="who" style:width={!measuring && !slim ? cssVar(fit.nameWidth) : null}>
    {#if !slim}<span class="label">{t("meter.input")}</span>{/if}
    <span
      class="who-name"
      data-part="name"
      style:max-width={!measuring && slim ? cssVar(fit.nameWidth) : null}
    >
      <span class="mark" class:on={keyboard !== null}></span>
      <span class="name-text" data-test="keyboard-name" use:tip={measuring ? null : nameTip}>{name}</span>
      {#if more}<span class="more" data-test="other-keyboards" use:tip={measuring ? null : nameTip}>{` ${more}`}</span>{/if}
    </span>
  </div>

  {#if measuring}
    <div class="keys"></div>
  {:else}
    <div
      class="keys"
      class:narrow={!slim && fit.narrowKeys}
      use:glSlot={{
        kind: "keyboard",
        size: slim ? "slim" : "full",
        narrow: !slim && fit.narrowKeys,
        firstNote: FIRST_NOTE,
        lastNote: LAST_NOTE,
        disabled: keyboard === null,
      }}
    ></div>
  {/if}

  {#if keyboard}
    <div class="item">
      {#if !slim}<span class="label">{t("meter.held")}</span>{/if}
      <span class="value" class:held={live} class:none={!live} data-test="held">
        {live ? heldText(held) : t("meter.heldNone")}
      </span>
    </div>

    <div class="item velocity">
      <span class="label" use:term={"ベロシティ"}>{t("meter.velocity")}</span>
      {#if slim}
        <span class="value" class:live>{velocity}</span>
      {:else}
        <span class="velocity-row">
          <span class="value" class:live>{velocity}</span>
          {#if measuring}
            <span class="velocity-bar"></span>
          {:else}
            <span class="velocity-bar" use:glSlot={{ kind: "velocity" }}></span>
          {/if}
        </span>
      {/if}
    </div>

    {#if measuring || fit.sustain}
      <div class="item" data-part="sustain">
        <span class="label" use:term={"サステイン"}>{t("meter.sustain")}</span>
        <span class="value" class:live data-test="sustain">
          {sustain >= SUSTAIN_ON ? t("meter.on") : t("meter.off")}{slim ? "" : ` ${pad3(sustain)}`}
        </span>
      </div>
    {/if}

    {#if measuring || fit.pitchBend}
      <div class="item" data-part="bend" data-test="pitch-bend">
        <span class="label" use:term={"ピッチベンド"}>{t("meter.pitchBend")}</span>
        <span class="value" class:live>{signed4(bend)}</span>
      </div>
    {/if}
    {#if measuring || fit.modulation}
      <div class="item" data-part="mod" data-test="modulation">
        <span class="label" use:term={"モジュレーション"}>{t("meter.modulation")}</span>
        <span class="value" class:live>{pad3(modulation)}</span>
      </div>
    {/if}
  {/if}

  <div class="spacer"></div>

  {#if message}
    <span class="message" data-test="meter-right" data-part="msg">
      {#each message.parts as part, i (i)}
        {#if measuring || i < fit.sentences}<span class={i === 0 ? message.first : message.rest} data-part="msg-{i}">{part}</span>{/if}
      {/each}
    </span>
  {:else if right?.kind === "target"}
    <div class="item target" data-test="meter-right" data-part="target">
      {#if measuring || fit.targetLabel}<span class="label" data-part="target-label">{t("meter.playTarget")}</span>{/if}
      <span class="target-value" data-part="target-value">
        {#each targetPieces(right.track.name) as piece, i (i)}
          {#if piece.instrument}
            {#if !slim && (measuring || fit.instrument)}
              <span data-part="target-instrument"
                >{#each piece.text.split(new RegExp(`(${INSTRUMENT})`)) as bit, j (j)}{#if bit === INSTRUMENT}<span
                      use:term={"仮の音"}>{t("instrument.placeholder")}</span
                    >{:else}{bit}{/if}{/each}</span
              >
            {/if}
          {:else if piece.text === TRACK}
            <span class="mono">{right.track.number}</span>
          {:else}{piece.text}{/if}
        {/each}
      </span>
    </div>
  {/if}
{/snippet}

{#if app}
  <div
    class="meter"
    class:slim
    class:empty={keyboard === null}
    style:--white-keys={WHITE_KEYS}
    bind:this={row}
  >
    {@render body(false)}
    <div class="meter ruler" class:slim aria-hidden="true" bind:this={ruler}>
      {@render body(true)}
    </div>
  </div>
{/if}

<style>
  .meter {
    position: relative;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-5);
    height: var(--size-meter-full);
    padding: 0 var(--space-3);
    background: var(--color-gray-0);
    border-bottom: var(--line-hair) solid var(--color-gray-6);
    color: var(--color-gray-9);
    font-family: var(--font-sans);
    font-size: var(--text-s-size);
    line-height: var(--text-s-line-height);
    font-weight: var(--text-s-weight-400);
    white-space: nowrap;
    overflow: hidden;
  }
  .meter.slim {
    gap: var(--space-4);
    height: var(--size-meter-slim);
  }
  .ruler {
    position: absolute;
    left: 0;
    top: 0;
    width: max-content;
    padding: 0;
    border: 0;
    visibility: hidden;
    pointer-events: none;
    overflow: visible;
  }
  .who {
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    min-width: 0;
  }
  .slim .who {
    flex-shrink: 1;
  }
  .label {
    color: var(--color-gray-8);
  }
  .who-name {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    color: var(--color-gray-10);
    font-weight: var(--text-s-weight-500);
  }
  .name-text {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mark {
    flex-shrink: 0;
    box-sizing: border-box;
    width: var(--part-endpoint-link);
    height: var(--part-endpoint-link);
    border: var(--line-hair) solid var(--color-gray-8);
  }
  .more {
    flex-shrink: 0;
    margin-left: calc(-1 * var(--space-2));
    white-space: pre;
  }
  .mark.on {
    background: var(--color-accent);
    border: 0;
  }
  .keys {
    flex-shrink: 0;
    width: calc(var(--mock-input-meter-key-full-w) * var(--white-keys) + var(--line-hair));
    height: var(--mock-input-meter-keys-full-h);
  }
  .keys.narrow {
    width: calc(var(--mock-input-meter-key-slim-w) * var(--white-keys) + var(--line-hair));
  }
  .slim .keys {
    width: calc(var(--mock-input-meter-key-slim-w) * var(--white-keys) + var(--line-hair));
    height: var(--mock-input-meter-keys-slim-h);
  }
  .item {
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
  }
  .slim .item {
    flex-direction: row;
    align-items: baseline;
    gap: var(--space-1);
  }
  .value {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    font-size: var(--text-m-size);
    line-height: var(--text-m-line-height);
    font-weight: var(--text-m-weight-400);
    color: var(--color-gray-8);
  }
  .slim .value {
    font-size: var(--text-s-size);
    line-height: var(--text-s-line-height);
    font-weight: var(--text-s-weight-400);
  }
  .value.live {
    color: var(--color-gray-10);
  }
  .value.held {
    color: var(--color-accent);
  }
  .value.none {
    color: var(--color-gray-7);
  }
  .velocity {
    width: var(--mock-input-meter-velocity);
  }
  .slim .velocity {
    width: auto;
  }
  .velocity-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .velocity-bar {
    flex: 1;
    height: var(--mock-input-meter-velocity-bar);
  }
  .spacer {
    flex: 1;
    min-width: 0;
  }
  .ruler .spacer {
    flex: none;
    width: 0;
  }
  .message {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .strong {
    color: var(--color-gray-10);
  }
  .dim {
    color: var(--color-gray-8);
  }
  .accent {
    color: var(--color-accent);
  }
  .target {
    align-items: flex-end;
    flex-shrink: 1;
    min-width: 0;
    overflow: hidden;
  }
  .target .label {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .target-value {
    color: var(--color-gray-10);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .slim .target-value {
    color: var(--color-gray-8);
  }
  .mono {
    font-family: var(--font-mono);
  }
</style>

<!-- 入力計（仕様書 3 章の表の入力計の行。モック F-Connect1・2・3、F-Empty、F-MeterLongName、Opt-Meter 1-B）。
     ベロシティの読み値の右に「固定」の切りかえ（docs/history/design-notes.ja.md B-66）。どの幅でも消さず、鍵盤がないときも出す。
     ● のついた歌のトラックか、選んだ歌のトラックがあるときは、左右に分けた形（仕様書 2 章「入力計の帯（左右に分けた形）」、3 章 31-C、
     モック F7-VocalTrack ・ R11-Fit ・ R12-Meters）: 左は鍵盤の計（ペダルなどは引っこめる）、右はマイクの計。せまいときは「弾くと鳴る先」、
     次に「モニター」の字、次に鍵盤の絵を引っこめ、鍵盤の名前を 88 で「…」（3 章 94-16 ・ 37-A）。「固定」と押している音は残す。
     マイクの計の棒は inputMeter の区画の中（区画も文脈もふやさない）。目盛は −60〜−12 が左半分、−12〜0 が右半分、地は gray.4、
     白い縦の線は 1 秒のピーク（dB の字と同じ数。3 章 94-17 ・ 38-B）。いまの dB の字は流れの外の箱で、同じ canvas の下（cover）。
     音の素材の案内（3 章 43-B ・ 44-A）が出ている間は、左に入力と、いま押している音だけ、右は pack/PackBand.svelte。 -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { keyIsDown, type TrackView } from "../bridge";
  import type { Scale } from "../design/tokens";
  import { keyboardLayout } from "../gl/keyboard";
  import { glRegion, glSlot } from "../gl/slot";
  import { term, tip, tipOf } from "../lib/tip.svelte";
  import { current, onEvent, send } from "../state/app.svelte";
  import { findTerm, t } from "../text/ja";
  import { dbNumber, PeakHold, textParts } from "./format";
  import { CLIP_HOLD_MS } from "../gl/dims";
  import { pad3, signed4, SUSTAIN_ON } from "./meterFormat";
  import { metrics } from "./metrics.svelte";
  import { charCount, meterFit, nameWidthPx, sentences, type MeterFit, type NameWidth } from "./meterFit";
  import NumberField from "./NumberField.svelte";
  import { packBand } from "./pack/pack";
  import PackBand from "./pack/PackBand.svelte";
  import { keysDrums, keysLabel } from "./pianoroll/drums";
  import { soundRefName, soundViewName, standInTip } from "./sounds";
  import { readVelocity, type Read } from "./valueClamp";

  const FIRST_NOTE = 36;
  const LAST_NOTE = 84;
  const WHITE_KEYS = keyboardLayout(FIRST_NOTE, LAST_NOTE).whiteCount;
  const TRACK = "\u0001";
  const INSTRUMENT = "\u0002";
  const TRACK_LABEL = "\u0003";

  const app = $derived(current.state);
  /** 弾くと鳴る先がドラムなら、いま押している音は楽器の名前（仕様書 3 章「ドラムの音源」）。 */
  const drums = $derived(app ? keysDrums(app) : false);
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
    // ためし弾き中（仕様書 3 章）: 鳴る先は棚でえらんだ音源。引っこめる順のどこでも消さない。
    const picked = app.shelf.selectedSound;
    if (picked !== null && !app.transport.recording) return { kind: "audition" as const, name: soundRefName(picked, app.library) };
    // カウントイン中と録音中は、録っているトラックと、いま録っているテイク（モック F-Connect4・F-ArrangeRec）。
    const rec = app.transport.recording;
    const recTrack = rec && app.song.tracks.find((x) => x.id === rec.track);
    if (rec && recTrack) return { kind: "target" as const, track: recTrack, take: rec.take };
    // 弾くと鳴る先は、鍵盤の行き先（歌のトラックを選んでいる間も、最後に選んでいた鍵盤のトラック。B-94-1）。
    const track = app.song.tracks.find((x) => x.id === app.song.keysTarget);
    return track ? { kind: "target" as const, track, take: null } : { kind: "none" as const };
  });

  // ---- 左右に分けた形（31-C） ----

  /** マイクの計に出す歌のトラック: 選んだトラックが歌ならそれ、ほかは ● のついた最初の歌のトラック。 */
  const vocal = $derived.by((): { track: TrackView; input: "mono1" | "mono2" | "stereo12"; monitor: boolean } | null => {
    if (!app) return null;
    const of = (x: TrackView | undefined) => (x && x.kind.kind === "vocal" ? { track: x, input: x.kind.input, monitor: x.kind.monitor } : null);
    return of(app.song.tracks.find((x) => x.id === app.song.playTarget)) ?? of(app.song.tracks.find((x) => x.kind.kind === "vocal" && x.kind.armed));
  });
  /** 音の素材の案内（44-A）が出ている間は、入力と、いま押している音だけ（左右に分けた形より先）。 */
  const pack = $derived(app ? packBand(app) : null);
  const split = $derived(vocal !== null && pack === null);

  /** いまの dB: 直近 1 秒のいちばん大きい値（マスターの dB と同じ。字を毎コマ変えない）。0 dB に届いたら 2 秒 rec。 */
  let micHold = new PeakHold(1000);
  let micPeak = $state(0);
  let micClipUntil = 0;
  let micClipped = $state(false);
  let micTrack: number | null = null;
  $effect(() => {
    const tel = current.telemetry;
    const id = vocal?.track.id ?? null;
    untrack(() => {
      if (id !== micTrack) {
        micTrack = id;
        micHold = new PeakHold(1000);
        micPeak = 0;
        micClipUntil = 0;
      }
      if (!tel || id === null) return;
      const mic = tel.mics.find((m) => m.track === id);
      const now = performance.now();
      if (mic) {
        micHold.push(Math.max(mic.peak[0], mic.peak[1]), now);
        if (mic.clipped) micClipUntil = now + CLIP_HOLD_MS;
      }
      micPeak = micHold.max(now);
      micClipped = now < micClipUntil;
    });
  });
  $effect(() => {
    if (!split) return;
    const timer = setInterval(() => {
      const now = performance.now();
      micPeak = micHold.max(now);
      micClipped = now < micClipUntil;
    }, 250);
    return () => clearInterval(timer);
  });
  /** 整数の dB（モック F7-VocalTrack の「−32 dB」）。−60 より小さいときは −∞。 */
  const micDb = $derived(micPeak > 0 ? Math.round(20 * Math.log10(micPeak)) : -Infinity);
  const micText = $derived(micDb < -60 ? t("common.dbSilent") : t("common.db", { value: dbNumber(micDb).replace(/\.0$/, "") }));

  let splitFit = $state({ target: true, monitor: true, keys: true });

  // 押している鍵は計器で受け、鍵の on/off は計器を待たずに当てる。
  let held = $state<number[]>([]);
  let velocity = $state(0);
  $effect(() => {
    const tel = current.telemetry;
    if (!tel) return;
    const notes: number[] = [];
    for (let n = 0; n < 128; n++) if (keyIsDown(tel, n)) notes.push(n);
    // 計器は 60 Hz で来る。同じ鍵なら書かない（書けば、そのたびに新しい $state の配列ができる）。
    const was = untrack(() => held);
    if (notes.length !== was.length || notes.some((n, i) => n !== was[i])) held = notes;
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

  // ベロシティ固定（B-66）。オンの間、読み値は固定の値で、押すと打ちこめる（1〜127。範囲の外は近いはしにして状態行で言う）。
  const fixed = $derived(app?.keyboards.velocityFixed ?? { on: false, value: 100 });
  function toggleFixed(): void {
    send({ type: "setVelocityFixed", on: !fixed.on, value: fixed.value });
  }
  function commitFixed(r: Read<number>): void {
    send({ type: "setVelocityFixed", on: true, value: r.value });
    if (r.clamp) send({ type: "clampValue", ...r.clamp });
  }
  function rejectFixed(): void {
    send({ type: "rejectValue", field: "velocity", kept: String(fixed.value), range: null });
  }
  const stepFixed = (r: Read<number>, delta: number) => String(Math.min(127, Math.max(1, r.value + delta)));

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

  /** 録っているときの鳴る先（文は「{track} ● テイク {take}」）: トラックの番号、名前、rec の「● テイク {take}」。 */
  function recordingPieces(trackName: string, take: number): { text: string; kind: "number" | "plain" | "rec" }[] {
    const [, ...rest] = textParts("meter.playTargetRecording", { track: TRACK_LABEL, take });
    const tail = rest.map((p) => p.text).join("");
    const lead = tail.length - tail.trimStart().length;
    const track = t("track.label", { number: TRACK, name: trackName })
      .split(new RegExp(`(${TRACK})`))
      .filter((x) => x !== "")
      .map((x) => ({ text: x, kind: x === TRACK ? ("number" as const) : ("plain" as const) }));
    return [...track, { text: tail.slice(0, lead), kind: "plain" }, { text: tail.slice(lead), kind: "rec" }];
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
    if (!row || !ruler || pack) return;
    if (split) {
      measureSplit();
      return;
    }
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

  /**
   * 分けた形: 半分ずつの箱に、ちぢめない幅が入るか。入らなければ「弾くと鳴る先」、次に「モニター」の字、次に鍵盤の絵
   * （鍵盤の名前は 88 まで）の順に引っこめ、両方の半分が入る所で止める。
   */
  function measureSplit(): void {
    const width = (part: string) => ruler!.querySelector<HTMLElement>(`[data-part="${part}"]`)?.offsetWidth ?? 0;
    const box = (part: string) => row!.querySelector<HTMLElement>(`:scope > [data-part="${part}"]`)?.clientWidth ?? 0;
    const half = ruler!.querySelector<HTMLElement>('[data-part="left"]');
    const gap = half ? parseFloat(getComputedStyle(half).columnGap) || 0 : 0;
    const left = [width("left")];
    left.push(left[0]! - (width("target") > 0 ? width("target") + gap : 0));
    left.push(left[1]!);
    left.push(left[2]! - width("keys") - gap - Math.max(0, width("who") - metrics.px("part.meter.name.split-narrow")));
    const right = [width("right"), width("right"), width("right") - width("monitor") - gap, width("right") - width("monitor") - gap];
    const [boxL, boxR] = [box("left"), box("right")];
    let stage = 0;
    while (stage < 3 && (left[stage]! > boxL || right[stage]! > boxR)) stage++;
    const next = { target: stage < 1, monitor: stage < 2, keys: stage < 3 };
    if (next.target !== splitFit.target || next.monitor !== splitFit.monitor || next.keys !== splitFit.keys) splitFit = next;
  }

  $effect(() => {
    const r = row;
    const u = ruler;
    if (!r || !u) return;
    const observer = new ResizeObserver(() => measure());
    observer.observe(r);
    observer.observe(u);
    return () => observer.disconnect();
  });
  $effect(() => {
    // 状態が変わったら測りなおす（ResizeObserver のない所でも、決まりの通りに出す）。
    void [slim, scale, name, more, keyboard, right, message, split, vocal];
    measure();
  });

  const cssVar = (w: NameWidth) => `var(--${w.replaceAll(".", "-")})`;
  const truncated = $derived(nameNatural > 0 && nameNatural > nameWidthPx(fit.nameWidth, scale));
  // meterFit may have left sentences out before the CSS "…", so the tip gives the whole message, not the text shown.
  const messageTip = (el: HTMLElement): string | null =>
    message && el.scrollWidth > el.clientWidth ? message.parts.join("") : null;
  const nameTip = $derived(others > 0 ? allNames : truncated ? name : null);
</script>

{#snippet who(measuring: boolean, fixedWidth: boolean)}
  <div
    class="who"
    data-part={fixedWidth ? null : "who"}
    style:width={!measuring && !slim && fixedWidth ? cssVar(fit.nameWidth) : null}
    style:max-width={!measuring && !fixedWidth && !splitFit.keys ? "var(--part-meter-name-split-narrow)" : null}
  >
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
{/snippet}

{#snippet heldItem()}
  <div class="item">
    {#if !slim}<span class="label">{t("meter.held")}</span>{/if}
    <span class="value" class:held={live} class:none={!live} data-test="held">
      {live ? keysLabel(held, drums) : t("meter.heldNone")}
    </span>
  </div>
{/snippet}

{#snippet reading(measuring: boolean)}
  {#if fixed.on}
    {#if measuring}
      <span class="value">{fixed.value}</span>
    {:else}
      <NumberField
        value={String(fixed.value)}
        parse={readVelocity}
        step={stepFixed}
        commit={commitFixed}
        reject={rejectFixed}
        label={t("meter.velocity")}
        chars={3}
        test="velocity-fixed-value"
        small={slim}
      />
    {/if}
  {:else if keyboard}
    <span class="value" class:live>{velocity}</span>
  {/if}
{/snippet}

{#snippet fixedToggle(measuring: boolean)}
  {#if measuring}
    <span class="fixed">{t("input.velocityFixed")}</span>
  {:else}
    <button class="fixed" class:on={fixed.on} aria-pressed={fixed.on} data-test="velocity-fixed" use:term={"ベロシティ固定"} onclick={toggleFixed}
      >{t("input.velocityFixed")}</button
    >
  {/if}
{/snippet}

{#snippet body(measuring: boolean)}
  {@render who(measuring, true)}

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
        {live ? keysLabel(held, drums) : t("meter.heldNone")}
      </span>
    </div>
  {/if}

  <div class="item velocity" data-test="velocity">
    <span class="label" use:term={"ベロシティ"}>{t("meter.velocity")}</span>
    {#if slim}
      {@render reading(measuring)}
      {@render fixedToggle(measuring)}
    {:else}
      <span class="velocity-row">
        <span class="velocity-main" class:wide={keyboard !== null}>
          {@render reading(measuring)}
          {#if keyboard}
            {#if measuring}
              <span class="velocity-bar"></span>
            {:else}
              <span class="velocity-bar" use:glSlot={{ kind: "velocity" }}></span>
            {/if}
          {/if}
        </span>
        {@render fixedToggle(measuring)}
      </span>
    {/if}
  </div>

  {#if keyboard}
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
    <span class="message" data-test="meter-right" data-part="msg" use:tipOf={measuring ? () => null : messageTip}>
      {#each message.parts as part, i (i)}
        {#if measuring || i < fit.sentences}<span class={i === 0 ? message.first : message.rest} data-part="msg-{i}">{part}</span>{/if}
      {/each}
    </span>
  {:else if right?.kind === "audition"}
    <div class="item target" data-test="meter-right" data-part="target">
      {#if measuring || fit.targetLabel}<span class="label" data-part="target-label">{t("meter.playTarget")}</span>{/if}
      <span class="target-value accent" data-part="target-value" data-test="meter-target"
        >{t("meter.auditionTarget", { instrument: right.name })}</span
      >
    </div>
  {:else if right?.kind === "none"}
    <div class="item target" data-test="meter-right" data-part="target">
      {#if measuring || fit.targetLabel}<span class="label" data-part="target-label">{t("meter.playTarget")}</span>{/if}
      <span class="target-value" data-part="target-value" data-test="meter-target">{t("meter.playTargetNone")}</span>
    </div>
  {:else if right?.kind === "target"}
    <div class="item target" data-test="meter-right" data-part="target">
      {#if measuring || fit.targetLabel}<span class="label" data-part="target-label">{t("meter.playTarget")}</span>{/if}
      <span class="target-value" data-part="target-value" data-test="meter-target">
        {#if right.take !== null}
          {#each recordingPieces(right.track.name, right.take) as piece, i (i)}
            {#if piece.kind === "number"}<span class="mono">{right.track.number}</span
              >{:else if piece.kind === "rec"}<span class="rec">{piece.text}</span>{:else}{piece.text}{/if}
          {/each}
        {:else}
        {#each targetPieces(right.track.name) as piece, i (i)}
          {#if piece.instrument}
            {#if !slim && (measuring || fit.instrument)}
              <span data-part="target-instrument"
                >{#each piece.text.split(new RegExp(`(${INSTRUMENT})`)) as bit, j (j)}{#if bit === INSTRUMENT}{@const standIn = right.track.standIn}{@const name = standIn
                      ? t("track.standIn")
                      : soundViewName(right.track.sound)}{#if standIn}<span class="stand-in" use:tip={standInTip(standIn, right.track.sound)}
                        >{name}</span
                      >{:else if findTerm(name)}<span use:term={name}>{name}</span>{:else}<span>{name}</span>{/if}{:else}{bit}{/if}{/each}</span
              >
            {/if}
          {:else if piece.text === TRACK}
            <span class="mono">{right.track.number}</span>
          {:else}{piece.text}{/if}
        {/each}
        {/if}
      </span>
    </div>
  {/if}
{/snippet}

{#snippet splitBody(measuring: boolean)}
  <div class="half" data-part="left" data-test="meter-keys-half">
    {@render who(measuring, false)}
    {#if measuring}
      <div class="keys narrow" class:slim-keys={slim} data-part="keys"></div>
    {:else if splitFit.keys}
      <div
        class="keys narrow"
        class:slim-keys={slim}
        use:glSlot={{
          kind: "keyboard",
          size: slim ? "slim" : "full",
          narrow: !slim,
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
          {live ? keysLabel(held, drums) : t("meter.heldNone")}
        </span>
      </div>
    {/if}
    <div class="item velocity" data-test="velocity">
      <span class="label" use:term={"ベロシティ"}>{t("meter.velocity")}</span>
      {#if slim}
        {@render reading(measuring)}
        {@render fixedToggle(measuring)}
      {:else}
        <span class="velocity-row">
          {@render reading(measuring)}
          {@render fixedToggle(measuring)}
        </span>
      {/if}
    </div>
    <div class="spacer"></div>
    {#if message}
      {#if measuring || splitFit.target}
        <span class="message" data-test="meter-right" data-part="target">
          <span class={message.first}>{message.parts[0]}</span>
        </span>
      {/if}
    {:else if measuring || splitFit.target}
      <div class="item target" data-test="meter-right" data-part="target">
        {#if !slim}<span class="label">{t("meter.playTarget")}</span>{/if}
        <span class="target-value" class:accent={right?.kind === "audition"} data-test="meter-target">
          {#if right?.kind === "audition"}
            {t("meter.auditionTarget", { instrument: right.name })}
          {:else if right?.kind === "target"}
            {#each t("track.label", { number: TRACK, name: right.track.name }).split(new RegExp(`(${TRACK})`)) as piece, i (i)}{#if piece === TRACK}<span
                  class="mono">{right.track.number}</span
                >{:else}{piece}{/if}{/each}
          {:else}
            {t("meter.playTargetNone")}
          {/if}
        </span>
      </div>
    {/if}
  </div>
  <div class="half mic" data-part="right" data-test="meter-mic-half">
    <div class="item mic-who">
      <span class="label" data-test="mic-input">{t(`vocal.input.${vocal!.input}`)}</span>
      <span class="strong" data-test="mic-track"
        >{#each t("track.label", { number: TRACK, name: vocal!.track.name }).split(new RegExp(`(${TRACK})`)) as piece, i (i)}{#if piece === TRACK}<span
              class="mono">{vocal!.track.number}</span
            >{:else}{piece}{/if}{/each}</span
      >
    </div>
    <div class="mic-meter">
      {#if measuring}
        <div class="mic-bar"></div>
      {:else}
        <div class="mic-bar" data-test="mic-bar" use:glSlot={{ kind: "meter", source: { mic: vocal!.track.id }, gap: 0, mic: { peakDb: micDb } }}></div>
      {/if}
      {#if !slim}
        <div class="mic-scale">
          <span>{t("meter.micScaleLow")}</span><span>{t("meter.micScaleMid")}</span><span>{t("meter.micScaleTop")}</span>
        </div>
      {/if}
    </div>
    {#if measuring}
      <span class="value mic-db">{t("common.db", { value: t("meter.micScaleLow") })}</span>
    {:else}
      <!-- 字が変わるコマは canvas も出す（cover の stamp）。箱の幅は「−60 dB」の字で決め、いまの値は流れの外の箱。 -->
      <span class="value mic-db" use:glSlot={{ kind: "cover", stamp: (micClipped ? 1000 : 0) + (Number.isFinite(micDb) ? micDb : -999) }}>
        <span class="hidden-width">{t("common.db", { value: t("meter.micScaleLow") })}</span>
        <span class="mic-db-in" class:rec={micClipped} data-test="mic-db" use:term={"dB"}>{micText}</span>
      </span>
    {/if}
    <div class="spacer"></div>
    {#if measuring || splitFit.monitor}
      <span class="label" data-part="monitor" data-test="mic-monitor">{vocal!.monitor ? t("meter.monitorOn") : t("meter.monitorOff")}</span>
    {/if}
  </div>
{/snippet}

{#if app}
  <div
    class="meter"
    class:slim
    class:split
    class:pack={pack !== null}
    class:empty={keyboard === null}
    style:--white-keys={WHITE_KEYS}
    bind:this={row}
    use:glRegion={"inputMeter"}
  >
    {#if pack}
      <div class="pack-left">
        {@render who(false, false)}
        {#if keyboard}{@render heldItem()}{/if}
      </div>
      <PackBand band={pack} {slim} />
    {:else if split}
      {@render splitBody(false)}
      <div class="meter ruler split" class:slim aria-hidden="true" bind:this={ruler}>
        {@render splitBody(true)}
      </div>
    {:else}
      {@render body(false)}
      <div class="meter ruler" class:slim aria-hidden="true" bind:this={ruler}>
        {@render body(true)}
      </div>
    {/if}
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
  /* 音の素材の案内（モック F8-PackAsk）: 左に入力と、いま押している音。右は PackBand。 */
  .meter.pack {
    gap: 0;
    padding: 0;
  }
  .pack-left {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 0 var(--space-3);
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
  .velocity-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  /* 読み値と棒（モック F-Connect3 の幅 96）。「固定」はその右。 */
  .velocity-main {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .velocity-main.wide {
    width: var(--mock-input-meter-velocity);
  }
  /* 「固定」の切りかえ（M ・ S ・ ● と同じ形と高さ。オンは accent。仕様書 2 章）。 */
  .fixed {
    box-sizing: border-box;
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: var(--part-toggle-size);
    min-width: var(--part-toggle-size);
    padding: 0 var(--space-1);
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-gray-8);
    font: inherit;
    cursor: pointer;
  }
  .fixed:hover {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .fixed:active {
    background: var(--color-gray-2);
    border-color: var(--color-gray-8);
  }
  .fixed.on {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  .slim .velocity {
    align-items: center;
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
  .target-value.accent {
    color: var(--color-accent);
  }
  /* 「サイン波で代わり」（仕様書 3 章 85: gray.8）。 */
  .stand-in {
    color: var(--color-gray-8);
  }
  .mono {
    font-family: var(--font-mono);
  }
  .rec {
    color: var(--color-rec);
  }
  /* 左右に分けた形（モック F7-VocalTrack ・ R11-Fit）: 半分ずつ。右は gray.6 の線で分ける。鍵盤はいつも細い鍵（幅 8）。 */
  .meter.split {
    gap: 0;
    padding: 0;
  }
  .half {
    flex: 1 1 0;
    min-width: 0;
    height: 100%;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 0 var(--space-3);
    overflow: hidden;
  }
  /* 分けた形では、名前の欄は 108 まで。入りきらない名前は「…」（仕様書 3 章）。 */
  .half .who {
    flex-shrink: 0;
    max-width: var(--size-meter-name);
  }
  .half.mic {
    border-left: var(--line-hair) solid var(--color-gray-6);
  }
  .ruler .half {
    flex: none;
    height: auto;
  }
  .keys.slim-keys {
    height: var(--mock-input-meter-keys-slim-h);
  }
  .strong {
    color: var(--color-gray-10);
  }
  .mic-who {
    flex-shrink: 0;
  }
  .mic-meter {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    width: var(--part-meter-mic);
  }
  /* 横のメーターの 2 本（左と右。モノは同じ値）。 */
  .mic-bar {
    height: calc(var(--part-meter-horizontal) * 2);
  }
  /* −12 は計のまん中（38-B）。 */
  .mic-scale {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    color: var(--color-gray-8);
    font-family: var(--font-mono);
  }
  .mic-scale > :last-child {
    text-align: right;
  }
  .mic-db {
    position: relative;
    flex-shrink: 0;
    color: var(--color-gray-10);
  }
  .hidden-width {
    visibility: hidden;
  }
  .mic-db-in {
    position: absolute;
    left: 0;
    top: 0;
  }
  .mic-db-in.rec {
    color: var(--color-rec);
  }
</style>


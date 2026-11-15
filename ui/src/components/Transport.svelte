<!-- 再生まわりの帯（モック Main・F-Empty・F-Connect3・F-Connect4・F-ArrangeRec の上の帯）。 -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { clockOffsetUs, type Command } from "../bridge";
  import { platformOf } from "../keys/keys";
  import { glRegion, glSlot } from "../gl/slot";
  import { menuOpened } from "../lib/menu";
  import { term } from "../lib/tip.svelte";
  import { current, onTelemetry, send } from "../state/app.svelte";
  import { t } from "../text/ja";
  import ChoiceMenu from "./ChoiceMenu.svelte";
  import { transportParts, type TransportParts } from "./fit";
  import { PeakHold, peakText, tempoText } from "./format";
  import { metrics } from "./metrics.svelte";
  import NumberField from "./NumberField.svelte";
  import { clockText, meterText, parseMeter, parseTempo, positionText, shownTick, stepMeter, stepTempo, ticksToSeconds, type Meter } from "./time";

  const song = $derived(current.state!.song);
  const transport = $derived(current.state!.transport);
  const meter = $derived<Meter>({ num: song.meterNum, den: song.meterDen });

  const counting = $derived(transport.state === "countIn");
  const recordingNow = $derived(counting || transport.state === "recording");
  /** カウントイン中の今の拍（全体の中で 1 から）。計器がカウントインを言っていなければ 1 拍目。 */
  const countInBeat = $derived.by(() => {
    const p = current.telemetry?.position;
    const left = p?.state === "countIn" ? p.countInBeatsLeft : transport.countInBeats;
    return Math.min(transport.countInBeats, Math.max(1, transport.countInBeats - left + 1));
  });
  /** 箱は 1 小節ぶん（拍子の上の数）で、小節ごとに数えなおす。 */
  const beatsPerBar = $derived(song.meterNum);
  const beatInBar = $derived(((countInBeat - 1) % beatsPerBar) + 1);

  // 横の入れかた: 出ている物と、見えない写し（ruler）の引っこむ物を測り、仕様書 1 章の順に引っこめる。
  let row = $state<HTMLElement>();
  let ruler = $state<HTMLElement>();
  let parts = $state<TransportParts>({ songName: true, load: true, countIn: true, positionLabels: true });

  function measure(): void {
    if (!row || !ruler) return;
    const style = getComputedStyle(row);
    const gap = parseFloat(style.columnGap) || 0;
    const pad = (parseFloat(style.paddingLeft) || 0) + (parseFloat(style.paddingRight) || 0);
    const fixed = [...row.children].filter((el) => el !== ruler && !(el as HTMLElement).dataset.part) as HTMLElement[];
    const w = (part: string) => ruler!.querySelector<HTMLElement>(`[data-part="${part}"]`)?.offsetWidth ?? 0;
    const labels = [0, 1].reduce((sum, i) => sum + Math.max(0, w(`label-${i}`) - w(`readout-${i}`)), 0);
    const shownLabels = parts.positionLabels ? labels : 0;
    // すきま（m-spacer）は残りの幅を取るので、幅は数えず、両どなりのすきまだけ数える。
    const widths = fixed.reduce((sum, el) => sum + (el.classList.contains("m-spacer") ? 0 : el.offsetWidth), 0);
    const base = pad + widths + gap * Math.max(0, fixed.length - 1) - shownLabels;
    parts = transportParts(row.clientWidth, {
      base,
      songName: w("songName") + gap,
      load: w("load") + gap,
      countIn: w("countIn") + gap,
      positionLabels: labels,
    });
  }

  onMount(() => {
    const observer = new ResizeObserver(() => measure());
    if (row) observer.observe(row);
    if (ruler) observer.observe(ruler);
    return () => observer.disconnect();
  });

  const load = $derived(Math.round((current.telemetry?.loadPermille ?? 0) / 10));

  // 位置: 計器の位置を、画面の時計で今まで進める（rAF ごと）。カウントイン中は録りはじめる位置のまま（モック F-Connect4）。
  let offsetUs = 0;
  let frame = $state(0);
  $effect(() => {
    if (!current.connected) return;
    clockOffsetUs()
      .then((us) => (offsetUs = us))
      .catch(() => {});
  });
  const tick = $derived.by(() => {
    void frame;
    return shownTick(transport, current.telemetry, performance.now() * 1000 + offsetUs);
  });
  // 止まっているときの位置は差分で来るので、コマを回すのは動いている間だけ。
  $effect(() => {
    if (transport.state === "stopped") return;
    let raf = requestAnimationFrame(function loop() {
      frame++;
      raf = requestAnimationFrame(loop);
    });
    return () => cancelAnimationFrame(raf);
  });
  const position = $derived(positionText(tick, meter));
  const clock = $derived(clockText(ticksToSeconds(Math.max(0, tick), song.tempoBpm, meter)));

  $effect(() => {
    // 中身が変わったら測りなおす（ResizeObserver のない所でも、決まりの通りに出す）。
    void [counting, beatsPerBar, song.name, song.tempoBpm, meter, transport.countInBars, metrics.scale];
    untrack(measure);
  });

  // マスターの dB の数字: 直近 1 秒のいちばん大きい値。音が止まると計器は送られてこないので、時計でも見なおす。
  const hold = new PeakHold(1000);
  let masterPeak = $state(0);
  onMount(() => {
    const off = onTelemetry((tm) => {
      hold.push(Math.max(tm.masterPeak[0], tm.masterPeak[1]), performance.now());
      masterPeak = hold.max(performance.now());
    });
    const timer = setInterval(() => (masterPeak = hold.max(performance.now())), 250);
    return () => {
      off();
      clearInterval(timer);
    };
  });

  // ボタン → 意図。再生と停止は playStop の 1 つなので、いまの状態で送るかどうかを決める。
  const playUsable = $derived(transport.canPlay);
  function play(): void {
    if (transport.canPlay && transport.state === "stopped") send({ type: "playStop" });
  }
  function stop(): void {
    if (transport.state !== "stopped") send({ type: "playStop" });
  }
  // 録音待機のトラックがなくても押せる（Rust が「録るトラックが決まっていません」を出す）。音の出口がまだなら何もしない。
  function record(): void {
    if (transport.canPlay) send({ type: "record" });
  }

  // カウントインの一覧と、曲の名前の一覧。画面だけが持つ「ひらいている」。決めるのは core（意図を送るだけ）。
  let menu = $state<"countIn" | "song" | null>(null);
  let countInButton = $state<HTMLButtonElement>();
  let songButton = $state<HTMLButtonElement>();
  const menuButton = $derived(menu === "song" ? songButton : countInButton);
  let popup = $state<HTMLElement>();
  let anchor = $state({ left: 0, top: 0 });
  const countInRows = $derived([
    { value: 0, label: t("transport.countInNone") },
    { value: 1, label: t("transport.countInBars", { bars: 1 }) },
    { value: 2, label: t("transport.countInBars", { bars: 2 }) },
  ]);
  const countInValue = $derived(
    transport.countInBars === 0 ? t("transport.countInNone") : t("transport.countInBars", { bars: transport.countInBars }),
  );

  // 曲の名前の一覧（ヤチヨさんの決め 2026-11-15）。録音中とカウントイン中は 3 行とも使えない。
  const platform = platformOf(navigator);
  const modKey = (letter: string) => `${platform === "mac" ? t("key.cmd") : t("key.ctrl")} ${letter}`;
  const songRows = $derived.by(() => {
    const usable = !recordingNow;
    return [
      { value: { type: "newSong" } as Command, label: t("song.new"), key: modKey(t("key.newSong")), usable },
      { value: { type: "openSong" } as Command, label: t("song.open"), key: modKey(t("key.openSong")), usable },
      { value: { type: "saveSong" } as Command, label: t("song.save"), key: modKey(t("key.saveSong")), usable },
    ];
  });
  const fileText = $derived(
    song.file === "neverSaved"
      ? t("transport.songNew")
      : song.file === "saved"
        ? t("transport.songSaved", { count: song.undoCount })
        : t("transport.songChanged", { count: song.undoCount }),
  );
  function pickSong(command: Command): void {
    send(command);
    closeMenu();
  }

  function place(): void {
    if (!menuButton) return;
    const r = menuButton.getBoundingClientRect();
    anchor = { left: r.left, top: r.bottom };
  }
  // 一覧はダイアログより手前に出すので、.app の重なりの外（body）に置く。
  function toBody(node: HTMLElement): { destroy(): void } {
    document.body.append(node);
    return { destroy: () => node.remove() };
  }
  function closeMenu(): void {
    const hadFocus = popup?.contains(document.activeElement) ?? false;
    const button = menuButton;
    menu = null;
    if (hadFocus) button?.focus();
  }
  function toggle(which: "countIn" | "song"): void {
    menu = menu === which ? null : which;
  }
  function pickCountIn(bars: number): void {
    send({ type: "setCountIn", bars });
    closeMenu();
  }
  $effect(() => {
    if (!menu) return;
    place();
    window.addEventListener("resize", place);
    const close = () => closeMenu();
    const forget = menuOpened(close);
    const onDown = (ev: PointerEvent) => {
      const target = ev.target as Node | null;
      if (target && (popup?.contains(target) || menuButton?.contains(target))) return;
      close();
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => {
      forget();
      window.removeEventListener("resize", place);
      document.removeEventListener("pointerdown", onDown, true);
    };
  });
  // カウントインが始まったり、帯から引っこんだりしたら、一覧はとじる。
  $effect(() => {
    if (menu === "countIn" && (counting || !parts.countIn)) closeMenu();
    if (menu === "song" && !parts.songName) closeMenu();
  });
</script>

{#snippet songName()}
  <span class="m-name ellipsis name">{song.name}</span>
  <span class="m-lbl">{fileText}</span>
{/snippet}
{#snippet loadValue()}
  <span class="m-lbl">{t("transport.cpu")}</span>
  <span class="m-num">{t("transport.cpuValue", { percent: load })}</span>
{/snippet}
{#snippet countingIn()}
  <span>{t("transport.countIn")}</span>
  <span class="beat">{beatInBar}</span>
  <span class="m-num m-lbl">{t("transport.countInOf", { beats: beatsPerBar })}</span>
  <span class="boxes">
    {#each { length: beatsPerBar }, i (i)}<span class="box" class:done={i < beatInBar}></span>{/each}
  </span>
{/snippet}
{#snippet countInLabel()}{t("transport.countIn")}<span class="m-num value">{countInValue}</span>{/snippet}

<header class="transport" bind:this={row}>
  {#if parts.songName}
    <button
      class="m-plain song"
      class:open={menu === "song"}
      aria-haspopup="menu"
      aria-expanded={menu === "song"}
      data-part="songName"
      data-test="transport-song"
      bind:this={songButton}
      onclick={() => toggle("song")}>{@render songName()}</button
    >
  {/if}

  <div class="buttons">
    <button class="m-btn" aria-label={t("transport.toStart")} data-test="transport-to-start" onclick={() => send({ type: "goToStart" })}>
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 0h2v10H0zM10 0v10L3 5z" fill="currentColor" /></svg>
    </button>
    <button
      class="m-btn"
      class:hi={transport.state === "playing"}
      aria-disabled={!playUsable}
      aria-label={t("transport.play")}
      data-test="transport-play"
      onclick={play}
    >
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 0l8 5-8 5z" fill="currentColor" /></svg>
    </button>
    <button
      class="m-btn"
      class:hi={transport.state === "stopped"}
      aria-label={t("transport.stop")}
      data-test="transport-stop"
      onclick={stop}
    >
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 1h8v8H1z" fill="currentColor" /></svg>
    </button>
    <button
      class="m-btn record"
      class:recording={recordingNow}
      aria-disabled={!recordingNow && !transport.canRecord}
      aria-label={t("transport.record")}
      data-test="transport-record"
      onclick={record}
    >
      <span class="rec"></span>
    </button>
    <button
      class="m-btn"
      class:on={song.loopOn}
      aria-pressed={song.loopOn}
      data-test="transport-loop"
      use:term={"ループ"}
      onclick={() => send({ type: "toggleLoop" })}>{t("transport.loop")}</button
    >
  </div>

  <div class="position">
    <div class="col">
      {#if parts.positionLabels}<span class="m-lbl" use:term={"小節 . 拍 . ティック"}>{t("transport.position")}</span>{/if}
      <span class="m-readout" data-test="transport-position">{position}</span>
    </div>
    <div class="col">
      {#if parts.positionLabels}<span class="m-lbl">{t("transport.time")}</span>{/if}
      <span class="m-readout" data-test="transport-time">{clock}</span>
    </div>
  </div>

  <div class="col">
    <span class="m-lbl" use:term={"テンポ"}>{t("transport.tempo")}</span>
    <NumberField
      value={tempoText(song.tempoBpm)}
      parse={parseTempo}
      step={(v, d) => tempoText(stepTempo(v, d))}
      commit={(bpm) => send({ type: "setTempo", bpm })}
      label={t("transport.tempo")}
      chars={6}
      test="transport-tempo"
    />
  </div>
  <div class="col">
    <span class="m-lbl" use:term={"拍子"}>{t("transport.timeSignature")}</span>
    <NumberField
      value={meterText(meter)}
      parse={parseMeter}
      step={(v, d) => meterText(stepMeter(v, d))}
      commit={(m) => send({ type: "setMeter", num: m.num, den: m.den })}
      label={t("transport.timeSignature")}
      chars={5}
      test="transport-meter"
    />
  </div>

  <button
    class="m-btn"
    class:on={transport.metronome}
    aria-pressed={transport.metronome}
    data-test="transport-metronome"
    use:term={"メトロノーム"}
    onclick={() => send({ type: "toggleMetronome" })}>{t("transport.metronome")}</button
  >
  {#if parts.countIn}
    {#if counting}
      <div class="counting" data-part="countIn" data-test="transport-counting" use:term={"カウントイン"}>{@render countingIn()}</div>
    {:else}
      <button
        class="m-btn"
        class:open={menu === "countIn"}
        aria-haspopup="menu"
        aria-expanded={menu === "countIn"}
        data-part="countIn"
        data-test="transport-count-in"
        bind:this={countInButton}
        use:term={"カウントイン"}
        onclick={() => toggle("countIn")}>{@render countInLabel()}</button
      >
    {/if}
  {/if}

  <div class="m-spacer"></div>

  {#if parts.load}
    <div class="col end" data-part="load">{@render loadValue()}</div>
  {/if}

  <div class="master">
    <div class="master-head">
      <span class="m-lbl" use:term={"マスター"}>{t("transport.master")}</span>
      <span class="m-num master-db">{peakText(masterPeak)}</span>
    </div>
    <div class="meter" use:glRegion={"masterMeter"} use:glSlot={{ kind: "meter", source: "master", gap: metrics.px("space.1") }}></div>
  </div>

  <div class="ruler" aria-hidden="true" bind:this={ruler}>
    <div class="song" data-part="songName">{@render songName()}</div>
    <div class="col end" data-part="load">{@render loadValue()}</div>
    {#if counting}
      <div class="counting" data-part="countIn">{@render countingIn()}</div>
    {:else}
      <span class="m-btn" data-part="countIn">{@render countInLabel()}</span>
    {/if}
    <span class="m-lbl" data-part="label-0">{t("transport.position")}</span>
    <span class="m-readout" data-part="readout-0">{position}</span>
    <span class="m-lbl" data-part="label-1">{t("transport.time")}</span>
    <span class="m-readout" data-part="readout-1">{clock}</span>
  </div>
</header>

{#if menu}
  <div class="popup" style:left="{anchor.left}px" style:top="{anchor.top}px" use:toBody bind:this={popup}>
    {#if menu === "countIn"}
      <ChoiceMenu heading={t("transport.countIn")} rows={countInRows} selected={transport.countInBars} onpick={pickCountIn} test="count-in-menu" />
    {:else}
      <ChoiceMenu heading={null} label={song.name} rows={songRows} onpick={pickSong} test="song-menu" />
    {/if}
  </div>
{/if}

<style>
  .transport {
    position: relative;
    height: var(--size-transport);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 0 var(--space-3);
    background: var(--color-gray-3);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    white-space: nowrap;
    overflow: hidden;
  }
  /* 幅は max(mock.transport.name, 名前の下の文)。下の文は切らない。名前は幅に入れず、長ければ「…」。 */
  .song {
    width: max-content;
    min-width: var(--mock-transport-name);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .name {
    width: 0;
    min-width: 100%;
  }
  .ellipsis {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .buttons {
    display: flex;
    gap: var(--space-1);
  }

  /* いまの状態のボタン（停止中の ■、再生中の ▶）は、乗ったときと同じ明るさ（モック Main の「b hi」）。 */
  .m-btn.hi:not([aria-disabled="true"]) {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .m-btn.on,
  .m-btn.on:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  .m-btn.open {
    background: var(--color-gray-4);
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .value {
    color: var(--color-gray-10);
  }

  /* 録音ボタン（仕様書 2 章）: 止まっているときは rec の輪、録音中とカウントイン中は rec のぬり（モック F-Connect4・F-ArrangeRec）。 */
  .rec {
    width: var(--part-rec-dot);
    height: var(--part-rec-dot);
    border-radius: var(--radius-round);
    border: var(--line-mark) solid var(--color-rec);
  }
  .record[aria-disabled="true"] .rec {
    border-color: currentColor;
  }
  .record.recording,
  .record.recording:hover {
    border-color: var(--color-rec);
    background: var(--color-gray-1);
  }
  .record.recording .rec {
    border: 0;
    background: var(--color-rec);
  }

  .position {
    height: var(--mock-transport-readout);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: 0 var(--space-3);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-5);
  }
  .col {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    flex-shrink: 0;
  }
  .col.end {
    align-items: flex-end;
  }

  /* カウントイン中（モック F-Connect4）: accent の枠、拍の数字は text.l、拍の箱は済んだ所までぬる。 */
  .counting {
    height: var(--mock-transport-count-in);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    border: var(--line-hair) solid var(--color-accent);
    color: var(--color-accent);
  }
  /* text.l は 600 だけ（仕様書 1 章）。等幅の書体は 600 を読みこんでいないので、数字は和文の書体の 600。 */
  .beat {
    font-variant-numeric: tabular-nums;
    font-size: var(--text-l-size);
    line-height: var(--text-l-line-height);
    font-weight: var(--text-l-weight-600);
  }
  .boxes {
    display: flex;
    gap: var(--space-1);
  }
  .box {
    width: var(--mock-transport-count-in-beat);
    height: var(--mock-transport-count-in-beat);
    border: var(--line-hair) solid var(--color-gray-7);
  }
  .box.done {
    border-color: var(--color-accent);
    background: var(--color-accent);
  }

  .master {
    width: var(--mock-transport-master);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }
  .master-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  .master-db {
    font-size: var(--text-m-size);
    line-height: var(--text-m-line-height);
    font-weight: var(--text-m-weight-400);
  }
  .meter {
    height: calc(var(--part-meter-horizontal) * 2 + var(--space-1));
  }

  .ruler {
    position: absolute;
    left: 0;
    top: 0;
    display: flex;
    align-items: center;
    width: max-content;
    visibility: hidden;
    pointer-events: none;
  }

  /* 重なり: ダイアログ（30）より上、用語の説明（1000）より下。 */
  .popup {
    position: fixed;
    z-index: 40;
  }
</style>

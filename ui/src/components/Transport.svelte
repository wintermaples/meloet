<!-- 再生まわりの帯（モック Main・F-Empty・F-Connect3・F-Connect4・F-ArrangeRec の上の帯。▾ は F-Menus・F-PR150）。 -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { clockOffsetUs, type Command } from "../bridge";
  import { platformOf } from "../keys/keys";
  import { glRegion, glSlot, type GlSlot } from "../gl/slot";
  import { menuOpened } from "../lib/menu";
  import { term, tip } from "../lib/tip.svelte";
  import { current, onTelemetry, send } from "../state/app.svelte";
  import { t } from "../text/ja";
  import ChoiceMenu from "./ChoiceMenu.svelte";
  import { transportParts, type TransportParts } from "./fit";
  import { PeakHold, peakNumberText, peakText, tempoText } from "./format";
  import { metrics } from "./metrics.svelte";
  import NumberField from "./NumberField.svelte";
  import { readMeter, readPosition, readTempo, type ClampNote } from "./valueClamp";
  import {
    clockText,
    meterFieldOf,
    meterText,
    positionText,
    shownTick,
    stepMeter,
    stepTempo,
    type Meter,
  } from "./time";
  import { mapOf } from "../time/bars";

  const song = $derived(current.state!.song);
  const timeline = $derived(current.state!.timeline);
  const transport = $derived(current.state!.transport);
  /** テンポと拍子の地図（拍子の変更のある曲でも、位置 ・ 時間 ・ テンポ ・ 拍子は、ここから）。 */
  const map = $derived(mapOf(timeline));

  const counting = $derived(transport.state === "countIn");
  const recordingNow = $derived(counting || transport.state === "recording");
  /** カウントイン中の今の鳴る拍（全体の中で 1 から。countInBeats も鳴る拍で数える）。計器がカウントインを言っていなければ 1 つめ。 */
  const countInBeat = $derived.by(() => {
    const p = current.telemetry?.position;
    const left = p?.state === "countIn" ? p.countInBeatsLeft : transport.countInBeats;
    return Math.min(transport.countInBeats, Math.max(1, transport.countInBeats - left + 1));
  });
  /** 箱は 1 小節ぶん（録りはじめる所の拍子で、メトロノームが鳴る数。6/8 は 2、5/8 は 5。22-2）で、小節ごとに数えなおす。 */
  const clicksPerBar = $derived(map.clicksPerBarAt(transport.recording?.from ?? transport.position));
  const beatInBar = $derived(((countInBeat - 1) % clicksPerBar) + 1);

  // 横の入れかた: 出ている物と、見えない写し（ruler）の引っこむ物を測り、仕様書 1 章の順に引っこめる。
  const COVER: GlSlot = { kind: "cover" };
  let row = $state<HTMLElement>();
  let ruler = $state<HTMLElement>();
  let parts = $state<TransportParts>({ keys: true, songName: true, load: true, countIn: true, positionLabels: true });

  // ボタンの右はしのキー（仕様書 4 章、モック F-WideArrange）。停止とカウントインには書かない。
  const KEYS = [t("key.toStart"), t("key.playStop"), t("key.record"), t("key.loop"), t("key.metronome")] as const;

  function measure(): void {
    if (!row || !ruler) return;
    const style = getComputedStyle(row);
    const gap = parseFloat(style.columnGap) || 0;
    const pad = (parseFloat(style.paddingLeft) || 0) + (parseFloat(style.paddingRight) || 0);
    const fixed = [...row.children].filter(
      (el) => el !== ruler && !(el as HTMLElement).dataset.part && !(el instanceof HTMLCanvasElement),
    ) as HTMLElement[];
    const w = (part: string) => ruler!.querySelector<HTMLElement>(`[data-part="${part}"]`)?.offsetWidth ?? 0;
    // 位置のラベルを引っこめると、位置の枠とマスターもモック F-PR150 の細い形になる。
    const narrowing =
      metrics.px("mock.transport.master") -
      metrics.px("mock.transport.master-narrow") +
      2 * (metrics.px("space.3") - metrics.px("space.2")) +
      (metrics.px("space.4") - metrics.px("space.3"));
    const labels = [0, 1].reduce((sum, i) => sum + Math.max(0, w(`label-${i}`) - w(`readout-${i}`)), 0) + narrowing;
    const shownLabels = parts.positionLabels ? labels : 0;
    // キーは、ボタンの中のすきま（space.1）とともにふえる。
    const keys = KEYS.reduce((sum, _, i) => sum + (w(`key-${i}`) > 0 ? w(`key-${i}`) + metrics.px("space.1") : 0), 0);
    const shownKeys = parts.keys ? keys : 0;
    // すきま（m-spacer）は残りの幅を取るので、幅は数えず、両どなりのすきまだけ数える。
    const widths = fixed.reduce((sum, el) => sum + (el.classList.contains("m-spacer") ? 0 : el.offsetWidth), 0);
    const base = pad + widths + gap * Math.max(0, fixed.length - 1) - shownLabels - shownKeys;
    parts = transportParts(
      row.clientWidth,
      {
        base,
        keys,
        songName: w("songName") + gap,
        load: w("load") + gap,
        countIn: Math.max(w("countIn"), w("countInCounting")) + gap,
        positionLabels: labels,
      },
      // メトロノームの ▾ は、すきまなくくっつく（左のすきまを打ち消す）。
      { songName: w("songStub") + gap, countIn: w("countInStub") },
    );
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
  const position = $derived(positionText(tick, map));
  const clock = $derived(clockText(map.secondsAt(Math.max(0, tick))));
  // テンポと拍子は、再生の線がいる所の値（仕様書 3 章 Y17。だんだんの中は、その瞬間の値）。
  const tempoShown = $derived(tempoText(Math.round(map.tempoAt(Math.max(0, tick)) * 100) / 100));
  const meter = $derived.by<Meter>(() => {
    const m = map.meterAt(Math.max(0, tick));
    return { num: m.num, den: m.den };
  });
  const meterShown = $derived(meterText(meter));

  $effect(() => {
    // 中身が変わったら測りなおす（ResizeObserver のない所でも、決まりの通りに出す）。
    void [counting, clicksPerBar, song.name, fileText, tempoShown, meterShown, transport.countInBars, metrics.scale];
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

  // カウントインの一覧、曲の名前の一覧、メトロノームの一覧。画面だけが持つ「ひらいている」。決めるのは core（意図を送るだけ）。
  type Menu = "countIn" | "song" | "metronome";
  let menu = $state<Menu | null>(null);
  let countInButton = $state<HTMLButtonElement>();
  let songButton = $state<HTMLButtonElement>();
  let songStub = $state<HTMLButtonElement>();
  let metronomeStub = $state<HTMLButtonElement>();
  const menuButton = $derived(
    menu === "song" ? (parts.songName ? songButton : songStub) : menu === "metronome" ? metronomeStub : countInButton,
  );
  let popup = $state<HTMLElement>();
  let anchor = $state({ left: 0, top: 0 });
  const countInRows = $derived([
    { value: 0, label: t("transport.countInNone") },
    { value: 1, label: t("transport.countInBars", { bars: 1 }) },
    { value: 2, label: t("transport.countInBars", { bars: 2 }) },
  ]);
  // メトロノームの ▾ の一覧（仕様書 2 章「メトロノームの ▾」、モック F-Menus）。カウントインの行は、カウントインの一覧と同じ。
  type MetronomeRow = "metronome" | number;
  const metronomeRows = $derived([
    { value: "metronome" as MetronomeRow, label: t("transport.metronome"), key: t("key.metronome"), checked: transport.metronome },
    { heading: t("transport.countIn") },
    ...countInRows.map((r) => ({ ...r, checked: r.value === transport.countInBars })),
  ]);
  function pickMetronome(v: MetronomeRow): void {
    send(v === "metronome" ? { type: "toggleMetronome" } : { type: "setCountIn", bars: v });
    closeMenu();
  }
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
  // 取り消しは「取り消せる手 / 全部の手」（取り消しの履歴の帯札と同じ数）。
  const fileText = $derived.by(() => {
    const undo = { undo: song.undoCount, total: song.undoCount + song.redoCount };
    return song.file === "neverSaved"
      ? t("transport.songNewUndo", undo)
      : song.file === "saved"
        ? t("transport.songSaved", undo)
        : t("transport.songChanged", undo);
  });
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
  function toggle(which: Menu): void {
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
  // カウントインが始まったり、ボタンが引っこんだり戻ったりしたら、一覧はとじる。曲の名前の一覧は、名前と ▾ のどちらかに付く。
  $effect(() => {
    if (menu === "countIn" && (counting || !parts.countIn)) closeMenu();
    if (menu === "metronome" && parts.countIn) closeMenu();
  });

  /** 決める命令。範囲の外を丸めたときは、そのあとに状態行で言うように頼む（docs/design-notes.md B-68）。 */
  function commitClamped(command: Command, clamp: ClampNote | null): void {
    send(command);
    if (clamp) send({ type: "clampValue", ...clamp });
  }

  // 数字として読めない値のまま Enter: 状態行で言うように core へ頼む（仕様書 6 章「使えない値を打った」）。
  function rejectTempo(): void {
    send({ type: "rejectValue", field: "tempo", kept: tempoShown, range: null });
  }
  function rejectMeter(draft: string): void {
    const field = meterFieldOf(draft);
    send({ type: "rejectValue", field, kept: String(field === "meterNum" ? meter.num : meter.den), range: null });
  }
  function rejectPosition(): void {
    send({ type: "rejectValue", field: "playPosition", kept: position, range: null });
  }
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
  <span class="boxes">
    {#each { length: clicksPerBar }, i (i)}<span class="box" class:done={i < beatInBar}></span>{/each}
  </span>
{/snippet}
{#snippet key(i: number)}{#if parts.keys}<span class="m-num m-lbl key" data-test="transport-key">{KEYS[i]}</span>{/if}{/snippet}
{#snippet countInLabel()}{t("transport.countIn")}<span class="m-num value">{countInValue}</span>{/snippet}

<!-- 毎コマ変わる数字（位置、時間、処理の負荷、マスターの dB）とマスターのメーターは、masterMeter の区画の 1 枚の canvas の下に入れる（cover）。
     上下の端がそろわない小さな四角が切りあって 11 個をこえると、WebKit は窓のほとんどを描きなおす（docs/architecture.md 7 章「CPU で描く WebKit」）。 -->
<header class="transport" bind:this={row} use:glRegion={"masterMeter"}>
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
  {:else}
    <button
      class="m-btn song-stub"
      class:open={menu === "song"}
      aria-haspopup="menu"
      aria-expanded={menu === "song"}
      aria-label={t("transport.songMenu", { name: song.name })}
      data-part="songStub"
      data-test="transport-song-stub"
      bind:this={songStub}
      use:tip={t("transport.songMenuTip", { name: song.name })}
      onclick={() => toggle("song")}>▾</button
    >
  {/if}

  <div class="buttons">
    <button class="m-btn" aria-label={t("transport.toStart")} data-test="transport-to-start" onclick={() => send({ type: "goToStart" })}>
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 0h2v10H0zM10 0v10L3 5z" fill="currentColor" /></svg>
      {@render key(0)}
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
      {@render key(1)}
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
      {@render key(2)}
    </button>
    <button
      class="m-btn"
      class:on={song.loopOn}
      aria-pressed={song.loopOn}
      data-test="transport-loop"
      use:term={"ループ"}
      onclick={() => send({ type: "toggleLoop" })}>{t("transport.loop")}{@render key(3)}</button
    >
  </div>

  <div class="position" class:narrow={!parts.positionLabels}>
    <div class="col" use:glSlot={COVER}>
      {#if parts.positionLabels}<span class="m-lbl" use:term={"小節 . 拍 . ティック"}>{t("transport.position")}</span>{/if}
      <NumberField
        value={position}
        parse={(s) => readPosition(s, map)}
        commit={(r) => commitClamped({ type: "locate", tick: r.value }, r.clamp)}
        reject={rejectPosition}
        label={t("transport.position")}
        chars={10}
        test="transport-position"
      />
    </div>
    <div class="col" use:glSlot={COVER}>
      {#if parts.positionLabels}<span class="m-lbl">{t("transport.time")}</span>{/if}
      <span class="m-readout" data-test="transport-time">{clock}</span>
    </div>
  </div>

  <div class="col" use:glSlot={COVER}>
    <span class="m-lbl" use:term={"テンポ"}>{t("transport.tempo")}</span>
    <NumberField
      value={tempoShown}
      parse={readTempo}
      step={(r, d) => tempoText(stepTempo(r.value, d))}
      commit={(r) => commitClamped({ type: "setTempo", bpm: r.value }, r.clamp)}
      reject={rejectTempo}
      label={t("transport.tempo")}
      chars={6}
      test="transport-tempo"
    />
  </div>
  <div class="col" use:glSlot={COVER}>
    <span class="m-lbl" use:term={"拍子"}>{t("transport.timeSignature")}</span>
    <NumberField
      value={meterShown}
      parse={readMeter}
      step={(r, d) => meterText(stepMeter(r.value, d))}
      commit={(r) => commitClamped({ type: "setMeter", num: r.value.num, den: r.value.den }, r.clamp)}
      reject={rejectMeter}
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
    onclick={() => send({ type: "toggleMetronome" })}>{t("transport.metronome")}{@render key(4)}</button
  >
  {#if !parts.countIn}
    <button
      class="m-btn metronome-stub"
      class:on={transport.metronome}
      class:open={menu === "metronome"}
      aria-haspopup="menu"
      aria-expanded={menu === "metronome"}
      aria-label={t("transport.metronomeMenu")}
      data-part="countInStub"
      data-test="transport-metronome-stub"
      bind:this={metronomeStub}
      onclick={() => toggle("metronome")}>▾</button
    >
  {/if}
  {#if parts.countIn}
    {#if counting}
      <div class="m-btn on counting" data-part="countIn" data-test="transport-counting" use:term={"カウントイン"}>{@render countingIn()}</div>
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
    <div class="col end" data-part="load" use:glSlot={COVER}>{@render loadValue()}</div>
  {/if}

  <div class="master" class:narrow={!parts.positionLabels} use:glSlot={COVER}>
    <div class="master-head">
      <span class="m-lbl" use:term={"マスター"}>{t("transport.master")}</span>
      {#if parts.positionLabels}
        <span class="m-num master-db" data-test="transport-master-db" use:term={"dB"}>{peakText(masterPeak)}</span>
      {:else}
        <span class="m-num master-db" data-test="transport-master-db" use:tip={t("transport.masterTip", { value: peakText(masterPeak) })}
          >{peakNumberText(masterPeak)}</span
        >
      {/if}
    </div>
    <div class="meter" use:glSlot={{ kind: "meter", source: "master", gap: metrics.px("space.1") }}></div>
  </div>

  <div class="ruler" aria-hidden="true" bind:this={ruler}>
    <div class="song" data-part="songName">{@render songName()}</div>
    <span class="m-btn song-stub" data-part="songStub">▾</span>
    <div class="col end" data-part="load">{@render loadValue()}</div>
    <span class="m-btn" data-part="countIn">{@render countInLabel()}</span>
    <span class="m-btn counting" data-part="countInCounting">{@render countingIn()}</span>
    <span class="m-btn metronome-stub" data-part="countInStub">▾</span>
    <span class="m-lbl" data-part="label-0">{t("transport.position")}</span>
    <span class="m-readout" data-part="readout-0">{position}</span>
    <span class="m-lbl" data-part="label-1">{t("transport.time")}</span>
    <span class="m-readout" data-part="readout-1">{clock}</span>
    {#each KEYS as k, i (i)}<span class="m-num" data-part="key-{i}">{k}</span>{/each}
  </div>
</header>

{#if menu}
  <div class="popup" style:left="{anchor.left}px" style:top="{anchor.top}px" use:toBody bind:this={popup}>
    {#if menu === "countIn"}
      <ChoiceMenu heading={t("transport.countIn")} rows={countInRows} selected={transport.countInBars} onpick={pickCountIn} test="count-in-menu" />
    {:else if menu === "metronome"}
      <ChoiceMenu heading={null} label={t("transport.metronomeMenu")} rows={metronomeRows} onpick={pickMetronome} test="metronome-menu" />
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
  /* キーは、オンでも使えないときも gray.8 のまま（一覧のキーと同じ。モック F-Zoom）。 */
  .key {
    color: var(--color-gray-8);
  }

  /* 曲の ▾（仕様書 2 章、モック F-Menus）: 28×24。 */
  .song-stub {
    width: var(--part-songmenu-width);
    padding: 0;
  }
  /* メトロノームの ▾: 幅 20 で、メトロノームのボタンの右にすきまなく。間の線は 1 本（モックはメトロノームの右の枠を消す）。 */
  .metronome-stub {
    width: calc(var(--part-metronomemenu-width) - var(--line-hair));
    padding: 0;
    border-left: 0;
  }
  .transport > .metronome-stub {
    margin-left: calc(-1 * var(--space-3));
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
  /* 位置のラベルを引っこめたとき（モック F-PR150・F-Menus）。 */
  .position.narrow {
    height: var(--mock-transport-readout-narrow);
    gap: var(--space-3);
    padding: 0 var(--space-2);
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

  /* カウントイン中（仕様書 3 章、モック F-Connect4）: オンのボタンに、1 小節の拍の数だけの四角。打った拍は accent のぬり。 */
  .counting {
    cursor: default;
  }
  .boxes {
    display: flex;
    gap: var(--part-countin-gap);
  }
  .box {
    width: var(--part-countin-beat);
    height: var(--part-countin-beat);
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
  .master.narrow {
    width: var(--mock-transport-master-narrow);
  }
  .master-head {
    display: flex;
    justify-content: space-between;
    gap: var(--space-1);
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

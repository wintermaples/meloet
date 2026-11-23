<!-- The transport band (the top band of mock Main, F-Empty, F-Connect3, F-Connect4, F-ArrangeRec; the ▾ from F-Menus, F-PR150). -->
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
  import MetronomeWindow from "./MetronomeWindow.svelte";
  import { transportWithOverdub, type TransportParts } from "./fit";
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
  /** The tempo and time signature map (position, time, tempo and time signature come from here, even in songs with time signature changes). */
  const map = $derived(mapOf(timeline));

  const counting = $derived(transport.state === "countIn");
  const recordingNow = $derived(counting || transport.state === "recording");
  /** The click sounding now during the count-in (from 1 over the whole count-in; countInBeats also counts clicks). The first one if the telemetry does not report a count-in. */
  const countInBeat = $derived.by(() => {
    const p = current.telemetry?.position;
    const left = p?.state === "countIn" ? p.countInBeatsLeft : transport.countInBeats;
    return Math.min(transport.countInBeats, Math.max(1, transport.countInBeats - left + 1));
  });
  /** One box per click in a bar (the metronome clicks in the time signature where recording starts: 2 for 6/8, 5 for 5/8; 22-2), counted again for each bar. */
  const clicksPerBar = $derived(map.clicksPerBarAt(transport.recording?.from ?? transport.position));
  const beatInBar = $derived(((countInBeat - 1) % clicksPerBar) + 1);

  // Fitting across: measure the shown parts and the hideable parts in a hidden copy (ruler), and hide in the order of spec 1.
  const COVER: GlSlot = { kind: "cover" };
  let row = $state<HTMLElement>();
  let ruler = $state<HTMLElement>();
  let parts = $state<TransportParts & { overdub: boolean }>({ keys: true, songName: true, load: true, countIn: true, positionLabels: true, overdub: true });

  // The keys at the right end of buttons (spec 4, mock F-WideArrange). Not on stop and count-in.
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
    // When the position labels are hidden, the position field and the master also take the narrow form of mock F-PR150.
    const narrowing =
      metrics.px("mock.transport.master") -
      metrics.px("mock.transport.master-narrow") +
      2 * (metrics.px("space.3") - metrics.px("space.2")) +
      (metrics.px("space.4") - metrics.px("space.3"));
    const labels = [0, 1].reduce((sum, i) => sum + Math.max(0, w(`label-${i}`) - w(`readout-${i}`)), 0) + narrowing;
    const shownLabels = parts.positionLabels ? labels : 0;
    // A key adds its width plus the gap inside the button (space.1).
    const keys = KEYS.reduce((sum, _, i) => sum + (w(`key-${i}`) > 0 ? w(`key-${i}`) + metrics.px("space.1") : 0), 0);
    const shownKeys = parts.keys ? keys : 0;
    // The spacer (m-spacer) takes the rest of the width, so its width is not counted, only the gaps on both sides.
    const widths = fixed.reduce((sum, el) => sum + (el.classList.contains("m-spacer") ? 0 : el.offsetWidth), 0);
    const overdub = w("overdub") + metrics.px("space.1");
    const base = pad + widths + gap * Math.max(0, fixed.length - 1) - shownLabels - shownKeys - (parts.overdub ? overdub : 0);
    parts = transportWithOverdub(
      row.clientWidth,
      {
        base,
        keys,
        songName: w("songName") + gap,
        load: w("load") + gap,
        countIn: Math.max(w("countIn"), w("countInCounting")) + gap,
        positionLabels: labels,
      },
      { songName: w("songStub") + gap },
      overdub,
    );
  }

  onMount(() => {
    const observer = new ResizeObserver(() => measure());
    if (row) observer.observe(row);
    if (ruler) observer.observe(ruler);
    return () => observer.disconnect();
  });

  const load = $derived(Math.round((current.telemetry?.loadPermille ?? 0) / 10));

  // Position: move the telemetry position forward to now with the screen clock (each rAF). During the count-in it stays at the recording start (mock F-Connect4).
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
  // When stopped, the position comes in diffs, so frames run only while playing.
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
  // Tempo and time signature are the values at the playhead (spec 3 Y17; inside a gradual change, the value at that moment).
  const tempoShown = $derived(tempoText(Math.round(map.tempoAt(Math.max(0, tick)) * 100) / 100));
  const meter = $derived.by<Meter>(() => {
    const m = map.meterAt(Math.max(0, tick));
    return { num: m.num, den: m.den };
  });
  const meterShown = $derived(meterText(meter));

  $effect(() => {
    // Measure again when the content changes (so it follows the rules even where there is no ResizeObserver).
    void [counting, clicksPerBar, song.name, fileText, tempoShown, meterShown, transport.countInBars, metrics.scale];
    untrack(measure);
  });

  // The master dB number: the largest value in the last second. Telemetry stops when the sound stops, so a timer also checks again.
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

  // Buttons → intents. Play and stop are one playStop, so whether to send depends on the current state.
  const playUsable = $derived(transport.canPlay);
  function play(): void {
    if (transport.canPlay && transport.state === "stopped") send({ type: "playStop" });
  }
  function stop(): void {
    if (transport.state !== "stopped") send({ type: "playStop" });
  }
  // It can be pressed with no armed track (Rust shows 「録るトラックが決まっていません」). Does nothing if there is no output yet.
  function record(): void {
    if (transport.canPlay) send({ type: "record" });
  }

  // The count-in list, the song name list and the metronome window. Only the screen holds "open"; core decides (the screen only sends intents).
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
  // The metronome ▾ (round 12 R3): always there. Its window has the count-in too while the count-in button is hidden.
  const metronomeLabel = $derived(parts.countIn ? t("transport.metronome") : t("transport.metronomeMenu"));
  const countInValue = $derived(
    transport.countInBars === 0 ? t("transport.countInNone") : t("transport.countInBars", { bars: transport.countInBars }),
  );

  // The song name list (the owner's decision). All 3 rows are unusable while recording and during the count-in.
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
  // Undo shows "undo steps / all steps" (the same numbers as the undo history band tab).
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
  // The lists appear in front of dialogs, so they are placed outside the .app stacking (in body).
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
  // The list closes when the count-in starts or a button is hidden or comes back. The song name list is attached to either the name or the ▾.
  $effect(() => {
    if (menu === "countIn" && (counting || !parts.countIn)) closeMenu();
  });

  /** The command that sets the value. When an out-of-range value was clamped, also ask for it to be told in the status line (docs/history/design-notes.ja.md B-68). */
  function commitClamped(command: Command, clamp: ClampNote | null): void {
    send(command);
    if (clamp) send({ type: "clampValue", ...clamp });
  }

  // Enter on a value that does not read as a number: ask core to tell it in the status line (spec 6 "Not readable as a number").
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

<!-- The numbers that change every frame (position, time, load, master dB) and the master meter go under the one canvas of the masterMeter slot (cover).
     When more than 11 small rects with unaligned top and bottom edges are damaged, WebKit repaints most of the window (docs/architecture.md 7 "WebKit that paints on the CPU"). -->
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
    {#if parts.overdub}
      <!-- Stays accent while recording (7 A-27); no key for now. -->
      <button
        class="m-btn"
        class:on={transport.overdub}
        aria-pressed={transport.overdub}
        data-test="transport-overdub"
        use:term={"重ねる"}
        onclick={() => send({ type: "setOverdub", on: !transport.overdub })}>{t("transport.overdub")}</button
      >
    {/if}
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

  <div class="metro">
    <button
      class="m-btn"
      class:on={transport.metronome}
      aria-pressed={transport.metronome}
      data-test="transport-metronome"
      use:term={"メトロノーム"}
      onclick={() => send({ type: "toggleMetronome" })}>{t("transport.metronome")}{@render key(4)}</button
    >
    <button
      class="m-btn metronome-stub"
      class:on={transport.metronome}
      class:open={menu === "metronome"}
      aria-haspopup="dialog"
      aria-expanded={menu === "metronome"}
      aria-label={metronomeLabel}
      data-test="transport-metronome-stub"
      bind:this={metronomeStub}
      onclick={() => toggle("metronome")}>▾</button
    >
  </div>
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
    <span class="m-btn" data-part="overdub">{t("transport.overdub")}</span>
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
      <MetronomeWindow {transport} countIn={!parts.countIn} label={metronomeLabel} />
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
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    white-space: nowrap;
    overflow: hidden;
  }
  /* The width is max(mock.transport.name, the text under the name). The text under is not cut. The name does not set the width; if long, it gets 「…」. */
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

  /* The button for the current state (■ when stopped, ▶ when playing) is as bright as on hover ("b hi" in mock Main). */
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
  /* Keys stay gray.8 when on and when unusable (same as keys in lists; mock F-Zoom). */
  .key {
    color: var(--color-gray-8);
  }

  /* The song ▾ (spec 2, mock F-Menus): 28×24. */
  .song-stub {
    width: var(--part-songmenu-width);
    padding: 0;
  }
  /* The metronome ▾: width 20, right of the metronome button with no gap. One line between them (the mock removes the metronome's right border). */
  .metro {
    display: flex;
    flex-shrink: 0;
  }
  .metronome-stub {
    width: calc(var(--part-metronomemenu-width) - var(--line-hair));
    padding: 0;
    border-left: 0;
  }

  /* The record button (spec 2): a rec ring when stopped, filled rec while recording and during the count-in (mock F-Connect4, F-ArrangeRec). */
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
    border: var(--line-hair) solid var(--color-line-thin);
  }
  /* When the position labels are hidden (mock F-PR150, F-Menus). */
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

  /* During the count-in (spec 3, mock F-Connect4): squares on the on button, one per click in a bar. Clicks already played are filled accent. */
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

  /* Stacking: above dialogs (30), below term tips (1000). */
  .popup {
    position: fixed;
    z-index: 40;
  }
</style>

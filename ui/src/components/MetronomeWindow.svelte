<!-- The small window of the metronome ▾ (round 12 R3; the mock in docs/history/round12-spec.ja.html). Rows: the metronome on and off, 「音量」,
     「音」, 「鳴らすとき」 and 「テンポを手拍子で」; while the count-in button is hidden, the count-in too (it was in the ▾ list before).
     「音量」 is the bar of a parameter row (spec 2): it jumps to the press and moves while held. Only the sound changes while moving
     (SetClick commit: false, at most once a frame) and the release keeps it (commit: true); the setting is a gray.7 frame meanwhile, and Esc
     puts the sound back. ← → move it by 1 dB (commit: false each; commit: true a second after the key is let go). A double click on the number types it.
     The taps are the screen's own; 「この速さにする」 sends one SetTempo. Neither can be used while counting in or recording. -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { CLICK_SOUNDS, CLICK_WHENS, type ClickSound, type ClickWhen, type Command, type TransportView } from "../bridge";
  import { send } from "../state/app.svelte";
  import { t, type TextKey } from "../text";
  import { tempoText } from "./format";
  import { metrics } from "./metrics.svelte";
  import { TapTempo, clickDbAt, clickDbText, clickFrac, parseClickDb } from "./metronome";
  import { KeySteps } from "./mixer/mixDrag";
  import ValueField from "./params/ValueField.svelte";
  import { onDragCancel } from "./pianoroll/view.svelte";

  interface Props {
    transport: TransportView;
    /** Show the count-in row (the count-in button is hidden). */
    countIn: boolean;
    label: string;
  }
  let { transport, countIn, label }: Props = $props();

  const SOUND_TEXT: Record<ClickSound, TextKey> = { beep: "metronome.beep", tick: "metronome.tick", cowbell: "metronome.cowbell" };
  const WHEN_TEXT: Record<ClickWhen, TextKey> = { always: "metronome.always", recordingOnly: "metronome.recordingOnly" };
  const COUNT_IN = [0, 1, 2] as const;

  const busy = $derived(transport.state === "countIn" || transport.state === "recording");

  function setClick(volumeDb: number | null, sound: ClickSound | null, when: ClickWhen | null, commit: boolean): void {
    send({ type: "setClick", volumeDb, sound, when, commit });
  }
  const volumeCommand = (db: number, commit: boolean): Command => ({ type: "setClick", volumeDb: db, sound: null, when: null, commit });

  // 「音量」
  let bar = $state<HTMLElement>();
  let barWidth = $state(0);
  let dragging = $state(false);
  /** The value being moved, or one sent and waiting for Rust's answer. */
  let held = $state<number | null>(null);
  let from = $state(0);
  const shown = $derived(held ?? transport.clickVolumeDb);
  let previewed = false;
  let lastSent: number | null = null;
  let frame = 0;
  const keys = new KeySteps<number>(volumeCommand, send);

  $effect(() => {
    void transport.clickVolumeDb;
    untrack(() => {
      if (!dragging) held = null;
    });
  });

  function at(ev: PointerEvent): number {
    const r = bar!.getBoundingClientRect();
    return r.width <= 0 ? shown : clickDbAt((ev.clientX - r.left) / r.width);
  }

  /** Let the value be heard, at most once a frame. */
  function preview(): void {
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      if (!dragging || held === null || held === lastSent) return;
      lastSent = held;
      previewed = true;
      send(volumeCommand(held, false));
    });
  }

  function stopFrame(): void {
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
  }

  function ondown(ev: PointerEvent): void {
    if (ev.button !== 0 || !bar) return;
    ev.preventDefault();
    bar.setPointerCapture?.(ev.pointerId);
    keys.flush();
    from = transport.clickVolumeDb;
    previewed = false;
    lastSent = null;
    dragging = true;
    held = at(ev);
    preview();
  }

  function onmove(ev: PointerEvent): void {
    if (!dragging) return;
    held = at(ev);
    preview();
  }

  function onup(ev: PointerEvent): void {
    if (!dragging) return;
    stopFrame();
    dragging = false;
    const v = at(ev);
    if (v === from && !previewed) {
      held = null;
      return;
    }
    held = v;
    send(volumeCommand(v, true));
  }

  function cancel(): boolean {
    if (!dragging) return false;
    stopFrame();
    dragging = false;
    held = null;
    if (previewed) send(volumeCommand(from, false));
    return true;
  }

  onMount(() => {
    const off = onDragCancel(cancel);
    return () => {
      off();
      stopFrame();
      keys.flush();
    };
  });

  /** The window is in body, where Svelte hears its keys on document: the document's key handler (keys/attach.ts) is the next listener there. */
  function keepKey(ev: KeyboardEvent): void {
    ev.preventDefault();
    ev.stopImmediatePropagation();
  }

  function onkeydown(ev: KeyboardEvent): void {
    if (ev.key !== "ArrowLeft" && ev.key !== "ArrowRight") return;
    if (ev.ctrlKey || ev.metaKey || ev.altKey || ev.isComposing) return;
    keepKey(ev);
    if (dragging) return;
    const v = clickDbAt(clickFrac(shown + (ev.key === "ArrowRight" ? 1 : -1)));
    if (v === shown) return;
    held = v;
    keys.step(v);
  }

  let typing = $state(false);
  function startTyping(): void {
    if (dragging) return;
    keys.flush();
    typing = true;
  }
  /** Text that cannot be read keeps the box open. */
  function submit(text: string): void {
    const v = parseClickDb(text);
    if (v === undefined) return;
    typing = false;
    if (v === transport.clickVolumeDb) return;
    held = v;
    send(volumeCommand(v, true));
  }

  const fraction = $derived(clickFrac(shown));
  /** The fill and the handle move with transform (changing width or left runs a layout); ends on screen pixels. */
  const snapped = $derived(Math.round(fraction * barWidth * metrics.dpr) / metrics.dpr);

  // 「テンポを手拍子で」
  const taps = new TapTempo();
  let candidate = $state<number | null>(null);
  $effect(() => {
    if (busy)
      untrack(() => {
        taps.clear();
        candidate = null;
      });
  });
  function tap(): void {
    if (!busy) candidate = taps.tap(performance.now());
  }
  /** The time of the press, not of the release: a pointer goes down on the beat. */
  function tapDown(ev: PointerEvent): void {
    if (ev.button === 0) tap();
  }
  function tapKey(ev: KeyboardEvent): void {
    if (ev.key !== " " && ev.key !== "Enter") return;
    // Not play and stop: the key taps.
    keepKey(ev);
    if (!ev.repeat) tap();
  }
  function useTempo(): void {
    if (busy || candidate === null) return;
    send({ type: "setTempo", bpm: candidate });
    taps.clear();
    candidate = null;
  }

  // Inside the window when it would go outside (as the lists do).
  let box = $state<HTMLElement>();
  let shift = $state({ x: 0, y: 0 });
  function keepInside(): void {
    if (!box) return;
    const r = box.getBoundingClientRect();
    const left = r.left - shift.x;
    const top = r.top - shift.y;
    const x = Math.max(-left, Math.min(0, window.innerWidth - (left + r.width)));
    const y = Math.max(-top, Math.min(0, window.innerHeight - (top + r.height)));
    if (x !== shift.x || y !== shift.y) shift = { x, y };
  }
  let first = $state<HTMLButtonElement>();
  // The opener moves the window to body first (moving it loses focus).
  onMount(() => {
    queueMicrotask(() => {
      keepInside();
      first?.focus();
    });
    const onResize = () => requestAnimationFrame(keepInside);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  });
</script>

<div
  class="window"
  role="dialog"
  tabindex="-1"
  aria-label={label}
  style:transform={shift.x || shift.y ? `translate(${shift.x}px, ${shift.y}px)` : null}
  bind:this={box}
  data-test="metronome-window"
>
  <div class="row">
    <span class="m-name">{t("transport.metronome")}</span>
    <span class="m-spacer"></span>
    <button
      class="m-btn"
      class:on={transport.metronome}
      aria-pressed={transport.metronome}
      data-test="metronome-on"
      bind:this={first}
      onclick={() => send({ type: "toggleMetronome" })}>{t(transport.metronome ? "metronome.on" : "metronome.off")}</button
    >
  </div>
  <div class="row" class:moving={dragging}>
    <span class="m-lbl">{t("metronome.volume")}</span>
    <div
      class="slider"
      role="slider"
      aria-label={t("metronome.volume")}
      aria-valuemin={clickDbAt(0)}
      aria-valuemax={clickDbAt(1)}
      aria-valuenow={shown}
      aria-valuetext={clickDbText(shown)}
      tabindex="0"
      data-test="metronome-volume"
      bind:this={bar}
      bind:clientWidth={barWidth}
      onpointerdown={ondown}
      onpointermove={onmove}
      onpointerup={onup}
      onpointercancel={() => cancel()}
      {onkeydown}
      onkeyup={() => keys.release()}
      onblur={() => keys.flush()}
    >
      <div class="track"></div>
      <div class="fill" style:transform="scaleX({barWidth > 0 ? snapped / barWidth : fraction})"></div>
      {#if dragging}<div class="knob ghost" style:left="{clickFrac(from) * 100}%" data-test="metronome-volume-from"></div>{/if}
      <div class="knob" style:transform="translateX({barWidth > 0 ? snapped : 0}px)" style:left={barWidth > 0 ? null : `${fraction * 100}%`}></div>
    </div>
    {#if typing}
      <span class="value"><ValueField initial={String(Math.round(shown))} label={t("metronome.volume")} unit="dB" onsubmit={submit} oncancel={() => (typing = false)} /></span>
    {:else}
      <span class="m-num m-hi value m-typable" role="presentation" data-test="metronome-volume-value" ondblclick={startTyping}>{clickDbText(shown)}</span>
    {/if}
  </div>
  <div class="row">
    <span class="m-lbl">{t("metronome.sound")}</span>
    <span class="m-spacer"></span>
    {#each CLICK_SOUNDS as sound (sound)}
      <button
        class="m-btn"
        class:on={transport.clickSound === sound}
        aria-pressed={transport.clickSound === sound}
        data-test="metronome-sound"
        data-sound={sound}
        onclick={() => setClick(null, sound, null, true)}>{t(SOUND_TEXT[sound])}</button
      >
    {/each}
  </div>
  <div class="row">
    <span class="m-lbl">{t("metronome.when")}</span>
    <span class="m-spacer"></span>
    {#each CLICK_WHENS as when (when)}
      <button
        class="m-btn"
        class:on={transport.clickWhen === when}
        aria-pressed={transport.clickWhen === when}
        data-test="metronome-when"
        data-when={when}
        onclick={() => setClick(null, null, when, true)}>{t(WHEN_TEXT[when])}</button
      >
    {/each}
  </div>
  {#if countIn}
    <div class="row">
      <span class="m-lbl">{t("transport.countIn")}</span>
      <span class="m-spacer"></span>
      {#each COUNT_IN as bars (bars)}
        <button
          class="m-btn"
          class:on={transport.countInBars === bars}
          aria-pressed={transport.countInBars === bars}
          data-test="metronome-count-in"
          onclick={() => send({ type: "setCountIn", bars })}
          >{bars === 0 ? t("transport.countInNone") : t("transport.countInBars", { bars })}</button
        >
      {/each}
    </div>
  {/if}
  <div class="row">
    <span class="m-lbl">{t("metronome.tapRow")}</span>
    <span class="m-spacer"></span>
    <button class="m-btn" aria-disabled={busy} data-test="metronome-tap" onpointerdown={tapDown} onkeydown={tapKey}>{t("metronome.tap")}</button>
    <span class="m-num m-hi tempo" data-test="metronome-tap-tempo">{candidate === null ? t("metronome.tapNone") : tempoText(candidate)}</span>
    <button class="m-btn" aria-disabled={busy || candidate === null} data-test="metronome-use-tempo" onclick={useTempo}>{t("metronome.useTempo")}</button>
  </div>
</div>

<style>
  .window {
    min-width: var(--part-list-width);
    width: max-content;
    display: flex;
    flex-direction: column;
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-8);
    outline: none;
  }
  /* Rows of buttons, like a small toolbar: label on the left, choices on the right. */
  .row {
    height: var(--size-toolbar);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-line-row);
    white-space: nowrap;
  }
  .row:last-child {
    border-bottom: 0;
  }
  .row > .m-lbl,
  .row > .m-name {
    margin-right: var(--space-2);
  }
  .row.moving {
    background: var(--color-gray-4);
  }
  .m-btn.on,
  .m-btn.on:hover {
    border-color: var(--color-accent-ink);
    background: var(--color-accent);
    color: var(--color-accent-fg);
  }
  /* The bar of a parameter row (spec 2 "Parameter row"): line gray.7, fill gray.9, a 6×12 gray.10 handle. */
  .slider {
    flex: 1;
    min-width: var(--part-param-control);
    position: relative;
    height: var(--part-param-knob-h);
    cursor: pointer;
    touch-action: none;
  }
  .track {
    position: absolute;
    left: 0;
    right: 0;
    top: calc(var(--part-param-knob-h) / 2);
    height: var(--line-hair);
    background: var(--color-gray-7);
  }
  .fill {
    position: absolute;
    left: 0;
    top: calc((var(--part-param-knob-h) - var(--mock-inspector-slider-fill)) / 2);
    width: 100%;
    height: var(--mock-inspector-slider-fill);
    background: var(--color-accent-ink);
    transform-origin: 0 50%;
  }
  .knob {
    position: absolute;
    left: 0;
    top: 0;
    width: var(--part-param-knob-w);
    height: var(--part-param-knob-h);
    margin-left: calc(var(--part-param-knob-w) / -2);
    background: var(--color-gray-10);
  }
  .knob.ghost {
    background: none;
    border: var(--line-hair) solid var(--color-gray-7);
  }
  .value {
    width: var(--part-param-value);
    flex-shrink: 0;
    text-align: right;
  }
  .tempo {
    margin: 0 var(--space-1);
  }
</style>

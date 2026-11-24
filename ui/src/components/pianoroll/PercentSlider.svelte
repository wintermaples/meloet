<!-- A "smoothly moving value" from 0 to 100 % (spec 2 "Parameter row": a horizontal bar, line gray.7, fill gray.9, a 6×12 gray.10 knob; the fill thickness is from mock A-PianoRoll) and
     the value on the right (「75 %」). It jumps to the pressed place and moves sideways while held. While moving only the screen moves, and onchange is called once on release (only when the value changed).
     After release, it stays shown at the released place until Rust's value (value) changes (so it does not flicker).
     Reached with Tab; ← → move by 10, Shift ← → by 1 (spec 4 "In the parameters: move the value / move it finely"). onchange on each press.
     Esc while dragging goes back to before moving (onDragCancel in view.svelte.ts). -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { t } from "../../text";
  import { onDragCancel } from "./view.svelte";
  import ValueField from "../params/ValueField.svelte";
  import { readScaled } from "../valueClamp";

  interface Props {
    value: number;
    label: string;
    test: string;
    onchange: (value: number) => void;
    /** Started or stopped moving (for the row's "moving" look). */
    onmoving?: (moving: boolean) => void;
  }
  let { value, label, test, onchange, onmoving }: Props = $props();

  /** One step of ← → (finer with Shift). */
  const KEY_STEP = 10;
  const FINE_STEP = 1;

  let bar = $state<HTMLElement>();
  let dragging = $state(false);
  $effect(() => onmoving?.(dragging));
  /** The value being moved, or a released value waiting for Rust's answer. */
  let held = $state<number | null>(null);
  const shown = $derived(held ?? value);

  $effect(() => {
    void value;
    untrack(() => {
      if (!dragging) held = null;
    });
  });

  function at(ev: PointerEvent): number {
    const r = bar!.getBoundingClientRect();
    if (r.width <= 0) return value;
    return Math.round(Math.min(1, Math.max(0, (ev.clientX - r.left) / r.width)) * 100);
  }

  function ondown(ev: PointerEvent): void {
    if (ev.button !== 0 || !bar) return;
    ev.preventDefault();
    bar.setPointerCapture?.(ev.pointerId);
    dragging = true;
    held = at(ev);
  }

  function onmove(ev: PointerEvent): void {
    if (dragging) held = at(ev);
  }

  function cancel(): boolean {
    if (!dragging) return false;
    dragging = false;
    held = null;
    return true;
  }
  onMount(() => onDragCancel(cancel));

  function onkeydown(ev: KeyboardEvent): void {
    if (ev.key !== "ArrowLeft" && ev.key !== "ArrowRight") return;
    if (ev.ctrlKey || ev.metaKey || ev.altKey || ev.isComposing) return;
    // Not passed on to ← → that move notes (the document's key handler).
    ev.preventDefault();
    ev.stopPropagation();
    if (dragging) return;
    const step = (ev.shiftKey ? FINE_STEP : KEY_STEP) * (ev.key === "ArrowRight" ? 1 : -1);
    const v = Math.min(100, Math.max(0, shown + step));
    if (v === shown) return;
    held = v;
    onchange(v);
  }

  let typing = $state(false);
  /** Typed text: a whole number, "%" may follow. Out of range goes to the nearest end. Text that cannot be read keeps the field open. */
  function submit(text: string): void {
    const raw = readScaled(text.replace(/\s*[%％]\s*$/, ""), 0);
    if (raw === null) return;
    typing = false;
    const v = Math.min(100, Math.max(0, raw));
    if (v === value) return;
    held = v;
    onchange(v);
  }

  function onup(ev: PointerEvent): void {
    if (!dragging) return;
    dragging = false;
    const v = at(ev);
    if (v === value) {
      held = null;
      return;
    }
    held = v;
    onchange(v);
  }
</script>

<div
  class="slider"
  role="slider"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={shown}
  tabindex="0"
  data-test={test}
  bind:this={bar}
  onpointerdown={ondown}
  onpointermove={onmove}
  onpointerup={onup}
  onpointercancel={cancel}
  {onkeydown}
>
  <div class="track"></div>
  <div class="fill" style:width="{shown}%"></div>
  <div class="knob" style:left="{shown}%"></div>
</div>
{#if typing}
  <span class="value"><ValueField initial={String(shown)} {label} onsubmit={submit} oncancel={() => (typing = false)} /></span>
{:else}
  <span class="m-num m-hi value" class:m-typable={!dragging} role="presentation" data-test="{test}-value" ondblclick={() => (typing = !dragging)}
    >{t("inspector.percent", { value: shown })}</span
  >
{/if}

<style>
  /* The value field width is from mock A-PianoRoll (40, right-aligned). */
  .value {
    width: var(--mock-inspector-slider-value);
    flex-shrink: 0;
    text-align: right;
  }
  .slider {
    flex: 1;
    min-width: 0;
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
    height: var(--mock-inspector-slider-fill);
    background: var(--color-accent-ink);
  }
  .knob {
    position: absolute;
    top: 0;
    width: var(--part-param-knob-w);
    height: var(--part-param-knob-h);
    margin-left: calc(var(--part-param-knob-w) / -2);
    background: var(--color-gray-10);
  }
</style>

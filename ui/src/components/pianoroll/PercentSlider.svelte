<!-- 0〜100 % の「なめらかに動く値」（仕様書 2 章「パラメータの行」: 横の棒、線 gray.7、ぬり gray.9、つまみ 6×12 の gray.10。ぬりの太さはモック A-PianoRoll）と、
     右の値（「75 %」）。押した所へ飛び、押したまま横に動かす。動かしているあいだは画面だけが動き、離したときに onchange を 1 回（値が変わったときだけ）。
     離したあとは、Rust の値（value）が変わるまで離した所に見せておく（ちらつかないように）。
     Tab で届き、← → で 10 ずつ、Shift ← → で 1 ずつ（仕様書 4 章「パラメータの中: 値を動かす ・ 細かく動かす」）。押すたびに onchange。
     ドラッグ中の Esc は、動かす前に戻す（view.svelte.ts の onDragCancel）。 -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { t } from "../../text/ja";
  import { onDragCancel } from "./view.svelte";
  import ValueField from "../params/ValueField.svelte";
  import { readScaled } from "../valueClamp";

  interface Props {
    value: number;
    label: string;
    test: string;
    onchange: (value: number) => void;
    /** 動かしはじめた ・ やめた（行の「動かしている」の見た目のため）。 */
    onmoving?: (moving: boolean) => void;
  }
  let { value, label, test, onchange, onmoving }: Props = $props();

  /** ← → の 1 回ぶん（Shift で細かく）。 */
  const KEY_STEP = 10;
  const FINE_STEP = 1;

  let bar = $state<HTMLElement>();
  let dragging = $state(false);
  $effect(() => onmoving?.(dragging));
  /** 動かしている値か、離して Rust の答えを待っている値。 */
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
    // ノートを動かす ← → （document のキーの係）に渡さない。
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
  <span class="m-num m-hi value" role="presentation" data-test="{test}-value" ondblclick={() => (typing = !dragging)}
    >{t("inspector.percent", { value: shown })}</span
  >
{/if}

<style>
  /* 値の欄の幅はモック A-PianoRoll（40、右そろえ）。 */
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
    background: var(--color-gray-9);
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

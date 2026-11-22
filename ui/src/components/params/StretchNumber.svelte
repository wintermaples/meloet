<!-- 選んだクリップの欄の、整数の値（音の高さの半音 ・ セント）。上下にドラッグして変える（2 回押すと ontype: 打ちこむ枠は欄の側）。
     ドラッグ中は字だけが先に動き、離したときに 1 つ送る（ドラッグ 1 回が 1 手）。Esc はドラッグをやめて、もとの値に戻す。
     reading があれば、動かしている間、状態行の文（dragReading）と、カーソルの左上のその場の読み値を出す。 -->
<script lang="ts">
  import { onDestroy, onMount, untrack } from "svelte";
  import { metrics } from "../metrics.svelte";
  import { dragReading, onDragCancel } from "../pianoroll/view.svelte";
  import { dragSteps } from "./clipFields";

  interface Props {
    /** 曲の値。 */
    value: number;
    min: number;
    max: number;
    text: (v: number) => string;
    label: string;
    test: string;
    disabled: boolean;
    commit: (v: number) => void;
    ontype: () => void;
    reading?: (v: number) => { status: string; reading: string };
  }
  let { value, min, max, text, label, test, disabled, commit, ontype, reading }: Props = $props();

  let dragging = $state(false);
  /** 動かしている値か、送って答えを待っている値。 */
  let held = $state<number | null>(null);
  const shown = $derived(held ?? value);
  let fromY = 0;
  let node = $state<HTMLElement>();
  let pointerAt = $state<{ x: number; y: number } | null>(null);
  let readingW = $state(0);
  let readingH = $state(0);
  const shownReading = $derived.by(() => {
    if (!dragging || held === null || held === value || !pointerAt || !reading) return null;
    const off = metrics.px("part.reading.offset");
    return { text: reading(held).reading, left: Math.max(0, pointerAt.x - off - readingW), top: Math.max(0, pointerAt.y - off - readingH) };
  });

  $effect(() => {
    void value;
    untrack(() => {
      if (!dragging) held = null;
    });
  });

  function ondown(e: PointerEvent): void {
    if (e.button !== 0 || disabled) return;
    e.preventDefault();
    node?.setPointerCapture?.(e.pointerId);
    fromY = e.clientY;
    dragging = true;
    held = value;
  }
  function onmove(e: PointerEvent): void {
    if (!dragging) return;
    held = Math.min(max, Math.max(min, value + dragSteps(fromY - e.clientY, metrics.px("space.2"))));
    if (!reading) return;
    pointerAt = { x: e.clientX, y: e.clientY };
    dragReading.text = held === value ? null : reading(held).status;
  }
  function stop(): void {
    dragging = false;
    pointerAt = null;
    if (reading) dragReading.text = null;
  }
  function onup(): void {
    if (!dragging) return;
    stop();
    if (held === null || held === value) {
      held = null;
      return;
    }
    commit(held);
  }
  function cancel(): boolean {
    if (!dragging) return false;
    stop();
    held = null;
    return true;
  }
  onMount(() => onDragCancel(cancel));
  onDestroy(cancel);
</script>

<span
  bind:this={node}
  class="drag"
  role="slider"
  tabindex="-1"
  aria-label={label}
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={shown}
  data-test={test}
  onpointerdown={ondown}
  onpointermove={onmove}
  onpointerup={onup}
  onpointercancel={cancel}
  ondblclick={() => {
    if (!disabled) ontype();
  }}>{text(shown)}</span
>{#if shownReading}<span
    class="m-num reading"
    style:left="{shownReading.left}px"
    style:top="{shownReading.top}px"
    bind:clientWidth={readingW}
    bind:clientHeight={readingH}
    data-test="{test}-reading">{shownReading.text}</span
  >{/if}

<style>
  .drag {
    cursor: ns-resize;
    touch-action: none;
  }
  /* その場の読み値（仕様書 2 章。アレンジの読み値と同じ形）。欄の外にも出るよう、窓に対して置く。 */
  .reading {
    position: fixed;
    z-index: 10;
    height: var(--part-reading-height);
    padding: 0 var(--part-reading-pad-x);
    display: flex;
    align-items: center;
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-8);
    color: var(--color-gray-10);
    white-space: nowrap;
    pointer-events: none;
  }
</style>

<!-- 選んだクリップの欄の、整数の値（音の高さの半音 ・ セント）。上下にドラッグして変える（2 回押すと ontype: 打ちこむ枠は欄の側）。
     ドラッグ中は字だけが先に動き、離したときに 1 つ送る（ドラッグ 1 回が 1 手）。Esc はドラッグをやめて、もとの値に戻す。 -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { metrics } from "../metrics.svelte";
  import { onDragCancel } from "../pianoroll/view.svelte";
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
  }
  let { value, min, max, text, label, test, disabled, commit, ontype }: Props = $props();

  let dragging = $state(false);
  /** 動かしている値か、送って答えを待っている値。 */
  let held = $state<number | null>(null);
  const shown = $derived(held ?? value);
  let fromY = 0;
  let node = $state<HTMLElement>();

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
  }
  function onup(): void {
    if (!dragging) return;
    dragging = false;
    if (held === null || held === value) {
      held = null;
      return;
    }
    commit(held);
  }
  function cancel(): boolean {
    if (!dragging) return false;
    dragging = false;
    held = null;
    return true;
  }
  onMount(() => onDragCancel(cancel));
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
>

<style>
  .drag {
    cursor: ns-resize;
    touch-action: none;
  }
</style>

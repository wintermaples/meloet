<!-- 値を打ちこむ枠（仕様書 3 章 86-6、モック R9-Params）。地 gray.0、枠 gray.10、右そろえ。単位はなくても入る。
     Enter で決める、Esc でやめる。ほかを押して枠から出たときも、やめる（決めるのは Enter だけ）。 -->
<script lang="ts">
  import { onMount } from "svelte";

  interface Props {
    /** はじめに入れておく文字（いまの値）。 */
    initial: string;
    label: string;
    /** 枠のうしろにそえる単位（gray.8）。 */
    unit?: string;
    onsubmit: (text: string) => void;
    oncancel: () => void;
  }
  let { initial, label, unit = "", onsubmit, oncancel }: Props = $props();

  let input = $state<HTMLInputElement>();
  let done = false;

  onMount(() => {
    input?.focus();
    input?.select();
  });

  function onkeydown(e: KeyboardEvent): void {
    if (e.isComposing) return;
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      done = true;
      onsubmit(input?.value ?? "");
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      done = true;
      oncancel();
    } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
      // 欄の行えらびに渡さない。
      e.stopPropagation();
    }
  }
  function onblur(): void {
    if (!done) oncancel();
  }
</script>

<span class="field">
  <input
    bind:this={input}
    class="m-num"
    type="text"
    value={initial}
    aria-label={label}
    data-test="param-input"
    {onkeydown}
    {onblur}
    onpointerdown={(e) => e.stopPropagation()}
  />
  {#if unit}<span class="m-num m-lbl unit">{unit}</span>{/if}
</span>

<style>
  .field {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  input {
    flex: 1;
    min-width: 0;
    height: var(--mock-inspector-bulk-field-h);
    padding: 0 var(--space-1);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-10);
    border-radius: var(--radius-1);
    color: var(--color-gray-10);
    font: inherit;
    text-align: right;
    outline: none;
  }
  .unit {
    flex-shrink: 0;
  }
</style>

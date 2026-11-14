<!-- ボタン（仕様書 2 章「ボタン」）。primary はモックの「ここから鳴らす」「これで決まり」の明るい枠。 -->
<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    onclick,
    disabled = false,
    on = false,
    primary = false,
    test,
    children,
  }: {
    onclick?: () => void;
    disabled?: boolean;
    on?: boolean;
    primary?: boolean;
    test?: string;
    children: Snippet;
  } = $props();
</script>

<button type="button" class="b" class:on class:primary {disabled} data-test={test} {onclick}>
  {@render children()}
</button>

<style>
  .b {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-1);
    height: var(--part-button-height);
    padding: 0 var(--part-button-pad-x);
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    background: var(--color-gray-4);
    color: var(--color-gray-9);
    font: inherit;
    white-space: nowrap;
    cursor: pointer;
  }
  .b:hover:not(:disabled),
  .b.primary {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .b:active:not(:disabled) {
    background: var(--color-gray-2);
    border-color: var(--color-gray-8);
  }
  .b.on,
  .b.on:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  .b:disabled {
    border-color: var(--color-gray-5);
    color: var(--color-gray-7);
    cursor: default;
  }
  .b:focus-visible {
    outline: var(--line-hair) solid var(--color-gray-10);
    outline-offset: var(--line-hair);
  }
</style>

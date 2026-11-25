<!-- A reading that becomes an input box (spec 2) when the number is pressed. Enter decides, Esc cancels, ↑ ↓ step by 1 (10 with Shift; only fields with step).
     A value that cannot be read as a number gets the broken border and cannot be decided (spec 3). Enter on it tells reject (the status line says it; spec 6).
     parse reads numbers out of range, and commit decides them as the nearest end (docs/history/design-notes.ja.md B-68).
     A press outside the box cancels. -->
<script lang="ts" generics="T">
  import { tick } from "svelte";

  interface Props {
    /** Text of the current value (Rust's state). */
    value: string;
    parse: (s: string) => T | null;
    step?: (v: T, delta: number) => string;
    commit: (v: T) => void;
    /** Enter was pressed on an unusable value. draft is the typed text. */
    reject: (draft: string) => void;
    label: string;
    /** Number of characters in the box (monospace). */
    chars: number;
    /** The reading is at least this many characters wide (monospace), so it does not move as it grows. */
    minChars?: number;
    test: string;
    /** Shows the reading in text.s (when the input meter is slim). */
    small?: boolean;
    /** The number cannot be typed now (the core takes no value): pressing does nothing, and it does not look typable. */
    disabled?: boolean;
  }
  let { value, parse, step, commit, reject, label, chars, minChars, test, small = false, disabled = false }: Props = $props();

  let editing = $state(false);
  let draft = $state("");
  let input = $state<HTMLInputElement>();
  let shown = $state<HTMLButtonElement>();
  const broken = $derived(editing && parse(draft) === null);

  async function open(): Promise<void> {
    if (disabled) return;
    draft = value;
    editing = true;
    await tick();
    input?.focus();
    input?.select();
  }

  async function close(refocus: boolean): Promise<void> {
    if (!editing) return;
    editing = false;
    if (!refocus) return;
    await tick();
    shown?.focus();
  }

  function onkeydown(ev: KeyboardEvent): void {
    if (ev.isComposing || ev.ctrlKey || ev.metaKey || ev.altKey) return;
    if (ev.key === "Enter" && !ev.shiftKey) {
      const v = parse(draft);
      if (v !== null) {
        commit(v);
        void close(true);
      } else {
        reject(draft);
      }
    } else if (ev.key === "Escape" && !ev.shiftKey) {
      void close(true);
    } else if (step && (ev.key === "ArrowUp" || ev.key === "ArrowDown")) {
      const v = parse(draft);
      if (v !== null) draft = step(v, (ev.key === "ArrowUp" ? 1 : -1) * (ev.shiftKey ? 10 : 1));
    } else {
      return;
    }
    ev.preventDefault();
    ev.stopPropagation();
  }
</script>

{#if editing}
  <input
    class="field"
    class:broken
    type="text"
    spellcheck="false"
    autocomplete="off"
    aria-label={label}
    aria-invalid={broken}
    data-test={test}
    style:--chars={chars}
    bind:value={draft}
    bind:this={input}
    onblur={() => close(false)}
    {onkeydown}
  />
{:else}
  <button
    class="m-plain m-readout shown"
    class:m-typable={!disabled}
    class:small
    aria-disabled={disabled || undefined}
    style:min-width={minChars === undefined ? undefined : `${minChars}ch`}
    aria-label={label}
    data-test={test}
    bind:this={shown}
    onclick={open}
    >{value}</button
  >
{/if}

<style>
  .shown {
    color: var(--color-gray-10);
  }
  .shown[aria-disabled="true"] {
    cursor: default;
  }
  .shown.small {
    font-size: var(--text-s-size);
    line-height: var(--text-s-line-height);
    font-weight: var(--text-s-weight-400);
  }
  /* Hover (spec 2: border gray.8, text gray.10; the dark layer is m-typable). The border is drawn on the outside so the reading size does not change. */
  .shown:not([aria-disabled="true"]):hover {
    box-shadow: 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  /* Input box: height 24, bg gray.0, corner 1. While typing, border gray.8, the same as pressed. */
  .field {
    box-sizing: border-box;
    width: calc(var(--chars) * 1ch + 2 * var(--part-input-pad-x) + 2 * var(--line-hair));
    height: var(--part-input-height);
    padding: 0 var(--part-input-pad-x);
    margin: 0;
    border: var(--line-hair) solid var(--color-gray-8);
    border-radius: var(--radius-1);
    background: var(--color-gray-0);
    color: var(--color-gray-10);
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
    font-size: var(--text-m-size);
    line-height: var(--text-m-line-height);
    font-weight: var(--text-m-weight-400);
    outline: none;
  }
  .field.broken {
    border-color: var(--color-rec);
    color: var(--color-rec);
  }
</style>

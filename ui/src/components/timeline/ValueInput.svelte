<!-- An input box in place (a tempo point's value, a time signature flag). Opens ready to type. Enter decides, Esc or outside the box cancels, ↑ ↓ step by 1 (10 with Shift).
     The look and rules are the same as the transport's number box (NumberField.svelte). -->
<script lang="ts" generics="T">
  import { onMount } from "svelte";

  interface Props {
    value: string;
    parse: (s: string) => T | null;
    step: (v: T, delta: number) => string;
    commit: (v: T) => void;
    reject: (draft: string) => void;
    close: () => void;
    label: string;
    chars: number;
    test: string;
  }
  let { value, parse, step, commit, reject, close, label, chars, test }: Props = $props();

  let draft = $state("");
  let input = $state<HTMLInputElement>();
  const broken = $derived(parse(draft) === null);

  onMount(() => {
    draft = value;
    queueMicrotask(() => {
      input?.focus();
      input?.select();
    });
  });

  function onkeydown(ev: KeyboardEvent): void {
    if (ev.isComposing || ev.ctrlKey || ev.metaKey || ev.altKey) return;
    if (ev.key === "Enter" && !ev.shiftKey) {
      const v = parse(draft);
      if (v !== null) {
        commit(v);
        close();
      } else {
        reject(draft);
      }
    } else if (ev.key === "Escape" && !ev.shiftKey) {
      close();
    } else if (ev.key === "ArrowUp" || ev.key === "ArrowDown") {
      const v = parse(draft);
      if (v !== null) draft = step(v, (ev.key === "ArrowUp" ? 1 : -1) * (ev.shiftKey ? 10 : 1));
    } else {
      return;
    }
    ev.preventDefault();
    ev.stopPropagation();
  }
</script>

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
  onblur={close}
  onpointerdown={(e) => e.stopPropagation()}
  {onkeydown}
/>

<style>
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
    font-size: var(--text-s-size);
    line-height: var(--text-s-line-height);
    font-weight: var(--text-s-weight-400);
    outline: none;
  }
  .field.broken {
    border-color: var(--color-rec);
    color: var(--color-rec);
  }
</style>

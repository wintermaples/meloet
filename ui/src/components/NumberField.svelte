<!-- 数字を押すと「入力の枠」（仕様書 2 章）になる読み値。Enter で決める、Esc でやめる、↑ ↓ で 1 ずつ（Shift で 10）。
     使えない値は「こわれた」の枠で、決めさせない（ヤチヨさんの決め 2026-11-15）。枠の外を押したときも、やめる。 -->
<script lang="ts" generics="T">
  import { tick } from "svelte";

  interface Props {
    /** いまの値（Rust の状態）の文字。 */
    value: string;
    parse: (s: string) => T | null;
    step: (v: T, delta: number) => string;
    commit: (v: T) => void;
    label: string;
    /** 枠の中の文字数（等幅）。 */
    chars: number;
    test: string;
  }
  let { value, parse, step, commit, label, chars, test }: Props = $props();

  let editing = $state(false);
  let draft = $state("");
  let input = $state<HTMLInputElement>();
  let shown = $state<HTMLButtonElement>();
  const broken = $derived(editing && parse(draft) === null);

  async function open(): Promise<void> {
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
      }
    } else if (ev.key === "Escape" && !ev.shiftKey) {
      void close(true);
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
  <button class="m-plain m-readout shown" aria-label={label} data-test={test} bind:this={shown} onclick={open}
    >{value}</button
  >
{/if}

<style>
  .shown {
    color: var(--color-gray-10);
  }
  /* 入力の枠: 高さ 24、地 gray.0、角 1。打っている最中は「押した」と同じ枠 gray.8。 */
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

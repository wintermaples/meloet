<!-- The 「入力の枠」 of a note field (spec 2, spec 3 "Note list", 「ノートの一覧で値を打ちこむ」 in mock R3-Lanes).
     While typing, the border is gray.10; an unusable value is "broken" (border and text rec). Enter decides, Esc cancels, Tab goes to the next field (Shift Tab to the previous),
     ↑ ↓ step by 1 (10 with Shift; 12 semitones in the note field). Enter or Tab with something that cannot be read as a number calls onreject (nothing is decided).
     A number out of range is set to the nearest end and decided (docs/history/design-notes.ja.md B-68; what was rounded goes in clamped). Pressing outside the box cancels.
     The text being typed (draft) is kept by the parent (so it is not lost when the list row goes out of view and is made again). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { readField, stepField, type FieldRead, type FieldSpec } from "./noteFields";

  interface Props {
    spec: FieldSpec;
    draft: string;
    label: string;
    test: string;
    /** Right-aligned (position, length, velocity). */
    right?: boolean;
    /** Decided. tab: 0 is Enter, 1 is Tab, −1 is Shift Tab. clamped: how to say what was rounded, if it was out of range. */
    onfinish: (value: number, tab: -1 | 0 | 1, clamped: FieldRead["clamped"]) => void;
    /** refocus: true for Esc (the parent takes the focus back), false when the focus went elsewhere. */
    oncancel: (refocus: boolean) => void;
    onreject: () => void;
  }
  let { spec, draft = $bindable(), label, test, right = false, onfinish, oncancel, onreject }: Props = $props();

  let input = $state<HTMLInputElement>();
  const broken = $derived(readField(spec, draft) === null);

  onMount(() => {
    input?.focus();
    input?.select();
  });

  function onkeydown(ev: KeyboardEvent): void {
    if (ev.isComposing || ev.ctrlKey || ev.metaKey || ev.altKey) return;
    const r = readField(spec, draft);
    if (ev.key === "Enter" && !ev.shiftKey) {
      if (r === null) onreject();
      else onfinish(r.value, 0, r.clamped);
    } else if (ev.key === "Tab") {
      if (r === null) onreject();
      else onfinish(r.value, ev.shiftKey ? -1 : 1, r.clamped);
    } else if (ev.key === "Escape" && !ev.shiftKey) {
      oncancel(true);
    } else if (ev.key === "ArrowUp" || ev.key === "ArrowDown") {
      if (r !== null) draft = spec.format(stepField(spec, r.value, (ev.key === "ArrowUp" ? 1 : -1) * (ev.shiftKey ? spec.shift : 1)));
    } else {
      return;
    }
    ev.preventDefault();
    ev.stopPropagation();
  }
</script>

<input
  class="field m-num"
  class:broken
  class:right
  type="text"
  spellcheck="false"
  autocomplete="off"
  aria-label={label}
  aria-invalid={broken}
  data-test={test}
  bind:value={draft}
  bind:this={input}
  onblur={() => oncancel(false)}
  {onkeydown}
/>

<style>
  /* The input box (spec 2: background gray.0). The border while typing is gray.10; the padding 4 is from mock R3-Lanes. Height and width fill the place it is put. */
  .field {
    box-sizing: border-box;
    width: 100%;
    height: 100%;
    min-width: 0;
    padding: 0 var(--space-1);
    margin: 0;
    border: var(--line-hair) solid var(--color-gray-10);
    border-radius: var(--radius-1);
    background: var(--color-gray-0);
    color: var(--color-gray-10);
    font-size: inherit;
    line-height: inherit;
    outline: none;
  }
  .field.right {
    text-align: right;
  }
  .field.broken {
    border-color: var(--color-rec);
    color: var(--color-rec);
  }
</style>

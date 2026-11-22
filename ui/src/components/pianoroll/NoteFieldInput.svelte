<!-- ノートの欄の「入力の枠」（仕様書 2 章、3 章「ノートの一覧」、モック R3-Lanes の「ノートの一覧で値を打ちこむ」）。
     打っている最中は枠 gray.10、使えない値は「こわれた」（枠と文字 rec）。Enter で決める、Esc でやめる、Tab で次の欄（Shift Tab で前の欄）、
     ↑ ↓ で 1 ずつ（Shift で 10。音の欄は 12 半音）。数字として読めない物のまま Enter か Tab を押したら onreject（決めない）。
     範囲の外の数は、近いはしにして決める（docs/history/design-notes.ja.md B-68。clamped に丸めた所）。枠の外を押したら、やめる。
     打っている文字（draft）は親が持つ（一覧の行が見えなくなって作りなおされても消えないように）。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import { readField, stepField, type FieldRead, type FieldSpec } from "./noteFields";

  interface Props {
    spec: FieldSpec;
    draft: string;
    label: string;
    test: string;
    /** 右そろえ（位置 ・ 長さ ・ ベロシティ）。 */
    right?: boolean;
    /** 決めた。tab: 0 は Enter、1 は Tab、−1 は Shift Tab。clamped: 範囲の外を丸めたなら、その言いかた。 */
    onfinish: (value: number, tab: -1 | 0 | 1, clamped: FieldRead["clamped"]) => void;
    oncancel: () => void;
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
      oncancel();
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
  onblur={oncancel}
  {onkeydown}
/>

<style>
  /* 入力の枠（仕様書 2 章: 地 gray.0）。打っている最中の枠 gray.10、内がわ 4 はモック R3-Lanes。高さと幅は置いた所いっぱい。 */
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

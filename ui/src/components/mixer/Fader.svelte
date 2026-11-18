<!-- フェーダー（仕様書 2 章「フェーダー」「メーターとフェーダーの目盛」、モック F4-Mixer）。左に目盛の字、まん中に溝とつまみ。
     いちばん下は −∞（A-14）。押したまま上下に動かすと、画面だけが先に動き、音だけ変える意図（commit: false）を送る。離したら 1 手（faderControl）。 -->
<script lang="ts">
  import type { MixControl } from "./control.svelte";
  import { dbToFrac, SCALE_MARKS } from "./scale";

  /** ドラッグは Strip が持つ（下のフェーダーの値の字も、動かしている間の値を出すため）。 */
  let { control: c }: { control: MixControl<number | null> } = $props();

  const top = (frac: number) => `${(1 - frac) * 100}%`;
</script>

<div class="marks m-num m-lbl" aria-hidden="true">
  {#each SCALE_MARKS as mark, i (mark.label)}
    <span class:first={i === 0} class:last={i === SCALE_MARKS.length - 1} style:top={top(mark.frac)}>{mark.label}</span>
  {/each}
</div>
<div
  class="fader"
  class:dragging={c.drag !== null}
  role="presentation"
  data-test="mixer-fader"
  data-db={c.shown ?? "-inf"}
  onpointerdown={c.down}
  onpointermove={c.move}
  onpointerup={c.up}
  onpointercancel={() => c.cancel()}
>
  <div class="groove"></div>
  <div class="zero" style:top={top(dbToFrac(0))}></div>
  {#if c.drag}
    <div class="knob from" style:top={top(dbToFrac(c.drag.from))} data-test="mixer-fader-from"></div>
  {/if}
  <div class="knob" style:top={top(dbToFrac(c.shown))}><div class="line"></div></div>
</div>

<style>
  .marks {
    position: relative;
    flex-shrink: 0;
    width: calc(var(--space-6) + var(--space-1));
  }
  .marks span {
    position: absolute;
    right: 0;
    transform: translateY(-50%);
  }
  .marks span.first {
    transform: none;
  }
  .marks span.last {
    transform: translateY(-100%);
  }
  .fader {
    position: relative;
    flex: 1;
    min-width: 0;
    cursor: ns-resize;
    touch-action: none;
  }
  /* 溝は gray.7 の線、0 dB の位置に gray.6 の横線（仕様書 2 章）。 */
  .groove {
    position: absolute;
    left: 50%;
    top: 0;
    bottom: 0;
    width: var(--line-hair);
    background: var(--color-gray-7);
  }
  .zero {
    position: absolute;
    left: 0;
    right: 0;
    height: var(--line-hair);
    background: var(--color-gray-6);
  }
  /* つまみ 24×12（gray.9 の面、gray.10 の枠）。乗った＝枠 accent、動かしている＝面 gray.10。 */
  .knob {
    position: absolute;
    left: 50%;
    width: var(--part-fader-knob-w);
    height: var(--part-fader-knob-h);
    transform: translate(-50%, -50%);
    background: var(--color-gray-9);
    border: var(--line-hair) solid var(--color-gray-10);
    border-radius: var(--radius-1);
  }
  .knob .line {
    position: absolute;
    left: 0;
    right: 0;
    top: 50%;
    height: var(--line-hair);
    background: var(--color-gray-0);
  }
  .fader:hover .knob:not(.from) {
    border-color: var(--color-accent);
  }
  .fader.dragging .knob:not(.from) {
    background: var(--color-gray-10);
  }
  /* もとの位置（ドラッグ中）: gray.7 の枠。 */
  .knob.from {
    background: none;
    border-color: var(--color-gray-7);
  }
</style>

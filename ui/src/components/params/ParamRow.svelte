<!-- パラメータの行（仕様書 2 章「パラメータの行」。モック F4-Mixer ・ R6-Delay）。左から名前（104）、動かす所、値（68、右そろえ）、オートメーションに書く印（20×20）。
     動かす所は 3 種類: なめらかに動く値 ＝ 横の棒、段階 ＝ ◂ ▸ と 6×6 の四角のならび（6 段からは「3 / 12」）、オン ・ オフ ＝ 12×12 の四角の印。
     棒のドラッグ中は画面だけが先に動き、音は SetParam { commit: false }（1 コマに 1 回まで）、離したときに commit: true で 1 手。
     もとの位置は gray.7 の枠。Esc は動かす前に戻して CancelMixPreview（pianoroll/view.svelte.ts の cancelDrag）。
     行をえらんで Enter か、値を 2 回押すと、値を打ちこむ枠（仕様書 3 章 86-6。ValueField）。Enter で TypeParam。
     右の印はオートメーションに書く（AutoMark）。帯が動かしているあいだは、値は線の上の値（auto。画面が出す。docs/architecture.md 21-8）。 -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import type { Command } from "../../bridge";
  import { EFFECT_PARAMS, type EffectKind, type InsertRef } from "../../bridge/types";
  import { forgetHand, noteHand } from "../automation/handValue";
  import { term } from "../../lib/tip.svelte";
  import { send } from "../../state/app.svelte";
  import { findTerm, t } from "../../text/ja";
  import { metrics } from "../metrics.svelte";
  import { onDragCancel } from "../pianoroll/view.svelte";
  import { fractionOf, keyStep, typedText, unitText, valueAt, valueText } from "./paramRows";
  import { paramUi, stopTyping } from "./paramUi.svelte";
  import ValueField from "./ValueField.svelte";
  import AutoMark from "./AutoMark.svelte";

  interface Props {
    insert: InsertRef;
    effect: EffectKind;
    param: number;
    name: string;
    /** 曲の値。 */
    value: number;
    selected: boolean;
    onselect: () => void;
    /** 右の印。 */
    mark: { on: boolean; usable: boolean; add: () => void };
    /** 帯が動かしている値（なければ null）。 */
    auto?: number | null;
  }
  let { insert, effect: kind, param, name, value, selected, onselect, mark, auto = null }: Props = $props();

  const spec = $derived(EFFECT_PARAMS[kind][param]!);

  let bar = $state<HTMLElement>();
  let dragging = $state(false);
  /** 動かしている値か、送って Rust の答えを待っている値。 */
  let held = $state<number | null>(null);
  const shown = $derived(held ?? auto ?? value);
  let from = $state(0);
  let previewed = false;
  let lastSent: number | null = null;
  let frame = 0;

  $effect(() => {
    void value;
    untrack(() => {
      if (!dragging) held = null;
    });
  });

  function set(v: number, commit: boolean): void {
    const c: Command = { type: "setParam", insert, effect: kind, param, value: v, commit };
    if (commit) forgetHand(c);
    else noteHand(c);
    send(c);
  }

  function at(ev: PointerEvent): number {
    const r = bar!.getBoundingClientRect();
    return r.width <= 0 ? shown : valueAt(spec, (ev.clientX - r.left) / r.width);
  }

  /** 1 コマに 1 回まで、いまの値を聞かせる。 */
  function preview(): void {
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      if (!dragging || held === null || held === lastSent) return;
      lastSent = held;
      previewed = true;
      set(held, false);
    });
  }

  function stopFrame(): void {
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
  }

  function ondown(ev: PointerEvent): void {
    if (ev.button !== 0 || !bar) return;
    ev.preventDefault();
    bar.setPointerCapture?.(ev.pointerId);
    from = value;
    previewed = false;
    lastSent = null;
    dragging = true;
    held = at(ev);
    preview();
  }

  function onmove(ev: PointerEvent): void {
    if (!dragging) return;
    held = at(ev);
    preview();
  }

  function onup(ev: PointerEvent): void {
    if (!dragging) return;
    stopFrame();
    dragging = false;
    const v = at(ev);
    if (v === from && !previewed) {
      held = null;
      return;
    }
    held = v;
    set(v, true);
  }

  function cancel(): boolean {
    if (!dragging) return false;
    stopFrame();
    dragging = false;
    held = null;
    forgetHand({ type: "setParam", insert, effect: kind, param, value: 0, commit: false });
    send({ type: "cancelMixPreview" });
    return true;
  }
  onMount(() => {
    const off = onDragCancel(cancel);
    return () => {
      off();
      stopFrame();
    };
  });

  function change(v: number): void {
    if (v === shown) return;
    held = v;
    set(v, true);
  }

  // 仕様書 4 章「パラメータの中: 値を動かす ・ 細かく動かす」（← → ・ Shift ← →）。押すたびに 1 手。
  function onkeydown(ev: KeyboardEvent): void {
    if (ev.key !== "ArrowLeft" && ev.key !== "ArrowRight") return;
    if (ev.ctrlKey || ev.metaKey || ev.altKey || ev.isComposing) return;
    ev.preventDefault();
    ev.stopPropagation();
    if (dragging) return;
    change(keyStep(spec, shown, ev.key === "ArrowRight" ? 1 : -1, ev.shiftKey));
  }

  const typing = $derived(paramUi.typing === param && spec.shape === "continuous");
  function startTyping(): void {
    if (spec.shape === "continuous") paramUi.typing = param;
  }
  function submit(text: string): void {
    stopTyping();
    send({ type: "typeParam", param: { kind: "builtin", insert, effect: kind, param }, text });
  }

  const fraction = $derived(fractionOf(spec, shown));
  /** 棒とつまみは transform で動かす（幅や left を変えると、行のレイアウトが走る。docs/architecture.md 7 章「CPU で描く WebKit」）。 */
  let barWidth = $state(0);
  /** はしを、幅や left のときと同じく画面の画素に合わせる。 */
  const snapped = $derived(Math.round(fraction * barWidth * metrics.dpr) / metrics.dpr);
  const step = $derived(Math.round(shown));
  const SQUARES_UP_TO = 5;
</script>

<div
  class="row"
  class:selected
  class:moving={dragging}
  role="presentation"
  data-param={param}
  data-test="param-row"
  onpointerdown={onselect}
>
  {#if findTerm(name)}
    <span class="name" use:term={name}>{name}</span>
  {:else}
    <span class="name" title={name}>{name}</span>
  {/if}
  {#if typing}
    <ValueField initial={typedText(spec, shown)} label={name} unit={unitText(spec)} onsubmit={submit} oncancel={stopTyping} />
  {:else if spec.shape === "continuous"}
    <div
      class="slider"
      role="slider"
      aria-label={name}
      aria-valuemin={spec.min}
      aria-valuemax={spec.max}
      aria-valuenow={shown}
      tabindex="0"
      data-test="param-slider"
      bind:this={bar}
      bind:clientWidth={barWidth}
      onpointerdown={ondown}
      onpointermove={onmove}
      onpointerup={onup}
      onpointercancel={cancel}
      {onkeydown}
    >
      <div class="track"></div>
      <div class="fill" style:transform="scaleX({barWidth > 0 ? snapped / barWidth : fraction})"></div>
      {#if dragging}<div class="knob ghost" style:left="{fractionOf(spec, from) * 100}%"></div>{/if}
      <div class="knob" style:transform="translateX({snapped}px)" data-test="param-knob"></div>
    </div>
  {:else if spec.shape === "steps"}
    <div class="control" role="slider" aria-label={name} aria-valuemin={0} aria-valuemax={spec.steps.length - 1} aria-valuenow={step} aria-valuetext={valueText(kind, spec, step)} tabindex="0" {onkeydown} data-test="param-steps">
      <button class="m-plain arrow m-lbl" tabindex="-1" aria-label={name} onclick={() => change(Math.max(0, step - 1))} data-test="param-prev"
        >◂</button
      >
      <div class="squares">
        {#if spec.steps.length <= SQUARES_UP_TO}
          {#each spec.steps as id, i (id)}
            <button class="m-plain square" class:on={i === step} tabindex="-1" aria-label={valueText(kind, spec, i)} onclick={() => change(i)}
            ></button>
          {/each}
        {:else}
          <span class="m-num m-lbl">{step + 1} / {spec.steps.length}</span>
        {/if}
      </div>
      <button
        class="m-plain arrow m-lbl"
        tabindex="-1"
        aria-label={name}
        onclick={() => change(Math.min(spec.steps.length - 1, step + 1))}
        data-test="param-next">▸</button
      >
    </div>
  {:else}
    <div class="control">
      <button
        class="m-plain switch"
        class:on={shown >= 0.5}
        role="switch"
        aria-checked={shown >= 0.5}
        aria-label={name}
        onclick={() => change(shown >= 0.5 ? 0 : 1)}
        data-test="param-switch"><span></span></button
      >
    </div>
  {/if}
  {#if !typing}
    <span class="m-num m-hi value live"
      ><span class="in" role="presentation" data-test="param-value" ondblclick={startTyping}>{valueText(kind, spec, shown)}</span></span
    >
  {/if}
  <AutoMark {...mark} />
</div>

<style>
  /* モック F4-Mixer: 行は左右 8、すきま 8、下に gray.4 の線。 */
  .row {
    height: var(--part-param-height);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-2);
    border-bottom: var(--line-hair) solid var(--color-gray-4);
    white-space: nowrap;
  }
  .row:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  /* えらんだ ・ 動かしている: 地 gray.4、左に accent の line.mark。 */
  .row.selected,
  .row.moving {
    background: var(--color-gray-4);
    border-left: var(--line-mark) solid var(--color-accent);
    padding-left: calc(var(--space-2) - var(--line-mark));
  }
  .name {
    width: var(--part-param-name);
    flex-shrink: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .slider,
  .control {
    flex: 1;
    min-width: 0;
  }
  .slider {
    position: relative;
    height: var(--part-param-knob-h);
    cursor: pointer;
    touch-action: none;
  }
  .track {
    position: absolute;
    left: 0;
    right: 0;
    top: calc(var(--part-param-knob-h) / 2);
    height: var(--line-hair);
    background: var(--color-gray-7);
  }
  .fill {
    position: absolute;
    left: 0;
    top: calc((var(--part-param-knob-h) - var(--mock-inspector-slider-fill)) / 2);
    width: 100%;
    height: var(--mock-inspector-slider-fill);
    background: var(--color-gray-9);
    transform-origin: 0 50%;
  }
  .knob {
    position: absolute;
    left: 0;
    top: 0;
    width: var(--part-param-knob-w);
    height: var(--part-param-knob-h);
    margin-left: calc(var(--part-param-knob-w) / -2);
    background: var(--color-gray-10);
  }
  .knob.ghost {
    box-sizing: border-box;
    background: none;
    border: var(--line-hair) solid var(--color-gray-7);
  }
  .control {
    display: flex;
    align-items: center;
  }
  .arrow {
    width: var(--part-param-switch);
    text-align: center;
  }
  .arrow:hover {
    color: var(--color-gray-10);
  }
  .squares {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--space-1);
  }
  .square {
    box-sizing: border-box;
    width: var(--part-param-step);
    height: var(--part-param-step);
    border: var(--line-hair) solid var(--color-gray-7);
  }
  .square.on {
    border: 0;
    background: var(--color-gray-10);
  }
  /* 12×12 の四角の印。オンは枠と中の 6×6 が accent（モック R6-Delay）。 */
  .switch {
    box-sizing: border-box;
    width: var(--part-param-switch);
    height: var(--part-param-switch);
    display: flex;
    align-items: center;
    justify-content: center;
    border: var(--line-hair) solid var(--color-gray-8);
  }
  .switch span {
    width: var(--part-param-step);
    height: var(--part-param-step);
  }
  .switch.on {
    border-color: var(--color-accent);
  }
  .switch.on span {
    background: var(--color-accent);
  }
  .value {
    width: var(--part-param-value);
    flex-shrink: 0;
    text-align: right;
  }
  /* 帯が動かす値の字は、流れの外の箱に入れる: 字が変わっても、行はレイアウトしなおされない（mixer/Strip.svelte の .live と同じ）。 */
  .live {
    position: relative;
  }
  .live::before {
    content: "\200b";
  }
  .live > .in {
    position: absolute;
    inset: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
</style>

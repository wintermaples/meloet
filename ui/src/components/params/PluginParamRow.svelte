<!-- A plugin parameter row (spec 2 "Parameter row"; mocks F-ParamFew, F-ParamMany, F-ParamNoWindow). The same form as ParamRow.
     Name and value text exactly as the plugin gives them. The value is 0..1 (ParamRow.value). A row whose value is unknown:
     bar not filled, no handle, value 「—」 (user:2026-11-19). While dragging the bar, only the screen moves first, with
     SetPluginParam { commit: false } at most once a frame; release sends commit: true as one step. Esc sends CancelMixPreview.
     While dragging, the status line says 「（名前）の（パラメータ）を動かしています」 (paramUi.moving). Rows of a stopped plugin are disabled. -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import type { ParamRow, PluginSlotRef } from "../../bridge";
  import { send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { MixPreview } from "../mixer/mixDrag";
  import { onDragCancel } from "../pianoroll/view.svelte";
  import { paramUi, stopTyping } from "./paramUi.svelte";
  import { pluginValueText, stepCount, stepOf, valueOfStep } from "./pluginRows";
  import ValueField from "./ValueField.svelte";
  import AutoMark from "./AutoMark.svelte";

  interface Props {
    slot: PluginSlotRef;
    table: number;
    plugin: string;
    row: ParamRow;
    /** A number that changes each time values come (ParamTableBook rewrites the row values, so this is the sign to read again). */
    tick: number;
    selected: boolean;
    automated: boolean;
    usable: boolean;
    onselect: () => void;
    /** Whether the mark on the right can be pressed (return and master inserts make no band). Pressing it calls add. */
    markUsable: boolean;
    add: () => void;
  }
  let { slot, table, plugin, row, tick, selected, automated, usable, onselect, markUsable, add }: Props = $props();

  const now = $derived((void tick, { value: row.value, text: row.text }));
  const kind = $derived(row.kind);

  let bar = $state<HTMLElement>();
  let drag = $state<{ from: number | null; now: number; pointer: number; preview: MixPreview<number> } | null>(null);
  /** A value sent and waiting for the answer. Cleared when the value comes. */
  let held = $state<number | null>(null);
  $effect(() => {
    void now;
    untrack(() => {
      if (!drag) held = null;
    });
  });
  const value = $derived(drag ? drag.now : (held ?? now.value));
  const text = $derived(pluginValueText({ value: now.value, text: now.text, kind }));

  const make = (v: number, commit: boolean) => ({ type: "setPluginParam", slot, table, param: row.id, value: v, commit }) as const;

  function at(ev: PointerEvent): number {
    const r = bar!.getBoundingClientRect();
    return r.width <= 0 ? (value ?? 0) : Math.max(0, Math.min(1, (ev.clientX - r.left) / r.width));
  }

  function ondown(ev: PointerEvent): void {
    if (ev.button !== 0 || !bar || !usable) return;
    ev.preventDefault();
    bar.setPointerCapture?.(ev.pointerId);
    const v = at(ev);
    drag = { from: now.value, now: v, pointer: ev.pointerId, preview: new MixPreview(make, send) };
    drag.preview.move(v);
    paramUi.moving = { name: plugin, param: row.name };
  }
  function onmove(ev: PointerEvent): void {
    if (!drag || ev.pointerId !== drag.pointer) return;
    const v = at(ev);
    if (v === drag.now) return;
    drag.now = v;
    drag.preview.move(v);
  }
  function end(): void {
    drag = null;
    paramUi.moving = null;
  }
  function onup(ev: PointerEvent): void {
    if (!drag || ev.pointerId !== drag.pointer) return;
    const v = at(ev);
    drag.preview.commit(v);
    held = v;
    end();
  }
  function cancel(): boolean {
    if (!drag) return false;
    drag.preview.cancel();
    end();
    return true;
  }
  onMount(() => {
    const off = onDragCancel(cancel);
    return () => {
      off();
      if (drag) cancel();
    };
  });

  function change(v: number): void {
    if (!usable || v === value) return;
    held = v;
    send(make(v, true));
  }

  // Spec 4 "In the parameters: move the value / move it finely" (← →, Shift ← →). Each press is one step.
  function onkeydown(ev: KeyboardEvent): void {
    if (ev.key !== "ArrowLeft" && ev.key !== "ArrowRight") return;
    if (ev.ctrlKey || ev.metaKey || ev.altKey || ev.isComposing) return;
    ev.preventDefault();
    ev.stopPropagation();
    if (drag) return;
    const dir = ev.key === "ArrowRight" ? 1 : -1;
    const v = value ?? 0;
    if (kind.kind === "smooth") change(Math.max(0, Math.min(1, Math.round((v + dir / (ev.shiftKey ? 100 : 20)) * 1000) / 1000)));
    else if (kind.kind === "steps") change(valueOfStep(kind, stepOf(kind, v) + dir));
    else change(dir > 0 ? 1 : 0);
  }

  const typing = $derived(paramUi.typing === row.id && kind.kind === "smooth");
  function startTyping(): void {
    if (usable && kind.kind === "smooth") paramUi.typing = row.id;
  }
  function submit(typed: string): void {
    stopTyping();
    send({ type: "typeParam", param: { kind: "plugin", slot, table, param: row.id }, text: typed });
  }

  const steps = $derived(stepCount(kind) ?? 0);
  const step = $derived(value === null ? null : stepOf(kind, value));
  const SQUARES_UP_TO = 5;
</script>

<div
  class="row"
  class:selected
  class:moving={drag !== null}
  class:unusable={!usable}
  role="presentation"
  data-param={row.id}
  data-test="param-row"
  onpointerdown={onselect}
>
  <span class="name">{row.name}</span>
  {#if typing}
    <ValueField initial={now.text ?? ""} label={row.name} onsubmit={submit} oncancel={stopTyping} />
  {:else if kind.kind === "smooth"}
    <div
      class="slider"
      role="slider"
      aria-label={row.name}
      aria-valuemin={0}
      aria-valuemax={1}
      aria-valuenow={value ?? undefined}
      aria-valuetext={text}
      aria-disabled={!usable || undefined}
      tabindex={usable ? 0 : -1}
      data-test="param-slider"
      bind:this={bar}
      onpointerdown={ondown}
      onpointermove={onmove}
      onpointerup={onup}
      onpointercancel={cancel}
      {onkeydown}
    >
      <div class="track"></div>
      {#if value !== null}
        <div class="fill" style:width="{value * 100}%"></div>
        {#if drag && drag.from !== null}<div class="knob ghost" style:left="{drag.from * 100}%"></div>{/if}
        <div class="knob" style:left="{value * 100}%"></div>
      {/if}
    </div>
  {:else if kind.kind === "steps"}
    <div
      class="control"
      role="slider"
      aria-label={row.name}
      aria-valuemin={0}
      aria-valuemax={steps - 1}
      aria-valuenow={step ?? undefined}
      aria-valuetext={text}
      tabindex={usable ? 0 : -1}
      {onkeydown}
      data-test="param-steps"
    >
      <button class="m-plain arrow m-lbl" tabindex="-1" disabled={!usable} aria-label={row.name} onclick={() => change(valueOfStep(kind, (step ?? 0) - 1))} data-test="param-prev"
        >◂</button
      >
      <div class="squares">
        {#if steps <= SQUARES_UP_TO}
          {#each { length: steps } as _, i (i)}
            <button class="m-plain square" class:on={i === step} tabindex="-1" disabled={!usable} aria-label={String(i + 1)} onclick={() => change(valueOfStep(kind, i))}
            ></button>
          {/each}
        {:else}
          <span class="m-num m-lbl">{step === null ? t("params.unknownValue") : `${step + 1} / ${steps}`}</span>
        {/if}
      </div>
      <button class="m-plain arrow m-lbl" tabindex="-1" disabled={!usable} aria-label={row.name} onclick={() => change(valueOfStep(kind, (step ?? -1) + 1))} data-test="param-next"
        >▸</button
      >
    </div>
  {:else}
    <div class="control">
      <button
        class="m-plain switch"
        class:on={value !== null && value >= 0.5}
        role="switch"
        aria-checked={value !== null && value >= 0.5}
        aria-label={row.name}
        disabled={!usable}
        onclick={() => change(value !== null && value >= 0.5 ? 0 : 1)}
        data-test="param-switch"><span></span></button
      >
    </div>
  {/if}
  {#if !typing}
    <span class="m-num m-hi value" role="presentation" data-test="param-value" ondblclick={startTyping}>{text}</span>
  {/if}
  <AutoMark on={automated} usable={markUsable} {add} />
</div>

<style>
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
  .row.selected,
  .row.moving {
    background: var(--color-gray-4);
    border-left: var(--line-mark) solid var(--color-accent);
    padding-left: calc(var(--space-2) - var(--line-mark));
  }
  /* Disabled (the plugin has stopped; spec 2: text gray.7). */
  .row.unusable,
  .row.unusable .value {
    color: var(--color-gray-7);
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
  .row.unusable .slider {
    cursor: default;
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
    height: var(--mock-inspector-slider-fill);
    background: var(--color-gray-9);
  }
  .knob {
    position: absolute;
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
  .arrow:hover:not(:disabled) {
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
    overflow: hidden;
    text-overflow: ellipsis;
    text-align: right;
  }
</style>

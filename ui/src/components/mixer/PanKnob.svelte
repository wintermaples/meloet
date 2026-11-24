<!-- The pan row (mock F4-Mixer: 「パン」, the knob, the value). The knob is a circle of diameter 24, gray.8 border, gray.10 needle (spec 2 "Pan knob").
     Hold and move right or up for right, left or down for left (1 px is 1 at 100 % scale). While moving, the needle is accent and the start angle is a gray.7 needle.
     Keys (spec 3 (86-7)): ← → by 1, by 10 with Shift, Home for C, Enter to type. -->
<script lang="ts">
  import type { StripRef } from "../../bridge";
  import { term } from "../../lib/tip.svelte";
  import { t } from "../../text";
  import { metrics } from "../metrics.svelte";
  import { mixControl } from "./control.svelte";
  import ValueField from "../params/ValueField.svelte";
  import { clampPan, panAngle, panText, parsePan } from "./scale";

  /** auto: automation is moving it (accent border). */
  let { strip, pan, auto = false }: { strip: StripRef; pan: number; auto?: boolean } = $props();

  const c = mixControl<number>({
    value: () => pan,
    read: (start, dx, dy) => clampPan(start + (dx - dy) / (metrics.scale / 100)),
    make: (value, commit) => ({ type: "setPan", strip, pan: value, commit }),
    step: (value, key, shift) => (key === "ArrowLeft" || key === "ArrowRight" ? clampPan(value + (key === "ArrowRight" ? 1 : -1) * (shift ? 10 : 1)) : undefined),
    home: () => 0,
    parse: parsePan,
  });
</script>

<div class="row">
  <span class="m-lbl" use:term={"パン"}>{t("mixer.pan")}</span>
  <div
    class="knob"
    class:dragging={c.drag !== null}
    class:auto
    role="slider"
    tabindex="0"
    aria-label={t("mixer.pan")}
    aria-valuemin={-100}
    aria-valuemax={100}
    aria-valuenow={c.shown}
    aria-valuetext={panText(c.shown)}
    data-test="mixer-pan"
    data-pan={c.shown}
    onpointerdown={c.down}
    onpointermove={c.move}
    onpointerup={c.up}
    onpointercancel={() => c.cancel()}
    onkeydown={c.keydown}
    onkeyup={c.keyup}
    onblur={c.blur}
  >
    {#if c.drag}
      <div class="needle from" style:transform="rotate({panAngle(c.drag.from)}deg)" data-test="mixer-pan-from"></div>
    {/if}
    <div class="needle" style:transform="rotate({panAngle(c.shown)}deg)"></div>
  </div>
  {#if c.typing}
    <ValueField initial={panText(c.shown)} label={t("mixer.pan")} onsubmit={(text) => c.submit(text)} oncancel={c.stopTyping} />
  {:else}
    <span class="m-num live"><span class="in m-typable" role="presentation" data-test="mixer-pan-text" ondblclick={c.startTyping}>{panText(c.shown)}</span></span>
  {/if}
</div>

<style>
  .row {
    height: var(--size-row);
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }
  .knob {
    position: relative;
    flex-shrink: 0;
    width: var(--part-pan-size);
    height: var(--part-pan-size);
    border: var(--line-hair) solid var(--color-gray-8);
    border-radius: var(--radius-round);
    background: var(--color-gray-1);
    cursor: ns-resize;
    touch-action: none;
  }
  /* Text that may change every frame goes in a box out of the flow (like .live in Strip.svelte). */
  .live {
    position: relative;
    flex: 1;
    min-width: 0;
  }
  .live::before {
    content: "\200b";
  }
  .live > .in {
    position: absolute;
    inset: 0;
    overflow: hidden;
    white-space: nowrap;
  }
  .knob.auto {
    border-color: var(--color-accent-ink);
  }
  .knob:hover {
    border-color: var(--color-gray-10);
  }
  /* The needle goes up from the center, 3/4 of the radius (9 / 12 in mock F4-Mixer). */
  .needle {
    position: absolute;
    left: 50%;
    bottom: 50%;
    width: var(--line-hair);
    height: calc(var(--part-pan-size) * 3 / 8);
    background: var(--color-gray-10);
    transform-origin: 50% 100%;
  }
  .knob.dragging .needle:not(.from) {
    background: var(--color-accent-ink);
  }
  .needle.from {
    background: var(--color-gray-7);
  }
</style>

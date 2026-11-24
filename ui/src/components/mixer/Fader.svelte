<!-- Fader (spec 2 "Fader", "Scale marks of meters and faders", mock F4-Mixer). Scale mark text on the left, the groove and handle in the middle.
     The bottom is −∞ (A-14). Holding and moving up and down moves only the screen first and sends sound-only intents (commit: false). Release is one step (faderControl).
     Focus with Tab; ↑ ↓ by 1 dB, 0.1 dB with Shift, Home for 0.0 dB, Enter to type (spec 3 (86-7); the box is in the value place of Strip). -->
<script lang="ts">
  import type { MixControl } from "./control.svelte";
  import { t } from "../../text";
  import { metrics } from "../metrics.svelte";
  import { dbToFrac, SCALE_MARKS } from "./scale";

  /** Strip owns the drag (so the fader value text below can also show the value while moving). */
  /** auto: automation is moving it (accent handle border; spec 2 "Handle moved by automation"). */
  let { control: c, auto = false }: { control: MixControl<number | null>; auto?: boolean } = $props();

  const top = (frac: number) => `${(1 - frac) * 100}%`;

  /** The handle moves with transform (changing top would lay out the whole strip again; docs/architecture.md 7 "WebKit that paints on the CPU"). */
  let height = $state(0);
  /** Snap the handle's top edge to screen pixels, as with top (otherwise the edge blurs). */
  const at = (frac: number) => {
    const y = (1 - frac) * height - metrics.px("part.fader.knob-h") / 2;
    return `translate(-50%, ${Math.round(y * metrics.dpr) / metrics.dpr}px)`;
  };
</script>

<div class="marks m-num m-lbl" aria-hidden="true">
  {#each SCALE_MARKS as mark, i (mark.label)}
    <span class:first={i === 0} class:last={i === SCALE_MARKS.length - 1} style:top={top(mark.frac)}>{mark.label}</span>
  {/each}
</div>
<div
  class="fader"
  class:dragging={c.drag !== null}
  class:auto
  role="slider"
  tabindex="0"
  aria-label={t("mixer.db")}
  aria-valuenow={c.shown ?? undefined}
  data-test="mixer-fader"
  data-db={c.shown ?? "-inf"}
  bind:clientHeight={height}
  onpointerdown={c.down}
  onpointermove={c.move}
  onpointerup={c.up}
  onpointercancel={() => c.cancel()}
  onkeydown={c.keydown}
  onkeyup={c.keyup}
  onblur={c.blur}
>
  <div class="groove"></div>
  <div class="zero" style:top={top(dbToFrac(0))}></div>
  {#if c.drag}
    <div class="knob from" style:transform={at(dbToFrac(c.drag.from))} data-test="mixer-fader-from"></div>
  {/if}
  <div class="knob" style:transform={at(dbToFrac(c.shown))} data-test="mixer-fader-knob"><div class="line"></div></div>
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
  /* The groove is a gray.7 line, with a gray.6 horizontal line at 0 dB (spec 2). */
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
  /* Handle 24×12 (gray.9 face, gray.10 border). Hover = accent border, moving = gray.10 face. */
  .knob {
    position: absolute;
    left: 50%;
    top: 0;
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
    border-color: var(--color-accent-ink);
  }
  .fader.auto .knob:not(.from) {
    border-color: var(--color-accent-ink);
  }
  .fader.dragging .knob:not(.from) {
    background: var(--color-gray-10);
  }
  /* The start position (during a drag): gray.7 frame. */
  .knob.from {
    background: none;
    border-color: var(--color-gray-7);
  }
</style>

<!-- One send row (mock F4-Mixer: 「A」, a horizontal bar, 「−15.0」). The bar length uses the same scale as the fader (the left end is −∞).
     Holding and moving left and right moves only the screen first and sends sound-only intents. Release is one step. The start amount is a gray.7 frame.
     Keys (spec 3 (86-7)): ↑ ↓ by 1 dB, 0.1 dB with Shift, Home for the amount when the track was added (TrackView.sendDefaults), Enter to type. -->
<script lang="ts">
  import type { SendBus } from "../../bridge";
  import { t } from "../../text";
  import { metrics } from "../metrics.svelte";
  import { mixControl } from "./control.svelte";
  import ValueField from "../params/ValueField.svelte";
  import { stepDb } from "./control.svelte";
  import { dbToFrac, fracToDb, parseDb, sendText } from "./scale";

  /** auto: automation is moving it (accent bar border). */
  let { track, bus, db, home, auto = false }: { track: number; bus: SendBus; db: number | null; home: number | null; auto?: boolean } = $props();

  const c = mixControl<number | null>({
    value: () => db,
    read: (start, dx, _dy, box) => fracToDb(Math.max(0, Math.min(1, dbToFrac(start) + dx / Math.max(1, box.width)))),
    make: (value, commit) => ({ type: "setSend", track, bus, db: value, commit }),
    step: (value, key, shift) => (key === "ArrowUp" || key === "ArrowDown" ? stepDb(value, key === "ArrowUp" ? 1 : -1, shift) : undefined),
    home: () => home,
    parse: parseDb,
  });
  const pct = (v: number | null) => `${dbToFrac(v) * 100}%`;
  /** The bar grows with transform (changing the width would run a row layout; docs/architecture.md 7 "WebKit that paints on the CPU"). */
  let width = $state(0);
  /** Snap the bar's right edge to screen pixels, as with the width. */
  const scale = (v: number | null) => {
    const f = dbToFrac(v);
    return `scaleX(${width > 0 ? Math.round(f * width * metrics.dpr) / metrics.dpr / width : f})`;
  };
</script>

<div class="row">
  <span class="m-lbl bus">{t(bus === "a" ? "mixer.sendA" : "mixer.sendB")}</span>
  <div
    class="hit"
    role="slider"
    tabindex="0"
    aria-label={t(bus === "a" ? "mixer.sendA" : "mixer.sendB")}
    aria-valuenow={c.shown ?? undefined}
    aria-valuetext={sendText(c.shown)}
    data-test="mixer-send"
    data-bus={bus}
    data-db={c.shown ?? "-inf"}
    onpointerdown={c.down}
    onpointermove={c.move}
    onpointerup={c.up}
    onpointercancel={() => c.cancel()}
    onkeydown={c.keydown}
    onkeyup={c.keyup}
    onblur={c.blur}
  >
    <div class="bar" class:auto bind:clientWidth={width}>
      <div class="fill" style:transform={scale(c.shown)}></div>
      {#if c.drag}
        <div class="from" style:width={pct(c.drag.from)} data-test="mixer-send-from"></div>
      {/if}
    </div>
  </div>
  {#if c.typing}
    <span class="value"><ValueField initial={sendText(c.shown)} label={t(bus === "a" ? "mixer.sendA" : "mixer.sendB")} onsubmit={(text) => c.submit(text)} oncancel={c.stopTyping} /></span>
  {:else}
    <span class="m-num value live"><span class="in" role="presentation" data-test="mixer-send-text" ondblclick={c.startTyping}>{sendText(c.shown)}</span></span>
  {/if}
</div>

<style>
  .row {
    height: var(--size-row);
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  .bus {
    flex-shrink: 0;
    width: var(--space-2);
  }
  .hit {
    flex: 1;
    min-width: 0;
    align-self: stretch;
    display: flex;
    align-items: center;
    cursor: ew-resize;
    touch-action: none;
  }
  .bar {
    position: relative;
    flex: 1;
    height: var(--part-meter-horizontal);
    background: var(--color-gray-0);
  }
  .bar.auto {
    outline: var(--line-hair) solid var(--color-accent-ink);
  }
  .fill {
    height: 100%;
    background: var(--color-accent-ink);
    transform-origin: 0 50%;
  }
  .from {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    border: var(--line-hair) solid var(--color-gray-7);
  }
  .value {
    flex-shrink: 0;
    display: flex;
    justify-content: flex-end;
    width: calc(var(--space-6) + var(--space-4));
    text-align: right;
  }
  /* Text that may change every frame goes in a box out of the flow (like .live in Strip.svelte). */
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
  }
</style>

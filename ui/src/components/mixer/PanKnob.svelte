<!-- パンの行（モック F4-Mixer: 「パン」、つまみ、値）。つまみは直径 24 の丸、gray.8 の枠、gray.10 の針（仕様書 2 章「パンのつまみ」）。
     押したまま右か上へ動かすと右へ、左か下へ動かすと左へ（拡大率 100 % で 1 px が 1）。動かしている間は針が accent、もとの向きは gray.7 の針。
     キー（仕様書 3 章 86-7）: ← → で 1、Shift で 10、Home で C、Enter で打ちこむ。 -->
<script lang="ts">
  import type { StripRef } from "../../bridge";
  import { term } from "../../lib/tip.svelte";
  import { t } from "../../text/ja";
  import { metrics } from "../metrics.svelte";
  import { mixControl } from "./control.svelte";
  import ValueField from "../params/ValueField.svelte";
  import { clampPan, panAngle, panText, parsePan } from "./scale";

  let { strip, pan }: { strip: StripRef; pan: number } = $props();

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
    <span class="m-num" data-test="mixer-pan-text">{panText(c.shown)}</span>
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
  .knob:hover {
    border-color: var(--color-gray-10);
  }
  /* 針は中心から上へ、半径の 3/4（モック F4-Mixer の 9 / 12）。 */
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
    background: var(--color-accent);
  }
  .needle.from {
    background: var(--color-gray-7);
  }
</style>

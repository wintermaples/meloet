<!-- パンの行（モック F4-Mixer: 「パン」、つまみ、値）。つまみは直径 24 の丸、gray.8 の枠、gray.10 の針（仕様書 2 章「パンのつまみ」）。
     押したまま右か上へ動かすと右へ、左か下へ動かすと左へ（拡大率 100 % で 1 px が 1）。動かしている間は針が accent、もとの向きは gray.7 の針。 -->
<script lang="ts">
  import type { StripRef } from "../../bridge";
  import { term } from "../../lib/tip.svelte";
  import { t } from "../../text/ja";
  import { metrics } from "../metrics.svelte";
  import { mixControl } from "./control.svelte";
  import { clampPan, panAngle, panText } from "./scale";

  let { strip, pan }: { strip: StripRef; pan: number } = $props();

  const c = mixControl<number>({
    value: () => pan,
    read: (start, dx, dy) => clampPan(start + (dx - dy) / (metrics.scale / 100)),
    make: (value, commit) => ({ type: "setPan", strip, pan: value, commit }),
  });
</script>

<div class="row">
  <span class="m-lbl" use:term={"パン"}>{t("mixer.pan")}</span>
  <div
    class="knob"
    class:dragging={c.drag !== null}
    role="presentation"
    data-test="mixer-pan"
    data-pan={c.shown}
    onpointerdown={c.down}
    onpointermove={c.move}
    onpointerup={c.up}
    onpointercancel={() => c.cancel()}
  >
    {#if c.drag}
      <div class="needle from" style:transform="rotate({panAngle(c.drag.from)}deg)" data-test="mixer-pan-from"></div>
    {/if}
    <div class="needle" style:transform="rotate({panAngle(c.shown)}deg)"></div>
  </div>
  <span class="m-num" data-test="mixer-pan-text">{panText(c.shown)}</span>
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

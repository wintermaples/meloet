<!-- センドの 1 行（モック F4-Mixer: 「A」、横の棒、「−15.0」）。棒の長さはフェーダーと同じ目盛（左はしが −∞）。
     押したまま左右に動かすと、画面だけが先に動き、音だけ変える意図を送る。離したら 1 手。もとの量は gray.7 の枠。 -->
<script lang="ts">
  import type { SendBus } from "../../bridge";
  import { t } from "../../text/ja";
  import { mixControl } from "./control.svelte";
  import { dbToFrac, fracToDb, sendText } from "./scale";

  let { track, bus, db }: { track: number; bus: SendBus; db: number | null } = $props();

  const c = mixControl<number | null>({
    value: () => db,
    read: (start, dx, _dy, box) => fracToDb(Math.max(0, Math.min(1, dbToFrac(start) + dx / Math.max(1, box.width)))),
    make: (value, commit) => ({ type: "setSend", track, bus, db: value, commit }),
  });
  const pct = (v: number | null) => `${dbToFrac(v) * 100}%`;
</script>

<div class="row">
  <span class="m-lbl bus">{t(bus === "a" ? "mixer.sendA" : "mixer.sendB")}</span>
  <div
    class="hit"
    role="presentation"
    data-test="mixer-send"
    data-bus={bus}
    data-db={c.shown ?? "-inf"}
    onpointerdown={c.down}
    onpointermove={c.move}
    onpointerup={c.up}
    onpointercancel={() => c.cancel()}
  >
    <div class="bar">
      <div class="fill" style:width={pct(c.shown)}></div>
      {#if c.drag}
        <div class="from" style:width={pct(c.drag.from)} data-test="mixer-send-from"></div>
      {/if}
    </div>
  </div>
  <span class="m-num value">{sendText(c.shown)}</span>
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
  .fill {
    height: 100%;
    background: var(--color-gray-9);
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
    width: calc(var(--space-6) + var(--space-4));
    text-align: right;
  }
</style>

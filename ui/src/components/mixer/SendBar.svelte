<!-- センドの 1 行（モック F4-Mixer: 「A」、横の棒、「−15.0」）。棒の長さはフェーダーと同じ目盛（左はしが −∞）。
     押したまま左右に動かすと、画面だけが先に動き、音だけ変える意図を送る。離したら 1 手。もとの量は gray.7 の枠。
     キー（仕様書 3 章 86-7）: ↑ ↓ で 1 dB、Shift で 0.1 dB、Home でトラックを足したときの量（TrackView.sendDefaults）、Enter で打ちこむ。 -->
<script lang="ts">
  import type { SendBus } from "../../bridge";
  import { t } from "../../text/ja";
  import { metrics } from "../metrics.svelte";
  import { mixControl } from "./control.svelte";
  import ValueField from "../params/ValueField.svelte";
  import { stepDb } from "./control.svelte";
  import { dbToFrac, fracToDb, parseDb, sendText } from "./scale";

  /** auto: オートメーションが動かしている（棒の枠が accent）。 */
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
  /** 棒は transform でのばす（幅を変えると、行のレイアウトが走る。docs/architecture.md 7 章「CPU で描く WebKit」）。 */
  let width = $state(0);
  /** 棒の右のはしを、幅のときと同じく画面の画素に合わせる。 */
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
    <span class="m-num value live"><span class="in">{sendText(c.shown)}</span></span>
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
    outline: var(--line-hair) solid var(--color-accent);
  }
  .fill {
    height: 100%;
    background: var(--color-gray-9);
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
  /* 毎コマ変わるかもしれない字は、流れの外の箱に入れる（Strip.svelte の .live と同じ）。 */
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

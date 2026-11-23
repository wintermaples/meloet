<!-- 「仕上げのメーター」 (round 12 M3): right of the master strip, two strips wide, there while the mixer band is open.
     From the top: its name and 「モノで聞く」; then, level with the master's meter, 「いまの大きさ」 and its bar, 「曲ぜんたい」, 「トゥルーピーク」
     and the 24 bars of the master's sound (E7's bands, color.meter.ok); under them 「低い音 ← → 高い音」; at the bottom 「公開の目安は …」.
     The two bars are slots of the mixerMeters region. Level with the strips' meters the region's surface does not grow taller, and the
     numbers under it change inside the rectangle it shows (docs/architecture.md 7, "WebKit that paints on the CPU"). The numbers are in
     boxes out of the flow (no layout of the band when they change), and change at most 4 times a second (finish.ts). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { glSlot } from "../../gl/slot";
  import { current, onTelemetry, send } from "../../state/app.svelte";
  import { t } from "../../text";
  import { FINISH_TEXT_MS, calmly, finishTarget, finishTexts, type FinishTexts } from "./finish";

  const mono = $derived(current.state?.workspace.monoListen ?? false);
  let texts = $state<FinishTexts>(finishTexts(current.telemetry));
  onMount(() => {
    const calm = calmly(() => {
      const next = finishTexts(current.telemetry);
      if (next.now !== texts.now || next.whole !== texts.whole || next.peak !== texts.peak) texts = next;
    }, FINISH_TEXT_MS);
    const off = onTelemetry(calm.poke);
    return () => {
      off();
      calm.stop();
    };
  });

  /** The master's meter, from the top of this box (CSS px); null until measured. */
  let box = $state<HTMLElement>();
  let level = $state<{ top: number; height: number } | null>(null);
  onMount(() => {
    const meter = box?.parentElement?.querySelector<HTMLElement>('[data-strip="master"] [data-gl-slot="meter"]');
    if (!box || !meter || typeof ResizeObserver === "undefined") return;
    const measure = () => {
      const at = box!.getBoundingClientRect();
      const m = meter.getBoundingClientRect();
      const next = { top: m.top - at.top, height: m.height };
      if (level?.top !== next.top || level?.height !== next.height) level = next;
    };
    const observer = new ResizeObserver(measure);
    observer.observe(box);
    observer.observe(meter);
    return () => observer.disconnect();
  });
</script>

<div class="finish" bind:this={box} data-test="mixer-finish">
  <div class="head">
    <span class="m-name name">{t("mixer.finish")}</span>
    <button
      class="m-btn mono"
      class:on={mono}
      aria-pressed={mono}
      data-test="mixer-mono"
      onclick={() => send({ type: "setMonoListen", on: !mono })}>{t("mixer.monoListen")}</button
    >
  </div>
  <div class="level" class:placed={level !== null} style:top={level ? `${level.top}px` : null} style:height={level ? `${level.height}px` : null}>
    <div class="row">
      <span class="m-lbl">{t("mixer.finishNow")}</span>
      <span class="num live"><span class="in" data-test="mixer-finish-now">{texts.now}</span></span>
    </div>
    <div class="bar" use:glSlot={{ kind: "loudness" }}></div>
    <div class="row">
      <span class="m-lbl">{t("mixer.finishWhole")}</span>
      <span class="num live m-hi"><span class="in" data-test="mixer-finish-whole">{texts.whole}</span></span>
    </div>
    <div class="row">
      <span class="m-lbl">{t("mixer.finishTruePeak")}</span>
      <span class="num live"><span class="in" data-test="mixer-finish-peak">{texts.peak}</span></span>
    </div>
    <div class="bands" use:glSlot={{ kind: "spectrum", color: "color.meter.ok" }} data-test="mixer-finish-bands"></div>
  </div>
  <div class="caption m-lbl" class:placed={level !== null} style:top={level ? `calc(${level.top + level.height}px + var(--space-2))` : null}>{t("mixer.finishBands")}</div>
  <div class="target m-lbl" data-test="mixer-finish-target">{finishTarget()}</div>
</div>

<style>
  .finish {
    position: relative;
    flex-shrink: 0;
    width: calc(var(--size-strip) * 2);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: 0 var(--space-1) var(--space-2);
    background: var(--color-gray-3);
    border-right: var(--line-hair) solid var(--color-line-thin);
  }
  /* As the head of a strip. */
  .head {
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0 calc(-1 * var(--space-1));
    padding: 0 var(--space-1);
    border-top: var(--line-mark) solid var(--color-gray-10);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .m-btn.mono {
    height: var(--part-toggle-size);
  }
  .m-btn.mono.on,
  .m-btn.mono.on:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  .level {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    min-height: 0;
  }
  /* Level with the master's meter (measured), out of the flow. */
  .placed {
    position: absolute;
    left: var(--space-1);
    right: var(--space-1);
  }
  .row {
    flex-shrink: 0;
    display: flex;
    align-items: baseline;
    gap: var(--space-1);
  }
  .bar {
    flex-shrink: 0;
    height: var(--part-meter-horizontal);
  }
  .bands {
    flex: 1;
    min-height: 0;
  }
  .num {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  /* The number changes without laying out the box around it: its text is in a box out of the flow; the outer box keeps the
     line's height and baseline with a zero-width character (as the strips' fader values). */
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
    text-align: right;
  }
  .target {
    margin-top: auto;
    height: var(--part-toggle-size);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    overflow: hidden;
  }
</style>

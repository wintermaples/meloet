<!-- The small window of the arpeggio ▾ in the input meter (round 12 E2; the mock in docs/history/round12-spec.ja.html). Rows: the arpeggio on and off,
     「形」, 「速さ」 with 「三連符」, 「オクターブ」, 「指を離しても続ける」 and the hint. Every press sends one SetArp with the whole new setting
     (one step each); settings can be changed while it is off. Nothing can be pressed while counting in or recording. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { ARP_OCTAVES_MAX, ARP_RATES, ARP_SHAPES, type Arp, type ArpRate, type ArpShape } from "../bridge";
  import { send } from "../state/app.svelte";
  import { t, type TextKey } from "../text";

  interface Props {
    track: number;
    arp: Arp;
    busy: boolean;
    label: string;
  }
  let { track, arp, busy, label }: Props = $props();

  const SHAPE_TEXT: Record<ArpShape, TextKey> = { up: "arp.up", down: "arp.down", upDown: "arp.upDown", random: "arp.random" };
  const RATE_TEXT: Record<ArpRate, TextKey> = { quarter: "arp.quarter", eighth: "arp.eighth", sixteenth: "arp.sixteenth", thirtySecond: "arp.thirtySecond" };
  const OCTAVES = Array.from({ length: ARP_OCTAVES_MAX }, (_, i) => i + 1);

  function set(change: Partial<Arp>): void {
    if (busy) return;
    send({ type: "setArp", track, arp: { ...arp, ...change } });
  }

  // Inside the window when it would go outside (as the metronome window does).
  let box = $state<HTMLElement>();
  let shift = $state({ x: 0, y: 0 });
  function keepInside(): void {
    if (!box) return;
    const r = box.getBoundingClientRect();
    const left = r.left - shift.x;
    const top = r.top - shift.y;
    const x = Math.max(-left, Math.min(0, window.innerWidth - (left + r.width)));
    const y = Math.max(-top, Math.min(0, window.innerHeight - (top + r.height)));
    if (x !== shift.x || y !== shift.y) shift = { x, y };
  }
  let first = $state<HTMLButtonElement>();
  // The opener moves the window to body first (moving it loses focus).
  onMount(() => {
    queueMicrotask(() => {
      keepInside();
      first?.focus();
    });
    const onResize = () => requestAnimationFrame(keepInside);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  });
</script>

{#snippet choice(on: boolean, text: string, test: string, change: Partial<Arp>, value: string | null = null)}
  <button class="m-btn" class:on aria-pressed={on} aria-disabled={busy || undefined} data-test={test} data-value={value} onclick={() => set(change)}
    >{text}</button
  >
{/snippet}

<div
  class="window"
  role="dialog"
  tabindex="-1"
  aria-label={label}
  style:transform={shift.x || shift.y ? `translate(${shift.x}px, ${shift.y}px)` : null}
  bind:this={box}
  data-test="arp-window"
>
  <div class="row">
    <span class="m-name">{t("input.arp")}</span>
    <span class="m-spacer"></span>
    <button
      class="m-btn"
      class:on={arp.on}
      aria-pressed={arp.on}
      aria-disabled={busy || undefined}
      data-test="arp-window-on"
      bind:this={first}
      onclick={() => set({ on: !arp.on })}>{t(arp.on ? "arp.on" : "arp.off")}</button
    >
  </div>
  <div class="row">
    <span class="m-lbl">{t("arp.shape")}</span>
    <span class="m-spacer"></span>
    {#each ARP_SHAPES as shape (shape)}
      {@render choice(arp.shape === shape, t(SHAPE_TEXT[shape]), "arp-shape", { shape }, shape)}
    {/each}
  </div>
  <div class="row">
    <span class="m-lbl">{t("arp.rate")}</span>
    <span class="m-spacer"></span>
    {#each ARP_RATES as rate (rate)}
      {@render choice(arp.rate === rate, t(RATE_TEXT[rate]), "arp-rate", { rate }, rate)}
    {/each}
    <span class="apart"></span>
    {@render choice(arp.triplet, t("arp.triplet"), "arp-triplet", { triplet: !arp.triplet })}
  </div>
  <div class="row">
    <span class="m-lbl">{t("arp.octaves")}</span>
    <span class="m-spacer"></span>
    {#each OCTAVES as octaves (octaves)}
      {@render choice(arp.octaves === octaves, String(octaves), "arp-octaves", { octaves }, String(octaves))}
    {/each}
  </div>
  <div class="row">
    <span class="m-lbl">{t("arp.hold")}</span>
    <span class="m-spacer"></span>
    {@render choice(arp.hold, t(arp.hold ? "arp.on" : "arp.off"), "arp-hold", { hold: !arp.hold })}
  </div>
  <div class="row">
    <span class="m-lbl" data-test="arp-hint">{t("arp.hint")}</span>
  </div>
</div>

<style>
  .window {
    min-width: var(--part-list-width);
    width: max-content;
    display: flex;
    flex-direction: column;
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-8);
    outline: none;
  }
  /* Rows of buttons, like a small toolbar: label on the left, choices on the right (the metronome window's rows). */
  .row {
    height: var(--size-toolbar);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-line-row);
    white-space: nowrap;
  }
  .row:last-child {
    border-bottom: 0;
  }
  .row > .m-lbl,
  .row > .m-name {
    margin-right: var(--space-2);
  }
  .apart {
    width: var(--space-2);
    flex-shrink: 0;
  }
  .m-btn.on,
  .m-btn.on:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
</style>

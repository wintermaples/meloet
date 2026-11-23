<!-- Hint in the input meter band (sound pack; spec 2 "Hint band (sound pack)", spec 3 43-B, 44-A; mock F8-PackAsk, F8-PackGet, F8-PackFail, R12-Fit).
     On the right, from a line.mark line in accent: the hint text (2 lines), a 160 bar, buttons. Asking / fetching / failed. Only the buttons send intents.
     When it does not fit, the small text of the asking band becomes the short text of R12-Fit; if it still does not fit, the end of the text is cut with 「…」 (the bar and buttons stay).
     When the input meter is slim (a lower band is open), the 2 lines go on 1 line. -->
<script lang="ts">
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import Button from "../Button.svelte";
  import { againLabel, askLines, failedLines, fetchingLines, percentOf, type PackBand } from "./pack";

  let { band, slim }: { band: PackBand; slim: boolean } = $props();

  const sounds = $derived(current.state!.sounds);
  let short = $state(false);
  const lines = $derived.by((): [string, string] => {
    if (band.kind === "ask") return askLines(sounds, short);
    if (band.kind === "fetching") return fetchingLines(band.fetch);
    return failedLines(band.fetch.failure, band.fetch.doneBytes, band.fetch.totalBytes);
  });
  const percent = $derived(band.kind === "fetching" ? percentOf(band.fetch.doneBytes, band.fetch.totalBytes) : 0);

  // Asking band: whether the width of the long text (a hidden copy) fits in the band.
  let box = $state<HTMLElement>();
  let ruler = $state<HTMLElement>();
  function measure(): void {
    if (!box || !ruler || band.kind !== "ask") return;
    const style = getComputedStyle(box);
    const available = box.clientWidth - (parseFloat(style.paddingLeft) || 0) - (parseFloat(style.paddingRight) || 0);
    const next = ruler.offsetWidth > available;
    if (next !== short) short = next;
  }
  $effect(() => {
    const b = box;
    const r = ruler;
    if (!b || !r) return;
    const observer = new ResizeObserver(() => measure());
    observer.observe(b);
    observer.observe(r);
    return () => observer.disconnect();
  });
  $effect(() => {
    void [band.kind, slim, sounds.packBytes, sounds.total];
    measure();
  });
</script>

{#snippet buttons(measuring: boolean)}
  {#if band.kind === "ask"}
    <Button primary test={measuring ? undefined : "pack-fetch"} onclick={() => send({ type: "fetchPack" })}>{t("button.fetchPack")}</Button>
    <Button test={measuring ? undefined : "pack-later"} onclick={() => send({ type: "packLater" })}>{t("button.packLater")}</Button>
  {:else if band.kind === "fetching"}
    <Button test={measuring ? undefined : "pack-stop"} onclick={() => send({ type: "stopPackFetch" })}>{t("button.cancel")}</Button>
  {:else}
    <Button primary test={measuring ? undefined : "pack-again"} onclick={() => send({ type: "fetchPack" })}>{againLabel(band.fetch.failure)}</Button>
    <Button test={measuring ? undefined : "pack-later"} onclick={() => send({ type: "packLater" })}>{t("button.packLater")}</Button>
  {/if}
{/snippet}

<div class="band" class:slim data-test="pack-band" data-kind={band.kind} bind:this={box}>
  <div class="in">
    <div class="lines">
      <span class="title" data-test="pack-title">{lines[0]}</span>
      <span class="detail" data-test="pack-detail">{lines[1]}</span>
    </div>
    {#if band.kind === "fetching"}
      <span class="bar" data-test="pack-bar"><span class="fill" style:transform="scaleX({percent / 100})"></span></span>
    {/if}
    {@render buttons(false)}
  </div>
  {#if band.kind === "ask"}
    <div class="in ruler" aria-hidden="true" bind:this={ruler}>
      <div class="lines">
        <span class="title">{askLines(sounds, false)[0]}</span>
        <span class="detail">{askLines(sounds, false)[1]}</span>
      </div>
      {@render buttons(true)}
    </div>
  {/if}
</div>

<style>
  .band {
    position: relative;
    flex: 1;
    min-width: 0;
    height: 100%;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding: 0 var(--space-3);
    border-left: var(--line-mark) solid var(--color-accent);
    white-space: nowrap;
    overflow: hidden;
  }
  .in {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-width: 0;
  }
  .in > :not(.lines) {
    flex-shrink: 0;
  }
  .lines {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    min-width: 0;
    overflow: hidden;
  }
  .slim .lines {
    flex-direction: row;
    align-items: baseline;
    gap: var(--space-2);
  }
  .title,
  .detail {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .slim .title {
    flex-shrink: 0;
  }
  .title {
    color: var(--color-gray-10);
  }
  .detail {
    color: var(--color-gray-8);
  }
  .bar {
    width: var(--part-packband-bar);
    height: var(--mock-packband-bar);
    background: var(--color-gray-4);
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--color-accent);
    transform-origin: left;
  }
  /* In the slim input meter (height 24), buttons are the same height as 「やめる」 in the status line. */
  .slim :global(.b) {
    height: var(--mock-exporting-stop);
  }
  .ruler {
    position: absolute;
    left: 0;
    top: 0;
    width: max-content;
    visibility: hidden;
    pointer-events: none;
  }
</style>

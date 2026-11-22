<!-- 入力計の帯の案内（音の素材。仕様書 2 章「案内の帯（音の素材）」、3 章 43-B ・ 44-A、モック F8-PackAsk ・ F8-PackGet ・ F8-PackFail ・ R12-Fit）。
     右は accent の line.mark の線から、案内の文（2 行）、160 の棒、ボタン。聞いている / 取っている / 取れなかった。意図を送るのはボタンだけ。
     入りきらないときは、聞いている帯の小さい字を R12-Fit の短い文にし、それでも入らなければ文のうしろを「…」でちぢめる（棒とボタンは残す）。
     入力計が細いとき（下の帯がひらいている）は、2 行を 1 行にならべる。 -->
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

  // 聞いている帯: 長い文の幅（見えない写し）が、帯の中に入るか。
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
  /* 細い入力計（高さ 24）では、状態行の「やめる」と同じ高さのボタン。 */
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

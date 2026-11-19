<!-- 棚の下のスキャンの帯（仕様書 3 章 Y13、6 章「スキャン中」、モック R9-Scan）。スキャンが動いている間だけ。
     数が変わったときだけ書きかわる（60 Hz では動かさない）。 -->
<script lang="ts">
  import { scanStripText } from "./scan";

  let { done, total }: { done: number; total: number } = $props();
  const text = $derived(scanStripText(done, total));
</script>

<div class="scan" data-test="scan-strip">
  <div class="line">
    <span>{text.label}</span>
    <span class="m-num m-lbl" data-test="scan-count">{text.count}</span>
  </div>
  <div class="bar"><div class="fill" style:width="{text.done * 100}%"></div></div>
  <span class="m-lbl note">{text.note}</span>
</div>

<style>
  .scan {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-3);
    border-top: var(--line-hair) solid var(--color-gray-5);
    white-space: nowrap;
  }
  .line {
    display: flex;
    justify-content: space-between;
    gap: var(--space-2);
  }
  .bar {
    height: var(--mock-scan-progress);
    background: var(--color-gray-0);
  }
  .fill {
    height: 100%;
    background: var(--color-accent);
  }
  .note {
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>

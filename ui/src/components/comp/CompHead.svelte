<!-- The head side of the band 「いいとこ取り」 (round 12, A8): 「▾ いいとこ取り」 (pressing it closes the band), then the name of each take's row. -->
<script lang="ts">
  import { t } from "../../text";
  import { metrics } from "../metrics.svelte";
  import { bandHeight } from "./comp";
  import { closeComp, type OpenComp } from "./compState.svelte";

  let { band }: { band: OpenComp } = $props();
  const height = $derived(bandHeight({ headH: metrics.px("size.row"), rowH: metrics.px("size.lane") }, band.takes.length));
</script>

<div class="comp-head" style:height="{height}px" data-test="comp-head">
  <button class="m-plain title" aria-expanded="true" data-test="comp-close" onclick={closeComp}>
    <span>▾</span>
    <span>{t("comp.title")}</span>
  </button>
  {#each band.takes as take (take.takeId)}
    <div class="m-lbl take" data-test="comp-take">{t("comp.take", { take: take.number })}</div>
  {/each}
</div>

<style>
  .comp-head {
    box-sizing: border-box;
    background: var(--color-gray-2);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    white-space: nowrap;
    overflow: hidden;
  }
  .title {
    width: 100%;
    height: var(--size-row);
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: 0 var(--space-2) 0 var(--space-6);
    cursor: pointer;
  }
  .take {
    height: var(--size-lane);
    display: flex;
    align-items: center;
    padding: 0 var(--space-2) 0 var(--space-6);
  }
</style>

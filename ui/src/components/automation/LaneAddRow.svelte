<!-- The 「＋ 帯を足す ▾」 row below a track's lanes (spec 3 (92-5, 20-A), mock Opt7-AddLane, F7-Answers92).
     Pressing it opens the list to add a lane (laneAdd.svelte.ts). The number at the right end is how many parameter lanes are folded (2 in mock F6-ParamMany). -->
<script lang="ts">
  import type { TrackView } from "../../bridge";
  import { current } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { hasLaneRef } from "./laneValue";
  import { openLaneAdd } from "./laneAdd.svelte";

  let { track }: { track: TrackView } = $props();

  const folded = $derived(current.state!.automation.folded);
  const busy = $derived(current.state!.transport.state === "recording" || current.state!.transport.state === "countIn");
  const foldedParams = $derived(
    track.lanes.filter((l) => (l.target.kind === "insert" || l.target.kind === "instrument") && hasLaneRef(folded, track.id, l.target)).length,
  );

  function open(ev: MouseEvent): void {
    if (busy) return;
    openLaneAdd(ev.currentTarget as HTMLElement, track, busy, false);
  }
</script>

<div class="add-row" data-test="lane-add-row">
  <button class="m-plain m-lbl item" class:usable={!busy} aria-disabled={busy || undefined} aria-haspopup="menu" data-test="lane-add" onclick={open}
    >{t("lane.add")}</button
  >
  <span class="m-spacer"></span>
  {#if foldedParams > 0}<span class="m-num m-lbl count">{foldedParams}</span>{/if}
</div>

<style>
  .add-row {
    height: var(--size-row);
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: 0 var(--space-2) 0 var(--space-6);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    white-space: nowrap;
    overflow: hidden;
  }
  .item {
    flex-shrink: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-gray-7);
    cursor: default;
  }
  .count {
    flex-shrink: 0;
  }
  .item.usable {
    color: var(--color-gray-8);
    cursor: pointer;
  }
  .item.usable:hover {
    color: var(--color-gray-10);
  }
</style>

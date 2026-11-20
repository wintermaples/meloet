<!-- トラックの帯の下の「▸ パン ▸ プラグインのパラメータ」の行（仕様書 3 章の帯の表「たたんで 24」、Y16、モック F6-AutoRec ・ F6-ParamMany）。
     「▸ （何を）」を押すと、その帯を足す（たたんだ帯なら、ひらく）。「▸ プラグインのパラメータ」は、右のパラメータの欄の一覧をひらく
     （そこの行の右の印で帯を足す）。右はしの数は、たたんでいるパラメータの帯の数（モック F6-AutoRec の 1、F6-ParamMany の 2）。 -->
<script lang="ts">
  import type { AutoTarget, Command, LaneName, TrackView } from "../../bridge";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { laneNameText } from "../history/stepText";
  import { hasLaneRef, laneOf } from "./laneValue";

  let { track }: { track: TrackView } = $props();

  const folded = $derived(current.state!.automation.folded);
  const busy = $derived(current.state!.transport.state === "recording" || current.state!.transport.state === "countIn");
  const isFolded = (target: AutoTarget) => hasLaneRef(folded, track.id, target);

  /** 音量とパンは、ひらいた帯がなければいつも。ミュートとセンドは、たたんだ帯があるときだけ（ひらく所がほかにないので）。 */
  const items = $derived.by(() => {
    const out: { target: AutoTarget; name: LaneName }[] = [];
    for (const kind of ["volume", "pan"] as const) {
      const lane = laneOf(track, { kind });
      if (!lane || isFolded(lane.target)) out.push({ target: { kind }, name: { kind } });
    }
    for (const lane of track.lanes) {
      if ((lane.target.kind === "mute" || lane.target.kind === "send") && isFolded(lane.target)) out.push({ target: lane.target, name: lane.name });
    }
    return out;
  });
  const foldedParams = $derived(track.lanes.filter((l) => (l.target.kind === "insert" || l.target.kind === "instrument") && isFolded(l.target)).length);

  /** パラメータの欄にひらく物: プラグインの音源、なければいちばん上のエフェクト。 */
  const openParams = $derived.by((): Command | null => {
    if (track.sound.kind === "plugin") return { type: "selectInstrument", track: track.id };
    const slot = track.slots.findIndex((s) => s !== null);
    return slot < 0 ? null : { type: "selectInsert", insert: { strip: { kind: "track", id: track.id }, slot } };
  });
</script>

<div class="add-row" data-test="lane-add-row">
  {#each items as item (item.target.kind + (item.target.kind === "send" ? item.target.bus : ""))}
    <button
      class="m-plain m-lbl item"
      class:usable={!busy}
      aria-disabled={busy || undefined}
      data-test="lane-add-{item.target.kind}"
      onclick={() => !busy && send({ type: "addLane", track: track.id, target: item.target })}>▸ {laneNameText(item.name)}</button
    >
  {/each}
  <button
    class="m-plain m-lbl item params"
    class:usable={openParams !== null}
    aria-disabled={openParams === null || undefined}
    data-test="lane-add-params"
    onclick={() => openParams && send(openParams)}>▸ {t("lane.pluginParams")}</button
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
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    white-space: nowrap;
    overflow: hidden;
  }
  .item {
    flex-shrink: 0;
    color: var(--color-gray-7);
    cursor: default;
  }
  /* 入りきらないときは「プラグインのパラメータ」を「…」で切る（数は残す）。 */
  .item.params {
    flex-shrink: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
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

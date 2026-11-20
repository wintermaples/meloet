<!-- オートメーションの帯の見出し（仕様書 2 章「オートメーションの帯の見出し ・ 書く ・ 休み」、モック F6-AutoRec ・ F6-AutoRest ・ F6-ParamMany）。
     上の段は「▾ オートメーション ・ （何を）」（押すとたたむ）。下の段は「● 書く」と短い文（書いている間は「書いています」、ふだんは
     いまの値と点の数）。休みのときは「もどす」と「休み ・ （いまの値）」。右クリックで「帯を消す」。
     いまの値は再生中に毎コマ変わるので、trackMeters の canvas の下に入れる（cover。docs/architecture.md 7 章「CPU で描く WebKit」）。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { LaneView, TrackView } from "../../bridge";
  import { glSlot } from "../../gl/slot";
  import { term } from "../../lib/tip.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { laneNameText } from "../history/stepText";
  import { termParts, textParts, type TextPart } from "../format";
  import { builtinOf } from "../params/selected";
  import { livePosition, pointsOf } from "./autoState.svelte";
  import { laneMenu } from "./laneMenu";
  import { handPosition, hasLaneRef, laneMoves, positionText, valueAt } from "./laneValue";

  let { track, lane }: { track: TrackView; lane: LaneView } = $props();

  const automation = $derived(current.state!.automation);
  const busy = $derived(current.state!.transport.state === "recording" || current.state!.transport.state === "countIn");
  const held = $derived(hasLaneRef(automation.held, track.id, lane.target));
  const resting = $derived(lane.resting || held);
  const armed = $derived(hasLaneRef(automation.armed, track.id, lane.target));
  const writing = $derived(automation.writing.some((w) => w.lane.track === track.id && JSON.stringify(w.lane.target) === JSON.stringify(lane.target)));
  const title = $derived(termParts(t("lane.title", { target: laneNameText(lane.name) }), "オートメーション"));

  /** 最初から入っているエフェクトの手で置いた値は、パラメータの欄が見せている枠のときだけ分かる。 */
  const values = $derived.by(() => {
    const sel = builtinOf(current.state!.params);
    if (!sel || lane.target.kind !== "insert" || sel.insert.strip.kind !== "track" || sel.insert.strip.id !== track.id) return null;
    return track.slots[sel.insert.slot]?.id === lane.target.insert ? sel.values : null;
  });
  const moves = $derived(laneMoves(lane, automation.held, track.id));
  /** 動かしているときは線の上の値。書く係（AutoClock）がまだ書いていなければ、止まっている所の値をここで出す。 */
  const position = $derived.by(() => {
    if (!moves) return handPosition(track, lane.target, values);
    // 見えていない見出しは、毎コマの値を読まない（64 本の曲で、見えない字を毎コマ書きかえないため）。
    const live = visible ? livePosition(track.id, lane.target) : null;
    if (live !== null) return live;
    const points = pointsOf(lane.laneId);
    return points ? valueAt(points, current.state!.transport.position) : null;
  });
  const value = $derived(position === null ? null : positionText(lane.name, position));

  let box = $state<HTMLElement>();
  let visible = $state(true);
  onMount(() => {
    if (!box || typeof IntersectionObserver === "undefined") return;
    const seen = new IntersectionObserver((entries) => (visible = entries[entries.length - 1]!.isIntersecting));
    seen.observe(box);
    return () => seen.disconnect();
  });
  /** いまの値の字が変わるコマは、上の canvas（trackMeters）も出す: 字の四角が canvas の中に入り、再生の線の四角を切らない。 */
  const stamp = $derived(position ?? -1);

  function fold(): void {
    send({ type: "foldLane", track: track.id, target: lane.target, folded: true });
  }
  function context(e: MouseEvent): void {
    e.preventDefault();
    laneMenu(e, track, lane, busy);
  }
</script>

{#snippet parts(list: TextPart[])}{#each list as part, i (i)}{#if part.hole}<span class="m-num">{part.text}</span>{:else}{part.text}{/if}{/each}{/snippet}

<div class="lane-head" role="presentation" data-test="lane-head" bind:this={box} oncontextmenu={context}>
  <button class="m-plain title" aria-expanded="true" data-test="lane-fold" onclick={fold}
    ><span class="m-hi">▾</span>
    {#each title as piece, i (i)}{#if piece.term}<span use:term={"オートメーション"}>{piece.text}</span>{:else}{piece.text}{/if}{/each}</button
  >
  <div class="line">
    {#if resting}
      {@const usable = lane.resting && !busy}
      <button
        class="m-btn small"
        aria-disabled={!usable || undefined}
        data-test="lane-restore"
        onclick={() => usable && send({ type: "restoreLane", track: track.id, target: lane.target })}>{t("lane.restore")}</button
      >
      <span class="m-lbl live" use:glSlot={{ kind: "cover", stamp }}
        ><span class="in" data-test="lane-state"
          >{#if value === null}{t("lane.restingBare")}{:else}{@render parts(textParts("lane.resting", { value }))}{/if}</span
        ></span
      >
    {:else}
      <button
        class="m-btn small write"
        class:on={armed}
        aria-pressed={armed}
        aria-disabled={busy || undefined}
        data-test="lane-write"
        onclick={() => !busy && send({ type: "setLaneWrite", track: track.id, target: lane.target, on: !armed })}>{t("lane.write")}</button
      >
      <span class="m-lbl live" use:glSlot={{ kind: "cover", stamp }}
        ><span class="in" data-test="lane-state"
          >{#if writing}{t("lane.writing")}{:else if value !== null}{@render parts(textParts("lane.now", { value, count: lane.points }))}{:else}{@render parts(
              textParts("lane.count", { count: lane.points }),
            )}{/if}</span
        ></span
      >
    {/if}
  </div>
</div>

<style>
  /* 帯の見出し（モック F6-AutoRec: 高さ size.lane、地 gray.2、左のあき space.6）。 */
  .lane-head {
    /* 縦に送った外の帯は、中を描かない ・ レイアウトしない（64 本の曲。高さは決まっている）。 */
    content-visibility: auto;
    height: var(--size-lane);
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 0 var(--space-2) 0 var(--space-6);
    background: var(--color-gray-2);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    white-space: nowrap;
    overflow: hidden;
  }
  .title {
    display: flex;
    gap: var(--space-1);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }
  /* いまの値の字は、流れの外の箱に入れる: 字が変わっても、見出しのならびはレイアウトしなおされない
     （docs/architecture.md 7 章「CPU で描く WebKit」）。外の箱は、字と同じ高さとベースラインを幅のない字で保つ。 */
  .live {
    position: relative;
    flex: 1;
    min-width: 0;
  }
  .live::before {
    content: "\200b";
  }
  .live > .in {
    position: absolute;
    inset: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
  .small {
    height: var(--part-toggle-size);
    flex-shrink: 0;
  }
  /* 書くがオン（仕様書 2 章: ボタンは「えらんでいる」の形で、● と文字が rec の色）。 */
  .write.on {
    border-color: var(--color-accent);
    color: var(--color-rec);
  }
</style>

<!-- The heading of an automation lane (spec 2 "Automation band heading, write, resting", mock F6-AutoRec, F6-AutoRest, F6-ParamMany).
     The upper line is 「▾ オートメーション ・ （何を）」 (pressing folds it). The lower line is 「● 書く」 and a short text (「書いています」 while writing, otherwise
     the current value and the number of points). When resting, 「もどす」 and 「休み ・ （いまの値）」. Right-click gives 「帯を消す」.
     The current value changes every frame while playing, so it goes under the trackMeters canvas (cover. docs/architecture.md 7, "WebKit that paints on the CPU"). -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { LaneView } from "../../bridge";
  import { glSlot } from "../../gl/slot";
  import { term } from "../../lib/tip.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { laneNameText } from "../history/stepText";
  import { termParts, textParts, type TextPart } from "../format";
  import { builtinOf } from "../params/selected";
  import { livePosition, pointsOf } from "./autoState.svelte";
  import { laneMenu } from "./laneMenu";
  import { handPosition, hasLaneRef, isOnTrack, laneMoves, positionText, trackHome, valueAt, MASTER_HOME, type LaneOwner } from "./laneValue";

  let { track, lane }: { track: LaneOwner; lane: LaneView } = $props();

  const automation = $derived(current.state!.automation);
  const busy = $derived(current.state!.transport.state === "recording" || current.state!.transport.state === "countIn");
  const held = $derived(hasLaneRef(automation.held, track.id, lane.target));
  const resting = $derived(lane.resting || held);
  const armed = $derived(hasLaneRef(automation.armed, track.id, lane.target));
  const writing = $derived(automation.writing.some((w) => isOnTrack(w.lane, track.id) && JSON.stringify(w.lane.target) === JSON.stringify(lane.target)));
  const title = $derived(termParts(t("lane.title", { target: laneNameText(lane.name) }), "オートメーション"));
  /** The master's row says whose it is in place of 「オートメーション」 (round 12 M5). */
  const ofMaster = $derived(track.id === MASTER_HOME);

  /** A value set by a step of a built-in effect is known only while the params panel shows that slot. */
  const values = $derived.by(() => {
    const sel = builtinOf(current.state!.params);
    if (!sel || lane.target.kind !== "insert" || sel.insert.strip.kind !== "track" || sel.insert.strip.id !== track.id) return null;
    return track.slots[sel.insert.slot]?.id === lane.target.insert ? sel.values : null;
  });
  const moves = $derived(laneMoves(lane, automation.held, track.id));
  /** While moving, the value on the line. If the writer (AutoClock) has not written yet, the value at the stopped place is shown here. */
  const position = $derived.by(() => {
    if (!moves) return handPosition(track, lane.target, values);
    // A heading that is not visible does not read the value every frame (so that hidden text is not rewritten every frame in songs with 64 tracks).
    const live = visible ? livePosition(track.id, lane.target) : null;
    if (live !== null) return live;
    const points = pointsOf(lane.laneId);
    return points ? valueAt(points, current.state!.transport.position) : null;
  });
  const value = $derived(position === null ? null : positionText(lane.name, position));

  let box = $state<HTMLElement>();
  // Where there is no IntersectionObserver (tests), it is always treated as visible.
  let visible = $state(typeof IntersectionObserver === "undefined");
  onMount(() => {
    if (!box || typeof IntersectionObserver === "undefined") return;
    const seen = new IntersectionObserver((entries) => (visible = entries[entries.length - 1]!.isIntersecting));
    seen.observe(box);
    return () => seen.disconnect();
  });
  /** In a frame where the current value text changes, the canvas above (trackMeters) is also shown: the text rectangle is inside the canvas and does not cut the play line's rectangle. */
  const stamp = $derived(position ?? -1);
  /** Only while visible, the text box goes under the canvas (so the WebGL manager does not measure the boxes of hidden headings every frame). */
  function coverWhile(node: HTMLElement, now: { on: boolean; stamp: number }) {
    let slot = now.on ? glSlot(node, { kind: "cover", stamp: now.stamp }) : null;
    return {
      update(next: { on: boolean; stamp: number }) {
        if (!next.on) {
          slot?.destroy();
          slot = null;
        } else if (slot) slot.update({ kind: "cover", stamp: next.stamp });
        else slot = glSlot(node, { kind: "cover", stamp: next.stamp });
      },
      destroy() {
        slot?.destroy();
      },
    };
  }

  function fold(): void {
    send({ type: "foldLane", home: trackHome(track.id), target: lane.target, folded: true });
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
    {#if ofMaster}<span class="m-hi" data-test="lane-master">{t("mixer.master")}</span><span class="m-lbl">{laneNameText(lane.name)}</span
      >{:else}{#each title as piece, i (i)}{#if piece.term}<span use:term={"オートメーション"}>{piece.text}</span>{:else}{piece.text}{/if}{/each}{/if}</button
  >
  <div class="line">
    {#if resting}
      {@const usable = lane.resting && !busy}
      <button
        class="m-btn small"
        aria-disabled={!usable || undefined}
        data-test="lane-restore"
        onclick={() => usable && send({ type: "restoreLane", home: trackHome(track.id), target: lane.target })}>{t("lane.restore")}</button
      >
      <span class="m-lbl live" use:coverWhile={{ on: visible, stamp }}
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
        onclick={() => !busy && send({ type: "setLaneWrite", home: trackHome(track.id), target: lane.target, on: !armed })}>{t("lane.write")}</button
      >
      <span class="m-lbl live" use:coverWhile={{ on: visible, stamp }}
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
  /* The lane heading (mock F6-AutoRec: height size.lane, background gray.2, space.6 on the left). */
  .lane-head {
    /* Headings scrolled out of view are not painted or laid out (songs with 64 tracks. The height is fixed). */
    content-visibility: auto;
    height: var(--size-lane);
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 0 var(--space-2) 0 var(--space-6);
    background: var(--color-gray-2);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
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
  /* The current value text goes in an out-of-flow box: when the text changes, the heading is not laid out again
     (docs/architecture.md 7, "WebKit that paints on the CPU"). The outer box keeps the same height and baseline as the text with a zero-width character. */
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
  /* Write is on (spec 2: the button has the 「えらんでいる」 shape, with ● and the text in the rec color). */
  .write.on {
    border-color: var(--color-accent);
    color: var(--color-rec);
  }
</style>

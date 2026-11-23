<!-- The frame where audio files will go while they are dragged in (spec 3 (29-A): 「持ってきている間、入る所に gray.7 の枠」, mock F7-AudioDrop).
     Laid over the arrange clip area (.lanes). Inside the frame, the name of the first file (with several, 「… ほか 7」).
     The length is not known yet, so it is two bars wide (128 px in the mock). Registers the page place → drop target function with fileDrag (the same function on drop). -->
<script lang="ts">
  import type { DropMode, TrackView } from "../../bridge";
  import { current } from "../../state/app.svelte";
  import { t } from "../../text";
  import type { TempoMap } from "../../time/tempoMap";
  import { nearestBeatTick } from "../../time/bars";
  import { type LaneDims, type View } from "../arrangeGeom";
  import { dropFrameBox, dropTarget, type DropTarget } from "./audioClipGeom";
  import { dropModeFor, fileDrag, setDropResolver } from "./fileDrag.svelte";

  let { view, dims, tracks, map, perBar }: { view: View; dims: LaneDims; tracks: readonly TrackView[]; map: TempoMap; perBar: number } = $props();

  let root = $state<HTMLElement>();
  const drop = $derived(current.state!.workspace.fileDrop);

  /** Page place → where it goes inside the clip area. 「別々のトラックに」 makes a new track even over a vocal track (B-94-21). */
  function targetAt(pageX: number, pageY: number, mode: DropMode): DropTarget {
    const r = root?.getBoundingClientRect();
    const x = Math.min(Math.max(pageX - (r?.left ?? 0), 0), view.width);
    const y = pageY - (r?.top ?? 0);
    const target = dropTarget(view, dims, tracks, x, y, (tick) => nearestBeatTick(map, tick));
    return mode === "separate" ? { ...target, track: null, row: tracks.length } : target;
  }

  $effect(() => setDropResolver((x, y, mode) => targetAt(x, y, mode)));
  $effect(() => {
    if (!drop) fileDrag.at = null;
  });

  const frame = $derived.by(() => {
    const at = fileDrag.at;
    if (!drop || drop.audio === 0 || !at || !root) return null;
    // Show no frame outside the visible clip area (the vertical scroll box).
    const seen = (root.closest("[data-gl-clip]") ?? root).getBoundingClientRect();
    if (at.x < seen.left || at.x >= seen.right || at.y < seen.top || at.y >= seen.bottom) return null;
    const target = targetAt(at.x, at.y, dropModeFor(drop.audio, at.x, window.innerWidth));
    const box = dropFrameBox(view, dims, target, tracks.length, 2 * perBar);
    const name = drop.audio > 1 ? t("drop.frameMore", { file: drop.name, more: drop.audio - 1 }) : drop.name;
    return { box, name };
  });
</script>

<div class="host" bind:this={root}>
  {#if frame}
    <div
      class="frame"
      data-test="drop-frame"
      style:left="{frame.box.x0}px"
      style:top="{frame.box.y0}px"
      style:width="{frame.box.x1 - frame.box.x0}px"
      style:height="{frame.box.y1 - frame.box.y0}px"
    >
      <span class="m-lbl name">{frame.name}</span>
    </div>
  {/if}
</div>

<style>
  .host {
    position: absolute;
    inset: 0;
    z-index: 3;
    pointer-events: none;
  }
  .frame {
    position: absolute;
    box-sizing: border-box;
    border: var(--line-hair) solid var(--color-gray-7);
  }
  /* The name may stick out of the frame (「ドラム キック.wav ほか 7」 in mock F7-AudioDrop). */
  .name {
    display: block;
    padding: 0 var(--space-1);
    white-space: nowrap;
  }
</style>

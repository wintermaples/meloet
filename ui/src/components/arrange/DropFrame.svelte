<!-- 音のファイルを持ってきている間の、入る所の枠（仕様書 3 章 29-A「持ってきている間、入る所に gray.7 の枠」、モック F7-AudioDrop）。
     アレンジのクリップの面（.lanes）にかさねる。枠の中に、いちばん目のファイルの名前（いくつもなら「… ほか 7」）。
     長さはまだ分からないので、2 小節の幅（モックの 128 px）。ページの場所 → 落とす所の計算を fileDrag に登録する（落としたときも同じ計算）。 -->
<script lang="ts">
  import type { DropMode, TrackView } from "../../bridge";
  import { current } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import type { TempoMap } from "../../time/tempoMap";
  import { nearestBeatTick } from "../../time/bars";
  import { type LaneDims, type View } from "../arrangeGeom";
  import { dropFrameBox, dropTarget, type DropTarget } from "./audioClipGeom";
  import { dropModeFor, fileDrag, setDropResolver } from "./fileDrag.svelte";

  let { view, dims, tracks, map, perBar }: { view: View; dims: LaneDims; tracks: readonly TrackView[]; map: TempoMap; perBar: number } = $props();

  let root = $state<HTMLElement>();
  const drop = $derived(current.state!.workspace.fileDrop);

  /** ページの場所 → クリップの面の中の入る所。「別々のトラックに」は、歌のトラックの上でも新しいトラック（B-94-21）。 */
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
    // 見えているクリップの面（縦に送る箱）の外にいるときは、枠を出さない。
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
  /* 名前は枠からはみ出してよい（モック F7-AudioDrop の「ドラム キック.wav ほか 7」）。 */
  .name {
    display: block;
    padding: 0 var(--space-1);
    white-space: nowrap;
  }
</style>

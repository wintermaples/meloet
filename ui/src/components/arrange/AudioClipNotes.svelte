<!-- 歌のクリップの中の文（HTML。地の色と波形は WebGL の側）。
     見つからない音: 名前の行の下に gray.8 で「音のファイルが見つかりません」（仕様書 3 章、モック F7-AudioMissing ・ R11-Audio）。
     読みこんでいる途中: クリップの下に「読みこんでいます 40 %」（仕様書 2 章「歌のクリップ」、3 章「読めるファイル」。箱はモック R11-Audio の進みの形）。 -->
<script lang="ts">
  import type { TrackView } from "../../bridge";
  import { current } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { clipBox, type LaneDims, type View } from "../arrangeGeom";
  import { waveArea } from "./audioClipGeom";

  let { view, dims, tracks }: { view: View; dims: LaneDims; tracks: readonly TrackView[] } = $props();

  const jobs = $derived(current.state!.audioWork.jobs);

  const notes = $derived.by(() => {
    const out: { id: number; x: number; top: number; bottom: number; w: number; kind: "missing" | "loading"; text: string }[] = [];
    tracks.forEach((track, row) => {
      for (const clip of track.clips) {
        const audio = clip.audio;
        if (audio === null) continue;
        const job = jobs.find((j) => j.kind === "loading" && j.audio === audio.audio);
        const kind = audio.state === "missing" ? "missing" : job ? "loading" : null;
        if (kind === null) continue;
        const box = clipBox(view, dims, row, clip.start, clip.end);
        if (box.x1 < 0 || box.x0 > view.width) continue;
        const area = waveArea(dims, box);
        const x = Math.max(area.x0, 0);
        out.push({
          id: clip.id,
          x,
          top: area.top,
          bottom: area.bottom,
          w: area.x1 - x,
          kind,
          text: kind === "missing" ? t("clip.missing") : t("clip.loading", { percent: job!.percent }),
        });
      }
    });
    return out;
  });
</script>

{#each notes as n (n.id)}
  <div
    class="note {n.kind}"
    data-test="audio-clip-{n.kind}"
    style:left="{n.x}px"
    style:width="{n.w}px"
    style:top="{n.top}px"
    style:height="{n.bottom - n.top}px"
  >
    <span class="m-lbl text">{n.text}</span>
  </div>
{/each}

<style>
  .note {
    position: absolute;
    z-index: 2;
    pointer-events: none;
    overflow: hidden;
    display: flex;
    padding: 0 var(--space-2);
    box-sizing: border-box;
  }
  .note.missing {
    padding-top: var(--space-1);
  }
  /* 読みこんでいる進み: クリップの下のはしに、地 gray.0 ・ 枠 gray.7 の小さな箱。 */
  .note.loading {
    align-items: flex-end;
    padding-bottom: var(--space-1);
  }
  .note.loading .text {
    padding: 0 var(--part-reading-pad-x);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-7);
  }
  /* せまいクリップでは、クリップのはしで切る（モック F7-AudioMissing）。 */
  .text {
    white-space: nowrap;
    overflow: hidden;
  }
</style>

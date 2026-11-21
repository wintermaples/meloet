<!-- 歌のクリップの中の文（HTML。地の色と波形は WebGL の側）。
     見つからない音: 名前の行の下に gray.8 で「音のファイルが見つかりません」（仕様書 3 章、モック F7-AudioMissing ・ R11-Audio）。
     読みこんでいる途中: クリップの下に「読みこんでいます 40 %」（仕様書 2 章「歌のクリップ」、3 章「読めるファイル」。箱はモック R11-Audio の進みの形）。
     音を作っている間: 同じ所と形で「音を作っています 34 %」（仕様書 2 章、3 章「音を作っている間」、モック F7-Stretch）。
     % は state/audioWork.svelte の見せる進み（1 秒に 2 回まで）。字が変わる箱は、上から下までの列（askHost の中の見えない箱）の四角を
     先に 1 つ頼む: 箱だけの四角だと、トラックのメーターと再生の線の四角を上下に切って、1 コマの四角が 10 個をこえる（architecture 7 章）。 -->
<script lang="ts">
  import type { TrackView } from "../../bridge";
  import { current } from "../../state/app.svelte";
  import { shownWork } from "../../state/audioWork.svelte";
  import { t } from "../../text/ja";
  import { askWhole } from "../automation/coverIf";
  import { clipBox, type LaneDims, type View } from "../arrangeGeom";
  import { waveArea } from "./audioClipGeom";

  let {
    view,
    dims,
    tracks,
    askHost = null,
  }: { view: View; dims: LaneDims; tracks: readonly TrackView[]; askHost?: HTMLElement | null } = $props();

  const work = $derived(shownWork.current);
  /** 再生の線とトラックのメーターが毎コマ動いている間（このときだけ列の四角を頼む）。 */
  const moving = $derived(current.state!.transport.state !== "stopped");

  const notes = $derived.by(() => {
    const out: { id: number; x: number; top: number; bottom: number; w: number; kind: "missing" | "loading" | "rendering"; text: string }[] = [];
    tracks.forEach((track, row) => {
      for (const clip of track.clips) {
        const audio = clip.audio;
        if (audio === null) continue;
        const missing = audio.state === "missing";
        if (!missing && work.jobs.length === 0) continue;
        const box = clipBox(view, dims, row, clip.start, clip.end);
        if (box.x1 < 0 || box.x0 > view.width) continue;
        const job = missing ? null : (work.loading.get(audio.audio) ?? work.rendering.get(clip.id) ?? null);
        const kind = missing ? "missing" : (job?.kind ?? null);
        if (kind === null) continue;
        const area = waveArea(dims, box);
        const x = Math.max(area.x0, 0);
        out.push({
          id: clip.id,
          x,
          top: area.top,
          bottom: area.bottom,
          w: area.x1 - x,
          kind,
          text: kind === "missing" ? t("clip.missing") : t(kind === "loading" ? "clip.loading" : "clip.rendering", { percent: job!.percent }),
        });
      }
    });
    return out;
  });

  type Ask = { x: number; w: number; kind: "missing" | "loading" | "rendering"; text: string };

  /** 2 けたの % のうち、いちばん幅の広い字はこのどれか（同じ数字 2 つか、数字と 0）。 */
  const WIDEST = Array.from({ length: 9 }, (_, i) => [(i + 1) * 11, (i + 1) * 10]).flat();

  /**
   * 再生の線が動いている間に字が変わったら、その字の列（askHost の上から下まで。再生の線と同じ）の四角を先に 1 つ頼む。列の箱は、はじめて
   * 頼むときに作る。列の幅は、いちばん幅の広い % の字を見えない字で入れて決める（字の四角が、いつも列の中に入る）。止まっている間は
   * 頼まない: ほかに動く物がなければ、列と状態行だけで囲む四角の 25 % をこえて、囲む四角をまるごと描く。
   */
  function askColumn(node: HTMLElement, at: Ask) {
    let text = at.text;
    let column: HTMLElement | null = null;
    function make(kind: "loading" | "rendering"): HTMLElement {
      const box = document.createElement("span");
      box.setAttribute("aria-hidden", "true");
      box.dataset.test = "audio-clip-ask";
      box.className = "m-lbl";
      Object.assign(box.style, { position: "absolute", top: "0", bottom: "0", display: "grid", overflow: "hidden", pointerEvents: "none" });
      for (const percent of WIDEST) {
        const line = document.createElement("span");
        line.textContent = t(kind === "loading" ? "clip.loading" : "clip.rendering", { percent });
        Object.assign(line.style, {
          gridArea: "1 / 1",
          visibility: "hidden",
          whiteSpace: "nowrap",
          padding: "0 var(--part-reading-pad-x)",
          border: "var(--line-hair) solid transparent",
        });
        box.append(line);
      }
      return box;
    }
    return {
      update(next: Ask) {
        if (next.text === text) return;
        text = next.text;
        if (!askHost || !moving || next.kind === "missing") return;
        if (column?.dataset.kind !== next.kind) {
          column?.remove();
          column = make(next.kind);
          column.dataset.kind = next.kind;
          askHost.append(column);
        }
        column.style.left = `calc(${next.x}px + var(--space-2))`;
        column.style.maxWidth = `calc(${next.w}px - 2 * var(--space-2))`;
        askWhole(column);
      },
      destroy() {
        column?.remove();
      },
    };
  }
</script>

{#each notes as n (n.id)}
  <div
    class="note {n.kind === 'missing' ? 'missing' : 'loading'}"
    data-test="audio-clip-{n.kind}"
    style:left="{n.x}px"
    style:width="{n.w}px"
    style:top="{n.top}px"
    style:height="{n.bottom - n.top}px"
    use:askColumn={{ x: n.x, w: n.w, kind: n.kind, text: n.text }}
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

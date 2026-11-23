<!-- Text inside audio clips (HTML; the background and waveform are on the WebGL side).
     Missing audio: 「音のファイルが見つかりません」 in gray.8 under the name row (spec 3, mock F7-AudioMissing / R11-Audio).
     While loading: 「読みこんでいます 40 %」 at the bottom of the clip (spec 2 "Audio clip", spec 3 "Files that can be read"; the box is the progress shape of mock R11-Audio).
     While making the sound: 「音を作っています 34 %」 in the same place and shape (spec 2, spec 3 "While the sound is being made", mock F7-Stretch).
     The % is the shown progress from state/audioWork.svelte (up to twice a second). A box whose text changes first asks for the rect of a top-to-bottom
     column (an invisible box in askHost): a rect of the box alone would cut the track meter and play line rects into pieces, giving more than 10 rects in one frame (architecture 7). -->
<script lang="ts">
  import type { TrackView } from "../../bridge";
  import { current } from "../../state/app.svelte";
  import { shownWork } from "../../state/audioWork.svelte";
  import { t } from "../../text";
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
  /** While the play line and track meters move every frame (only then ask for the column rect). */
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

  /** The widest two-digit % text is one of these (two same digits, or a digit and 0). */
  const WIDEST = Array.from({ length: 9 }, (_, i) => [(i + 1) * 11, (i + 1) * 10]).flat();

  /**
   * If the text changes while the play line moves, first ask for one rect of that text's column (top to bottom of askHost, like the play line). The column box is made
   * the first time it is asked for. Its width comes from the widest % text put in as invisible text (so the text rect always fits in the column). While stopped,
   * do not ask: with nothing else moving, the column and the status line alone would go over 25 % of the bounding rect, and the whole bounding rect would be drawn.
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
  /* Loading progress: a small box at the bottom of the clip, background gray.0, border gray.7. */
  .note.loading {
    align-items: flex-end;
    padding-bottom: var(--space-1);
  }
  .note.loading .text {
    padding: 0 var(--part-reading-pad-x);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-7);
  }
  /* In a narrow clip, cut it at the clip edge (mock F7-AudioMissing). */
  .text {
    white-space: nowrap;
    overflow: hidden;
  }
</style>

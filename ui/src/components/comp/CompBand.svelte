<!-- The time side of the band 「いいとこ取り」 under a vocal track (round 12, A8): a heading row, then a row for each take (the newest first).
     Each row has a bar as wide as the clip with the take's wave on it; where the take is chosen the bar is accent.
     Holding a row and moving sideways chooses that take for the range: only the screen changes first (the parts chosen before keep a gray.7
     frame), and one setComp is sent on release. The ends snap to the grid lines in view (not with Shift). Esc cancels (cancelDrag in pianoroll/view.svelte.ts).
     The bars and waves are WebGL (one slot of the arrange's arrangeLanes region; no new region); the heading is HTML. -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { CompPiece } from "../../bridge";
  import { glSlot, type GlWave } from "../../gl/slot";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text";
  import type { TempoMap } from "../../time/tempoMap";
  import * as W from "../arrange/wave";
  import type { WaveBook } from "../arrange/wavePeaks.svelte";
  import { xToTick, type View } from "../arrangeGeom";
  import { metrics } from "../metrics.svelte";
  import { onDragCancel } from "../pianoroll/view.svelte";
  import * as C from "./comp";
  import type { OpenComp } from "./compState.svelte";

  interface Props {
    band: OpenComp;
    view: View;
    map: TempoMap;
    busy: boolean;
    /** The grid line nearest to a tick, of the lines the arrange shows now. */
    snap: (tick: number) => number;
    waveBook: WaveBook;
    waveSizes: Pick<W.WaveLook, "pitch" | "bar" | "minH">;
  }
  let { band, view, map, busy, snap, waveBook, waveSizes }: Props = $props();

  const clip = $derived(band.clip);
  const dims: C.BandDims = $derived({
    headH: metrics.px("size.row"),
    rowH: metrics.px("size.lane"),
    pad: metrics.px("space.1"),
    hair: metrics.px("line.hair"),
  });
  const still = () => metrics.px("space.1");

  // ---- Dragging ----

  let drag = $state.raw<(C.CompDrag & { downX: number; moved: boolean }) | null>(null);
  /** The look kept after release until the answer arrives (so the pieces do not jump back for a moment). */
  let pending = $state.raw<{ clip: number; pieces: CompPiece[]; rev: number | null } | null>(null);
  $effect(() => {
    const p = pending;
    if (p && (p.clip !== clip.id || (p.rev !== null && current.rev >= p.rev))) pending = null;
  });

  const now = $derived(C.piecesOf(clip));
  const dragged = $derived(drag?.moved ? C.draggedPieces(clip, drag) : null);
  const shown = $derived(dragged ?? (pending?.clip === clip.id ? pending.pieces : now));

  const bars = $derived(C.takeBars(view, dims, clip, band.takes, shown));
  const picture = $derived.by(() => {
    void waveBook.version;
    const rects = C.bandRects(bars);
    const waves: GlWave[] = [];
    const length = clip.end - clip.start;
    for (const b of bars) {
      if (b.take.state === "missing") continue;
      const timing = W.takeTiming(clip, b.take);
      const found = waveBook.sourceFor(current.state!.song.serial, b.take.audio, W.waveLevel(W.framesPerPx(map, view, timing, clip.start, clip.end), waveSizes.pitch));
      if (!found) continue;
      const shape: W.WaveClip = { timing, start: clip.start, end: clip.end, fadeIn: 0, fadeOut: 0, gain: 1, endFrame: b.take.firstFrame + b.take.frames };
      const area = { x0: b.x, x1: b.x + b.w, top: b.y, bottom: b.y + b.h, mid: b.y + b.h / 2 };
      // Light on the gray ground, dark where the bar is accent.
      for (const [i, p] of shown.entries()) {
        const range = { from: clip.start + p.from, to: clip.start + (shown[i + 1]?.from ?? length) };
        const look = W.waveLook(waveSizes, p.take === b.take.takeId ? C.WAVE_CHOSEN : C.WAVE_PLAIN, false);
        W.clipWaves(waves, rects.length, map, view, area, shape, found.source, found.framesPerPeak, look, range);
      }
    }
    const was = dragged ? C.takeBars(view, dims, clip, band.takes, now) : null;
    return { rects: rects.concat(C.wasFrames(dims, was)), waves };
  });

  let node = $state<HTMLElement>();
  const at = (e: MouseEvent) => {
    const r = node!.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };
  const tickAt = (x: number, shift: boolean) => C.dragTick(clip, xToTick(view, x), shift ? null : snap);

  function down(e: PointerEvent): void {
    // The band is not the track's row: a press here never reaches the clips.
    e.stopPropagation();
    if (e.button !== 0 || busy) return;
    const p = at(e);
    const row = C.rowAt(dims, band.takes.length, p.y);
    const bar = bars[row];
    if (!bar || p.x < bar.x || p.x >= bar.x + bar.w) return;
    node!.setPointerCapture?.(e.pointerId);
    pending = null;
    const tick = tickAt(p.x, e.shiftKey);
    drag = { take: bar.take.takeId, from: tick, to: tick, downX: p.x, moved: false };
  }
  function move(e: PointerEvent): void {
    const d = drag;
    if (!d) return;
    const p = at(e);
    const moved = d.moved || Math.abs(p.x - d.downX) >= still();
    const to = tickAt(p.x, e.shiftKey);
    if (moved !== d.moved || to !== d.to) drag = { ...d, to, moved };
  }
  function up(): void {
    const d = drag;
    if (!d) return;
    drag = null;
    if (!d.moved) return;
    const pieces = C.draggedPieces(clip, d);
    if (C.samePieces(pieces, now)) return;
    const made = { clip: clip.id, pieces, rev: null as number | null };
    pending = made;
    void send({ type: "setComp", clip: clip.id, span: { start: clip.start, end: clip.end }, pieces }).then((rev) => {
      if (pending === made) pending = { ...made, rev: rev ?? -Infinity };
    });
  }
  function cancel(): boolean {
    if (!drag) return false;
    drag = null;
    return true;
  }
  onMount(() => onDragCancel(cancel));

  const clear = () => send({ type: "clearComp", clip: clip.id });
  const canClear = $derived(!busy && clip.comp.length > 0);
</script>

<div
  class="comp-band"
  role="presentation"
  bind:this={node}
  style:height="{C.bandHeight(dims, band.takes.length)}px"
  use:glSlot={{ kind: "rects", rects: picture.rects, waves: picture.waves }}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={cancel}
  oncontextmenu={(e) => (e.preventDefault(), e.stopPropagation())}
  ondblclick={(e) => e.stopPropagation()}
  data-test="comp-band"
>
  <div class="heading">
    <span class="m-lbl name" data-test="comp-clip">{t("comp.clipTake", { clip: clip.name, take: clip.take })}</span>
    <span class="m-spacer"></span>
    <button
      class="m-btn m-focus-in clear"
      aria-disabled={!canClear || undefined}
      data-test="comp-clear"
      onpointerdown={(e) => e.stopPropagation()}
      onclick={() => canClear && clear()}>{t("comp.clear")}</button
    >
  </div>
</div>

<style>
  /* The open band: background gray.2, as an automation lane. */
  .comp-band {
    position: relative;
    box-sizing: border-box;
    background: var(--color-gray-2);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    touch-action: none;
  }
  .heading {
    height: var(--size-row);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-2);
    white-space: nowrap;
    overflow: hidden;
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .clear {
    flex-shrink: 0;
  }
</style>

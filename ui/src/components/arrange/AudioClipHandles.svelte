<!-- 歌のクリップのフェードの取っ手と音量の線（仕様書 2 章「歌のクリップ」、3 章 27-A 「切る ・ フェード ・ 重なり」と「クリップの音量」、モック F7-AudioEdit ・ R11-Audio）。
     取っ手: 上の左右の角に part.audioclip.handle（6×6）の gray.10。横にひっぱるとフェードの長さ。
     取っ手の絵は WebGL の側（Arrange の paintAudioClip が audioClipGeom.ts の handleRects で描く）。ここは、押した所がどの取っ手かを
     形から数える（クリップごとの HTML を置かない: 横に送る ・ 拡大するコマが、クリップの数だけ重くなる）。HTML は、ポインタの下の取っ手 1 つだけ（カーソルの形のため）。
     音量の線: えらんだ歌のクリップの中に accent の line.hair の横線。上下にドラッグして −60〜+12 dB、2 回押すと 0.0 dB。
     ドラッグの間は画面だけが先に動き、値を audioDrag.now に書く（波形を描く WebGL の側が、同じコマで読む）。離したら SetClipField を 1 つ（ドラッグ 1 回が 1 手）。
     もとの位置は gray.7 の枠（線）。Esc はキーの順のいちばん前でやめる（pianoroll/view.svelte.ts の cancelDrag）。
     音量の読み値「−3.0 dB」は、その場の読み値（カーソルの左上）と状態行（6 章「クリップの音量を動かしている」）。
     フェードの長さ「0. 1. 0480」も同じ（6 章「フェードを動かしている」。長さの書きかたはクリップの長さと同じ）。 -->
<script lang="ts">
  import { onDestroy, onMount, untrack } from "svelte";
  import type { ClipView, TrackView } from "../../bridge";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { clipBox, rowHeight, rowTop, type Box, type LaneDims, type View } from "../arrangeGeom";
  import { dbNumber } from "../format";
  import { clipLengthLabel } from "../params/clipFields";
  import { mapOf } from "../../time/bars";
  import { metrics } from "../metrics.svelte";
  import { dragReading, onDragCancel } from "../pianoroll/view.svelte";
  import { audioDrag, type AudioDragValue } from "./audioDrag.svelte";
  import { audioShapeOf, fadeHandleBox, fadeTicksAt, fadeXs, gainAtY, gainLineY, waveArea, type AudioClipShape, type FadeEdge } from "./audioClipGeom";

  let { view, dims, tracks, selected, busy }: { view: View; dims: LaneDims; tracks: readonly TrackView[]; selected: ReadonlySet<number>; busy: boolean } = $props();

  type Field = "gainDb" | FadeEdge;
  interface Drag {
    field: Field;
    clip: ClipView;
    row: number;
    pointer: number;
    /** ポインタをつかまえている物: 音量の線、ポインタの下の取っ手、どちらもなければクリップの面。 */
    el: HTMLElement;
    /** 押した所と、線（取っ手のまん中）とのずれ。 */
    grab: number;
    downX: number;
    downY: number;
    from: number;
    value: number;
    moved: boolean;
  }

  let root = $state<HTMLElement>();
  let drag = $state.raw<Drag | null>(null);
  /** 離して送ったあと、答えの状態が届くまで audioDrag.now を残す（ちらつかないように）。 */
  let pendingRev = $state<number | null>(null);
  let pointerAt = $state<{ x: number; y: number } | null>(null);
  let readingW = $state(0);
  let readingH = $state(0);

  const handle = $derived(metrics.px("part.audioclip.handle"));
  const still = () => metrics.px("space.1");

  $effect(() => {
    if (pendingRev !== null && current.rev >= pendingRev) {
      untrack(() => {
        pendingRev = null;
        if (!drag) audioDrag.now = null;
      });
    }
  });

  /** ドラッグ中の値をかぶせた形。 */
  function shapeOf(clip: ClipView, now: AudioDragValue | null): AudioClipShape & { gainDb: number } {
    const base = audioShapeOf(clip)!;
    const gainDb = clip.audio!.gainDb;
    if (!now || now.clip !== clip.id) return { ...base, gainDb };
    return { ...base, fadeIn: now.fadeIn ?? base.fadeIn, fadeOut: now.fadeOut ?? base.fadeOut, gainDb: now.gainDb ?? gainDb };
  }

  interface Part {
    clip: ClipView;
    row: number;
    box: Box;
    handles: { edge: FadeEdge; box: Box }[];
    line: { x0: number; x1: number; y: number } | null;
  }

  function partOf(clip: ClipView, row: number, now: AudioDragValue | null): Part | null {
    const box = clipBox(view, dims, row, clip.start, clip.end);
    if (box.x1 < 0 || box.x0 > view.width) return null;
    const shape = shapeOf(clip, now);
    const area = waveArea(dims, box);
    const handles = (["fadeIn", "fadeOut"] as const).map((edge) => ({ edge, box: fadeHandleBox(view, dims, box, shape, edge, handle) }));
    const line = selected.has(clip.id) ? { x0: Math.max(area.x0, 0), x1: Math.min(area.x1, view.width), y: gainLineY(area, shape.gainDb) } : null;
    return { clip, row, box, handles, line };
  }

  /** 音量の線が出るクリップ（えらんだ歌のクリップ）。 */
  const lines = $derived.by(() => {
    const now = audioDrag.now;
    const out: Part[] = [];
    tracks.forEach((track, row) => {
      for (const clip of track.clips) {
        if (!selected.has(clip.id) || clip.audio === null || clip.audio.state === "missing") continue;
        const p = partOf(clip, row, now);
        if (p?.line) out.push(p);
      }
    });
    return out;
  });

  /** (x, y) の下の取っ手。重なっていれば、上に描かれている物（あとのトラック、あとのクリップ、フェードアウト）。 */
  function handleAt(p: { x: number; y: number }): { part: Part; edge: FadeEdge } | null {
    const now = audioDrag.now;
    for (let row = tracks.length - 1; row >= 0; row--) {
      const top = rowTop(dims, row);
      if (p.y < top || p.y >= top + rowHeight(dims, row)) continue;
      const clips = tracks[row]!.clips;
      for (let i = clips.length - 1; i >= 0; i--) {
        const clip = clips[i]!;
        if (clip.audio === null || clip.audio.state === "missing") continue;
        const part = partOf(clip, row, now);
        if (!part) continue;
        for (let h = part.handles.length - 1; h >= 0; h--) {
          const b = part.handles[h]!.box;
          if (p.x >= b.x0 && p.x < b.x1 && p.y >= b.y0 && p.y < b.y1) return { part, edge: part.handles[h]!.edge };
        }
      }
    }
    return null;
  }

  /** ポインタの下の取っ手（ドラッグの間は、ひっぱっている取っ手）。HTML はこの 1 つだけ。 */
  let hover = $state.raw<{ clip: number; edge: FadeEdge } | null>(null);
  let grip = $state<HTMLElement>();
  const gripBox = $derived.by(() => {
    const h = drag && drag.field !== "gainDb" ? { clip: drag.clip.id, edge: drag.field } : hover;
    if (!h) return null;
    for (let row = 0; row < tracks.length; row++) {
      const clip = tracks[row]!.clips.find((c) => c.id === h.clip);
      if (!clip || clip.audio === null) continue;
      return partOf(clip, row, audioDrag.now)?.handles.find((x) => x.edge === h.edge)?.box ?? null;
    }
    return null;
  });

  /** ドラッグを始めた所（もとの位置の gray.7）。 */
  const origin = $derived.by(() => {
    const d = drag;
    if (!d || !d.moved) return null;
    const p = partOf(d.clip, d.row, null);
    if (!p) return null;
    if (d.field === "gainDb") return { kind: "line" as const, line: p.line ?? { x0: Math.max(p.box.x0, 0), x1: p.box.x1, y: gainLineY(waveArea(dims, p.box), d.from) } };
    return { kind: "handle" as const, box: p.handles.find((h) => h.edge === d.field)!.box };
  });

  function point(e: PointerEvent | MouseEvent): { x: number; y: number } {
    const r = root!.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  }

  function down(e: PointerEvent, part: Part, field: Field, el: HTMLElement): void {
    if (e.button !== 0 || busy || !root) return;
    e.stopPropagation();
    e.preventDefault();
    const p = point(e);
    const audio = part.clip.audio!;
    const from = field === "gainDb" ? audio.gainDb : audio[field];
    let grab: number;
    if (field === "gainDb") grab = p.y - (part.line?.y ?? p.y);
    else {
      const xs = fadeXs(view, audioShapeOf(part.clip)!);
      grab = p.x - (field === "fadeIn" ? xs.inX : xs.outX);
    }
    el.setPointerCapture?.(e.pointerId);
    drag = { field, clip: part.clip, row: part.row, pointer: e.pointerId, el, grab, downX: p.x, downY: p.y, from, value: from, moved: false };
  }

  /** その場の読み値: 音量は「−3.0 dB」、フェードは長さ「0. 1. 0480」（クリップのはじまりの拍子で）。 */
  function valueText(field: Field, clip: ClipView, value: number): string {
    if (field === "gainDb") return t("common.db", { value: dbNumber(value) });
    return clipLengthLabel(value, mapOf(current.state!.timeline).meterAt(clip.start));
  }

  function readingText(field: Field, clip: ClipView, value: number): string {
    const values = { clip: clip.name, value: valueText(field, clip, value), cancelKey: t("key.esc") };
    if (field === "gainDb") return t("msg.clipGainDragging", values);
    return t("msg.fadeDragging", { ...values, edge: t(field === "fadeIn" ? "clip.fadeEdgeIn" : "clip.fadeEdgeOut") });
  }

  function move(e: PointerEvent): void {
    const d = drag;
    if (!d || e.pointerId !== d.pointer) return;
    const p = point(e);
    const moved = d.moved || Math.hypot(p.x - d.downX, p.y - d.downY) >= still();
    if (!moved) return;
    let value: number;
    if (d.field === "gainDb") {
      const area = waveArea(dims, clipBox(view, dims, d.row, d.clip.start, d.clip.end));
      value = gainAtY(area, p.y - d.grab);
    } else {
      value = fadeTicksAt(view, audioShapeOf(d.clip)!, d.field, p.x - d.grab);
    }
    pointerAt = p;
    dragReading.text = readingText(d.field, d.clip, value);
    drag = { ...d, moved, value };
    audioDrag.now = { clip: d.clip.id, [d.field]: value };
  }

  function release(d: Drag): void {
    if (d.el.hasPointerCapture?.(d.pointer)) d.el.releasePointerCapture(d.pointer);
    drag = null;
    pointerAt = null;
    dragReading.text = null;
  }

  function up(e: PointerEvent): void {
    const d = drag;
    if (!d || e.pointerId !== d.pointer) return;
    release(d);
    if (!d.moved || d.value === d.from) {
      audioDrag.now = null;
      return;
    }
    pendingRev = Infinity;
    void send({ type: "setClipField", clip: d.clip.id, field: d.field, value: d.value }).then((rev) => (pendingRev = rev ?? -Infinity));
  }

  function cancel(): boolean {
    const d = drag;
    if (!d) return false;
    release(d);
    audioDrag.now = null;
    return true;
  }
  onMount(() => onDragCancel(cancel));
  onDestroy(cancel);

  // ---- 取っ手: クリップの面（この部品の親）に来た出来事を、先に見る ----

  /** 取っ手をひっぱっている間の出来事か。 */
  const handleDrag = (e: PointerEvent) => drag !== null && drag.field !== "gainDb" && e.pointerId === drag.pointer;
  /** 取っ手より上にある物（テイクの切りかえの札）の上では、取っ手を押した事にしない。 */
  const over = (e: Event) => e.target instanceof Element && e.target.closest("[data-over-clips]") !== null;

  function lanesDown(e: PointerEvent): void {
    if (e.button !== 0 || busy || drag || over(e)) return;
    const hit = handleAt(point(e));
    if (!hit) return;
    hover = { clip: hit.part.clip.id, edge: hit.edge };
    down(e, hit.part, hit.edge, e.target === grip && grip ? grip : (e.currentTarget as HTMLElement));
  }
  function lanesMove(e: PointerEvent): void {
    if (handleDrag(e)) {
      e.stopPropagation();
      move(e);
      return;
    }
    if (drag || e.buttons !== 0) return;
    const hit = busy || over(e) ? null : handleAt(point(e));
    if (hit?.part.clip.id !== hover?.clip || hit?.edge !== hover?.edge) hover = hit ? { clip: hit.part.clip.id, edge: hit.edge } : null;
  }
  function lanesUp(e: PointerEvent): void {
    if (!handleDrag(e)) return;
    e.stopPropagation();
    up(e);
  }
  function lanesCancel(e: PointerEvent): void {
    if (!handleDrag(e)) return;
    e.stopPropagation();
    cancel();
  }
  function lanesDouble(e: MouseEvent): void {
    if (!over(e) && handleAt(point(e))) e.stopPropagation();
  }
  onMount(() => {
    const lanes = root!.parentElement!;
    const leave = () => {
      if (!drag) hover = null;
    };
    const on = <K extends keyof HTMLElementEventMap>(type: K, f: (e: HTMLElementEventMap[K]) => void) => {
      lanes.addEventListener(type, f, { capture: true });
      return () => lanes.removeEventListener(type, f, { capture: true });
    };
    const off = [on("pointerdown", lanesDown), on("pointermove", lanesMove), on("pointerup", lanesUp), on("pointercancel", lanesCancel), on("dblclick", lanesDouble)];
    // クリップの面から出たときだけ（中の部品から出たときの知らせは見ない）。
    lanes.addEventListener("pointerleave", leave);
    return () => {
      off.forEach((f) => f());
      lanes.removeEventListener("pointerleave", leave);
    };
  });

  /** 2 回押すと 0.0 dB（仕様書 3 章「クリップの音量」）。 */
  function double(e: MouseEvent, part: Part): void {
    e.stopPropagation();
    if (busy || part.clip.audio!.gainDb === 0) return;
    void send({ type: "setClipField", clip: part.clip.id, field: "gainDb", value: 0 });
  }

  const reading = $derived.by(() => {
    const d = drag;
    if (!d || !d.moved || !pointerAt) return null;
    const off = metrics.px("part.reading.offset");
    return { text: valueText(d.field, d.clip, d.value), left: Math.max(0, pointerAt.x - off - readingW), top: Math.max(0, pointerAt.y - off - readingH) };
  });
</script>

<div class="host" class:busy bind:this={root}>
  {#if origin?.kind === "line"}
    <div class="origin-line" style:left="{origin.line.x0}px" style:width="{origin.line.x1 - origin.line.x0}px" style:top="{origin.line.y}px"></div>
  {:else if origin?.kind === "handle"}
    <div
      class="origin-handle"
      style:left="{origin.box.x0}px"
      style:top="{origin.box.y0}px"
      style:width="{origin.box.x1 - origin.box.x0}px"
      style:height="{origin.box.y1 - origin.box.y0}px"
    ></div>
  {/if}
  {#each lines as part (part.clip.id)}
    {#if part.line}
      {@const line = part.line}
      <div
        class="gain-hit"
        role="slider"
        tabindex="-1"
        aria-label={t("clipParams.gain")}
        aria-valuemin={-60}
        aria-valuemax={12}
        aria-valuenow={part.clip.audio!.gainDb}
        data-test="clip-gain-line"
        style:left="{line.x0}px"
        style:width="{Math.max(0, line.x1 - line.x0)}px"
        style:top="{line.y - handle / 2}px"
        style:height="{handle}px"
        onpointerdown={(e) => down(e, part, "gainDb", e.currentTarget)}
        onpointermove={move}
        onpointerup={up}
        onpointercancel={cancel}
        ondblclick={(e) => double(e, part)}
      >
        <div class="gain-line" style:top="{handle / 2}px"></div>
      </div>
    {/if}
  {/each}
  {#if gripBox}
    <div
      class="grip"
      data-test="clip-fade-grip"
      bind:this={grip}
      style:left="{gripBox.x0}px"
      style:top="{gripBox.y0}px"
      style:width="{gripBox.x1 - gripBox.x0}px"
      style:height="{gripBox.y1 - gripBox.y0}px"
    ></div>
  {/if}
  {#if reading}
    <div
      class="m-num reading"
      style:left="{reading.left}px"
      style:top="{reading.top}px"
      bind:clientWidth={readingW}
      bind:clientHeight={readingH}
      data-test="clip-gain-reading"
    >
      {reading.text}
    </div>
  {/if}
</div>

<style>
  .host {
    position: absolute;
    inset: 0;
    z-index: 4;
    pointer-events: none;
  }
  .grip,
  .gain-hit {
    position: absolute;
    pointer-events: auto;
  }
  .host.busy .grip,
  .host.busy .gain-hit {
    pointer-events: none;
  }
  .grip {
    cursor: ew-resize;
  }
  .gain-hit {
    cursor: ns-resize;
    outline: none;
  }
  .gain-line {
    position: absolute;
    left: 0;
    right: 0;
    height: var(--line-hair);
    background: var(--color-accent);
  }
  .origin-line {
    position: absolute;
    height: var(--line-hair);
    background: var(--color-gray-7);
  }
  .origin-handle {
    position: absolute;
    box-sizing: border-box;
    border: var(--line-hair) solid var(--color-gray-7);
  }
  .reading {
    position: absolute;
    z-index: 10;
    height: var(--part-reading-height);
    padding: 0 var(--part-reading-pad-x);
    display: flex;
    align-items: center;
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-8);
    color: var(--color-gray-10);
    white-space: nowrap;
    pointer-events: none;
  }
</style>

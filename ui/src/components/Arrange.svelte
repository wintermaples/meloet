<!-- The arrange (mock Main, F-ArrangeRec, F-Takes, F-Empty). When shrunk, the overview (the upper band of A-PianoRoll).
     Bar lines, clips and overview lines are WebGL. Names, tags and the playhead line are HTML. children are things laid over the arrange (output choice, hints).
     While playing, the lines and the tip of the recording clip move on every frame of the WebGL side (everyGlFrame).
     A clip drag (mock R3-Arrange) leaves the original place on the big face with a gray.7 border and draws the moving clips in a small slot (recordingClip;
     there is no drag while recording, so it is never used at the same time as the tip of a recording clip). A plain move only moves the box with transform.
     Any number of selected clips move together (docs/history/design-notes.ja.md B-71). The context menu is B-60. -->
<script lang="ts">
  import { onDestroy, onMount, tick as nextTick, untrack, type Snippet } from "svelte";
  import {
    clockOffsetUs,
    liveNotes,
    peekTakeNotes,
    takeNotes,
    type ClipView,
    type LiveNote,
    type LiveNotesBook,
    type RecordingView,
    type TakeNotes,
    type TrackView,
    type VocalRecordingView,
  } from "../bridge";
  import { WAVE } from "../gl/dims";
  import { everyGlFrame, glRegion, glSlot, type GlRect, type GlSlot, type GlWave } from "../gl/slot";
  import { handleRects, waveArea } from "./arrange/audioClipGeom";
  import { clipMarks, NO_MARKS } from "./params/clipFields";
  import { audioDrag, type AudioDragValue } from "./arrange/audioDrag.svelte";
  import * as W from "./arrange/wave";
  import { WaveBook } from "./arrange/wavePeaks.svelte";
  import WriteLine from "./automation/WriteLine.svelte";
  import { platformOf } from "../keys/keys";
  import HScrollbar from "../lib/HScrollbar.svelte";
  import TrackHeads from "./TrackHeads.svelte";
  import VScrollbar from "./mixer/VScrollbar.svelte";
  import { term } from "../lib/tip.svelte";
  import { openContextMenu, type MenuEntry, type MenuRow } from "./menu/contextMenu.svelte";
  import { joinable, overdubClipName, splitTick } from "./clipEdits";
  import { clipEdit, renameCommand, startClipRename, stopClipRename } from "./clipName.svelte";
  import { current, send } from "../state/app.svelte";
  import { t } from "../text/ja";
  import * as CD from "./arrangeDrag";
  import * as G from "./arrangeGeom";
  import { gridText, tempoText, termParts, textParts } from "./format";
  import { metrics } from "./metrics.svelte";
  import {
    arrangeView,
    DEFAULT_ARRANGE_BAR,
    dragReading,
    onDragCancel,
    prView,
    scrollToTick,
    visibleTicks,
    zoomArrangeBy,
  } from "./pianoroll/view.svelte";
  import { shownTick, ticksPerBar, ticksPerBeat } from "./time";
  import { barLineTick, eachLine, mapOf, nearestBarTick, snapToStep } from "../time/bars";
  import type { GridLine } from "../gl/slot";
  import MarkerRow from "./timeline/MarkerRow.svelte";
  import TempoBand from "./timeline/TempoBand.svelte";
  import { changeCount, summaries } from "./timeline/tempoGeom";
  import { pickClips } from "./timeline/pick";
  import { arrangeRows, showsAddRow } from "./automation/autoGeom";
  import AutoLane from "./automation/AutoLane.svelte";
  import AudioClipHandles from "./arrange/AudioClipHandles.svelte";
  import AudioClipNotes from "./arrange/AudioClipNotes.svelte";
  import DropFrame from "./arrange/DropFrame.svelte";

  let { children }: { children?: Snippet } = $props();

  const song = $derived(current.state!.song);
  const timeline = $derived(current.state!.timeline);
  const transport = $derived(current.state!.transport);
  /** The clip the take switch tag points to (the last pressed clip). */
  const selected = $derived(current.state!.arrange.selectedClip);
  /** All selected clips (spec 2 "Selected clip": accent border and top edge). */
  const selectedSet = $derived(new Set(current.state!.arrange.selectedClips));
  const layout = $derived(current.state!.workspace.layout);
  /** Whether keys go to the arrange (B-72; always the arrange when the piano roll is not open). */
  const keysHere = $derived(prView.focus === "arrange" || !layout.lower.some((l) => l.band === "pianoRoll"));
  const height = $derived(metrics.scaled(layout.arrangeHeight));

  let timeWidth = $state(0);
  let overviewWidth = $state(0);
  let overviewHeight = $state(0);
  /** Horizontal scroll. Kept in ticks so the same place stays in view when the zoom changes. */
  let scrollTick = $state(0);
  /** Whether to page along with the playhead. Stops when moved sideways by hand, and follows again on the next play. */
  let follow = true;

  /** px per bar. The horizontal zoom step (arrangeView.barPx is px at 100 %) times the same scale as mock.arrange.bar. */
  const barPx = $derived((metrics.px("mock.arrange.bar") * arrangeView.barPx) / DEFAULT_ARRANGE_BAR);
  /** Every how many bars a bar number is shown. Thinned when zoomed out so they would overlap. */
  const barLabelEvery = $derived(G.labelEvery(barPx, metrics.px("mock.arrange.bar") / 2));
  /** The tempo and time signature map. Bar lines, ruler numbers, snapping and the loop range come from here (time signature changes alter bar length). */
  const map = $derived(mapOf(timeline));
  /** One bar in the time signature at the song start. The horizontal zoom step (barPx) is px for a bar of this length. */
  const perBar = $derived(ticksPerBar({ num: timeline.meterNum, den: timeline.meterDen }));
  const pxPerTick = $derived(barPx / perBar);
  /** Whether to draw beat lines in the clip area too. Only bar lines unless zoomed in enough to snap to beats. */
  const gridBeats = $derived(G.beatsShown(barPx, perBar, ticksPerBeat({ num: timeline.meterNum, den: timeline.meterDen }), metrics.px("mock.arrange.bar") / 2));
  const viewWidth = $derived(layout.arrangeOverview ? overviewWidth : timeWidth);
  const view: G.View = $derived({ pxPerTick, scrollX: scrollTick * pxPerTick, width: viewWidth });
  /** Track rows and the automation bands under them (the same order as the head side; automation/autoGeom.ts). */
  const rows = $derived(
    arrangeRows(song.tracks, current.state!.automation.folded, {
      trackH: metrics.px("size.track"),
      laneH: metrics.px("size.lane"),
      rowH: metrics.px("size.row"),
      tall: { track: current.state!.arrange.tallTrack, h: metrics.px("part.track.vocal") },
    }),
  );
  const dims: G.LaneDims = $derived({
    tops: rows.tops,
    heights: rows.heights,
    trackH: metrics.px("size.track"),
    inset: metrics.px("mock.arrange.clip-inset"),
    hair: metrics.px("line.hair"),
    mark: metrics.px("line.mark"),
    nameH: metrics.px("text.s.line-height"),
    pad: metrics.px("space.1"),
    noteH: metrics.px("mock.arrange.note"),
  });
  const tagMin = $derived(metrics.px("part.take.tag-min-width"));
  const switchH = $derived(metrics.px("part.take.switch-height"));

  /** Ruler numbers (bar number and the x of its line from the song start). The whole number strip moves with transform, so scrolling does not change these x. */
  const barNumbers = $derived.by(() => {
    const out: { n: number; x: number }[] = [];
    eachLine(map, map.meterAt(G.xToTick(view, 0)).barTick, G.xToTick(view, viewWidth), (_, bar) => bar * barLabelEvery, (tick, _, bar) =>
      out.push({ n: bar + 1, x: tick * pxPerTick }),
    );
    return out;
  });
  /** Bar lines (and beat lines) in the clip area. */
  const gridLines = $derived.by(() => {
    const out: GridLine[] = [];
    eachLine(map, G.xToTick(view, 0), G.xToTick(view, viewWidth), (beat, bar) => (gridBeats ? beat : bar), (tick, kind) =>
      out.push({ x: G.tickToX(view, tick), bar: kind === 0 }),
    );
    return out;
  });
  /** The row in the order, from a track id. */
  const rowOf = (track: number) => song.tracks.findIndex((tr) => tr.id === track);

  // ---- Take notes (once fetched, they can be kept by id) ----

  let takeVersion = $state(0);
  const asked = new Set<number>();
  function takeOf(id: number): TakeNotes | undefined {
    const had = peekTakeNotes(id);
    if (had || asked.has(id)) return had;
    asked.add(id);
    try {
      // Remove from asked when it arrives (so it can be fetched again if it is cleaned up later and needed).
      takeNotes(id).then(
        () => {
          asked.delete(id);
          takeVersion += 1;
        },
        () => asked.delete(id),
      );
    } catch {
      asked.delete(id);
    }
    return undefined;
  }

  // ---- Audio clip waveforms (arrange/wave.ts; the bridge keeps the peaks and the WebGL side makes them textures) ----

  const waveBook = new WaveBook();
  onDestroy(() => waveBook.stop());
  // Every time the song changes, also drop from the book the peaks the bridge let go of.
  $effect(() => {
    void song;
    waveBook.trim(song.serial);
  });
  const waveSizes = $derived({
    pitch: metrics.px("mock.wave.pitch"),
    bar: metrics.px("mock.wave.bar"),
    minH: metrics.px("mock.wave.min"),
  });
  const tallTrack = $derived(current.state!.arrange.tallTrack);
  const handleSize = $derived(metrics.px("part.audioclip.handle"));

  /**
   * One audio clip: border and face, with the waveform and fade lines on top. `drag` is the value while the volume line or a handle is dragged.
   * A clip whose sound file is not found: background gray.2, top line gray.7, no waveform (spec 3 (93-3)).
   * Fade handles (gray.10 at the left and right corners) go into `handles`: the caller draws them above all clips (so an overlapping clip does not hide them).
   */
  function paintAudioClip(rects: GlRect[], waves: GlWave[], handles: GlRect[], v: G.View, box: G.Box, track: TrackView, clip: ClipView, colors: G.ClipColors, drag: AudioDragValue | null): void {
    if (paintAudioFrame(rects, handles, v, box, track, clip, colors, drag)) paintAudioWaves(waves, rects.length, v, box, track, clip, colors, drag);
  }

  /** The border and fade handles of an audio clip. true if this clip also draws a waveform. */
  function paintAudioFrame(rects: GlRect[], handles: GlRect[], v: G.View, box: G.Box, track: TrackView, clip: ClipView, colors: G.ClipColors, drag: AudioDragValue | null): boolean {
    if (clip.audio!.state === "missing") {
      const missing = colors === G.CLIP_SELECTED ? colors : { ...colors, top: WAVE.missingTop };
      G.paintClip(rects, v, dims, box, missing, [], { range: null, ground: WAVE.missingGround });
      return false;
    }
    G.paintClip(rects, v, dims, box, G.clipColorsFor(colors, track.muted), [], { range: null });
    handleRects(handles, v, dims, box, W.waveClipOf(clip, drag)!, handleSize);
    return true;
  }

  /** The waveform of an audio clip. `after` is the number of rectangles under the waveform. */
  function paintAudioWaves(waves: GlWave[], after: number, v: G.View, box: G.Box, track: TrackView, clip: ClipView, colors: G.ClipColors, drag: AudioDragValue | null): void {
    const a = clip.audio!;
    const shown = G.clipColorsFor(colors, track.muted);
    const shape = W.waveClipOf(clip, drag)!;
    const level = W.waveLevel(W.framesPerPx(map, v, shape.timing, clip.start, clip.end), waveSizes.pitch);
    const found = waveBook.sourceFor(song.serial, a.audio, level);
    if (!found) return;
    W.clipWaves(waves, after, map, v, waveArea(dims, box), shape, found.source, found.framesPerPeak, W.waveLook(waveSizes, shown.note, track.id === tallTrack));
  }

  /** The peaks of the audio being recorded (they grow; the tip is read every frame, so growth does not ask for a redraw). None yet during count-in. */
  function recSource(r: VocalRecordingView, v: G.View, from: number, to: number) {
    if (r.firstFrame === null) return null;
    const timing: W.WaveTiming = { start: r.from, takeOffset: 0, firstFrame: r.firstFrame, sampleRate: r.sampleRate, source: null };
    const found = waveBook.sourceFor(song.serial, r.audio, W.waveLevel(W.framesPerPx(map, v, timing, from, to), waveSizes.pitch), true);
    return found ? { ...found, timing } : null;
  }

  /** The waveform of [from, to] of the audio clip being recorded (rec color). `box` is the box of that part. */
  function recWaves(waves: GlWave[], after: number, v: G.View, box: G.Box, r: VocalRecordingView, from: number, to: number, openLeft: boolean): void {
    const found = recSource(r, v, from, to);
    if (!found) return;
    const area = waveArea(dims, box);
    const shape: W.WaveClip = { timing: found.timing, start: r.from, end: to, fadeIn: 0, fadeOut: 0, gain: 1, endFrame: Infinity };
    const look = W.waveLook(waveSizes, G.CLIP_RECORDING.note, r.track === tallTrack);
    W.clipWaves(waves, after, map, v, { ...area, x0: openLeft ? box.x0 : area.x0, x1: box.x1 }, shape, found.source, found.framesPerPeak, look, { from, to });
  }

  // ---- Notes being recorded ----

  let book = $state.raw<LiveNotesBook | null>(null);
  let stopBook: (() => void) | null = null;
  let liveVersion = $state(0);
  function ensureBook(): LiveNotesBook | null {
    if (book) return book;
    try {
      const found = liveNotes();
      stopBook = found.listen(() => (liveVersion += 1));
      book = found;
    } catch {
      // Not connected yet. Try again with the next state.
    }
    return book;
  }
  $effect(() => {
    void current.state;
    ensureBook();
  });
  onDestroy(() => stopBook?.());

  const rec = $derived(transport.state === "recording" ? transport.recording : null);
  /** The pass going on overdubs a clip: that clip's name (mock P10-Overdub). */
  const overdubName = $derived(rec ? overdubClipName(song.tracks, rec) : null);
  /**
   * Vocal tracks being recorded (spec 3 (24-C): every clip being recorded is rec colored; mock F7-VocalRec, F7-TwoRec).
   * The waveform is in protocol 24, so for now it is a rec clip without content. Like keyboards, finished bars go on the big face and the growing tip (up to 2 bars) on recordingClip.
   */
  const recVocals = $derived(transport.state === "recording" ? transport.recordingVocals : []);
  /** How far the audio clip has been moved to the big face (bar by bar). */
  let vocalCommit = $state(0);
  /** The lap being drawn now, and how far it has been moved to the big face (bar by bar). */
  let recShown = $state.raw<{ lap: number; lapFrom: number; commit: number } | null>(null);
  let lastLap = 0;
  let lastNow = 0;
  /** The last notes being recorded, shown until the take arrives after recording ends. */
  let fallback = $state.raw<{ track: number; notes: G.NoteShape[] } | null>(null);
  let prevRecording: RecordingView | null = null;

  $effect(() => {
    const r = rec;
    untrack(() => {
      if (r && (!recShown || prevRecording?.id !== r.id)) {
        const lapFrom = G.lapStart(r.from, r.lap, song.loopRange);
        recShown = { lap: r.lap, lapFrom, commit: lapFrom };
        lastLap = r.lap;
        lastNow = lapFrom;
        fallback = null;
      }
      if (!r && prevRecording) {
        const b = ensureBook();
        let notes: G.NoteShape[] = [];
        for (let lap = lastLap; lap >= 0 && notes.length === 0; lap--) {
          notes = G.liveShapes(b?.notesOf(prevRecording.id, lap) ?? [], lastNow);
        }
        fallback = notes.length > 0 ? { track: prevRecording.track, notes } : null;
        recShown = null;
      }
      prevRecording = r;
    });
  });

  function shapesOf(trackId: number, clip: ClipView): G.NoteShape[] {
    // The take of an audio clip is sound. It is not in take_notes.
    if (clip.audio !== null) return [];
    const take = takeOf(clip.takeId);
    if (take) return G.takeShapes(clip, take);
    if (fallback?.track !== trackId) return [];
    return fallback.notes.filter((n) => n.start < clip.end && n.start + n.length > clip.start);
  }

  /** The note shapes of a clip and their height range. `longest` is the longest note (-1: not in start order). */
  interface LaneShapes {
    notes: G.NoteShape[];
    range: { low: number; high: number } | null;
    longest: number;
  }
  /** Takes never change, so shapes are kept per take and clip place (not remade on every zoom). */
  const laneShapeBook = new WeakMap<TakeNotes, Map<string, LaneShapes>>();
  function laneShapesOf(trackId: number, clip: ClipView): LaneShapes {
    const take = clip.audio === null ? takeOf(clip.takeId) : undefined;
    if (!take) {
      const notes = shapesOf(trackId, clip);
      return { notes, range: G.keyRange(notes), longest: -1 };
    }
    let byPlace = laneShapeBook.get(take);
    if (!byPlace) laneShapeBook.set(take, (byPlace = new Map()));
    const key = `${clip.start} ${clip.end} ${clip.takeOffset}`;
    let found = byPlace.get(key);
    if (!found) {
      const notes = G.takeShapes(clip, take);
      found = { notes, range: G.keyRange(notes), longest: notes.reduce((m, n) => Math.max(m, n.length), 0) };
      byPlace.set(key, found);
    }
    return found;
  }
  /** From the shapes in start order, only the ones touching ticks [from, to). */
  function notesNear(shapes: LaneShapes, from: number, to: number): G.NoteShape[] {
    const first = (tick: number) => {
      let lo = 0;
      let hi = shapes.notes.length;
      while (lo < hi) {
        const mid = (lo + hi) >> 1;
        if (shapes.notes[mid]!.start < tick) lo = mid + 1;
        else hi = mid;
      }
      return lo;
    };
    return shapes.notes.slice(first(from - shapes.longest), first(to));
  }

  // ---- Loop ----

  const songLoop = $derived(song.loopOn ? G.loopBars(song.loopRange, map) : null);
  /** The look during a drag. Cleared when the Rust answer (the next diff) comes. */
  let preview = $state<G.BarRange | null>(null);
  const shownLoop = $derived(preview ?? songLoop);
  $effect(() => {
    void current.rev;
    untrack(() => {
      if (!drag) preview = null;
    });
  });
  const loopX = (bar: number) => G.tickToX(view, barLineTick(map, bar));

  // ---- Clip drag (move, copy with Alt, trim an edge; arrangeDrag.ts) ----

  let clipDrag = $state.raw<CD.ClipDrag | null>(null);
  /** The look kept after release and sending the intent, until the answer state arrives (so it does not flicker). */
  let clipPending = $state.raw<CD.ClipDrag | null>(null);
  let pendingRev = $state<number | null>(null);
  const shownDrag = $derived.by(() => {
    const d = clipDrag ?? clipPending;
    return d && d.press.moved ? d : null;
  });
  /**
   * The big face only looks at which clips, and whether the original place gets a border (unless copying). It is not redrawn for the move amount.
   * Which clips is kept as a string (if it is the same, neither the Set below nor the big face is remade).
   */
  const dragKey = $derived.by(() => {
    const d = shownDrag;
    if (!d) return "";
    return d.kind === "trim" ? String(d.clip.id) : d.group.map((g) => g.clip.id).join(",");
  });
  const dragIds = $derived(new Set(dragKey ? dragKey.split(",").map(Number) : []));
  const dragFrame = $derived(shownDrag !== null && (shownDrag.kind === "trim" || !shownDrag.copy));
  /** Changes only when the shape of the moving picture changes (not for a plain move). */
  const dragShape = $derived.by(() => {
    const d = shownDrag;
    if (!d) return null;
    return d.kind === "trim" ? `t${d.clip.id}:${d.start}:${d.end}` : `m${dragKey}`;
  });
  const dragShift = $derived.by(() => {
    const d = shownDrag;
    if (!d || d.kind !== "move" || (d.ticks === 0 && d.toRow === d.row)) return null;
    return `translate(${d.ticks * pxPerTick}px, ${G.rowTop(dims, d.toRow) - G.rowTop(dims, d.row)}px)`;
  });

  $effect(() => {
    if (clipPending && pendingRev !== null && current.rev >= pendingRev) clipPending = null;
  });

  /** The moving picture: the small slot box (lane CSS px), the rectangles in it, the names. */
  const dragPicture = $derived.by(() => {
    void dragShape;
    void takeVersion;
    const d = untrack(() => shownDrag);
    if (!d) return null;
    const rects: GlRect[] = [];
    const waves: GlWave[] = [];
    const handles: GlRect[] = [];
    const labels: Label[] = [];
    void waveBook.version;
    const notesOf = (row: number, clip: ClipView) => {
      const take = takeOf(clip.takeId);
      return take ? G.takeShapes(clip, take) : shapesOf(song.tracks[row]?.id ?? -1, clip);
    };
    const labelOf = (clip: ClipView, box: G.Box): Label => ({
      id: clip.id,
      x: Math.max(box.x0, 0) + dims.hair,
      y: box.y0 + dims.mark,
      w: box.x1 - Math.max(box.x0, 0) - 2 * dims.hair,
      name: clip.name,
      mark: G.takeMark(clip, box.x1 - box.x0, tagMin),
      take: clip.take,
      takes: clip.takes,
      selected: true,
      marks: clip.audio ? clipMarks(clip.audio.stretch) : NO_MARKS,
    });
    let place: { left: number; top: number; width: number; height: number };
    if (d.kind === "move") {
      // Draw all the clips with the start of the front clip and the top row at 0 (including parts that come into view as it moves).
      const first = Math.min(...d.group.map((g) => g.clip.start));
      const last = Math.max(...d.group.map((g) => g.clip.end));
      const top = G.clipBox(view, dims, Math.min(...d.group.map((g) => g.row)), first, last).y0;
      const bottom = G.clipBox(view, dims, Math.max(...d.group.map((g) => g.row)), first, last).y1;
      const local: G.View = { pxPerTick, scrollX: first * pxPerTick, width: Infinity };
      for (const { clip, row } of d.group) {
        const inner = G.clipBox(local, dims, row, clip.start, clip.end);
        const colors = G.clipColorsFor(G.CLIP_SELECTED, song.tracks[row]?.muted ?? false);
        const moved = { ...inner, y0: inner.y0 - top, y1: inner.y1 - top };
        if (clip.audio && song.tracks[row]) paintAudioClip(rects, waves, handles, local, moved, song.tracks[row], clip, G.CLIP_SELECTED, null);
        else G.paintClip(rects, local, dims, moved, colors, notesOf(row, clip));
        labels.push(labelOf(clip, G.clipBox(view, dims, row, clip.start, clip.end)));
      }
      for (const h of handles) rects.push(h);
      place = { left: G.tickToX(view, first), top, width: (last - first) * pxPerTick, height: bottom - top };
    } else {
      // A clip being trimmed gets a box the full width of the row. Tick 0 of the take does not move (the notes stay at the same place in the song).
      const take = takeOf(d.clip.takeId);
      const clip = { start: d.start, end: d.end, takeOffset: d.clip.start + d.clip.takeOffset - d.start };
      const box = G.clipBox(view, dims, d.row, d.start, d.end);
      const notes = take ? G.takeShapes(clip, take) : notesOf(d.row, d.clip).filter((n) => n.start >= d.start && n.start < d.end);
      const track = song.tracks[d.row];
      if (d.clip.audio && track) paintAudioClip(rects, waves, handles, view, box, track, { ...d.clip, ...clip }, G.CLIP_SELECTED, null);
      else G.paintClip(rects, view, dims, box, G.clipColorsFor(G.CLIP_SELECTED, track?.muted ?? false), notes);
      for (const h of handles) rects.push(h);
      for (const q of rects) q.y -= box.y0;
      W.shiftWaves(waves, 0, -box.y0);
      place = { left: 0, top: box.y0, width: view.width, height: box.y1 - box.y0 };
      labels.push(labelOf(d.clip, box));
    }
    return { place, rects, waves, labels };
  });

  let dragSlot: { update(next: GlSlot): void } | null = null;
  function draggedClip(node: HTMLElement) {
    const slot = glSlot(node, { kind: "rects", rects: dragPicture?.rects ?? [], waves: dragPicture?.waves });
    dragSlot = slot;
    return {
      destroy() {
        slot.destroy();
        if (dragSlot === slot) dragSlot = null;
      },
    };
  }
  // When the box moves with transform, fit the slot canvas to the moved box again too (the content is the same, so no redraw).
  $effect(() => {
    const p = dragPicture;
    void dragShift;
    if (p) untrack(() => dragSlot?.update({ kind: "rects", rects: p.rects, waves: p.waves }));
  });

  // ---- Big face: clips, the finished part of the recording clip, loop ends ----

  /**
   * The rectangles of the big face are made as if looking through a window three visible widths wide (1 to the left, 2 to the right); the offset from the window's left edge is sent as the slot's shiftX.
   * They are not remade on every scroll, only when the left edge of the visible part moves by one width.
   * Waveform steps and cuts depend on the visible part, so `laneWaves` makes them on every scroll (one per audio clip).
   */
  const laneWindow = $derived(Math.floor((scrollTick * pxPerTick) / Math.max(1, viewWidth)));

  const lanePicture = $derived.by(() => {
    void takeVersion;
    void liveVersion;
    const from = (laneWindow - 1) * viewWidth;
    const v: G.View = { pxPerTick, scrollX: from, width: 3 * viewWidth };
    const out: GlRect[] = [];
    const handles: GlRect[] = [];
    const audio: { after: number; row: number; track: TrackView; clip: ClipView; colors: G.ClipColors }[] = [];
    const vocals: { after: number; row: number; rec: VocalRecordingView; commit: number }[] = [];
    song.tracks.forEach((track, row) => {
      for (const clip of track.clips) {
        const box = G.clipBox(v, dims, row, clip.start, clip.end);
        if (box.x1 < 0 || box.x0 > v.width) continue;
        const dragged = dragIds.has(clip.id);
        if (dragged && dragFrame) {
          G.paintFrame(out, v, dims, box, "color.gray.7");
          continue;
        }
        const colors = selectedSet.has(clip.id) && !dragged ? G.CLIP_SELECTED : G.CLIP_PLAIN(G.trackColor(track));
        if (clip.audio) {
          if (paintAudioFrame(out, handles, v, box, track, clip, colors, null)) audio.push({ after: out.length, row, track, clip, colors });
        } else {
          const shapes = laneShapesOf(track.id, clip);
          const notes = shapes.longest < 0 ? shapes.notes : notesNear(shapes, G.xToTick(v, -dims.hair), G.xToTick(v, v.width + dims.hair));
          G.paintClip(out, v, dims, box, G.clipColorsFor(colors, track.muted), notes, { range: shapes.range });
        }
      }
    });
    const shown = recShown;
    if (rec && shown && shown.commit > shown.lapFrom) {
      const row = rowOf(rec.track);
      const all = G.liveShapes(book?.notesOf(rec.id, shown.lap) ?? [], shown.commit);
      const box = G.clipBox(v, dims, row, shown.lapFrom, shown.commit);
      if (row >= 0) {
        G.paintClip(
          out,
          v,
          dims,
          box,
          G.CLIP_RECORDING,
          all.filter((n) => n.start < shown.commit),
          { openRight: true, range: G.keyRange(all), until: shown.commit },
        );
      }
    }
    for (const r of recVocals) {
      const row = rowOf(r.track);
      const commit = Math.max(r.from, vocalCommit);
      if (row < 0 || commit <= r.from) continue;
      G.paintClip(out, v, dims, G.clipBox(v, dims, row, r.from, commit), G.CLIP_RECORDING, [], { openRight: true, range: null });
      vocals.push({ after: out.length, row, rec: r, commit });
    }
    if (shownLoop) {
      const h = rows.height;
      for (const bar of [shownLoop.from, shownLoop.to]) {
        const x = G.tickToX(v, barLineTick(map, bar));
        if (x >= 0 && x < v.width) out.push({ x, y: 0, w: dims.hair, h, color: "color.gray.7" });
      }
    }
    for (const h of handles) out.push(h);
    return { from, rects: out, audio, vocals };
  });

  /** The waveforms of the big face (px from the window's left edge, like `lanePicture`). */
  const laneWaves = $derived.by(() => {
    void waveBook.version;
    const waves: GlWave[] = [];
    for (const a of lanePicture.audio) {
      const box = G.clipBox(view, dims, a.row, a.clip.start, a.clip.end);
      if (box.x1 < 0 || box.x0 > view.width) continue;
      paintAudioWaves(waves, a.after, view, box, a.track, a.clip, a.colors, null);
    }
    for (const r of lanePicture.vocals) recWaves(waves, r.after, view, G.clipBox(view, dims, r.row, r.rec.from, r.commit), r.rec, r.rec.from, r.commit, false);
    W.shiftWaves(waves, view.scrollX - lanePicture.from, 0);
    return waves;
  });

  // ---- Drag of the volume line and fade handles (audioDrag; the big face is not redrawn, only the moving clips are drawn over it in a small slot) ----

  /** After release, keep the picture of the drag value until the answer state arrives (when cancelled, only for that time). */
  const LIVE_HOLD_MS = 100;
  let liveDrag = $state.raw<AudioDragValue | null>(null);
  let liveTimer = 0;
  $effect(() => {
    const now = audioDrag.now;
    clearTimeout(liveTimer);
    if (now) liveDrag = now;
    else if (untrack(() => liveDrag)) liveTimer = window.setTimeout(() => (liveDrag = null), LIVE_HOLD_MS);
  });
  $effect(() => {
    void current.rev;
    if (!untrack(() => audioDrag.now)) liveDrag = null;
  });
  onDestroy(() => clearTimeout(liveTimer));

  /**
   * Draw the dragged clip and the clips whose volume shifts with it (d.shift) in one slot that holds them all.
   * With one slot, WebKit has one dirty rectangle per frame (architecture 7, "WebKit painting on the CPU"). The big face shows through the clear parts between.
   */
  const livePicture = $derived.by(() => {
    const d = liveDrag;
    if (!d) return null;
    void waveBook.version;
    const ids = new Set([d.clip, ...(d.shift?.clips ?? [])]);
    const rects: GlRect[] = [];
    const waves: GlWave[] = [];
    const handles: GlRect[] = [];
    let left = Infinity;
    let right = -Infinity;
    let top = Infinity;
    let bottom = -Infinity;
    song.tracks.forEach((track, row) => {
      for (const clip of track.clips) {
        if (!clip.audio || !ids.has(clip.id)) continue;
        const box = G.clipBox(view, dims, row, clip.start, clip.end);
        const x0 = Math.max(box.x0, 0);
        const x1 = Math.min(box.x1, view.width);
        if (x1 <= x0) continue;
        paintAudioClip(rects, waves, handles, view, box, track, clip, selectedSet.has(clip.id) ? G.CLIP_SELECTED : G.CLIP_PLAIN(G.trackColor(track)), d);
        left = Math.min(left, x0);
        right = Math.max(right, x1);
        top = Math.min(top, box.y0);
        bottom = Math.max(bottom, box.y1);
      }
    });
    if (right <= left) return null;
    rects.push(...handles);
    for (const q of rects) {
      q.x -= left;
      q.y -= top;
    }
    W.shiftWaves(waves, -left, -top);
    return { place: { left, top, width: right - left, height: bottom - top }, rects, waves };
  });

  // ---- Names and tags (HTML) ----

  interface Label {
    id: number;
    x: number;
    y: number;
    w: number;
    name: string;
    mark: G.TakeMark;
    take: number;
    takes: number;
    selected: boolean;
    /** The marks of an audio clip (「×1.25」 「+2」; clipMarks in clipFields.ts). */
    marks: readonly string[];
  }
  const labels = $derived.by(() => {
    const out: Label[] = [];
    const shown = recShown;
    song.tracks.forEach((track, row) => {
      for (const clip of track.clips) {
        const box = G.clipBox(view, dims, row, clip.start, clip.end);
        if (box.x1 < 0 || box.x0 > view.width) continue;
        // Do not show the names of clips under a recording clip.
        if (rec && shown && rec.track === track.id && clip.start < shown.commit + perBar && clip.end > shown.lapFrom) continue;
        // The names of clips being moved or trimmed are shown on the moving picture side.
        if (dragIds.has(clip.id) && dragFrame) continue;
        if (naming?.clip.id === clip.id) continue;
        const x = Math.max(box.x0, 0) + dims.hair;
        // A clip that reaches past two visible widths to the right is cut there (so tag widths do not change on every scroll; no name is longer than that).
        const w = Math.min(box.x1, 2 * view.width) - Math.max(box.x0, 0) - 2 * dims.hair;
        out.push({
          id: clip.id,
          x,
          y: box.y0 + dims.mark,
          w,
          name: clip.name,
          mark: G.takeMark(clip, box.x1 - box.x0, tagMin),
          take: clip.take,
          takes: clip.takes,
          selected: selectedSet.has(clip.id) && !dragIds.has(clip.id),
          marks: clip.audio ? clipMarks(clip.audio.stretch) : NO_MARKS,
        });
      }
    });
    return out;
  });

  // ---- The clip name box (mock P10-ClipName (a)): over the name strip, at least as wide as the strip ----

  const naming = $derived.by(() => {
    const id = clipEdit.renaming;
    if (id === null || clipEdit.place !== "arrange") return null;
    for (const [row, track] of song.tracks.entries()) {
      const clip = track.clips.find((c) => c.id === id);
      if (!clip) continue;
      const box = G.clipBox(view, dims, row, clip.start, clip.end);
      const x = Math.max(box.x0, 0) + dims.hair;
      return { clip, x, y: box.y0 + dims.mark + dims.hair, w: Math.max(0, box.x1 - x - dims.hair), tag: G.takeMark(clip, box.x1 - box.x0, tagMin) === "tag" };
    }
    return null;
  });
  let nameDraft = $state("");
  let nameField = $state<HTMLInputElement>();
  let nameFor: number | null = null;
  $effect(() => {
    const id = clipEdit.place === "arrange" ? clipEdit.renaming : null;
    if (id === nameFor) return;
    nameFor = id;
    if (id === null) return;
    const clip = song.tracks.flatMap((tr) => tr.clips).find((c) => c.id === id);
    if (!clip || untrack(() => busy)) {
      stopClipRename();
      return;
    }
    nameDraft = clip.name;
    void nextTick().then(() => {
      nameField?.focus();
      nameField?.select();
    });
  });
  // Recording starting, or the clip going away, closes the box (core would not take the name).
  $effect(() => {
    if (clipEdit.place === "arrange" && clipEdit.renaming !== null && (busy || !song.tracks.some((tr) => tr.clips.some((c) => c.id === clipEdit.renaming)))) stopClipRename();
  });
  function decideName(clip: ClipView): void {
    stopClipRename();
    const command = renameCommand(clip.id, clip.name, nameDraft);
    if (command) send(command);
  }
  function nameKey(e: KeyboardEvent, clip: ClipView): void {
    if (e.isComposing) return;
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      decideName(clip);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      stopClipRename();
    }
  }
  // Pressing outside decides, as the track name box does (A-27). After Enter or Esc the box is already closed.
  function nameBlur(clip: ClipView): void {
    if (clipEdit.renaming === clip.id && clipEdit.place === "arrange") decideName(clip);
  }

  /** If the chosen clip has two or more takes, the switch tag right above it. */
  const switcher = $derived.by(() => {
    if (selected === null) return null;
    for (const [row, track] of song.tracks.entries()) {
      const clip = track.clips.find((c) => c.id === selected);
      if (!clip) continue;
      if (clip.takes < 2) return null;
      const box = G.clipBox(view, dims, row, clip.start, clip.end);
      if (box.x1 < 0 || box.x0 > view.width) return null;
      return { x: Math.max(box.x0, 0), y: box.y0 - switchH, take: clip.take, takes: clip.takes };
    }
    return null;
  });

  // ---- Overview ----

  /**
   * While the piano roll is open, the "visible range frame" of the overview is the range visible in the piano roll (spec 3).
   * When it is not open (the overview came from the mixer or the like), the frame fills the overview box as before (the overview slot).
   */
  const pianoRollOpen = $derived(layout.lower.some((b) => b.band === "pianoRoll"));
  const prRange = $derived.by(() => {
    if (!pianoRollOpen) return null;
    const r = visibleTicks(metrics.scale);
    return r.end > r.start ? r : null;
  });

  const overviewRects = $derived.by(() => {
    const out = G.overviewRects(
      view,
      song.tracks,
      overviewHeight,
      {
        overviewH: metrics.px("size.overview"),
        lineH: metrics.px("line.track"),
        gap: metrics.px("line.mark"),
        hair: metrics.px("line.hair"),
      },
      rec && recShown ? { track: rec.track, from: recShown.lapFrom, now: recShown.commit } : null,
    );
    if (prRange) {
      out.push(
        ...G.rangeFrameRects(view, prRange, overviewHeight, {
          inset: metrics.px("mock.range-frame.inset"),
          hair: metrics.px("line.hair"),
          dpr: metrics.dpr,
        }),
      );
    }
    return out;
  });

  // When the piano roll scrolls sideways and the frame goes outside the overview, page the overview to show the frame (not while the frame is dragged).
  let frameDrag: { x: number; tick: number } | null = null;
  $effect(() => {
    const r = prRange;
    const width = viewWidth;
    const ppt = pxPerTick;
    if (!r || !layout.arrangeOverview || width <= 0) return;
    untrack(() => {
      if (frameDrag) return;
      const next = G.revealRange(r.start * ppt, r.end * ppt, scrollTick * ppt, width);
      if (next !== scrollTick * ppt) scrollTick = next / ppt;
    });
  });

  let overviewSlot = $state<HTMLElement>();
  function frameDown(e: PointerEvent): void {
    if (e.button !== 0 || !prRange || !overviewSlot) return;
    const x = e.clientX - overviewSlot.getBoundingClientRect().left;
    if (x < G.tickToX(view, prRange.start) || x > G.tickToX(view, prRange.end)) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    frameDrag = { x: e.clientX, tick: prView.scrollTick };
  }
  function frameMove(e: PointerEvent): void {
    if (!frameDrag) return;
    scrollToTick(frameDrag.tick + (e.clientX - frameDrag.x) / pxPerTick);
  }
  function frameUp(): void {
    frameDrag = null;
  }
  // Esc: cancel the frame drag and go back to where it was before (spec 4).
  onMount(() =>
    onDragCancel(() => {
      if (!frameDrag) return false;
      scrollToTick(frameDrag.tick);
      frameDrag = null;
      return true;
    }),
  );

  // ---- Moving things: the playhead line, paging, the tip of the recording clip ----

  let timeLine = $state<HTMLElement | null>(null);
  let overviewLine = $state<HTMLElement | null>(null);
  let recLabel = $state<HTMLElement | null>(null);
  let tailNode: HTMLElement | null = null;
  let tailSlot: { update(next: GlSlot): void } | null = null;
  let offsetUs: number | null = null;

  /**
   * The tip of the recording clip is shown every frame. Meanwhile, align the top and bottom of the slot canvas with the canvas of the track meters shown in the same frame
   * (otherwise WebKit's dirty rectangles cut each other at their edges and the playhead line into 10. architecture 7).
   */
  const TAIL_ALIGN = "trackMeters";

  /** The growing tip of the recording clip (inside the recordingClip slot; the slot box exists even when not recording). */
  function recordingTail(node: HTMLElement) {
    const slot = glSlot(node, { kind: "rects", rects: [], alignY: TAIL_ALIGN });
    tailNode = node;
    tailSlot = slot;
    tailDrawn.row = -1;
    return {
      destroy() {
        slot.destroy();
        tailNode = null;
        tailSlot = null;
      },
    };
  }

  // Called every frame while playing. Write only when the value changes in screen pixels (writing even the same value makes the page redo that frame).
  let drawnLine: HTMLElement | null = null;
  let drawnLinePx = NaN;
  function placeLines(tick: number, scrollX: number): void {
    const line = layout.arrangeOverview ? overviewLine : timeLine;
    if (!line) return;
    const dpr = metrics.dpr;
    const px = Math.round((tick * pxPerTick - scrollX) * dpr);
    if (line === drawnLine && px === drawnLinePx) return;
    drawnLine = line;
    drawnLinePx = px;
    const x = px / dpr;
    line.style.transform = `translateX(${x}px)`;
    line.style.visibility = x < 0 || x >= viewWidth ? "hidden" : "";
  }

  function setStyle(node: HTMLElement, name: "left" | "top" | "width" | "height", px: number): void {
    const v = `${px}px`;
    if (node.style[name] !== v) node.style[name] = v;
  }

  /** The shape of the growing tip drawn now. If they are all the same, the rectangles are not remade. */
  const tailDrawn = { nowPx: NaN, scrollX: NaN, live: -1, lapFrom: NaN, commit: NaN, row: -1, pxPerTick: NaN, trackH: NaN, top: NaN, labelPx: NaN };
  let labelNode: HTMLElement | null = null;
  let labelTake = 0;
  let labelFullPx = 0;

  function moveTail(now: number, scrollX: number): void {
    const r = rec;
    if (!r) return;
    const lap = current.telemetry?.position?.lap ?? r.lap;
    const lapFrom = G.lapStart(r.from, lap, song.loopRange);
    const at = Math.max(now, lapFrom);
    const { commit, tailFrom } = G.recordingSplit(lapFrom, at, perBar);
    lastLap = lap;
    lastNow = at;
    const shown = recShown;
    if (!shown || shown.lap !== lap || shown.lapFrom !== lapFrom || shown.commit !== commit) recShown = { lap, lapFrom, commit };

    const dpr = metrics.dpr;
    const nowPx = Math.round((at * pxPerTick - scrollX) * dpr);
    if (recLabel) {
      // The name is cut at the right end of the clip. Once the width fits the whole name, stop writing (each write redraws the page).
      if (recLabel !== labelNode || r.take !== labelTake) {
        labelNode = recLabel;
        labelTake = r.take;
        recLabel.style.clipPath = "";
        labelFullPx = Math.ceil(recLabel.offsetWidth * dpr);
        tailDrawn.labelPx = NaN;
      }
      const startPx = Math.round(Math.max(lapFrom * pxPerTick - scrollX, 0) * dpr);
      const labelPx = Math.min(Math.max(0, nowPx - startPx - 2), labelFullPx);
      if (labelPx !== tailDrawn.labelPx) {
        tailDrawn.labelPx = labelPx;
        recLabel.style.clipPath = labelPx >= labelFullPx ? "" : `inset(0 calc(100% - ${labelPx / dpr}px) 0 0)`;
      }
    }
    const row = rowOf(r.track);
    if (!tailNode || !tailSlot || row < 0) return;
    const d = tailDrawn;
    if (
      d.nowPx === nowPx &&
      d.scrollX === scrollX &&
      d.live === liveVersion &&
      d.lapFrom === lapFrom &&
      d.commit === commit &&
      d.row === row &&
      d.pxPerTick === pxPerTick &&
      d.trackH === dims.trackH &&
      d.top === G.rowTop(dims, row)
    ) {
      return;
    }
    const placeChanged = d.scrollX !== scrollX || d.commit !== commit || d.lapFrom !== lapFrom || d.row !== row || d.pxPerTick !== pxPerTick || d.trackH !== dims.trackH || d.top !== G.rowTop(dims, row);
    d.nowPx = nowPx;
    d.scrollX = scrollX;
    d.live = liveVersion;
    d.lapFrom = lapFrom;
    d.commit = commit;
    d.row = row;
    d.pxPerTick = pxPerTick;
    d.trackH = dims.trackH;
    d.top = G.rowTop(dims, row);

    const v: G.View = { pxPerTick, scrollX, width: viewWidth };
    const box = G.clipBox(v, dims, row, tailFrom, at);
    if (placeChanged) {
      setStyle(tailNode, "left", box.x0);
      setStyle(tailNode, "top", box.y0);
      setStyle(tailNode, "width", 2 * perBar * pxPerTick + dims.hair);
      setStyle(tailNode, "height", box.y1 - box.y0);
    }
    const { notes, range } = liveOfLap(r.id, lap);
    const rects: GlRect[] = [];
    G.paintClip(rects, v, dims, box, G.CLIP_RECORDING, G.liveShapes(notes, at, tailFrom), { openLeft: tailFrom > lapFrom, range, until: at });
    for (const q of rects) {
      q.x -= box.x0;
      q.y -= box.y0;
    }
    tailSlot.update({ kind: "rects", rects, alignY: TAIL_ALIGN });
  }

  /** The growing tips and names of audio clips being recorded (per track id). Written only when they change in screen pixels. */
  interface VocalTail {
    node: HTMLElement;
    slot: { update(next: GlSlot): void; destroy(): void };
    label: HTMLElement | null;
    labelFullPx: number;
    drawn: string;
    labelPx: number;
  }
  const vocalTails = new Map<number, VocalTail>();
  function vocalTail(node: HTMLElement, track: number) {
    const slot = glSlot(node, { kind: "rects", rects: [], alignY: TAIL_ALIGN });
    vocalTails.set(track, { node, slot, label: null, labelFullPx: 0, drawn: "", labelPx: NaN });
    return {
      destroy() {
        slot.destroy();
        vocalTails.delete(track);
      },
    };
  }
  function vocalLabel(node: HTMLElement, track: number) {
    const tail = vocalTails.get(track);
    if (tail) tail.label = node;
    return {
      destroy() {
        const t = vocalTails.get(track);
        if (t?.label === node) t.label = null;
      },
    };
  }

  function moveVocalTails(now: number, scrollX: number): void {
    if (recVocals.length === 0) return;
    const dpr = metrics.dpr;
    const v: G.View = { pxPerTick, scrollX, width: viewWidth };
    let commitAll = vocalCommit;
    for (const r of recVocals) {
      const tail = vocalTails.get(r.track);
      const row = rowOf(r.track);
      if (!tail || row < 0) continue;
      const at = Math.max(now, r.from);
      const { commit, tailFrom } = G.recordingSplit(r.from, at, perBar);
      commitAll = commit;
      const nowPx = Math.round((at * pxPerTick - scrollX) * dpr);
      if (tail.label) {
        if (tail.labelFullPx === 0) {
          tail.label.style.clipPath = "";
          tail.labelFullPx = Math.ceil(tail.label.offsetWidth * dpr);
        }
        const startPx = Math.round(Math.max(r.from * pxPerTick - scrollX, 0) * dpr);
        const labelPx = Math.min(Math.max(0, nowPx - startPx - 2), tail.labelFullPx);
        if (labelPx !== tail.labelPx) {
          tail.labelPx = labelPx;
          tail.label.style.clipPath = labelPx >= tail.labelFullPx ? "" : `inset(0 calc(100% - ${labelPx / dpr}px) 0 0)`;
        }
      }
      // The waveform is also redrawn when more peaks arrive (about 10 times a second).
      const peaks = recSource(r, v, tailFrom, at)?.source;
      const key = [nowPx, scrollX, commit, row, pxPerTick, G.rowHeight(dims, row), G.rowTop(dims, row), peaks?.id, peaks?.stamp].join(" ");
      if (key === tail.drawn) continue;
      tail.drawn = key;
      const box = G.clipBox(v, dims, row, tailFrom, at);
      setStyle(tail.node, "left", box.x0);
      setStyle(tail.node, "top", box.y0);
      setStyle(tail.node, "width", 2 * perBar * pxPerTick + dims.hair);
      setStyle(tail.node, "height", box.y1 - box.y0);
      const rects: GlRect[] = [];
      G.paintClip(rects, v, dims, box, G.CLIP_RECORDING, [], { openLeft: tailFrom > r.from, range: null, until: at });
      const waves: GlWave[] = [];
      recWaves(waves, rects.length, v, box, r, tailFrom, at, tailFrom > r.from);
      for (const q of rects) {
        q.x -= box.x0;
        q.y -= box.y0;
      }
      W.shiftWaves(waves, -box.x0, -box.y0);
      tail.slot.update({ kind: "rects", rects, waves, alignY: TAIL_ALIGN });
    }
    if (commitAll !== vocalCommit) vocalCommit = commitAll;
  }
  $effect(() => {
    if (recVocals.length === 0) untrack(() => (vocalCommit = 0));
  });

  // The tip is drawn every frame, so the lap notes and key range are made only when the recording notes change (architecture 7).
  let lapCache: { recording: number; lap: number; version: number; notes: readonly LiveNote[]; range: { low: number; high: number } | null } | null = null;
  function liveOfLap(recording: number, lap: number): { notes: readonly LiveNote[]; range: { low: number; high: number } | null } {
    const c = lapCache;
    if (c && c.recording === recording && c.lap === lap && c.version === liveVersion) return c;
    const notes = book?.notesOf(recording, lap) ?? [];
    lapCache = { recording, lap, version: liveVersion, notes, range: G.keyRange(G.liveShapes(notes, 0)) };
    return lapCache;
  }

  function frame(): void {
    const tick = shownTick(transport, current.telemetry, offsetUs === null ? null : performance.now() * 1000 + offsetUs);
    let scrollX = scrollTick * pxPerTick;
    if (follow) {
      const next = G.followScroll(tick * pxPerTick, scrollX, viewWidth);
      if (next !== scrollX) {
        scrollX = next;
        scrollTick = next / pxPerTick;
      }
    }
    placeLines(tick, scrollX);
    moveTail(tick, scrollX);
    moveVocalTails(tick, scrollX);
    showNow(tick);
  }

  // ---- Tempo and time signature at the playhead, in the folded 「テンポ ・ 拍子の変更」 row (every frame while playing; written only when the text changes) ----

  function nowText(tick: number): string {
    const at = Math.max(0, tick);
    const m = map.meterAt(at);
    return t("arrange.tempoAndSignature", { tempo: tempoText(Math.round(map.tempoAt(at) * 100) / 100), signature: `${m.num}/${m.den}` });
  }
  let now = $state("");

  /** While playing, written at most 10 times a second (writing every frame cuts the playhead rectangle and adds redraw rectangles; docs/history/measurements/2026-11-20-round6-step1.md (5)). */
  const NOW_EVERY_MS = 100;
  let nowAt = -Infinity;
  function showNow(tick: number): void {
    if (timeline.open) return;
    const ms = performance.now();
    if (ms - nowAt < NOW_EVERY_MS) return;
    const text = nowText(tick);
    if (text === now) return;
    now = text;
    nowAt = ms;
  }
  $effect(() => {
    if (transport.state === "stopped") now = nowText(transport.position);
  });

  let lastState = "stopped";
  $effect(() => {
    const s = transport.state;
    if (lastState === "stopped" && s !== "stopped") follow = true;
    lastState = s;
  });

  $effect(() => {
    if (transport.state === "stopped") return;
    if (offsetUs === null) {
      clockOffsetUs().then(
        (v) => (offsetUs = v),
        () => {},
      );
    }
    return everyGlFrame(frame);
  });

  $effect(() => {
    void timeLine;
    void overviewLine;
    if (transport.state === "stopped") placeLines(transport.position, view.scrollX);
  });

  // ---- By hand: horizontal scroll, ruler drag, choosing clips ----

  const mac = platformOf(navigator) === "mac";
  let zoomWheel = 0;

  /** Zoom in or out sideways by `by` steps. Scrolls so that position x (px from the left edge of the visible part) stays at the same place on the screen. */
  function zoomAt(by: number, x: number): void {
    const anchor = G.xToTick(view, x);
    if (!zoomArrangeBy(by)) return;
    scrollTick = Math.max(0, anchor - x / pxPerTick);
  }

  /** The wheel. With Mod held, zoom in or out sideways around the cursor (spec 3). */
  function wheelOn(area: () => HTMLElement | undefined) {
    return (e: WheelEvent): void => {
      if (mac ? e.metaKey : e.ctrlKey) {
        e.preventDefault();
        const z = G.wheelZoomSteps(zoomWheel, e.deltaY, e.deltaMode);
        zoomWheel = z.acc;
        const el = area();
        if (z.steps !== 0 && el) zoomAt(z.steps, e.clientX - el.getBoundingClientRect().left);
        return;
      }
      wheel(e);
    };
  }

  function wheel(e: WheelEvent): void {
    let dx = e.deltaX;
    let dy = e.deltaY;
    if (e.shiftKey) {
      if (dx === 0) dx = dy;
      dy = 0;
    }
    if (dx !== 0) {
      e.preventDefault();
      const unit = e.deltaMode === 1 ? metrics.px("size.row") : e.deltaMode === 2 ? viewWidth : 1;
      const max = G.scrollMax(song.tracks, song.loopRange, transport.position, { pxPerTick, width: viewWidth });
      scrollTick = G.wheelScroll(view.scrollX, dx * unit, max) / pxPerTick;
      follow = false;
    }
    if (dy !== 0 && wheelRows(dy, e.deltaMode)) e.preventDefault();
  }

  // ---- Scroll the track rows vertically (a temporary form, the owner's decision; the head column and the lanes by the same amount; the ruler, marker and tempo rows above do not move) ----
  // The scroll position is kept only by the screen (not in the song, not in undo). It uses the browser scroll (scrollTop): rows out of view
  // are not drawn thanks to content-visibility, and WebGL marks move with their boxes (gl/engine.ts clips with the data-gl-clip box).

  let lanesView = $state<HTMLElement>();
  let headsView = $state<HTMLElement>();
  let rowsHeight = $state(0);
  let scrollY = $state(0);
  /** The height of all rows. The head column has a 「＋ トラック」 row at the bottom, so the lane side leaves the same space. */
  const rowsTotal = $derived(rows.height + metrics.px("mock.arrange.add-track"));
  const maxScrollY = $derived(Math.max(0, rowsTotal - rowsHeight));
  /** Stop at the bottom when rows decrease or the window grows taller. */
  const shownY = $derived(Math.min(scrollY, maxScrollY));
  $effect(() => {
    const y = shownY;
    if (lanesView) lanesView.scrollTop = y;
    if (headsView) headsView.scrollTop = y;
  });
  const rowsScroll = (y: number) => (scrollY = Math.max(0, Math.min(y, maxScrollY)));

  /** Scroll vertically with the wheel. Not during a clip drag or a box select (what is held would slip from under the pointer). true if it scrolled. */
  function wheelRows(dy: number, mode: number): boolean {
    if (layout.arrangeOverview || clipDrag || boxSel || maxScrollY <= 0) return false;
    const unit = mode === 1 ? metrics.px("size.row") : mode === 2 ? rowsHeight : 1;
    rowsScroll(shownY + dy * unit);
    return true;
  }
  function wheelHeads(e: WheelEvent): void {
    if (e.ctrlKey || e.metaKey || e.shiftKey || e.deltaY === 0) return;
    if (wheelRows(e.deltaY, e.deltaMode)) e.preventDefault();
  }

  // Scroll so that the row and bands of a track chosen by keys, or of an added track, are visible (spec 3 "Scrolling the arrange track rows vertically (the rest of 92)").
  // Do not scroll to a track chosen with the mouse. Do not scroll for play or record either.
  let seenTarget: number | null | undefined = undefined;
  let seenTracks = 0;
  /** The last input was the pointer (it was chosen with the mouse). */
  let lastByPointer = false;
  onMount(() => {
    const pointer = () => (lastByPointer = true);
    const key = () => (lastByPointer = false);
    document.addEventListener("pointerdown", pointer, true);
    document.addEventListener("keydown", key, true);
    return () => {
      document.removeEventListener("pointerdown", pointer, true);
      document.removeEventListener("keydown", key, true);
    };
  });
  $effect(() => {
    const target = song.playTarget;
    const count = song.tracks.length;
    untrack(() => {
      const row = song.tracks.findIndex((tr) => tr.id === target);
      const added = count > seenTracks;
      if (seenTarget !== undefined && (added || (target !== seenTarget && !lastByPointer)) && row >= 0) {
        rowsScroll(G.revealRange(rows.tops[row]!, rows.bottoms[row]!, shownY, rowsHeight));
      }
      seenTarget = target;
      seenTracks = count;
    });
  });

  let drag: { d: G.RulerDrag; downX: number; left: number } | null = null;
  const still = () => metrics.px("space.1");

  function rulerDown(e: PointerEvent): void {
    if (e.button !== 0) return;
    const left = (e.currentTarget as HTMLElement).getBoundingClientRect().left;
    const x = e.clientX - left;
    drag = { d: G.startRulerDrag(view, x, map, songLoop, metrics.px("space.2")), downX: x, left };
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
  }
  function rulerMove(e: PointerEvent): void {
    if (!drag) return;
    const x = e.clientX - drag.left;
    if (Math.abs(x - drag.downX) >= still()) preview = G.dragRange(drag.d, view, x, map);
  }
  function rulerUp(e: PointerEvent): void {
    if (!drag) return;
    const step = e.shiftKey ? null : rulerLine;
    const command = G.rulerCommand(drag.d, view, drag.downX, e.clientX - drag.left, map, still(), step);
    drag = null;
    if (command.type === "locate") preview = null;
    const before = current.rev;
    // If the range does not change, no diff comes, so the look is cleared by the rev of the reply.
    void send(command).then((rev) => {
      if (rev === before && !drag) preview = null;
    });
  }
  /** Snap to the lines visible on the ruler (B-61): beats if beat lines are visible, usually bars, numbered bars when thinned. */
  function rulerLine(at: number): number {
    if (gridBeats) return snapToStep(map, at, 0);
    const every = barLabelEvery;
    const m = map.meterAt(Math.max(0, at));
    const line = Math.round((m.bar + (Math.max(0, at) - m.barTick) / ticksPerBar(m)) / every) * every;
    return map.tickOfBar(line);
  }
  function rulerCancel(): boolean {
    if (!drag) return false;
    drag = null;
    preview = null;
    return true;
  }
  // Esc: cancel the loop range drag and go back to the old range (spec 4).
  onMount(() => onDragCancel(rulerCancel));

  // Press a clip: choose it. Press and move: move (copy with Alt); on a left or right edge (part.clip.edge), the length. While recording and during count-in, only choose.
  const busy = $derived(transport.state === "recording" || transport.state === "countIn");
  let lanesBox = $state<HTMLElement>();
  let laneCursor = $state<CD.ArrangeCursor>("default");
  let readingAt = $state<{ x: number; y: number } | null>(null);
  let readingW = $state(0);
  let readingH = $state(0);

  function lanePoint(e: MouseEvent): { x: number; y: number } {
    const r = lanesBox!.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  }
  const hitAt = (p: { x: number; y: number }) => CD.clipHit(view, dims, song.tracks, p.x, p.y, metrics.px("part.clip.edge"));
  const dragInput = (shift: boolean): CD.DragInput => ({
    view,
    dims,
    tracks: song.tracks,
    step: shift ? null : perBar,
    snap: (tick: number) => nearestBarTick(map, tick),
    stillPx: still(),
  });

  /** Press in an empty place and move to select by a box (B-71). x0, y0 are the pressed place; add is Shift. */
  let boxSel = $state<{ x0: number; y0: number; x: number; y: number; moved: boolean; add: boolean } | null>(null);
  const boxFrame = $derived(
    boxSel?.moved
      ? { x: Math.min(boxSel.x0, boxSel.x), y: Math.min(boxSel.y0, boxSel.y), w: Math.abs(boxSel.x - boxSel.x0), h: Math.abs(boxSel.y - boxSel.y0) }
      : null,
  );
  /** Pressed one of several selected clips. If released without moving, select only that one. */
  let clickedOnly: number | null = null;

  function lanesDown(e: PointerEvent): void {
    if (e.button !== 0) return;
    const p = lanePoint(e);
    const hit = hitAt(p);
    clickedOnly = null;
    pickClips();
    const lanes = e.currentTarget as HTMLElement;
    if (!hit) {
      boxSel = { x0: p.x, y0: p.y, x: p.x, y: p.y, moved: false, add: e.shiftKey };
      lanes.setPointerCapture?.(e.pointerId);
      return;
    }
    const id = hit.clip.id;
    const had = current.state!.arrange.selectedClips;
    let ids: readonly number[];
    if (e.shiftKey) {
      send({ type: "selectClips", clips: [id], mode: "add" });
      ids = had.includes(id) ? had : [...had, id];
    } else if (had.includes(id)) {
      ids = had;
      if (had.length > 1 || selected !== id) clickedOnly = id;
    } else {
      send({ type: "selectClip", clip: id });
      ids = [id];
    }
    if (busy) {
      if (clickedOnly !== null) send({ type: "selectClip", clip: clickedOnly });
      clickedOnly = null;
      return;
    }
    clipPending = null;
    clipDrag = CD.startClipDrag(hit, p.x, p.y, { shift: e.shiftKey, alt: e.altKey }, CD.groupOf(song.tracks, ids));
    laneCursor = CD.clipDragCursor(clipDrag);
    lanes.setPointerCapture?.(e.pointerId);
  }
  function lanesMove(e: PointerEvent): void {
    const p = lanePoint(e);
    const b = boxSel;
    if (b) {
      const moved = b.moved || Math.hypot(p.x - b.x0, p.y - b.y0) >= still();
      boxSel = { ...b, x: p.x, y: p.y, moved };
      return;
    }
    const d = clipDrag;
    if (!d) {
      laneCursor = busy ? "default" : CD.hoverCursor(hitAt(p), e.altKey);
      return;
    }
    const next = CD.dragClipTo(d, dragInput(e.shiftKey), p.x, p.y, { shift: e.shiftKey, alt: e.altKey });
    if (next !== d) {
      clipDrag = next;
      dragReading.text = CD.clipDragReading(next, map);
      laneCursor = CD.clipDragCursor(next);
    }
    const r = timeBox!.getBoundingClientRect();
    readingAt = dragReading.text ? { x: e.clientX - r.left, y: e.clientY - r.top } : null;
  }
  function lanesUp(e: PointerEvent): void {
    const b = boxSel;
    if (b) {
      boxSel = null;
      const p = lanePoint(e);
      if (b.moved) {
        const clips = CD.clipsInBox(view, dims, song.tracks, b.x0, b.y0, p.x, p.y);
        send({ type: "selectClips", clips, mode: b.add ? "add" : "replace" });
      } else if (!b.add && (selectedSet.size > 0 || selected !== null)) {
        send({ type: "selectClip", clip: null });
      }
      return;
    }
    const d = clipDrag;
    if (!d) return;
    clipDrag = null;
    dragReading.text = null;
    readingAt = null;
    if (!d.press.moved && clickedOnly !== null) send({ type: "selectClip", clip: clickedOnly });
    clickedOnly = null;
    const command = CD.clipDragCommand(d, song.tracks);
    if (!command) return;
    clipPending = d;
    pendingRev = null;
    void send(command).then((rev) => (pendingRev = rev ?? -Infinity));
  }
  function cancelClipDrag(): boolean {
    if (boxSel) {
      boxSel = null;
      return true;
    }
    if (!clipDrag) return false;
    clipDrag = null;
    clickedOnly = null;
    dragReading.text = null;
    readingAt = null;
    laneCursor = "default";
    return true;
  }
  onMount(() => onDragCancel(cancelClipDrag));
  onDestroy(cancelClipDrag);

  // ---- Context menu (B-60). Right-clicking something not selected selects only it, then opens ----

  const platform = platformOf(navigator);
  const modKey = (letter: string) => `${platform === "mac" ? t("key.cmd") : t("key.ctrl")} ${letter}`;
  const shiftKey = (letter: string) => `${t("key.shift")} ${letter}`;
  const menuRow = (label: MenuRow["label"], usable: boolean, run: () => void, key?: string): MenuRow => ({ kind: "row", label, key, usable, run });
  /** The selection when a row is pressed (any change from the right-click has arrived by then). */
  const chosen = () => current.state!.arrange.selectedClips;
  const clipboardCount = $derived(current.state!.arrange.clipboardClips);

  /** While recording and during count-in, rows that change the song cannot be used. */
  function clipMenu(clip: ClipView, at: number, ids: readonly number[]): MenuEntry[] {
    const change = !busy;
    const keys = clip.audio === null;
    const open = () => {
      if (!pianoRollOpen) send({ type: "toggleBand", band: "pianoRoll" });
    };
    const cutAt = splitTick(clip, rulerLine(at));
    // 「逆にする」 and 「ノーマライズ」 for audio clips are seats for a later round (spec 3 "While the sound is being made").
    const later: MenuEntry[] = clip.audio
      ? [{ kind: "separator" }, menuRow("menu.reverse", false, () => {}), menuRow("menu.normalize", false, () => {})]
      : [];
    return [
      ...(keys ? ([menuRow("menu.openInPianoRoll", true, open, t("key.pianoRoll")), { kind: "separator" }] satisfies MenuEntry[]) : []),
      menuRow("menu.cut", change, () => send({ type: "cutClips", clips: chosen() }), modKey(t("key.cut"))),
      menuRow("menu.copy", true, () => send({ type: "copyClips", clips: chosen() }), modKey(t("key.copy"))),
      menuRow("menu.paste", change && clipboardCount > 0, () => send({ type: "pasteClips", to: null }), modKey(t("key.paste"))),
      menuRow("menu.duplicate", change, () => send({ type: "duplicateClips", clips: chosen() }), modKey(t("key.duplicate"))),
      menuRow("menu.delete", change, () => send({ type: "deleteSelected" }), t("key.delete")),
      { kind: "separator" },
      menuRow("menu.splitHere", change && cutAt !== null, () => cutAt !== null && send({ type: "splitClipAt", clip: clip.id, tick: cutAt })),
      ...(keys ? [menuRow("menu.join", change && joinable(song.tracks, ids), () => send({ type: "joinClips", clips: chosen() }), modKey(t("key.join")))] : []),
      menuRow("menu.renameClip", change, () => startClipRename(clip.id), t("key.renameTrack")),
      { kind: "separator" },
      menuRow("menu.nextTake", change && clip.takes > 1, () => send({ type: "stepTake", delta: 1 }), t("key.take")),
      menuRow("menu.prevTake", change && clip.takes > 1, () => send({ type: "stepTake", delta: -1 }), shiftKey(t("key.take"))),
      menuRow("menu.deleteTake", change && clip.takes > 1, () => send({ type: "deleteTake", clip: clip.id })),
      ...later,
    ];
  }

  /**
   * Double press (spec 3, round 10): on a clip's name strip, the name box; on the wave of an audio clip, that track's row
   * to 160 and back (B-94-23); on an empty place of a keyboard track, a new clip there (Rust snaps it to the bar).
   * An empty place of a vocal track does nothing.
   */
  function lanesDouble(e: MouseEvent): void {
    if (e.button !== 0 || clipDrag || boxSel) return;
    const p = lanePoint(e);
    const hit = hitAt(p);
    if (!hit) {
      const track = song.tracks[G.trackRowAt(dims, p.y)];
      if (track && track.kind.kind === "keys" && !busy) send({ type: "createClip", track: track.id, tick: Math.max(0, Math.round(G.xToTick(view, p.x))) });
      return;
    }
    const box = G.clipBox(view, dims, hit.row, hit.clip.start, hit.clip.end);
    if (p.y < box.y0 + dims.mark + dims.nameH) {
      if (!busy) startClipRename(hit.clip.id);
      return;
    }
    if (hit.clip.audio === null) return;
    const track = song.tracks[hit.row];
    if (track) send({ type: "setTallTrack", track: track.id });
  }

  function lanesContext(e: MouseEvent): void {
    e.preventDefault();
    // The name box is a text box: its menu is the text menu (ContextMenu.svelte).
    if (clipDrag || boxSel || e.target instanceof HTMLInputElement) return;
    const p = lanePoint(e);
    const hit = hitAt(p);
    const at = Math.max(0, G.xToTick(view, p.x));
    if (hit) {
      pickClips();
      const had = selectedSet.has(hit.clip.id);
      if (!had) send({ type: "selectClip", clip: hit.clip.id });
      openContextMenu(e.clientX, e.clientY, clipMenu(hit.clip, at, had ? current.state!.arrange.selectedClips : [hit.clip.id]));
      return;
    }
    // Move the pressed place to the same grid as clip moves (usually 1 bar). Not with Shift.
    const track = song.tracks[G.trackRowAt(dims, p.y)];
    const tick = e.shiftKey ? Math.round(at) : Math.max(0, nearestBarTick(map, at));
    const paste = () => {
      if (track) send({ type: "pasteClips", to: { track: track.id, tick } });
    };
    const more: MenuEntry[] = !track
      ? []
      : track.kind.kind === "keys"
        ? [menuRow("menu.createClipHere", !busy, () => send({ type: "createClip", track: track.id, tick: Math.round(at) }))]
        : [menuRow("menu.importAudio", !busy, () => send({ type: "importAudio", track: track.id }))];
    openContextMenu(e.clientX, e.clientY, [
      menuRow("menu.paste", !busy && track !== undefined && clipboardCount > 0, paste, modKey(t("key.paste"))),
      ...more,
    ]);
  }

  const readingBox = $derived.by(() => {
    if (!readingAt || !dragReading.text) return null;
    const off = metrics.px("part.reading.offset");
    return { left: Math.max(0, readingAt.x - off - readingW), top: Math.max(0, readingAt.y - off - readingH) };
  });

  const lineRec = $derived(transport.state === "countIn" || transport.state === "recording");

  // ---- Horizontal bar ----

  let timeBox = $state<HTMLElement>();
  const scrollRight = $derived(G.scrollMax(song.tracks, song.loopRange, transport.position, { pxPerTick, width: viewWidth }));
  function barScroll(offset: number): void {
    scrollTick = offset / pxPerTick;
    follow = false;
  }

  /** Where Mod + and Mod − go (see focus in pianoroll/view.svelte.ts). Pressing the arrange sends them to the arrange. */
  function focusArrange(): void {
    prView.focus = "arrange";
  }
</script>

{#snippet termed(text: string, word: string)}{#each termParts(text, word) as piece, j (j)}{#if piece.term}<span use:term={word}
        >{piece.text}</span
      >{:else}{piece.text}{/if}{/each}{/snippet}

{#if layout.arrangeOverview}
  <section class="overview" style:height="{height}px" onwheel={wheelOn(() => overviewSlot)} onpointerdowncapture={focusArrange}>
    <div class="overview-label">
      <span class="m-lbl">{@render termed(t("arrange.overview"), "アレンジ")}</span>
    </div>
    <div
      class="overview-slot"
      role="presentation"
      use:glRegion={"overview"}
      bind:this={overviewSlot}
      bind:clientWidth={overviewWidth}
      bind:clientHeight={overviewHeight}
      onpointerdown={frameDown}
      onpointermove={frameMove}
      onpointerup={frameUp}
      onpointercancel={frameUp}
    >
      <div class="fill" use:glSlot={{ kind: "rects", rects: overviewRects }}></div>
      {#if !prRange}<div class="fill" use:glSlot={{ kind: "overview" }}></div>{/if}
      <div class="play-line" class:rec={lineRec} bind:this={overviewLine}></div>
    </div>
    <div class="layer">{@render children?.()}</div>
    {#if keysHere}<div class="focus-mark" data-test="arrange-focus-mark"></div>{/if}
  </section>
{:else}
  <section class="arrange" style:height="{height}px" onpointerdowncapture={focusArrange}>
    <div class="head" onwheel={wheelHeads}>
      <div class="row title-row">
        <span class="m-name" use:term={"アレンジ"}>{t("arrange.title")}</span>
        <span class="zoom">
          <button class="m-btn zoom-btn m-focus-in" aria-label={t("zoom.out")} onclick={() => zoomArrangeBy(-1)}>{t("zoom.outMark")}</button>
          <button class="m-btn zoom-btn m-focus-in" aria-label={t("zoom.in")} onclick={() => zoomArrangeBy(1)}>{t("zoom.inMark")}</button>
        </span>
        <span class="m-lbl" use:term={"グリッド"}>{t("arrange.grid")} <span class="m-num m-hi">{gridText(0, false)}</span></span>
      </div>
      <div class="row">
        <span class="m-lbl" use:term={"マーカー"}>
          {#if shownLoop}
            {#each textParts("arrange.markerLoop", { range: `${shownLoop.from}–${shownLoop.to}` }) as part, i (i)}
              {#if part.hole}<span class="m-num loop-range">{part.text}</span>{:else}{part.text}{/if}
            {/each}
          {:else}
            {t("arrange.marker")}
          {/if}
        </span>
      </div>
      <div class="tempo-head" class:open={timeline.open}>
        <button
          class="m-plain row tempo-toggle"
          aria-expanded={timeline.open}
          data-test="tempo-toggle"
          onclick={() => send({ type: "setTimelineOpen", open: !timeline.open })}
        >
          <span class:m-lbl={!timeline.open}>{timeline.open ? "▾" : "▸"}</span>
          <span class:m-lbl={!timeline.open} use:term={"テンポの変更"}>{t("arrange.tempoChanges")}</span>
          <span class="m-spacer"></span>
          <span class="m-num m-lbl">{t("arrange.changeCount", { count: changeCount(timeline, map) })}</span>
        </button>
        {#if timeline.open}
          {@const sum = summaries(timeline)}
          <div class="tempo-sums">
            <span class="m-lbl" data-test="tempo-summary">{sum.tempo}</span>
            <span class="m-lbl" data-test="meter-summary">{sum.meter}</span>
          </div>
        {/if}
      </div>
      <div class="heads-view" bind:this={headsView} use:glRegion={"trackMeters"}>
        <TrackHeads />
      </div>
    </div>

    <div class="time" bind:this={timeBox} bind:clientWidth={timeWidth} onwheel={wheelOn(() => timeBox)}>
      <div
        class="ruler bars"
        role="presentation"
        onpointerdown={rulerDown}
        onpointermove={rulerMove}
        onpointerup={rulerUp}
        onpointercancel={rulerCancel}
      >
        <div class="bar-strip" style:transform="translateX({-view.scrollX}px)">
          {#each barNumbers as b (b.n)}
            <span class="m-num m-lbl bar-number" style:left="{b.x}px">{b.n}</span>
          {/each}
        </div>
        {#if shownLoop}
          <div class="loop-mark" style:left="{loopX(shownLoop.from)}px" style:width="{loopX(shownLoop.to) - loopX(shownLoop.from)}px"></div>
        {/if}
      </div>
      <MarkerRow {view} {map} {timeline} {busy} />
      <!-- The tempo band lines and points are drawn on the same arrangeLanes sheet as the clips (no extra slot). The slot box holds the band and the clip area. -->
      <div class="lanes-wrap" use:glRegion={"arrangeLanes"}>
      <TempoBand {view} {map} {timeline} {now} {busy} />
      <div class="lanes-box">
      <div class="lanes-view" data-gl-clip bind:this={lanesView} bind:clientHeight={rowsHeight} use:glSlot={{ kind: "gridLines", lines: gridLines, beats: gridBeats }}>
      <div
        class="lanes"
        role="presentation"
        data-tip-through-root
        bind:this={lanesBox}
        style:cursor={laneCursor}
        onpointerdown={lanesDown}
        onpointermove={lanesMove}
        onpointerup={lanesUp}
        onpointercancel={cancelClipDrag}
        oncontextmenu={lanesContext}
        ondblclick={lanesDouble}
      >
        {#each song.tracks as track (track.id)}
          <div class="lane" style:height={track.id === current.state!.arrange.tallTrack ? `${metrics.px("part.track.vocal")}px` : null}></div>
          {#each rows.lanes.filter((l) => l.track.id === track.id) as l (JSON.stringify(l.lane.target))}
            <AutoLane {track} lane={l.lane} {view} {map} {busy} />
          {/each}
          {#if showsAddRow(track)}<div class="add-row"></div>{/if}
        {/each}
        <div class="fill" use:glSlot={{ kind: "rects", rects: lanePicture.rects, waves: laneWaves, shiftX: view.scrollX - lanePicture.from }}></div>
        <div class="rec-host" use:glRegion={"recordingClip"}>
          {#each rows.lanes as l (l.track.id + JSON.stringify(l.lane.target))}
            <WriteLine track={l.track} lane={l.lane} top={l.top} {view} />
          {/each}
          {#if rec}
            <div class="rec-tail" use:recordingTail></div>
          {/if}
          {#each recVocals as v (v.track)}
            <div class="rec-tail" data-test="vocal-rec-tail" use:vocalTail={v.track}></div>
          {/each}
          {#if livePicture}
            {@const at = livePicture.place}
            <div
              class="drag-clip"
              data-test="audio-live"
              style:left="{at.left}px"
              style:top="{at.top}px"
              style:width="{at.width}px"
              style:height="{at.height}px"
              use:glSlot={{ kind: "rects", rects: livePicture.rects, waves: livePicture.waves }}
            ></div>
          {/if}
          {#if dragPicture}
            {@const at = dragPicture.place}
            <div
              class="drag-clip"
              style:left="{at.left}px"
              style:top="{at.top}px"
              style:width="{at.width}px"
              style:height="{at.height}px"
              style:transform={dragShift}
              use:draggedClip
            ></div>
          {/if}
        </div>
        <div class="labels">
          {#each labels as c (c.id)}
            <div class="clip-label" style:left="{c.x}px" style:top="{c.y}px" style:width="{c.w}px">
              {#if c.mark === "square"}
                <span class="take-square"></span>
              {:else}
                {#if c.mark === "tag"}
                  <span class="m-num take-tag" class:selected={c.selected}>{t("arrange.takeTag", { take: c.take, takes: c.takes })}</span>
                {/if}
                <span class="clip-name" data-tip-through>{c.name}</span>
                {#each c.marks as mark, i (i)}<span class="m-num clip-mark">{mark}</span>{/each}
              {/if}
            </div>
          {/each}
          {#if rec && recShown}
            {@const row = rowOf(rec.track)}
            <div
              class="clip-label recording"
              bind:this={recLabel}
              style:left="{Math.max(G.tickToX(view, recShown.lapFrom), 0) + dims.hair}px"
              style:top="{G.rowTop(dims, row) + dims.inset + dims.mark}px"
            >
              {overdubName === null ? t("arrange.recordingClip", { take: rec.take }) : t("arrange.overdubClip", { clip: overdubName })}
            </div>
          {/if}
          {#each recVocals as v (v.track)}
            {@const row = rowOf(v.track)}
            {#if row >= 0}
              <div
                class="clip-label recording"
                data-test="vocal-rec-label"
                use:vocalLabel={v.track}
                style:left="{Math.max(G.tickToX(view, v.from), 0) + dims.hair}px"
                style:top="{G.rowTop(dims, row) + dims.inset + dims.mark}px"
              >
                {t("arrange.recordingVocalClip", { track: song.tracks[row]!.name, take: v.take })}
              </div>
            {/if}
          {/each}
          {#each dragPicture?.labels ?? [] as c (c.id)}
            <div class="clip-label" style:left="{c.x}px" style:top="{c.y}px" style:width="{c.w}px" style:transform={dragShift}>
              {#if c.mark === "square"}
                <span class="take-square"></span>
              {:else}
                {#if c.mark === "tag"}
                  <span class="m-num take-tag selected">{t("arrange.takeTag", { take: c.take, takes: c.takes })}</span>
                {/if}
                <span class="clip-name" data-tip-through>{c.name}</span>
                {#each c.marks as mark, i (i)}<span class="m-num clip-mark">{mark}</span>{/each}
              {/if}
            </div>
          {/each}
        </div>
        {#if naming}
          {@const c = naming.clip}
          <div
            class="clip-naming"
            role="presentation"
            data-over-clips
            style:left="{naming.x}px"
            style:top="{naming.y}px"
            style:min-width="{naming.w}px"
            onpointerdown={(e) => e.stopPropagation()}
            ondblclick={(e) => e.stopPropagation()}
          >
            {#if naming.tag}
              <span class="m-num take-tag selected">{t("arrange.takeTag", { take: c.take, takes: c.takes })}</span>
            {/if}
            <span class="name-cell">
              <span class="name-sizer" aria-hidden="true">{nameDraft}</span>
              <input
                class="name-field"
                type="text"
                size="1"
                spellcheck="false"
                autocomplete="off"
                aria-label={t("menu.renameClip")}
                data-test="clip-name-field"
                bind:this={nameField}
                bind:value={nameDraft}
                onkeydown={(e) => nameKey(e, c)}
                onblur={() => nameBlur(c)}
              />
            </span>
          </div>
        {/if}
        <AudioClipNotes {view} {dims} tracks={song.tracks} askHost={timeBox} />
        <AudioClipHandles {view} {dims} tracks={song.tracks} selected={selectedSet} {busy} />
        <DropFrame {view} {dims} tracks={song.tracks} {map} {perBar} />
        {#if boxFrame}
          <div
            class="box-frame"
            style:left="{boxFrame.x}px"
            style:top="{boxFrame.y}px"
            style:width="{boxFrame.w}px"
            style:height="{boxFrame.h}px"
            data-test="arrange-box"
          ></div>
        {/if}
        {#if switcher}
          <div class="switcher" role="presentation" data-over-clips onpointerdown={(e) => e.stopPropagation()} style:left="{switcher.x}px" style:top="{switcher.y}px" style:height="{switchH}px">
            <button class="tog step" aria-label={t("arrange.takePrev")} onclick={() => send({ type: "stepTake", delta: -1 })}>◂</button>
            <span class="m-hi">
              {#each textParts("arrange.takeSwitch", { take: switcher.take, takes: switcher.takes }) as part, i (i)}
                {#if part.hole}<span class="m-num">{part.text}</span>{:else}{@render termed(part.text, "テイク")}{/if}
              {/each}
            </span>
            <button class="tog step" aria-label={t("arrange.takeNext")} onclick={() => send({ type: "stepTake", delta: 1 })}>▸</button>
            <span class="m-num m-lbl">{t("arrange.takeKey")}</span>
          </div>
        {/if}
      </div>
      </div>
        <HScrollbar total={Math.max(scrollRight, view.scrollX) + viewWidth} visible={viewWidth} offset={view.scrollX} onscroll={barScroll} />
        <VScrollbar total={rowsTotal} visible={rowsHeight} offset={shownY} onscroll={rowsScroll} test="arrange-vscrollbar" />
      </div>
      </div>
      {#if readingBox}
        <div
          class="m-num reading"
          style:left="{readingBox.left}px"
          style:top="{readingBox.top}px"
          bind:clientWidth={readingW}
          bind:clientHeight={readingH}
          data-test="arrange-reading"
        >
          {dragReading.text}
        </div>
      {/if}
      <div class="play-line" class:rec={lineRec} bind:this={timeLine}></div>
      <div class="layer">{@render children?.()}</div>
    </div>
    {#if keysHere}<div class="focus-mark" data-test="arrange-focus-mark"></div>{/if}
  </section>
{/if}

<style>
  .arrange,
  .overview {
    flex-shrink: 0;
    display: flex;
    overflow: hidden;
    position: relative;
  }
  .layer {
    position: absolute;
    inset: 0;
    z-index: 20;
    pointer-events: none;
  }
  .layer > :global(*) {
    pointer-events: auto;
  }
  .fill {
    position: absolute;
    inset: 0;
  }

  .overview {
    background: var(--color-gray-3);
    border-bottom: var(--line-hair) solid var(--color-gray-6);
  }
  /* The mark of the band keys go to (B-72): a full-width accent line.mark on the heading row and the top edge of the ruler. Drawn on top without changing the height. */
  .focus-mark {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    height: var(--line-mark);
    background: var(--color-accent);
    pointer-events: none;
    z-index: 21;
  }
  .overview-label {
    position: relative;
    width: var(--mock-overview-label);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    padding: 0 var(--space-3);
    overflow: hidden;
    border-right: var(--line-hair) solid var(--color-gray-6);
  }
  .overview-slot {
    flex: 1;
    min-width: 0;
    position: relative;
    overflow: hidden;
  }

  .head {
    width: var(--size-col-head);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-gray-3);
    border-right: var(--line-hair) solid var(--color-gray-6);
    overflow: hidden;
  }
  /* The track head rows. Scrolled vertically by the same amount as the lanes (Arrange writes scrollTop). The 3 rows above do not move. */
  .heads-view {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
  .row {
    flex-shrink: 0;
    height: var(--size-row);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-2);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    white-space: nowrap;
  }
  /* − ＋ (mock R3-Arrange: 24×24, space.1 between buttons). Pushed right with the grid tag. */
  .zoom {
    margin-left: auto;
    display: flex;
    gap: var(--space-1);
  }
  .zoom-btn {
    width: var(--part-button-height);
    padding: 0;
  }
  .tempo-head {
    flex-shrink: 0;
    background: var(--color-gray-4);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .tempo-toggle {
    width: 100%;
    border-bottom: 0;
  }
  /* An open row (mock F6-Tempo): background gray.2, height size.row + size.lane, the same as the time side. */
  .tempo-head.open {
    background: var(--color-gray-2);
    height: calc(var(--size-row) + var(--size-lane));
  }
  .tempo-head.open .tempo-toggle {
    border-bottom: 0;
  }
  .tempo-sums {
    display: flex;
    flex-direction: column;
    padding: 0 var(--space-2) 0 var(--space-6);
    white-space: nowrap;
  }

  .time {
    flex: 1;
    min-width: 0;
    position: relative;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--color-gray-1);
  }
  .ruler {
    height: var(--size-row);
    flex-shrink: 0;
    position: relative;
    background: var(--color-gray-3);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .ruler.bars,
  .lanes {
    touch-action: none;
  }
  .ruler.bars {
    cursor: default;
  }
  .bar-strip {
    position: absolute;
    inset: 0;
  }
  .bar-number {
    position: absolute;
    top: var(--space-1);
    margin-left: var(--space-1);
  }
  .loop-mark {
    position: absolute;
    top: 0;
    height: var(--mock-arrange-loop);
    background: var(--color-accent);
  }
  .lanes-wrap {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    position: relative;
  }
  /* The track row area. The horizontal and vertical bars lie over this box. */
  .lanes-box {
    flex: 1;
    min-height: 0;
    position: relative;
    display: flex;
    flex-direction: column;
  }
  /* The visible part (bar lines fill it). The rows inside scroll vertically. */
  .lanes-view {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
  /* All rows (scrollable if taller than the visible part). Clip pictures and names start at the top left of this box. */
  .lanes {
    position: relative;
    min-height: 100%;
    box-sizing: border-box;
    padding-bottom: var(--mock-arrange-add-track);
  }
  .lane {
    height: var(--size-track);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  /* The time side of the 「▸ パン ▸ プラグインのパラメータ」 row (nothing is placed). */
  .add-row {
    height: var(--size-row);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .rec-host {
    position: absolute;
    inset: 0;
    z-index: 1;
    pointer-events: none;
  }
  .rec-tail,
  .drag-clip {
    position: absolute;
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
  .labels {
    position: absolute;
    inset: 0;
    z-index: 2;
    pointer-events: none;
  }
  .clip-label {
    position: absolute;
    height: var(--text-s-line-height);
    display: flex;
    align-items: flex-start;
    gap: var(--space-1);
    padding: 0 var(--space-1);
    overflow: hidden;
    white-space: nowrap;
    color: var(--color-gray-9);
  }
  .clip-label.recording {
    color: var(--color-rec);
  }
  .clip-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .take-tag {
    flex-shrink: 0;
    padding: 0 calc(var(--space-1) - var(--line-hair));
    border: var(--line-hair) solid var(--color-gray-8);
    line-height: calc(var(--text-s-line-height) - 2 * var(--line-hair));
    color: var(--color-gray-10);
  }
  /* The marks of an audio clip (spec 2: right of the name, background gray.0, border gray.7). In the same element as the tag, so the element count stays the clip count. */
  .clip-mark {
    flex-shrink: 0;
    padding: 0 var(--space-1);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-7);
    line-height: calc(var(--text-s-line-height) - 2 * var(--line-hair));
    color: var(--color-gray-10);
  }
  .take-tag.selected {
    border-color: var(--color-accent);
  }
  .loop-range {
    color: var(--color-gray-9);
  }
  .take-square {
    width: var(--part-take-mark);
    height: var(--part-take-mark);
    position: absolute;
    left: 0;
    top: 0;
    background: var(--color-gray-10);
  }
  /* The clip name box: the same as the track name box (gray.0, border gray.10, weight 600, height 20). It may stick out of a narrow clip. */
  .clip-naming {
    position: absolute;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: var(--space-1);
    width: max-content;
    pointer-events: auto;
  }
  .name-cell {
    flex: 1;
    display: grid;
  }
  .name-sizer,
  .name-field {
    grid-area: 1 / 1;
    box-sizing: border-box;
    height: var(--part-toggle-size);
    padding: 0 var(--space-1);
    border: var(--line-hair) solid var(--color-gray-10);
    font: inherit;
    font-weight: var(--text-s-weight-600);
    white-space: pre;
  }
  .name-sizer {
    visibility: hidden;
  }
  .name-field {
    width: 100%;
    min-width: 0;
    background: var(--color-gray-0);
    border-radius: var(--radius-1);
    color: var(--color-gray-10);
    outline: none;
  }
  /* The box-select frame (the same accent line.hair as the box select in the piano roll). */
  .box-frame {
    position: absolute;
    z-index: 3;
    border: var(--line-hair) solid var(--color-accent);
    pointer-events: none;
  }
  .switcher {
    position: absolute;
    z-index: 4;
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: 0 var(--space-1);
    background: var(--color-gray-4);
    border: var(--line-hair) solid var(--color-accent);
    white-space: nowrap;
    pointer-events: auto;
  }
  .play-line {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: var(--line-hair);
    z-index: 3;
    pointer-events: none;
    background: var(--color-accent);
  }
  .play-line.rec {
    background: var(--color-rec);
  }
</style>

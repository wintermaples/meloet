<!-- アレンジ（モック Main・F-ArrangeRec・F-Takes・F-Empty）。ちぢんだときは全体図（A-PianoRoll の上の帯）。
     小節の線、クリップ、全体図の線は WebGL。名前、札、再生の位置の線は HTML。children は、アレンジの上に重ねる物（出口えらび、案内）。
     動いている間は、WebGL の係のコマごとに（everyGlFrame）線と録っているクリップの先を動かす。
     クリップのドラッグ（モック R3-Arrange）は、もとの場所を大きな面に gray.7 の枠で残し、動いているクリップを小さな区画（recordingClip。
     録音中はドラッグしないので、録っているクリップの先と同じ時に使わない）で描く。動かすだけなら箱を transform で動かす。
     選んであるクリップは何個でも、いっしょに動かす（docs/design-notes.md B-71）。右クリックの一覧は B-60。 -->
<script lang="ts">
  import { onDestroy, onMount, untrack, type Snippet } from "svelte";
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
  } from "../bridge";
  import { everyGlFrame, glRegion, glSlot, type GlRect, type GlSlot } from "../gl/slot";
  import { platformOf } from "../keys/keys";
  import HScrollbar from "../lib/HScrollbar.svelte";
  import TrackHeads from "./TrackHeads.svelte";
  import VScrollbar from "./mixer/VScrollbar.svelte";
  import { term } from "../lib/tip.svelte";
  import { openContextMenu, type MenuEntry, type MenuRow } from "./menu/contextMenu.svelte";
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

  let { children }: { children?: Snippet } = $props();

  const song = $derived(current.state!.song);
  const timeline = $derived(current.state!.timeline);
  const transport = $derived(current.state!.transport);
  /** テイクの切りかえの札が向くクリップ（最後に押したクリップ）。 */
  const selected = $derived(current.state!.arrange.selectedClip);
  /** 選んであるクリップぜんぶ（仕様書 2 章「選んだ」の枠と上辺 accent）。 */
  const selectedSet = $derived(new Set(current.state!.arrange.selectedClips));
  const layout = $derived(current.state!.workspace.layout);
  /** キーがアレンジに向いているか（B-72。ピアノロールがひらいていなければ、いつもアレンジ）。 */
  const keysHere = $derived(prView.focus === "arrange" || !layout.lower.some((l) => l.band === "pianoRoll"));
  const height = $derived(metrics.scaled(layout.arrangeHeight));

  let timeWidth = $state(0);
  let overviewWidth = $state(0);
  let overviewHeight = $state(0);
  /** 横のスクロール。拡大率を変えても同じ所を見せるよう、ティックで持つ。 */
  let scrollTick = $state(0);
  /** 再生の位置を追ってページを送るか。手で横に動かすとやめ、次に再生を始めるとまた追う。 */
  let follow = true;

  /** 1 小節の px。横の拡大の段（arrangeView.barPx は 100 % の px）に、mock.arrange.bar と同じ率をかける。 */
  const barPx = $derived((metrics.px("mock.arrange.bar") * arrangeView.barPx) / DEFAULT_ARRANGE_BAR);
  /** 小節の番号を何小節おきに出すか。縮小して重なるときは間引く。 */
  const barLabelEvery = $derived(G.labelEvery(barPx, metrics.px("mock.arrange.bar") / 2));
  /** テンポと拍子の地図。小節の線、目盛の数字、吸いつき、ループの範囲は、ここから（拍子の変更で小節の長さが変わる）。 */
  const map = $derived(mapOf(timeline));
  /** 曲の頭の拍子の 1 小節。横の拡大の段（barPx）は、この長さの小節の px。 */
  const perBar = $derived(ticksPerBar({ num: timeline.meterNum, den: timeline.meterDen }));
  const pxPerTick = $derived(barPx / perBar);
  /** クリップの場所に拍の線も描くか。拍に吸いつくほど拡大していなければ、小節の線だけ。 */
  const gridBeats = $derived(G.beatsShown(barPx, perBar, ticksPerBeat({ num: timeline.meterNum, den: timeline.meterDen }), metrics.px("mock.arrange.bar") / 2));
  const viewWidth = $derived(layout.arrangeOverview ? overviewWidth : timeWidth);
  const view: G.View = $derived({ pxPerTick, scrollX: scrollTick * pxPerTick, width: viewWidth });
  /** トラックの行と、その下のオートメーションの帯（見出しの側と同じ並び。automation/autoGeom.ts）。 */
  const rows = $derived(
    arrangeRows(song.tracks, current.state!.automation.folded, {
      trackH: metrics.px("size.track"),
      laneH: metrics.px("size.lane"),
      rowH: metrics.px("size.row"),
    }),
  );
  const dims: G.LaneDims = $derived({
    tops: rows.tops,
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

  /** 目盛の数字（小節の番号と、その線の x）。 */
  const barNumbers = $derived.by(() => {
    const out: { n: number; x: number }[] = [];
    eachLine(map, map.meterAt(G.xToTick(view, 0)).barTick, G.xToTick(view, viewWidth), (_, bar) => bar * barLabelEvery, (tick, _, bar) =>
      out.push({ n: bar + 1, x: G.tickToX(view, tick) }),
    );
    return out;
  });
  /** クリップの場所の小節の線（と拍の線）。 */
  const gridLines = $derived.by(() => {
    const out: GridLine[] = [];
    eachLine(map, G.xToTick(view, 0), G.xToTick(view, viewWidth), (beat, bar) => (gridBeats ? beat : bar), (tick, kind) =>
      out.push({ x: G.tickToX(view, tick), bar: kind === 0 }),
    );
    return out;
  });
  /** トラックの id から、並びの中の行。 */
  const rowOf = (track: number) => song.tracks.findIndex((tr) => tr.id === track);

  // ---- テイクのノート（一度取れば id で持っておける） ----

  let takeVersion = $state(0);
  const asked = new Set<number>();
  function takeOf(id: number): TakeNotes | undefined {
    const had = peekTakeNotes(id);
    if (had || asked.has(id)) return had;
    asked.add(id);
    try {
      // 届いたら asked から外す（あとで片づけられても、また要れば取りなおせるように）。
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

  // ---- 録っているノート ----

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
      // つながる前。次の状態でもう一度。
    }
    return book;
  }
  $effect(() => {
    void current.state;
    ensureBook();
  });
  onDestroy(() => stopBook?.());

  const rec = $derived(transport.state === "recording" ? transport.recording : null);
  /** いま描いている周と、大きな面に移した所まで（小節ごと）。 */
  let recShown = $state.raw<{ lap: number; lapFrom: number; commit: number } | null>(null);
  let lastLap = 0;
  let lastNow = 0;
  /** 録り終わってテイクが届くまで出しておく、最後に録っていたノート。 */
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
    const take = takeOf(clip.takeId);
    if (take) return G.takeShapes(clip, take);
    if (fallback?.track !== trackId) return [];
    return fallback.notes.filter((n) => n.start < clip.end && n.start + n.length > clip.start);
  }

  // ---- ループ ----

  const songLoop = $derived(song.loopOn ? G.loopBars(song.loopRange, map) : null);
  /** ドラッグ中の見た目。Rust の答え（次の差分）が来たら消す。 */
  let preview = $state<G.BarRange | null>(null);
  const shownLoop = $derived(preview ?? songLoop);
  $effect(() => {
    void current.rev;
    untrack(() => {
      if (!drag) preview = null;
    });
  });
  const loopX = (bar: number) => G.tickToX(view, barLineTick(map, bar));

  // ---- クリップのドラッグ（動かす、Alt で複製、はしをつめる。arrangeDrag.ts） ----

  let clipDrag = $state.raw<CD.ClipDrag | null>(null);
  /** 離して意図を送ったあと、答えの状態が届くまで残しておく見た目（ちらつかないように）。 */
  let clipPending = $state.raw<CD.ClipDrag | null>(null);
  let pendingRev = $state<number | null>(null);
  const shownDrag = $derived.by(() => {
    const d = clipDrag ?? clipPending;
    return d && d.press.moved ? d : null;
  });
  /**
   * 大きな面が見るのは、どのクリップか、もとの場所を枠にするか（複製でなければ）だけ。動かす量では描きなおさない。
   * どのクリップかは文字で持つ（同じなら、下の Set も大きな面も作りなおさない）。
   */
  const dragKey = $derived.by(() => {
    const d = shownDrag;
    if (!d) return "";
    return d.kind === "trim" ? String(d.clip.id) : d.group.map((g) => g.clip.id).join(",");
  });
  const dragIds = $derived(new Set(dragKey ? dragKey.split(",").map(Number) : []));
  const dragFrame = $derived(shownDrag !== null && (shownDrag.kind === "trim" || !shownDrag.copy));
  /** 動いている絵の形が変わる時だけ変わる（動かすだけなら変わらない）。 */
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

  /** 動いている絵: 小さな区画の箱（レーンの CSS px）、その中の四角、名前。 */
  const dragPicture = $derived.by(() => {
    void dragShape;
    void takeVersion;
    const d = untrack(() => shownDrag);
    if (!d) return null;
    const rects: GlRect[] = [];
    const labels: Label[] = [];
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
    });
    let place: { left: number; top: number; width: number; height: number };
    if (d.kind === "move") {
      // いちばん前のクリップの始まりと、いちばん上の行を 0 にして、クリップぜんぶを描く（動かすと見えてくる所も）。
      const first = Math.min(...d.group.map((g) => g.clip.start));
      const last = Math.max(...d.group.map((g) => g.clip.end));
      const top = G.clipBox(view, dims, Math.min(...d.group.map((g) => g.row)), first, last).y0;
      const bottom = G.clipBox(view, dims, Math.max(...d.group.map((g) => g.row)), first, last).y1;
      const local: G.View = { pxPerTick, scrollX: first * pxPerTick, width: Infinity };
      for (const { clip, row } of d.group) {
        const inner = G.clipBox(local, dims, row, clip.start, clip.end);
        const colors = G.clipColorsFor(G.CLIP_SELECTED, song.tracks[row]?.muted ?? false);
        G.paintClip(rects, local, dims, { ...inner, y0: inner.y0 - top, y1: inner.y1 - top }, colors, notesOf(row, clip));
        labels.push(labelOf(clip, G.clipBox(view, dims, row, clip.start, clip.end)));
      }
      place = { left: G.tickToX(view, first), top, width: (last - first) * pxPerTick, height: bottom - top };
    } else {
      // つめているクリップは行の幅いっぱいの箱に。テイクの 0 は動かさない（ノートは曲の中の同じ所に残る）。
      const take = takeOf(d.clip.takeId);
      const clip = { start: d.start, end: d.end, takeOffset: d.clip.start + d.clip.takeOffset - d.start };
      const box = G.clipBox(view, dims, d.row, d.start, d.end);
      const notes = take ? G.takeShapes(clip, take) : notesOf(d.row, d.clip).filter((n) => n.start >= d.start && n.start < d.end);
      G.paintClip(rects, view, dims, box, G.clipColorsFor(G.CLIP_SELECTED, song.tracks[d.row]?.muted ?? false), notes);
      for (const q of rects) q.y -= box.y0;
      place = { left: 0, top: box.y0, width: view.width, height: box.y1 - box.y0 };
      labels.push(labelOf(d.clip, box));
    }
    return { place, rects, labels };
  });

  let dragSlot: { update(next: GlSlot): void } | null = null;
  function draggedClip(node: HTMLElement) {
    const slot = glSlot(node, { kind: "rects", rects: dragPicture?.rects ?? [] });
    dragSlot = slot;
    return {
      destroy() {
        slot.destroy();
        if (dragSlot === slot) dragSlot = null;
      },
    };
  }
  // 箱を transform で動かしたときも、区画の canvas を動いた箱に合わせなおす（描く中身は同じなので、描きなおしはしない）。
  $effect(() => {
    const p = dragPicture;
    void dragShift;
    if (p) untrack(() => dragSlot?.update({ kind: "rects", rects: p.rects }));
  });

  // ---- 大きな面: クリップ、録っているクリップのできた部分、ループのはし ----

  const laneRects = $derived.by(() => {
    void takeVersion;
    void liveVersion;
    const out: GlRect[] = [];
    song.tracks.forEach((track, row) => {
      for (const clip of track.clips) {
        const box = G.clipBox(view, dims, row, clip.start, clip.end);
        if (box.x1 < 0 || box.x0 > view.width) continue;
        const dragged = dragIds.has(clip.id);
        if (dragged && dragFrame) {
          G.paintFrame(out, view, dims, box, "color.gray.7");
          continue;
        }
        const colors = selectedSet.has(clip.id) && !dragged ? G.CLIP_SELECTED : G.CLIP_PLAIN(G.trackColor(track));
        G.paintClip(out, view, dims, box, G.clipColorsFor(colors, track.muted), shapesOf(track.id, clip));
      }
    });
    const shown = recShown;
    if (rec && shown && shown.commit > shown.lapFrom) {
      const row = rowOf(rec.track);
      const all = G.liveShapes(book?.notesOf(rec.id, shown.lap) ?? [], shown.commit);
      const box = G.clipBox(view, dims, row, shown.lapFrom, shown.commit);
      if (row >= 0) {
        G.paintClip(
          out,
          view,
          dims,
          box,
          G.CLIP_RECORDING,
          all.filter((n) => n.start < shown.commit),
          { openRight: true, range: G.keyRange(all), until: shown.commit },
        );
      }
    }
    if (shownLoop) {
      const h = rows.height;
      for (const bar of [shownLoop.from, shownLoop.to]) {
        const x = loopX(bar);
        if (x >= 0 && x < view.width) out.push({ x, y: 0, w: dims.hair, h, color: "color.gray.7" });
      }
    }
    return out;
  });

  // ---- 名前と札（HTML） ----

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
  }
  const labels = $derived.by(() => {
    const out: Label[] = [];
    const shown = recShown;
    song.tracks.forEach((track, row) => {
      for (const clip of track.clips) {
        const box = G.clipBox(view, dims, row, clip.start, clip.end);
        if (box.x1 < 0 || box.x0 > view.width) continue;
        // 録っているクリップの下になったクリップの名前は出さない。
        if (rec && shown && rec.track === track.id && clip.start < shown.commit + perBar && clip.end > shown.lapFrom) continue;
        // 動かしている・つめているクリップの名前は、動いている絵の側に出す。
        if (dragIds.has(clip.id) && dragFrame) continue;
        const x = Math.max(box.x0, 0) + dims.hair;
        const w = box.x1 - Math.max(box.x0, 0) - 2 * dims.hair;
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
        });
      }
    });
    return out;
  });

  /** えらんだクリップのテイクが 2 つ以上なら、すぐ上に切りかえの札。 */
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

  // ---- 全体図 ----

  /**
   * ピアノロールがひらいているとき、全体図の「見えている範囲の枠」はピアノロールで見えている範囲（仕様書 3 章）。
   * ひらいていない（ミキサーなどで全体図になった）ときは、今までどおり全体図の箱いっぱい（overview の slot）。
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

  // ピアノロールが横に送られて枠が全体図の外へ出たら、全体図を送って枠を見せる（枠をドラッグしている間はしない）。
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
  // Esc: 枠のドラッグをやめて、動かす前の所へ戻す（仕様書 4 章）。
  onMount(() =>
    onDragCancel(() => {
      if (!frameDrag) return false;
      scrollToTick(frameDrag.tick);
      frameDrag = null;
      return true;
    }),
  );

  // ---- 動く物: 再生の位置の線、ページ送り、録っているクリップの先 ----

  let timeLine = $state<HTMLElement | null>(null);
  let overviewLine = $state<HTMLElement | null>(null);
  let recLabel = $state<HTMLElement | null>(null);
  let tailNode: HTMLElement | null = null;
  let tailSlot: { update(next: GlSlot): void } | null = null;
  let offsetUs: number | null = null;

  /** 録っているクリップの、のびている先（区画 recordingClip の中。区画の箱は録っていない間もある）。 */
  function recordingTail(node: HTMLElement) {
    const slot = glSlot(node, { kind: "rects", rects: [] });
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

  // 動いている間は毎コマ呼ばれる。画面の画素で変わったときだけ書く（同じ値でも書けば、ページはそのコマを作りなおす）。
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

  /** いま描いてある、のびている先の形。どれも同じなら、四角を作りなおさない。 */
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
      // 名前は、クリップの右はしで切る。名前の全部が入る幅になったら、もう書かない（書けば、そのたびにページを描きなおす）。
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
    tailSlot.update({ kind: "rects", rects });
  }

  // 先は毎コマ描くので、周のノートと鍵の範囲は、録っているノートが変わったときだけ作る（docs/architecture.md 7 章）。
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
    showNow(tick);
  }

  // ---- たたんだ「テンポ ・ 拍子の変更」の行の、再生の線がいる所のテンポと拍子（動いている間は毎コマ。文字が変わったときだけ書く） ----

  function nowText(tick: number): string {
    const at = Math.max(0, tick);
    const m = map.meterAt(at);
    return t("arrange.tempoAndSignature", { tempo: tempoText(Math.round(map.tempoAt(at) * 100) / 100), signature: `${m.num}/${m.den}` });
  }
  let now = $state("");

  /** 動いている間は 1 秒に 10 回まで書く（毎コマ書くと、再生の線の四角を切って、描きなおしの四角がふえる。2026-11-20-round6-step1.md の 5）。 */
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

  // ---- 手で: 横のスクロール、目盛のドラッグ、クリップをえらぶ ----

  const mac = platformOf(navigator) === "mac";
  let zoomWheel = 0;

  /** 横の拡大 ・ 縮小を by 段。x（見えている所の左はしからの px）の位置が、画面の同じ所に残るように送る。 */
  function zoomAt(by: number, x: number): void {
    const anchor = G.xToTick(view, x);
    if (!zoomArrangeBy(by)) return;
    scrollTick = Math.max(0, anchor - x / pxPerTick);
  }

  /** ホイール。Mod を押しながらなら、カーソルの所を中心に横の拡大 ・ 縮小（仕様書 3 章）。 */
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

  // ---- トラックの行を縦に送る（仮の形。user:2026-11-20。見出しの列とレーンを同じだけ。上の目盛 ・ マーカー ・ テンポの行は動かない） ----
  // 送った位置は画面だけが持つ（曲にも、取り消しにも入らない）。送るのは、ブラウザのスクロール（scrollTop）: 見えない行は
  // content-visibility で描かれず、WebGL の印は箱ごと動く（gl/engine.ts は data-gl-clip の箱で切る）。

  let lanesView = $state<HTMLElement>();
  let headsView = $state<HTMLElement>();
  let rowsHeight = $state(0);
  let scrollY = $state(0);
  /** 行ぜんぶの高さ。見出しの列のいちばん下には「＋ トラック」の行があるので、レーンの側も同じだけあける。 */
  const rowsTotal = $derived(rows.height + metrics.px("mock.arrange.add-track"));
  const maxScrollY = $derived(Math.max(0, rowsTotal - rowsHeight));
  /** 行がへったり窓が高くなったりしたら、下のはしで止める。 */
  const shownY = $derived(Math.min(scrollY, maxScrollY));
  $effect(() => {
    const y = shownY;
    if (lanesView) lanesView.scrollTop = y;
    if (headsView) headsView.scrollTop = y;
  });
  const rowsScroll = (y: number) => (scrollY = Math.max(0, Math.min(y, maxScrollY)));

  /** ホイールで縦に送る。クリップのドラッグ中と囲んでえらぶ間は送らない（つかんだ物が、ポインタの下からずれる）。送ったら true。 */
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

  // えらんだトラックが変わった時（キー、足した時）は、そのトラックの行と帯が見える所まで送る。再生や録音では送らない。
  let seenTarget: number | null | undefined = undefined;
  let seenTracks = 0;
  $effect(() => {
    const target = song.playTarget;
    const count = song.tracks.length;
    untrack(() => {
      const row = song.tracks.findIndex((tr) => tr.id === target);
      if (seenTarget !== undefined && (target !== seenTarget || count > seenTracks) && row >= 0) {
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
    // 目盛に見えている線に吸いつく（B-61）: 拍の線が見えていれば拍、ふだんは小節、間引いていれば番号のある小節。
    const every = barLabelEvery;
    const step = e.shiftKey
      ? null
      : gridBeats
        ? (at: number) => snapToStep(map, at, 0)
        : (at: number) => {
            const m = map.meterAt(Math.max(0, at));
            const line = Math.round((m.bar + (Math.max(0, at) - m.barTick) / ticksPerBar(m)) / every) * every;
            return map.tickOfBar(line);
          };
    const command = G.rulerCommand(drag.d, view, drag.downX, e.clientX - drag.left, map, still(), step);
    drag = null;
    if (command.type === "locate") preview = null;
    const before = current.rev;
    // 範囲が変わらなければ差分は来ないので、返事の rev で見た目を消す。
    void send(command).then((rev) => {
      if (rev === before && !drag) preview = null;
    });
  }
  function rulerCancel(): boolean {
    if (!drag) return false;
    drag = null;
    preview = null;
    return true;
  }
  // Esc: ループの範囲のドラッグをやめて、前の範囲に戻す（仕様書 4 章）。
  onMount(() => onDragCancel(rulerCancel));

  // クリップを押す: えらぶ。押したまま動かす: 動かす（Alt で複製）、左右のはし（part.clip.edge）なら長さ。録音中とカウントイン中はえらぶだけ。
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

  /** 空いた所から押したまま動かして囲む（B-71）。x0 ・ y0 は押した所、add は Shift。 */
  let boxSel = $state<{ x0: number; y0: number; x: number; y: number; moved: boolean; add: boolean } | null>(null);
  const boxFrame = $derived(
    boxSel?.moved
      ? { x: Math.min(boxSel.x0, boxSel.x), y: Math.min(boxSel.y0, boxSel.y), w: Math.abs(boxSel.x - boxSel.x0), h: Math.abs(boxSel.y - boxSel.y0) }
      : null,
  );
  /** 何個も選んでいる中の 1 つを押した。動かさずに離したら、それだけを選ぶ。 */
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

  // ---- 右クリックの一覧（B-60）。選ばれていない物を右クリックしたら、それだけを選んでからひらく ----

  const platform = platformOf(navigator);
  const modKey = (letter: string) => `${platform === "mac" ? t("key.cmd") : t("key.ctrl")} ${letter}`;
  const shiftKey = (letter: string) => `${t("key.shift")} ${letter}`;
  const menuRow = (label: MenuRow["label"], usable: boolean, run: () => void, key?: string): MenuRow => ({ kind: "row", label, key, usable, run });
  /** 行を押したときの選択（右クリックで選びなおした分は、そのときには届いている）。 */
  const chosen = () => current.state!.arrange.selectedClips;
  const clipboardCount = $derived(current.state!.arrange.clipboardClips);

  /** 録音中とカウントイン中は、曲を変える行を使えない。 */
  function clipMenu(clip: ClipView): MenuEntry[] {
    const change = !busy;
    const open = () => {
      if (!pianoRollOpen) send({ type: "toggleBand", band: "pianoRoll" });
    };
    return [
      menuRow("menu.openInPianoRoll", true, open, t("key.pianoRoll")),
      menuRow("menu.cut", change, () => send({ type: "cutClips", clips: chosen() }), modKey(t("key.cut"))),
      menuRow("menu.copy", true, () => send({ type: "copyClips", clips: chosen() }), modKey(t("key.copy"))),
      menuRow("menu.paste", change && clipboardCount > 0, () => send({ type: "pasteClips", to: null }), modKey(t("key.paste"))),
      menuRow("menu.duplicate", change, () => send({ type: "duplicateClips", clips: chosen() }), modKey(t("key.duplicate"))),
      menuRow("menu.delete", change, () => send({ type: "deleteSelected" }), t("key.delete")),
      { kind: "separator" },
      menuRow("menu.nextTake", change && clip.takes > 1, () => send({ type: "stepTake", delta: 1 }), t("key.take")),
      menuRow("menu.prevTake", change && clip.takes > 1, () => send({ type: "stepTake", delta: -1 }), shiftKey(t("key.take"))),
      menuRow("menu.deleteTake", change && clip.takes > 1, () => send({ type: "deleteTake", clip: clip.id })),
    ];
  }

  function lanesContext(e: MouseEvent): void {
    e.preventDefault();
    if (clipDrag || boxSel) return;
    const p = lanePoint(e);
    const hit = hitAt(p);
    if (hit) {
      pickClips();
      if (!selectedSet.has(hit.clip.id)) send({ type: "selectClip", clip: hit.clip.id });
      openContextMenu(e.clientX, e.clientY, clipMenu(hit.clip));
      return;
    }
    // 押した所を、クリップを動かすときと同じグリッド（ふだん 1 小節）に寄せる。Shift なら寄せない。
    const track = song.tracks[G.trackRowAt(dims, p.y)];
    const at = G.xToTick(view, p.x);
    const tick = Math.max(0, e.shiftKey ? Math.round(at) : nearestBarTick(map, at));
    const paste = () => {
      if (track) send({ type: "pasteClips", to: { track: track.id, tick } });
    };
    openContextMenu(e.clientX, e.clientY, [
      menuRow("menu.paste", !busy && track !== undefined && clipboardCount > 0, paste, modKey(t("key.paste"))),
    ]);
  }

  const readingBox = $derived.by(() => {
    if (!readingAt || !dragReading.text) return null;
    const off = metrics.px("part.reading.offset");
    return { left: Math.max(0, readingAt.x - off - readingW), top: Math.max(0, readingAt.y - off - readingH) };
  });

  const lineRec = $derived(transport.state === "countIn" || transport.state === "recording");

  // ---- 横の棒 ----

  let timeBox = $state<HTMLElement>();
  const scrollRight = $derived(G.scrollMax(song.tracks, song.loopRange, transport.position, { pxPerTick, width: viewWidth }));
  function barScroll(offset: number): void {
    scrollTick = offset / pxPerTick;
    follow = false;
  }

  /** Mod ＋ ・ Mod − の行き先（docs: pianoroll/view.svelte.ts の focus）。アレンジを押したら、アレンジへ。 */
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
        {#each barNumbers as b (b.n)}
          <span class="m-num m-lbl bar-number" style:left="{b.x}px">{b.n}</span>
        {/each}
        {#if shownLoop}
          <div class="loop-mark" style:left="{loopX(shownLoop.from)}px" style:width="{loopX(shownLoop.to) - loopX(shownLoop.from)}px"></div>
        {/if}
      </div>
      <MarkerRow {view} {map} {timeline} {busy} />
      <!-- テンポの帯の線と点も、クリップと同じ arrangeLanes の 1 枚に描く（区画をふやさない）。区画の箱は、帯とクリップの場所を囲む。 -->
      <div class="lanes-wrap" use:glRegion={"arrangeLanes"}>
      <TempoBand {view} {map} {timeline} {now} {busy} />
      <div class="lanes-box">
      <div class="lanes-view" data-gl-clip bind:this={lanesView} bind:clientHeight={rowsHeight} use:glSlot={{ kind: "gridLines", lines: gridLines, beats: gridBeats }}>
      <div
        class="lanes"
        role="presentation"
        bind:this={lanesBox}
        style:cursor={laneCursor}
        onpointerdown={lanesDown}
        onpointermove={lanesMove}
        onpointerup={lanesUp}
        onpointercancel={cancelClipDrag}
        oncontextmenu={lanesContext}
      >
        {#each song.tracks as track (track.id)}
          <div class="lane"></div>
          {#each rows.lanes.filter((l) => l.track.id === track.id) as l (JSON.stringify(l.lane.target))}
            <AutoLane {track} lane={l.lane} {view} {map} {busy} />
          {/each}
          {#if showsAddRow(track)}<div class="add-row"></div>{/if}
        {/each}
        <div class="fill" use:glSlot={{ kind: "rects", rects: laneRects }}></div>
        <div class="rec-host" use:glRegion={"recordingClip"}>
          {#if rec}
            <div class="rec-tail" use:recordingTail></div>
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
                <span class="clip-name">{c.name}</span>
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
              {t("arrange.recordingClip", { take: rec.take })}
            </div>
          {/if}
          {#each dragPicture?.labels ?? [] as c (c.id)}
            <div class="clip-label" style:left="{c.x}px" style:top="{c.y}px" style:width="{c.w}px" style:transform={dragShift}>
              {#if c.mark === "square"}
                <span class="take-square"></span>
              {:else}
                {#if c.mark === "tag"}
                  <span class="m-num take-tag selected">{t("arrange.takeTag", { take: c.take, takes: c.takes })}</span>
                {/if}
                <span class="clip-name">{c.name}</span>
              {/if}
            </div>
          {/each}
        </div>
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
          <div class="switcher" role="presentation" onpointerdown={(e) => e.stopPropagation()} style:left="{switcher.x}px" style:top="{switcher.y}px" style:height="{switchH}px">
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
  /* キーが向く帯の印（B-72）: 見出しの行と目盛の上辺に、横いっぱいの accent の line.mark。高さは変えず、重ねて描く。 */
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
  /* トラックの見出しの行。レーンと同じだけ縦に送る（scrollTop は Arrange が書く）。上の 3 行は動かない。 */
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
  /* − ＋（モック R3-Arrange: 24×24、ボタンどうしは space.1）。グリッドの札とともに右へ寄せる。 */
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
  /* ひらいた行（モック F6-Tempo）: 地 gray.2、高さは時間の側と同じ size.row ＋ size.lane。 */
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
  /* トラックの行の場所。横と縦の棒は、この箱に重ねる。 */
  .lanes-box {
    flex: 1;
    min-height: 0;
    position: relative;
    display: flex;
    flex-direction: column;
  }
  /* 見えている所（小節の線は、ここいっぱいに描く）。中の行は縦に送る。 */
  .lanes-view {
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }
  /* 行ぜんぶ（見えている所より高ければ、送れる）。クリップの絵と名前は、この箱の左上から。 */
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
  /* 「▸ パン ▸ プラグインのパラメータ」の行の時間の側（何も置かない）。 */
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
  /* 囲む枠（ピアノロールの囲みと同じ accent の line.hair）。 */
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

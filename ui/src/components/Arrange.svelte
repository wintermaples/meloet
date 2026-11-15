<!-- アレンジ（モック Main・F-ArrangeRec・F-Takes・F-Empty）。ちぢんだときは全体図（A-PianoRoll の上の帯）。
     小節の線、クリップ、全体図の線は WebGL。名前、札、再生の位置の線は HTML。children は、アレンジの上に重ねる物（出口えらび、案内）。
     動いている間は、WebGL の係のコマごとに（everyGlFrame）線と録っているクリップの先を動かす。 -->
<script lang="ts">
  import { onDestroy, untrack, type Snippet } from "svelte";
  import {
    clockOffsetUs,
    liveNotes,
    peekTakeNotes,
    takeNotes,
    type ClipView,
    type LiveNotesBook,
    type RecordingView,
    type TakeNotes,
  } from "../bridge";
  import { everyGlFrame, glRegion, glSlot, type GlRect, type GlSlot } from "../gl/slot";
  import { platformOf } from "../keys/keys";
  import HScrollbar from "../lib/HScrollbar.svelte";
  import { term } from "../lib/tip.svelte";
  import { current, send } from "../state/app.svelte";
  import { t } from "../text/ja";
  import * as G from "./arrangeGeom";
  import { dbNumber, gridText, tempoText, textParts } from "./format";
  import { metrics } from "./metrics.svelte";
  import { arrangeView, DEFAULT_ARRANGE_BAR, prView, scrollToTick, visibleTicks, zoomArrangeBy } from "./pianoroll/view.svelte";
  import { shownTick, ticksPerBar } from "./time";

  let { children }: { children?: Snippet } = $props();

  const song = $derived(current.state!.song);
  const transport = $derived(current.state!.transport);
  const selected = $derived(current.state!.arrange.selectedClip);
  const layout = $derived(current.state!.workspace.layout);
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
  const perBar = $derived(ticksPerBar({ num: song.meterNum, den: song.meterDen }));
  const pxPerTick = $derived(barPx / perBar);
  const viewWidth = $derived(layout.arrangeOverview ? overviewWidth : timeWidth);
  const view: G.View = $derived({ pxPerTick, scrollX: scrollTick * pxPerTick, width: viewWidth });
  const dims: G.LaneDims = $derived({
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

  const firstBar = $derived(Math.max(1, Math.floor(view.scrollX / barPx) + 1));
  const lastBar = $derived(Math.max(firstBar, Math.ceil((view.scrollX + viewWidth) / barPx)));
  const barNumbers = $derived(
    Array.from({ length: lastBar - firstBar + 1 }, (_, i) => firstBar + i).filter((n) => (n - 1) % barLabelEvery === 0),
  );
  const rowOf = (track: number) => song.tracks.findIndex((tr) => tr.number === track);

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

  function shapesOf(trackNumber: number, clip: ClipView): G.NoteShape[] {
    const take = takeOf(clip.takeId);
    if (take) return G.takeShapes(clip, take);
    if (fallback?.track !== trackNumber) return [];
    return fallback.notes.filter((n) => n.start < clip.end && n.start + n.length > clip.start);
  }

  // ---- ループ ----

  const songLoop = $derived(song.loopOn ? G.loopBars(song.loopRange, perBar) : null);
  /** ドラッグ中の見た目。Rust の答え（次の差分）が来たら消す。 */
  let preview = $state<G.BarRange | null>(null);
  const shownLoop = $derived(preview ?? songLoop);
  $effect(() => {
    void current.rev;
    untrack(() => {
      if (!drag) preview = null;
    });
  });
  const loopX = (bar: number) => G.tickToX(view, (bar - 1) * perBar);

  // ---- 大きな面: クリップ、録っているクリップのできた部分、ループのはし ----

  const laneRects = $derived.by(() => {
    void takeVersion;
    void liveVersion;
    const out: GlRect[] = [];
    song.tracks.forEach((track, row) => {
      for (const clip of track.clips) {
        const box = G.clipBox(view, dims, row, clip.start, clip.end);
        if (box.x1 < 0 || box.x0 > view.width) continue;
        const colors = clip.id === selected ? G.CLIP_SELECTED : G.CLIP_PLAIN(G.trackColor(track));
        G.paintClip(out, view, dims, box, colors, shapesOf(track.number, clip));
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
      const h = song.tracks.length * dims.trackH;
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
        if (rec && shown && rec.track === track.number && clip.start < shown.commit + perBar && clip.end > shown.lapFrom) continue;
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
          selected: clip.id === selected,
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

  // ---- 動く物: 再生の位置の線、ページ送り、録っているクリップの先 ----

  let timeLine = $state<HTMLElement | null>(null);
  let overviewLine = $state<HTMLElement | null>(null);
  let recLabel = $state<HTMLElement | null>(null);
  let tailNode: HTMLElement | null = null;
  let tailSlot: { update(next: GlSlot): void } | null = null;
  let offsetUs: number | null = null;

  /** 録っているクリップの、のびている先だけの区画（recordingClip）。 */
  function recordingTail(node: HTMLElement) {
    const region = glRegion(node, "recordingClip");
    const slot = glSlot(node, { kind: "rects", rects: [] });
    tailNode = node;
    tailSlot = slot;
    tailDrawn.row = -1;
    return {
      destroy() {
        slot.destroy();
        region.destroy();
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
  const tailDrawn = { nowPx: NaN, scrollX: NaN, live: -1, lapFrom: NaN, commit: NaN, row: -1, pxPerTick: NaN, trackH: NaN, labelPx: NaN };
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
      d.trackH === dims.trackH
    ) {
      return;
    }
    const placeChanged = d.scrollX !== scrollX || d.commit !== commit || d.lapFrom !== lapFrom || d.row !== row || d.pxPerTick !== pxPerTick || d.trackH !== dims.trackH;
    d.nowPx = nowPx;
    d.scrollX = scrollX;
    d.live = liveVersion;
    d.lapFrom = lapFrom;
    d.commit = commit;
    d.row = row;
    d.pxPerTick = pxPerTick;
    d.trackH = dims.trackH;

    const v: G.View = { pxPerTick, scrollX, width: viewWidth };
    const box = G.clipBox(v, dims, row, tailFrom, at);
    if (placeChanged) {
      setStyle(tailNode, "left", box.x0);
      setStyle(tailNode, "top", box.y0);
      setStyle(tailNode, "width", 2 * perBar * pxPerTick + dims.hair);
      setStyle(tailNode, "height", box.y1 - box.y0);
    }
    const all = G.liveShapes(book?.notesOf(r.id, lap) ?? [], at);
    const rects: GlRect[] = [];
    G.paintClip(rects, v, dims, box, G.CLIP_RECORDING, all, { openLeft: tailFrom > lapFrom, range: G.keyRange(all), until: at });
    for (const q of rects) {
      q.x -= box.x0;
      q.y -= box.y0;
    }
    tailSlot.update({ kind: "rects", rects });
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
  }

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
    if (dx === 0 && e.shiftKey) dx = e.deltaY;
    if (dx === 0) return;
    e.preventDefault();
    const unit = e.deltaMode === 1 ? metrics.px("size.row") : e.deltaMode === 2 ? viewWidth : 1;
    const max = G.scrollMax(song.tracks, song.loopRange, transport.position, { pxPerTick, width: viewWidth });
    scrollTick = G.wheelScroll(view.scrollX, dx * unit, max) / pxPerTick;
    follow = false;
  }

  let drag: { d: G.RulerDrag; downX: number; left: number } | null = null;
  const still = () => metrics.px("space.1");

  function rulerDown(e: PointerEvent): void {
    if (e.button !== 0) return;
    const left = (e.currentTarget as HTMLElement).getBoundingClientRect().left;
    const x = e.clientX - left;
    drag = { d: G.startRulerDrag(view, x, perBar, songLoop, metrics.px("space.2")), downX: x, left };
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
  }
  function rulerMove(e: PointerEvent): void {
    if (!drag) return;
    const x = e.clientX - drag.left;
    if (Math.abs(x - drag.downX) >= still()) preview = G.dragRange(drag.d, view, x, perBar);
  }
  function rulerUp(e: PointerEvent): void {
    if (!drag) return;
    const command = G.rulerCommand(drag.d, view, drag.downX, e.clientX - drag.left, perBar, still());
    drag = null;
    if (command.type === "locate") preview = null;
    const before = current.rev;
    // 範囲が変わらなければ差分は来ないので、返事の rev で見た目を消す。
    void send(command).then((rev) => {
      if (rev === before && !drag) preview = null;
    });
  }
  function rulerCancel(): void {
    drag = null;
    preview = null;
  }

  function lanesDown(e: PointerEvent): void {
    if (e.button !== 0) return;
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const id = G.clipAt(view, dims, song.tracks, e.clientX - r.left, e.clientY - r.top);
    if (id !== selected) send({ type: "selectClip", clip: id });
  }

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

{#if layout.arrangeOverview}
  <section class="overview" style:height="{height}px" onwheel={wheelOn(() => overviewSlot)} onpointerdowncapture={focusArrange}>
    <div class="overview-label"><span class="m-lbl">{t("arrange.overview")}</span></div>
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
  </section>
{:else}
  <section class="arrange" style:height="{height}px" onpointerdowncapture={focusArrange}>
    <div class="head" use:glRegion={"trackMeters"}>
      <div class="row">
        <span class="m-name">{t("arrange.title")}</span>
        <span class="zoom">
          <button class="m-btn zoom-btn" aria-label={t("zoom.out")} onclick={() => zoomArrangeBy(-1)}>{t("zoom.outMark")}</button>
          <button class="m-btn zoom-btn" aria-label={t("zoom.in")} onclick={() => zoomArrangeBy(1)}>{t("zoom.inMark")}</button>
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
      <button class="m-plain row tempo-head" aria-disabled="true" tabindex="-1">
        <span>▸</span>
        <span>{t("arrange.tempoChanges")}</span>
        <span class="m-spacer"></span>
        <span class="m-num">{t("arrange.changeCount", { count: song.tempoChanges })}</span>
      </button>
      {#each song.tracks as track (track.number)}
        <div class="track-head">
          <div class="track-color" style:background="var(--color-track-{((track.color - 1) % 5) + 1})"></div>
          <div class="track-body">
            <div class="line">
              <span class="m-num m-lbl track-number">{track.number}</span>
              <span class="m-name track-name">{track.name}</span>
              <button class="tog" aria-disabled="true" tabindex="-1" aria-label={t("arrange.mute")} use:term={"ミュート"}>{t("arrange.muteShort")}</button>
              <button class="tog" aria-disabled="true" tabindex="-1" aria-label={t("arrange.solo")} use:term={"ソロ"}>{t("arrange.soloShort")}</button>
              <button
                class="tog arm"
                class:on={track.armed}
                aria-pressed={track.armed}
                aria-label={t("arrange.arm")}
                use:term={"録音待機"}
                onclick={() => send({ type: "toggleArm", track: track.number })}>{t("arrange.armShort")}</button
              >
            </div>
            <div class="line gap2">
              <span class="m-lbl instrument" use:term={"仮の音"}>{t("instrument.placeholder")}</span>
              <span class="m-num">{t("common.db", { value: dbNumber(track.gainDb) })}</span>
            </div>
            <div class="meter" use:glSlot={{ kind: "meter", source: { track: track.number }, gap: metrics.px("line.hair") }}></div>
          </div>
        </div>
      {/each}
      <button class="m-plain add" aria-disabled="true" tabindex="-1">
        {t("arrange.addTrack")}<span>{t("arrange.addTrackKinds")}</span>
      </button>
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
        {#each barNumbers as n (n)}
          <span class="m-num m-lbl bar-number" style:left="{(n - 1) * barPx - view.scrollX}px">{n}</span>
        {/each}
        {#if shownLoop}
          <div class="loop-mark" style:left="{loopX(shownLoop.from)}px" style:width="{loopX(shownLoop.to) - loopX(shownLoop.from)}px"></div>
        {/if}
      </div>
      <div class="ruler"></div>
      <div class="ruler">
        <span class="m-num m-lbl bar-number" style:left="{-view.scrollX}px">
          {t("arrange.tempoAndSignature", { tempo: tempoText(song.tempoBpm), signature: `${song.meterNum}/${song.meterDen}` })}
        </span>
      </div>
      <div
        class="lanes"
        role="presentation"
        use:glRegion={"arrangeLanes"}
        use:glSlot={{ kind: "arrangeGrid", barPx, firstBarX: -view.scrollX }}
        onpointerdown={lanesDown}
      >
        {#each song.tracks as track (track.number)}
          <div class="lane"></div>
        {/each}
        <div class="fill" use:glSlot={{ kind: "rects", rects: laneRects }}></div>
        {#if rec}
          <div class="rec-tail" use:recordingTail></div>
        {/if}
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
              style:top="{row * dims.trackH + dims.inset + dims.mark}px"
            >
              {t("arrange.recordingClip", { take: rec.take })}
            </div>
          {/if}
        </div>
        {#if switcher}
          <div class="switcher" role="presentation" onpointerdown={(e) => e.stopPropagation()} style:left="{switcher.x}px" style:top="{switcher.y}px" style:height="{switchH}px">
            <button class="tog step" aria-label={t("arrange.takePrev")} onclick={() => send({ type: "stepTake", delta: -1 })}>◂</button>
            <span class="m-hi">
              {#each textParts("arrange.takeSwitch", { take: switcher.take, takes: switcher.takes }) as part, i (i)}
                {#if part.hole}<span class="m-num">{part.text}</span>{:else}{part.text}{/if}
              {/each}
            </span>
            <button class="tog step" aria-label={t("arrange.takeNext")} onclick={() => send({ type: "stepTake", delta: 1 })}>▸</button>
            <span class="m-num m-lbl">{t("arrange.takeKey")}</span>
          </div>
        {/if}
        <HScrollbar total={Math.max(scrollRight, view.scrollX) + viewWidth} visible={viewWidth} offset={view.scrollX} onscroll={barScroll} />
      </div>
      <div class="play-line" class:rec={lineRec} bind:this={timeLine}></div>
      <div class="layer">{@render children?.()}</div>
    </div>
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
  .overview-label {
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
    background: var(--color-gray-3);
    border-right: var(--line-hair) solid var(--color-gray-6);
    overflow: hidden;
  }
  .row {
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
    width: 100%;
    background: var(--color-gray-4);
    color: var(--color-gray-7);
  }

  .track-head {
    height: var(--size-track);
    display: flex;
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .track-color {
    width: var(--line-track);
    flex-shrink: 0;
  }
  .track-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-2) 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  .line.gap2 {
    gap: var(--space-2);
  }
  .track-number {
    width: var(--mock-track-number);
    flex-shrink: 0;
  }
  .track-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .instrument {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .tog {
    width: var(--part-toggle-size);
    height: var(--part-toggle-size);
    flex-shrink: 0;
    padding: 0;
    border: var(--line-hair) solid var(--color-gray-5);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-gray-7);
    font: inherit;
    text-align: center;
    cursor: default;
  }
  .tog.arm,
  .tog.step {
    border-color: var(--color-gray-6);
    color: var(--color-gray-8);
    cursor: pointer;
  }
  .tog.arm:hover,
  .tog.step:hover {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .tog.arm.on {
    border-color: var(--color-rec);
    color: var(--color-rec);
  }
  .meter {
    height: calc(var(--part-meter-horizontal) * 2 + var(--line-hair));
  }
  .add {
    width: 100%;
    height: var(--mock-arrange-add-track);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-2);
    color: var(--color-gray-7);
    cursor: default;
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
  .ruler.bars {
    cursor: default;
    touch-action: none;
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
  .lanes {
    flex: 1;
    min-height: 0;
    position: relative;
  }
  .lane {
    height: var(--size-track);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .rec-tail {
    position: absolute;
    z-index: 1;
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

<!-- ピアノロールのまん中（モック A-PianoRoll）: 目盛、鍵盤の列、ノートの場所、ベロシティの帯。
     ノートの場所とベロシティの帯は WebGL の 1 枚（pianoRoll）。ノートか見ている範囲が変わったときだけ描く。
     鍵盤の列は小さな 1 枚（pianoRollKeys）で、押している鍵と鳴っている音で光る。
     ドラッグ中のノートと、録音中にのびるノートは、それだけを囲む小さな 1 枚（pianoRollLive）。動かすだけなら transform で動かす。
     pianoRollLive の箱は、ノートの場所の左上から CC の帯の下はしまで（CC の帯のドラッグも同じ 1 枚に入れる）。
     再生の線、囲む枠、文字は HTML。
     行の高さは縦の拡大の段（prView.rowPx、design-notes B-63）。ノートの高さは行 − 1。縦の − ＋ は目盛の左の角。
     左の鍵盤の列を押すと、押している間だけ鳴らす（B-62。audition.ts）。目盛を押すと再生の位置、ドラッグでループの範囲（B-61）。
     右クリックは一覧（B-60。描くのは App の ContextMenu）。
     ドラムのトラックでは、行はキットの 12 の音だけ（鍵の列は楽器の名前、幅 part.drum.keys）、ノートは叩いた所の四角（drums.ts、geom.ts の drum）。
     ベロシティの帯は「▾ ベロシティ」でたたむ（仕様書 3 章の帯の表。ひらくのは CC の札の行の「▸ ベロシティ」）。たたんだ分はノートの場所へ。
     たたんでも区画（pianoRoll）の箱はそのままなので、WebGL の文脈は作りなおさない。 -->
<script lang="ts">
  import { labelsBesideMarkers } from "./rulerMarks";
  import { onDestroy, onMount, untrack } from "svelte";
  import { clockOffsetUs, liveNotes, type Command, type LiveNote, type LiveNotesBook } from "../../bridge";
  import type { ColorName } from "../../design/tokens";
  import { everyGlFrame, glRegion, glSlot, type GlSlot } from "../../gl/slot";
  import { platformOf } from "../../keys/keys";
  import HScrollbar from "../../lib/HScrollbar.svelte";
  import { term, tipAround } from "../../lib/tip.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { termParts, textParts } from "../format";
  import { closeContextMenu, openContextMenu } from "../menu/contextMenu.svelte";
  import * as AG from "../arrangeGeom";
  import { metrics } from "../metrics.svelte";
  import { shownTick, type Meter } from "../time";
  import { eachLine, gridTick, mapOf } from "../../time/bars";
  import CcLanes from "./CcLanes.svelte";
  import { cursorCss } from "./cursor";
  import { Auditioner, chordOf, fixedOr100, NUDGE_MS, notesMayAudition, onNudgeKeys, ticksToMs } from "./audition";
  import { pianoRollCommand } from "./commands";
  import { RollDrag, type CursorKind, type Drag, type Mods } from "./drag";
  import { drumKeysOf, drumName, drumTerm, rollShowsDrums } from "./drums";
  import * as G from "./geom";
  import { noteMarks, paintLive, paintRoll, paintRollKeys, paintVelocityLane, type LivePicture, type RollPicture, type RollSizes } from "./paint";
  import { rollMenu, type RollMenuActions } from "./rollMenu";
  import { shown } from "./shown.svelte";
  import { originFor, shownFor } from "./shownNotes";
  import { sameKeys, soundingKeys } from "./sounding";
  import {
    clampTopRow,
    dragReading,
    DRUM_ROW_MAX,
    DRUM_ROW_MIN,
    drumRowPx,
    drumView,
    onDragCancel,
    prPanels,
    prView,
    pxPerTick,
    ROW_PX_STEPS,
    rowHeight,
    scrollToTick,
    velocityLane,
    zoomBy,
    zoomRowsBy,
  } from "./view.svelte";

  let { onreading }: { onreading?: (at: { x: number; y: number } | null) => void } = $props();

  const app = $derived(current.state!);
  const song = $derived(app.song);
  const timeline = $derived(app.timeline);
  const pr = $derived(app.pianoRoll);
  const transport = $derived(app.transport);
  const meter: Meter = $derived({ num: timeline.meterNum, den: timeline.meterDen });
  /** テンポと拍子の地図（目盛、方眼、目盛を押したときの吸いつき、ループの範囲）。 */
  const map = $derived(mapOf(timeline));
  const scale = $derived(metrics.scale);

  /** ドラムの行を見せているか（仕様書 3 章「ドラムのピアノロール」）。 */
  const drums = $derived(rollShowsDrums(app, shown.clip, shown.track));
  /** ドラムの行の鍵（上から）。SF2 のキットは、そのキットが音を持つ鍵だけ。 */
  const drumKeys = $derived(
    drumKeysOf(shown.clip ? shown.track : (song.tracks.find((x) => x.id === song.playTarget) ?? null), drums),
  );
  $effect(() => {
    prView.drums = !!drums;
    drumView.rows = drumKeys.length;
  });
  const drumRowsPx = $derived(drumRowPx(drumKeys.length, scale));
  const rowH = $derived(drums ? (drumRowsPx * scale) / 100 : rowHeight(scale));
  const sizes: RollSizes = $derived({
    // 仕様書の size.note と part.note.height の差（行 − 1）を、どの段でも保つ。
    noteH: rowH - (metrics.px("size.note") - metrics.px("part.note.height")),
    edge: metrics.px("part.note.edge"),
    velBar: metrics.px("mock.pianoroll.vel-bar"),
    velCap: metrics.px("mock.pianoroll.vel-cap"),
    velFull: metrics.px("mock.pianoroll.vel-full"),
    velMid: metrics.px("mock.pianoroll.vel-mid"),
    endMark: metrics.px("mock.pianoroll.end-mark"),
    drumGap: metrics.px("part.drum.hit-gap"),
  });

  let area = $state<HTMLElement>();
  let liveHost = $state<HTMLElement>();
  let areaW = $state(0);
  let areaH = $state(0);
  let velH = $state(0);
  $effect(() => {
    prView.areaWidth = areaW;
    prView.areaHeight = areaH;
  });

  const ppt = $derived(pxPerTick(scale));
  const step = $derived(G.gridTicks(pr.grid, meter));
  /** ドラムの行: いちばん上の行は、見えている行の数で動ける範囲に。四角の幅はグリッドの 1 マス − 2（6 より小さくしない）。 */
  const drumGeom: G.DrumGeom | undefined = $derived(
    drums
      ? {
          keys: drumKeys,
          top: clampTopRow(drumView.topRow, drumKeys.length, G.wholeRows(areaH, rowH)),
          hitW: Math.max(metrics.px("part.drum.hit-min"), step * ppt - metrics.px("part.drum.hit-inset")),
          names: drums,
        }
      : undefined,
  );
  const g: G.RollGeom = $derived({ scrollTick: prView.scrollTick, ppt, topKey: prView.topKey, rowH, width: areaW, height: areaH, drum: drumGeom });
  /** ドラムには音階がないので、スケール固定を当てない（仕様書 3 章）。 */
  const rollScale = $derived(drums ? { ...pr.scale, lock: false } : pr.scale);

  const rec = $derived(transport.state === "recording" ? transport.recording : null);
  /** 録っているクリップ（か、まだクリップのない録音）を見せているときは、テイクの代わりに録っている物を描く。 */
  const liveMode = $derived(rec !== null && (shown.clip === null || rec.clip === shown.clip.id));

  const n = $derived(shownFor(shown.take, shown.clip));
  const origin = $derived(originFor(shown.origin, n));
  const selectedRanges = $derived(n && !liveMode && pr.take === n.take.id ? pr.selectedNotes : []);
  const selFlags = $derived(n ? G.selectionFlags(G.noteCount(n), selectedRanges) : null);

  // ---- 録っている最中のノート ----

  let book = $state.raw<LiveNotesBook | null>(null);
  let stopBook: (() => void) | null = null;
  let liveVersion = $state(0);
  $effect(() => {
    void current.state;
    void (current as { connected?: boolean }).connected;
    if (book) return;
    try {
      const found = liveNotes();
      stopBook = found.listen(() => (liveVersion += 1));
      book = found;
    } catch {
      // つながる前。次の状態でもう一度。
    }
  });
  onDestroy(() => stopBook?.());
  let lap = $state(0);
  const live: readonly LiveNote[] | null = $derived.by(() => {
    void liveVersion;
    if (!liveMode || !rec || !book) return null;
    return book.notesOf(rec.id, lap);
  });
  const held = $derived(live ? live.filter((l) => l.length === null) : []);

  // ---- マウス ----

  let dragVersion = $state(0);
  let eraseHover = $state(-1);
  /** 離して意図を送ったあと、答え（新しいテイク）が来るまで残しておく見た目。 */
  let pending = $state.raw<Drag | null>(null);
  let pendingReplied = $state(false);
  let lastSent: Promise<unknown> | null = null;

  // ---- 画面が鳴らす音（B-62）: 鍵盤の列、ノートを押す ・ 動かす、↑ ↓、書く道具 ----

  /** 画面が鳴らしている鍵（鍵盤の列で光らせる）。 */
  let heard: readonly number[] = [];
  const audition = new Auditioner(
    (c) => void send(c),
    (keys) => {
      heard = keys;
      redraw("keys");
    },
  );
  const mayAudition = () => notesMayAudition(app) && !liveMode;
  const msOf = (ticks: number) => ticksToMs(ticks, timeline.tempoBpm);
  onDestroy(() => audition.stop());

  onMount(() =>
    onNudgeKeys((c) => {
      const notes = liveMode ? null : n;
      if (!notes || c.take !== notes.take.id || !mayAudition()) return;
      let velocity = 1;
      for (const i of c.notes) velocity = Math.max(velocity, notes.take.velocities[i] ?? 1);
      audition.play(
        chordOf(
          c.notes.filter((i) => i < notes.take.keys.length).map((i) => notes.take.keys[i]!),
          c.keys,
        ),
        velocity,
        NUDGE_MS,
      );
    }),
  );

  // 左の鍵盤の列: 押している間だけ鳴らす。押したまま上下になぞると次の鍵に入れかえる。録音中も鳴らす（テイクには入らない）。
  let keyHeld: number | null = null;
  function keyAt(e: PointerEvent): number | null {
    const top = (e.currentTarget as HTMLElement).getBoundingClientRect().top;
    const key = G.yToKey(g, e.clientY - top);
    return key >= G.KEY_MIN && key <= G.KEY_MAX ? key : null;
  }
  function keysDown(e: PointerEvent): void {
    if (e.button !== 0) return;
    const key = keyAt(e);
    if (key === null) return;
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    keyHeld = key;
    audition.play([key], fixedOr100(app));
  }
  function keysMove(e: PointerEvent): void {
    if (keyHeld === null) return;
    const key = keyAt(e);
    if (key === null || key === keyHeld) return;
    keyHeld = key;
    audition.play([key], fixedOr100(app));
  }
  function keysUp(): void {
    if (keyHeld === null) return;
    keyHeld = null;
    audition.stop();
  }

  const drag = new RollDrag({
    shown: () => (liveMode ? null : n),
    geom: () => g,
    step: () => step,
    grid: () => (tick: number, floor: boolean) => gridTick(map, tick, step, floor, pr.grid.division === 0),
    scale: () => rollScale,
    meter: () => meter,
    tool: () => prView.tool,
    selected: () => selectedRanges,
    edgePx: () => metrics.px("part.note.resize"),
    stillPx: () => metrics.px("space.1"),
    velocityFullPx: () => sizes.velFull,
    velocityHeight: () => velH,
    send: (c: Command) => {
      const sent = send(c);
      lastSent = sent;
      if (c.type === "addNote" && c.take === null) awaitMadeClip(c.start, sent);
    },
    drawVelocity: () => fixedOr100(app),
    mayAudition,
    ticksToMs: msOf,
    audition,
  });
  const shownDrag = $derived.by(() => {
    void dragVersion;
    return drag.drag ?? pending;
  });

  $effect(() => {
    if (!pending || !pendingReplied) return;
    if (pr.take === (n?.take.id ?? null) || pending.kind === "box") pending = null;
  });

  const platform = platformOf(navigator);
  const modsOf = (e: MouseEvent): Mods => ({ shift: e.shiftKey, alt: e.altKey, mod: platform === "mac" ? e.metaKey : e.ctrlKey });

  let cursor = $state<CursorKind>("default");
  const cursorStyle = $derived(cursorCss(cursor, scale));

  function at(e: MouseEvent): { x: number; y: number } {
    const r = area!.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  }

  function changed(e: MouseEvent | null): void {
    dragVersion += 1;
    dragReading.text = drag.reading();
    onreading?.(dragReading.text && e ? { x: e.clientX, y: e.clientY } : null);
    if (drag.drag) cursor = drag.cursor();
  }

  function down(e: PointerEvent, where: "notes" | "velocity"): void {
    if (e.button !== 0) return;
    prView.focus = "pianoRoll";
    prView.picked = "notes";
    pending = null;
    const p = at(e);
    if (where === "notes") drag.down(p.x, p.y, modsOf(e));
    else {
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
      drag.downVelocity(p.x, e.clientY - r.top);
    }
    eraseHover = -1;
    if (drag.drag) (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
    changed(e);
  }

  function move(e: PointerEvent, where: "notes" | "velocity"): void {
    const p = at(e);
    if (drag.drag) {
      const y = where === "velocity" ? e.clientY - (e.currentTarget as HTMLElement).getBoundingClientRect().top : p.y;
      if (drag.move(p.x, y, modsOf(e))) changed(e);
      else if (dragReading.text) onreading?.({ x: e.clientX, y: e.clientY });
      return;
    }
    if (where === "velocity") {
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
      cursor = drag.overVelocityBar(p.x, e.clientY - r.top) ? "ns-resize" : "default";
      return;
    }
    const h = drag.hover(p.x, p.y, modsOf(e));
    cursor = h.cursor;
    if (h.changed) eraseHover = drag.eraseHover;
  }

  function up(): void {
    if (!drag.drag) return;
    const was = drag.drag;
    lastSent = null;
    const command = drag.up();
    if (command && lastSent) {
      pending = was;
      pendingReplied = false;
      void (lastSent as Promise<unknown>).then(() => (pendingReplied = true));
    }
    changed(null);
  }

  function leave(): void {
    if (drag.drag) return;
    if (eraseHover >= 0) eraseHover = drag.eraseHover = -1;
  }

  onMount(() =>
    onDragCancel(() => {
      if (!drag.cancel()) return false;
      changed(null);
      return true;
    }),
  );
  onDestroy(() => {
    if (drag.cancel()) {
      dragReading.text = null;
      onreading?.(null);
    }
  });

  // ---- ホイール: 縦に送る、Shift で横、Mod で横の拡大、Mod Shift で縦の拡大（カーソルの所を中心に） ----

  let wheelRows = 0;
  let wheelZoom = 0;
  let wheelRowZoom = 0;
  const ZOOM_NOTCH = 100;
  const scrollMaxTick = $derived.by(() => {
    let end = Math.max(transport.position, n && G.noteCount(n) > 0 ? G.noteEnd(n, G.noteCount(n) - 1) : 0, shown.clip?.end ?? 0);
    for (const tr of song.tracks) for (const c of tr.clips) end = Math.max(end, c.end);
    return Math.max(0, end - areaW / 2 / ppt);
  });

  function wheel(e: WheelEvent): void {
    const unit = e.deltaMode === 1 ? rowH : e.deltaMode === 2 ? areaH : 1;
    if (modsOf(e).mod && e.shiftKey) {
      e.preventDefault();
      e.stopPropagation();
      // Shift ＋ ホイールを横の送りに変える環境では、回した量が deltaX に来る。
      const d = (e.deltaY !== 0 ? e.deltaY : e.deltaX) * unit;
      wheelRowZoom += d;
      const steps = Math.trunc(wheelRowZoom / ZOOM_NOTCH) || (Math.abs(d) >= ZOOM_NOTCH / 2 ? Math.sign(d) : 0);
      if (steps === 0) return;
      wheelRowZoom = 0;
      zoomRowsBy(-steps, area ? at(e).y : undefined);
      return;
    }
    if (modsOf(e).mod) {
      e.preventDefault();
      e.stopPropagation();
      wheelZoom += e.deltaY * unit;
      const steps = Math.trunc(wheelZoom / ZOOM_NOTCH) || (Math.abs(e.deltaY * unit) >= ZOOM_NOTCH / 2 ? Math.sign(e.deltaY) : 0);
      if (steps === 0) return;
      wheelZoom = 0;
      const anchor = area ? G.xToTick(g, at(e).x) : undefined;
      zoomBy(-steps, scale, anchor);
      return;
    }
    let dx = e.deltaX * unit;
    let dy = e.deltaY * unit;
    if (e.shiftKey && dx === 0) [dx, dy] = [dy, 0];
    if (dx === 0 && dy === 0) return;
    e.preventDefault();
    if (dx !== 0) scrollToTick(Math.min(Math.max(prView.scrollTick + dx / ppt, 0), Math.max(scrollMaxTick, prView.scrollTick)));
    if (dy !== 0) {
      wheelRows += dy;
      const rows = Math.trunc(wheelRows / rowH);
      if (rows !== 0) {
        wheelRows -= rows * rowH;
        if (drums) drumView.topRow = clampTopRow((drumGeom?.top ?? 0) + rows, drumKeys.length, G.wholeRows(areaH, rowH));
        else prView.topKey = G.clampTopKey(prView.topKey - rows, G.wholeRows(areaH, rowH));
      }
    }
  }

  // ---- ひらいたとき: クリップのはじまりまで送り、入る拡大にし、ノートが見える高さに合わせる ----

  let fitKeysFor: number | null = null;
  /** クリップのない所で書いた位置（ティック）。その答えでできたクリップは、ひらいたのではないので、見ている範囲をそのままにする（B-52）。 */
  let madeAt: number | null = null;
  function awaitMadeClip(start: number, sent: Promise<number | null>): void {
    madeAt = start;
    void sent.then((rev) => {
      // 書けなかった（音階の外など）ときは、もう来ない。
      if (madeAt === start && (rev === null || current.rev >= rev) && current.state?.arrange.pianoRollClip == null) madeAt = null;
    });
  }
  $effect(() => {
    const clip = shown.clip;
    const w = areaW;
    const h = areaH;
    const notes = n;
    // 左右の欄をたたむかどうかが決まってから（ノートの場所の幅が変わるので）。
    if (!clip || w <= 0 || h <= 0 || prPanels.list === null || prPanels.inspector === null) return;
    untrack(() => {
      if (prView.shownClip !== clip.id) {
        prView.shownClip = clip.id;
        const made = madeAt !== null && clip.start <= madeAt && madeAt < clip.end;
        madeAt = null;
        if (made) {
          fitKeysFor = null;
          return;
        }
        // 欄をたたんだすぐあとは、bind の幅がまだ前の物なので、いまの幅を読む。
        prView.quarterPx = G.fitQuarterPx(clip.end - clip.start, area?.clientWidth || w, scale);
        prView.scrollTick = clip.start;
        fitKeysFor = clip.takeId;
      }
      if (fitKeysFor !== null && notes && notes.take.id === fitKeysFor) {
        // ドラムの行は、見ている範囲を別に持つ（drumView）。
        if (!drums) prView.topKey = G.fitTopKey(G.keyRangeOf(notes), G.wholeRows(area?.clientHeight || h, rowH));
        fitKeysFor = null;
      }
    });
  });

  // ---- WebGL: 3 つの面 ----

  const handles: Record<"roll" | "vel" | "keys" | "live", { update(s: GlSlot): void } | null> = { roll: null, vel: null, keys: null, live: null };
  let rollPicture: RollPicture | null = null;
  let rollMarks: Uint8Array | null = null;
  let livePicture: LivePicture | null = null;
  let sounding: number[] = [];
  const isSounding = (k: number) => sounding.includes(k);
  function setSounding(keys: number[]): void {
    if (sameKeys(keys, sounding)) return;
    sounding = keys;
    redraw("keys");
  }
  let keysGeom: G.RollGeom = { scrollTick: 0, ppt: 1, topKey: 60, rowH: 12, width: 0, height: 0 };

  /**
   * ドラムの行の名前の光り（仕様書 3 章 85）: 鳴っている行と弾いている行の名前を accent（録音中は rec）に。左の line.mark は WebGL。
   * 鍵の列の絵と同じコマで書きかえる（名前は鍵の列の canvas の上にあるので、よごれる四角が増えない）。
   */
  let drumNamesBox = $state<HTMLElement>();
  function markDrumNames(lit: (key: number) => boolean): void {
    if (!drumNamesBox) return;
    for (const el of drumNamesBox.children as HTMLCollectionOf<HTMLElement>) {
      const on = lit(Number(el.dataset.key));
      if (el.classList.contains("lit") !== on) el.classList.toggle("lit", on);
    }
  }
  $effect(() => {
    void transport.state;
    untrack(() => redraw("keys"));
  });

  const SLOTS: Record<"roll" | "vel" | "keys" | "live", GlSlot> = {
    roll: { kind: "paint", paint: (out, box, ctx) => rollPicture && paintRoll(out, box, rollPicture, rollMarks, ctx.dpr) },
    vel: { kind: "paint", paint: (out, box, ctx) => rollPicture && paintVelocityLane(out, box, rollPicture, ctx.dpr) },
    keys: {
      kind: "paint",
      keys: true,
      paint: (out, box, ctx) => {
        const lit = (k: number) => ctx.isDown(k) || isSounding(k) || heard.includes(k);
        const recording = transport.state === "recording";
        paintRollKeys(out, box, keysGeom, lit, ctx.dpr, { width: metrics.px("line.mark"), color: recording ? "color.rec" : "color.accent" });
        if (keysGeom.drum) markDrumNames(lit);
      },
    },
    live: { kind: "paint", paint: (out, box, ctx) => livePicture && paintLive(out, box, livePicture, ctx.dpr) },
  };

  function painted(node: HTMLElement, name: "roll" | "vel" | "keys" | "live") {
    const h = glSlot(node, SLOTS[name]);
    handles[name] = h;
    return {
      destroy() {
        h.destroy();
        if (handles[name] === h) handles[name] = null;
      },
    };
  }
  const redraw = (name: "roll" | "vel" | "keys" | "live") => handles[name]?.update(SLOTS[name]);

  /** 大きな面の絵に関係するドラッグの形。動かす・長さを変えるあいだは始めたときから変わらないので、大きな面は描きなおさない。 */
  const serials = new WeakMap<Drag, number>();
  let serial = 0;
  const rollDragKey = $derived.by(() => {
    void dragVersion;
    const d = shownDrag;
    if (!d) return "";
    if (!serials.has(d)) serials.set(d, ++serial);
    return `${serials.get(d)}:${rollDragPart(d)}`;
  });
  function rollDragPart(d: Drag): string {
    switch (d.kind) {
      case "move":
        return `m${d.press.moved && !d.copy ? 1 : 0}`;
      case "resize":
        return `r${d.press.moved ? 1 : 0}`;
      case "box":
        return `b${d.hits.length}:${d.hits.reduce((a, b) => a + b, 0)}`;
      case "erase":
        return `e${d.hits.size}`;
      case "velocity":
        return `v${d.delta}`;
      case "draw":
        return "d";
    }
  }

  $effect(() => {
    void rollDragKey;
    const d = untrack(() => shownDrag);
    const notes = liveMode ? null : n;
    rollPicture = {
      g,
      sizes,
      scale: pr.scale,
      meter,
      step,
      map,
      barStep: pr.grid.division === 0,
      shown: notes,
      origin: liveMode ? null : origin,
      selected: selFlags,
      live,
      drag: d,
      eraseHover,
    };
    rollMarks = noteMarks(G.noteCount(notes), d, eraseHover);
    untrack(() => {
      redraw("roll");
      redraw("vel");
    });
  });

  $effect(() => {
    keysGeom = g;
    untrack(() => redraw("keys"));
  });

  // 動いている物の面: 置き場所（CSS px）と、動かす量（transform）。
  interface LiveBox {
    left: number;
    top: number;
    width: number;
    height: number;
    tx: number;
    ty: number;
  }
  let nowTick = $state(0);
  const liveBox: LiveBox | null = $derived.by(() => {
    void dragVersion;
    const d = shownDrag;
    if (d && n && !liveMode) {
      if (d.kind === "move" && d.press.moved) {
        const b = movedBoxOf(d);
        if (!b) return null;
        const grabKey = n.take.keys[d.grab]!;
        const ty = d.keys === 0 ? 0 : G.keyToY(g, grabKey + d.keys) - G.keyToY(g, grabKey);
        return { left: b.x0, top: b.y0, width: b.x1 - b.x0, height: b.y1 - b.y0, tx: d.ticks * ppt, ty: Number.isNaN(ty) ? 0 : ty };
      }
      if (d.kind === "resize" && d.press.moved) {
        const b = movedBoxOf(d);
        if (!b) return null;
        // 始まりを動かすときは、左へのびる分まで（ノートの場所の左はしから）。
        const left = d.edge === "start" ? Math.min(0, b.x0) : b.x0;
        const right = d.edge === "start" ? b.x1 : areaW;
        return { left, top: b.y0, width: Math.max(1, right - left), height: b.y1 - b.y0, tx: 0, ty: 0 };
      }
    }
    if (d && d.kind === "draw") {
      const left = G.tickToX(g, d.start);
      return { left, top: G.keyToY(g, d.key), width: Math.max(1, areaW - left), height: rowH, tx: 0, ty: 0 };
    }
    if (held.length > 0) {
      let first = Infinity;
      let y0 = Infinity;
      let y1 = -Infinity;
      for (const l of held) {
        const top = G.keyToY(g, l.key);
        if (Number.isNaN(top)) continue;
        first = Math.min(first, l.start);
        y0 = Math.min(y0, top);
        y1 = Math.max(y1, top + rowH);
      }
      if (!(y1 > y0)) return null;
      const left = G.tickToX(g, first);
      return { left, top: y0, width: Math.max(1, areaW - left), height: y1 - y0, tx: 0, ty: 0 };
    }
    return null;
  });

  /** ドラッグを始めたときの箱（ノートの場所の CSS px）。 */
  function movedBoxOf(d: Drag): { x0: number; x1: number; y0: number; y1: number } | null {
    if (!n || (d.kind !== "move" && d.kind !== "resize")) return null;
    let x0 = Infinity;
    let x1 = -Infinity;
    let y0 = Infinity;
    let y1 = -Infinity;
    for (const k of d.notes) {
      const top = G.keyToY(g, n.take.keys[k]!);
      if (Number.isNaN(top)) continue;
      x0 = Math.min(x0, G.tickToX(g, G.noteStart(n, k)));
      // ドラムの四角は長さを見せないので、四角の右はしまで。
      x1 = Math.max(x1, drumGeom ? G.tickToX(g, G.noteStart(n, k)) + drumGeom.hitW + 1 : G.tickToX(g, G.noteEnd(n, k)));
      y0 = Math.min(y0, top);
      y1 = Math.max(y1, top + rowH);
    }
    if (!(x1 > x0)) return null;
    return { x0, x1, y0, y1 };
  }

  function liveNotesOf(d: Drag | null): LivePicture["notes"] {
    const out: { start: number; end: number; key: number; face: ColorName }[] = [];
    const face = (i: number): ColorName => (selFlags && selFlags[i] ? "color.accent" : G.velocityColor(n!.take.velocities[i]!));
    if (d && n && !liveMode) {
      if (d.kind === "move" && d.press.moved) {
        for (const i of d.notes) out.push({ start: G.noteStart(n, i), end: G.noteEnd(n, i), key: n.take.keys[i]!, face: face(i) });
        return out;
      }
      if (d.kind === "resize" && d.press.moved) {
        for (const i of d.notes) {
          const start = G.noteStart(n, i) + (d.edge === "start" ? d.ticks : 0);
          const end = G.noteEnd(n, i) + (d.edge === "end" ? d.ticks : 0);
          out.push({ start, end, key: n.take.keys[i]!, face: face(i) });
        }
        return out;
      }
    }
    if (d && d.kind === "draw") {
      out.push({ start: d.start, end: d.start + d.length, key: d.key, face: "color.accent" });
      return out;
    }
    for (const l of held) out.push({ start: l.start, end: Math.max(l.start, nowTick), key: l.key, face: "color.rec" });
    return out;
  }

  $effect(() => {
    const box = liveBox;
    void dragVersion;
    const d = shownDrag;
    void nowTick;
    livePicture = box ? { g, sizes, left: box.left, top: box.top, ty: box.ty, areaH, notes: liveNotesOf(d) } : null;
    untrack(() => redraw("live"));
  });

  // ---- 動く物: 再生の線、ページ送り、鳴っている音、録っているノートの先 ----

  let playLine = $state<HTMLElement | null>(null);
  let offsetUs: number | null = null;
  let drawnLinePx = NaN;
  let drawnLineRec = false;

  function placeLine(tick: number): void {
    const line = playLine;
    if (!line) return;
    const dpr = metrics.dpr;
    const px = Math.round((tick - prView.scrollTick) * ppt * dpr);
    const recLine = transport.state === "recording" || transport.state === "countIn";
    if (px === drawnLinePx && recLine === drawnLineRec) return;
    drawnLinePx = px;
    drawnLineRec = recLine;
    line.style.transform = `translateX(${px / dpr}px)`;
    line.style.visibility = px < 0 || px / dpr >= areaW ? "hidden" : "";
  }

  function frame(): void {
    const tel = current.telemetry;
    const tick = shownTick(transport, tel, offsetUs === null ? null : performance.now() * 1000 + offsetUs);
    if (prView.follow && areaW > 0) {
      const x = AG.followScroll(tick * ppt, prView.scrollTick * ppt, areaW);
      if (x !== prView.scrollTick * ppt) prView.scrollTick = x / ppt;
    }
    placeLine(tick);
    setSounding(soundingKeys(liveMode ? null : shown.take, shown.clip, tick));
    if (rec) {
      const l = tel?.position?.lap ?? rec.lap;
      if (l !== lap) lap = l;
      const px = Math.round(tick * ppt * metrics.dpr);
      if (held.length > 0 && px !== Math.round(nowTick * ppt * metrics.dpr)) nowTick = tick;
    }
  }

  let lastState = "stopped";
  $effect(() => {
    const s = transport.state;
    if (lastState === "stopped" && s !== "stopped") prView.follow = true;
    lastState = s;
  });

  $effect(() => {
    if (transport.state === "stopped") {
      untrack(() => setSounding([]));
      return;
    }
    if (offsetUs === null) {
      clockOffsetUs().then(
        (v) => (offsetUs = v),
        () => {},
      );
    }
    return everyGlFrame(frame);
  });

  $effect(() => {
    void playLine;
    void prView.scrollTick;
    void ppt;
    void areaW;
    const stopped = transport.state === "stopped";
    const position = transport.position;
    drawnLinePx = NaN;
    if (stopped) untrack(() => placeLine(position));
  });
  const lineRec = $derived(transport.state === "countIn" || transport.state === "recording");

  // ---- 文字: 目盛と鍵盤の列の音の名前 ----

  const LABEL_MIN_PX = 40;
  const rulerLabels = $derived.by(() => {
    const out: { tick: number; x: number; text: string; bar: boolean }[] = [];
    if (!(areaW > 0)) return out;
    const end = prView.scrollTick + areaW / ppt;
    const every = (perBeat: number, perBar: number) =>
      perBeat * ppt >= LABEL_MIN_PX ? perBeat : perBar * Math.max(1, Math.ceil(LABEL_MIN_PX / (perBar * ppt)));
    const at = { bar: 0, beat: 0, tick: 0 };
    eachLine(map, prView.scrollTick, end, every, (tick, kind) => {
      map.barBeatTick(tick, at);
      out.push({ tick, x: (tick - prView.scrollTick) * ppt, text: `${at.bar}.${at.beat}`, bar: kind === 0 });
    });
    return out;
  });
  /** 目盛のマーカーの名前（仕様書 2 章「ピアノロールの目盛のマーカー」。押せない）。 */
  const rulerMarkers = $derived.by(() => {
    if (!(areaW > 0)) return [];
    return timeline.markers
      .map((m) => ({ id: m.id, name: m.name, x: (m.tick - prView.scrollTick) * ppt }))
      .filter((m) => m.x > -areaW && m.x < areaW);
  });
  /** 名前の幅と拍の数字の幅（CSS px）。名前がかかる拍の数字は出さない（仕様書 3 章、モック R11-Answers92）。 */
  const markerW: Record<number, number> = $state({});
  const labelW: Record<number, number> = $state({});
  const shownLabels = $derived(labelsBesideMarkers(rulerLabels, rulerMarkers, labelW, markerW));
  /** ドラムの行の名前。このクリップで叩いている行は gray.10、ほかは gray.9（モック R7-Drum。使っていない行も暗くしない）。 */
  const usedKeys = $derived.by(() => {
    const used = new Set<number>();
    if (!drums || !n) return used;
    for (let i = 0; i < n.take.keys.length; i++) if (G.noteShown(n, i)) used.add(n.take.keys[i]!);
    return used;
  });
  const drumLabels = $derived.by(() => {
    const out: { key: number; y: number; name: string; term: string | null; note: string; used: boolean }[] = [];
    const d = drumGeom;
    if (!d) return out;
    const rows = Math.min(d.keys.length - d.top, G.rowsShown(g));
    for (let r = 0; r < rows; r++) {
      const key = d.keys[d.top + r]!;
      const name = drumName(key, d.names) ?? G.noteName(key);
      out.push({ key, y: r * rowH, name, term: drumTerm(name), note: G.noteName(key), used: usedKeys.has(key) });
    }
    return out;
  });
  const keyLabels = $derived.by(() => {
    const out: { key: number; y: number; text: string }[] = [];
    if (drums) return out;
    const rows = G.rowsShown(g);
    for (let r = 0; r < rows; r++) {
      const key = prView.topKey - r;
      if (key < 0) break;
      if (key % 12 === 0) out.push({ key, y: r * rowH, text: G.noteName(key) });
    }
    return out;
  });

  // ---- 目盛: 押すと再生の位置（グリッドに寄せる。Shift で寄せない）、ドラッグでループの範囲（小節の単位。アレンジの目盛と同じ） ----

  const rulerView = $derived<AG.View>({ pxPerTick: ppt, scrollX: prView.scrollTick * ppt, width: areaW });
  const songLoop = $derived(song.loopOn ? AG.loopBars(song.loopRange, map) : null);
  /** ドラッグ中の見た目。Rust の答え（次の差分）が来たら消す。 */
  let loopPreview = $state<AG.BarRange | null>(null);
  const shownLoop = $derived(loopPreview ?? songLoop);
  let rulerDrag: { d: AG.RulerDrag; downX: number; left: number } | null = null;
  $effect(() => {
    void current.rev;
    untrack(() => {
      if (!rulerDrag) loopPreview = null;
    });
  });
  const loopX = (bar: number) => AG.tickToX(rulerView, AG.lineTick(map, bar));

  function rulerDown(e: PointerEvent): void {
    if (e.button !== 0) return;
    const left = (e.currentTarget as HTMLElement).getBoundingClientRect().left;
    const x = e.clientX - left;
    rulerDrag = { d: AG.startRulerDrag(rulerView, x, map, songLoop, metrics.px("space.2")), downX: x, left };
    (e.currentTarget as HTMLElement).setPointerCapture?.(e.pointerId);
  }
  function rulerMove(e: PointerEvent): void {
    if (!rulerDrag) return;
    const x = e.clientX - rulerDrag.left;
    if (Math.abs(x - rulerDrag.downX) >= metrics.px("space.1")) loopPreview = AG.dragRange(rulerDrag.d, rulerView, x, map);
  }
  function rulerUp(e: PointerEvent): void {
    if (!rulerDrag) return;
    const r = rulerDrag;
    rulerDrag = null;
    const x = e.clientX - r.left;
    let command: Command;
    if (Math.abs(x - r.downX) < metrics.px("space.1")) {
      loopPreview = null;
      const tick = AG.xToTick(rulerView, r.downX);
      command = { type: "locate", tick: Math.max(0, e.shiftKey ? Math.round(tick) : gridTick(map, tick, step, false, pr.grid.division === 0)) };
    } else {
      const range = AG.dragRange(r.d, rulerView, x, map);
      command = { type: "setLoop", fromBar: range.from, toBar: range.to };
    }
    const before = current.rev;
    // 範囲が変わらなければ差分は来ないので、返事の rev で見た目を消す。
    void send(command).then((rev) => {
      if (rev === before && !rulerDrag) loopPreview = null;
    });
  }
  function rulerCancel(): boolean {
    if (!rulerDrag) return false;
    rulerDrag = null;
    loopPreview = null;
    return true;
  }
  // Esc: ループの範囲のドラッグをやめて、前の範囲に戻す（仕様書 4 章）。
  onMount(() => onDragCancel(rulerCancel));

  // ---- 右クリックの一覧（B-60）: ノートの上と、ノートの場所の空いた所 ----

  function contextMenu(e: MouseEvent): void {
    e.preventDefault();
    if (drag.drag || !area) return;
    prView.focus = "pianoRoll";
    prView.picked = "notes";
    const notes = liveMode ? null : n;
    const p = at(e);
    const hit = G.hitNote(notes, g, p.x, p.y, 0);
    // 選ばれていないノートなら、それだけを選んでから。
    if (hit && notes && !G.indexesOf(selectedRanges).includes(hit.note)) {
      void send({ type: "selectNotes", take: notes.take.id, notes: [hit.note], mode: "replace" });
    }
    // 行を押すのは一覧が出たあとなので、そのときの状態で意図を作る。
    const actions: RollMenuActions = {
      key(k) {
        const state = current.state;
        if (!state) return;
        const c = pianoRollCommand(k, { state, shown: liveMode ? null : n, playTick: shownTick(state.transport, current.telemetry, null) });
        if (c) void send(c);
      },
      unquantize() {
        const state = current.state;
        const shownNow = liveMode ? null : n;
        if (!state || !shownNow || state.pianoRoll.take !== shownNow.take.id) return;
        const picked = G.indexesOf(state.pianoRoll.selectedNotes).filter((i) => i < shownNow.take.starts.length);
        void send({ type: "unquantize", take: shownNow.take.id, notes: picked });
      },
      pasteAt: (tick) => void send({ type: "pasteNotes", at: tick }),
    };
    const entries = rollMenu(
      {
        onNote: hit !== null && notes !== null,
        busy: transport.state === "recording" || transport.state === "countIn",
        clipboard: pr.clipboardNotes > 0,
        origin: (notes?.take.origin ?? 0) !== 0,
        pasteAt: Math.max(0, G.snapTick(G.xToTick(g, p.x), step)),
        mac: platform === "mac",
      },
      actions,
    );
    closeContextMenu();
    openContextMenu(e.clientX, e.clientY, entries);
  }

  // ---- 囲む枠 ----

  const boxFrame = $derived.by(() => {
    void dragVersion;
    const d = shownDrag;
    if (!d || d.kind !== "box" || !d.press.moved || d !== drag.drag) return null;
    return { x: Math.min(d.press.x, d.x), y: Math.min(d.press.y, d.y), w: Math.abs(d.x - d.press.x), h: Math.abs(d.y - d.press.y) };
  });

  // ---- 横の棒 ----

  const scrollTotal = $derived(Math.max((scrollMaxTick + areaW / 2 / ppt) * ppt + areaW / 2, prView.scrollTick * ppt + areaW));
</script>

<div class="roll" class:drums use:glRegion={"pianoRoll"}>
  <div class="ruler">
    <div class="ruler-gap">
      <button
        class="m-plain vzoom"
        disabled={drums ? drumRowsPx <= DRUM_ROW_MIN : prView.rowPx === ROW_PX_STEPS[0]}
        aria-label={t("menu.zoomRowsOut")}
        onclick={() => zoomRowsBy(-1)}
        data-test="pr-row-zoom-out">{t("zoom.outMark")}</button
      >
      <button
        class="m-plain vzoom"
        aria-label={t("menu.zoomRowsIn")}
        disabled={drums ? drumRowsPx >= DRUM_ROW_MAX : prView.rowPx === ROW_PX_STEPS[ROW_PX_STEPS.length - 1]}
        onclick={() => zoomRowsBy(1)}
        data-test="pr-row-zoom-in">{t("zoom.inMark")}</button
      >
    </div>
    <div
      class="ruler-marks"
      role="presentation"
      onpointerdown={rulerDown}
      onpointermove={rulerMove}
      onpointerup={rulerUp}
      onpointercancel={rulerCancel}
      data-test="pr-ruler"
    >
      {#if shownLoop}
        <div class="loop-mark" style:left="{loopX(shownLoop.from)}px" style:width="{loopX(shownLoop.to) - loopX(shownLoop.from)}px"></div>
      {/if}
      {#each shownLabels as l (l.tick)}
        <span class="m-num mark" class:bar={l.bar} style:left="{l.x}px" bind:offsetWidth={labelW[l.tick]}>{l.text}</span>
      {/each}
      {#each rulerMarkers as m (m.id)}
        <span class="ruler-marker m-lbl" style:left="{m.x}px" bind:offsetWidth={markerW[m.id]} data-test="pr-ruler-marker">{m.name}</span>
      {/each}
    </div>
  </div>
  <div class="middle" onwheel={wheel}>
    <div
      class="keys"
      role="presentation"
      onpointerdown={keysDown}
      onpointermove={keysMove}
      onpointerup={keysUp}
      onpointercancel={keysUp}
      data-test="pr-keys"
    >
      {#if drums}
        <div class="drum-names" class:rec={transport.state === "recording"} data-test="pr-drum-names" bind:this={drumNamesBox}>
          {#each drumLabels as k (k.key)}
            <span class="drum-name" class:used={k.used} style:top="{k.y}px" style:height="{rowH}px" use:tipAround={k.note} data-key={k.key}
              >{#if k.term}{#each termParts(k.name, k.term) as piece, j (j)}{#if piece.term}<span use:term={k.term}>{piece.text}</span
                    >{:else}{piece.text}{/if}{/each}{:else}{k.name}{/if}</span
            >
          {/each}
        </div>
      {:else}
        <div class="key-names">
          {#each keyLabels as k (k.key)}
            <span class="m-num key-name" style:top="{k.y}px" style:height="{rowH}px">{k.text}</span>
          {/each}
        </div>
      {/if}
      <div class="key-shapes" use:glRegion={"pianoRollKeys"} use:painted={"keys"}></div>
    </div>
    <div class="notes">
    <div
      class="area"
      role="presentation"
      bind:this={area}
      bind:clientWidth={areaW}
      bind:clientHeight={areaH}
      style:cursor={cursorStyle}
      onpointerdown={(e) => down(e, "notes")}
      onpointermove={(e) => move(e, "notes")}
      onpointerup={up}
      onpointercancel={() => drag.cancel() && changed(null)}
      onpointerleave={leave}
      oncontextmenu={contextMenu}
    >
      <div class="fill" use:painted={"roll"}></div>
      {#if boxFrame}
        <div class="box-frame" style:left="{boxFrame.x}px" style:top="{boxFrame.y}px" style:width="{boxFrame.w}px" style:height="{boxFrame.h}px"></div>
      {/if}
      <div class="play-line" class:rec={lineRec} bind:this={playLine}></div>
      {#if shown.clip === null && !liveMode}
        <div class="empty-hint" data-test="pr-empty-hint">
          <span class="strong">{t("pr.emptyTitle")}</span>
          <span
            >{#each textParts("pr.emptyBody", { key: t("key.toolDraw") }) as part, i (i)}{#if part.hole !== null}<span class="m-num"
                  >{part.text}</span
                >{:else}{part.text}{/if}{/each}</span
          >
        </div>
      {/if}
    </div>
    <HScrollbar total={scrollTotal} visible={areaW} offset={prView.scrollTick * ppt} onscroll={(x) => scrollToTick(x / ppt)} />
    </div>
  </div>
  {#if velocityLane.open}
  <div class="velocity" data-test="pr-velocity">
    <div class="velocity-head">
      <button class="m-plain m-hi fold m-focus-in" aria-expanded="true" onclick={() => (velocityLane.open = false)} data-test="pr-velocity-fold"
        >▾ <span use:term={"ベロシティ"}>{t("pr.velocity")}</span></button
      >
      {#if selectedRanges.length > 0 && n}
        {@const values = G.indexesOf(selectedRanges).slice(0, 3).map((i) => n.take.velocities[i])}
        <span class="m-num m-lbl">{t("pr.velocitySelected", { values: values.join(" ") })}</span>
      {:else}
        <span></span>
      {/if}
      <span class="m-num m-lbl">{t("pr.velocityScale")}</span>
    </div>
    <div
      class="bars"
      role="presentation"
      bind:clientHeight={velH}
      style:cursor={cursorStyle}
      onpointerdown={(e) => down(e, "velocity")}
      onpointermove={(e) => move(e, "velocity")}
      onpointerup={up}
      onpointercancel={() => drag.cancel() && changed(null)}
    >
      <div class="fill" use:painted={"vel"}></div>
    </div>
  </div>
  {/if}
  <CcLanes {g} {meter} {step} take={liveMode ? null : n} {areaH} {liveHost} {onreading} />
  <div class="live-host" use:glRegion={"pianoRollLive"} bind:this={liveHost}>
    {#if liveBox}
      <div
        class="live-box"
        style:left="{liveBox.left}px"
        style:top="{liveBox.top}px"
        style:width="{liveBox.width}px"
        style:height="{liveBox.height}px"
        style:transform={liveBox.tx || liveBox.ty ? `translate(${liveBox.tx}px, ${liveBox.ty}px)` : null}
        use:painted={"live"}
      ></div>
    {/if}
  </div>
</div>

<style>
  .roll {
    --pr-keys: var(--size-col-keys);
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    position: relative;
  }
  /* ドラムの鍵の列は幅 132（仕様書 3 章）。CC の帯の見出しの列も同じ幅。 */
  .roll.drums {
    --pr-keys: var(--part-drum-keys);
  }
  .ruler {
    height: var(--mock-pianoroll-ruler);
    flex-shrink: 0;
    display: flex;
    background: var(--color-gray-3);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .ruler-gap {
    box-sizing: border-box;
    width: var(--pr-keys);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--mock-pianoroll-row-zoom-gap);
    padding: 0 var(--space-1);
    border-right: var(--line-hair) solid var(--color-gray-6);
  }
  /* 縦の − ＋（モック A-PianoRoll）: 幅 20 ・ 高さ 18 の切りかえ。いちばん小さい ・ 大きい段では「使えない」。 */
  .vzoom {
    box-sizing: border-box;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: var(--part-toggle-size);
    height: var(--mock-pianoroll-row-zoom-h);
    padding: 0;
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-gray-8);
  }
  .vzoom:hover:not(:disabled) {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .vzoom:active:not(:disabled) {
    background: var(--color-gray-2);
    border-color: var(--color-gray-8);
  }
  .vzoom:disabled {
    border-color: var(--color-gray-5);
    color: var(--color-gray-7);
    cursor: default;
  }
  .ruler-marks {
    flex: 1;
    min-width: 0;
    position: relative;
    overflow: hidden;
    touch-action: none;
  }
  .loop-mark {
    position: absolute;
    top: 0;
    height: var(--mock-arrange-loop);
    background: var(--color-accent);
    pointer-events: none;
  }
  /* 目盛のマーカー（仕様書 2 章: 名前を gray.8 で、左に gray.8 の line.hair。押せない）。名前は地 gray.3 で拍の数字の上に重ねる（3 章、R11-Answers92）。 */
  .ruler-marker {
    position: absolute;
    top: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    padding: 0 var(--space-1);
    background: var(--color-gray-3);
    border-left: var(--line-hair) solid var(--color-gray-8);
    white-space: nowrap;
    pointer-events: none;
  }
  .mark {
    position: absolute;
    top: 0;
    bottom: 0;
    display: flex;
    align-items: center;
    padding-left: var(--space-1);
    color: var(--color-gray-8);
    white-space: nowrap;
  }
  .mark.bar {
    color: var(--color-gray-10);
  }
  .middle {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .keys {
    width: var(--pr-keys);
    flex-shrink: 0;
    display: flex;
    background: var(--color-gray-3);
    border-right: var(--line-hair) solid var(--color-gray-6);
    overflow: hidden;
    touch-action: none;
  }
  .key-names {
    width: var(--mock-pianoroll-key-label);
    flex-shrink: 0;
    position: relative;
  }
  .key-name {
    position: absolute;
    right: 0;
    padding-right: var(--space-1);
    display: flex;
    align-items: center;
    color: var(--color-gray-8);
  }
  .key-shapes {
    flex: 1;
    min-width: 0;
  }
  /* ドラムの行の名前（モック R7-Drum）: 地 gray.5 は WebGL（key-shapes を列いっぱいに）、名前は HTML でその上に。 */
  .drums .keys {
    position: relative;
  }
  .drums .key-shapes {
    position: absolute;
    inset: 0;
  }
  .drum-names {
    position: absolute;
    inset: 0;
    z-index: 1;
  }
  .drum-name {
    position: absolute;
    left: 0;
    right: 0;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    padding: 0 var(--space-2);
    overflow: hidden;
    white-space: nowrap;
    color: var(--color-gray-9);
  }
  .drum-name.used {
    color: var(--color-gray-10);
  }
  .drum-names :global(.drum-name.lit) {
    color: var(--color-accent);
  }
  .drum-names.rec :global(.drum-name.lit) {
    color: var(--color-rec);
  }
  .notes {
    flex: 1;
    min-width: 0;
    position: relative;
  }
  .area {
    position: absolute;
    inset: 0;
    overflow: hidden;
    touch-action: none;
  }
  .fill {
    position: absolute;
    inset: 0;
  }
  .live-host {
    position: absolute;
    left: var(--pr-keys);
    right: 0;
    top: var(--mock-pianoroll-ruler);
    bottom: 0;
    overflow: hidden;
    pointer-events: none;
    z-index: 1;
  }
  .live-box {
    position: absolute;
  }
  .box-frame {
    position: absolute;
    border: var(--line-hair) solid var(--color-accent);
    pointer-events: none;
    z-index: 2;
  }
  /* クリップがない所の案内（モック F-PREmpty。形は「音が出ました」の箱と同じ）。下のノートの場所を押せるように、押せない箱。 */
  .empty-hint {
    position: absolute;
    left: var(--space-6);
    top: var(--space-6);
    z-index: 2;
    box-sizing: border-box;
    width: var(--mock-pianoroll-empty-hint);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-3);
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-7);
    color: var(--color-gray-8);
    white-space: normal;
    pointer-events: none;
  }
  .empty-hint .strong {
    color: var(--color-gray-10);
  }
  .play-line {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: var(--line-hair);
    background: var(--color-accent);
    pointer-events: none;
    z-index: 3;
  }
  .play-line.rec {
    background: var(--color-rec);
  }
  .notes :global([data-testid="hscrollbar"]) {
    z-index: 4;
  }
  .velocity {
    height: var(--size-lane-vel);
    flex-shrink: 0;
    display: flex;
    border-top: var(--line-hair) solid var(--color-gray-6);
  }
  .velocity-head {
    width: var(--pr-keys);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: var(--space-1) var(--space-2);
    background: var(--color-gray-3);
    border-right: var(--line-hair) solid var(--color-gray-6);
    white-space: nowrap;
    overflow: hidden;
  }
  .fold {
    align-self: flex-start;
  }
  .fold:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .fold:active {
    background: var(--color-gray-2);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .bars {
    flex: 1;
    min-width: 0;
    position: relative;
    overflow: hidden;
    touch-action: none;
  }
</style>

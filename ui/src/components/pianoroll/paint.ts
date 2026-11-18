// ピアノロールの WebGL の絵（四角の並び）。DOM を知らない。座標は区画の canvas の画面の画素（box は slot の箱）。
// ノートは仕様書 2 章「ノート」: 高さ 11、左はしに gray.10 の 2px、面はベロシティで gray.8 / 9 / 10、選んだら accent、
// そろえる前の位置は gray.7 の枠だけ（選んでいるノートの分は accent）、書けない行は gray.0。行の地と線の色はモック A-PianoRoll。
// ドラッグ中のもとの位置は accent の枠。どちらの枠も、ノートに隠れないよう、ノートを描いたあとに描く（docs/design-notes.md B-64）。

import type { ControlKind, LiveNote, MusicalScale } from "../../bridge";
import type { ColorName } from "../../design/tokens";
import type { Batch } from "../../gl/batch";
import { rgb } from "../../gl/color";
import { isBlack } from "../../gl/keyboard";
import { HAIR, type Box } from "../../gl/paint";
import { ticksPerBar, ticksPerBeat, type Meter } from "../time";
import { draggedPoint, firstPointFrom, isOnOff, lanePoints, pointTick, valueToY, type LaneBox, type LaneDrag } from "./cc";
import type { Drag } from "./drag";
import { draggedVelocity } from "./drag";
import { eachNoteIn, movedFrom, noteEnd, noteStart, rowColor, rowsShown, velocityColor, type RollGeom, type Shown } from "./geom";

/** 寸法（CSS px、拡大率をかけたあと）。 */
export interface RollSizes {
  noteH: number;
  edge: number;
  velBar: number;
  velCap: number;
  velFull: number;
  velMid: number;
  endMark: number;
}

/** ノートの場所とベロシティの帯の大きな絵が使う物。変わったら slot の update を呼ぶ。 */
export interface RollPicture {
  g: RollGeom;
  sizes: RollSizes;
  scale: MusicalScale;
  meter: Meter;
  step: number;
  /** 見せているテイク。録音中で、録っている物を見せるときは null。 */
  shown: Shown | null;
  /** クオンタイズしたテイクのもとのテイク（同じ base で読む）。 */
  origin: Shown | null;
  /** 選んだノートの印（shown の番号）。 */
  selected: Uint8Array | null;
  /** 録っている最中のノート（曲の中の位置）。length が null の物（押している）は、ここでは棒だけ。 */
  live: readonly LiveNote[] | null;
  drag: Drag | null;
  eraseHover: number;
}

const FRAME: ColorName = "color.gray.7";

/** 線の間がこれより狭くなったら、ひとつ粗い線にする（CSS px）。 */
const LINE_MIN_PX = 4;

function lineStep(step: number, meter: Meter, ppt: number): number {
  const perBeat = ticksPerBeat(meter);
  const perBar = ticksPerBar(meter);
  if (step * ppt >= LINE_MIN_PX) return step;
  if (perBeat * ppt >= LINE_MIN_PX) return perBeat;
  return perBar * Math.max(1, Math.ceil(LINE_MIN_PX / (perBar * ppt)));
}

/** ノートの印: 0 ふつう、1 動かしている（もとの所に accent の枠）、2 長さを変えている（終わりに線）、3 囲んでいる（accent）、4 消える（gray.7）。 */
export function noteMarks(count: number, drag: Drag | null, eraseHover: number): Uint8Array | null {
  if (!drag && eraseHover < 0) return null;
  const m = new Uint8Array(count);
  if (eraseHover >= 0 && eraseHover < count) m[eraseHover] = 4;
  if (!drag) return m;
  switch (drag.kind) {
    case "move":
      if (drag.press.moved && !drag.copy) for (const i of drag.notes) m[i] = 1;
      break;
    case "resize":
      if (drag.press.moved) for (const i of drag.notes) m[i] = 2;
      break;
    case "box":
      for (const i of drag.hits) m[i] = 3;
      break;
    case "erase":
      for (const i of drag.hits) m[i] = 4;
      break;
    default:
  }
  return m;
}

function frame(out: Batch, x0: number, y0: number, x1: number, y1: number, color: ColorName): void {
  const c = rgb(color);
  out.rect(x0, y0, x1, y0 + HAIR, c);
  out.rect(x0, y1 - HAIR, x1, y1, c);
  out.rect(x0, y0, x0 + HAIR, y1, c);
  out.rect(x1 - HAIR, y0, x1, y1, c);
}

/** ノート 1 つ。x0 と x1 は始まりと終わりの線の位置（画素）。線の 1px をあけて描く。 */
export function paintNote(out: Batch, x0: number, x1: number, y: number, h: number, edge: number, face: ColorName): void {
  const a = Math.round(x0) + HAIR;
  const b = Math.max(a + HAIR, Math.round(x1) - HAIR);
  const e = Math.min(b, a + edge);
  out.rect(a, y, e, y + h, rgb("color.gray.10"));
  out.rect(e, y, b, y + h, rgb(face));
}

/** ノートの場所: 行の地、方眼の線、ノート、そろえる前の位置の枠、ドラッグのもとの位置の枠。 */
export function paintRoll(out: Batch, box: Box, p: RollPicture, marks: Uint8Array | null, dpr: number): void {
  const { g, sizes } = p;
  const rowH = g.rowH * dpr;
  const ppt = g.ppt * dpr;
  const X = (tick: number) => box.x0 + (tick - g.scrollTick) * ppt;
  const Y = (key: number) => box.y0 + (g.topKey - key) * rowH;
  const noteH = Math.round(sizes.noteH * dpr);
  const edge = Math.round(sizes.edge * dpr);

  out.rect(box.x0, box.y0, box.x1, box.y1, rgb("color.gray.1"));
  const rows = rowsShown(g) + 1;
  for (let r = 0; r < rows; r++) {
    const key = g.topKey - r;
    if (key < 0) break;
    const y = Y(key);
    out.rect(box.x0, y, box.x1, y + rowH - HAIR, rgb(rowColor(key, p.scale)));
  }

  const from = g.scrollTick;
  const to = g.scrollTick + g.width / g.ppt;
  const s = lineStep(p.step, p.meter, g.ppt);
  const perBar = ticksPerBar(p.meter);
  const perBeat = ticksPerBeat(p.meter);
  for (let tick = Math.ceil(from / s) * s; tick < to; tick += s) {
    const x = Math.round(X(tick));
    const color = tick % perBar === 0 ? "color.gray.6" : tick % perBeat === 0 ? "color.gray.4" : "color.gray.3";
    out.rect(x, box.y0, x + HAIR, box.y1, rgb(color));
  }

  const rowTop = (key: number) => Math.round(Y(key));
  const noteFrame = (x0: number, x1: number, y: number, color: ColorName) =>
    frame(out, Math.round(x0) + HAIR, y, Math.max(Math.round(x0) + 2 * HAIR, Math.round(x1) - HAIR), y + noteH, color);
  const n = p.shown;
  const sel = p.selected;
  /** ドラッグで動かしているノート（もとの所に、あとで accent の枠を描く）。 */
  const moving: number[] = [];

  if (n) {
    eachNoteIn(n, from, to, (i) => {
      const mark = marks ? marks[i]! : 0;
      const key = n.take.keys[i]!;
      const y = rowTop(key);
      if (y > box.y1 || y + rowH < box.y0) return;
      const x0 = X(noteStart(n, i));
      const x1 = X(noteEnd(n, i));
      if (mark === 1) {
        moving.push(i);
        return;
      }
      if (mark === 2) {
        const h = Math.round(sizes.endMark * dpr);
        const top = Math.round(y + rowH / 2 - h / 2);
        const x = Math.round(x1);
        out.rect(x, top, x + HAIR, top + h, rgb(FRAME));
        return;
      }
      const face: ColorName =
        mark === 4 ? FRAME : mark === 3 || (sel && sel[i]) ? "color.accent" : velocityColor(n.take.velocities[i]!);
      paintNote(out, x0, x1, y, noteH, edge, face);
    });
  }

  if (n && p.origin) {
    const o = p.origin;
    eachNoteIn(n, from - o.maxLength - n.maxLength, to + o.maxLength, (i) => {
      const j = movedFrom(n.take, o.take, i);
      if (j < 0) return;
      const x0 = X(noteStart(o, j));
      const x1 = X(noteEnd(o, j));
      if (x1 < box.x0 || x0 > box.x1) return;
      noteFrame(x0, x1, rowTop(o.take.keys[j]!), sel && sel[i] ? "color.accent" : FRAME);
    });
  }

  if (n) for (const i of moving) noteFrame(X(noteStart(n, i)), X(noteEnd(n, i)), rowTop(n.take.keys[i]!), "color.accent");

  if (p.live) {
    for (const l of p.live) {
      if (l.length === null) continue;
      const x0 = X(l.start);
      const x1 = X(l.start + l.length);
      if (x1 < box.x0 || x0 > box.x1) continue;
      paintNote(out, x0, x1, rowTop(l.key), noteH, edge, velocityColor(l.velocity));
    }
  }
}

/** ベロシティの帯: まん中の線と、ノートごとの棒（上はしに横線）。box は帯の上の線より下。 */
export function paintVelocityLane(out: Batch, box: Box, p: RollPicture, dpr: number): void {
  const { g, sizes } = p;
  const ppt = g.ppt * dpr;
  const X = (tick: number) => box.x0 + (tick - g.scrollTick) * ppt;
  const mid = box.y0 + Math.round(sizes.velMid * dpr);
  out.rect(box.x0, mid, box.x1, mid + HAIR, rgb("color.gray.4"));
  const barW = Math.round(sizes.velBar * dpr);
  const capW = Math.round(sizes.velCap * dpr);
  const capOff = Math.floor((capW - barW) / 2);
  const full = sizes.velFull * dpr;
  const from = g.scrollTick - 1;
  const to = g.scrollTick + (g.width + sizes.velCap) / g.ppt;
  const bar = (tick: number, v: number, color: ColorName) => {
    const x = Math.round(X(tick)) + HAIR;
    const h = Math.max(HAIR, Math.round((v * full) / 127));
    const top = box.y1 - h;
    out.rect(x, top, x + barW, box.y1, rgb(color));
    out.rect(x - capOff, top, x - capOff + capW, top + HAIR, rgb(color));
  };
  const cap = (tick: number, v: number) => {
    const x = Math.round(X(tick)) + HAIR;
    const top = box.y1 - Math.max(HAIR, Math.round((v * full) / 127));
    out.rect(x - capOff, top, x - capOff + capW, top + HAIR, rgb(FRAME));
  };

  const n = p.shown;
  if (n) {
    const d = p.drag?.kind === "velocity" ? p.drag : null;
    const dragged = d ? new Set(d.notes) : null;
    const sel = p.selected;
    eachNoteIn(n, from, to, (i) => {
      const s = noteStart(n, i);
      if (s < from || s >= to) return;
      const v = n.take.velocities[i]!;
      const color: ColorName = sel && sel[i] ? "color.accent" : velocityColor(v);
      if (d && dragged!.has(i)) {
        bar(s, draggedVelocity(v, d.delta), color);
        cap(s, v);
      } else {
        bar(s, v, color);
      }
    });
  }
  if (p.live) {
    for (const l of p.live) {
      if (l.start < from || l.start >= to) continue;
      bar(l.start, l.velocity, l.length === null ? "color.rec" : velocityColor(l.velocity));
    }
  }
}

/** 鍵盤の列の、鍵の所（音の名前の欄の右）。白鍵 gray.5、黒鍵 gray.0、光る鍵 accent、下に gray.1 の線。 */
export function paintRollKeys(out: Batch, box: Box, g: RollGeom, lit: (key: number) => boolean, dpr: number): void {
  const rowH = g.rowH * dpr;
  out.rect(box.x0, box.y0, box.x1, box.y1, rgb("color.gray.1"));
  const rows = rowsShown(g) + 1;
  for (let r = 0; r < rows; r++) {
    const key = g.topKey - r;
    if (key < 0) break;
    const y = box.y0 + r * rowH;
    const color: ColorName = lit(key) ? "color.accent" : isBlack(key) ? "color.gray.0" : "color.gray.5";
    out.rect(box.x0, y, box.x1, y + rowH - HAIR, rgb(color));
  }
}

/** 動いている物の面の中身。ノートは、面の箱の左上を (left, top)（ノートの場所の CSS px）として置く。 */
export interface LivePicture {
  g: RollGeom;
  sizes: RollSizes;
  /** 面の箱の、動かす前の左上（ノートの場所の CSS px） */
  left: number;
  top: number;
  /** 面の箱を動かしている縦の量（CSS px）と、ノートの場所の高さ。ノートは、その下の帯にはみ出さない。 */
  ty: number;
  areaH: number;
  notes: readonly { start: number; end: number; key: number; face: ColorName }[];
}

export function paintLive(out: Batch, box: Box, p: LivePicture, dpr: number): void {
  const { g, sizes } = p;
  const noteH = Math.round(sizes.noteH * dpr);
  const edge = Math.round(sizes.edge * dpr);
  const ox = box.x0 - p.left * dpr;
  const oy = box.y0 - p.top * dpr;
  const areaTop = oy - p.ty * dpr;
  out.clip(box.x0, Math.max(box.y0, Math.round(areaTop)), box.x1, Math.min(box.y1, Math.round(areaTop + p.areaH * dpr)));
  for (const n of p.notes) {
    const x0 = ox + (n.start - g.scrollTick) * g.ppt * dpr;
    const x1 = ox + (n.end - g.scrollTick) * g.ppt * dpr;
    const y = Math.round(oy + (g.topKey - n.key) * g.rowH * dpr);
    paintNote(out, x0, x1, y, noteH, edge, n.face);
  }
}

// ---- CC の帯（モック R3-Lanes） ----

/** CC の帯 1 本の絵が使う物。 */
export interface LanePicture {
  g: RollGeom;
  meter: Meter;
  step: number;
  /** null は「そのほかの CC」の番号をまだえらんでいない帯。 */
  control: ControlKind | null;
  shown: Shown | null;
  lane: LaneBox;
  /** part.cc.point（CSS px） */
  point: number;
  /** 選んだ点の印（テイクのコントロールの番号）。 */
  selected: Uint8Array | null;
  /** この帯のドラッグ。 */
  drag: LaneDrag | null;
  eraseHover: number;
}

const LINE: ColorName = "color.gray.9";

/** 1 本の点の列の、点と点をつなぐ線（画素）。step: オン ・ オフの CC は段（次の点まで値をたもつ）、ほかは点から点へ。 */
function laneLine(out: Batch, xs: readonly number[], ys: readonly number[], x0: number, x1: number, restY: number, step: boolean): void {
  const c = rgb(LINE);
  const flat = (a: number, b: number, y: number) => out.rect(Math.min(a, b), y, Math.max(a, b) + HAIR, y + HAIR, c);
  const upright = (x: number, a: number, b: number) => out.rect(x, Math.min(a, b), x + HAIR, Math.max(a, b) + HAIR, c);
  let px = x0;
  let py = restY;
  for (let k = 0; k < xs.length; k++) {
    const x = xs[k]!;
    const y = ys[k]!;
    if (k === 0 || step) {
      flat(px, x, py);
      upright(x, py, y);
    } else {
      sloped(out, px, py, x, y, c);
    }
    px = x;
    py = y;
  }
  flat(px, x1, py);
}

/** 斜めの線を、同じ高さの所をまとめた 1 画素の四角の列で。 */
function sloped(out: Batch, xa: number, ya: number, xb: number, yb: number, c: ReturnType<typeof rgb>): void {
  if (xb <= xa) {
    out.rect(xa, Math.min(ya, yb), xa + HAIR, Math.max(ya, yb) + HAIR, c);
    return;
  }
  const at = (x: number) => Math.round(ya + ((yb - ya) * (x - xa)) / (xb - xa));
  let runX = xa;
  let runY = at(xa);
  for (let x = xa + 1; x <= xb; x++) {
    const y = at(x);
    if (y === runY) continue;
    // 次の高さへは、縦につないで切れ目を作らない。
    out.rect(runX, runY, x, runY + HAIR, c);
    out.rect(x, Math.min(runY, y), x + HAIR, Math.max(runY, y) + HAIR, c);
    runX = x;
    runY = y;
  }
  out.rect(runX, runY, xb + HAIR, runY + HAIR, c);
}

function laneSquare(out: Batch, cx: number, cy: number, size: number, color: ColorName, hollow: boolean): void {
  const x0 = Math.round(cx - size / 2);
  const y0 = Math.round(cy - size / 2);
  if (hollow) frame(out, x0, y0, x0 + size, y0 + size, color);
  else out.rect(x0, y0, x0 + size, y0 + size, rgb(color));
}

/** 帯の地と線: gray.1 の地、小節の線 gray.4、ほかの線 gray.2、ピッチベンドは 0 の所に gray.5 の線。 */
function laneGround(out: Batch, box: Box, p: LanePicture, dpr: number): void {
  const { g } = p;
  out.rect(box.x0, box.y0, box.x1, box.y1, rgb("color.gray.1"));
  const s = lineStep(p.step, p.meter, g.ppt);
  const perBar = ticksPerBar(p.meter);
  const to = g.scrollTick + g.width / g.ppt;
  for (let tick = Math.ceil(g.scrollTick / s) * s; tick < to; tick += s) {
    const x = Math.round(box.x0 + (tick - g.scrollTick) * g.ppt * dpr);
    out.rect(x, box.y0, x + HAIR, box.y1, rgb(tick % perBar === 0 ? "color.gray.4" : "color.gray.2"));
  }
  if (p.control?.kind === "pitchBend") {
    const y = Math.round(box.y0 + valueToY(p.control, p.lane, 0) * dpr);
    out.rect(box.x0, y, box.x1, y + HAIR, rgb("color.gray.5"));
  }
}

/** 線と点。moving の点は、ドラッグの量だけ動かして描く。 */
function laneDots(out: Batch, box: Box, p: LanePicture, dpr: number, moving: Set<number> | null, face: (i: number) => ColorName): void {
  const n = p.shown;
  const c = p.control;
  if (!n || !c) return;
  const { g } = p;
  const all = lanePoints(n.take, c);
  const X = (tick: number) => Math.round(box.x0 + (tick - g.scrollTick) * g.ppt * dpr);
  const Y = (v: number) => Math.round(box.y0 + valueToY(c, p.lane, v) * dpr);
  // 見えている所の点と、その前後の 1 つずつ（線が、はしの外からつながるように）。
  const from = g.scrollTick;
  const to = g.scrollTick + g.width / g.ppt;
  let list: { tick: number; value: number; i: number }[];
  if (moving && p.drag?.kind === "move") {
    const sorted = Array.from(all, (i) => ({ ...draggedPoint(n, c, p.drag, moving, i), i })).sort((a, b) => a.tick - b.tick);
    let a = 0;
    while (a < sorted.length && sorted[a]!.tick < from) a++;
    let b = a;
    while (b < sorted.length && sorted[b]!.tick <= to) b++;
    list = sorted.slice(Math.max(0, a - 1), Math.min(sorted.length, b + 1));
  } else {
    const k0 = Math.max(0, firstPointFrom(n, all, from) - 1);
    list = [];
    for (let k = k0; k < all.length; k++) {
      const i = all[k]!;
      const tick = pointTick(n, i);
      list.push({ tick, value: n.take.controlValues[i]!, i });
      if (tick > to) break;
    }
  }
  const before = list.length > 0 && list[0]!.tick < from;
  const xs = list.map((q) => X(q.tick));
  const ys = list.map((q) => Y(q.value));
  const restY = before ? ys[0]! : Y(0);
  laneLine(out, xs, ys, box.x0, box.x1, restY, isOnOff(c));
  const size = Math.round(p.point * dpr);
  for (let k = 0; k < list.length; k++) laneSquare(out, xs[k]!, ys[k]!, size, face(list[k]!.i), false);
}

/** CC の帯の大きな絵。点を動かしている間は、点と線を動く面にまかせ、もとの位置に gray.7 の枠だけを描く。 */
export function paintLane(out: Batch, box: Box, p: LanePicture, dpr: number): void {
  laneGround(out, box, p, dpr);
  const n = p.shown;
  const c = p.control;
  if (!n || !c) return;
  const d = p.drag;
  if (d?.kind === "move" && d.press.moved) {
    const size = Math.round(p.point * dpr);
    for (const i of d.points) {
      const x = box.x0 + (pointTick(n, i) - p.g.scrollTick) * p.g.ppt * dpr;
      laneSquare(out, x, box.y0 + valueToY(c, p.lane, n.take.controlValues[i]!) * dpr, size, FRAME, true);
    }
    return;
  }
  const hits = d?.kind === "box" ? new Set(d.hits) : d?.kind === "erase" ? d.hits : null;
  const erasing = d?.kind === "erase";
  const sel = p.selected;
  laneDots(out, box, p, dpr, null, (i) => {
    if (i === p.eraseHover || (erasing && hits!.has(i))) return FRAME;
    if ((hits && !erasing && hits.has(i)) || (sel && sel[i])) return "color.accent";
    return "color.gray.10";
  });
}

/** CC の帯の動く面: 動かしている間の線と点、書く道具で置く前の点（accent の枠）。box は帯 1 本ぶん。 */
export function paintLaneLive(out: Batch, box: Box, p: LanePicture, dpr: number): void {
  const n = p.shown;
  const c = p.control;
  const d = p.drag;
  if (!c || !d) return;
  if (d.kind === "add") {
    const x = box.x0 + (d.tick - p.g.scrollTick) * p.g.ppt * dpr;
    laneSquare(out, x, box.y0 + valueToY(c, p.lane, d.value) * dpr, Math.round(p.point * dpr), "color.accent", true);
    return;
  }
  if (d.kind !== "move" || !d.press.moved || !n) return;
  const moving = new Set(d.points);
  const sel = p.selected;
  laneDots(out, box, p, dpr, moving, (i) => (moving.has(i) || (sel && sel[i]) ? "color.accent" : "color.gray.10"));
}

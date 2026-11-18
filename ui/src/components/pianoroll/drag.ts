// ピアノロールのマウス（仕様書 3 章「第 3 回のための決まり」、モック R3-PRMouse・R3-Lanes）。DOM を知らない。
// 押す → 動かす → 離す。動かしている間は見た目だけを変え（drag を描く）、離したときに曲を変える意図を 1 つだけ送る。
// Esc（cancel）で、動かす前に戻す。座標はノートの場所（ベロシティの帯も同じ横の座標）の CSS px。

import type { Command, MusicalScale } from "../../bridge";
import type { Meter } from "../time";
import { chordOf } from "./audition";
import {
  clampVelocity,
  drawLength,
  drawStart,
  hitNote,
  indexesOf,
  keyToY,
  lengthText,
  moveDelta,
  noteEnd,
  noteName,
  noteShown,
  noteStart,
  notesInBox,
  positionText,
  resizeDelta,
  resizeStartDelta,
  signed,
  signedLength,
  tickToX,
  velocityDelta,
  writable,
  xToTick,
  yToKey,
  type RollGeom,
  type Shown,
} from "./geom";
import type { Tool } from "./view.svelte";
import { t } from "../../text/ja";
import type { IndexRange } from "../../bridge";

export interface Mods {
  shift: boolean;
  alt: boolean;
  /** Ctrl（Mac は Cmd） */
  mod: boolean;
}

interface Press {
  x: number;
  y: number;
  /** space.1 より動いたら true（それまでは、ただ押しただけ） */
  moved: boolean;
}

export type Drag =
  | { kind: "move"; press: Press; notes: number[]; grab: number; copy: boolean; ticks: number; keys: number; selectOnClick: boolean }
  /** edge: 動かすはし。start は始まりを動かし、ticks が + なら短くなる。 */
  | { kind: "resize"; press: Press; notes: number[]; grab: number; edge: "start" | "end"; ticks: number }
  | { kind: "draw"; press: Press; start: number; key: number; length: number; velocity: number }
  | { kind: "box"; press: Press; x: number; y: number; add: boolean; hits: number[] }
  | { kind: "erase"; press: Press; x: number; y: number; hits: Set<number> }
  | { kind: "velocity"; press: Press; notes: number[]; grab: number; delta: number };

/** カーソルの形。pencil と eraser は meloet の部品、ほかは OS の形。 */
export type CursorKind = "default" | "move" | "copy" | "ew-resize" | "ns-resize" | "not-allowed" | "pencil" | "eraser";

export interface RollInput {
  shown(): Shown | null;
  geom(): RollGeom;
  /** グリッド 1 つのティック */
  step(): number;
  scale(): MusicalScale;
  meter(): Meter;
  tool(): Tool;
  /** pianoRoll.selectedNotes（見せているテイクの物でなければ空） */
  selected(): readonly IndexRange[];
  /** part.note.resize、space.1（押しただけとみなす動き）、ベロシティの 127 の高さ（CSS px） */
  edgePx(): number;
  stillPx(): number;
  velocityFullPx(): number;
  /** ベロシティの帯の高さ（棒の下はし、CSS px） */
  velocityHeight(): number;
  send(command: Command): void;
  /** 書く道具のノートと、その試聴の強さ（ベロシティ固定がオンならその値、ほかは 100）。 */
  drawVelocity(): number;
  /** ノートを押した ・ 動かしたときに鳴らすか（止まっているときだけ）。 */
  mayAudition(): boolean;
  /** ticks の長さが何 ms か（テンポから）。 */
  ticksToMs(ticks: number): number;
  /** 画面が鳴らす音（audition.ts の Auditioner）。 */
  audition: { play(keys: readonly number[], velocity: number, ms?: number | null): void; stop(): void };
}

export class RollDrag {
  drag: Drag | null = null;
  /** 消す道具を乗せている所のノート（-1 はなし） */
  eraseHover = -1;

  constructor(private readonly input: RollInput) {}

  private selectedSet(): Set<number> {
    return new Set(indexesOf(this.input.selected()));
  }

  private takeId(): number | null {
    return this.input.shown()?.take.id ?? null;
  }

  /** 何も押していないときのカーソルと、消す道具で乗せているノート。変わったら true。 */
  hover(x: number, y: number, mods: Mods): { cursor: CursorKind; changed: boolean } {
    const i = this.input;
    const hit = hitNote(i.shown(), i.geom(), x, y, i.edgePx(), true);
    const erase = i.tool() === "erase" && hit && !hit.edge ? hit.note : -1;
    const changed = erase !== this.eraseHover;
    this.eraseHover = erase;
    let cursor: CursorKind;
    if (hit?.edge) cursor = "ew-resize";
    else if (i.tool() === "erase") cursor = "eraser";
    else if (hit) cursor = mods.alt ? "copy" : "move";
    else if (i.tool() === "draw") cursor = writable(yToKey(i.geom(), y), i.scale()) ? "pencil" : "not-allowed";
    else cursor = "default";
    return { cursor, changed };
  }

  /** ドラッグしている間のカーソル。 */
  cursor(): CursorKind {
    const d = this.drag;
    if (!d) return "default";
    switch (d.kind) {
      case "move":
        return d.copy ? "copy" : "move";
      case "resize":
        return "ew-resize";
      case "draw":
        return "pencil";
      case "erase":
        return "eraser";
      case "velocity":
        return "ns-resize";
      default:
        return "default";
    }
  }

  /** ノートの場所で押した。 */
  down(x: number, y: number, mods: Mods): void {
    const i = this.input;
    const n = i.shown();
    const g = i.geom();
    const press: Press = { x, y, moved: false };
    const hit = hitNote(n, g, x, y, i.edgePx(), true);
    const selected = this.selectedSet();
    this.eraseHover = -1;

    if (hit && n && hit.edge) {
      if (!selected.has(hit.note)) i.send({ type: "selectNotes", take: n.take.id, notes: [hit.note], mode: "replace" });
      const notes = selected.has(hit.note) ? [...selected] : [hit.note];
      this.drag = { kind: "resize", press, notes, grab: hit.note, edge: hit.start ? "start" : "end", ticks: 0 };
      return;
    }
    if (i.tool() === "erase") {
      this.drag = { kind: "erase", press, x, y, hits: new Set(hit ? [hit.note] : []) };
      return;
    }
    if (hit && n) {
      if (mods.mod) {
        i.send({ type: "selectNotes", take: n.take.id, notes: [hit.note], mode: "toggle" });
        return;
      }
      let notes: number[];
      let selectOnClick = false;
      if (selected.has(hit.note)) {
        notes = [...selected];
        selectOnClick = !mods.shift;
      } else {
        i.send({ type: "selectNotes", take: n.take.id, notes: [hit.note], mode: mods.shift ? "add" : "replace" });
        notes = mods.shift ? [...selected, hit.note] : [hit.note];
      }
      this.drag = { kind: "move", press, notes, grab: hit.note, copy: mods.alt, ticks: 0, keys: 0, selectOnClick };
      // 押したノートを、その高さとベロシティで、離すまで（ノートの長さで止める）。
      if (i.mayAudition()) i.audition.play([n.take.keys[hit.note]!], n.take.velocities[hit.note]!, i.ticksToMs(n.take.lengths[hit.note]!));
      return;
    }
    if (i.tool() === "draw") {
      const key = yToKey(g, y);
      const step = i.step();
      const start = drawStart(xToTick(g, x), mods.shift ? null : step);
      const velocity = i.drawVelocity();
      if (!writable(key, i.scale())) {
        // 書かない。Rust が状態行で言う（仕様書 6 章「音階の外に書こうとした」）。
        i.send({ type: "addNote", take: this.takeId(), start, length: step, key, velocity });
        return;
      }
      i.audition.play([key], velocity);
      this.drag = { kind: "draw", press, start, key, length: step, velocity };
      return;
    }
    this.drag = { kind: "box", press, x, y, add: mods.shift, hits: [] };
  }

  /** ベロシティの帯で押した。棒の上はしの近く（横は横線の幅、縦は space.1）でなければ何もしない。 */
  downVelocity(x: number, y: number): void {
    const i = this.input;
    const n = i.shown();
    if (!n) return;
    const g = i.geom();
    const full = i.velocityFullPx();
    const bottom = i.velocityHeight();
    const near = i.stillPx();
    let grab = -1;
    let best = Infinity;
    const t0 = xToTick(g, x - near);
    const t1 = xToTick(g, x + near);
    for (let k = 0; k < n.take.starts.length; k++) {
      const s = noteStart(n, k);
      if (s < t0 || s > t1 || !noteShown(n, k)) continue;
      const top = bottom - (n.take.velocities[k]! * full) / 127;
      const d = Math.abs(y - top) + Math.abs(x - tickToX(g, s));
      if (Math.abs(y - top) <= near && d < best) {
        best = d;
        grab = k;
      }
    }
    if (grab < 0) return;
    const selected = this.selectedSet();
    const notes = selected.has(grab) ? [...selected] : [grab];
    this.drag = { kind: "velocity", press: { x, y, moved: false }, notes, grab, delta: 0 };
  }

  /** 棒の上はしに乗っているか（カーソルを上下の形にする）。 */
  overVelocityBar(x: number, y: number): boolean {
    const keep = this.drag;
    this.downVelocity(x, y);
    const over = this.drag !== keep;
    this.drag = keep;
    return over;
  }

  /** 押したまま動かした。見た目が変わったら true。 */
  move(x: number, y: number, mods: Mods): boolean {
    const d = this.drag;
    const i = this.input;
    const n = i.shown();
    if (!d) return false;
    if (!d.press.moved) {
      if (Math.hypot(x - d.press.x, y - d.press.y) < i.stillPx() && d.kind !== "erase") return false;
      d.press.moved = true;
    }
    const g = i.geom();
    const step = mods.shift ? null : i.step();
    switch (d.kind) {
      case "move": {
        if (!n) return false;
        const dKeys = yToKey(g, y) - yToKey(g, d.press.y);
        const { ticks, keys } = moveDelta(n, d.notes, d.grab, (x - d.press.x) / g.ppt, dKeys, step);
        const copy = mods.alt;
        if (ticks === d.ticks && keys === d.keys && copy === d.copy) return false;
        // 高さが変わるたびに、動かしているノートを和音で。
        if (keys !== d.keys && i.mayAudition()) {
          i.audition.play(
            chordOf(d.notes.map((k) => n.take.keys[k]!), keys),
            n.take.velocities[d.grab]!,
            i.ticksToMs(n.take.lengths[d.grab]!),
          );
        }
        d.ticks = ticks;
        d.keys = keys;
        d.copy = copy;
        return true;
      }
      case "resize": {
        if (!n) return false;
        const dx = (x - d.press.x) / g.ppt;
        const ticks = d.edge === "start" ? resizeStartDelta(n, d.notes, d.grab, dx, step) : resizeDelta(n, d.notes, d.grab, dx, step);
        if (ticks === d.ticks) return false;
        d.ticks = ticks;
        return true;
      }
      case "draw": {
        const length = drawLength(d.start, xToTick(g, x), i.step(), step !== null);
        if (length === d.length) return false;
        d.length = length;
        return true;
      }
      case "box": {
        d.x = x;
        d.y = y;
        d.hits = notesInBox(n, g, d.press.x, d.press.y, x, y);
        return true;
      }
      case "erase": {
        const before = d.hits.size;
        // 速く動かしても間のノートを飛ばさないように、半行ずつ見る。
        const steps = Math.max(1, Math.ceil(Math.hypot(x - d.x, y - d.y) / (g.rowH / 2)));
        for (let s = 1; s <= steps; s++) {
          const px = d.x + ((x - d.x) * s) / steps;
          const py = d.y + ((y - d.y) * s) / steps;
          const hit = hitNote(n, g, px, py, 0);
          if (hit) d.hits.add(hit.note);
        }
        d.x = x;
        d.y = y;
        return d.hits.size !== before;
      }
      case "velocity": {
        const delta = velocityDelta(d.press.y - y, i.velocityFullPx());
        if (delta === d.delta) return false;
        d.delta = delta;
        return true;
      }
    }
  }

  /** 離した。送った意図（曲を変える物か、選ぶ物）を返す。 */
  up(): Command | null {
    const d = this.drag;
    this.drag = null;
    if (!d) return null;
    const i = this.input;
    const take = this.takeId();
    const command = finish(d, take);
    if (d.kind === "draw" || d.kind === "move") i.audition.stop();
    if (command) i.send(command);
    return command;
  }

  /** Esc: ドラッグをやめて、動かす前に戻す。やめたら true。 */
  cancel(): boolean {
    const d = this.drag;
    if (!d) return false;
    this.drag = null;
    if (d.kind === "draw" || d.kind === "move") this.input.audition.stop();
    return true;
  }

  /** 「その場の読み値」と状態行に出す文。出さないときは null。 */
  reading(): string | null {
    const d = this.drag;
    const n = this.input.shown();
    const meter = this.input.meter();
    if (!d) return null;
    switch (d.kind) {
      case "draw":
        return t("pr.readDraw", { key: noteName(d.key), position: positionText(d.start, meter), length: lengthText(d.length, meter) });
      case "move":
        return d.press.moved ? t("pr.readMove", { ticks: signedLength(d.ticks, meter), keys: signed(d.keys) }) : null;
      case "resize": {
        if (!d.press.moved || !n) return null;
        const length = n.take.lengths[d.grab]!;
        const to = d.edge === "start" ? length - d.ticks : length + d.ticks;
        return t("pr.readResize", { from: lengthText(length, meter), to: lengthText(to, meter) });
      }
      case "box":
        return d.press.moved ? t("pr.readBox", { count: d.hits.length }) : null;
      case "velocity":
        return d.press.moved ? t("pr.readVelocity", { delta: signed(d.delta), count: d.notes.length }) : null;
      default:
        return null;
    }
  }

  /** 動かしているノートの、いまの見た目の箱（ノートの場所の CSS px）。読み値やライブの面の置き場所に使う。 */
  movedBox(): { x0: number; x1: number; y0: number; y1: number } | null {
    const d = this.drag;
    const n = this.input.shown();
    if (!d || !n || d.kind !== "move") return null;
    const g = this.input.geom();
    let x0 = Infinity;
    let x1 = -Infinity;
    let y0 = Infinity;
    let y1 = -Infinity;
    for (const k of d.notes) {
      x0 = Math.min(x0, tickToX(g, noteStart(n, k)));
      x1 = Math.max(x1, tickToX(g, noteEnd(n, k)));
      y0 = Math.min(y0, keyToY(g, n.take.keys[k]!));
      y1 = Math.max(y1, keyToY(g, n.take.keys[k]!) + g.rowH);
    }
    return { x0, x1, y0, y1 };
  }
}

/** 離したときに送る意図。押しただけ、動かしても何も変わらないときは、選ぶ意図か null。 */
export function finish(d: Drag, take: number | null): Command | null {
  switch (d.kind) {
    case "move":
      if (take === null) return null;
      if (!d.press.moved || (d.ticks === 0 && d.keys === 0)) {
        return !d.press.moved && d.selectOnClick ? { type: "selectNotes", take, notes: [d.grab], mode: "replace" } : null;
      }
      return { type: "moveNotes", take, notes: d.notes, ticks: d.ticks, keys: d.keys, copy: d.copy };
    case "resize":
      return take !== null && d.ticks !== 0 ? { type: "resizeNotes", take, notes: d.notes, edge: d.edge, ticks: d.ticks } : null;
    case "draw":
      return { type: "addNote", take, start: d.start, length: d.length, key: d.key, velocity: d.velocity };
    case "box":
      if (!d.press.moved || (d.hits.length === 0 && !d.add)) return d.add ? null : { type: "clearNoteSelection" };
      if (take === null || d.hits.length === 0) return null;
      return { type: "selectNotes", take, notes: d.hits, mode: d.add ? "add" : "replace" };
    case "erase":
      return take !== null && d.hits.size > 0 ? { type: "deleteNotes", take, notes: [...d.hits].sort((a, b) => a - b) } : null;
    case "velocity":
      return take !== null && d.press.moved && d.delta !== 0 ? { type: "changeVelocity", take, notes: d.notes, delta: d.delta } : null;
  }
}

/** ベロシティの帯で動かしている間の値（1〜127 で止まる）。 */
export function draggedVelocity(v: number, delta: number): number {
  return clampVelocity(v + delta);
}

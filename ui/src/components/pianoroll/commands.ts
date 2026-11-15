// キー（仕様書 4 章）のうち、ピアノロールにフォーカスがあるときの意図。DOM を知らない純粋な関数。

import type { AppState, Command } from "../../bridge";
import type { Meter } from "../time";
import { duplicateTicks, gridTicks, indexesOf, snapTick, type Shown } from "./geom";

export type PianoRollKey =
  | { kind: "nudge"; steps: number; keys: number }
  | { kind: "deleteNotes" }
  /** Delete で、点とノートのうち点のほうを消す（点だけを選んでいるか、最後に押したのが点のとき）。 */
  | { kind: "deleteControls" }
  | { kind: "quantize" }
  | { kind: "selectAll" }
  | { kind: "duplicate" }
  | { kind: "cut" }
  | { kind: "copy" }
  | { kind: "paste" };

export interface PianoRollKeyContext {
  state: AppState;
  /** 見せているテイク（なければ null） */
  shown: Shown | null;
  /** いまの再生の位置（ティック） */
  playTick: number;
}

/** null は「何もしない」。 */
export function pianoRollCommand(key: PianoRollKey, c: PianoRollKeyContext): Command | null {
  const pr = c.state.pianoRoll;
  const meter: Meter = { num: c.state.song.meterNum, den: c.state.song.meterDen };
  const step = gridTicks(pr.grid, meter);
  if (key.kind === "paste") {
    if (pr.clipboardNotes === 0) return null;
    return { type: "pasteNotes", at: Math.max(0, snapTick(c.playTick, step)) };
  }
  if (key.kind === "selectAll") return { type: "selectAllNotes" };
  const n = c.shown;
  if (!n || pr.take !== n.take.id) return null;
  if (key.kind === "deleteControls") {
    const points = indexesOf(pr.selectedControls).filter((i) => i < n.take.controlTicks.length);
    return points.length > 0 ? { type: "deleteControls", take: n.take.id, points } : null;
  }
  if (key.kind === "quantize") {
    if (n.take.starts.length === 0) return null;
    // 何も選んでいなければ、空の notes（テイクの全部のノート）。
    return { type: "quantize", take: n.take.id, notes: indexesOf(pr.selectedNotes).filter((i) => i < n.take.starts.length) };
  }
  const notes = indexesOf(pr.selectedNotes).filter((i) => i < n.take.starts.length);
  if (notes.length === 0) return null;
  const take = n.take.id;
  switch (key.kind) {
    case "nudge":
      return { type: "moveNotes", take, notes, ticks: key.steps * step, keys: key.keys, copy: false };
    case "deleteNotes":
      return { type: "deleteNotes", take, notes };
    case "duplicate":
      return { type: "moveNotes", take, notes, ticks: duplicateTicks(n, notes, step), keys: 0, copy: true };
    case "cut":
      return { type: "cutNotes", take, notes };
    case "copy":
      return { type: "copyNotes", take, notes };
  }
}

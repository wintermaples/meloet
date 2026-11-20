// ピアノロールのテストで使うテイクと見かた。
import { NO_FROM, type TakeNotes } from "../src/bridge";
import type { RollGeom } from "../src/components/pianoroll/geom";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

/** [始まり, 長さ, 鍵, ベロシティ] のならびからテイクを作る。 */
export function take(notes: [number, number, number, number?][], id = 7, extra: Partial<TakeNotes> = {}): TakeNotes {
  return {
    id,
    length: 4 * 4 * TPQ,
    origin: 0,
    starts: Uint32Array.from(notes.map((n) => n[0])),
    lengths: Uint32Array.from(notes.map((n) => n[1])),
    keys: Uint8Array.from(notes.map((n) => n[2])),
    velocities: Uint8Array.from(notes.map((n) => n[3] ?? 80)),
    froms: new Uint32Array(notes.length).fill(NO_FROM),
    controlTicks: new Uint32Array(0),
    controlKinds: new Uint8Array(0),
    controlNumbers: new Uint8Array(0),
    controlValues: new Int16Array(0),
    ...extra,
  };
}

/** 1 拍 40 px（1 ティック 40 / TPQ px）、上の行が C5、1 行 12 px、696 × 492（モック A-PianoRoll の大きさ）。 */
export const GEOM: RollGeom = { scrollTick: 0, ppt: 40 / TPQ, topKey: 72, rowH: 12, width: 696, height: 492 };

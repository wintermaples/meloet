// ピアノロールで音を鳴らす（docs/design-notes.md B-62）。DOM を知らない。
// 鳴らすのは Rust（audition）。どれだけ鳴らすかは画面がはかって、止めるときに空の audition を送る。
// 鳴らしている鍵は、鍵盤の列で光らせる（Rust は audition の鍵を Event::Note で返さないので、画面が自分で光らせる）。

import type { AppState, Command } from "../../bridge";
import { TICKS_PER_QUARTER } from "../../time/ticks";
import { DRAW_VELOCITY } from "./geom";

/** 和音で鳴らすのは 16 個まで。 */
export const MAX_CHORD = 16;
/** ↑ ↓ で動かしたときに鳴らす長さ（ms）。 */
export const NUDGE_MS = 300;

/** 鍵盤の列と書く道具の強さ: ベロシティ固定がオンならその値、ほかは 100（B-66）。 */
export function fixedOr100(state: Pick<AppState, "keyboards">): number {
  const f = state.keyboards.velocityFixed;
  return f.on ? f.value : DRAW_VELOCITY;
}

/** ノートを押した ・ 動かした ・ ↑ ↓ で鳴らすか。止まっているときだけ（再生中、録音中、カウントイン中は鳴らさない）。 */
export const notesMayAudition = (state: Pick<AppState, "transport">) => state.transport.state === "stopped";

/** ticks（4 分音符 = TICKS_PER_QUARTER）が、テンポ bpm で何 ms か。 */
export function ticksToMs(ticks: number, bpm: number): number {
  return bpm > 0 ? (ticks / TICKS_PER_QUARTER) * (60000 / bpm) : 0;
}

/** 動かしたノートの和音: 鍵に delta を足し、0〜127 の中の物を、重ならずに低い順で 16 個まで。 */
export function chordOf(keys: Iterable<number>, delta: number): number[] {
  const out = new Set<number>();
  for (const k of keys) {
    const moved = k + delta;
    if (moved >= 0 && moved <= 127) out.add(moved);
  }
  return [...out].sort((a, b) => a - b).slice(0, MAX_CHORD);
}

export interface Timer {
  set(run: () => void, ms: number): unknown;
  clear(id: unknown): void;
}

const realTimer: Timer = {
  set: (run, ms) => setTimeout(run, ms),
  clear: (id) => clearTimeout(id as ReturnType<typeof setTimeout>),
};

/** いま画面が鳴らしている音。play で入れかえ、stop か時間切れで止める。 */
export class Auditioner {
  private timer: unknown = null;
  /** いま鳴らしている鍵（光らせる）。 */
  keys: number[] = [];

  constructor(
    private readonly send: (command: Command) => void,
    private readonly onKeys: (keys: readonly number[]) => void = () => {},
    private readonly clock: Timer = realTimer,
  ) {}

  /** keys を鳴らす（前の音は Rust が止める）。ms があれば、その時間で止める。 */
  play(keys: readonly number[], velocity: number, ms: number | null = null): void {
    this.clearTimer();
    if (keys.length === 0) {
      this.stop();
      return;
    }
    this.send({ type: "audition", keys: [...keys], velocity });
    this.keys = [...keys];
    this.onKeys(this.keys);
    if (ms !== null) this.timer = this.clock.set(() => this.stop(), Math.max(0, ms));
  }

  /** 鳴らしていれば止める。 */
  stop(): void {
    this.clearTimer();
    if (this.keys.length === 0) return;
    this.keys = [];
    this.send({ type: "audition", keys: [], velocity: 0 });
    this.onKeys(this.keys);
  }

  private clearTimer(): void {
    if (this.timer !== null) this.clock.clear(this.timer);
    this.timer = null;
  }
}

type MoveNotes = Extract<Command, { type: "moveNotes" }>;

// ---- ↑ ↓ で動かしたとき（キーの係が pianoRollCommand を呼ぶ。ピアノロールの部品がここに耳を置く） ----

let nudgeListeners: Array<(command: MoveNotes) => void> = [];

/** ↑ ↓ で、選んだノートを動かす意図を作ったら呼ばれる。返り値の関数で外す。 */
export function onNudgeKeys(listener: (command: MoveNotes) => void): () => void {
  nudgeListeners.push(listener);
  return () => {
    nudgeListeners = nudgeListeners.filter((l) => l !== listener);
  };
}

export function nudgedKeys(command: MoveNotes): void {
  for (const l of nudgeListeners) l(command);
}

// フェーダー ・ パン ・ センドのドラッグで送る意図（約束の Command のコメント「commit」）。
// 動かしている間は commit: false（音だけ変わる）を 1 コマに 1 回まで、離したら commit: true を 1 つ（1 手）。
// Esc（cancelDrag）でやめたら CancelMixPreview。動かさずに離したときは何も送らない。

import type { Command } from "../../bridge";
import { forgetHand, noteHand } from "../automation/handValue";

export class MixPreview<V> {
  private pending: { value: V } | null = null;
  private frame = 0;
  private moved = false;
  /** 最後に送った「音だけ変える」意図（書いている帯の線のために、手の位置を覚えておく。automation/handValue.ts）。 */
  private last: Command | null = null;

  constructor(
    private readonly make: (value: V, commit: boolean) => Command,
    private readonly send: (c: Command) => void,
  ) {}

  /** 動いた。次のコマで、いちばん新しい値だけを送る。 */
  move(value: V): void {
    this.moved = true;
    this.pending = { value };
    if (this.frame === 0) this.frame = requestAnimationFrame(() => this.flush());
  }

  /** 離した。 */
  commit(value: V): void {
    this.stop();
    if (this.moved) this.send(this.make(value, true));
  }

  /** やめた（Esc、ポインタを取られた）。 */
  cancel(): void {
    this.stop();
    if (this.moved) this.send({ type: "cancelMixPreview" });
  }

  private flush(): void {
    this.frame = 0;
    const p = this.pending;
    this.pending = null;
    if (!p) return;
    const c = this.make(p.value, false);
    this.last = c;
    noteHand(c);
    this.send(c);
  }

  private stop(): void {
    if (this.last) forgetHand(this.last);
    this.last = null;
    if (this.frame !== 0) cancelAnimationFrame(this.frame);
    this.frame = 0;
    this.pending = null;
  }
}

/** キーを離してから、1 手にまとめるまで（仕様書 3 章 86-7）。 */
export const KEY_COMMIT_MS = 1000;

/**
 * フェーダー ・ パン ・ センドのキー（約束の Command のコメント「keys」）。押すたびに commit: false、キーを離して 1 秒たつか、
 * フォーカスが外れたら（flush）、いちばん新しい値を commit: true で 1 つ。そのあいだに押せば、まとめる時を数えなおす。
 */
export class KeySteps<V> {
  private last: { value: V } | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(
    private readonly make: (value: V, commit: boolean) => Command,
    private readonly send: (c: Command) => void,
  ) {}

  step(value: V): void {
    this.clear();
    this.last = { value };
    this.send(this.make(value, false));
  }

  release(): void {
    if (!this.last) return;
    this.clear();
    this.timer = setTimeout(() => this.flush(), KEY_COMMIT_MS);
  }

  flush(): void {
    this.clear();
    const last = this.last;
    this.last = null;
    if (last) this.send(this.make(last.value, true));
  }

  private clear(): void {
    if (this.timer !== null) clearTimeout(this.timer);
    this.timer = null;
  }
}

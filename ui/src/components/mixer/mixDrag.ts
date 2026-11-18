// フェーダー ・ パン ・ センドのドラッグで送る意図（約束の Command のコメント「commit」）。
// 動かしている間は commit: false（音だけ変わる）を 1 コマに 1 回まで、離したら commit: true を 1 つ（1 手）。
// Esc（cancelDrag）でやめたら CancelMixPreview。動かさずに離したときは何も送らない。

import type { Command } from "../../bridge";

export class MixPreview<V> {
  private pending: { value: V } | null = null;
  private frame = 0;
  private moved = false;

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
    if (p) this.send(this.make(p.value, false));
  }

  private stop(): void {
    if (this.frame !== 0) cancelAnimationFrame(this.frame);
    this.frame = 0;
    this.pending = null;
  }
}

// フェーダー ・ パン ・ センドのつまみに共通のドラッグ。画面だけが先に動き（shown）、もとの値（from）は gray.7 の枠で描く。
// 押した所へは飛ばず、押した所からの動きだけで値を変える。Esc はキーの順のいちばん前でやめる（onDragCancel）。

import { onMount } from "svelte";
import type { Command, StripRef } from "../../bridge";
import { send } from "../../state/app.svelte";
import { onDragCancel } from "../pianoroll/view.svelte";
import { MixPreview } from "./mixDrag";
import { dbToFrac, fracToDb } from "./scale";

export interface MixControlOptions<V> {
  /** 曲の値（Rust の状態）。 */
  value: () => V;
  /** つかんだときの値 start から、dx ・ dy（CSS px）動かしたときの値。box はつかんだときの部品の箱。 */
  read: (start: V, dx: number, dy: number, box: DOMRect) => V;
  make: (value: V, commit: boolean) => Command;
  same?: (a: V, b: V) => boolean;
}

export type MixControl<V> = ReturnType<typeof mixControl<V>>;

export function mixControl<V>(opts: MixControlOptions<V>) {
  const same = opts.same ?? ((a: V, b: V) => a === b);
  let press: { x: number; y: number; pointer: number; box: DOMRect; el: HTMLElement; preview: MixPreview<V> } | null = null;
  let live = $state<{ from: V; now: V } | null>(null);

  function end(): void {
    if (press?.el.hasPointerCapture?.(press.pointer)) press.el.releasePointerCapture(press.pointer);
    press = null;
    live = null;
  }

  function down(e: PointerEvent): void {
    if (e.button !== 0 || press) return;
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture?.(e.pointerId);
    const from = opts.value();
    press = { x: e.clientX, y: e.clientY, pointer: e.pointerId, box: el.getBoundingClientRect(), el, preview: new MixPreview(opts.make, send) };
    live = { from, now: from };
  }

  function move(e: PointerEvent): void {
    if (!press || !live || e.pointerId !== press.pointer) return;
    const next = opts.read(live.from, e.clientX - press.x, e.clientY - press.y, press.box);
    if (same(next, live.now)) return;
    live = { from: live.from, now: next };
    press.preview.move(next);
  }

  function up(e: PointerEvent): void {
    if (!press || !live || e.pointerId !== press.pointer) return;
    press.preview.commit(live.now);
    end();
  }

  function cancel(): boolean {
    if (!press) return false;
    press.preview.cancel();
    end();
    return true;
  }

  onMount(() => onDragCancel(cancel));

  return {
    /** 動かしている間だけ { from, now }。 */
    get drag() {
      return live;
    },
    /** いま見せる値。 */
    get shown(): V {
      return live ? live.now : opts.value();
    },
    down,
    move,
    up,
    cancel,
  };
}

/** フェーダー: 上下に動かす。部品の高さぶんで目盛のいちばん下から上まで。いちばん下は −∞。 */
export function faderControl(strip: () => StripRef, db: () => number | null): MixControl<number | null> {
  return mixControl<number | null>({
    value: db,
    read: (start, _dx, dy, box) => fracToDb(Math.max(0, Math.min(1, dbToFrac(start) - dy / Math.max(1, box.height)))),
    make: (value, commit) => ({ type: "setGain", strip: strip(), db: value, commit }),
  });
}

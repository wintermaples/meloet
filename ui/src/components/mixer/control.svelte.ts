// フェーダー ・ パン ・ センドのつまみに共通のドラッグ。画面だけが先に動き（shown）、もとの値（from）は gray.7 の枠で描く。
// 押した所へは飛ばず、押した所からの動きだけで値を変える。Esc はキーの順のいちばん前でやめる（onDragCancel）。
// キー（仕様書 3 章 86-7）: Tab でフォーカスをあて、矢印で動かすたびに commit: false、キーを離して 1 秒たつか、フォーカスが外れたら
// commit: true を 1 つ（1 手）。Home ではじめの値、Enter で打ちこむ枠（typing）。

import { onMount } from "svelte";
import type { Command, StripRef } from "../../bridge";
import { send } from "../../state/app.svelte";
import { onDragCancel } from "../pianoroll/view.svelte";
import { KeySteps, MixPreview } from "./mixDrag";
import { dbToFrac, fracToDb, parseDb } from "./scale";

export interface MixControlOptions<V> {
  /** 曲の値（Rust の状態）。 */
  value: () => V;
  /** つかんだときの値 start から、dx ・ dy（CSS px）動かしたときの値。box はつかんだときの部品の箱。 */
  read: (start: V, dx: number, dy: number, box: DOMRect) => V;
  make: (value: V, commit: boolean) => Command;
  same?: (a: V, b: V) => boolean;
  /** 矢印キー 1 回ぶん動かした値。そのキーでは動かないなら undefined。 */
  step?: (value: V, key: string, shift: boolean) => V | undefined;
  /** Home の値（はじめの値）。 */
  home?: () => V;
  /** 打ちこんだ文字の値。読めなければ undefined。 */
  parse?: (text: string) => V | undefined;
}

export type MixControl<V> = ReturnType<typeof mixControl<V>>;

export function mixControl<V>(opts: MixControlOptions<V>) {
  const same = opts.same ?? ((a: V, b: V) => a === b);
  let press: { x: number; y: number; pointer: number; box: DOMRect; el: HTMLElement; preview: MixPreview<V> } | null = null;
  let live = $state<{ from: V; now: V } | null>(null);
  /** キーで動かして、まだ曲の値になっていない値。曲の値が変わったら消す。 */
  let keyed = $state<{ value: V } | null>(null);
  let typing = $state(false);
  const keys = new KeySteps<V>(opts.make, send);
  $effect(() => {
    opts.value();
    keyed = null;
  });

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

  onMount(() => {
    const off = onDragCancel(cancel);
    return () => {
      off();
      keys.flush();
    };
  });

  function keydown(e: KeyboardEvent): void {
    if (e.ctrlKey || e.metaKey || e.altKey || e.isComposing || press) return;
    const now = keyed ? keyed.value : opts.value();
    let next: V | undefined;
    if (e.key === "Enter" && !e.shiftKey && opts.parse) {
      e.preventDefault();
      e.stopPropagation();
      keys.flush();
      typing = true;
      return;
    }
    if (e.key === "Home" && !e.shiftKey) next = opts.home?.();
    else next = opts.step?.(now, e.key, e.shiftKey);
    if (next === undefined) return;
    // 先頭へ戻る（Home）や、ほかの帯の矢印に渡さない。
    e.preventDefault();
    e.stopPropagation();
    if (same(next, now)) return;
    keyed = { value: next };
    keys.step(next);
  }

  function keyup(): void {
    keys.release();
  }

  /** Opens the typing field, the same as Enter (a double click on the number). */
  function startTyping(): void {
    if (press || !opts.parse) return;
    keys.flush();
    typing = true;
  }

  function submit(text: string): boolean {
    const v = opts.parse?.(text);
    if (v === undefined) return false;
    typing = false;
    keyed = { value: v };
    send(opts.make(v, true));
    return true;
  }

  return {
    /** 動かしている間だけ { from, now }。 */
    get drag() {
      return live;
    },
    /** いま見せる値。 */
    get shown(): V {
      return live ? live.now : keyed ? keyed.value : opts.value();
    },
    /** 値を打ちこむ枠をひらいているか。 */
    get typing() {
      return typing;
    },
    stopTyping() {
      typing = false;
    },
    startTyping,
    down,
    move,
    up,
    cancel,
    keydown,
    keyup,
    /** フォーカスが外れた: キーで動かした分を、いま 1 手にする。 */
    blur: () => keys.flush(),
    submit,
  };
}

/** フェーダー: 上下に動かす。部品の高さぶんで目盛のいちばん下から上まで。いちばん下は −∞。 */
export function faderControl(strip: () => StripRef, db: () => number | null): MixControl<number | null> {
  return mixControl<number | null>({
    value: db,
    read: (start, _dx, dy, box) => fracToDb(Math.max(0, Math.min(1, dbToFrac(start) - dy / Math.max(1, box.height)))),
    make: (value, commit) => ({ type: "setGain", strip: strip(), db: value, commit }),
    step: (value, key, shift) => (key === "ArrowUp" || key === "ArrowDown" ? stepDb(value, key === "ArrowUp" ? 1 : -1, shift) : undefined),
    home: () => 0,
    parse: parseDb,
  });
}

/** フェーダー ・ センドの ↑ ↓: 1 dB、Shift で 0.1 dB。−∞ から上げると −60、−60 より下げると −∞、上は +6 まで。 */
export function stepDb(db: number | null, dir: 1 | -1, fine: boolean): number | null {
  const by = fine ? 0.1 : 1;
  if (db === null) return dir > 0 ? FLOOR : null;
  const next = Math.round((db + dir * by) * 10) / 10;
  if (next < FLOOR) return null;
  return Math.min(TOP, next);
}
const FLOOR = -60;
const TOP = 6;

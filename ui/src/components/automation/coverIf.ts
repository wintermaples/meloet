// オートメーションが毎コマ動かす物（つまみ、値の字）を、WebKit が小さく描きなおせる形にする道具
// （docs/architecture.md 7 章「CPU で描く WebKit」「レイアウトでためる Region」）。

import { glSlot } from "../../gl/slot";
import { gateTrack, showTrack } from "./autoState.svelte";

/** 背景の絵のない箱では、どちらも見た目は同じ。 */
const SAME = ["0% 0%", "100% 0%"] as const;

/**
 * 次のスタイルの計算で、WebKit に node まるごとの四角を 1 つ頼ませる（見た目は変えない。背景の絵のない箱に使う）。
 * 同じコマに node の中で動いたつまみや字の小さな四角は、その中に入る。小さな四角を何十もためると、WebKit は上の帯の数字まで入れた
 * 「ぜんぶを囲む四角」にまとめてしまうので、先に大きな四角を 1 つ頼んでおく（スタイルの分は、レイアウトの分より先にたまる）。
 */
export function askWhole(node: HTMLElement): void {
  node.style.backgroundPosition = node.style.backgroundPosition === SAME[0] ? SAME[1] : SAME[0];
}

/** on のあいだだけ、その箱を区画の canvas の下に入れ（glSlot の cover）、positions が変わるたびに箱まるごとの四角を頼めるようにする。 */
export function coverWhen(node: HTMLElement, on: boolean) {
  let slot: ReturnType<typeof glSlot> | null = null;
  function set(next: boolean): void {
    if (next === (slot !== null)) return;
    slot?.destroy();
    slot = next ? glSlot(node, { kind: "cover" }) : null;
    if (!next) node.style.backgroundPosition = "";
  }
  set(on);
  return {
    update: set,
    destroy() {
      set(false);
    },
  };
}

/**
 * track が数のあいだだけ、その箱を区画の canvas の下に入れる（動かない時に canvas を広げない）。
 * - 何も描かないが、WebGL の係がこの箱の上の canvas を描くとき（左右に分けて出す区画では、この箱のある半分の番のとき）に呼ばれるので、
 *   そこでそのトラックの位置を見せる（autoState の showTrack）。つまみと字は、上の canvas と同じコマで変わる。
 * - 位置を見せるたびに、箱まるごとの四角を頼む（askWhole）。頼むのは、箱いっぱいに置いた空の箱: node そのものに頼ませると、中の物の
 *   はみ出し（字の上下など）まで入った四角になり、canvas の外へ出る。node は position が static でない箱にする。
 *   空の箱を左右に出したいときは、node に CSS の変数 --ask-left ・ --ask-right（負の長さ）を決める。
 * track: この箱の中で帯が動かしている物のトラック。動かしていなければ null。
 */
export function coverIf(node: HTMLElement, track: number | null) {
  let on: { slot: ReturnType<typeof glSlot>; open: () => void; whole: HTMLElement } | null = null;
  let now: number | null = null;
  function set(next: number | null): void {
    if (next === now) return;
    now = next;
    on?.slot.destroy();
    on?.open();
    on?.whole.remove();
    on = null;
    if (next === null) return;
    const whole = document.createElement("span");
    whole.setAttribute("aria-hidden", "true");
    whole.dataset.test = "ask-whole";
    Object.assign(whole.style, { position: "absolute", inset: "0 var(--ask-right, 0) 0 var(--ask-left, 0)", pointerEvents: "none" });
    node.prepend(whole);
    on = { slot: glSlot(node, { kind: "paint", paint: () => showTrack(next) }), open: gateTrack(next, () => askWhole(whole)), whole };
  }
  set(track);
  return {
    update: set,
    destroy() {
      set(null);
    },
  };
}

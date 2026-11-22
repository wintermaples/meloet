// オートメーションが毎コマ動かす物（つまみ、値の字）を、WebKit が小さく描きなおせる形にする道具
// （docs/architecture.md 7 章「CPU で描く WebKit」「レイアウトでためる Region」）。

import { glSlot } from "../../gl/slot";
import { gateTrack, showTrack, watchLive, type StripPart } from "./autoState.svelte";

/** 背景の絵のない箱では、どちらも見た目は同じ。 */
const SAME = ["0% 0%", "100% 0%"] as const;

/**
 * 次のスタイルの計算で、WebKit に node まるごとの四角を 1 つ頼ませる（見た目は変えない。背景の絵のない箱に使う）。
 * 同じコマに node の中で動いたつまみや字の小さな四角は、その中に入る。小さな四角を何十もためると、WebKit は上の帯の数字まで入れた
 * 「ぜんぶを囲む四角」にまとめてしまうので、先に大きな四角を 1 つ頼んでおく（スタイルの分は、レイアウトの分より先にたまる）。
 */
export function askWhole(node: HTMLElement): void {
  // 同じコマに 2 回入れかえると、もとの値にもどって四角を頼まない。1 コマに 1 回だけ（コマの仕事が終わったら忘れる）。
  if (askedNow.has(node)) return;
  if (askedNow.size === 0) queueMicrotask(() => askedNow.clear());
  askedNow.add(node);
  node.style.backgroundPosition = node.style.backgroundPosition === SAME[0] ? SAME[1] : SAME[0];
}
const askedNow = new Set<HTMLElement>();

/** node いっぱいに置く空の箱（askWhole に渡す）。node は position が static でない箱。左右に出したいときは、node に CSS の変数
 * --ask-left ・ --ask-right、上に出したいときは --ask-top（負の長さ）を決める。 */
function wholeBox(node: HTMLElement): HTMLElement {
  const whole = document.createElement("span");
  whole.setAttribute("aria-hidden", "true");
  whole.dataset.test = "ask-whole";
  Object.assign(whole.style, { position: "absolute", inset: "var(--ask-top, 0) var(--ask-right, 0) 0 var(--ask-left, 0)", pointerEvents: "none" });
  node.prepend(whole);
  return whole;
}

/**
 * track が数のあいだだけ、その箱を区画の canvas の下に入れる（動かない時に canvas を広げない）。フェーダーと値の箱に使う。
 * - canvas の下に入れるのも、四角を頼むのも、node いっぱいに置く空の箱（同じ箱なので、頼む四角は canvas とぴったり重なる。node そのものに
 *   頼ませると、中の字のはみ出しまで入った四角になり、canvas の外へ出る）。
 * - 何も描かないが、WebGL の係がこの箱の上の canvas を描くとき（左右に分けて出す区画では、この箱のある半分の番のとき）に呼ばれるので、
 *   そこでそのトラックの位置を見せる（autoState の showTrack）。つまみと字は、上の canvas と同じコマで変わる。
 * - フェーダーか値の位置を見せるたびに、箱まるごとの四角を頼む（askWhole）。
 * track: 帯が何かを動かしているトラック。動かしていなければ null。
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
    const whole = wholeBox(node);
    on = { slot: glSlot(whole, { kind: "paint", paint: () => showTrack(next) }), open: gateTrack(next, "gain", () => askWhole(whole)), whole };
  }
  set(track);
  return {
    update: set,
    destroy() {
      set(null);
    },
  };
}

/**
 * canvas の下に入れない箱（パンの行、センドの行）: そのトラックのその組の位置を見せるたびに、箱まるごとの四角を頼む。
 * いつ見せるかは、同じトラックの coverIf の箱が決める。
 */
export function askIf(node: HTMLElement, what: { track: number; part: StripPart } | null) {
  let on: { open: () => void; whole: HTMLElement } | null = null;
  let now: string | null = null;
  function set(next: { track: number; part: StripPart } | null): void {
    const key = next ? `${next.track}/${next.part}` : null;
    if (key === now) return;
    now = key;
    on?.open();
    on?.whole.remove();
    on = null;
    if (!next) return;
    const whole = wholeBox(node);
    on = { open: gateTrack(next.track, next.part, () => askWhole(whole)), whole };
  }
  set(what);
  return {
    update: set,
    destroy() {
      set(null);
    },
  };
}

/**
 * canvas の下にある、毎コマ変わる値の字の箱（フェーダーの値）: いちばん新しい位置が変わるたびに、箱まるごとの四角を頼む。
 * その箱の上の canvas が出ないコマでも、となりのストリップの箱とつながった四角 1 つになる。
 */
export function askLive(node: HTMLElement, track: number | null) {
  let on: { close: () => void; whole: HTMLElement } | null = null;
  let now: number | null = null;
  function set(next: number | null): void {
    if (next === now) return;
    now = next;
    on?.close();
    on?.whole.remove();
    on = null;
    if (next === null) return;
    const whole = wholeBox(node);
    on = { close: watchLive(next, () => askWhole(whole)), whole };
  }
  set(track);
  return {
    update: set,
    destroy() {
      set(null);
    },
  };
}

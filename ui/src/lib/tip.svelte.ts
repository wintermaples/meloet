// 用語の説明（仕様書 2 章「用語の説明」・5 章）。画面に 1 つだけ。Tip.svelte が tipView を描く。
import type { Action } from "svelte/action";
import { findTerm } from "../text/ja";

/** マウスを止めてから出るまで（仕様書 2 章: 0.6 秒）。 */
export const TIP_DELAY_MS = 600;

export interface TipView {
  /** 用語。null なら body だけを出す（「…」でちぢめた名前の全部など）。 */
  term: string | null;
  body: string;
  left: number;
  top: number;
  bottom: number;
}

export const tipView = $state<{ value: TipView | null }>({ value: null });

let owner: HTMLElement | null = null;

/** いま用語の説明が出ているか（Esc の順番を決めるため）。 */
export function tipVisible(): boolean {
  return tipView.value !== null;
}

/** 出ている用語の説明を消す。 */
export function hideTip(): void {
  owner = null;
  tipView.value = null;
}

type Content = { term: string | null; body: string } | null;

function hover(node: HTMLElement, content: () => Content): { destroy(): void } {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const clear = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  };
  const show = () => {
    timer = undefined;
    const c = content();
    if (!c) return;
    const r = node.getBoundingClientRect();
    owner = node;
    tipView.value = { ...c, left: r.left, top: r.top, bottom: r.bottom };
  };
  // 動いているあいだは数えなおす。出たあとは、離れるまで出したまま。
  const arm = () => {
    if (owner === node && tipView.value) return;
    clear();
    timer = setTimeout(show, TIP_DELAY_MS);
  };
  const leave = () => {
    clear();
    if (owner === node) hideTip();
  };
  node.addEventListener("pointerenter", arm);
  node.addEventListener("pointermove", arm);
  node.addEventListener("pointerleave", leave);
  return {
    destroy() {
      leave();
      node.removeEventListener("pointerenter", arm);
      node.removeEventListener("pointermove", arm);
      node.removeEventListener("pointerleave", leave);
    },
  };
}

/** `use:term={"ループ"}`。説明は ja.ts の用語の説明から引く。 */
export const term: Action<HTMLElement, string> = (node, word) => {
  let current = word;
  const h = hover(node, () => {
    const found = findTerm(current);
    if (!found) {
      console.error(`用語の説明がない: ${current}`);
      return null;
    }
    return { term: found.term, body: found.description };
  });
  return {
    update(next: string) {
      current = next;
    },
    destroy: h.destroy,
  };
};

/** `use:tip={全部の名前}`。好きな文をそのまま出す。null か空なら出さない（ちぢめていないときなど）。 */
export const tip: Action<HTMLElement, string | null | undefined> = (node, text) => {
  let current = text;
  const h = hover(node, () => (current ? { term: null, body: current } : null));
  return {
    update(next: string | null | undefined) {
      current = next;
    },
    destroy: h.destroy,
  };
};

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

/**
 * `use:tipAround={文}`。tip と同じだが、中の用語（use:term）の説明が出ているときは出さない（ドラムの行の名前の、音名と用語の説明）。
 * 中の用語の説明が先に出るのは、中の要素の耳が先に呼ばれて、同じ時間で先に時間切れになるから。
 */
export const tipAround: Action<HTMLElement, string | null | undefined> = (node, text) => {
  let current = text;
  const h = hover(node, () => {
    if (!current || (owner && owner !== node && node.contains(owner) && tipView.value)) return null;
    return { term: null, body: current };
  });
  return {
    update(next: string | null | undefined) {
      current = next;
    },
    destroy: h.destroy,
  };
};

/** `use:tipOf={() => 文}`。乗った時に文を作る（null なら出さない）。 */
export const tipOf: Action<HTMLElement, (node: HTMLElement) => string | null> = (node, make) => {
  let current = make;
  const h = hover(node, () => {
    const body = current(node);
    return body ? { term: null, body } : null;
  });
  return {
    update(next: (node: HTMLElement) => string | null) {
      current = next;
    },
    destroy: h.destroy,
  };
};

/** `use:tipWhenCut`。「…」でちぢんでいるときだけ、要素の文を全部出す。 */
export const tipWhenCut: Action<HTMLElement> = (node) =>
  hover(node, () => (node.scrollWidth > node.clientWidth && node.textContent ? { term: null, body: node.textContent } : null));

/** The element whose cut text the page-wide watcher is showing. */
let cutOwner: HTMLElement | null = null;

const cutShown = (): boolean => cutOwner !== null && owner === cutOwner && tipView.value !== null;

/**
 * The nearest element, from `start` up, whose text is cut with "…", and its whole text:
 * a CSS cut (`text-overflow: ellipsis` with more text than room) or cutHead's `[data-cut]` (the "…" at the head).
 * Reads layout, so call it only once the pointer has rested.
 */
export function findCutText(start: Element | null): { el: HTMLElement; text: string } | null {
  for (let el = start; el && el !== document.body && el !== document.documentElement; el = el.parentElement) {
    if (!(el instanceof HTMLElement)) continue;
    if (el.hasAttribute("data-cut")) {
      const text = (el.querySelector("[data-cut-text]") ?? el).textContent?.trim();
      if (text) return { el, text };
    }
    if (getComputedStyle(el).textOverflow === "ellipsis" && el.scrollWidth > el.clientWidth) {
      const text = el.textContent?.trim();
      if (text) return { el, text };
    }
  }
  return null;
}

/**
 * One pointer listener for the whole page: rest on any text cut with "…" for TIP_DELAY_MS and its whole text shows,
 * in the same box as the term tips. A tip of the element's own (use:term, use:tip …) wins: its timer was set earlier
 * in the same pointer event, so it has already shown when this one fires. Nothing shows while a button is held.
 * Returns the function that takes the listener off.
 */
export function watchCutText(root: Document = document): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let target: Element | null = null;
  const clear = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  };
  const drop = () => {
    clear();
    if (cutShown()) hideTip();
    cutOwner = null;
  };
  const show = () => {
    timer = undefined;
    if (tipView.value !== null || !target?.isConnected) return;
    const found = findCutText(target);
    if (!found) return;
    const r = found.el.getBoundingClientRect();
    owner = cutOwner = found.el;
    tipView.value = { term: null, body: found.text, left: r.left, top: r.top, bottom: r.bottom };
  };
  const move = (e: PointerEvent) => {
    if (e.buttons !== 0) return drop();
    const at = e.target instanceof Element ? e.target : null;
    if (cutShown() && at && cutOwner!.contains(at)) return;
    drop();
    target = at;
    timer = setTimeout(show, TIP_DELAY_MS);
  };
  const out = (e: PointerEvent) => {
    if (e.relatedTarget === null) drop();
  };
  root.addEventListener("pointermove", move);
  root.addEventListener("pointerdown", drop);
  root.addEventListener("pointerout", out);
  return () => {
    drop();
    root.removeEventListener("pointermove", move);
    root.removeEventListener("pointerdown", drop);
    root.removeEventListener("pointerout", out);
  };
}

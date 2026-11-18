// ダイアログを、見出しの行（data-dialog-head の箱）を押したまま動かす。窓の外へは出さない。
// 動かした量はこの action の中だけに持つので、ひらきなおすと、もとの置き場（窓のまん中など）から始まる。
// ドラッグ中の Esc は、ほかのドラッグと同じく、動かす前の場所へ戻す（cancelDrag）。

import { onDragCancel } from "../components/pianoroll/view.svelte";

/** 見出しの行の中でも、押してもダイアログを動かさない物。 */
const NOT_HANDLE = "button, a, input, select, textarea, [role='button']";

export interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/**
 * 動かした量 (x, y) を、動かす前の箱 base が窓 (width × height) の中に入る量へ止める。
 * 箱が窓より大きいときは、左と上のはしを窓に入れる。
 */
export function clampOffset(base: Rect, x: number, y: number, width: number, height: number): { x: number; y: number } {
  return {
    x: Math.max(-base.left, Math.min(x, width - base.right)),
    y: Math.max(-base.top, Math.min(y, height - base.bottom)),
  };
}

export function movableDialog(node: HTMLElement) {
  let x = 0;
  let y = 0;
  let drag: { id: number; px: number; py: number; x: number; y: number; handle: HTMLElement } | null = null;

  /** WebGL の区画（音の設定のメーターなど）が画面の点にそろったままになるよう、点の境目に置く。 */
  function onDevicePixel(v: number): number {
    const dpr = window.devicePixelRatio || 1;
    return Math.round(v * dpr) / dpr;
  }

  function moveTo(nx: number, ny: number): void {
    const r = node.getBoundingClientRect();
    const base = { left: r.left - x, top: r.top - y, right: r.right - x, bottom: r.bottom - y };
    const c = clampOffset(base, onDevicePixel(nx), onDevicePixel(ny), window.innerWidth, window.innerHeight);
    x = c.x;
    y = c.y;
    node.style.transform = x === 0 && y === 0 ? "" : `translate(${x}px, ${y}px)`;
  }

  const refit = () => moveTo(x, y);

  function onDown(e: PointerEvent): void {
    const target = e.target as Element | null;
    const handle = target?.closest<HTMLElement>("[data-dialog-head]");
    if (e.button !== 0 || drag || !handle || !node.contains(handle) || target!.closest(NOT_HANDLE)) return;
    e.preventDefault();
    handle.setPointerCapture?.(e.pointerId);
    drag = { id: e.pointerId, px: e.clientX, py: e.clientY, x, y, handle };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
  }

  function onMove(e: PointerEvent): void {
    if (!drag || e.pointerId !== drag.id) return;
    moveTo(drag.x + e.clientX - drag.px, drag.y + e.clientY - drag.py);
  }

  function end(e?: PointerEvent): void {
    if (!drag || (e && e.pointerId !== drag.id)) return;
    if (drag.handle.hasPointerCapture?.(drag.id)) drag.handle.releasePointerCapture(drag.id);
    drag = null;
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", end);
    window.removeEventListener("pointercancel", end);
  }

  const stopCancel = onDragCancel(() => {
    if (!drag) return false;
    const back = drag;
    end();
    moveTo(back.x, back.y);
    return true;
  });

  // 中身の高さが変わったとき（くわしい設定をひらく）と、窓が小さくなったときに、窓の中へ戻す。
  const sizes = new ResizeObserver(refit);
  sizes.observe(node);
  node.addEventListener("pointerdown", onDown);
  window.addEventListener("resize", refit);

  return {
    destroy() {
      end();
      stopCancel();
      sizes.disconnect();
      node.removeEventListener("pointerdown", onDown);
      window.removeEventListener("resize", refit);
    },
  };
}

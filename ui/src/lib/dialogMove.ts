// Moves a dialog while its heading row (the data-dialog-head box) is held. It does not go out of the window.
// The amount moved lives only in this action, so opening it again starts from its usual place (the middle of the window, ...).
// Esc while dragging goes back to where it was, like the other drags (cancelDrag).

import { onDragCancel } from "../components/pianoroll/view.svelte";

/** Things in the heading row that do not move the dialog when pressed. */
const NOT_HANDLE = "button, a, input, select, textarea, [role='button']";

export interface Rect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/**
 * Limits the amount moved (x, y) so the box `base` (before moving) stays inside the window (width × height).
 * When the box is bigger than the window, its left and top ends stay inside.
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

  /** Placed on pixel edges, so WebGL regions (the meter in the sound settings, ...) stay aligned to screen pixels. */
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

  // Brought back inside the window when the content height changes (advanced settings open) and when the window gets smaller.
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

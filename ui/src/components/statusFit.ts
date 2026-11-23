// How the right side of the status line fits (spec 3 "When the width is not enough, the right side of the status line..."). Pure functions with no DOM.
// All widths are CSS px at the current scale.

export const STATUS_RIGHT_ORDER = ["inputs", "output", "buffer"] as const;
export type StatusRightPart = (typeof STATUS_RIGHT_ORDER)[number];
export type StatusRightShown = Record<StatusRightPart, boolean>;

export interface StatusFitInput {
  /** The usable width in the row (without padding). */
  available: number;
  /** The gap between parts. */
  gap: number;
  /** Width of the left parts (「■ 停止中」, the text, buttons) side by side, not cut (with the gaps between). */
  left: number;
  /** Widths of the right-side parts. Parts not shown are left out. */
  right: Partial<Record<StatusRightPart, number>>;
  /** Width of the scale button. Always shown. */
  scale: number;
}

/**
 * Which right-side parts to show. Hides input → output → buffer, in this order. The scale is never removed.
 * If it still does not fit, the screen cuts the end of the left text with 「…」.
 */
export function statusFit({ available, gap, left, right, scale }: StatusFitInput): StatusRightShown {
  const shown: StatusRightShown = { inputs: true, output: true, buffer: true };
  // Left, gap, growing space, gap, each right part and gap ..., scale.
  let need = left + 2 * gap + scale;
  for (const k of STATUS_RIGHT_ORDER) if (right[k] !== undefined) need += right[k] + gap;
  for (const k of STATUS_RIGHT_ORDER) {
    if (need <= available) break;
    if (right[k] === undefined) continue;
    shown[k] = false;
    need -= right[k] + gap;
  }
  return shown;
}

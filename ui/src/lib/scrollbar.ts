// Scroll bar math (spec 2 "Scroll bar"). Pure functions that do not know the DOM.

export interface ScrollBox {
  /** Height in view (clientHeight). */
  view: number;
  /** Height of the content (scrollHeight). */
  content: number;
  /** The current position (scrollTop). */
  top: number;
}

export interface Thumb {
  /** Handle length, and its position from the top of the bar (the bar is as long as view). */
  length: number;
  offset: number;
}

/** null when the content fits (no bar). Going over by less than 1px counts as fitting (fractions from the scale). */
export function thumbOf(box: ScrollBox, minLength: number): Thumb | null {
  const { view, content } = box;
  if (view <= 0 || content - view < 1) return null;
  const length = Math.min(view, Math.max(minLength, (view * view) / content));
  const range = content - view;
  const top = Math.min(Math.max(box.top, 0), range);
  return { length, offset: (top / range) * (view - length) };
}

/** The new scrollTop when the handle moves by dy. startTop is the scrollTop when it was taken. */
export function dragTo(box: ScrollBox, startTop: number, dy: number, minLength: number): number {
  const thumb = thumbOf(box, minLength);
  if (!thumb) return 0;
  const range = box.content - box.view;
  const track = box.view - thumb.length;
  if (track <= 0) return startTop;
  return Math.min(Math.max(startTop + (dy * range) / track, 0), range);
}

/** A press outside the handle: move one screen that way. y is from the top of the bar. */
export function pageTo(box: ScrollBox, y: number, minLength: number): number {
  const thumb = thumbOf(box, minLength);
  if (!thumb) return 0;
  const range = box.content - box.view;
  const next = y < thumb.offset ? box.top - box.view : box.top + box.view;
  return Math.min(Math.max(next, 0), range);
}

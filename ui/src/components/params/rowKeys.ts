// ← → on a parameter list (the focus is on the list, not on a row's control) move the chosen row's value
// (spec 4 "In the parameters: move the value / move it finely"). The row's own control does the step.

/** true when the key was taken: it does not go on to the keys of the rest of the app. */
export function arrowToChosenRow(ev: KeyboardEvent, list: HTMLElement | undefined, param: number | null): boolean {
  if (ev.key !== "ArrowLeft" && ev.key !== "ArrowRight") return false;
  if (ev.ctrlKey || ev.metaKey || ev.altKey || ev.isComposing || param === null || !list) return false;
  ev.preventDefault();
  ev.stopPropagation();
  const control = list.querySelector<HTMLElement>(`[data-param="${param}"] [role="slider"]`);
  control?.dispatchEvent(new KeyboardEvent("keydown", { key: ev.key, code: ev.code, shiftKey: ev.shiftKey, bubbles: true, cancelable: true }));
  return true;
}

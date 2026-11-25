// Markers and beat numbers on the piano roll ruler (spec 3, mock R11-Answers92). Does not know the DOM.

/**
 * The beat numbers to show: numbers covered by a marker name (laid over the numbers on a gray.3 background) are not shown. Widths are CSS px; those not measured yet are 0.
 * When it covers the number at the start of a bar, the name is shown, not the number.
 */
export function labelsBesideMarkers<L extends { tick: number; x: number }, M extends { id: number; x: number }>(
  labels: readonly L[],
  markers: readonly M[],
  labelW: Readonly<Record<number, number>>,
  markerW: Readonly<Record<number, number>>,
): L[] {
  return labels.filter((l) => !markers.some((m) => l.x < m.x + (markerW[m.id] ?? 0) && l.x + (labelW[l.tick] ?? 0) > m.x));
}

/** Drops the widths of beat numbers or markers no longer on the ruler, so the tables do not grow while scrolling. */
export function keepWidthsOf(widths: Record<number, number>, keys: readonly number[]): void {
  const keep = new Set(keys);
  for (const k of Object.keys(widths)) if (!keep.has(Number(k))) delete widths[Number(k)];
}

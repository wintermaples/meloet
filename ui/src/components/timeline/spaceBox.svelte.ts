// The box of 「この前に空きを入れる…」 (round 12 A2): which marker it is for belongs to the screen only; Enter sends
// one insertSpace. Opened from the marker's right-click menu; MarkerRow.svelte draws it at the marker.

export const spaceBox = $state({ marker: null as number | null });

export function openSpaceBox(marker: number): void {
  spaceBox.marker = marker;
}

export function closeSpaceBox(): void {
  spaceBox.marker = null;
}

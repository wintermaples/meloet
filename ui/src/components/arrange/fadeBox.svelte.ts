// The box of 「曲の終わりにフェードアウトをつける…」 (round 12 M5): whether it is open belongs to the screen only; Enter
// sends one addFadeOut. Opened from 「編集」; arrange/FadeBox.svelte draws it on the bar ruler at the song's end.

import type { TempoMap } from "../../time/tempoMap";

export const fadeBox = $state({ open: false });

export function openFadeBox(): void {
  fadeBox.open = true;
}

export function closeFadeBox(): void {
  fadeBox.open = false;
}

/** The song's end as the fade-out sees it (Rust `sections::song_end`): the latest clip end, up to the head of a bar; 0 without clips. */
export function songEndTick(tracks: readonly { clips: readonly { end: number }[] }[], map: TempoMap): number {
  const end = Math.max(0, ...tracks.flatMap((t) => t.clips.map((c) => c.end)));
  const at = map.meterAt(end);
  return at.barTick === end ? end : map.tickOfBar(at.bar + 1);
}

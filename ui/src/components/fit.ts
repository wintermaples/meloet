// What to hide when the width is not enough (spec 1 "Rules for the scale", the band tabs in spec 3).
// The transport uses measured widths; band tabs are estimated in px at 100 % and multiplied by the scale. Pure functions with no DOM.

import { line, space, text } from "../design/tokens";

/**
 * The order of hiding (spec 1 (1)). The keys at the right end of buttons (spec 4 "Keys are shown small at the right end of band tabs and buttons")
 * are hidden before that (docs/history/design-notes.ja.md B-73).
 */
const TRANSPORT_ORDER = ["keys", "songName", "load", "countIn", "positionLabels"] as const;
export type TransportPart = (typeof TRANSPORT_ORDER)[number];
export type TransportParts = Record<TransportPart, boolean>;

/**
 * Measured widths of the transport (CSS px). base is what is always shown (with the padding and gaps).
 * The others are the width a part adds when shown (with the gap before it).
 * For the count-in, pass the larger of the normal width and the width during the count-in (so nothing moves when it starts).
 */
export type TransportWidths = { base: number } & Record<TransportPart, number>;

/**
 * Width of what stays after hiding (spec 1: the song name becomes a ▾ of width 28, the count-in the metronome's ▾).
 * The added width, with the gap before it.
 */
export type TransportStubs = Partial<Record<TransportPart, number>>;

/** Hides in the order of order until it fits in available. widths is base and the added width of each part. stubs is the width of what stays after hiding. */
function foldParts<K extends string>(
  order: readonly K[],
  available: number,
  widths: { base: number } & Record<K, number>,
  stubs: Partial<Record<K, number>>,
): Record<K, boolean> {
  const shown = Object.fromEntries(order.map((k) => [k, true])) as Record<K, boolean>;
  let need = widths.base;
  for (const k of order) need += widths[k];
  for (const k of order) {
    if (need <= available) break;
    shown[k] = false;
    need -= widths[k] - (stubs[k] ?? 0);
  }
  return shown;
}

/** The set of parts to show. available is the usable width of the transport (CSS px). Hides in the spec's order until it fits. */
export function transportParts(available: number, widths: TransportWidths, stubs: TransportStubs = {}): TransportParts {
  return foldParts(TRANSPORT_ORDER, available, widths, stubs);
}

/**
 * The transport with 「重ねる」 (round 10, 7 A-27: it hides before 「ループ」, which never hides). It hides only when
 * nothing else is left to hide. overdub is the width it adds (with the gap before it).
 */
export function transportWithOverdub(
  available: number,
  widths: TransportWidths,
  stubs: TransportStubs,
  overdub: number,
): TransportParts & { overdub: boolean } {
  const parts = transportParts(available - overdub, widths, stubs);
  const need = TRANSPORT_ORDER.reduce((sum, k) => sum + (parts[k] ? widths[k] : (stubs[k] ?? 0)), widths.base);
  if (need + overdub <= available) return { ...parts, overdub: true };
  return { ...transportParts(available, widths, stubs), overdub: false };
}

/**
 * The transport with 「さっきのを残す」 too (round 12, R1): it is the first thing to hide, so that nothing that was
 * there before it gives way to it. keep is the width it adds (with the gap before it).
 */
export function transportWithKeep(
  available: number,
  widths: TransportWidths,
  stubs: TransportStubs,
  overdub: number,
  keep: number,
): TransportParts & { overdub: boolean; keepPlayed: boolean } {
  const rest = transportWithOverdub(available - keep, widths, stubs, overdub);
  if (rest.overdub && TRANSPORT_ORDER.every((k) => rest[k])) return { ...rest, keepPlayed: true };
  return { ...transportWithOverdub(available, widths, stubs, overdub), keepPlayed: false };
}

/**
 * The transport with the bars of the master's sound (round 12, E7): they come only into the room left when everything else is
 * as it would be without them, so they never push anything out. spectrum is the width they add (with the gap before them);
 * null while they are off.
 */
export function transportWithSpectrum(
  available: number,
  widths: TransportWidths,
  stubs: TransportStubs,
  overdub: number,
  keep: number,
  spectrum: number | null,
): TransportParts & { overdub: boolean; keepPlayed: boolean; spectrum: boolean } {
  const parts = transportWithKeep(available, widths, stubs, overdub, keep);
  if (spectrum === null) return { ...parts, spectrum: false };
  const shown = TRANSPORT_ORDER.reduce((sum, k) => sum + (parts[k] ? widths[k] : (stubs[k] ?? 0)), widths.base);
  const need = shown + (parts.overdub ? overdub : 0) + (parts.keepPlayed ? keep : 0) + spectrum;
  return { ...parts, spectrum: need <= available };
}

/** The order of hiding in the piano roll tool row: the tool keys, then the clip text (mock F-PR150). */
const TOOLBAR_ORDER = ["keys", "info"] as const;
export type ToolbarPart = (typeof TOOLBAR_ORDER)[number];
export type ToolbarParts = Record<ToolbarPart, boolean>;

/** The piano roll tool row. widths has the same shape as for the transport (base is what is always shown). */
export function toolbarParts(available: number, widths: { base: number } & Record<ToolbarPart, number>): ToolbarParts {
  return foldParts(TOOLBAR_ORDER, available, widths, {});
}

/**
 * The tool row with the label 「ほかのトラックも見る」 (round 12 P2): it hides first, so it never pushes out what the row showed before
 * (its ▾ stays, and the switch moves into the ▾'s list); then the keys and the clip text as before. ghost is the width the label adds
 * (with the gap before it).
 */
export function toolbarWithGhost(
  available: number,
  widths: { base: number } & Record<ToolbarPart, number>,
  ghost: number,
): ToolbarParts & { ghost: boolean } {
  const all = TOOLBAR_ORDER.reduce((sum, k) => sum + widths[k], widths.base);
  return { ...toolbarParts(available, widths), ghost: all + ghost <= available };
}

/** The advance of one IBM Plex Mono character (with the font size as 1). */
const MONO_ADVANCE = 0.6;

/** Estimated text width (text.s, 100 %). A Japanese character is the font size wide; others are one IBM Plex Mono character. */
export function textWidth(s: string): number {
  const em = text["text.s"].size[0];
  let w = 0;
  for (const ch of s) w += ch.charCodeAt(0) < 0x2000 ? em * MONO_ADVANCE : em;
  return w;
}

export interface BandTabWidth {
  /** Arrow, name, key, padding, divider. Always shown. */
  fixed: number;
  /** The count or current value (with the gap before it). */
  count: number;
}

/** Estimated band tab (px at 100 %). */
export function bandTabWidth(name: string, count: string, key: string): BandTabWidth {
  const pad = space["space.3"][0] * 2 + line["line.hair"];
  const gap = space["space.2"][0];
  return {
    fixed: pad + textWidth("▸") + gap + textWidth(name) + gap + textWidth(key),
    count: gap + textWidth(count),
  };
}

/**
 * Which band tab counts or current values to show. The bands on the right lose theirs first (spec 3).
 * width is the band tab row in CSS px; reserved is other width needed, such as the hint at the right end (px at 100 %).
 */
export function bandCountsShown(width: number, scale: number, tabs: readonly BandTabWidth[], reserved = 0): boolean[] {
  const room = (width * 100) / scale - reserved;
  const shown = tabs.map(() => true);
  let need = tabs.reduce((sum, tab) => sum + tab.fixed + tab.count, 0);
  for (let i = tabs.length - 1; i >= 0 && need > room; i--) {
    shown[i] = false;
    need -= tabs[i]!.count;
  }
  return shown;
}

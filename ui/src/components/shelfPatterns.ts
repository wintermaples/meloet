// What the shelf's 「パターン」 shows (round 12, P4): the headings 「基本」 and 「フィルイン」 and the patterns under them, and
// where 「ここに置く」 puts the chosen one. Pure functions that do not know the DOM.

import { PATTERN_GROUPS, type AppState, type PatternGroup, type PatternView } from "../bridge";
import { TempoMap } from "../time/tempoMap";
import { t, type TextKey } from "../text";
import { trackDrums } from "./pianoroll/drums";

export type PatternEntry =
  | { kind: "heading"; id: string; group: PatternGroup; label: string; folded: boolean }
  | { kind: "pattern"; id: string; pattern: PatternView; name: string };

export interface PatternModel {
  entries: PatternEntry[];
  /** The rows ↑ ↓ moves through (not the ones in a folded heading). */
  rows: PatternView[];
}

export const patternName = (id: string): string => t(`pattern.${id}` as TextKey);
const headingId = (group: PatternGroup) => `pt:${group}`;

/** The headings start open. `open`: the headings opened or folded (the shelf's look, `shelfUi.open`). */
export function patternModel(patterns: readonly PatternView[], open: Readonly<Record<string, boolean>>): PatternModel {
  const entries: PatternEntry[] = [];
  const rows: PatternView[] = [];
  for (const group of PATTERN_GROUPS) {
    const items = patterns.filter((p) => p.group === group);
    if (items.length === 0) continue;
    const id = headingId(group);
    const folded = !(open[id] ?? true);
    entries.push({ kind: "heading", id, group, label: t(`pattern.group.${group}`), folded });
    if (folded) continue;
    for (const p of items) {
      entries.push({ kind: "pattern", id: `pt:${p.id}`, pattern: p, name: patternName(p.id) });
      rows.push(p);
    }
  }
  return { entries, rows };
}

/**
 * The line beside 「ここに置く」: 「3 スタジオキット ・ 9 小節めに置きます」 on the selected track when it is a drum track, else the
 * studio kit track it makes. The bar is the one of the play line, as core places it.
 */
export function placeText(app: Pick<AppState, "song" | "timeline" | "transport">): string {
  const bar = new TempoMap(app.timeline).meterAt(app.transport.position).bar + 1;
  const track = app.song.tracks.find((x) => x.id === app.song.playTarget);
  if (track && track.kind.kind === "keys" && trackDrums(track))
    return t("shelf.placeOn", { track: t("track.label", { number: track.number, name: track.name }), bar });
  // The new track goes right below the selected one.
  const number = track ? track.number + 1 : app.song.tracks.length + 1;
  return t("shelf.placeOnNewTrack", { number, instrument: t("instrument.studioKit"), bar });
}

// What the shelf list shows (spec 2 "Instrument row (shelf)", "Plugin row in the shelf"; spec 3 (83, 85, 86-4, Y12)). Pure functions that do not know the DOM.
// Order: built-in → added later (an SF2 file heading (instruments only) → a plugin kind heading).
// Rows inside a folded heading are not made. Even with hundreds of rows, the screen draws only the visible part (Shelf.svelte).

import { EFFECT_KINDS, GM_FAMILIES, PLUGIN_EFFECT_CATEGORIES, PLUGIN_INSTRUMENT_CATEGORIES } from "../bridge/types";
import {
  INSTRUMENT_GROUPS,
  type EffectRef,
  type InstrumentKind,
  type InstrumentGroup,
  type LibraryView,
  type MissingView,
  type PluginRole,
  type PluginRowView,
  type ShelfView,
  type SoundRef,
} from "../bridge";
import { t, type TextKey } from "../text/ja";
import { rowHasKey, sameEffect, sameSound } from "./sounds";

/** "patterns": the drum patterns (P4), only in the shelf of a new track (shelfPatterns.ts). */
export type ShelfList = "instruments" | "effects" | "patterns";

/** The shelf look that only the screen keeps (shelfUi in tracks.svelte.ts). */
export interface ShelfLook {
  query: string;
  favoritesOnly: boolean;
  /** Headings opened or folded (only the ones changed from the first form). */
  open: Readonly<Record<string, boolean>>;
  /** Headings opened or folded while searching. Used only while the query is searchFor. */
  searchOpen: Readonly<Record<string, boolean>>;
  searchFor: string;
}

export interface ShelfRow {
  kind: "row";
  id: string;
  /** A row of the instrument list. */
  sound: SoundRef | null;
  /** A row of the effect list. */
  effect: EffectRef | null;
  name: string;
  /** Built in (the name is Japanese and may have a term tip). */
  builtin: boolean;
  /** The 「28」 or 「1 ・ バンク 8」 to the right of an SF2 preset. */
  number: string | null;
  plugin: PluginRowView | null;
  /** A row added later (space.6 on the left). */
  added: boolean;
  /**
   * A preset not fetched yet (spec 2 "「まだの音色」 row in the shelf", spec 3 (45-A)): being fetched now, or waiting.
   * No star, no ▶, no audition. It cannot be chosen (↑ ↓ skip it too).
   */
  notYet?: "fetching" | "waiting";
}

export interface ShelfHeading {
  kind: "heading";
  id: string;
  label: string;
  count: string;
  folded: boolean;
  /** 0: left space.3; 1: SF2 presets and the groups of the built-in ones (left mock.shelf.indent). */
  depth: 0 | 1;
  /** An SF2 file heading (format mark 「SF2」). */
  sf2: boolean;
}

/** The 「あとから入れたもの ・ 1」 row. */
export interface ShelfSection {
  kind: "section";
  id: "section:added";
  label: string;
}

export interface ShelfNotice {
  kind: "notice";
  id: string;
  /** null: the sound pack is not found. */
  missing: MissingView | null;
}

/** The box that opens under the chosen row (audition, loading, cannot load, put in the insert). */
export interface ShelfBox {
  kind: "box";
  id: "box";
  /** Under which row. null when the row is not visible (the bottom of the list). */
  row: string | null;
}

export type ShelfEntry = ShelfRow | ShelfHeading | ShelfSection | ShelfNotice | ShelfBox;

export interface ShelfModel {
  entries: ShelfEntry[];
  /** The visible rows (the rows ↑ ↓ moves through). */
  rows: ShelfRow[];
  /** The number of rows shown (including inside folded headings; after the query and favorites filter). */
  shown: number;
  /** The names of the open headings and their row counts (status line). */
  open: { label: string; count: number }[];
}

const soundId = (s: SoundRef) =>
  s.kind === "builtin" ? `b:${s.instrument}` : s.kind === "sf2" ? `s:${s.file}:${s.bank}:${s.program}` : `p:${s.key}`;
const pluginId = (r: PluginRowView) => `p:${r.name}\u0000${r.vendor}`;

/** The numbers of instruments and effects (「音源 182」 「エフェクト 512」 in the heading, the vertical heading). */
export function shelfCounts(shelf: ShelfView, library: LibraryView): { instruments: number; effects: number } {
  const plugins = (role: PluginRole) => library.plugins.filter((r) => r.kind.role === role).length;
  const sf2 = library.sf2.reduce((n, f) => n + f.presets.length, 0);
  const builtin = shelf.available.length + shelf.notYet.length;
  return { instruments: builtin + sf2 + plugins("instrument"), effects: EFFECT_KINDS.length + plugins("effect") };
}

/** The number of favorites (only the ones in the shelf). */
export function favoriteCount(list: ShelfList, shelf: ShelfView, library: LibraryView): number {
  if (list === "effects") {
    return shelf.favoriteEffects.filter((e) =>
      e.kind === "builtin" ? true : library.plugins.some((r) => r.kind.role === "effect" && rowHasKey(r, e.key)),
    ).length;
  }
  return shelf.favoriteSounds.filter((s) => {
    if (s.kind === "builtin") return shelf.available.includes(s.instrument);
    if (s.kind === "plugin") return library.plugins.some((r) => r.kind.role === "instrument" && rowHasKey(r, s.key));
    return library.sf2.some((f) => f.file === s.file && f.presets.some((p) => p.bank === s.bank && p.program === s.program));
  }).length;
}

/**
 * The first form of the headings (spec 3: Y12 "every heading starts folded"; 83 "only the file just added is open").
 * When nothing was added later, the built-in instrument groups are open.
 */
function openAtFirst(id: string, library: LibraryView): boolean {
  if (!id.includes(":")) return library.sf2.length === 0 && !library.plugins.some((r) => r.kind.role === "instrument");
  if (id.startsWith("f:")) return library.openedSf2 === id.slice(2);
  return false;
}

/** Whether the heading is open now. While searching, headings with matches are open. */
export function headingOpen(id: string, look: ShelfLook, library: LibraryView): boolean {
  if (look.query.trim() !== "") return look.searchFor === look.query ? (look.searchOpen[id] ?? true) : true;
  return look.open[id] ?? openAtFirst(id, library);
}

/** Whether the row is the chosen one (a plugin matches with the key of any format). */
export function rowIs(row: ShelfRow, sound: SoundRef | null | undefined, effect?: EffectRef | null): boolean {
  if (sound && row.sound) {
    if (row.plugin && sound.kind === "plugin") return rowHasKey(row.plugin, sound.key);
    return sameSound(row.sound, sound);
  }
  if (effect && row.effect) {
    if (row.plugin && effect.kind === "plugin") return rowHasKey(row.plugin, effect.key);
    return sameEffect(row.effect, effect);
  }
  return false;
}

/** The shelf list. fetching: the presets being fetched now (the 「取ってきています」 rows). */
export function shelfModel(
  list: ShelfList,
  shelf: ShelfView,
  library: LibraryView,
  look: ShelfLook,
  fetching: InstrumentKind | null = null,
): ShelfModel {
  const q = look.query.trim().toLowerCase();
  const searching = q !== "";
  const hit = (text: string) => text.toLowerCase().includes(q);
  const role: PluginRole = list === "instruments" ? "instrument" : "effect";
  const favSound = (s: SoundRef) =>
    shelf.favoriteSounds.some((f) => (f.kind === "plugin" && s.kind === "plugin" ? f.key === s.key : sameSound(f, s)));
  const favEffect = (e: EffectRef) => shelf.favoriteEffects.some((f) => sameEffect(f, e));
  const favPlugin = (r: PluginRowView) =>
    role === "instrument"
      ? shelf.favoriteSounds.some((f) => f.kind === "plugin" && rowHasKey(r, f.key))
      : shelf.favoriteEffects.some((f) => f.kind === "plugin" && rowHasKey(r, f.key));

  const entries: ShelfEntry[] = [];
  const rows: ShelfRow[] = [];
  const open: { label: string; count: number }[] = [];
  let shown = 0;

  const count = (n: number, total: number) => (searching ? t("shelf.matchCount", { shown: n, total }) : String(n));
  /** A heading and, if open, its rows. With no rows to show, the heading is not shown either. */
  const group = (h: Omit<ShelfHeading, "kind" | "folded" | "count">, items: ShelfRow[], total: number, leaf = true): void => {
    if (items.length === 0) return;
    const folded = !headingOpen(h.id, look, library);
    entries.push({ kind: "heading", ...h, count: count(items.length, total), folded });
    if (folded) return;
    if (leaf) open.push({ label: h.label, count: items.length });
    for (const r of items) {
      entries.push(r);
      if (!r.notYet) rows.push(r);
    }
  };

  // The notice of missing things (above the list).
  if (list === "instruments" && library.packMissing) entries.push({ kind: "notice", id: "notice:pack", missing: null });
  for (const m of shelf.missing) {
    if (m.kind === "sf2" ? list === "instruments" : m.role === role)
      entries.push({ kind: "notice", id: m.kind === "sf2" ? `notice:s:${m.file}` : `notice:p:${m.key}`, missing: m });
  }

  // ---- Built in ----
  const builtin: { heading: Omit<ShelfHeading, "kind" | "folded" | "count">; items: ShelfRow[]; total: number }[] = [];
  if (list === "instruments") {
    for (const { group: g, instruments } of INSTRUMENT_GROUPS) {
      const groupName = t(`instrumentGroup.${g as InstrumentGroup}`);
      const groupHit = !searching || hit(groupName);
      const all = instruments.filter((i) => shelf.available.includes(i) || shelf.notYet.includes(i));
      const items = all
        .filter((i) => {
          const waiting = !shelf.available.includes(i);
          return (!look.favoritesOnly || (!waiting && favSound({ kind: "builtin", instrument: i }))) && (groupHit || hit(t(`instrument.${i}`)));
        })
        .map((i): ShelfRow => {
          const sound: SoundRef = { kind: "builtin", instrument: i };
          const row: ShelfRow = { kind: "row", id: soundId(sound), sound, effect: null, name: t(`instrument.${i}`), builtin: true, number: null, plugin: null, added: false };
          if (!shelf.available.includes(i)) row.notYet = i === fetching ? "fetching" : "waiting";
          return row;
        });
      builtin.push({ heading: { id: g, label: groupName, depth: 0, sf2: false }, items, total: all.length });
    }
  } else {
    const items = EFFECT_KINDS.filter((e) => (!look.favoritesOnly || favEffect({ kind: "builtin", effect: e })) && (!searching || hit(t(`effect.${e}`)))).map(
      (e): ShelfRow => {
        const effect: EffectRef = { kind: "builtin", effect: e };
        return { kind: "row", id: `b:${e}`, sound: null, effect, name: t(`effect.${e}`), builtin: true, number: null, plugin: null, added: true };
      },
    );
    builtin.push({ heading: { id: "fx:builtin", label: t("shelf.builtin"), depth: 0, sf2: false }, items, total: EFFECT_KINDS.length });
  }

  const sf2 = list === "instruments" ? library.sf2 : [];
  const plugins = library.plugins.filter((r) => r.kind.role === role);
  const hasAdded = sf2.length > 0 || plugins.length > 0;
  const builtinShown = builtin.reduce((n, b) => n + b.items.length, 0);
  const builtinTotal = builtin.reduce((n, b) => n + b.total, 0);
  shown += builtinShown;

  if (!hasAdded) {
    // With nothing added later, the plain form (a list without headings).
    for (const b of builtin) {
      if (list === "instruments") group(b.heading, b.items, b.total);
      else
        for (const r of b.items) {
          const flat = { ...r, added: false };
          entries.push(flat);
          rows.push(flat);
        }
    }
  } else if (list === "instruments") {
    // 「最初から入っているもの」 is a heading that folds like the others. Its groups go one step in (spec 3 (89-11)).
    if (builtinShown > 0 || !searching) {
      const id = "builtin:sounds";
      const folded = !headingOpen(id, look, library);
      entries.push({ kind: "heading", id, label: t("shelf.builtin"), count: count(builtinShown, builtinTotal), folded, depth: 0, sf2: false });
      if (!folded) for (const b of builtin) group({ ...b.heading, depth: 1 }, b.items, b.total);
    }
  } else {
    const b = builtin[0]!;
    group(b.heading, b.items, b.total);
  }

  // ---- Added later ----
  let addedShown = 0;
  let addedTotal = 0;
  const addedEntries: ShelfEntry[] = [];
  const keep = entries.length;
  for (const file of sf2) {
    addedTotal += file.presets.length;
    const fileId = `f:${file.file}`;
    const families = GM_FAMILIES.map((family) => {
      const familyName = t(`gmFamily.${family}`);
      const all = file.presets.filter((p) => p.family === family);
      const items = all
        .filter((p) => {
          const sound: SoundRef = { kind: "sf2", file: file.file, bank: p.bank, program: p.program };
          return (!look.favoritesOnly || favSound(sound)) && (!searching || hit(familyName) || hit(p.name));
        })
        .map((p): ShelfRow => {
          const sound: SoundRef = { kind: "sf2", file: file.file, bank: p.bank, program: p.program };
          const number = String(p.program + 1) + (p.bank !== 0 ? ` ${t("shelf.bank", { bank: p.bank })}` : "");
          return { kind: "row", id: soundId(sound), sound, effect: null, name: p.name, builtin: false, number, plugin: null, added: true };
        });
      return { family, familyName, items, total: all.length };
    }).filter((f) => f.items.length > 0);
    const fileShown = families.reduce((n, f) => n + f.items.length, 0);
    addedShown += fileShown;
    if (fileShown === 0) continue;
    const folded = !headingOpen(fileId, look, library);
    entries.push({ kind: "heading", id: fileId, label: file.file, count: count(fileShown, file.presets.length), folded, depth: 0, sf2: true });
    if (folded) continue;
    for (const f of families) group({ id: `ff:${f.family}:${file.file}`, label: f.familyName, depth: 1, sf2: false }, f.items, f.total);
  }
  const categories = role === "instrument" ? PLUGIN_INSTRUMENT_CATEGORIES : PLUGIN_EFFECT_CATEGORIES;
  addedTotal += plugins.length;
  for (const category of categories) {
    const all = plugins.filter((r) => r.kind.category === category);
    if (all.length === 0) continue;
    const label = t(`pluginKind.${role}.${category}` as TextKey);
    const categoryHit = !searching || hit(label);
    const items = all
      .filter((r) => (!look.favoritesOnly || favPlugin(r)) && (categoryHit || hit(r.name) || hit(r.vendor)))
      .map((r): ShelfRow => {
        const ref = { kind: "plugin" as const, key: r.key };
        return {
          kind: "row",
          id: pluginId(r),
          sound: role === "instrument" ? ref : null,
          effect: role === "effect" ? ref : null,
          name: r.name,
          builtin: false,
          number: null,
          plugin: r,
          added: true,
        };
      });
    addedShown += items.length;
    group({ id: `c:${role}:${category}`, label, depth: 0, sf2: false }, items, all.length);
  }
  addedEntries.push(...entries.splice(keep));
  shown += addedShown;
  entries.push({ kind: "section", id: "section:added", label: t("shelf.sectionCount", { name: t("shelf.added"), count: count(addedShown, addedTotal) }) });
  entries.push(...addedEntries);

  return { entries, rows, shown, open };
}

/** Put the box under the chosen row. When the row is not in the list and insertAtEnd, at the bottom of the list. */
export function withBox(entries: readonly ShelfEntry[], row: ShelfRow | null, insertAtEnd: boolean): ShelfEntry[] {
  if (row) {
    const at = entries.findIndex((e) => e.kind === "row" && e.id === row.id);
    if (at >= 0) return [...entries.slice(0, at + 1), { kind: "box", id: "box", row: row.id }, ...entries.slice(at + 1)];
  }
  return insertAtEnd ? [...entries, { kind: "box", id: "box", row: null }] : [...entries];
}

/** The next row for ↑ ↓. Moves within the visible rows and stops at the ends. */
export function stepRow(rows: readonly ShelfRow[], at: number, delta: 1 | -1): ShelfRow | null {
  if (rows.length === 0) return null;
  if (at < 0) return delta > 0 ? rows[0]! : rows.at(-1)!;
  return rows[Math.max(0, Math.min(rows.length - 1, at + delta))]!;
}

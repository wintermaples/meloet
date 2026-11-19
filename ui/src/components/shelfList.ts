// 棚の一覧に出す物（仕様書 2 章「音源の行（棚）」「棚のプラグインの行」、3 章 83 ・ 85 ・ 86-4 ・ Y12）。DOM を知らない純粋な関数。
// 並びは 最初から入っているもの → あとから入れたもの（SF2 のファイルの見出し（音源だけ）→ プラグインの種類の見出し）。
// たたんだ見出しの中の行は作らない。何百行あっても、画面は見えている所だけを描く（Shelf.svelte）。

import { EFFECT_KINDS, GM_FAMILIES, PLUGIN_EFFECT_CATEGORIES, PLUGIN_INSTRUMENT_CATEGORIES } from "../bridge/types";
import {
  INSTRUMENT_GROUPS,
  type EffectRef,
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

export type ShelfList = "instruments" | "effects";

/** 画面だけが持つ棚のようす（tracks.svelte.ts の shelfUi）。 */
export interface ShelfLook {
  query: string;
  favoritesOnly: boolean;
  /** 見出しをひらいた ・ たたんだ（はじめの形からかえた物だけ）。 */
  open: Readonly<Record<string, boolean>>;
  /** さがしている間に、ひらいた ・ たたんだ見出し。searchFor の文字のときだけ使う。 */
  searchOpen: Readonly<Record<string, boolean>>;
  searchFor: string;
}

export interface ShelfRow {
  kind: "row";
  id: string;
  /** 音源の一覧の行。 */
  sound: SoundRef | null;
  /** エフェクトの一覧の行。 */
  effect: EffectRef | null;
  name: string;
  /** 最初から入っている物（名前は日本語。用語の説明がつくことがある）。 */
  builtin: boolean;
  /** SF2 の音色の右の「28」「1 ・ バンク 8」。 */
  number: string | null;
  plugin: PluginRowView | null;
  /** あとから入れた物の行（左を space.6 あける）。 */
  added: boolean;
}

export interface ShelfHeading {
  kind: "heading";
  id: string;
  label: string;
  count: string;
  folded: boolean;
  /** 0: 左 space.3、1: SF2 の仲間（左 mock.shelf.indent）。 */
  depth: 0 | 1;
  /** SF2 のファイルの見出し（形式の印「SF2」）。 */
  sf2: boolean;
}

/** 「最初から入っているもの ・ 31」「あとから入れたもの ・ 1」の行。 */
export interface ShelfSection {
  kind: "section";
  id: "section:builtin" | "section:added";
  label: string;
}

export interface ShelfNotice {
  kind: "notice";
  id: string;
  /** null: 音の素材が見つからない。 */
  missing: MissingView | null;
}

/** えらんだ行の下にひらく箱（ためし弾き ・ 読みこみ中 ・ 読みこめない ・ インサートに入れる）。 */
export interface ShelfBox {
  kind: "box";
  id: "box";
  /** どの行の下か。行が見えていないときは null（一覧のいちばん下）。 */
  row: string | null;
}

export type ShelfEntry = ShelfRow | ShelfHeading | ShelfSection | ShelfNotice | ShelfBox;

export interface ShelfModel {
  entries: ShelfEntry[];
  /** 見えている行（↑ ↓ で動く行）。 */
  rows: ShelfRow[];
  /** 出ている行の数（たたんだ見出しの中もふくむ。さがす文字とお気に入りでしぼった後）。 */
  shown: number;
  /** ひらいている見出しの名前と、その行の数（状態行）。 */
  open: { label: string; count: number }[];
}

const soundId = (s: SoundRef) =>
  s.kind === "builtin" ? `b:${s.instrument}` : s.kind === "sf2" ? `s:${s.file}:${s.bank}:${s.program}` : `p:${s.key}`;
const pluginId = (r: PluginRowView) => `p:${r.name}\u0000${r.vendor}`;

/** 音源とエフェクトの数（見出しの「音源 182」「エフェクト 512」、縦の見出し）。 */
export function shelfCounts(shelf: ShelfView, library: LibraryView): { instruments: number; effects: number } {
  const plugins = (role: PluginRole) => library.plugins.filter((r) => r.kind.role === role).length;
  const sf2 = library.sf2.reduce((n, f) => n + f.presets.length, 0);
  return { instruments: shelf.available.length + sf2 + plugins("instrument"), effects: EFFECT_KINDS.length + plugins("effect") };
}

/** お気に入りの数（棚に出る物だけ）。 */
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
 * 見出しのはじめの形（仕様書 3 章 Y12「見出しは、はじめはどれもたたむ」、83「ファイルの見出しは、いま足したファイルだけひらき」）。
 * あとから入れた音源がないときの最初から入っている音源のグループは、今までどおりひらく。
 */
function openAtFirst(id: string, library: LibraryView): boolean {
  if (!id.includes(":")) return library.sf2.length === 0 && !library.plugins.some((r) => r.kind.role === "instrument");
  if (id.startsWith("f:")) return library.openedSf2 === id.slice(2);
  return false;
}

/** いま見出しがひらいているか。さがしている間は、合う物のある見出しはひらく。 */
export function headingOpen(id: string, look: ShelfLook, library: LibraryView): boolean {
  if (look.query.trim() !== "") return look.searchFor === look.query ? (look.searchOpen[id] ?? true) : true;
  return look.open[id] ?? openAtFirst(id, library);
}

/** 行がえらばれている物か（プラグインはどの形式の鍵でも）。 */
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

/** 棚の一覧。 */
export function shelfModel(list: ShelfList, shelf: ShelfView, library: LibraryView, look: ShelfLook): ShelfModel {
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
  /** 見出しと、ひらいていればその行。出す行がなければ見出しごと出さない。 */
  const group = (h: Omit<ShelfHeading, "kind" | "folded" | "count">, items: ShelfRow[], total: number, leaf = true): void => {
    if (items.length === 0) return;
    const folded = !headingOpen(h.id, look, library);
    entries.push({ kind: "heading", ...h, count: count(items.length, total), folded });
    if (folded) return;
    if (leaf) open.push({ label: h.label, count: items.length });
    for (const r of items) {
      entries.push(r);
      rows.push(r);
    }
  };

  // 見つからない物の知らせ（一覧の上）。
  if (list === "instruments" && library.packMissing) entries.push({ kind: "notice", id: "notice:pack", missing: null });
  for (const m of shelf.missing) {
    if (m.kind === "sf2" ? list === "instruments" : m.role === role)
      entries.push({ kind: "notice", id: m.kind === "sf2" ? `notice:s:${m.file}` : `notice:p:${m.key}`, missing: m });
  }

  // ---- 最初から入っているもの ----
  const builtin: { heading: Omit<ShelfHeading, "kind" | "folded" | "count">; items: ShelfRow[]; total: number }[] = [];
  if (list === "instruments") {
    for (const { group: g, instruments } of INSTRUMENT_GROUPS) {
      const groupName = t(`instrumentGroup.${g as InstrumentGroup}`);
      const groupHit = !searching || hit(groupName);
      const all = instruments.filter((i) => shelf.available.includes(i));
      const items = all
        .filter((i) => (!look.favoritesOnly || favSound({ kind: "builtin", instrument: i })) && (groupHit || hit(t(`instrument.${i}`))))
        .map((i): ShelfRow => {
          const sound: SoundRef = { kind: "builtin", instrument: i };
          return { kind: "row", id: soundId(sound), sound, effect: null, name: t(`instrument.${i}`), builtin: true, number: null, plugin: null, added: false };
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
    // あとから入れた物がないときは、今までの形（見出しなしの一覧）。
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
    if (builtinShown > 0 || !searching) {
      entries.push({ kind: "section", id: "section:builtin", label: t("shelf.sectionCount", { name: t("shelf.builtin"), count: count(builtinShown, builtinTotal) }) });
      for (const b of builtin) group(b.heading, b.items, b.total);
    }
  } else {
    const b = builtin[0]!;
    group(b.heading, b.items, b.total);
  }

  // ---- あとから入れたもの ----
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

/** えらんだ行の下に箱を入れる。row が一覧にないときは、insertAtEnd なら一覧のいちばん下に。 */
export function withBox(entries: readonly ShelfEntry[], row: ShelfRow | null, insertAtEnd: boolean): ShelfEntry[] {
  if (row) {
    const at = entries.findIndex((e) => e.kind === "row" && e.id === row.id);
    if (at >= 0) return [...entries.slice(0, at + 1), { kind: "box", id: "box", row: row.id }, ...entries.slice(at + 1)];
  }
  return insertAtEnd ? [...entries, { kind: "box", id: "box", row: null }] : [...entries];
}

/** ↑ ↓ でえらぶ次の行。見えている行の中で動き、はしでは止まる。 */
export function stepRow(rows: readonly ShelfRow[], at: number, delta: 1 | -1): ShelfRow | null {
  if (rows.length === 0) return null;
  if (at < 0) return delta > 0 ? rows[0]! : rows.at(-1)!;
  return rows[Math.max(0, Math.min(rows.length - 1, at + delta))]!;
}

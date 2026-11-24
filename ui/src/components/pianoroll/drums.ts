// Drum instruments (spec 3 "Drum instruments (Y4)", "Drum piano roll", "SF2 drum kits"). Pure functions that do not know the DOM.
// The rows of the built-in kits are the 12 sounds of DRUM_ROWS (from the top). The rows of an SF2 kit are the keys that kit has sounds for (TrackView.drumKeys).
// The rows of a plugin instrument that names its keys (LV2 midnam) are those keys (TrackView.drumKeys, same order as an SF2 kit).
// Names are drum piece names (DRUM_ROWS names for built-in kits, the GM key number table for SF2, the plugin's own names for a plugin);
// keys not in the table use note names.

import { DRUM_ROWS, INSTRUMENT_GROUPS, type AppState, type ClipView, type InstrumentKind, type KeyCount, type PluginKeyName, type TrackView } from "../../bridge";
import { t, type TextKey } from "../../text";
import { noteName } from "../meterFormat";
import { gmDrumName } from "../sounds";

/** The keys of the built-in kit rows (from the top row). */
export const DRUM_KEYS: readonly number[] = DRUM_ROWS.map((r) => r.key);

/** A plugin instrument that names its keys: the names as the plugin gives them (TrackView.keyNames). */
export type PluginKit = { readonly kind: "plugin"; readonly names: readonly PluginKeyName[] };

/** Which kit's rows to show. false means not drums. */
export type Drums = false | "builtin" | "sf2" | PluginKit;

// One PluginKit for one names array, so a derived value that did not change stays the same object.
const pluginKits = new WeakMap<readonly PluginKeyName[], PluginKit>();
function pluginKit(names: readonly PluginKeyName[]): PluginKit {
  let kit = pluginKits.get(names);
  if (!kit) pluginKits.set(names, (kit = { kind: "plugin", names }));
  return kit;
}

/** Whether the rows are a plugin's named keys. */
export function isPluginKit(drums: Drums | boolean): drums is PluginKit {
  return typeof drums === "object";
}

const DRUM_INSTRUMENTS: readonly InstrumentKind[] = INSTRUMENT_GROUPS.find((g) => g.group === "drums")?.instruments ?? [];

/** Whether the instrument is in the drum group (「スタジオキット」「エレクトロキット」「ブラシキット」). */
export function isDrumKit(instrument: InstrumentKind | null | undefined): boolean {
  return instrument != null && DRUM_INSTRUMENTS.includes(instrument);
}

/** Whether the track's instrument is a drum kit, or a plugin instrument that names its keys. */
export function trackDrums(track: Pick<TrackView, "sound" | "keyNames"> | null | undefined): Drums {
  const s = track?.sound;
  if (!s) return false;
  if (s.kind === "builtin") return isDrumKit(s.instrument) ? "builtin" : false;
  if (s.kind === "plugin") return track.keyNames && track.keyNames.length > 0 ? pluginKit(track.keyNames) : false;
  return s.kind === "sf2" && s.family === "drumKit" ? "sf2" : false;
}

/** The keys of the drum rows (from the top row). While an SF2 kit is loading, the built-in kit rows. */
export function drumKeysOf(track: Pick<TrackView, "drumKeys"> | null | undefined, drums: Drums): readonly number[] {
  if (isPluginKit(drums)) return track?.drumKeys ?? drums.names.map((n) => n.key);
  return drums === "sf2" && track?.drumKeys ? track.drumKeys : DRUM_KEYS;
}

/**
 * Whether the piano roll shows drum rows: the instrument of the shown clip's track is a drum kit. Where there is no clip, decided by the instrument
 * where writing makes a clip (the keyboard's target keysTarget; usually the selected track).
 */
export function rollShowsDrums(app: Pick<AppState, "song">, clip: ClipView | null, track: TrackView | null): Drums {
  if (clip) return trackDrums(track);
  return trackDrums(app.song.tracks.find((x) => x.id === app.song.keysTarget));
}

/** The track of the clip the piano roll shows (or of the keyboard's target keysTarget if none). */
export function pianoRollTrack(app: Pick<AppState, "song" | "arrange">): TrackView | null {
  const id = app.arrange.pianoRollClip;
  if (id === null) return app.song.tracks.find((x) => x.id === app.song.keysTarget) ?? null;
  return app.song.tracks.find((x) => x.clips.some((c) => c.id === id)) ?? null;
}

/** Whether the instrument of the clip the piano roll shows (or of the selected track if none) is drums. Used from outside the part (the key handler). */
export function pianoRollDrums(app: Pick<AppState, "song" | "arrange">): Drums {
  return trackDrums(pianoRollTrack(app));
}

/** The drum piece name. Null for keys not in the kit. */
export function drumName(key: number, drums: Drums = "builtin"): string | null {
  if (isPluginKit(drums)) return drums.names.find((n) => n.key === key)?.name ?? null;
  if (drums === "sf2") return gmDrumName(key);
  const row = DRUM_ROWS.find((r) => r.key === key);
  return row ? t(`drum.${row.id}` as TextKey) : null;
}

/** The key of a typed drum piece name (spaces before and after are ignored). Null if it is not a name. */
export function drumKeyOf(text: string, drums: Drums = "builtin"): number | null {
  const name = text.normalize("NFKC").trim();
  if (isPluginKit(drums)) return drums.names.find((n) => n.name.normalize("NFKC") === name)?.key ?? null;
  if (drums === "sf2") {
    for (let k = 35; k <= 81; k++) if (gmDrumName(k)?.normalize("NFKC") === name) return k;
    return null;
  }
  return DRUM_ROWS.find((r) => t(`drum.${r.id}` as TextKey) === name)?.key ?? null;
}

/** The sound name: for drums, the piece name (note name for keys not in the kit); otherwise the note name. */
export function keyLabel(key: number, drums: Drums | boolean): string {
  const kind: Drums = drums === true ? "builtin" : drums;
  return (kind ? drumName(key, kind) : null) ?? noteName(key);
}

/** The held keys, lowest first: 「キック スネア」「C4 E4 G4」. */
export function keysLabel(keys: Iterable<number>, drums: Drums | boolean): string {
  return [...keys]
    .sort((a, b) => a - b)
    .map((k) => keyLabel(k, drums))
    .join(" ");
}

/** Words in drum piece names that have a term tip (spec 5). Longer ones are checked first (「サイドスティック」). */
const DRUM_TERMS = ["サイドスティック", "ハイハット", "クラッシュ", "キック", "スネア", "クラップ", "タム", "ライド"] as const;

export function drumTerm(name: string): string | null {
  return DRUM_TERMS.find((word) => name.includes(word)) ?? null;
}

/**
 * Whether the keys sound drums: the shelf audition (unless recording), the track being recorded, then the selected track (the same as 「弾くと鳴る先」 in the input meter).
 * The SF2 sound of an audition is decided by its SF2 family on the shelf.
 */
export function keysDrums(app: Pick<AppState, "shelf" | "transport" | "song" | "library">): Drums {
  const rec = app.transport.recording;
  const picked = app.shelf.selectedSound;
  if (picked && !rec) {
    if (picked.kind === "builtin") return isDrumKit(picked.instrument) ? "builtin" : false;
    if (picked.kind === "plugin") return false;
    const preset = app.library.sf2.find((f) => f.file === picked.file)?.presets.find((p) => p.bank === picked.bank && p.program === picked.program);
    return preset?.family === "drumKit" ? "sf2" : false;
  }
  const id = rec ? rec.track : app.song.keysTarget;
  return trackDrums(app.song.tracks.find((x) => x.id === id));
}

/** 「キック ×4 スネア ×2」「C4 ×2 E4」 with the same names grouped (spec 3 (85); from the lowest). */
export function keyCountsText(names: readonly KeyCount[], drums: Drums | boolean): string {
  return names
    .map((c) => (c.count > 1 ? t("note.keyCount", { name: keyLabel(c.key, drums), count: c.count }) : keyLabel(c.key, drums)))
    .join(" ");
}

/** 「キック ×4 スネア ×2」 from the keys of the selected notes. If there are 5 or more names, 「…」 after 4 (「音」 in the right panel). */
export function selectedKeysText(keys: Iterable<number>, drums: Drums | boolean): string {
  const names = countKeys(keys);
  const text = keyCountsText(names.slice(0, 4), drums);
  return names.length > 4 ? `${text} …` : text;
}

/** Every name that selectedKeysText left out behind its "…", for the tip; null when nothing was left out. */
export function selectedKeysFull(keys: Iterable<number>, drums: Drums | boolean): string | null {
  const names = countKeys(keys);
  return names.length > 4 ? keyCountsText(names, drums) : null;
}

function countKeys(keys: Iterable<number>): KeyCount[] {
  const counts = new Map<number, number>();
  for (const k of keys) counts.set(k, (counts.get(k) ?? 0) + 1);
  return [...counts].sort((a, b) => a[0] - b[0]).map(([key, count]) => ({ key, count }));
}

// ドラムの音源（仕様書 3 章「ドラムの音源（Y4）」「ドラムのピアノロール」「SF2 のドラムのキット」）。DOM を知らない純粋な関数。
// 最初から入っているキットの行は DRUM_ROWS の 12 の音（上から）。SF2 のキットの行は、そのキットが音を持つ鍵（TrackView.drumKeys）。
// 名前は楽器の名前（最初から入っているキットは DRUM_ROWS の名前、SF2 は GM の鍵の番号の表）、表にない鍵は音名。

import { DRUM_ROWS, INSTRUMENT_GROUPS, type AppState, type ClipView, type InstrumentKind, type KeyCount, type TrackView } from "../../bridge";
import { t, type TextKey } from "../../text/ja";
import { noteName } from "../meterFormat";
import { gmDrumName } from "../sounds";

/** 最初から入っているキットの行の鍵（上の行から）。 */
export const DRUM_KEYS: readonly number[] = DRUM_ROWS.map((r) => r.key);

/** どのキットの行を見せるか。false はドラムではない。 */
export type Drums = false | "builtin" | "sf2";

const DRUM_INSTRUMENTS: readonly InstrumentKind[] = INSTRUMENT_GROUPS.find((g) => g.group === "drums")?.instruments ?? [];

/** ドラムのグループの音源か（スタジオキット、エレクトロキット、ブラシキット）。 */
export function isDrumKit(instrument: InstrumentKind | null | undefined): boolean {
  return instrument != null && DRUM_INSTRUMENTS.includes(instrument);
}

/** トラックの音源がドラムのキットか。 */
export function trackDrums(track: Pick<TrackView, "sound"> | null | undefined): Drums {
  const s = track?.sound;
  if (!s) return false;
  if (s.kind === "builtin") return isDrumKit(s.instrument) ? "builtin" : false;
  return s.kind === "sf2" && s.family === "drumKit" ? "sf2" : false;
}

/** ドラムの行の鍵（上の行から）。SF2 のキットを読んでいる間は、最初から入っているキットの行。 */
export function drumKeysOf(track: Pick<TrackView, "drumKeys"> | null | undefined, drums: Drums): readonly number[] {
  return drums === "sf2" && track?.drumKeys ? track.drumKeys : DRUM_KEYS;
}

/**
 * ピアノロールがドラムの行を見せるか: 見せているクリップのトラックの音源がドラムのキット。クリップのない所では、書くと
 * クリップができる先（選んだトラック）の音源で決める。
 */
export function rollShowsDrums(app: Pick<AppState, "song">, clip: ClipView | null, track: TrackView | null): Drums {
  if (clip) return trackDrums(track);
  return trackDrums(app.song.tracks.find((x) => x.id === app.song.playTarget));
}

/** ピアノロールが見せているクリップ（なければ選んだトラック）のトラック。 */
export function pianoRollTrack(app: Pick<AppState, "song" | "arrange">): TrackView | null {
  const id = app.arrange.pianoRollClip;
  if (id === null) return app.song.tracks.find((x) => x.id === app.song.playTarget) ?? null;
  return app.song.tracks.find((x) => x.clips.some((c) => c.id === id)) ?? null;
}

/** ピアノロールが見せているクリップ（なければ選んだトラック）の音源がドラムか。部品の外（キーの係）から使う。 */
export function pianoRollDrums(app: Pick<AppState, "song" | "arrange">): Drums {
  return trackDrums(pianoRollTrack(app));
}

/** 楽器の名前。キットにない鍵は null。 */
export function drumName(key: number, drums: Drums = "builtin"): string | null {
  if (drums === "sf2") return gmDrumName(key);
  const row = DRUM_ROWS.find((r) => r.key === key);
  return row ? t(`drum.${row.id}` as TextKey) : null;
}

/** 打ちこんだ楽器の名前（前後の空白は見ない）の鍵。名前でなければ null。 */
export function drumKeyOf(text: string, drums: Drums = "builtin"): number | null {
  const name = text.normalize("NFKC").trim();
  if (drums === "sf2") {
    for (let k = 35; k <= 81; k++) if (gmDrumName(k)?.normalize("NFKC") === name) return k;
    return null;
  }
  return DRUM_ROWS.find((r) => t(`drum.${r.id}` as TextKey) === name)?.key ?? null;
}

/** 音の名前: ドラムなら楽器の名前（キットにない鍵は音名）、ほかは音名。 */
export function keyLabel(key: number, drums: Drums | boolean): string {
  const kind: Drums = drums === true ? "builtin" : drums;
  return (kind ? drumName(key, kind) : null) ?? noteName(key);
}

/** 押している鍵を低い順に「キック スネア」「C4 E4 G4」。 */
export function keysLabel(keys: Iterable<number>, drums: Drums | boolean): string {
  return [...keys]
    .sort((a, b) => a - b)
    .map((k) => keyLabel(k, drums))
    .join(" ");
}

/** 楽器の名前の中で、用語の説明（仕様書 5 章）のある言葉。長い物を先に見る（「サイドスティック」）。 */
const DRUM_TERMS = ["サイドスティック", "ハイハット", "クラッシュ", "キック", "スネア", "クラップ", "タム", "ライド"] as const;

export function drumTerm(name: string): string | null {
  return DRUM_TERMS.find((word) => name.includes(word)) ?? null;
}

/**
 * 鍵を弾くと鳴る先がドラムか: 棚のためし弾き（録音中でなければ）、録っているトラック、選んだトラックの順（入力計の「弾くと鳴る先」と同じ）。
 * ためし弾きの SF2 の音色は、棚の SF2 の仲間で決める。
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
  const id = rec ? rec.track : app.song.playTarget;
  return trackDrums(app.song.tracks.find((x) => x.id === id));
}

/** 同じ名前をまとめた「キック ×4 スネア ×2」「C4 ×2 E4」（仕様書 3 章 85。低い音から）。 */
export function keyCountsText(names: readonly KeyCount[], drums: Drums | boolean): string {
  return names
    .map((c) => (c.count > 1 ? t("note.keyCount", { name: keyLabel(c.key, drums), count: c.count }) : keyLabel(c.key, drums)))
    .join(" ");
}

/** 選んだノートの鍵から「キック ×4 スネア ×2」。名前が 5 つ以上なら、4 つのあとに「…」（右の欄の「音」）。 */
export function selectedKeysText(keys: Iterable<number>, drums: Drums | boolean): string {
  const counts = new Map<number, number>();
  for (const k of keys) counts.set(k, (counts.get(k) ?? 0) + 1);
  const names = [...counts].sort((a, b) => a[0] - b[0]).map(([key, count]) => ({ key, count }));
  const text = keyCountsText(names.slice(0, 4), drums);
  return names.length > 4 ? `${text} …` : text;
}

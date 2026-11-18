// ドラムの音源（仕様書 3 章「ドラムの音源（Y4）」「ドラムのピアノロール」）。DOM を知らない純粋な関数。
// 行は DRUM_ROWS の 12 の音だけ（上から）。名前は楽器の名前、キットにない鍵は音名。

import { DRUM_ROWS, INSTRUMENT_GROUPS, type AppState, type ClipView, type InstrumentKind, type TrackView } from "../../bridge";
import { t, type TextKey } from "../../text/ja";
import { noteName } from "../meterFormat";

/** 行の鍵（上の行から）。 */
export const DRUM_KEYS: readonly number[] = DRUM_ROWS.map((r) => r.key);

const DRUM_INSTRUMENTS: readonly InstrumentKind[] = INSTRUMENT_GROUPS.find((g) => g.group === "drums")?.instruments ?? [];

/** ドラムのグループの音源か（スタジオキット、エレクトロキット）。 */
export function isDrumKit(instrument: InstrumentKind | null | undefined): boolean {
  return instrument != null && DRUM_INSTRUMENTS.includes(instrument);
}

/**
 * ピアノロールがドラムの行を見せるか: 見せているクリップのトラックの音源がドラムのグループ。クリップのない所では、書くと
 * クリップができる先（選んだトラック）の音源で決める。
 */
export function rollShowsDrums(app: Pick<AppState, "song">, clip: ClipView | null, track: TrackView | null): boolean {
  if (clip) return isDrumKit(track?.instrument);
  return isDrumKit(app.song.tracks.find((x) => x.id === app.song.playTarget)?.instrument);
}

/** ピアノロールが見せているクリップ（なければ選んだトラック）の音源がドラムか。部品の外（キーの係）から使う。 */
export function pianoRollDrums(app: Pick<AppState, "song" | "arrange">): boolean {
  const id = app.arrange.pianoRollClip;
  const track = id === null ? null : (app.song.tracks.find((x) => x.clips.some((c) => c.id === id)) ?? null);
  const clip = track?.clips.find((c) => c.id === id) ?? null;
  return rollShowsDrums(app, clip, track);
}

/** 楽器の名前。キットにない鍵は null。 */
export function drumName(key: number): string | null {
  const row = DRUM_ROWS.find((r) => r.key === key);
  return row ? t(`drum.${row.id}` as TextKey) : null;
}

/** 打ちこんだ楽器の名前（前後の空白は見ない）の鍵。名前でなければ null。 */
export function drumKeyOf(text: string): number | null {
  const name = text.normalize("NFKC").trim();
  return DRUM_ROWS.find((r) => t(`drum.${r.id}` as TextKey) === name)?.key ?? null;
}

/** 音の名前: ドラムなら楽器の名前（キットにない鍵は音名）、ほかは音名。 */
export function keyLabel(key: number, drums: boolean): string {
  return (drums ? drumName(key) : null) ?? noteName(key);
}

/** 押している鍵を低い順に「キック スネア」「C4 E4 G4」。 */
export function keysLabel(keys: Iterable<number>, drums: boolean): string {
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

/** 鍵を弾くと鳴る先の音源: 棚のためし弾き（録音中でなければ）、録っているトラック、選んだトラックの順（入力計の「弾くと鳴る先」と同じ）。 */
export function keysInstrument(app: Pick<AppState, "shelf" | "transport" | "song">): InstrumentKind | null {
  const rec = app.transport.recording;
  if (app.shelf.selected !== null && !rec) return app.shelf.selected;
  const id = rec ? rec.track : app.song.playTarget;
  return app.song.tracks.find((x) => x.id === id)?.instrument ?? null;
}

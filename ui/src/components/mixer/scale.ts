// ミキサーの数の見せかたと、フェーダー ・ センドの位置（仕様書 2 章「メーターとフェーダーの目盛」）。DOM を知らない純粋な関数。

import type { SongView } from "../../bridge";
import { meterScale } from "../../design/tokens";
import { t } from "../../text/ja";
import { dbNumber } from "../format";

const MINUS = "−";
const TOP_DB = meterScale[0]![0];
const FLOOR_DB = meterScale[meterScale.length - 1]![0];

/** dB（null は −∞）→ 長さ 0..1（いちばん下が 0）。−60 より小さい音は 0、+6 より大きい音は 1。 */
export function dbToFrac(db: number | null): number {
  if (db === null || db < FLOOR_DB) return 0;
  if (db >= TOP_DB) return 1;
  for (let i = 1; i < meterScale.length; i++) {
    const [hiDb, hiLen] = meterScale[i - 1]!;
    const [loDb, loLen] = meterScale[i]!;
    if (db >= loDb) return (loLen + ((db - loDb) / (hiDb - loDb)) * (hiLen - loLen)) / 100;
  }
  return 0;
}

/** 長さ 0..1 → dB（0.1 きざみ）。いちばん下（0）は −∞（null）。 */
export function fracToDb(frac: number): number | null {
  if (frac <= 0) return null;
  if (frac >= 1) return TOP_DB;
  const len = frac * 100;
  for (let i = 1; i < meterScale.length; i++) {
    const [hiDb, hiLen] = meterScale[i - 1]!;
    const [loDb, loLen] = meterScale[i]!;
    if (len >= loLen) return Math.round((loDb + ((len - loLen) / (hiLen - loLen)) * (hiDb - loDb)) * 10) / 10;
  }
  return null;
}

/** 目盛の字（+6、0、−6、−12、−24、−60）と、その長さ 0..1。−3 は色の変わり目なので字にしない（モック F4-Mixer）。 */
export const SCALE_MARKS: readonly { label: string; frac: number }[] = meterScale
  .filter(([db]) => db !== -3)
  .map(([db, len]) => ({ label: db > 0 ? `+${db}` : String(db).replace("-", MINUS), frac: len / 100 }));

/** フェーダーの値の数字（「0.0」「−3.0」「−∞」）。単位の「dB」は横に別に出す（モック F4-Mixer）。 */
export function faderText(db: number | null): string {
  return db === null ? t("common.dbSilentBare") : t("common.dbBare", { value: dbNumber(db) });
}

/** センドの量（「−15.0」「−∞」）。 */
export const sendText = faderText;

/** パン（−100 〜 100）→「C」「L 20」「R 20」。 */
export function panText(pan: number): string {
  if (pan === 0) return t("mixer.panCenter");
  return pan < 0 ? t("mixer.panLeft", { value: -pan }) : t("mixer.panRight", { value: pan });
}

/** パンのつまみの針の向き（度）。いちばん左 −150、いちばん右 150（モック F4-Mixer の L 20 が −30°）。 */
export function panAngle(pan: number): number {
  return (pan / 100) * 150;
}

export const clampPan = (pan: number): number => Math.max(-100, Math.min(100, Math.round(pan)));

/** フェーダーのいちばん短い長さ（100 % の px）。仕様書 2 章の帯の表「516（道具 32 + ストリップ 484。フェーダーの長さ 120 をのこす）」。 */
export const FADER_MIN = 120;

/** マスターのパン（A-14）。 */
export function masterPan(song: SongView): number {
  // 約束の MasterView に pan が来たら、song.master.pan を読む形に直す。
  return (song.master as { pan?: number }).pan ?? 0;
}

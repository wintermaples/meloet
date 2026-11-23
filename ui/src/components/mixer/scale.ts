// How the mixer shows numbers, and fader / send positions (spec 2 "Scale marks of meters and faders"). Pure functions that do not know the DOM.

import { meterScale } from "../../design/tokens";
import { t } from "../../text/ja";
import { dbNumber } from "../format";

const MINUS = "−";
const TOP_DB = meterScale[0]![0];
const FLOOR_DB = meterScale[meterScale.length - 1]![0];

/** dB (null is −∞) → length 0..1 (bottom is 0). Quieter than −60 is 0, louder than +6 is 1. */
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

/** Length 0..1 → dB (steps of 0.1). The bottom (0) is −∞ (null). */
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

/** Scale mark text (+6, 0, −6, −12, −24, −60) and its length 0..1. −3 is where the color changes, so it gets no text (mock F4-Mixer). */
export const SCALE_MARKS: readonly { label: string; frac: number }[] = meterScale
  .filter(([db]) => db !== -3)
  .map(([db, len]) => ({ label: db > 0 ? `+${db}` : String(db).replace("-", MINUS), frac: len / 100 }));

/** The fader value number (「0.0」「−3.0」「−∞」). The unit 「dB」 is shown on its own beside it (mock F4-Mixer). */
export function faderText(db: number | null): string {
  return db === null ? t("common.dbSilentBare") : t("common.dbBare", { value: dbNumber(db) });
}

/** Send amount (「−15.0」「−∞」). */
export const sendText = faderText;

/** Pan (−100 to 100) → 「C」「L 20」「R 20」. */
export function panText(pan: number): string {
  if (pan === 0) return t("mixer.panCenter");
  return pan < 0 ? t("mixer.panLeft", { value: -pan }) : t("mixer.panRight", { value: pan });
}

/** Pan knob needle angle (degrees). Far left −150, far right 150 (L 20 is −30° in mock F4-Mixer). */
export function panAngle(pan: number): number {
  return (pan / 100) * 150;
}

export const clampPan = (pan: number): number => Math.max(-100, Math.min(100, Math.round(pan)));

/** Typed dB (「-3」「−3.0」「+2 dB」「−∞」). Out-of-range values are brought into range by Rust. undefined if it cannot be read. */
export function parseDb(text: string): number | null | undefined {
  const s = text.trim().replace(/[−ー]/g, "-").replace(/\s*dB$/i, "");
  if (/^-?(∞|inf)$/i.test(s)) return null;
  if (!/^[+-]?(\d+(\.\d*)?|\.\d+)$/.test(s)) return undefined;
  return Math.round(Number(s) * 100) / 100;
}

/** Typed pan (「C」「L 20」「R20」「-20」「20」). undefined if it cannot be read. */
export function parsePan(text: string): number | undefined {
  const s = text.trim().replace(/[−ー]/g, "-").toUpperCase();
  if (s === t("mixer.panCenter").toUpperCase() || s === "0") return 0;
  const m = s.match(/^([LR])\s*(\d+)$/);
  if (m) return clampPan(Number(m[2]) * (m[1] === "L" ? -1 : 1));
  if (!/^[+-]?\d+$/.test(s)) return undefined;
  return clampPan(Number(s));
}

/** The shortest fader length (px at 100 %). The band table in spec 3: 「516 (toolbar 32 + strips 484; keeps a fader length of 120)」. */
export const FADER_MIN = 120;

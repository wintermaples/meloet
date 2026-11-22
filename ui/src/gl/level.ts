import { DB_FLOOR, DB_SCALE, FALL_DB_PER_S, MIC_MID_DB } from "./dims";

/** 1.0 = 0 dBFS. Silence becomes -Infinity. */
export function peakToDb(peak: number): number {
  return peak > 0 ? 20 * Math.log10(peak) : -Infinity;
}

/** dB → length 0..1 of a level bar or a fader: straight lines between the points of the spec's scale. */
export function dbToLength(db: number): number {
  const top = DB_SCALE[0]!;
  if (db >= top[0]) return top[1];
  for (let i = 1; i < DB_SCALE.length; i++) {
    const [d1, l1] = DB_SCALE[i - 1]!;
    const [d0, l0] = DB_SCALE[i]!;
    if (db >= d0) return l0 + ((db - d0) / (d1 - d0)) * (l1 - l0);
  }
  return 0;
}

/** dB → length 0..1 of the mic meter (spec 3, 38-B): DB_FLOOR..MIC_MID_DB is the left half, MIC_MID_DB..0 the right half. */
export function micDbToLength(db: number): number {
  if (!(db > DB_FLOOR)) return 0;
  if (db >= 0) return 1;
  if (db >= MIC_MID_DB) return 0.5 + ((db - MIC_MID_DB) / -MIC_MID_DB) * 0.5;
  return ((db - DB_FLOOR) / (MIC_MID_DB - DB_FLOOR)) * 0.5;
}

/** The dB at a length of the spec's scale (dbToLength backwards). */
export function lengthToDb(length: number): number {
  for (let i = 1; i < DB_SCALE.length; i++) {
    const [d1, l1] = DB_SCALE[i - 1]!;
    const [d0, l0] = DB_SCALE[i]!;
    if (length >= l0) return d0 + ((length - l0) / (l1 - l0)) * (d1 - d0);
  }
  return DB_FLOOR;
}

/** The level to show: jumps up at once, falls at a steady dB rate. */
export function fall(shownDb: number, targetDb: number, dtMs: number): number {
  if (targetDb >= shownDb) return targetDb;
  const next = Math.max(shownDb, DB_FLOOR) - (FALL_DB_PER_S * dtMs) / 1000;
  if (next <= targetDb) return targetDb;
  return next <= DB_FLOOR ? -Infinity : next;
}

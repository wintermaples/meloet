import { DB_FLOOR, DB_SCALE, FALL_DB_PER_S } from "./dims";

/** 1.0 = 0 dBFS. Silence becomes -Infinity. */
export function peakToDb(peak: number): number {
  return peak > 0 ? 20 * Math.log10(peak) : -Infinity;
}

/** dB → length 0..1 of a level bar: straight lines between the marks of the mixer scale. */
export function dbToLength(db: number): number {
  const top = DB_SCALE[0]!;
  if (db >= top[0]) return top[1];
  for (let i = 1; i < DB_SCALE.length; i++) {
    const [d1, l1] = DB_SCALE[i - 1]!;
    const [d0, l0] = DB_SCALE[i]!;
    if (db >= d0) return l0 + ((db - d0) / (d1 - d0)) * (l1 - l0);
  }
  const [dLast, lLast] = DB_SCALE[DB_SCALE.length - 1]!;
  if (db <= DB_FLOOR) return 0;
  return ((db - DB_FLOOR) / (dLast - DB_FLOOR)) * lLast;
}

/** The level to show: jumps up at once, falls at a steady dB rate. */
export function fall(shownDb: number, targetDb: number, dtMs: number): number {
  if (targetDb >= shownDb) return targetDb;
  const next = Math.max(shownDb, DB_FLOOR) - (FALL_DB_PER_S * dtMs) / 1000;
  if (next <= targetDb) return targetDb;
  return next <= DB_FLOOR ? -Infinity : next;
}

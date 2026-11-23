// Round 12 F1 for the fake: what 「公開先に合わせる」 does, as Rust's `match_gain` (crates/meloet-core/src/export/loudness.rs).
import { EXPORT_MATCH_LUFS, EXPORT_MATCH_QUIETEST_LUFS, EXPORT_MATCH_TRUE_PEAK_DB, type ExportCheck, type ExportDialogView, type ExportLoudnessResult } from "./types";

/** The gain to −14 LUFS, or less so that the true peak stays at −1 dBTP. null: not measured, or quieter than −70 LUFS. */
export function fakeMatch(lufs: number | null, truePeakDb: number | null): ExportLoudnessResult | null {
  if (lufs === null || lufs < EXPORT_MATCH_QUIETEST_LUFS) return null;
  const wanted = EXPORT_MATCH_LUFS - lufs;
  const room = truePeakDb === null ? Infinity : EXPORT_MATCH_TRUE_PEAK_DB - truePeakDb;
  const gainDb = Math.min(wanted, room);
  return { gainDb, limited: wanted > room, lufs: lufs + gainDb };
}

/** The loudness part of the dialog for a check, as Rust has it: not for 「トラックごと」, and the result once the check is done. */
export function fakeLoudnessOf(
  check: ExportCheck,
  range: ExportDialogView["range"],
  chosen: ExportDialogView["loudness"],
): Pick<ExportDialogView, "loudness" | "canMatch" | "loudnessResult"> {
  const done = check.state === "done";
  const result = done ? fakeMatch(check.lufs, check.truePeakDb) : null;
  const canMatch = range !== "tracks" && check.state !== "failed" && (!done || result !== null);
  return { loudness: canMatch ? chosen : "asIs", canMatch, loudnessResult: canMatch ? result : null };
}

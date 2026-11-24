// What the dialog 「書き出しました」 shows when an export is done (round 12 E9). Pure functions that do not know the DOM.
// Core holds whether it is open: it is open while `export.run` is `done` (not MIDI), and `closeExport` closes it.
import type { AppState, ExportRunView } from "../../bridge";
import { t } from "../../text";
import { dbNumber, SILENT_DB } from "../format";
import { bitsOf, peakNumber } from "./exportText";

export type ExportedDone = Extract<ExportRunView, { state: "done" }>;

/** The finished run whose dialog is open, or null. A MIDI export opens none: only the status line tells it (spec 3 "MIDI export"). */
export const exportedOpen = (state: AppState): ExportedDone | null =>
  state.export.run?.state === "done" && state.export.run.midiTracks === null ? state.export.run : null;

/** One file for each track has no waveform (core leaves it empty), whatever the count. */
export const perTrack = (run: ExportedDone): boolean => run.wave.length === 0;

/** The name line: the file, or for 「トラックごと」 「5 つのファイル」. */
export const exportedName = (run: ExportedDone): string => (perTrack(run) ? t("exported.files", { count: run.files }) : run.file);

/** 「3 分 12 秒」; under a minute 「45 秒」. Seconds are cut off, as lengthText. */
export function exportedLength(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  if (s < 60) return t("exported.lengthSeconds", { seconds: s });
  return t("exported.length", { minutes: Math.floor(s / 60), seconds: s % 60 });
}

/**
 * 「3 分 12 秒 ・ 48000 Hz ・ 24 ビット」. The one place where the form of the file is said: an mp3 (round 12 F2) says its
 * bitrate here instead of the bits.
 */
export function exportedDetails(run: ExportedDone): string {
  const length = exportedLength(run.lengthS);
  if (run.mp3Kbps !== null) return t("exported.detailsMp3", { length, rate: run.sampleRate, kbps: run.mp3Kbps });
  return t("exported.details", { length, rate: run.sampleRate, bits: bitsOf(run.bits) });
}

/** 「大きさ −14.0 LUFS ・ トゥルーピーク −1.0 dBTP」. null for 「トラックごと」 and when nothing was measured. */
export function exportedLoudness(run: ExportedDone): string | null {
  if (perTrack(run) || (run.lufs === null && run.truePeakDb === null)) return null;
  const lufs = run.lufs === null || run.lufs < SILENT_DB ? t("common.dbSilentBare") : dbNumber(run.lufs);
  return t("exported.loudness", { lufs, peak: peakNumber(run.truePeakDb) });
}

/** The waveform as one SVG path in a box of `wave.length` × 255: a bar for each value, its middle on the middle line (as the mock). */
export function wavePath(wave: readonly number[]): string {
  return wave
    .map((v, i) => [i, Math.max(0, Math.min(255, v))] as const)
    .filter(([, v]) => v > 0)
    .map(([i, v]) => `M${i} ${(255 - v) / 2}h0.75v${v}h-0.75z`)
    .join("");
}

// Text of the export dialog and the progress in the status line (spec 3 34-B, spec 6; mock F7-Export, F7-Exporting, F-Export; docs/history/design-notes.ja.md B-94-69 to 75).
// Pure functions that do not know the DOM.
import { EXPORT_TAIL_MAX_S, RECORD_FORMATS, type ExportCheck, type ExportDialogView, type ExportFailure, type ExportRunView, type ExportSolo, type RecordFormat } from "../../bridge";
import { t, type TextKey } from "../../text/ja";
import { dbNumber, SILENT_DB, textParts, type TextPart } from "../format";
import { barBeatTick, positionText, type Bars } from "../time";

/** The peak number (「−0.6」, 「+1.2」 over 0 dB; spec 3 41-A). No sound (null) or too quiet gives 「−∞」 (the same text as the master meter; B-94-75). */
export function peakNumber(db: number | null): string {
  if (db === null || db < SILENT_DB) return t("common.dbSilentBare");
  const text = dbNumber(db);
  return db > 0 && text !== "0.0" ? `+${text}` : text;
}

/** The length 「3:02」 (spec 6 "Exported"). Seconds are cut off. */
export function lengthText(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** Time left: 「40 秒」 「2 分」. Seconds under 1 minute, otherwise minutes (rounded). 「—」 while unknown. */
export function leftText(seconds: number | null): string {
  if (seconds === null) return t("meter.playTargetNone");
  if (seconds < 59.5) return t("export.leftSeconds", { seconds: Math.max(1, Math.round(seconds)) });
  return t("export.leftMinutes", { minutes: Math.round(seconds / 60) });
}

export const bitsOf = (f: RecordFormat): number => RECORD_FORMATS.find((r) => r.format === f)?.bits ?? 24;

/** Text of the range buttons. 「ループ」 in a song without a loop is the text without numbers (cannot be chosen). */
export function rangeLabel(range: ExportDialogView["range"], d: ExportDialogView, bars: Bars): string {
  if (range === "song") return t("export.rangeSong");
  if (range === "tracks") return t("export.rangeTracks");
  if (!d.loopRange) return t("transport.loop");
  return t("export.rangeLoop", { from: barBeatTick(d.loopRange.start, bars).bar, to: barBeatTick(d.loopRange.end, bars).bar });
}

/** The line under the range (「曲ぜんぶ ・ 1. 1. 0000 〜 17. 1. 0000 と、音が消えるまで」). */
export function spanText(d: ExportDialogView, bars: Bars): string {
  if (d.range === "tracks") return t("export.spanTracks", { count: d.tracks });
  const span = { start: positionText(d.span.start, bars), end: positionText(d.span.end, bars) };
  const tail = d.tail === "untilSilent";
  if (d.range === "loop") return t(tail ? "export.spanLoop" : "export.spanLoopNoTail", span);
  return t(tail ? "export.spanSong" : "export.spanSongNoTail", span);
}

/** The right of the folded 「くわしく」 row (「48000 Hz ・ 24 ビット ・ しっぽは音が消えるまで（最大 10 秒）」, plus 「 ・ 実時間」 in real time). */
export function detailsSummary(d: ExportDialogView): string {
  const values = { rate: d.sampleRate, bits: bitsOf(d.bits), seconds: EXPORT_TAIL_MAX_S };
  const base = t(d.tail === "untilSilent" ? "export.detailsSummary" : "export.detailsSummaryNoTail", values);
  return d.realtime ? `${base} ${t("export.detailsRealtime")}` : base;
}

export const tailText = (tail: ExportDialogView["tail"]): string =>
  tail === "untilSilent" ? t("export.tailUntilSilent", { seconds: EXPORT_TAIL_MAX_S }) : t("export.tailOff");

/**
 * One line of 「書き出す前のたしかめ」. `counting` is 「たしかめています ・ 160 の棒 ・ 38 %」.
 * Line colors: `plain` is gray.8 with only the holes (numbers) in gray.10 monospace, `over` has the holes in rec, `rec` is the whole line in rec, `hint` is gray.8 and wraps.
 */
export type CheckLine = { kind: "counting"; percent: number } | { kind: "text"; parts: TextPart[]; tone: "plain" | "over" | "rec" | "hint" };

const textLine = (key: TextKey, values: Record<string, string | number>, tone: "plain" | "over" | "rec" | "hint" = "plain"): CheckLine => ({
  kind: "text",
  parts: textParts(key, values),
  tone,
});

/** The 2 lines about going over 0 dB (spec 3 41-A: a rec sentence and a gray.8 sentence). */
const overLines = (over: number, peakDb: number | null): CheckLine[] => [
  textLine("export.checkOver", { count: over, peak: peakNumber(peakDb) }, "rec"),
  textLine("export.checkOverHint", {}, "hint"),
];

/**
 * The peak line of 「書き出す前のたしかめ」 (spec 3 41-A, spec 6 "Before export"). The clipping sentence only when the current format would clip (`clips`).
 * The count of places over is `over` (how it is counted: docs/history/design-notes.ja.md B-95-2).
 */
export function checkLines(check: ExportCheck): CheckLine[] {
  switch (check.state) {
    case "waiting":
      return [textLine("export.checkPreparing", {})];
    case "counting": {
      const peak = textLine("export.checkPeakSoFar", { peak: t("export.peakDb", { peak: peakNumber(check.peakDb) }) }, check.clips ? "over" : "plain");
      return [{ kind: "counting", percent: check.percent }, peak, ...(check.clips ? overLines(check.over, check.peakDb) : [])];
    }
    case "done":
      if (!check.clips) return [textLine("msg.exportPeak", { peak: peakNumber(check.peakDb) })];
      return [
        { kind: "text", parts: [{ text: t("export.peak"), hole: null }, { text: t("export.peakDb", { peak: peakNumber(check.peakDb) }), hole: "peak" }], tone: "over" },
        ...overLines(check.over, check.peakDb),
      ];
    case "failed":
      return [
        check.reason.kind === "plugin" ? textLine("export.checkFailedPlugin", { plugin: check.reason.name }) : textLine("export.checkFailedWrite", {}),
      ];
  }
}

/** The solo sentence of the check lines (spec 3 42-C). null when no track is soloed. */
export function soloText(solo: ExportSolo | null): string | null {
  if (!solo) return null;
  const first = t("track.label", { number: solo.first.number, name: solo.first.name });
  return t("export.solo", { track: solo.others > 0 ? t("export.soloMore", { track: first, count: solo.others }) : first });
}

/** When it clips, 「このまま書き出す」 (spec 6 "Export would clip"). */
export const clips = (check: ExportCheck): boolean => (check.state === "done" || check.state === "counting") && check.clips;

export const failedKey = (reason: ExportFailure): TextKey => (reason.kind === "plugin" ? "msg.exportFailedPlugin" : "msg.exportFailedWrite");

/** The export state shown on the left of the status line (spec 2 "Export progress", spec 6). */
export type RunShown =
  | { kind: "running"; percent: number; rest: TextPart[] }
  | { kind: "message"; parts: TextPart[]; folder: boolean };

export function runShown(run: ExportRunView): RunShown {
  switch (run.state) {
    case "running": {
      const rest = run.realtimeForced
        ? textParts("msg.exportingRealtime", { length: lengthText(run.lengthS) })
        : run.realtime
          ? textParts("msg.exportingRealtimeChosen", { length: lengthText(run.lengthS) })
          : run.remainingS === null
            ? textParts("export.runningRestUnknown", { file: run.file })
            : textParts("export.runningRest", { file: run.file, left: leftText(run.remainingS) });
      return { kind: "running", percent: run.percent, rest };
    }
    case "done":
      return {
        kind: "message",
        parts:
          run.files > 1
            ? textParts("msg.exported", { path: run.place })
            : textParts("msg.exportedFile", { file: run.file, length: lengthText(run.lengthS), rate: run.sampleRate, bits: bitsOf(run.bits) }),
        folder: true,
      };
    case "cancelled":
      return { kind: "message", parts: textParts("msg.exportCanceled", {}), folder: false };
    case "failed":
      return {
        kind: "message",
        parts: textParts(failedKey(run.reason), run.reason.kind === "plugin" ? { plugin: run.reason.name } : {}),
        folder: false,
      };
  }
}

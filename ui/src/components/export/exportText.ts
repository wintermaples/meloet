// 書き出しのダイアログと状態行の進みの文（仕様書 3 章 34-B、6 章、モック F7-Export ・ F7-Exporting ・ F-Export、docs/history/design-notes.ja.md B-94-69〜75）。
// DOM を知らない純粋な関数。
import { EXPORT_TAIL_MAX_S, RECORD_FORMATS, type ExportCheck, type ExportDialogView, type ExportFailure, type ExportRunView, type ExportSolo, type RecordFormat } from "../../bridge";
import { t, type TextKey } from "../../text/ja";
import { dbNumber, SILENT_DB, textParts, type TextPart } from "../format";
import { barBeatTick, positionText, type Bars } from "../time";

/** いちばん大きい所の数（「−0.6」、0 dB をこえたら「+1.2」。仕様書 3 章 41-A）。音がない（null）か小さすぎれば「−∞」（マスターの計と同じ字。B-94-75）。 */
export function peakNumber(db: number | null): string {
  if (db === null || db < SILENT_DB) return t("common.dbSilentBare");
  const text = dbNumber(db);
  return db > 0 && text !== "0.0" ? `+${text}` : text;
}

/** 長さの「3:02」（仕様書 6 章「書き出した」）。秒は切りすて。 */
export function lengthText(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** 残りの時間の「40 秒」「2 分」。1 分より短ければ秒、ほかは分（四捨五入）。まだわからなければ「—」。 */
export function leftText(seconds: number | null): string {
  if (seconds === null) return t("meter.playTargetNone");
  if (seconds < 59.5) return t("export.leftSeconds", { seconds: Math.max(1, Math.round(seconds)) });
  return t("export.leftMinutes", { minutes: Math.round(seconds / 60) });
}

export const bitsOf = (f: RecordFormat): number => RECORD_FORMATS.find((r) => r.format === f)?.bits ?? 24;

/** 範囲のボタンの字。ループがない曲の「ループ」は、数のない字（えらべない）。 */
export function rangeLabel(range: ExportDialogView["range"], d: ExportDialogView, bars: Bars): string {
  if (range === "song") return t("export.rangeSong");
  if (range === "tracks") return t("export.rangeTracks");
  if (!d.loopRange) return t("transport.loop");
  return t("export.rangeLoop", { from: barBeatTick(d.loopRange.start, bars).bar, to: barBeatTick(d.loopRange.end, bars).bar });
}

/** 範囲の下の行（「曲ぜんぶ ・ 1. 1. 0000 〜 17. 1. 0000 と、音が消えるまで」）。 */
export function spanText(d: ExportDialogView, bars: Bars): string {
  if (d.range === "tracks") return t("export.spanTracks", { count: d.tracks });
  const span = { start: positionText(d.span.start, bars), end: positionText(d.span.end, bars) };
  const tail = d.tail === "untilSilent";
  if (d.range === "loop") return t(tail ? "export.spanLoop" : "export.spanLoopNoTail", span);
  return t(tail ? "export.spanSong" : "export.spanSongNoTail", span);
}

/** 「くわしく」をたたんだ行の右（「48000 Hz ・ 24 ビット ・ しっぽは音が消えるまで（最大 10 秒）」、実時間なら「 ・ 実時間」）。 */
export function detailsSummary(d: ExportDialogView): string {
  const values = { rate: d.sampleRate, bits: bitsOf(d.bits), seconds: EXPORT_TAIL_MAX_S };
  const base = t(d.tail === "untilSilent" ? "export.detailsSummary" : "export.detailsSummaryNoTail", values);
  return d.realtime ? `${base} ${t("export.detailsRealtime")}` : base;
}

export const tailText = (tail: ExportDialogView["tail"]): string =>
  tail === "untilSilent" ? t("export.tailUntilSilent", { seconds: EXPORT_TAIL_MAX_S }) : t("export.tailOff");

/**
 * 「書き出す前のたしかめ」の 1 行。`counting` は「たしかめています ・ 160 の棒 ・ 38 %」。
 * 字の行の色: `plain` は gray.8 で穴（数）だけ gray.10 の等幅、`over` は穴が rec、`rec` は行ぜんぶ rec、`hint` は gray.8 で折りかえす。
 */
export type CheckLine = { kind: "counting"; percent: number } | { kind: "text"; parts: TextPart[]; tone: "plain" | "over" | "rec" | "hint" };

const textLine = (key: TextKey, values: Record<string, string | number>, tone: "plain" | "over" | "rec" | "hint" = "plain"): CheckLine => ({
  kind: "text",
  parts: textParts(key, values),
  tone,
});

/** 0 dB をこえた所の 2 行（仕様書 3 章 41-A: rec の文と gray.8 の文）。 */
const overLines = (over: number, peakDb: number | null): CheckLine[] => [
  textLine("export.checkOver", { count: over, peak: peakNumber(peakDb) }, "rec"),
  textLine("export.checkOverHint", {}, "hint"),
];

/**
 * 「書き出す前のたしかめ」のいちばん大きい所の行（仕様書 3 章 41-A、6 章「書き出しの前」）。音割れの文は、いまの形で割れるとき（`clips`）だけ。
 * こえた所の数は `over`（数えかたは docs/history/design-notes.ja.md B-95-2）。
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

/** たしかめの行のソロの文（仕様書 3 章 42-C）。ソロのトラックがなければ null。 */
export function soloText(solo: ExportSolo | null): string | null {
  if (!solo) return null;
  const first = t("track.label", { number: solo.first.number, name: solo.first.name });
  return t("export.solo", { track: solo.others > 0 ? t("export.soloMore", { track: first, count: solo.others }) : first });
}

/** 音割れのときは「このまま書き出す」（仕様書 6 章「書き出しで音が割れる」）。 */
export const clips = (check: ExportCheck): boolean => (check.state === "done" || check.state === "counting") && check.clips;

export const failedKey = (reason: ExportFailure): TextKey => (reason.kind === "plugin" ? "msg.exportFailedPlugin" : "msg.exportFailedWrite");

/** 状態行の左に出す書き出しのようす（仕様書 2 章「書き出しの進み」、6 章）。 */
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

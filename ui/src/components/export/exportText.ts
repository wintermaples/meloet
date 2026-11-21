// 書き出しのダイアログと状態行の進みの文（仕様書 3 章 34-B、6 章、モック F7-Export ・ F7-Exporting ・ F-Export、design-notes B-94-69〜75）。
// DOM を知らない純粋な関数。
import { EXPORT_TAIL_MAX_S, RECORD_FORMATS, type ExportCheck, type ExportDialogView, type ExportFailure, type ExportRunView, type RecordFormat } from "../../bridge";
import { t, type TextKey } from "../../text/ja";
import { dbNumber, SILENT_DB, textParts, type TextPart } from "../format";
import { barBeatTick, positionText, type Bars } from "../time";

/** いちばん大きい所の数（「−0.6」）。音がない（null）か小さすぎれば「−∞」（マスターの計と同じ字。B-94-75）。 */
export function peakNumber(db: number | null): string {
  return db === null || db < SILENT_DB ? t("common.dbSilentBare") : dbNumber(db);
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

export interface CheckLine {
  parts: TextPart[];
  /** 長い文（音割れ）は折りかえす。 */
  wrap: boolean;
}

const CLIPPING: CheckLine = { parts: textParts("msg.exportClipping", {}), wrap: true };

/** 「書き出す前のたしかめ」のいちばん大きい所の行（B-94-71・75）。数えている途中で 0 dB をこえたら、音割れの文も足す。 */
export function checkLines(check: ExportCheck): CheckLine[] {
  switch (check.state) {
    case "waiting":
      return [{ parts: textParts("export.checkWaiting", {}), wrap: false }];
    case "counting": {
      const line = { parts: textParts("export.checkCounting", { percent: check.percent, peak: peakNumber(check.peakDb) }), wrap: false };
      return check.clips ? [line, CLIPPING] : [line];
    }
    case "done":
      return [check.clips ? CLIPPING : { parts: textParts("msg.exportPeak", { peak: peakNumber(check.peakDb) }), wrap: false }];
    case "failed":
      return [
        check.reason.kind === "plugin"
          ? { parts: textParts("export.checkFailedPlugin", { plugin: check.reason.name }), wrap: false }
          : { parts: textParts("export.checkFailedWrite", {}), wrap: false },
      ];
  }
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

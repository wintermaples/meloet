// Functions that do not know the DOM, used by the scan strip, the stopped-plugins dialog and plugin locations (spec 2, spec 3 Y13, spec 6).
import type { PluginFormat, ScanStopReason } from "../../bridge";
import { STATUS_JOIN, t, type TextKey } from "../../text";
import { textParts } from "../format";

export interface ScanStripText {
  /** 「スキャン中」 */
  label: string;
  /** 「37 / 112」 */
  count: string;
  /** 「調べ終えた分から、もう使えます」 */
  note: string;
  /** Fill of the progress bar (0–1). */
  done: number;
}

/** Splits 「スキャン中 37 / 112 ・ 調べ終えた分から、もう使えます」 of spec 6 into the 3 places of mock R9-Scan. */
export function scanStripText(done: number, total: number): ScanStripText {
  const parts = textParts("msg.scanning", { done, total });
  const first = parts.findIndex((p) => p.hole !== null);
  const last = parts.length - 1 - [...parts].reverse().findIndex((p) => p.hole !== null);
  const tail = parts
    .slice(last + 1)
    .map((p) => p.text)
    .join("");
  return {
    label: parts
      .slice(0, first)
      .map((p) => p.text)
      .join("")
      .trim(),
    count: parts
      .slice(first, last + 1)
      .map((p) => p.text)
      .join(""),
    note: tail.startsWith(STATUS_JOIN) ? tail.slice(STATUS_JOIN.length) : tail.trim(),
    done: total > 0 ? Math.min(1, Math.max(0, done / total)) : 0,
  };
}

const REASONS: Record<ScanStopReason, TextKey> = {
  crashed: "plugin.reason.crashed",
  timedOut: "plugin.reason.timedOut",
  broken: "plugin.reason.broken",
};

export const reasonText = (r: ScanStopReason): string => t(REASONS[r]);

export const formatText = (f: PluginFormat): string => t(`plugin.format.${f}`);

/**
 * When 「プラグインの場所…」 in the stopped-plugins dialog opens the sound settings, scroll the plugin locations into view.
 * Only the screen holds this (core does not need to know how far to scroll).
 */
export const placesReveal = { pending: false };

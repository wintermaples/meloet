// スキャンの帯、止まった物のダイアログ、プラグインの場所で使う、DOM を知らない関数（仕様書 2 章・3 章 Y13・6 章）。
import type { PluginFormat, ScanStopReason } from "../../bridge";
import { STATUS_JOIN, t, type TextKey } from "../../text/ja";
import { textParts } from "../format";

export interface ScanStripText {
  /** 「スキャン中」 */
  label: string;
  /** 「37 / 112」 */
  count: string;
  /** 「調べ終えた分から、もう使えます」 */
  note: string;
  /** 進みの棒のぬり（0〜1）。 */
  done: number;
}

/** 6 章の「スキャン中 37 / 112 ・ 調べ終えた分から、もう使えます」を、モック R9-Scan の 3 つの所に分ける。 */
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
 * 止まった物のダイアログの「プラグインの場所…」で音の設定をひらいたとき、プラグインの場所を見える所へ送る。
 * 画面だけが持つ（どこまでスクロールするかは core が知らなくてよい）。
 */
export const placesReveal = { pending: false };

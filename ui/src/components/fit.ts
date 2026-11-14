// 横が足りないときに何を引っこめるか（仕様書 1 章「拡大率の決まり」、3 章の帯札）。
// 幅は 100 % のときの px で見積もり、拡大率をかけてくらべる。DOM を知らない純粋な関数。

import { line, space, text } from "../design/tokens";

/**
 * 再生まわりの見積もり（100 % の px。後ろのすきまを含む）。
 * base は、いつも出す物（ボタン、位置と時間、テンポ、拍子、メトロノーム、マスター、左右の内がわ）。
 * 値は、この画面を 100 % で描いて測った幅に、少しのゆとりを足した物。
 */
export const TRANSPORT_WIDTHS = {
  base: 840,
  songName: 144,
  load: 72,
  countIn: 145,
  positionLabels: 17,
} as const;

/** 引っこめる順（仕様書 1 章 (1)）。 */
const TRANSPORT_ORDER = ["songName", "load", "countIn", "positionLabels"] as const;
export type TransportPart = (typeof TRANSPORT_ORDER)[number];
export type TransportParts = Record<TransportPart, boolean>;

/** 出す物の組。width は再生まわりの CSS px、scale は拡大率（%）。 */
export function transportParts(width: number, scale: number): TransportParts {
  const room = (width * 100) / scale;
  const shown: TransportParts = { songName: true, load: true, countIn: true, positionLabels: true };
  let need: number = TRANSPORT_WIDTHS.base;
  for (const k of TRANSPORT_ORDER) need += TRANSPORT_WIDTHS[k];
  for (const k of TRANSPORT_ORDER) {
    if (need <= room) break;
    shown[k] = false;
    need -= TRANSPORT_WIDTHS[k];
  }
  return shown;
}

/** IBM Plex Mono の 1 字の送り（文字の大きさを 1 として）。 */
const MONO_ADVANCE = 0.6;

/** 文字の幅の見積もり（text.s、100 %）。日本語は 1 字が文字の大きさ、ほかは IBM Plex Mono の 1 字。 */
export function textWidth(s: string): number {
  const em = text["text.s"].size[0];
  let w = 0;
  for (const ch of s) w += ch.charCodeAt(0) < 0x2000 ? em * MONO_ADVANCE : em;
  return w;
}

export interface BandTabWidth {
  /** 矢印、名前、キー、内がわ、区切り。いつも出す。 */
  fixed: number;
  /** 件数や現在値（前のすきまを含む）。 */
  count: number;
}

/** 帯札の見積もり（100 % の px）。 */
export function bandTabWidth(name: string, count: string, key: string): BandTabWidth {
  const pad = space["space.3"][0] * 2 + line["line.hair"];
  const gap = space["space.2"][0];
  return {
    fixed: pad + textWidth("▸") + gap + textWidth(name) + gap + textWidth(key),
    count: gap + textWidth(count),
  };
}

/**
 * 帯札の件数や現在値のうち、どれを出すか。右の帯から先に消す（仕様書 3 章）。
 * width は帯札の行の CSS px、reserved は右はしの案内など、ほかに要る幅（100 % の px）。
 */
export function bandCountsShown(width: number, scale: number, tabs: readonly BandTabWidth[], reserved = 0): boolean[] {
  const room = (width * 100) / scale - reserved;
  const shown = tabs.map(() => true);
  let need = tabs.reduce((sum, tab) => sum + tab.fixed + tab.count, 0);
  for (let i = tabs.length - 1; i >= 0 && need > room; i--) {
    shown[i] = false;
    need -= tabs[i]!.count;
  }
  return shown;
}

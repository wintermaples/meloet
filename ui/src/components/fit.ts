// 横が足りないときに何を引っこめるか（仕様書 1 章「拡大率の決まり」、3 章の帯札）。
// 再生まわりは測った幅で、帯札は 100 % のときの px で見積もって拡大率をかけてくらべる。DOM を知らない純粋な関数。

import { line, space, text } from "../design/tokens";

/** 引っこめる順（仕様書 1 章 (1)）。 */
const TRANSPORT_ORDER = ["songName", "load", "countIn", "positionLabels"] as const;
export type TransportPart = (typeof TRANSPORT_ORDER)[number];
export type TransportParts = Record<TransportPart, boolean>;

/**
 * 再生まわりの、測った幅（CSS px）。base は、いつも出す物（左右の内がわとすきまを含む）。
 * ほかは、その部品を出したときにふえる幅（前のすきまを含む）。
 * カウントインは、ふだんの幅とカウントイン中の幅の大きいほうを渡す（始まった瞬間に何も動かないように）。
 */
export type TransportWidths = { base: number } & Record<TransportPart, number>;

/**
 * 引っこめたあとに残す物の幅（仕様書 1 章: 曲の名前は幅 28 の ▾、カウントインはメトロノームの ▾）。
 * 前のすきまも含めた、ふえる幅。
 */
export type TransportStubs = Partial<Record<TransportPart, number>>;

/** 出す物の組。available は再生まわりの使える幅（CSS px）。入るまで、仕様書の順に引っこめる。 */
export function transportParts(available: number, widths: TransportWidths, stubs: TransportStubs = {}): TransportParts {
  const shown: TransportParts = { songName: true, load: true, countIn: true, positionLabels: true };
  let need = widths.base;
  for (const k of TRANSPORT_ORDER) need += widths[k];
  for (const k of TRANSPORT_ORDER) {
    if (need <= available) break;
    shown[k] = false;
    need -= widths[k] - (stubs[k] ?? 0);
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

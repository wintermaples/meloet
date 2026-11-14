// 数値。名前は docs/design/spec.html の「1 数値の一覧」のまま。値は拡大率 100 % のときの px。

export const SCALES = [100, 125, 150] as const;
export type Scale = (typeof SCALES)[number];
type Triple = readonly [number, number, number];

export const colors = {
  "color.gray.0": "#0A0A0A",
  "color.gray.1": "#121212",
  "color.gray.2": "#161616",
  "color.gray.3": "#1A1A1A",
  "color.gray.4": "#222222",
  "color.gray.5": "#2C2C2C",
  "color.gray.6": "#3A3A3A",
  "color.gray.7": "#5E5E5E",
  "color.gray.8": "#9A9A9A",
  "color.gray.9": "#D6D6D6",
  "color.gray.10": "#F2F2F2",
  "color.accent": "#FFB000",
  "color.rec": "#FF3B30",
  "color.meter.ok": "#4CD964",
  "color.meter.hot": "#FFD60A",
  "color.track.1": "#69E4E4",
  "color.track.2": "#5ABDF2",
  "color.track.3": "#6F8BED",
  "color.track.4": "#8C54C3",
  "color.track.5": "#FFD9F1",
  "color.tip.bg": "#F2F2F2",
  "color.tip.fg": "#0A0A0A",
} as const;
export type ColorName = keyof typeof colors;

/** 太さ: perScale は率ごとに変わる（100/125/150 %）。そうでなければ、使ってよい太さの一覧。 */
export interface TextToken {
  size: Triple;
  lineHeight: Triple;
  weight: { perScale: true; values: Triple } | { perScale: false; values: readonly number[] };
}

export const text = {
  "text.s": { size: [12, 15, 18], lineHeight: [16, 20, 24], weight: { perScale: true, values: [400, 500, 600] } },
  "text.m": { size: [16, 20, 24], lineHeight: [20, 25, 30], weight: { perScale: false, values: [400, 500] } },
  "text.l": { size: [20, 25, 30], lineHeight: [24, 30, 36], weight: { perScale: false, values: [600] } },
} as const satisfies Record<string, TextToken>;
export type TextName = keyof typeof text;

export const space = {
  "space.1": [4, 5, 6],
  "space.2": [8, 10, 12],
  "space.3": [12, 15, 18],
  "space.4": [16, 20, 24],
  "space.5": [20, 25, 30],
  "space.6": [24, 30, 36],
} as const satisfies Record<string, Triple>;
export type SpaceName = keyof typeof space;

export const size = {
  "size.transport": [48, 60, 72],
  "size.meter.full": [52, 65, 78],
  "size.meter.slim": [24, 30, 36],
  "size.meter.name": [108, 135, 162],
  "size.meter.name.slim": [176, 220, 264],
  "size.bandrow": [24, 30, 36],
  "size.status": [24, 30, 36],
  "size.row": [24, 30, 36],
  "size.row.s": [20, 25, 30],
  "size.note": [12, 15, 18],
  "size.toolbar": [32, 40, 48],
  "size.track": [64, 80, 96],
  "size.lane": [48, 60, 72],
  "size.lane.vel": [72, 90, 108],
  "size.overview": [40, 50, 60],
  "size.col.head": [220, 275, 330],
  "size.col.keys": [104, 130, 156],
  "size.panel.list": [216, 270, 324],
  "size.panel.insp": [240, 300, 360],
  "size.shelf": [320, 400, 480],
  "size.tab.v": [24, 30, 36],
  "size.strip": [108, 135, 162],
} as const satisfies Record<string, Triple>;
export type SizeName = keyof typeof size;

/** line.hair は率によらず物理 1px。ここでは 100 % の値だけを持ち、CSS px にするときに devicePixelRatio で割る。 */
export const line = {
  "line.hair": 1,
  "line.mark": [2, 2, 3],
  "line.track": [4, 5, 6],
} as const;
export type LineName = keyof typeof line;

/** radius.1 は拡大しても 1。radius.round は割合。 */
export const radius = {
  "radius.0": 0,
  "radius.1": 1,
  "radius.round": "50%",
} as const;
export type RadiusName = keyof typeof radius;

export const fontFamily = {
  sans: "'IBM Plex Sans JP','Noto Sans JP',sans-serif",
  mono: "'IBM Plex Mono',ui-monospace,Menlo,monospace",
} as const;

/**
 * 仕様書の表にない、文の中にしかない寸法（100 % の px）。
 * spec: どの章（s1 / s2）の、どの行（表の左はしのセル）の、どの言葉か。テストが仕様書と照らし合わせる。
 */
export interface PartToken {
  value: number;
  spec: { section: "s1" | "s2"; row: string; quote: string };
}
const p = (value: number, section: "s1" | "s2", row: string, quote: string): PartToken => ({
  value,
  spec: { section, row, quote },
});

export const part = {
  "part.button.height": p(24, "s2", "ボタン", "高さ 24"),
  "part.button.pad-x": p(8, "s2", "ボタン", "左右の内がわ 8"),
  "part.button.icon": p(10, "s2", "ボタン", "10px のアイコン"),
  "part.toggle.size": p(20, "s2", "切りかえ M ・ S ・ ●", "20×20"),
  "part.insert.height": p(24, "s2", "インサートの枠", "高さ 24"),
  "part.bandtab.height": p(24, "s2", "帯札（たたんだ見出し）", "高さ 24"),
  "part.listrow.height": p(24, "s2", "一覧の行", "高さ 20 か 24"),
  "part.listrow.height-s": p(20, "s2", "一覧の行", "高さ 20 か 24"),
  "part.input.height": p(24, "s2", "入力の枠", "高さ 24"),
  "part.fader.knob-w": p(24, "s2", "フェーダー", "つまみ 24×12"),
  "part.fader.knob-h": p(12, "s2", "フェーダー", "つまみ 24×12"),
  "part.pan.size": p(24, "s2", "パンのつまみ", "直径 24"),
  "part.rec.dot": p(12, "s2", "録音ボタン", "直径 12"),
  "part.meter.horizontal": p(4, "s2", "メーター", "横 4px"),
  "part.meter.vertical": p(6, "s2", "メーター", "幅 6px を 2 本"),
  "part.meter.clip": p(2, "s2", "メーター", "rec の 2px"),
  "part.note.height": p(11, "s2", "ノート", "高さ 11"),
  "part.note.edge": p(2, "s2", "ノート", "gray.10 の 2px"),
  "part.tip.width": p(248, "s2", "用語の説明", "幅 248"),
  "part.tip.pad": p(8, "s2", "用語の説明", "内がわ 8"),
  "part.choice.size": p(12, "s2", "選ぶ印（設定の中）", "12×12"),
  "part.choice.dot": p(6, "s2", "選ぶ印（設定の中）", "6×6"),
  "part.shelfrow.height": p(24, "s2", "音源の行（棚）", "高さ 24。"),
  "part.shelfrow.star": p(12, "s2", "音源の行（棚）", "12px の星"),
  "part.shelfrow.sample": p(20, "s2", "音源の行（棚）", "サンプルのボタン（20×20）"),
  "part.shelfrow.group": p(24, "s2", "音源の行（棚）", "グループの見出しは高さ 24"),
  "part.endpoint.name": p(168, "s2", "出口 ・ 入口の行", "本物の名前（168）"),
  "part.endpoint.friendly": p(150, "s2", "出口 ・ 入口の行", "やさしい言いかえ（150"),
  "part.endpoint.link": p(6, "s2", "出口 ・ 入口の行", "ささっているかの印（6×6"),
  "part.endpoint.height-connect": p(32, "s2", "出口 ・ 入口の行", "流れ 2 では高さ 32"),
  "part.endpoint.height-settings": p(24, "s2", "出口 ・ 入口の行", "音の設定では高さ 24"),
  "part.param.height": p(24, "s2", "パラメータの行", "高さ 24"),
  "part.param.name": p(104, "s2", "パラメータの行", "名前（104"),
  "part.param.control": p(88, "s2", "パラメータの行", "動かす所（88）"),
  "part.param.value": p(68, "s2", "パラメータの行", "値（68"),
  "part.param.automation": p(20, "s2", "パラメータの行", "オートメーションに書く印（20×20）"),
  "part.param.knob-w": p(6, "s2", "パラメータの行", "つまみ 6×12"),
  "part.param.knob-h": p(12, "s2", "パラメータの行", "つまみ 6×12"),
  "part.param.step": p(6, "s2", "パラメータの行", "6×6 の四角のならび"),
  "part.param.switch": p(12, "s2", "パラメータの行", "12×12 の四角の印"),
  "part.paramgroup.height": p(24, "s2", "パラメータのグループの見出し", "高さ 24"),
  "part.windowmark.size": p(10, "s2", "窓がひらいている印", "10×10"),
  "part.windowlist.width": p(380, "s2", "状態行の「窓」", "幅 380"),
  "part.windowlist.row": p(24, "s2", "状態行の「窓」", "行は高さ 24"),
  "part.dialog.width": p(560, "s2", "ダイアログ", "幅 560"),
  "part.dialog.head": p(32, "s2", "ダイアログ", "見出し 32"),
  "part.dialog.foot": p(48, "s2", "ダイアログ", "下の列 48"),
  "part.meter.name.long": p(148, "s1", "size.meter.name", "長い名前は 148"),
  "part.meter.name.slim-narrow": p(128, "s1", "size.meter.name.slim", "せまい窓は 128"),
  "part.track.vocal": p(160, "s1", "size.track", "歌を直すときは 160"),
  "part.strip.master": p(124, "s1", "size.strip", "マスターは 124"),
} as const satisfies Record<string, PartToken>;
export type PartName = keyof typeof part;

const scaleIndex = (scale: Scale): 0 | 1 | 2 => (scale === 100 ? 0 : scale === 125 ? 1 : 2);

/** 部品の寸法に率をかけた値。整数にならないことがある（tests/design で一覧にしている）。 */
export function partAt(name: PartName, scale: Scale): number {
  return (part[name].value * scale) / 100;
}

/** 名前 → その率での値。数は CSS px、色は 16 進数、太さは数。 */
export function tokensAt(scale: Scale, devicePixelRatio: number): Record<string, number | string> {
  const i = scaleIndex(scale);
  const out: Record<string, number | string> = {};
  for (const [k, v] of Object.entries(colors)) out[k] = v;
  for (const [k, v] of Object.entries(text) as [TextName, TextToken][]) {
    out[`${k}.size`] = v.size[i];
    out[`${k}.line-height`] = v.lineHeight[i];
    if (v.weight.perScale) out[`${k}.weight`] = v.weight.values[i];
    else if (v.weight.values.length === 1) out[`${k}.weight`] = v.weight.values[0];
    else v.weight.values.forEach((w, n) => (out[`${k}.weight-${n + 1}`] = w));
  }
  for (const [k, v] of Object.entries(space)) out[k] = v[i];
  for (const [k, v] of Object.entries(size)) out[k] = v[i];
  out["line.hair"] = line["line.hair"] / devicePixelRatio;
  out["line.mark"] = line["line.mark"][i];
  out["line.track"] = line["line.track"][i];
  for (const [k, v] of Object.entries(radius)) out[k] = v;
  for (const k of Object.keys(part) as PartName[]) out[k] = partAt(k, scale);
  return out;
}

const unitless = (name: string) => name.includes(".weight");

/** `--color-gray-4: #222222` のような CSS の変数の組。 */
export function cssVariables(scale: Scale, devicePixelRatio: number): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(tokensAt(scale, devicePixelRatio))) {
    out[`--${k.replaceAll(".", "-")}`] = typeof v === "string" ? v : unitless(k) ? String(v) : `${v}px`;
  }
  out["--font-sans"] = fontFamily.sans;
  out["--font-mono"] = fontFamily.mono;
  return out;
}

/** :root（または渡した要素）に CSS の変数を書く。率や devicePixelRatio が変わったら呼びなおす。 */
export function writeCssVariables(
  scale: Scale,
  devicePixelRatio: number = window.devicePixelRatio || 1,
  root: HTMLElement = document.documentElement,
): void {
  for (const [k, v] of Object.entries(cssVariables(scale, devicePixelRatio))) root.style.setProperty(k, v);
}

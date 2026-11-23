// The color sets of the themes (the owner's request, user:2026-11-23). Every theme gives a value
// to every color name of docs/spec/1-tokens.md; "dark" is the spec's own table. The values of the
// other themes, and the contrast rules they meet (WCAG AA), are in docs/spec/1-tokens.md too.

import { colors, type ColorName } from "./tokens";

export const THEME_IDS = ["dark", "light", "pinkDark", "pinkLight", "mintDark", "mintLight"] as const;
export type ThemeId = (typeof THEME_IDS)[number];

export type Palette = Readonly<Record<ColorName, string>>;

export const DEFAULT_THEME: ThemeId = "dark";

const light = {
  "color.gray.0": "#FFFFFF",
  "color.gray.1": "#EAEAEA",
  "color.gray.2": "#E1E1E1",
  "color.gray.3": "#D8D8D8",
  "color.gray.4": "#CDCDCD",
  "color.gray.5": "#BBBBBB",
  "color.gray.6": "#A7A7A7",
  "color.gray.7": "#7A7A7A",
  "color.gray.8": "#575757",
  "color.gray.9": "#333333",
  "color.gray.10": "#111111",
  "color.accent": "#735000",
  "color.rec": "#A5261F",
  "color.meter.ok": "#2D813C",
  "color.meter.hot": "#906C00",
  "color.track.1": "#007F7F",
  "color.track.2": "#1977B6",
  "color.track.3": "#4668E1",
  "color.track.4": "#9551CD",
  "color.track.5": "#C23F87",
  "color.tip.bg": "#111111",
  "color.tip.fg": "#FFFFFF",
} as const satisfies Palette;

const pinkDark = {
  "color.gray.0": "#040404",
  "color.gray.1": "#211B1E",
  "color.gray.2": "#292225",
  "color.gray.3": "#30282C",
  "color.gray.4": "#392F33",
  "color.gray.5": "#483B41",
  "color.gray.6": "#57494F",
  "color.gray.7": "#806E77",
  "color.gray.8": "#A7969E",
  "color.gray.9": "#DBD5D7",
  "color.gray.10": "#F3F2F2",
  "color.accent": "#FF7FC8",
  "color.rec": "#FF685F",
  "color.meter.ok": "#4CD964",
  "color.meter.hot": "#FFD60A",
  "color.track.1": "#69E4E4",
  "color.track.2": "#5ABDF2",
  "color.track.3": "#6F8BED",
  "color.track.4": "#9664C8",
  "color.track.5": "#FFD54A",
  "color.tip.bg": "#F3F2F2",
  "color.tip.fg": "#040404",
} as const satisfies Palette;

const pinkLight = {
  "color.gray.0": "#FBF9F9",
  "color.gray.1": "#EEE0E6",
  "color.gray.2": "#E8D6DE",
  "color.gray.3": "#E2CCD6",
  "color.gray.4": "#DBC1CD",
  "color.gray.5": "#CFADBD",
  "color.gray.6": "#BF98AA",
  "color.gray.7": "#866F7A",
  "color.gray.8": "#5D5056",
  "color.gray.9": "#373134",
  "color.gray.10": "#121011",
  "color.accent": "#8E2879",
  "color.rec": "#9F251E",
  "color.meter.ok": "#2C7D3A",
  "color.meter.hot": "#8B6900",
  "color.track.1": "#007B7B",
  "color.track.2": "#1873B0",
  "color.track.3": "#4164E0",
  "color.track.4": "#924CCC",
  "color.track.5": "#8E6800",
  "color.tip.bg": "#121011",
  "color.tip.fg": "#FBF9F9",
} as const satisfies Palette;

const mintDark = {
  "color.gray.0": "#040404",
  "color.gray.1": "#171F1C",
  "color.gray.2": "#1D2723",
  "color.gray.3": "#232E2A",
  "color.gray.4": "#293531",
  "color.gray.5": "#33423D",
  "color.gray.6": "#3F514A",
  "color.gray.7": "#637971",
  "color.gray.8": "#8D9E98",
  "color.gray.9": "#D3D7D6",
  "color.gray.10": "#F1F3F2",
  "color.accent": "#5CF0B4",
  "color.rec": "#FF6A61",
  "color.meter.ok": "#4CD964",
  "color.meter.hot": "#FFD60A",
  "color.track.1": "#FFD54A",
  "color.track.2": "#5ABDF2",
  "color.track.3": "#6F8BED",
  "color.track.4": "#9765C9",
  "color.track.5": "#FFD9F1",
  "color.tip.bg": "#F1F3F2",
  "color.tip.fg": "#040404",
} as const satisfies Palette;

const mintLight = {
  "color.gray.0": "#F8FAF9",
  "color.gray.1": "#DBE7E3",
  "color.gray.2": "#D0DFDA",
  "color.gray.3": "#C4D7D1",
  "color.gray.4": "#B6CEC6",
  "color.gray.5": "#A0BDB3",
  "color.gray.6": "#89AB9F",
  "color.gray.7": "#617C73",
  "color.gray.8": "#485852",
  "color.gray.9": "#2D3432",
  "color.gray.10": "#0F1111",
  "color.accent": "#0B603F",
  "color.rec": "#A0251E",
  "color.meter.ok": "#2C7E3A",
  "color.meter.hot": "#8C6900",
  "color.track.1": "#8F6800",
  "color.track.2": "#1874B0",
  "color.track.3": "#4165E0",
  "color.track.4": "#924DCC",
  "color.track.5": "#BD3D84",
  "color.tip.bg": "#0F1111",
  "color.tip.fg": "#F8FAF9",
} as const satisfies Palette;

export const palettes: Readonly<Record<ThemeId, Palette>> = {
  dark: colors,
  light,
  pinkDark,
  pinkLight,
  mintDark,
  mintLight,
};

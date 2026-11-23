// The color sets of the themes (the owner's request, user:2026-11-23). Every theme gives a value
// to every color name of docs/spec/1-tokens.md; "dark" is the spec's own table. The values of the
// other themes, and the contrast rules they meet (WCAG AA), are in docs/spec/1-tokens.md too.

import { colors, type ColorName } from "./tokens";

export const THEME_IDS = ["dark", "light", "pinkDark", "pinkLight", "mintDark", "mintLight"] as const;
export type ThemeId = (typeof THEME_IDS)[number];

export type Palette = Readonly<Record<ColorName, string>>;

export const DEFAULT_THEME: ThemeId = "dark";

export const palettes: Readonly<Record<ThemeId, Palette>> = {
  dark: colors,
  light: colors,
  pinkDark: colors,
  pinkLight: colors,
  mintDark: colors,
  mintLight: colors,
};

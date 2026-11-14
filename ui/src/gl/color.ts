import { colors, type ColorName } from "../design/tokens";

/** 0..1 per channel, straight from the sRGB hex (the canvas is sRGB too, so no conversion). */
export type Rgb = readonly [number, number, number];

export function hexToRgb(hex: string): Rgb {
  const m = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  if (!m) throw new Error(`not a #RRGGBB color: ${hex}`);
  return [parseInt(m[1]!, 16) / 255, parseInt(m[2]!, 16) / 255, parseInt(m[3]!, 16) / 255];
}

const cache = new Map<ColorName, Rgb>();

export function rgb(name: ColorName): Rgb {
  let c = cache.get(name);
  if (!c) cache.set(name, (c = hexToRgb(colors[name])));
  return c;
}

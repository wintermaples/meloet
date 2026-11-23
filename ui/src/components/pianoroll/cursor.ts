// Cursors (spec 2 "Cursor (pencil, eraser)", spec 3 "Cursors use the OS shapes").
// The pencil and eraser are meloet's own parts: a 16×16 line drawing, face gray.10, 1px edge gray.0. The shapes are the line drawings of mock R3-PRMouse.
// The point that acts is the tip at the bottom left (2, 14) for the pencil, and the bottom-left corner (4, 12) for the eraser. They grow with the scale. Others use the OS shapes.

import { palettes, type ThemeId } from "../../design/themes";
import { part } from "../../design/tokens";
import type { CursorKind } from "./drag";

const PATHS = {
  pencil: "M2 14L3 10L11 2L14 5L6 13ZM3 10L6 13",
  eraser: "M2 10L8 4L14 10L10 14H6ZM5 7L11 13",
} as const;
const HOT = { pencil: [2, 14], eraser: [4, 12] } as const;

function drawn(kind: "pencil" | "eraser", scale: number, theme: ThemeId): string {
  const unit = part["part.cursor.size"].value;
  const size = Math.round((unit * scale) / 100);
  const colors = palettes[theme];
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${unit} ${unit}">` +
    `<path d="${PATHS[kind]}" fill="${colors["color.gray.10"]}" stroke="${colors["color.gray.0"]}" stroke-width="1"/></svg>`;
  const [hx, hy] = HOT[kind];
  const k = size / unit;
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}") ${Math.round(hx * k)} ${Math.round(hy * k)}, crosshair`;
}

const cache = new Map<string, string>();

/** The CSS cursor value, in the colors of `theme`. scale is the scale. */
export function cursorCss(kind: CursorKind, scale: number, theme: ThemeId): string {
  if (kind !== "pencil" && kind !== "eraser") return kind;
  const id = `${kind}@${scale}@${theme}`;
  let css = cache.get(id);
  if (!css) cache.set(id, (css = drawn(kind, scale, theme)));
  return css;
}

// カーソル（仕様書 2 章「カーソル（鉛筆 ・ 消しゴム）」、3 章「カーソルは OS の形を使う」）。
// 鉛筆と消しゴムは meloet の部品: 16×16 の線画、面 gray.10、ふち gray.0 の 1px。形はモック R3-PRMouse の線画。
// 押す点は、鉛筆が左下の先 (2, 14)、消しゴムが左下の角 (4, 12)。拡大率に合わせて大きくする。ほかは OS の形。

import { colors, part } from "../../design/tokens";
import type { CursorKind } from "./drag";

const PATHS = {
  pencil: "M2 14L3 10L11 2L14 5L6 13ZM3 10L6 13",
  eraser: "M2 10L8 4L14 10L10 14H6ZM5 7L11 13",
} as const;
const HOT = { pencil: [2, 14], eraser: [4, 12] } as const;

function drawn(kind: "pencil" | "eraser", scale: number): string {
  const unit = part["part.cursor.size"].value;
  const size = Math.round((unit * scale) / 100);
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${unit} ${unit}">` +
    `<path d="${PATHS[kind]}" fill="${colors["color.gray.10"]}" stroke="${colors["color.gray.0"]}" stroke-width="1"/></svg>`;
  const [hx, hy] = HOT[kind];
  const k = size / unit;
  return `url("data:image/svg+xml,${encodeURIComponent(svg)}") ${Math.round(hx * k)} ${Math.round(hy * k)}, crosshair`;
}

const cache = new Map<string, string>();

/** CSS の cursor の値。scale は拡大率。 */
export function cursorCss(kind: CursorKind, scale: number): string {
  if (kind !== "pencil" && kind !== "eraser") return kind;
  const id = `${kind}@${scale}`;
  let css = cache.get(id);
  if (!css) cache.set(id, (css = drawn(kind, scale)));
  return css;
}

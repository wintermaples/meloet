// オートメーションの点の Delete（仕様書 3 章 Y16「えらんで Delete で消す」）。DOM を知らない。
// キーがアレンジに向いていて、最後に押してえらんだのがオートメーションの点のときだけ（timeline/pick.ts）。
// 文字を打っている最中と、録音中 ・ カウントイン中は送らない。

import type { AutomationView, Command } from "../../bridge";
import { lastPick } from "../timeline/pick";
import { indexSet } from "./autoGeom";

/** 送る意図。点の Delete でなければ null（ほかの決まりへ）。"none": 受けたが、えらんだ点がないので何もしない。 */
export function laneDeleteCommand(
  k: { key: string; ctrl: boolean; meta: boolean; alt: boolean; shift: boolean },
  c: { typing: boolean; busy: boolean; toArrange: boolean },
  automation: AutomationView,
): Command | "none" | null {
  if (k.key !== "Delete" || k.ctrl || k.meta || k.alt || k.shift || c.typing || !c.toArrange || lastPick.kind !== "lane") return null;
  const sel = automation.selected;
  if (c.busy || !sel) return "none";
  return { type: "deleteAutoPoints", lane: sel.laneId, points: [...indexSet(sel.points)].sort((a, b) => a - b) };
}

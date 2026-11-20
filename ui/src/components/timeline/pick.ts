// Delete がどれに効くか（棟梁の決め 2026-11-20）: アレンジでは、テンポの点 ・ 拍子の旗 ・ マーカーと、クリップと、
// オートメーションの点のうち、最後に押してえらんだ物。画面だけが持つ（core はどれの選択も持つ）。
export const lastPick = { kind: "clips" as "clips" | "timeline" | "lane" };

export function pickTimeline(): void {
  lastPick.kind = "timeline";
}

export function pickClips(): void {
  lastPick.kind = "clips";
}

export function pickLane(): void {
  lastPick.kind = "lane";
}

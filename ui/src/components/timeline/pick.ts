// Delete がどちらに効くか（棟梁の決め 2026-11-20）: アレンジでは、テンポの点 ・ 拍子の旗 ・ マーカーと、クリップのうち、
// 最後に押してえらんだほう。画面だけが持つ（core は両方の選択を持つ）。
export const lastPick = { kind: "clips" as "clips" | "timeline" };

export function pickTimeline(): void {
  lastPick.kind = "timeline";
}

export function pickClips(): void {
  lastPick.kind = "clips";
}

// What Delete acts on (the lead's decision): in the arrange, whichever of the tempo points, time signature flags, markers, clips
// and automation points was chosen by pressing last. Only the screen keeps this (the core keeps the selection of each).
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

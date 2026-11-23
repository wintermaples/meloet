// What the last copy or cut took, notes or clips: Mod V and the menu bar's paste paste that.
// Only the screen keeps it; Rust keeps the two clipboards themselves.

import type { Command } from "../bridge";

export const copied: { last: "notes" | "clips" | null } = { last: null };

export function noteCopied(command: Command): void {
  if (command.type === "copyNotes" || command.type === "cutNotes") copied.last = "notes";
  else if (command.type === "copyClips" || command.type === "cutClips") copied.last = "clips";
}

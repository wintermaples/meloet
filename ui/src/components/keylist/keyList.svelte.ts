// Whether the 「キーの一覧」 dialog (round 12 G1) is open, and what is typed and folded in it. Only the screen holds it,
// like 「meloet について」: it touches neither the song nor the sound. keys/attach.ts calls keyListBack() in the Esc order.

import type { KeyGroup } from "../../keys/keyList";

export const keyList = $state({
  open: false,
  query: "",
  folded: [] as KeyGroup[],
});

export function openKeyList(): void {
  keyList.query = "";
  keyList.folded = [];
  keyList.open = true;
}

export function closeKeyList(): void {
  keyList.open = false;
}

/** Esc: close. false if not open (next turn). */
export function keyListBack(): boolean {
  if (!keyList.open) return false;
  closeKeyList();
  return true;
}

export function toggleKeyGroup(group: KeyGroup): void {
  keyList.folded = keyList.folded.includes(group) ? keyList.folded.filter((g) => g !== group) : [...keyList.folded, group];
}

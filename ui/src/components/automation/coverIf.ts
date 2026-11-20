// on のあいだだけ、その箱を区画の canvas の下に入れる（glSlot の cover）。オートメーションが毎コマ動かす物だけを canvas の下に入れ、
// 動かない時に canvas を広げないため（docs/architecture.md 7 章「CPU で描く WebKit」の (b)）。

import { glSlot } from "../../gl/slot";

export function coverIf(node: HTMLElement, on: boolean) {
  let slot: ReturnType<typeof glSlot> | null = on ? glSlot(node, { kind: "cover" }) : null;
  return {
    update(next: boolean) {
      if (next && !slot) slot = glSlot(node, { kind: "cover" });
      else if (!next && slot) {
        slot.destroy();
        slot = null;
      }
    },
    destroy() {
      slot?.destroy();
    },
  };
}

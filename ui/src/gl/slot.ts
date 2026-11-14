// Places where WebGL draws. A component puts a box in the HTML layout and marks it with
// `use:glSlot={...}`; the engine (engine.ts) puts a canvas of that box's size into the box, above
// what the box holds, and draws it when its picture changes.

export type MeterSource = "master" | { track: number };

export type GlSlot =
  /** Input meter keyboard. Notes are MIDI numbers, both ends included. `disabled`: no keyboard, drawn dim. */
  | { kind: "keyboard"; size: "full" | "slim"; firstNote: number; lastNote: number; disabled?: boolean }
  /** The bar beside the last velocity in the input meter. */
  | { kind: "velocity" }
  /** Two horizontal level bars (left above right) with `gap` CSS px between them. */
  | { kind: "meter"; source: MeterSource; gap: number }
  /** Bar lines of the arrange area. `barPx` and `firstBarX` are CSS px. */
  | { kind: "arrangeGrid"; barPx: number; firstBarX: number }
  /** The 40 px arrange overview (clip lines, visible range, play position). */
  | { kind: "overview" };

/** Told about every marked box: the ones already in the page, then each change. */
export interface SlotWatcher {
  attach(node: HTMLElement, slot: GlSlot): void;
  update(node: HTMLElement, slot: GlSlot): void;
  detach(node: HTMLElement): void;
}

const slots = new Map<HTMLElement, GlSlot>();
let watcher: SlotWatcher | null = null;

export function glSlot(node: HTMLElement, slot: GlSlot) {
  slots.set(node, slot);
  node.dataset.glSlot = slot.kind;
  watcher?.attach(node, slot);
  return {
    update(next: GlSlot) {
      slots.set(node, next);
      node.dataset.glSlot = next.kind;
      watcher?.update(node, next);
    },
    destroy() {
      slots.delete(node);
      watcher?.detach(node);
    },
  };
}

/** One watcher at a time (the engine). Returns how to stop watching. */
export function watchSlots(next: SlotWatcher): () => void {
  watcher = next;
  for (const [node, slot] of slots) next.attach(node, slot);
  return () => {
    if (watcher === next) watcher = null;
  };
}

// Places where the WebGL surface draws. A component puts an empty box in the HTML layout and
// marks it with `use:glSlot={...}`; the surface draws into that box's rect on every frame.
// The surface itself lives in ui/src/gl/ (one canvas for the whole window).

export type MeterSource = "master" | { track: number };

export type GlSlot =
  /** Input meter keyboard. Notes are MIDI numbers, both ends included. */
  | { kind: "keyboard"; size: "full" | "slim"; firstNote: number; lastNote: number }
  /** The bar beside the last velocity in the input meter. */
  | { kind: "velocity" }
  /** Two horizontal level bars (left above right) with `gap` CSS px between them. */
  | { kind: "meter"; source: MeterSource; gap: number }
  /** Bar lines of the arrange area. `barPx` and `firstBarX` are CSS px. */
  | { kind: "arrangeGrid"; barPx: number; firstBarX: number }
  /** The 40 px arrange overview (clip lines, visible range, play position). */
  | { kind: "overview" };

const slots = new Map<HTMLElement, GlSlot>();

export function glSlot(node: HTMLElement, slot: GlSlot) {
  slots.set(node, slot);
  node.dataset.glSlot = slot.kind;
  return {
    update(next: GlSlot) {
      slots.set(node, next);
      node.dataset.glSlot = next.kind;
    },
    destroy() {
      slots.delete(node);
    },
  };
}

/** For the surface: every marked box that is in the page right now. */
export function liveSlots(): IterableIterator<[HTMLElement, GlSlot]> {
  return slots.entries();
}

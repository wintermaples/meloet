// The fake's side of 「キーで弾く」 (round 12, R2): the switch as Rust takes it, and the keys told back at once as a
// keyboard's are. The fake makes no sound.

import { TYPED_KEYBOARD_ID, type AppState, type Change, type Command, type Event, type KeyboardView, type KeyboardsView, type StatusMessage } from "./types";

/** Rust's TYPED_KEY_VELOCITY. */
const TYPED_KEY_VELOCITY = 100;

/** 「パソコンのキーボード」 in the list: Rust leaves the name to the screen. */
export function typedKeyboard(): KeyboardView {
  return { id: TYPED_KEYBOARD_ID, name: "", portName: "", address: "" };
}

/** The keyboards with 「キーで弾く」 on; `played`: the PC keyboard was played last. */
export function typedOn(keyboards: KeyboardsView, played: boolean): KeyboardsView {
  const list = [...keyboards.list.filter((k) => k.id !== TYPED_KEYBOARD_ID), typedKeyboard()];
  const active = played || keyboards.active === null ? TYPED_KEYBOARD_ID : keyboards.active;
  return { ...keyboards, typedKeys: true, list, active };
}

export class FakeTypedKeys {
  private held = new Set<number>();

  /**
   * `typedKeys` and `typedKey` as Rust takes them; `null` for other commands. `rest`: the scene's status line, back
   * when it is turned off. The events are told after the changes.
   */
  reduce(state: Pick<AppState, "keyboards" | "status">, command: Command, rest: StatusMessage | null): { changes: Change[]; events: Event[] } | null {
    const keyboards = state.keyboards;
    const note = (on: boolean, n: number, velocity: number, tUs: number): Event => ({ type: "note", on, note: n, velocity, tUs });
    switch (command.type) {
      case "typedKeys": {
        if (command.on === keyboards.typedKeys) return { changes: [], events: [] };
        if (command.on) {
          return {
            changes: [
              { section: "keyboards", value: typedOn(keyboards, false) },
              { section: "status", value: { message: { id: "typedKeysOn" } } },
            ],
            events: [],
          };
        }
        const list = keyboards.list.filter((k) => k.id !== TYPED_KEYBOARD_ID);
        const active = keyboards.active === TYPED_KEYBOARD_ID ? (list.at(-1)?.id ?? null) : keyboards.active;
        const events = [...this.held].sort((a, b) => a - b).map((n) => note(false, n, 0, 0));
        this.held.clear();
        const changes: Change[] = [{ section: "keyboards", value: { ...keyboards, typedKeys: false, list, active } }];
        if (state.status.message?.id === "typedKeysOn") changes.push({ section: "status", value: { message: rest?.id === "typedKeysOn" ? null : rest } });
        return { changes, events };
      }
      case "typedKey": {
        const { note: n, down, tUs } = command;
        if (!keyboards.typedKeys || n > 127 || down === this.held.has(n)) return { changes: [], events: [] };
        if (!down) {
          this.held.delete(n);
          return { changes: [], events: [note(false, n, 0, tUs)] };
        }
        this.held.add(n);
        const fixed = keyboards.velocityFixed;
        const changes: Change[] = keyboards.active === TYPED_KEYBOARD_ID ? [] : [{ section: "keyboards", value: { ...keyboards, active: TYPED_KEYBOARD_ID } }];
        return { changes, events: [note(true, n, fixed.on ? fixed.value : TYPED_KEY_VELOCITY, tUs)] };
      }
      default:
        return null;
    }
  }
}

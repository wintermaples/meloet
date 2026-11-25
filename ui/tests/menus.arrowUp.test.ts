// ↑ with the focus on a list box itself (no row has it) goes to the last row, in every ▾ list (as MenuList does).
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, describe, expect, it } from "vitest";
import { fakeScene } from "../src/bridge/fake";
import ChoiceMenu from "../src/components/ChoiceMenu.svelte";
import WindowsMenu from "../src/components/WindowsMenu.svelte";
import ZoomMenu from "../src/components/ZoomMenu.svelte";

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement | null = null;
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target?.remove();
});

const windows = fakeScene("windowOpen")!.state.windows.list;

describe.each([
  ["ChoiceMenu", ChoiceMenu as Component<any>, { heading: null, label: "x", rows: [{ value: 1, label: "a" }, { value: 2, label: "b" }, { value: 3, label: "c" }], onpick: () => {}, test: "list" }],
  ["ZoomMenu", ZoomMenu as Component<any>, { scale: 100, unfit: [], platform: "other", onpick: () => {} }],
  ["WindowsMenu", WindowsMenu as Component<any>, { list: windows, placeOf: () => "", onpick: () => {} }],
])("UI-PARITY #10 (later): %s", (_name, component, props) => {
  it("↑ on the list box goes to the last row, ↓ to the first", async () => {
    target = document.createElement("div");
    document.body.append(target);
    mounted = mount(component, { target, props });
    flushSync();
    await Promise.resolve();
    const box = target.querySelector<HTMLElement>('[role="menu"]')!;
    const rows = [...box.querySelectorAll<HTMLButtonElement>("button:not(:disabled)")];
    expect(rows.length).toBeGreaterThan(1);
    const press = (key: string) => {
      box.focus();
      box.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
    };
    press("ArrowUp");
    expect(document.activeElement).toBe(rows.at(-1));
    press("ArrowDown");
    expect(document.activeElement).toBe(rows[0]);
  });
});

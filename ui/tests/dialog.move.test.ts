import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import SettingsDialog from "../src/components/SettingsDialog.svelte";
import SongDialog from "../src/components/SongDialog.svelte";
import ScanStoppedDialog from "../src/components/plugins/ScanStoppedDialog.svelte";
import { handleKeydown } from "../src/keys/attach";
import { clampOffset } from "../src/lib/dialogMove";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const WIDTH = 1024;
const HEIGHT = 768;
/** The box before moving (around the middle of the window). */
const BASE = { left: 232, top: 200, width: 560, height: 300 };

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;

beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
  vi.spyOn(window, "innerWidth", "get").mockReturnValue(WIDTH);
  vi.spyOn(window, "innerHeight", "get").mockReturnValue(HEIGHT);
  // jsdom does not lay out boxes, so the dialog box is BASE plus the transform amount.
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    const m = /translate\((-?[\d.]+)px, (-?[\d.]+)px\)/.exec(this.style.transform);
    const x = BASE.left + (m ? Number(m[1]) : 0);
    const y = BASE.top + (m ? Number(m[2]) : 0);
    return DOMRect.fromRect({ x, y, width: BASE.width, height: BASE.height });
  });
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  document.body.innerHTML = "";
  vi.restoreAllMocks();
});

function show(component: Component<any>, state: AppState): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}

const settings = () =>
  app.stateWith((s) => {
    s.workspace.settings = { draftOutput: null, advancedOpen: false };
  });
const song = () =>
  app.stateWith((s) => {
    s.workspace.songDialog = { id: 7, kind: "unsaved", title: "はじめての曲", previous: false };
  });

const scanStopped = () => fakeScene("scanStopped")!.state;

const q = <T extends HTMLElement = HTMLElement>(sel: string) => document.querySelector<T>(sel)!;
const dialog = () => q('[role="dialog"]');
const head = () => q("[data-dialog-head]");

function pointer(on: EventTarget, type: string, x: number, y: number): PointerEvent {
  const ev = new PointerEvent(type, { clientX: x, clientY: y, button: 0, pointerId: 1, bubbles: true, cancelable: true });
  on.dispatchEvent(ev);
  return ev;
}

/** Presses from, moves by (dx, dy), and does not release. */
function press(from: Element, dx: number, dy: number): void {
  pointer(from, "pointerdown", 300, 210);
  pointer(from, "pointermove", 300 + dx, 210 + dy);
}

function dragBy(from: Element, dx: number, dy: number): void {
  press(from, dx, dy);
  pointer(from, "pointerup", 300 + dx, 210 + dy);
}

function esc(state: AppState, sent: Command[]): void {
  const deps = { state: () => state, send: (c: Command) => void sent.push(c), nav: { platform: "Linux x86_64" } };
  handleKeydown(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }), deps);
}

describe("clampOffset", () => {
  const base = { left: 232, top: 200, right: 792, bottom: 500 };
  it.each([
    [10, -20, 10, -20],
    [-1000, -1000, -232, -200],
    [1000, 1000, WIDTH - 792, HEIGHT - 500],
  ])("(%i, %i) → (%i, %i)", (x, y, ex, ey) => {
    expect(clampOffset(base, x, y, WIDTH, HEIGHT)).toEqual({ x: ex, y: ey });
  });

  it("a box bigger than the window keeps its left and top edges in the window", () => {
    expect(clampOffset({ left: -20, top: -10, right: 540, bottom: 290 }, 100, 100, 400, 200)).toEqual({ x: 20, y: 10 });
  });
});

describe.each([
  ["音の設定", SettingsDialog as Component<any>, settings],
  ["曲のダイアログ", SongDialog as Component<any>, song],
  ["スキャンで止まったプラグイン", ScanStoppedDialog as Component<any>, scanStopped],
])("%s を動かす", (_name, component, state) => {
  it("moves when the heading row is held and moved", () => {
    show(component, state());
    dragBy(head(), 50, 40);
    expect(dialog().style.transform).toBe("translate(50px, 40px)");
    // moves after release do not move it.
    pointer(head(), "pointermove", 600, 600);
    expect(dialog().style.transform).toBe("translate(50px, 40px)");
  });

  it("does not move when the contents or the bottom row are pressed", () => {
    show(component, state());
    dragBy(q(".foot"), 50, 40);
    expect(dialog().style.transform).toBe("");
  });

  it("does not leave the window. Comes back inside when the window gets smaller", () => {
    show(component, state());
    dragBy(head(), -1000, -1000);
    expect(dialog().style.transform).toBe(`translate(${-BASE.left}px, ${-BASE.top}px)`);
    dragBy(head(), 5000, 5000);
    const right = WIDTH - BASE.left - BASE.width;
    const bottom = HEIGHT - BASE.top - BASE.height;
    expect(dialog().style.transform).toBe(`translate(${right}px, ${bottom}px)`);

    vi.spyOn(window, "innerWidth", "get").mockReturnValue(WIDTH - 100);
    vi.spyOn(window, "innerHeight", "get").mockReturnValue(HEIGHT - 50);
    window.dispatchEvent(new Event("resize"));
    expect(dialog().style.transform).toBe(`translate(${right - 100}px, ${bottom - 50}px)`);
  });

  it("opening again starts from the first place", () => {
    show(component, state());
    dragBy(head(), 50, 40);
    unmount(mounted!);
    mounted = null;
    show(component, state());
    expect(dialog().style.transform).toBe("");
  });

  it("Esc during a drag only goes back to the place before the move. Esc without a drag keeps it as it is", () => {
    const s = state();
    show(component, s);
    dragBy(head(), 50, 40);
    press(head(), 100, 100);
    expect(dialog().style.transform).toBe("translate(150px, 140px)");
    const sent: Command[] = [];
    esc(s, sent);
    expect(dialog().style.transform).toBe("translate(50px, 40px)");
    expect(sent).toEqual([]);
    // after cancelling, it does not move until release.
    pointer(head(), "pointermove", 700, 700);
    pointer(head(), "pointerup", 700, 700);
    expect(dialog().style.transform).toBe("translate(50px, 40px)");
    esc(s, sent);
    expect(sent).toEqual([{ type: "back" }]);
  });
});

it("pressing 「とじる」 in the sound settings does not move it", () => {
  show(SettingsDialog, settings());
  const close = q('[data-test="settings-close"]');
  expect(head().contains(close)).toBe(true);
  dragBy(close, 50, 40);
  dragBy(close.querySelector(".mono")!, 50, 40);
  expect(dialog().style.transform).toBe("");
});

it("the cursor on the heading row is move", () => {
  for (const file of ["SettingsDialog.svelte", "SongDialog.svelte"]) {
    const css = /\.head \{[^}]*\}/.exec(readSource(file))![0];
    expect(css).toContain("cursor: move;");
  }
});

const sources = import.meta.glob<string>("../src/components/*Dialog.svelte", { query: "?raw", import: "default", eager: true });
function readSource(file: string): string {
  return sources[`../src/components/${file}`]!;
}

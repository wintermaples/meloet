// 「拡大率」 in the status line and its list (spec 2, spec 3, mock F-Zoom). How the right side of the status line hides. Scale and band notices.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, StatusMessage } from "../src/bridge";
import StatusLine from "../src/components/StatusLine.svelte";
import { statusFit } from "../src/components/statusFit";
import ZoomMenu from "../src/components/ZoomMenu.svelte";
import { handleKeydown } from "../src/keys/attach";
import { menuVisible } from "../src/lib/menu";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;

beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  vi.restoreAllMocks();
});

function show(component: Component<any>, state: AppState, props: Record<string, unknown> = {}): void {
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}

// The list is placed in body, so search from document.
const q = (sel: string) => document.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...document.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

/** Now 100 %; 125 % does not fit (same as the fake scene zoom). */
const zoomState = (edit: (s: AppState) => void = () => {}) =>
  app.stateWith((s) => {
    s.workspace.scale = 100;
    s.workspace.unfitScales = [125];
    edit(s);
  });

function openMenu(): void {
  q('[data-test="status-scale"]')!.click();
  flushSync();
}

const rows = () => qa('[data-test="zoom-menu"] [role="menuitemradio"]');

describe("the 「拡大率」 button in the status line", () => {
  it("the current scale and ▾. At the far right, pressing opens the list", () => {
    show(StatusLine, zoomState());
    const button = q('[data-test="status-scale"]')!;
    expect(text(button)).toBe("100 %▾");
    expect(q("footer.status")!.lastElementChild).toBe(button);
    expect(button.getAttribute("aria-expanded")).toBe("false");
    expect(q('[data-test="zoom-menu"]')).toBeNull();
    openMenu();
    expect(button.getAttribute("aria-expanded")).toBe("true");
    expect(button.classList.contains("open")).toBe(true);
    expect(q('[data-test="zoom-menu"]')).not.toBeNull();
  });

  it("pressing again closes", () => {
    show(StatusLine, zoomState());
    openMenu();
    openMenu();
    expect(q('[data-test="zoom-menu"]')).toBeNull();
  });
});

describe("scale list", () => {
  it("a heading and 2 rows. The current scale is 「えらんだ」, scales that do not fit are disabled (cannot be pressed) with 「この画面には入りません」", () => {
    show(StatusLine, zoomState());
    openMenu();
    expect(text(q('[data-test="zoom-menu"] .head'))).toBe("拡大率");
    expect(rows().map((r) => r.querySelector(".rate")!.textContent)).toEqual(["100 %", "125 %"]);
    expect(rows().map((r) => r.getAttribute("aria-checked"))).toEqual(["true", "false"]);
    expect(rows().map((r) => r.querySelector(".choice")!.classList.contains("selected"))).toEqual([true, false]);
    expect(rows().map((r) => (r as HTMLButtonElement).disabled)).toEqual([false, true]);
    expect(rows().map((r) => text(r.querySelector('[data-test="zoom-unfit"]')))).toEqual([null, "この画面には入りません"]);
  });

  it("if every scale fits, every row can be pressed", () => {
    show(StatusLine, app.stateWith((s) => (s.workspace.unfitScales = [])));
    openMenu();
    expect(rows().map((r) => (r as HTMLButtonElement).disabled)).toEqual([false, false]);
    expect(qa('[data-test="zoom-unfit"]')).toEqual([]);
  });

  it("key text: 「Ctrl Alt 1」 on Linux and Windows, 「Cmd Option 1」 on Mac (docs/history/design-notes.ja.md A-2)", () => {
    app.setState(zoomState());
    mounted = mount(ZoomMenu, { target, props: { scale: 100, unfit: [125], platform: "other", onpick: () => {} } });
    flushSync();
    expect(qa('[data-test="zoom-key"]').map(text)).toEqual(["Ctrl Alt 1", "Ctrl Alt 2"]);
    unmount(mounted);
    mounted = mount(ZoomMenu, { target, props: { scale: 100, unfit: [125], platform: "mac", onpick: () => {} } });
    flushSync();
    expect(qa('[data-test="zoom-key"]').map(text)).toEqual(["Cmd Option 1", "Cmd Option 2"]);
  });

  it("pressing a row sends setScale and closes the list", () => {
    show(StatusLine, zoomState((s) => ((s.workspace.scale = 125), (s.workspace.unfitScales = []))));
    openMenu();
    rows()[0]!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "setScale", percent: 100 }]);
    expect(q('[data-test="zoom-menu"]')).toBeNull();
    expect(menuVisible()).toBe(false);
  });

  it("disabled rows cannot be pressed (nothing sent, stays open)", () => {
    show(StatusLine, zoomState());
    openMenu();
    rows()[1]!.click();
    flushSync();
    expect(app.sent).toEqual([]);
    expect(q('[data-test="zoom-menu"]')).not.toBeNull();
  });

  it("opening focuses the current scale row. ↑ ↓ skip disabled rows, Enter decides", async () => {
    const key = (k: string) => {
      document.activeElement!.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true }));
      flushSync();
    };
    show(StatusLine, zoomState());
    openMenu();
    await Promise.resolve();
    expect(document.activeElement).toBe(rows()[0]);
    key("ArrowDown");
    expect(document.activeElement).toBe(rows()[0]);
    openMenu();
    unmount(mounted!);
    mounted = null;
    show(StatusLine, zoomState((s) => ((s.workspace.scale = 125), (s.workspace.unfitScales = []))));
    openMenu();
    await Promise.resolve();
    expect(document.activeElement).toBe(rows()[1]);
    key("ArrowUp");
    expect(document.activeElement).toBe(rows()[0]);
    key("ArrowDown");
    expect(document.activeElement).toBe(rows()[1]);
    key("ArrowUp");
    key("Enter");
    expect(app.sent).toEqual([{ type: "setScale", percent: 100 }]);
    expect(q('[data-test="zoom-menu"]')).toBeNull();
    expect(document.activeElement).toBe(q('[data-test="status-scale"]'));
  });

  it("Esc closes the menu before the dialog (core back is not sent)", () => {
    show(StatusLine, zoomState((s) => (s.workspace.settings = {} as never)));
    openMenu();
    const deps = { state: () => app.current.state, send: app.send };
    handleKeydown(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }), deps);
    flushSync();
    expect(q('[data-test="zoom-menu"]')).toBeNull();
    expect(app.sent).toEqual([]);
    handleKeydown(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }), deps);
    expect(app.sent).toEqual([{ type: "back" }]);
  });

  it("Esc order: the box being typed in → menu → core back (spec 3)", () => {
    show(StatusLine, zoomState());
    openMenu();
    const input = document.createElement("input");
    document.body.append(input);
    input.focus();
    const deps = { state: () => app.current.state, send: app.send };
    const esc = () => {
      handleKeydown(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }), deps);
      flushSync();
    };
    esc();
    expect(document.activeElement).not.toBe(input);
    expect(q('[data-test="zoom-menu"]')).not.toBeNull();
    esc();
    expect(q('[data-test="zoom-menu"]')).toBeNull();
    expect(app.sent).toEqual([]);
    esc();
    expect(app.sent).toEqual([{ type: "back" }]);
    input.remove();
  });

  it("the list is placed in body and shown in front of dialogs (30)", () => {
    show(StatusLine, zoomState());
    openMenu();
    const popup = q('[data-test="zoom-menu"]')!.parentElement!;
    expect(popup.parentElement).toBe(document.body);
    expect(target.contains(popup)).toBe(false);
  });

  it("pressing outside closes. Pressing inside the list does not", () => {
    show(StatusLine, zoomState());
    openMenu();
    q('[data-test="zoom-menu"] .head')!.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    flushSync();
    expect(q('[data-test="zoom-menu"]')).not.toBeNull();
    document.body.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    flushSync();
    expect(q('[data-test="zoom-menu"]')).toBeNull();
  });
});

describe("how the right side of the status line hides (spec 3)", () => {
  const right = { output: 160, inputs: 140, buffer: 80 };
  const base = { gap: 16, left: 300, right, scale: 60 };
  // All: 300 + 2 × 16 + 60 + (160 + 16) + (140 + 16) + (80 + 16) = 820
  const all = 820;

  it("fits: show all", () => {
    expect(statusFit({ ...base, available: all })).toEqual({ inputs: true, output: true, buffer: true });
  });

  it("hides input → output → buffer in that order", () => {
    expect(statusFit({ ...base, available: all - 1 })).toEqual({ inputs: false, output: true, buffer: true });
    expect(statusFit({ ...base, available: all - 156 })).toEqual({ inputs: false, output: true, buffer: true });
    expect(statusFit({ ...base, available: all - 157 })).toEqual({ inputs: false, output: false, buffer: true });
    expect(statusFit({ ...base, available: all - 332 - 1 })).toEqual({ inputs: false, output: false, buffer: false });
    expect(statusFit({ ...base, available: 10 })).toEqual({ inputs: false, output: false, buffer: false });
  });

  it("when the buffer is not shown (sound not running), input → output", () => {
    const noBuffer = { ...base, right: { output: 160, inputs: 140 } };
    expect(statusFit({ ...noBuffer, available: all - 96 })).toEqual({ inputs: true, output: true, buffer: true });
    expect(statusFit({ ...noBuffer, available: all - 97 })).toEqual({ inputs: false, output: true, buffer: true });
  });

  describe("on screen (with the width measuring replaced)", () => {
    const widths: Record<string, number> = { left: 300, output: 160, inputs: 140, buffer: 80, scale: 60 };
    let rowWidth = 0;
    beforeEach(() => {
      vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function (this: HTMLElement) {
        return widths[this.dataset.part ?? ""] ?? 0;
      });
      vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockImplementation(function (this: HTMLElement) {
        return this.matches("footer.status") ? rowWidth : 0;
      });
      // jsdom does not resolve CSS variables, so the spaces are set here.
      const real = window.getComputedStyle;
      vi.spyOn(window, "getComputedStyle").mockImplementation((el, p) => {
        const style = real(el, p);
        return (el as HTMLElement).matches?.("footer.status") ? ({ ...style, columnGap: "16px", paddingLeft: "0", paddingRight: "0" } as CSSStyleDeclaration) : style;
      });
    });

    const status = (width: number) => {
      rowWidth = width;
      show(
        StatusLine,
        app.stateWith((s) => {
          s.keyboards = { ...s.keyboards, list: [{ id: "kb", name: "鍵盤 49", portName: "p", address: "24:0" }], active: "kb", lost: null, inputStopped: false };
          s.audio.actualFrames = 256;
        }),
      );
      return {
        output: q('[data-test="status-output"]') !== null,
        inputs: q('[data-test="status-inputs"]') !== null,
        buffer: q('[data-test="status-buffer"]') !== null,
        scale: q('[data-test="status-scale"]') !== null,
      };
    };

    it("wide: all", () => {
      expect(status(all)).toEqual({ output: true, inputs: true, buffer: true, scale: true });
    });
    it("a little short: from the input", () => {
      expect(status(all - 1)).toEqual({ output: true, inputs: false, buffer: true, scale: true });
    });
    it("shorter: then the output", () => {
      expect(status(all - 157)).toEqual({ output: false, inputs: false, buffer: true, scale: true });
    });
    it("still short: the buffer too. The scale stays, the left text shrinks with 「…」 (CSS min-width: 0)", () => {
      expect(status(200)).toEqual({ output: false, inputs: false, buffer: false, scale: true });
    });
  });
});

describe("scale and band notices (spec 6)", () => {
  const cases: [StatusMessage, string][] = [
    [{ id: "scaleDoesNotFit", requested: 125, kept: 100 }, "125 % は、この画面には入りません。100 % のままです。"],
    [{ id: "scaleWindowGrown", scale: 125 }, "拡大率を 125 % にしました。窓を、いちばん小さい大きさまで広げました。"],
    [{ id: "scaleLoweredForScreen", from: 125, to: 100 }, "画面が小さくなったので、拡大率を 125 % から 100 % にしました。"],
    [{ id: "bandFoldedWindowLow", band: "mixer" }, "窓が低くなったので、ミキサーをたたみました。帯札を押すと、またひらきます。"],
    [{ id: "bandFoldedWindowLow", band: "history" }, "窓が低くなったので、取り消しの履歴をたたみました。帯札を押すと、またひらきます。"],
    [{ id: "bandFoldedToOpen", opened: "pianoRoll", folded: "mixer" }, "ピアノロールをひらくために、ミキサーをたたみました。窓を高くすると、両方ひらけます。"],
  ];
  it.each(cases)("%o", (message, expected) => {
    show(StatusLine, app.stateWith((s) => (s.status.message = message)));
    expect(text(q("footer.status .message"))).toBe(expected);
  });
});

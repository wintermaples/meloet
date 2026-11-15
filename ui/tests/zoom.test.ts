// 状態行の「拡大率」と、その一覧（仕様書 2 章・3 章、モック F-Zoom）。状態行の右がわの引っこめかた。拡大率と帯の知らせ。
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

// 一覧は body に置かれるので、document から探す。
const q = (sel: string) => document.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...document.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

/** いまは 125 %、150 % が入らない（偽物の場面 zoom と同じ）。 */
const zoomState = (edit: (s: AppState) => void = () => {}) =>
  app.stateWith((s) => {
    s.workspace.scale = 125;
    s.workspace.unfitScales = [150];
    edit(s);
  });

function openMenu(): void {
  q('[data-test="status-scale"]')!.click();
  flushSync();
}

const rows = () => qa('[data-test="zoom-menu"] [role="menuitemradio"]');

describe("状態行の「拡大率」のボタン", () => {
  it("いまの率と ▾。いちばん右にあり、押すと一覧がひらく", () => {
    show(StatusLine, zoomState());
    const button = q('[data-test="status-scale"]')!;
    expect(text(button)).toBe("125 %▾");
    expect(q("footer.status")!.lastElementChild).toBe(button);
    expect(button.getAttribute("aria-expanded")).toBe("false");
    expect(q('[data-test="zoom-menu"]')).toBeNull();
    openMenu();
    expect(button.getAttribute("aria-expanded")).toBe("true");
    expect(button.classList.contains("open")).toBe(true);
    expect(q('[data-test="zoom-menu"]')).not.toBeNull();
  });

  it("もう一度押すと、とじる", () => {
    show(StatusLine, zoomState());
    openMenu();
    openMenu();
    expect(q('[data-test="zoom-menu"]')).toBeNull();
  });
});

describe("拡大率の一覧", () => {
  it("見出しと 3 行。いまの率が「えらんだ」、入らない率は使えない（押せない）で「この画面には入りません」", () => {
    show(StatusLine, zoomState());
    openMenu();
    expect(text(q('[data-test="zoom-menu"] .head'))).toBe("拡大率");
    expect(rows().map((r) => r.querySelector(".rate")!.textContent)).toEqual(["100 %", "125 %", "150 %"]);
    expect(rows().map((r) => r.getAttribute("aria-checked"))).toEqual(["false", "true", "false"]);
    expect(rows().map((r) => r.querySelector(".choice")!.classList.contains("selected"))).toEqual([false, true, false]);
    expect(rows().map((r) => (r as HTMLButtonElement).disabled)).toEqual([false, false, true]);
    expect(rows().map((r) => text(r.querySelector('[data-test="zoom-unfit"]')))).toEqual([null, null, "この画面には入りません"]);
  });

  it("入らない率がなければ、どの行も押せる", () => {
    show(StatusLine, app.stateWith((s) => (s.workspace.unfitScales = [])));
    openMenu();
    expect(rows().map((r) => (r as HTMLButtonElement).disabled)).toEqual([false, false, false]);
    expect(qa('[data-test="zoom-unfit"]')).toEqual([]);
  });

  it("キーの書きかた: Linux と Windows では「Ctrl Alt 1」、Mac では「Cmd Option 1」（design-notes A-2）", () => {
    app.setState(zoomState());
    mounted = mount(ZoomMenu, { target, props: { scale: 125, unfit: [150], platform: "other", onpick: () => {} } });
    flushSync();
    expect(qa('[data-test="zoom-key"]').map(text)).toEqual(["Ctrl Alt 1", "Ctrl Alt 2", "Ctrl Alt 3"]);
    unmount(mounted);
    mounted = mount(ZoomMenu, { target, props: { scale: 125, unfit: [150], platform: "mac", onpick: () => {} } });
    flushSync();
    expect(qa('[data-test="zoom-key"]').map(text)).toEqual(["Cmd Option 1", "Cmd Option 2", "Cmd Option 3"]);
  });

  it("行を押すと setScale を送り、一覧をとじる", () => {
    show(StatusLine, zoomState());
    openMenu();
    rows()[0]!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "setScale", percent: 100 }]);
    expect(q('[data-test="zoom-menu"]')).toBeNull();
    expect(menuVisible()).toBe(false);
  });

  it("使えない行は押せない（何も送らず、ひらいたまま）", () => {
    show(StatusLine, zoomState());
    openMenu();
    rows()[2]!.click();
    flushSync();
    expect(app.sent).toEqual([]);
    expect(q('[data-test="zoom-menu"]')).not.toBeNull();
  });

  it("ひらくと、いまの率の行にフォーカス。↑ ↓ は使えない行をとばし、Enter で決める", async () => {
    show(StatusLine, zoomState((s) => ((s.workspace.scale = 100), (s.workspace.unfitScales = [125]))));
    openMenu();
    await Promise.resolve();
    expect(document.activeElement).toBe(rows()[0]);
    const key = (k: string) => {
      document.activeElement!.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true }));
      flushSync();
    };
    key("ArrowDown");
    expect(document.activeElement).toBe(rows()[2]);
    key("ArrowDown");
    expect(document.activeElement).toBe(rows()[2]);
    key("ArrowUp");
    expect(document.activeElement).toBe(rows()[0]);
    key("ArrowDown");
    key("Enter");
    expect(app.sent).toEqual([{ type: "setScale", percent: 150 }]);
    expect(q('[data-test="zoom-menu"]')).toBeNull();
    expect(document.activeElement).toBe(q('[data-test="status-scale"]'));
  });

  it("Esc は、メニューをダイアログより先にとじる（core の back は送らない）", () => {
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

  it("Esc の順: 文字を打っている枠 → メニュー → core の back（仕様書 3 章）", () => {
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

  it("一覧は body に置き、ダイアログ（30）より手前に出す", () => {
    show(StatusLine, zoomState());
    openMenu();
    const popup = q('[data-test="zoom-menu"]')!.parentElement!;
    expect(popup.parentElement).toBe(document.body);
    expect(target.contains(popup)).toBe(false);
  });

  it("外を押すと、とじる。一覧の中を押してもとじない", () => {
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

describe("状態行の右がわの引っこめかた（仕様書 3 章）", () => {
  const right = { output: 160, inputs: 140, buffer: 80 };
  const base = { gap: 16, left: 300, right, scale: 60 };
  // 全部: 300 + 2 × 16 + 60 + (160 + 16) + (140 + 16) + (80 + 16) = 820
  const all = 820;

  it("入る: 全部出す", () => {
    expect(statusFit({ ...base, available: all })).toEqual({ inputs: true, output: true, buffer: true });
  });

  it("入口 → 出口 → バッファ の順に引っこめる", () => {
    expect(statusFit({ ...base, available: all - 1 })).toEqual({ inputs: false, output: true, buffer: true });
    expect(statusFit({ ...base, available: all - 156 })).toEqual({ inputs: false, output: true, buffer: true });
    expect(statusFit({ ...base, available: all - 157 })).toEqual({ inputs: false, output: false, buffer: true });
    expect(statusFit({ ...base, available: all - 332 - 1 })).toEqual({ inputs: false, output: false, buffer: false });
    expect(statusFit({ ...base, available: 10 })).toEqual({ inputs: false, output: false, buffer: false });
  });

  it("バッファが出ていない（音が動いていない）ときは、入口 → 出口", () => {
    const noBuffer = { ...base, right: { output: 160, inputs: 140 } };
    expect(statusFit({ ...noBuffer, available: all - 96 })).toEqual({ inputs: true, output: true, buffer: true });
    expect(statusFit({ ...noBuffer, available: all - 97 })).toEqual({ inputs: false, output: true, buffer: true });
  });

  describe("画面で（幅を測る所を差しかえる）", () => {
    const widths: Record<string, number> = { left: 300, output: 160, inputs: 140, buffer: 80, scale: 60 };
    let rowWidth = 0;
    beforeEach(() => {
      vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function (this: HTMLElement) {
        return widths[this.dataset.part ?? ""] ?? 0;
      });
      vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockImplementation(function (this: HTMLElement) {
        return this.matches("footer.status") ? rowWidth : 0;
      });
      // jsdom は CSS の変数を解かないので、すきまはここで決める。
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
          s.keyboards = { list: [{ id: "kb", name: "鍵盤 49", portName: "p", address: "24:0" }], active: "kb", lost: null };
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

    it("広い: 全部", () => {
      expect(status(all)).toEqual({ output: true, inputs: true, buffer: true, scale: true });
    });
    it("少し足りない: 入口から", () => {
      expect(status(all - 1)).toEqual({ output: true, inputs: false, buffer: true, scale: true });
    });
    it("もっと足りない: 次に出口", () => {
      expect(status(all - 157)).toEqual({ output: false, inputs: false, buffer: true, scale: true });
    });
    it("それでも足りない: バッファも。拡大率は残り、左の文は「…」でちぢむ（CSS の min-width: 0）", () => {
      expect(status(200)).toEqual({ output: false, inputs: false, buffer: false, scale: true });
    });
  });
});

describe("拡大率と帯の知らせ（仕様書 6 章）", () => {
  const cases: [StatusMessage, string][] = [
    [{ id: "scaleDoesNotFit", requested: 150, kept: 125 }, "150 % は、この画面には入りません。125 % のままです。"],
    [{ id: "scaleWindowGrown", scale: 125 }, "拡大率を 125 % にしました。窓を、いちばん小さい大きさまで広げました。"],
    [{ id: "scaleLoweredForScreen", from: 150, to: 125 }, "画面が小さくなったので、拡大率を 150 % から 125 % にしました。"],
    [{ id: "bandFoldedWindowLow", band: "mixer" }, "窓が低くなったので、ミキサーをたたみました。帯札を押すと、またひらきます。"],
    [{ id: "bandFoldedWindowLow", band: "history" }, "窓が低くなったので、取り消しの履歴をたたみました。帯札を押すと、またひらきます。"],
    [{ id: "bandFoldedToOpen", opened: "pianoRoll", folded: "mixer" }, "ピアノロールをひらくために、ミキサーをたたみました。窓を高くすると、両方ひらけます。"],
  ];
  it.each(cases)("%o", (message, expected) => {
    show(StatusLine, app.stateWith((s) => (s.status.message = message)));
    expect(text(q("footer.status .message"))).toBe(expected);
  });
});

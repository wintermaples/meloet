// The scan band, the dialog 「スキャンで止まったプラグイン」, plugin locations, status line text, moving dialogs, the Esc order.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import AboutDialog from "../src/components/about/AboutDialog.svelte";
import { about, closeAbout, openAbout } from "../src/components/about/about.svelte";
import PluginPlaces from "../src/components/plugins/PluginPlaces.svelte";
import { placesReveal, scanStripText } from "../src/components/plugins/scan";
import ScanStoppedDialog from "../src/components/plugins/ScanStoppedDialog.svelte";
import SettingsDialog from "../src/components/SettingsDialog.svelte";
import Shelf from "../src/components/Shelf.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { shelfHasKeys, shelfUi } from "../src/components/tracks.svelte";
import { handleKeydown } from "../src/keys/attach";
import { t } from "../src/text";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state;

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(1920);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  document.body.innerHTML = "";
  closeAbout();
  placesReveal.pending = false;
  Object.assign(shelfUi, { query: "", favoritesOnly: false, folded: [], keys: false, view: "instruments" });
  vi.restoreAllMocks();
});

function show(component: Component<any>, state: AppState, props: Record<string, unknown> = {}): void {
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}
const q = (sel: string) => document.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...document.querySelectorAll<HTMLElement>(sel)];
const words = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const click = (el: Element | null) => {
  (el as HTMLElement).click();
  flushSync();
};

describe("scan band below the shelf (R9-Scan)", () => {
  it("splits the chapter 6 text into the mock's 3 places", () => {
    expect(scanStripText(37, 112)).toEqual({ label: "スキャン中", count: "37 / 112", note: "調べ終えた分から、もう使えます", done: 37 / 112 });
    expect(scanStripText(0, 0).done).toBe(0);
    expect(scanStripText(5, 3).done).toBe(1);
  });

  it("shown at the bottom of the shelf only while scanning. The progress bar is done / total", () => {
    show(Shelf, scene("scanning"));
    const strip = q('[data-test="scan-strip"]')!;
    expect(words(strip)).toBe("スキャン中 37 / 112 調べ終えた分から、もう使えます");
    expect(strip.parentElement!.lastElementChild).toBe(strip);
    expect(parseFloat(q('[data-test="scan-strip"] .fill')!.style.width)).toBeCloseTo((37 / 112) * 100, 3);
  });

  it("when done, the band is not shown", () => {
    show(Shelf, scene("scanFinished"));
    expect(q('[data-test="scan-strip"]')).toBeNull();
  });
});

describe("status line text (spec 6)", () => {
  const status = () => words(q(".status .message"));
  const link = () => q(".status .link");

  it("scan done with stopped plugins: 「止まった 4 個を見る」 opens the list", () => {
    show(StatusLine, scene("scanFinished"));
    expect(status()).toBe(
      "スキャンが終わりました ・ 667 個を棚に入れました。4 個は調べている途中で止まったので、入れていません。meloet と曲は無事です。",
    );
    expect(link()!.textContent).toBe("止まった 4 個を見る");
    click(link());
    expect(app.sent).toEqual([{ type: "openScanStopped" }]);
  });

  it("scan done with 0 stopped: only the first half. No button", () => {
    show(StatusLine, scene("scanFinishedNone"));
    expect(status()).toBe("スキャンが終わりました ・ 667 個を棚に入れました。");
    expect(link()).toBeNull();
  });

  it.each([
    ["pluginsFound", "新しいプラグインが 3 個見つかりました。棚の「あとから入れたもの」に入れました。"],
    ["scanRetryAdded", "Granular Fog を、もう一度ためしました。今度は入りました。"],
    ["scanRetryStopped", "Granular Fog は、また止まりました。棚には入れません。"],
    ["scanStopped", "スキャンで止まったプラグイン をひらいています"],
    [
      "pluginStopped",
      "Surge XT が止まりました ・ 1 ピアノ。曲のほかの部分は動いています。音はこの枠をとばして流しています。パラメータの値は残っています。",
    ],
  ])("%s", (name, text) => {
    show(StatusLine, scene(name));
    expect(status()).toBe(text);
  });

  it("when an insert plugin stops, the name of that strip", () => {
    const s = scene("pluginStopped");
    s.status.message = { id: "pluginStopped", name: "Dragonfly", slot: { kind: "insert", insert: { strip: { kind: "master" }, slot: 0 } } };
    show(StatusLine, s);
    expect(status()).toContain(`Dragonfly が止まりました ・ ${t("mixer.master")}。`);
  });

  it("while a dialog is open (85): sound settings, about meloet", () => {
    show(StatusLine, scene("settings"));
    expect(status()).toBe("音の設定をひらいています");
    unmount(mounted!);
    openAbout();
    show(StatusLine, scene("scanFinished"));
    expect(status()).toBe("meloet について をひらいています");
    expect(link()).toBeNull();
    closeAbout();
    flushSync();
    expect(status()).toContain("スキャンが終わりました");
  });
});

describe("dialog 「スキャンで止まったプラグイン」 (F5-ScanStopped)", () => {
  const rows = () => qa('[data-test="scan-stopped-row"]');
  const retries = () => qa('[data-test="scan-stopped-retry"]');

  it("heading, top text, rows (name, format mark, reason, try again, location below), the 2 buttons at the bottom", () => {
    show(ScanStoppedDialog, scene("scanStopped"));
    expect(words(q("[data-dialog-head]"))).toBe("スキャンで止まったプラグイン 閉じるEsc");
    expect(words(q(".intro"))).toBe(
      "この 4 個は、調べている途中で止まったので、棚に入れていません。meloet と曲は無事です。次のスキャンからは、とばします。",
    );
    expect(rows().map(words)).toEqual([
      "Old Chorus VST3 読みこむと止まりました もう一度ためす ~/.vst3/OldChorus.vst3",
      "Granular Fog LV2 30 秒たっても終わりませんでした もう一度ためす /usr/lib/lv2/granfog.lv2",
      "Tape Wobble LV2 作りがこわれています もう一度ためす ~/.lv2/TapeWobble.lv2",
      "Brick EQ 1.0 VST3 読みこむと止まりました もう一度ためす /usr/lib/vst3/BrickEQ.vst3",
    ]);
    expect(qa(".foot button").map(words)).toEqual(["プラグインの場所…", "閉じる"]);
  });

  it("try again: sends the row's location and id. While trying, 「ためしています…」 and nothing more is sent", () => {
    show(ScanStoppedDialog, scene("scanStoppedRetrying"));
    expect(retries().map(words)).toEqual(["もう一度ためす", "ためしています…", "もう一度ためす", "もう一度ためす"]);
    click(retries()[1]!);
    click(retries()[2]!);
    expect(app.sent).toEqual([{ type: "retryStoppedPlugin", path: "/home/yachiyo/.lv2/TapeWobble.lv2", id: null }]);
  });

  it("a row that got in goes away (the protocol stops sending it)", () => {
    show(ScanStoppedDialog, scene("scanRetryAdded"));
    expect(rows().map((r) => r.querySelector(".name")!.textContent)).toEqual(["Old Chorus", "Tape Wobble", "Brick EQ 1.0"]);
    expect(words(q(".intro"))).toContain("この 3 個は");
  });

  it("close (heading and bottom row), plugin locations…", () => {
    show(ScanStoppedDialog, scene("scanStopped"));
    click(q('[data-test="scan-stopped-close-head"]'));
    click(q('[data-test="scan-stopped-close"]'));
    click(q('[data-test="scan-stopped-places"]'));
    expect(app.sent).toEqual([{ type: "closeScanStopped" }, { type: "closeScanStopped" }, { type: "openPluginPlaces" }]);
    expect(placesReveal.pending).toBe(true);
  });

  it("with many rows, the heading and bottom row stay and only the middle scrolls", () => {
    show(ScanStoppedDialog, scene("scanStoppedMany"));
    const body = q('[data-test="scan-stopped-body"]')!;
    expect(body.contains(q(".intro"))).toBe(true);
    expect(rows()).toHaveLength(20);
    expect(rows().every((r) => body.contains(r))).toBe(true);
    expect(body.contains(q("[data-dialog-head]"))).toBe(false);
    expect(body.contains(q(".foot"))).toBe(false);
  });

  it("not shown when not open", () => {
    show(ScanStoppedDialog, scene("scanFinished"));
    expect(q('[data-test="scan-stopped-dialog"]')).toBeNull();
  });
});

describe("sound settings ▸ advanced settings ▸ plugin locations (R9-Scan)", () => {
  const rows = () => qa('[data-test="plugin-place"]');

  it("shown inside the advanced settings. The heading has ▾ and a count. Default places say 「ふつうの場所」, only added places have 「外す」", () => {
    show(SettingsDialog, scene("pluginPlaces"));
    expect(words(q('[data-test="plugin-places-head"]'))).toBe("▾ プラグインの場所 8");
    expect(rows().map(words)).toEqual([
      "~/.lv2 ふつうの場所",
      "/usr/lib/lv2 ふつうの場所",
      "/usr/local/lib/lv2 ふつうの場所",
      "~/.vst3 ふつうの場所",
      "/usr/lib/vst3 ふつうの場所",
      "/usr/local/lib/vst3 ふつうの場所",
      "~/音楽/プラグイン 外す",
      "/opt/surge-xt/lib/vst3 外す",
    ]);
    expect(rows().map((r) => r.classList.contains("added"))).toEqual([false, false, false, false, false, false, true, true]);
    expect(qa('[data-test="plugin-place-remove"]')).toHaveLength(2);
    expect(rows().some((r) => r.textContent!.includes("clap"))).toBe(false);
  });

  it("remove, add a place…, scan again, fold by the heading", () => {
    show(SettingsDialog, scene("pluginPlaces"));
    click(qa('[data-test="plugin-place-remove"]')[1]!);
    click(q('[data-test="plugin-place-add"]'));
    click(q('[data-test="plugin-rescan"]'));
    click(q('[data-test="plugin-places-head"]'));
    expect(app.sent).toEqual([
      { type: "removePluginPlace", path: "/opt/surge-xt/lib/vst3" },
      { type: "addPluginPlace" },
      { type: "rescanPlugins" },
      { type: "togglePluginPlaces" },
    ]);
    expect(words(q('[data-test="plugin-rescan"]'))).toBe("もう一度スキャン");
    expect(words(q('[data-test="plugin-place-add"]'))).toBe("場所を足す…");
  });

  it("when folded, only 「ひらく…」 on the 「プラグインの場所」 row (mock F7-SettingsAdv). Pressing opens it", () => {
    show(SettingsDialog, scene("pluginPlacesFolded"));
    expect(q('[data-test="plugin-places-head"]')).toBeNull();
    expect(rows()).toHaveLength(0);
    expect(q('[data-test="plugin-rescan"]')).toBeNull();
    click(q('[data-test="settings-places"]'));
    expect(app.sent).toEqual([{ type: "togglePluginPlaces" }]);
  });

  it("not shown when the advanced settings are not open", () => {
    show(SettingsDialog, scene("settings"));
    expect(q('[data-test="plugin-places"]')).toBeNull();
  });

  it("scrolled into view only when opened from the stopped-plugins dialog", () => {
    const into = vi.fn();
    HTMLElement.prototype.scrollIntoView = into;
    vi.spyOn(window, "requestAnimationFrame").mockImplementation((f) => (f(0), 0));
    show(PluginPlaces, scene("pluginPlaces"), { plugins: scene("pluginPlaces").plugins });
    expect(into).not.toHaveBeenCalled();
    unmount(mounted!);
    placesReveal.pending = true;
    show(PluginPlaces, scene("pluginPlaces"), { plugins: scene("pluginPlaces").plugins });
    expect(into).toHaveBeenCalledWith({ block: "nearest" });
    expect(placesReveal.pending).toBe(false);
  });
});

describe("moving dialogs (82): about meloet", () => {
  it("moves by the heading row, never outside the window, back to the middle when opened again. Esc while dragging only puts it back", () => {
    vi.spyOn(window, "innerWidth", "get").mockReturnValue(1024);
    vi.spyOn(window, "innerHeight", "get").mockReturnValue(768);
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      const m = /translate\((-?[\d.]+)px, (-?[\d.]+)px\)/.exec(this.style.transform);
      return DOMRect.fromRect({ x: 232 + (m ? Number(m[1]) : 0), y: 200 + (m ? Number(m[2]) : 0), width: 560, height: 300 });
    });
    const p = (type: string, x: number, y: number) =>
      q("[data-dialog-head]")!.dispatchEvent(new PointerEvent(type, { clientX: x, clientY: y, button: 0, pointerId: 1, bubbles: true }));
    openAbout();
    show(AboutDialog, scene("saved"));
    const dialog = () => q('[role="dialog"]')!;
    p("pointerdown", 300, 210);
    p("pointermove", 5000, 5000);
    p("pointerup", 5000, 5000);
    expect(dialog().style.transform).toBe(`translate(${1024 - 232 - 560}px, ${768 - 200 - 300}px)`);

    p("pointerdown", 300, 210);
    p("pointermove", 200, 110);
    const sent: Command[] = [];
    const s = scene("saved");
    const esc = () =>
      handleKeydown(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }), {
        state: () => s,
        send: (c) => void sent.push(c),
        nav: { platform: "Linux x86_64" },
      });
    esc();
    expect(dialog().style.transform).toBe(`translate(${1024 - 232 - 560}px, ${768 - 200 - 300}px)`);
    expect(about.open).toBe(true);
    p("pointerup", 200, 110);
    esc();
    expect(about.open).toBe(false);
    expect(sent).toEqual([]);
    flushSync();

    openAbout();
    flushSync();
    expect(dialog().style.transform).toBe("");
  });
});

it("every dialog's heading row has the move pointer, and bg gray.4 on hover", () => {
  const sources = import.meta.glob<string>(["../src/components/**/*Dialog.svelte"], { query: "?raw", import: "default", eager: true });
  const files = Object.keys(sources).sort();
  expect(files).toEqual([
    "../src/components/LatencyDialog.svelte",
    "../src/components/SettingsDialog.svelte",
    "../src/components/SongDialog.svelte",
    "../src/components/about/AboutDialog.svelte",
    "../src/components/export/ExportDialog.svelte",
    "../src/components/export/ExportedDialog.svelte",
    "../src/components/keylist/KeyListDialog.svelte",
    "../src/components/plugins/ScanStoppedDialog.svelte",
  ]);
  for (const f of files) {
    const css = sources[f]!;
    expect(css, f).toContain("data-dialog-head");
    expect(css, f).toContain("use:movableDialog");
    expect(/\.head \{[^}]*cursor: move;[^}]*\}/.exec(css), f).not.toBeNull();
    expect(/\.head:hover \{\s*background: var\(--color-gray-4\);\s*\}/.exec(css), f).not.toBeNull();
  }
  // Every dialog is in the vertical middle of the window (boxes that grow the same above and below).
  for (const f of files) {
    const css = sources[f]!;
    expect(css.match(/<div class="lead"><\/div>/g), f).toHaveLength(2);
    expect(/\.lead \{\s*flex: 1 1 0;\s*\}/.exec(css), f).not.toBeNull();
  }
});

describe("Esc order (spec 3): box being typed in -> term tip or menu -> dialog -> shelf -> lower band", () => {
  function esc(s: AppState, input?: HTMLElement): Command[] {
    const sent: Command[] = [];
    handleKeydown(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }), {
      state: () => s,
      send: (c) => void sent.push(c),
      nav: { platform: "Linux x86_64" },
      doc: input ? ({ activeElement: input } as unknown as Document) : undefined,
    });
    return sent;
  }

  it("the stopped-plugins dialog closes before a shelf in try-out (back)", () => {
    const s = scene("shelfAudition");
    expect(esc(s)).toEqual([{ type: "shelfCancel" }]);
    s.plugins.stoppedOpen = true;
    expect(esc(s)).toEqual([{ type: "back" }]);
  });

  it("Enter in the stopped-plugins dialog is 「閉じる」 at the right end", () => {
    const sent: Command[] = [];
    const s = scene("scanStopped");
    s.workspace.settings = { draftOutput: null, advancedOpen: false };
    const ev = new KeyboardEvent("keydown", { key: "Enter", cancelable: true });
    handleKeydown(ev, { state: () => s, send: (c) => void sent.push(c), nav: { platform: "Linux x86_64" } });
    expect(sent).toEqual([{ type: "closeScanStopped" }]);
    expect(ev.defaultPrevented).toBe(true);
  });

  it("the box being typed in goes first (only leaves the box)", () => {
    const s = scene("scanStopped");
    const input = document.createElement("input");
    document.body.append(input);
    input.focus();
    expect(esc(s, input)).toEqual([]);
    expect(document.activeElement).not.toBe(input);
  });

  it("about meloet (a screen-only dialog) goes before core dialogs", () => {
    const s = scene("scanStopped");
    openAbout();
    expect(esc(s)).toEqual([]);
    expect(about.open).toBe(false);
    expect(esc(s)).toEqual([{ type: "back" }]);
  });

  it("while the stopped-plugins dialog is open, shelf keys do not work and 「meloet について」 does not open", () => {
    const s = scene("shelfAudition");
    shelfUi.keys = true;
    expect(shelfHasKeys(s)).toBe(true);
    s.plugins.stoppedOpen = true;
    expect(shelfHasKeys(s)).toBe(false);
  });

  it("fake: back closes the stopped-plugins dialog -> sound settings -> band, in that order", async () => {
    const { createFakeTransport } = await import("../src/bridge/fake");
    const start = scene("scanStopped");
    start.workspace.settings = { draftOutput: null, advancedOpen: true };
    const transport = createFakeTransport(structuredClone(start));
    let held = structuredClone(start);
    await transport.subscribe(
      (m) => {
        if (m.kind !== "patch") return;
        held = { ...held };
        for (const c of m.changes) (held as Record<string, unknown>)[c.section] = c.value;
      },
      () => {},
    );
    await transport.send({ type: "back" });
    expect([held.plugins.stoppedOpen, held.workspace.settings !== null]).toEqual([false, true]);
    await transport.send({ type: "back" });
    expect(held.workspace.settings).toBeNull();
  });
});

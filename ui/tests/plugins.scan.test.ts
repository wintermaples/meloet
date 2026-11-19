// 第 5 回 区切り1: スキャンの帯、ダイアログ「スキャンで止まったプラグイン」、プラグインの場所、状態行の文、ダイアログを動かす、Esc の順。
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
import { t } from "../src/text/ja";
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

describe("棚の下のスキャンの帯（R9-Scan）", () => {
  it("6 章の文を、モックの 3 つの所に分ける", () => {
    expect(scanStripText(37, 112)).toEqual({ label: "スキャン中", count: "37 / 112", note: "調べ終えた分から、もう使えます", done: 37 / 112 });
    expect(scanStripText(0, 0).done).toBe(0);
    expect(scanStripText(5, 3).done).toBe(1);
  });

  it("スキャン中だけ、棚のいちばん下に出る。進みの棒は done / total", () => {
    show(Shelf, scene("scanning"));
    const strip = q('[data-test="scan-strip"]')!;
    expect(words(strip)).toBe("スキャン中 37 / 112 調べ終えた分から、もう使えます");
    expect(strip.parentElement!.lastElementChild).toBe(strip);
    expect(parseFloat(q('[data-test="scan-strip"] .fill')!.style.width)).toBeCloseTo((37 / 112) * 100, 3);
  });

  it("終わったら、帯は出さない", () => {
    show(Shelf, scene("scanFinished"));
    expect(q('[data-test="scan-strip"]')).toBeNull();
  });
});

describe("状態行の文（6 章）", () => {
  const status = () => words(q(".status .message"));
  const link = () => q(".status .link");

  it("スキャンが終わった ・ 止まった物があれば「止まった 4 個を見る」で一覧をひらく", () => {
    show(StatusLine, scene("scanFinished"));
    expect(status()).toBe(
      "スキャンが終わりました ・ 667 個を棚に入れました。4 個は調べている途中で止まったので、入れていません。meloet と曲は無事です。",
    );
    expect(link()!.textContent).toBe("止まった 4 個を見る");
    click(link());
    expect(app.sent).toEqual([{ type: "openScanStopped" }]);
  });

  it("スキャンが終わった ・ 止まった物が 0 なら前半だけ。ボタンはない", () => {
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

  it("インサートのプラグインが止まったときは、そのストリップの名前", () => {
    const s = scene("pluginStopped");
    s.status.message = { id: "pluginStopped", name: "Dragonfly", slot: { kind: "insert", insert: { strip: { kind: "master" }, slot: 0 } } };
    show(StatusLine, s);
    expect(status()).toContain(`Dragonfly が止まりました ・ ${t("mixer.master")}。`);
  });

  it("ダイアログをひらいている間（85）: 音の設定、meloet について", () => {
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

describe("ダイアログ「スキャンで止まったプラグイン」（F5-ScanStopped）", () => {
  const rows = () => qa('[data-test="scan-stopped-row"]');
  const retries = () => qa('[data-test="scan-stopped-retry"]');

  it("見出し、上の文、行（名前 ・ 形式の印 ・ わけ ・ もう一度ためす、下に場所）、下の 2 つのボタン", () => {
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

  it("もう一度ためす: 行の場所と id を送る。ためしている間は「ためしています…」で、もう送らない", () => {
    show(ScanStoppedDialog, scene("scanStoppedRetrying"));
    expect(retries().map(words)).toEqual(["もう一度ためす", "ためしています…", "もう一度ためす", "もう一度ためす"]);
    click(retries()[1]!);
    click(retries()[2]!);
    expect(app.sent).toEqual([{ type: "retryStoppedPlugin", path: "/home/yachiyo/.lv2/TapeWobble.lv2", id: null }]);
  });

  it("入った行は消える（約束が送ってこない）", () => {
    show(ScanStoppedDialog, scene("scanRetryAdded"));
    expect(rows().map((r) => r.querySelector(".name")!.textContent)).toEqual(["Old Chorus", "Tape Wobble", "Brick EQ 1.0"]);
    expect(words(q(".intro"))).toContain("この 3 個は");
  });

  it("閉じる（見出しと下の列）、プラグインの場所…", () => {
    show(ScanStoppedDialog, scene("scanStopped"));
    click(q('[data-test="scan-stopped-close-head"]'));
    click(q('[data-test="scan-stopped-close"]'));
    click(q('[data-test="scan-stopped-places"]'));
    expect(app.sent).toEqual([{ type: "closeScanStopped" }, { type: "closeScanStopped" }, { type: "openPluginPlaces" }]);
    expect(placesReveal.pending).toBe(true);
  });

  it("行が多ければ、見出しと下の列は動かさず、中身だけスクロールする", () => {
    show(ScanStoppedDialog, scene("scanStoppedMany"));
    const body = q('[data-test="scan-stopped-body"]')!;
    expect(body.contains(q(".intro"))).toBe(true);
    expect(rows()).toHaveLength(20);
    expect(rows().every((r) => body.contains(r))).toBe(true);
    expect(body.contains(q("[data-dialog-head]"))).toBe(false);
    expect(body.contains(q(".foot"))).toBe(false);
  });

  it("ひらいていなければ出さない", () => {
    show(ScanStoppedDialog, scene("scanFinished"));
    expect(q('[data-test="scan-stopped-dialog"]')).toBeNull();
  });
});

describe("音の設定 ▸ くわしい設定 ▸ プラグインの場所（R9-Scan）", () => {
  const rows = () => qa('[data-test="plugin-place"]');

  it("くわしい設定の中に出る。見出しは ▾ と数。ふつうの場所は「ふつうの場所」、足した場所だけ「外す」", () => {
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

  it("外す ・ 場所を足す… ・ もう一度スキャン ・ 見出しでたたむ", () => {
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

  it("たたんでいると、見出し（▸ と数）だけ", () => {
    show(SettingsDialog, scene("pluginPlacesFolded"));
    expect(words(q('[data-test="plugin-places-head"]'))).toBe("▸ プラグインの場所 7");
    expect(rows()).toHaveLength(0);
    expect(q('[data-test="plugin-rescan"]')).toBeNull();
  });

  it("くわしい設定をひらいていなければ出さない", () => {
    show(SettingsDialog, scene("settings"));
    expect(q('[data-test="plugin-places"]')).toBeNull();
  });

  it("止まった物のダイアログからひらいたときだけ、見える所へ送る", () => {
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

describe("ダイアログを動かす（82）: meloet について", () => {
  it("見出しの行で動き、窓の外へは出ず、ひらきなおすとまん中。ドラッグ中の Esc は動かす前へ戻すだけ", () => {
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

it("どのダイアログも、見出しの行は「動かす」の指で、乗ると地 gray.4", () => {
  const sources = import.meta.glob<string>(["../src/components/**/*Dialog.svelte"], { query: "?raw", import: "default", eager: true });
  const files = Object.keys(sources).sort();
  expect(files).toEqual([
    "../src/components/SettingsDialog.svelte",
    "../src/components/SongDialog.svelte",
    "../src/components/about/AboutDialog.svelte",
    "../src/components/plugins/ScanStoppedDialog.svelte",
  ]);
  for (const f of files) {
    const css = sources[f]!;
    expect(css, f).toContain("data-dialog-head");
    expect(css, f).toContain("use:movableDialog");
    expect(/\.head \{[^}]*cursor: move;[^}]*\}/.exec(css), f).not.toBeNull();
    expect(/\.head:hover \{\s*background: var\(--color-gray-4\);\s*\}/.exec(css), f).not.toBeNull();
  }
  // どのダイアログも、窓の上下のまん中（上と下に同じのびる箱）。
  for (const f of files) {
    const css = sources[f]!;
    expect(css.match(/<div class="lead"><\/div>/g), f).toHaveLength(2);
    expect(/\.lead \{\s*flex: 1 1 0;\s*\}/.exec(css), f).not.toBeNull();
  }
});

describe("Esc の順（3 章）: 文字を打っている枠 → 用語の説明やメニュー → ダイアログ → 棚 → 下の帯", () => {
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

  it("止まった物のダイアログは、ためし弾き中の棚より先に閉じる（back）", () => {
    const s = scene("shelfAudition");
    expect(esc(s)).toEqual([{ type: "shelfCancel" }]);
    s.plugins.stoppedOpen = true;
    expect(esc(s)).toEqual([{ type: "back" }]);
  });

  it("止まった物のダイアログの Enter は、右はしの「閉じる」", () => {
    const sent: Command[] = [];
    const s = scene("scanStopped");
    s.workspace.settings = { draftOutput: null, advancedOpen: false };
    const ev = new KeyboardEvent("keydown", { key: "Enter", cancelable: true });
    handleKeydown(ev, { state: () => s, send: (c) => void sent.push(c), nav: { platform: "Linux x86_64" } });
    expect(sent).toEqual([{ type: "closeScanStopped" }]);
    expect(ev.defaultPrevented).toBe(true);
  });

  it("文字を打っている枠が先（枠から出るだけ）", () => {
    const s = scene("scanStopped");
    const input = document.createElement("input");
    document.body.append(input);
    input.focus();
    expect(esc(s, input)).toEqual([]);
    expect(document.activeElement).not.toBe(input);
  });

  it("meloet について（画面だけのダイアログ）は、core のダイアログより先", () => {
    const s = scene("scanStopped");
    openAbout();
    expect(esc(s)).toEqual([]);
    expect(about.open).toBe(false);
    expect(esc(s)).toEqual([{ type: "back" }]);
  });

  it("止まった物のダイアログをひらいている間は、棚のキーは効かず、「meloet について」もひらかない", () => {
    const s = scene("shelfAudition");
    shelfUi.keys = true;
    expect(shelfHasKeys(s)).toBe(true);
    s.plugins.stoppedOpen = true;
    expect(shelfHasKeys(s)).toBe(false);
  });

  it("偽物: back で止まった物のダイアログ → 音の設定 → 帯の順に閉じる", async () => {
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

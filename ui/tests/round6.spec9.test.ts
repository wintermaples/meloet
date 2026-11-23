// 88 to 91 of design version 9: new status line text, dialog names in song dialogs, a single window, the 「サイン波で代わり」 text, right-click on the instrument name.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, StatusMessage } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import { contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import MixerStrips from "../src/components/mixer/MixerStrips.svelte";
import { standInTip } from "../src/components/sounds";
import StatusLine from "../src/components/StatusLine.svelte";
import TrackHeads from "../src/components/TrackHeads.svelte";
import { closeMenu } from "../src/lib/menu";
import { t } from "../src/text";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string): AppState => fakeScene(name)!.state;
const text = (el: Element | null | undefined) => (el?.textContent ?? "").replace(/\s+/g, " ").trim();

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  contextMenu.open = null;
  closeMenu();
});

function show(state: AppState, component: Component<any>): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];

describe("status line (spec 6 text of version 9)", () => {
  const says = (message: StatusMessage, base = scene("tracks6")) => {
    base.status.message = message;
    show(base, StatusLine);
    const out = text(q(".message"));
    unmount(mounted!);
    mounted = null;
    return out;
  };
  const FLUID = "FluidR3_GM.sf2";

  it("a file that is not an SF2, and many SF2s added (89-8, 89-10)", () => {
    expect(says({ id: "notSf2Dragging" })).toBe("SF2 ではないファイルを持ってきています（足せません）");
    expect(says({ id: "sf2AddedMany", added: 2, first: FLUID, skipped: 1 })).toBe(
      "SF2 を 2 つ足しました（FluidR3_GM.sf2 ほか 1 つ）。1 つは SF2 ではないので、足していません。",
    );
    expect(says({ id: "sf2AddedMany", added: 3, first: FLUID, skipped: 0 })).toBe("SF2 を 3 つ足しました（FluidR3_GM.sf2 ほか 2 つ）。");
    expect(says({ id: "sf2Added", file: FLUID, presets: 128, kits: 9, skipped: 2 })).toBe(
      "FluidR3_GM.sf2 を足しました。音色 128 ・ ドラムのキット 9 ・ 2 つは SF2 ではないので、足していません。",
    );
  });

  it("missing / unreadable plugins (song opened), an instrument plugin removed (89-5, 89-9, 89-12)", () => {
    expect(says({ id: "pluginMissing", name: "Surge XT", tracks: 1 })).toBe("曲で使っている Surge XT が見つかりません。1 本のトラックはサイン波で鳴らしています。");
    expect(says({ id: "pluginUnreadable", name: "Surge XT", tracks: 1 })).toBe(
      "曲で使っている Surge XT を読みこめませんでした。1 本のトラックはサイン波で鳴らしています。",
    );
    expect(says({ id: "instrumentPluginRemoved", track: { number: 5, name: "Surge XT" } })).toBe(
      "5 Surge XT の音源を外しました。サイン波で鳴らします。取り消せます（Ctrl Z）",
    );
  });

  it("plugin windows: one by name, two or more by count. When it could not open (91-1, 91-2)", () => {
    expect(says({ id: "pluginWindowOpen", name: "Surge XT" })).toBe("Surge XT の窓がひらいています");
    expect(says({ id: "pluginWindowsOpen", count: 3 })).toBe("プラグインの窓が 3 つひらいています");
    expect(says({ id: "pluginWindowFailed", name: "Surge XT" })).toBe("Surge XT の窓をひらけませんでした。パラメータの欄では動かせます。");
  });

  it("while the 「窓」 list is open, still by name if there is one window", () => {
    const s = scene("paramMany");
    s.status.message = { id: "summary", loopBars: null, nothingSelected: true, notes: null };
    show(s, StatusLine);
    q("[data-test=status-windows]")!.click();
    flushSync();
    expect(text(q(".message"))).toBe("Surge XT の窓がひらいています");
  });

  it("song dialogs are named by the dialog name (no song name; 89-6)", () => {
    for (const [name, said] of [
      ["unsaved", "「保存していない変更」のダイアログをひらいています"],
      ["cannotSave", "「保存できませんでした」のダイアログをひらいています"],
      ["broken", "「ひらけない曲」のダイアログをひらいています"],
    ] as const) {
      show(scene(name), StatusLine);
      expect(text(q(".message"))).toBe(said);
      unmount(mounted!);
      mounted = null;
    }
  });
});

describe("text when hovering 「サイン波で代わり」 (89-9)", () => {
  it("sound pack, SF2, missing plugin, unreadable plugin", () => {
    const sf2 = { kind: "sf2", file: "FluidR3_GM.sf2", bank: 0, program: 0, name: "Piano", family: "piano" } as const;
    const plugin = { kind: "plugin", key: "vst3:x", name: "Surge XT", format: "vst3", state: "missing" } as const;
    expect(standInTip("packMissing", { kind: "builtin", instrument: "grandPiano" })).toBe("音の素材が見つからないので、サイン波で鳴らしています。");
    expect(standInTip("sf2Missing", sf2)).toBe("FluidR3_GM.sf2 が見つからないので、サイン波で鳴らしています。");
    expect(standInTip("pluginMissing", plugin)).toBe("Surge XT が見つからないので、サイン波で鳴らしています。");
    expect(standInTip("pluginUnreadable", { ...plugin, state: "stopped" })).toBe("Surge XT を読みこめなかったので、サイン波で鳴らしています。");
  });

  it("the instrument slot of an unreadable plugin is not the red stopped border", () => {
    const s = scene("pluginTrack");
    const surge = s.song.tracks[4]!;
    if (surge.sound.kind !== "plugin") throw new Error("the scene's fifth track is Surge XT");
    surge.sound = { ...surge.sound, state: "stopped" };
    surge.standIn = "pluginUnreadable";
    show(s, MixerStrips);
    const frame = q(`[data-strip="track-${surge.id}"] [data-test=mixer-frame]`)!;
    expect(text(frame)).toContain(t("track.standIn"));
    expect(frame.classList.contains("stopped")).toBe(false);
  });
});

describe("right-click on the instrument name gives 「音源を入れかえる…」 (90)", () => {
  it("a list on right-click for built-in sounds too", () => {
    const s = scene("tracks6");
    show(s, TrackHeads);
    qa("[data-test=track-instrument]")[0]!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
    flushSync();
    const rows = contextMenu.open!.entries as MenuRow[];
    expect(rows.map((r) => t(r.label))).toEqual([t("track.replaceSound")]);
    rows[0]!.run();
    expect(app.sent).toEqual([{ type: "openShelf", mode: { kind: "replace", track: s.song.tracks[0]!.id } }]);
  });
});

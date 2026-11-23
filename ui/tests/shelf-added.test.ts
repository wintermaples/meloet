// The shelf's 「あとから入れたもの」 (SF2 and plugins), search, favorites, the try-out box, the notice while a file is brought in,
// the missing-item notice, 「サイン波で代わり」, the stopped frame, the rest of 85 (light on the drum key column, readouts above and below, 「スネア ×2」, held notes).
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import { FLUID, lv2Key, vst3Key } from "../src/bridge/fakeLibrary";
import FileDropNotice from "../src/components/FileDropNotice.svelte";
import InputMeter from "../src/components/InputMeter.svelte";
import { contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import MixerStrips from "../src/components/mixer/MixerStrips.svelte";
import { drumMoveReading } from "../src/components/pianoroll/drag";
import { drumKeysOf, drumName, keyCountsText, keyLabel, selectedKeysText, trackDrums } from "../src/components/pianoroll/drums";
import type { DrumGeom, RollGeom } from "../src/components/pianoroll/geom";
import { paintRollKeys } from "../src/components/pianoroll/paint";
import RightRail from "../src/components/RightRail.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import TrackHeads from "../src/components/TrackHeads.svelte";
import { shelfUi } from "../src/components/tracks.svelte";
import { Batch } from "../src/gl/batch";
import { rgb } from "../src/gl/color";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";
import { t } from "../src/text/ja";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state;
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
  vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] });
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  contextMenu.open = null;
  Object.assign(shelfUi, { query: "", favoritesOnly: false, open: {}, searchOpen: {}, searchFor: "", keys: false, view: "instruments" });
  vi.useRealTimers();
});
function show(component: Component<any>, state: AppState, props: Record<string, unknown> = {}): void {
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const click = (sel: string) => {
  q(sel)!.click();
  flushSync();
};
const headings = () => qa("[data-test=shelf-heading]").map(text);
const rowNamed = (name: string) => qa("[data-test=shelf-row]").find((r) => text(r.querySelector(".name")) === name)!;
function press(state: AppState, key: string): Command[] {
  const sent: Command[] = [];
  const deps: KeyDeps = { state: () => state, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0 };
  handleKeydown(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }), deps);
  return sent;
}

describe("shelf headings (mock F5-SF2Shelf, F5-PluginLoading, spec 3 83, Y12)", () => {
  it("built-in -> added later. Only the SF2 file just added is open; its group and other headings are folded", () => {
    show(RightRail, scene("sf2Shelf"));
    // 「最初から入っているもの」 is a foldable heading like the others (spec 3 89-11). Folded at first.
    expect(q("[data-test='section:builtin']")).toBeNull();
    expect(text(q("[data-test='section:added']"))).toBe("あとから入れたもの ・ 137 SF2 を足す…");
    let h = headings();
    expect(h[0]).toBe("▸ 最初から入っているもの 31");
    expect(h[1]).toBe(`▾ ${FLUID} SF2 137`);
    expect(h.slice(2, 5)).toEqual(["▸ ピアノ 8", "▸ マレット 8", "▸ オルガン 8"]);
    click('[data-group="builtin:sounds"]');
    h = headings();
    expect(h.slice(0, 8)).toEqual(["▾ 最初から入っているもの 31", "▸ ピアノ ・ 鍵盤 11", "▸ ギター 4", "▸ ベース 4", "▸ ドラム 3", "▸ パッド ・ 弦 3", "▸ 管楽器 4", "▸ シンセ 2"]);
    expect(q('[data-group="guitar"]')!.classList.contains("sub")).toBe(true);
    click('[data-group="builtin:sounds"]');
    expect(h.at(-1)).toBe("▸ ドラムのキット 9");
    expect(qa("[data-test=shelf-row]")).toEqual([]);
    // Opening a group shows rows (English names as they are, the GM number on the right. The 「SF2」 format mark is not shown on rows).
    click(`[data-group="ff:guitar:${FLUID}"]`);
    expect(qa("[data-test=shelf-row]").map(text)).toEqual([
      "Nylon String Guitar 25",
      "Steel String Guitar 26",
      "Jazz Guitar 27",
      "Clean Guitar 28",
      "Palm Muted Guitar 29",
      "Overdrive Guitar 30",
      "Distortion Guitar 31",
      "Guitar Harmonics 32",
    ]);
    expect(rowNamed("Clean Guitar").classList.contains("selected")).toBe(true);
    // Instruments whose bank is not 0 show 「・ バンク 128」.
    click(`[data-group="ff:drumKit:${FLUID}"]`);
    expect(text(rowNamed("Standard"))).toBe("Standard 1 ・ バンク 128");
    // Folding by the file heading hides the headings inside too (the heading and count stay).
    click(`[data-group="f:${FLUID}"]`);
    expect(headings().at(-1)).toBe(`▸ ${FLUID} SF2 137`);
  });

  it("plugin type headings (in the 14-A order). A row has a star, ▶, name, maker, the mark of the format used, and 「＋1」", () => {
    show(RightRail, scene("shelfPlugins"));
    // 31 + SF2 137 + plugins 34 (the mock's 182 is from when there were 11 instruments).
    expect(text(q(".head"))).toBe("▾ 棚 音源 202 エフェクト 512 P");
    expect(headings().slice(-5)).toEqual(["▸ シンセ 21", "▸ サンプラー 6", "▸ ドラム 4", "▸ ピアノ ・ 鍵盤 2", "▸ そのほか 1"]);
    click('[data-group="c:instrument:synth"]');
    const surge = rowNamed("Surge XT");
    expect(text(surge)).toBe("Surge XT Surge Synth Team LV2 ＋1");
    expect(surge.querySelector("[data-test=shelf-sample]")).not.toBeNull();
    expect(surge.querySelector(".mark.used")).not.toBeNull();
    expect(text(rowNamed("Helm"))).toBe("Helm Matt Tytel LV2");
    // Effect headings (in the order EQ, dynamics, reverb…).
    click("[data-test=shelf-effects]");
    expect(headings()).toEqual([
      "▸ 最初から入っているもの 6",
      "▸ EQ 64",
      "▸ ダイナミクス 88",
      "▸ リバーブ 23",
      "▸ ディレイ 31",
      "▸ モジュレーション 52",
      "▸ ひずみ 40",
      "▸ フィルター 36",
      "▸ ピッチ 9",
      "▸ 空間 14",
      "▸ 分析 70",
      "▸ そのほか 79",
    ]);
    expect(text(q("[data-test='section:added']"))).toBe("あとから入れたもの ・ 506 もう一度スキャン");
    click("[data-test='section:added'] [data-test=shelf-rescan]");
    expect(app.sent).toContainEqual({ type: "rescanPlugins" });
  });

  it("「＋1」: hover shows 「LV2 ・ VST3 があります。…」, pressing opens a list to choose the format used (UsePluginFormat)", () => {
    show(RightRail, scene("shelfPlugins"));
    click('[data-group="c:instrument:synth"]');
    const more = rowNamed("Surge XT").querySelector<HTMLElement>("[data-test=shelf-formats]")!;
    more.click();
    flushSync();
    const rows = contextMenu.open!.entries as MenuRow[];
    expect(rows.map((r) => [t(r.label), r.checked])).toEqual([
      ["VST3", false],
      ["LV2", true],
    ]);
    // The row is not chosen.
    expect(app.sent).toEqual([]);
    rows[0]!.run();
    expect(app.sent).toEqual([{ type: "usePluginFormat", key: vst3Key("Surge XT") }]);
    expect(t("shelf.formatsTip", { formats: "LV2 ・ VST3" })).toBe("LV2 ・ VST3 があります。押すと、使う形式をえらべます。");
  });

  it("with nothing added later, the old form (no headings). At the bottom 「あとから入れたもの ・ 0」 and 「SF2 を足す…」", () => {
    show(RightRail, scene("shelfNewTrack"));
    expect(q("[data-test='section:builtin']")).toBeNull();
    expect(headings()[0]).toBe("▾ ピアノ ・ 鍵盤 11");
    expect(text(q("[data-test='section:added']"))).toBe("あとから入れたもの ・ 0 SF2 を足す…");
    click("[data-test='section:added'] [data-test=shelf-add-sf2]");
    expect(app.sent).toEqual([{ type: "addSf2" }]);
  });

  it("while scanning, the scan band is just below the 「あとから入れたもの」 heading", () => {
    const s = scene("shelfPlugins");
    s.plugins.scan = { state: "running", done: 37, total: 112 };
    show(RightRail, s);
    const added = q("[data-test='section:added']")!;
    expect(added.nextElementSibling!.querySelector("[data-test=scan-strip]")).not.toBeNull();
  });
});

describe("search (mock F5-ShelfSearch)", () => {
  it("matches name, maker and type, and opens only headings with a match. The count is 「matches / all」", () => {
    show(RightRail, scene("shelfSearch"));
    shelfUi.query = "Dragonfly";
    flushSync();
    expect(q("[data-test='section:builtin']")).toBeNull();
    expect(text(q("[data-test='section:added']"))).toBe("あとから入れたもの ・ 4 / 506 もう一度スキャン");
    expect(headings()).toEqual(["▾ リバーブ 4 / 23"]);
    expect(qa("[data-test=shelf-row]").map((r) => text(r.querySelector(".name")))).toEqual([
      "Dragonfly Early Reflections",
      "Dragonfly Hall Reverb",
      "Dragonfly Plate Reverb",
      "Dragonfly Room Reverb",
    ]);
    // By maker too.
    shelfUi.query = "michael";
    flushSync();
    expect(qa("[data-test=shelf-row]").length).toBe(4);
    // If the type name matches, all rows under that heading.
    shelfUi.query = "ピッチ";
    flushSync();
    expect(headings()).toEqual(["▾ ピッチ 9 / 9"]);
    // Headings fold while searching too.
    click('[data-group="c:effect:pitch"]');
    expect(headings()).toEqual(["▸ ピッチ 9 / 9"]);
    shelfUi.query = "";
    flushSync();
    expect(headings()[3]).toBe("▸ リバーブ 23");
  });

  it("SF2 matches the instrument name and the group name", () => {
    show(RightRail, scene("sf2Added"));
    shelfUi.query = "guitar";
    flushSync();
    expect(headings()).toEqual([`▾ ${FLUID} SF2 8 / 137`, "▾ ギター 8 / 8"]);
    shelfUi.query = "マレット";
    flushSync();
    expect(headings()).toEqual([`▾ ${FLUID} SF2 8 / 137`, "▾ マレット 8 / 8"]);
  });

  it("the status line is 「さがす「Dragonfly」・ 合う物 4 個」", () => {
    const s = scene("shelfSearch");
    shelfUi.view = "effects";
    shelfUi.query = "Dragonfly";
    show(StatusLine, s);
    expect(text(q(".message"))).toBe("さがす「Dragonfly」・ 合う物 4 個");
  });
});

describe("favorites (spec 3 86-4)", () => {
  it("the effect list also has stars and 「すべて ・ お気に入り」. At first, reverb and delay. A star sends ToggleFavoriteEffect", () => {
    show(RightRail, scene("shelfFx"));
    expect(text(q("[data-test=shelf-all]"))).toBe("すべて 512");
    expect(text(q("[data-test=shelf-favorites]"))).toBe("お気に入り 2");
    expect(text(q(".keys"))).toBe("印 F");
    click("[data-test=shelf-favorites]");
    expect(headings()).toEqual(["▸ 最初から入っているもの 2"]);
    click('[data-group="fx:builtin"]');
    expect(qa("[data-test=shelf-row]").map((r) => r.dataset.effect)).toEqual(["reverb", "delay"]);
    click("[data-test=shelf-all]");
    click('[data-group="c:effect:reverb"]');
    rowNamed("MVerb").querySelector<HTMLElement>("[data-test=shelf-star]")!.click();
    expect(app.sent).toEqual([{ type: "toggleFavoriteEffect", effect: { kind: "plugin", key: lv2Key("MVerb") } }]);
  });

  it("stars on plugins and SF2 instruments send ToggleFavoriteSound", () => {
    const s = scene("shelfPlugins");
    s.shelf.favoriteSounds = [...s.shelf.favoriteSounds, { kind: "plugin", key: lv2Key("Helm") }];
    show(RightRail, s);
    expect(text(q("[data-test=shelf-favorites]"))).toBe("お気に入り 3");
    click('[data-group="c:instrument:synth"]');
    expect(rowNamed("Helm").querySelector("[data-test=shelf-star]")!.getAttribute("aria-pressed")).toBe("true");
    rowNamed("Dexed").querySelector<HTMLElement>("[data-test=shelf-star]")!.click();
    click(`[data-group="f:${FLUID}"]`);
    click(`[data-group="ff:piano:${FLUID}"]`);
    rowNamed("Honky-tonk Piano").querySelector<HTMLElement>("[data-test=shelf-star]")!.click();
    expect(app.sent).toEqual([
      { type: "toggleFavoriteSound", sound: { kind: "plugin", key: lv2Key("Dexed") } },
      { type: "toggleFavoriteSound", sound: { kind: "sf2", file: FLUID, bank: 0, program: 3 } },
    ]);
    // With favorites only, only headings of rows with a star.
    click("[data-test=shelf-favorites]");
    expect(headings().filter((h) => !h!.includes("鍵盤") && !h!.includes("ドラム") && !h!.includes("最初から"))).toEqual(["▾ シンセ 1"]);
  });
});

describe("try-out box (mock F5-PluginLoading, F5-PluginAudition, F5-ShelfFx, spec 3 Y14)", () => {
  it("loading: 「読みこみ中」「Surge XT を読みこんでいます…」 and 「やめる Esc」", () => {
    show(RightRail, scene("pluginLoading"));
    click('[data-group="c:instrument:synth"]');
    const box = q("[data-test=shelf-loading]")!;
    expect(text(box)).toBe("読みこみ中 Surge XT を読みこんでいます… やめる Esc");
    expect(box.parentElement!.previousElementSibling).toBe(rowNamed("Surge XT"));
    click("[data-test=shelf-loading] [data-test=shelf-cancel]");
    expect(app.sent).toEqual([{ type: "shelfCancel" }]);
  });

  it("cannot load: the missing-item notice box shows 「Surge XT を読みこめませんでした。こわれているか…」 and 「やめる Esc」 (spec 3 89-12)", () => {
    show(RightRail, scene("pluginUnreadable"));
    click('[data-group="c:instrument:synth"]');
    const box = q("[data-test=shelf-unreadable]")!;
    expect(text(box)).toBe("Surge XT を読みこめませんでした。こわれているか、この PC では動かないのかもしれません。 やめる Esc");
    expect(box.classList.contains("notice")).toBe(true);
    click("[data-test=shelf-unreadable] [data-test=shelf-cancel]");
    expect(app.sent).toEqual([{ type: "shelfCancel" }]);
    expect(press(scene("pluginUnreadable"), "Escape")).toEqual([{ type: "shelfCancel" }]);
    expect(rowNamed("Surge XT").classList.contains("selected")).toBe(true);
    expect(q("[data-test=shelf-confirm]")).toBeNull();
  });

  it("in try-out: 「トラック 6 を作る」「やめる」, 「窓をひらく」 and 「音色は窓でえらべます」. 「窓をひらく」 sends OpenPluginWindow (no slot)", () => {
    show(RightRail, scene("pluginAudition"));
    click('[data-group="c:instrument:synth"]');
    expect(text(q("[data-test=shelf-audition]"))).toBe(
      `ためし弾き中 ${t("msg.audition")} トラック 6 を作る Enter やめる Esc 窓をひらく 音色は窓でえらべます`,
    );
    click("[data-test=shelf-open-window]");
    expect(app.sent).toEqual([{ type: "openPluginWindow", slot: null }]);
  });

  it("sample ▶: while loading, the time place shows 「読みこみ中…」. SF2 and built-in instruments have no 「窓をひらく」", () => {
    const s = scene("pluginAudition");
    s.shelf.soundSample = { sound: { kind: "plugin", key: lv2Key("Surge XT") }, playing: null };
    show(RightRail, s);
    click('[data-group="c:instrument:synth"]');
    expect(text(rowNamed("Surge XT").querySelector("[data-test=shelf-sample-time]"))).toBe("読みこみ中…");
    app.setState(scene("sf2Shelf"));
    flushSync();
    expect(q("[data-test=shelf-open-window]")).toBeNull();
  });

  it("adding an effect: 「5 Surge XT のインサート 2 に入れます。ミックスは、はじめ 30 %。」 and 「入れる」「やめる」", () => {
    show(RightRail, scene("shelfFx"));
    click('[data-group="c:effect:reverb"]');
    const box = q("[data-test=shelf-insert]")!;
    expect(box.parentElement!.previousElementSibling).toBe(rowNamed("Dragonfly Hall Reverb"));
    expect(text(box)).toBe("5 Surge XT のインサート 2 に入れます。ミックスは、はじめ 30 %。 入れる Enter やめる Esc");
    click("[data-test=shelf-insert] [data-test=shelf-cancel]");
    rowNamed("Calf Reverb").click();
    expect(app.sent).toEqual([{ type: "shelfCancel" }, { type: "shelfChooseEffect", effect: { kind: "plugin", key: lv2Key("Calf Reverb") } }]);
  });
});

describe("notice while a file is dragged in (spec 2, mock F5-SF2Drop)", () => {
  it("can add: the file name, what happens on drop, how to stop. Cannot add: 「SF2 のファイルではないので、足せません。」 in gray.8", () => {
    show(FileDropNotice, scene("sf2DropAddable"), { drop: { name: FLUID, addable: true, files: 1, sf2: 1, audio: 0 } });
    expect(text(q("[data-test=file-drop]"))).toBe(`${FLUID} ${t("sf2.dropHere")} ${t("sf2.dropCancel")}`);
    expect(q(".box")!.classList.contains("no")).toBe(false);
    unmount(mounted!);
    show(FileDropNotice, scene("sf2DropNotAddable"), { drop: { name: "piano-demo.wav", addable: false, files: 1, sf2: 0, audio: 0 } });
    expect(text(q("[data-test=file-drop]"))).toBe(`piano-demo.wav SF2 のファイルではないので、足せません。 ${t("sf2.dropCancel")}`);
    expect(q(".box")!.classList.contains("no")).toBe(true);
    // With several files, 「3 つのファイル ・ SF2 は 2 つ」 (spec 3 89-10).
    unmount(mounted!);
    show(FileDropNotice, scene("sf2DropMany"), { drop: { name: FLUID, addable: true, files: 3, sf2: 2, audio: 0 } });
    expect(text(q("[data-test=file-drop-name]"))).toBe("3 つのファイル ・ SF2 は 2 つ");
    unmount(mounted!);
    show(FileDropNotice, scene("sf2DropNotAddable"), { drop: { name: "a.wav", addable: false, files: 2, sf2: 0, audio: 0 } });
    expect(text(q("[data-test=file-drop]"))).toBe(`2 つのファイル SF2 のファイルではないので、足せません。 ${t("sf2.dropCancel")}`);
  });

  it("Esc sends CancelFileDrop (before the turn to close the shelf). The status line is 「SF2 のファイルを持ってきています」", () => {
    const s = scene("sf2DropAddable");
    expect(press(s, "Escape")).toEqual([{ type: "cancelFileDrop" }]);
    show(StatusLine, s);
    expect(text(q(".message"))).toBe("SF2 のファイルを持ってきています");
  });
});

describe("notice of missing things (spec 2, spec 3 83 and 85, mock R8-Missing)", () => {
  it("no sound pack: a notice above the list, and only the sine wave row", () => {
    show(RightRail, scene("missingPack"));
    expect(text(q("[data-test=shelf-notice-pack]"))).toBe(`${t("shelf.soundsMissing")} ${t("button.fetchPack")}`);
    expect(qa("[data-test=shelf-row]").map((r) => r.dataset.row)).toEqual(["sine"]);
    // Spec 3, where the sound pack can be fetched later: the shelf's missing-item notice also has 「取りにいく」.
    q("[data-test=shelf-fetch-pack]")!.click();
    expect(app.sent.at(-1)).toEqual({ type: "fetchPack" });
  });

  it("no SF2: 「曲で使っている FluidR3_GM.sf2 が見つかりません。…」 with 「SF2 を足す…」「曲から外す」", () => {
    show(RightRail, scene("missingSf2"));
    const notice = q("[data-test=shelf-notice-sf2]")!;
    expect(text(notice)).toBe(`曲で使っている ${FLUID} が見つかりません。2 本のトラックはサイン波で鳴らしています。 SF2 を足す… 曲から外す`);
    notice.querySelector<HTMLElement>("[data-test=shelf-add-sf2]")!.click();
    notice.querySelector<HTMLElement>("[data-test=shelf-remove-missing]")!.click();
    expect(app.sent).toEqual([{ type: "addSf2" }, { type: "removeMissing", missing: { kind: "sf2", file: FLUID } }]);
  });

  it("no plugin: the name (gray.8) and 「見つかりません」, 「前はここにありました: …」 and 3 buttons. The instrument list has only instruments", () => {
    show(RightRail, scene("missingPlugin"));
    const notices = qa("[data-test=shelf-notice-plugin]");
    expect(notices.length).toBe(1);
    expect(text(notices[0]!)).toBe(
      "Surge XT 見つかりません LV2 前はここにありました: /usr/lib/lv2/Surge XT.lv2。曲の中の設定は残しています。 場所をさがす もう一度スキャン 曲から外す",
    );
    expect(notices[0]!.querySelector(".name.gone")).not.toBeNull();
    for (const b of ["shelf-locate", "shelf-rescan", "shelf-remove-missing"]) notices[0]!.querySelector<HTMLElement>(`[data-test=${b}]`)!.click();
    expect(app.sent).toEqual([
      { type: "addPluginPlace" },
      { type: "rescanPlugins" },
      { type: "removeMissing", missing: { kind: "plugin", key: lv2Key("Surge XT") } },
    ]);
    click("[data-test=shelf-effects]");
    expect(text(q("[data-test=shelf-notice-plugin]"))).toContain("MVerb 見つかりません LV2 前はここにありました: ~/.lv2/MVerb.lv2。");
  });

  it("the track head, the mixer's instrument frame and the input meter's play target show 「サイン波で代わり」 (gray.8)", () => {
    const s = scene("missingPack");
    show(TrackHeads, s);
    expect(qa("[data-test=track-instrument]").map(text)).toEqual(Array(4).fill("サイン波で代わり"));
    unmount(mounted!);
    show(MixerStrips, s);
    const frame = q("[data-strip=track-" + s.song.tracks[0]!.id + "] [data-test=mixer-frame]")!;
    expect(text(frame)).toBe("音源 ・ サイン波で代わり");
    expect(frame.classList.contains("dim")).toBe(true);
    unmount(mounted!);
    show(InputMeter, s);
    expect(text(q("[data-test=meter-target]"))).toMatch(/^1 ピアノ/);
    expect(t("track.standInPackTip")).toBe("音の素材が見つからないので、サイン波で鳴らしています。");
  });

  it("status line: when a song is opened", () => {
    show(StatusLine, scene("missingPack"));
    expect(text(q(".message"))).toBe("音の素材が見つからないので、4 本のトラックをサイン波で鳴らしています。");
    app.setState(scene("missingSf2"));
    flushSync();
    expect(text(q(".message"))).toBe(`曲で使っている ${FLUID} が見つかりません。2 本のトラックはサイン波で鳴らしています。`);
  });
});

describe("plugin name and stopped frame (spec 2, spec 6 (Plugin stopped))", () => {
  it("track and mixer show the plugin name. The stopped frame is a red frame with 「止」; right click gives 「もう一度読みこむ」「外す」", () => {
    const s = scene("shelfFx");
    show(MixerStrips, s);
    const surge = s.song.tracks[4]!;
    expect(text(q(`[data-strip=track-${surge.id}] [data-test=mixer-frame]`))).toBe("音源 ・ Surge XT");
    const pad = s.song.tracks[3]!;
    expect(qa(`[data-strip=track-${pad.id}] [data-test=mixer-insert]`).map(text)).toEqual(["MVerb", "EQ 4 バンド", "空き＋", "空き＋"]);
    const drums = s.song.tracks[2]!;
    const brick = qa(`[data-strip=track-${drums.id}] [data-test=mixer-insert]`)[1]!;
    expect(text(brick)).toBe("Brick EQ 止");
    expect(brick.classList.contains("stopped")).toBe(true);
    brick.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 5, clientY: 5 }));
    flushSync();
    const rows = contextMenu.open!.entries as MenuRow[];
    expect(rows.map((r) => t(r.label))).toEqual(["もう一度読みこむ", "外す"]);
    app.sent.length = 0;
    rows[0]!.run();
    rows[1]!.run();
    const slot = { kind: "insert", insert: { strip: { kind: "track", id: drums.id }, slot: 1 } };
    expect(app.sent).toEqual([
      { type: "reloadPlugin", slot },
      { type: "removePlugin", slot },
    ]);
  });

  it("below the shelf, the box of the stopped plugin (「Brick EQ が止まりました ・ 3 ドラム」 and 2 buttons)", () => {
    const s = scene("shelfFx");
    show(RightRail, s);
    const box = q("[data-test=shelf-stopped]")!;
    expect(text(box)).toBe(`Brick EQ が止まりました ・ 3 ドラム ${t("plugin.stoppedBody")} もう一度読みこむ 外す`);
    box.querySelector<HTMLElement>("[data-test=shelf-reload]")!.click();
    expect(app.sent).toEqual([{ type: "reloadPlugin", slot: { kind: "insert", insert: { strip: { kind: "track", id: s.song.tracks[2]!.id }, slot: 1 } } }]);
  });

  it("the instrument name in the head is the plugin name. While trying out a replacement, the SF2 instrument name (in English as it is)", () => {
    const s = scene("pluginTrack");
    show(TrackHeads, s);
    expect(text(qa("[data-test=track-instrument]")[4]!)).toBe("Surge XT");
    const r = scene("sf2Shelf");
    r.shelf.mode = { kind: "replace", track: r.song.tracks[0]!.id };
    app.setState(r);
    flushSync();
    expect(text(qa("[data-test=track-instrument]")[0]!)).toBe("Clean Guitar（ためし弾き）");
  });
});

describe("new status line text (protocol 17)", () => {
  it("11 kinds", () => {
    const s = scene("tracks6");
    show(StatusLine, s);
    const say = (message: AppState["status"]["message"]) => {
      app.setState({ ...s, status: { message } });
      flushSync();
      return text(q(".message"));
    };
    const clean = { kind: "sf2" as const, name: "Clean Guitar", file: FLUID };
    const surge = { kind: "plugin" as const, name: "Surge XT" };
    const piano = { kind: "builtin" as const, instrument: "grandPiano" as const };
    const track = { number: 1, name: "ピアノ" };
    expect(say({ id: "auditionNewTrackSound", sound: clean })).toBe("Clean Guitar ・ FluidR3_GM.sf2 ・ まだトラックには入っていません ・ 入れるには Enter");
    expect(say({ id: "auditionNewTrackSound", sound: piano })).toBe("グランドピアノ ・ まだトラックには入っていません ・ 入れるには Enter");
    expect(say({ id: "auditionReplaceSound", sound: surge, track, current: piano })).toBe(
      "Surge XT ・ 1 ピアノ の音源は、まだグランドピアノです ・ 入れかえるには Enter",
    );
    expect(say({ id: "samplePlayingSound", sound: surge })).toBe("サンプルを再生中 ・ Surge XT。行をえらぶと、鍵盤でためし弾きできます。");
    expect(say({ id: "instrumentReplacedSound", track, sound: clean, lanesRemoved: 0 })).toBe(
      "1 ピアノ の音源を Clean Guitar ・ FluidR3_GM.sf2 にしました。取り消せます（Ctrl Z）",
    );
    expect(say({ id: "soundLoading", sound: surge })).toBe("Surge XTを読みこんでいます… 少しだけ待つと、鍵盤で鳴らせます。");
    expect(say({ id: "pluginLoading", name: "Surge XT" })).toBe("Surge XT を読みこんでいます… ・ やめるには Esc");
    expect(say({ id: "sf2Dragging" })).toBe("SF2 のファイルを持ってきています");
    expect(say({ id: "sf2Added", file: FLUID, presets: 128, kits: 9, skipped: 0 })).toBe("FluidR3_GM.sf2 を足しました。音色 128 ・ ドラムのキット 9");
    expect(say({ id: "sf2Unreadable", file: FLUID })).toBe("FluidR3_GM.sf2 は読めませんでした。ファイルがこわれているか、SF2 ではないかもしれません。");
    expect(say({ id: "sf2Missing", file: FLUID, tracks: 2 })).toBe("曲で使っている FluidR3_GM.sf2 が見つかりません。2 本のトラックはサイン波で鳴らしています。");
    expect(say({ id: "soundsMissing", tracks: 4 })).toBe("音の素材が見つからないので、4 本のトラックをサイン波で鳴らしています。");
  });
});

describe("the rest of 85", () => {
  const KICK = 36;
  const SNARE = 38;

  it("drum key column: a mark left of the sounding row and the played row (accent, rec while recording)", () => {
    const drum: DrumGeom = { keys: [49, 42, 38, 36], top: 0, hitW: 8, names: "sf2" };
    const g: RollGeom = { scrollTick: 0, ppt: 1, topKey: 0, rowH: 20, width: 100, height: 80, drum };
    const out = new Batch();
    out.clip(0, 0, 132, 80);
    paintRollKeys(out, { x0: 0, y0: 0, x1: 132, y1: 80 }, g, (k) => k === SNARE, 1, { width: 2, color: "color.rec" });
    const color = (r: number[]) => [r[4], r[5], r[6]].join(",");
    const rec = Array.from(Float32Array.from(rgb("color.rec"))).join(",");
    const marks = out.rects().filter((r) => color(r) === rec);
    expect(marks.map((r) => [r[0], r[1], r[2]])).toEqual([[0, 40, 2]]);
  });

  it("readout when moving drums up and down: 「+0.120 ・ 上へ 1 行」「下へ 2 行」", () => {
    const rows = [49, 42, 38, 36];
    expect(drumMoveReading(rows, SNARE, 42 - SNARE, "+0.120", true)).toBe("+0.120 ・ 上へ 1 行");
    expect(drumMoveReading(rows, 49, SNARE - 49, "+0.000", false)).toBe("下へ 2 行");
    expect(drumMoveReading(rows, SNARE, 0, "+0.240", true)).toBe("+0.240");
  });

  it("same names are grouped: 「スネア ×2」「C4 ×2 E4」. 5 or more show 4 and 「…」", () => {
    expect(keyCountsText([{ key: KICK, count: 4 }, { key: SNARE, count: 2 }], "builtin")).toBe("キック ×4 スネア ×2");
    expect(keyCountsText([{ key: 60, count: 2 }, { key: 64, count: 1 }], false)).toBe("C4 ×2 E4");
    expect(selectedKeysText([64, 60, 60], false)).toBe("C4 ×2 E4");
    expect(selectedKeysText([60, 62, 64, 65, 67, 67], false)).toBe("C4 D4 E4 F4 …");
    // The status line comes from the protocol's names.
    const s = scene("tracks6");
    show(StatusLine, s);
    app.setState({
      ...s,
      status: { message: { id: "summary", loopBars: null, nothingSelected: false, notes: { count: 6, keys: [], quantized: false, names: [60, 62, 64, 65, 67].map((key) => ({ key, count: key === 67 ? 2 : 1 })) } } },
    });
    flushSync();
    expect(text(q(".message"))).toBe("選択 6 ノート ・ C4 D4 E4 F4 …");
  });

  it("even when stopped, while keys are held the end shows 「・ 押している音 キック スネア」", () => {
    const s = scene("drumRoll");
    s.transport.state = "stopped";
    s.status.message = { id: "summary", loopBars: null, nothingSelected: true, notes: null };
    show(StatusLine, s);
    app.setTelemetry(app.telemetryWith({ keys: [SNARE, KICK] }));
    flushSync();
    expect(text(q(".message"))).toBe("選択なし ・ 押している音 キック スネア");
    app.setTelemetry(app.telemetryWith({ keys: [] }));
    flushSync();
    expect(text(q(".message"))).toBe("選択なし");
  });

  it("SF2 drum kit: rows are the keys the kit has (drumKeys), names come from the key number table, keys not in the table use the note name", () => {
    const s = scene("drumRollSf2");
    const kit = s.song.tracks[2]!;
    expect(trackDrums(kit)).toBe("sf2");
    expect(drumKeysOf(kit, "sf2")).toEqual(kit.drumKeys);
    expect(drumName(47, "sf2")).toBe("ミッドタム 2");
    expect(drumName(47, "builtin")).toBe("ミッドタム");
    expect(drumName(62, "sf2")).toBe("コンガ（高 ・ ミュート）");
    expect(keyLabel(82, "sf2")).toBe("A#5");
    expect(keyLabel(35, "sf2")).toBe("キック 2");
  });
});

describe("reads only the new fields (so the old fields can be removed)", () => {
  it("even when the old fields (instrument, inserts, selected …) disagree, shows the new fields", () => {
    const s = scene("shelfAudition");
    const picked = s.shelf.selectedSound;
    // Make only the old fields random.
    s.shelf.selected = "organ";
    s.shelf.favorites = [];
    for (const tr of s.song.tracks) {
      tr.instrument = "organ";
      tr.inserts = ["limiter", "limiter", "limiter", "limiter"];
    }
    show(RightRail, s);
    expect(picked).toEqual({ kind: "builtin", instrument: "synthBass" });
    expect(q("[data-row=synthBass]")!.classList.contains("selected")).toBe(true);
    expect(q("[data-row=organ]")!.classList.contains("selected")).toBe(false);
    expect(text(q("[data-test=shelf-favorites]"))).toBe("お気に入り 2");
    unmount(mounted!);
    show(TrackHeads, s);
    expect(text(qa("[data-test=track-instrument]")[0]!)).toBe("グランドピアノ");
    unmount(mounted!);
    show(MixerStrips, s);
    expect(qa("[data-test=mixer-insert]").some((el) => text(el) === "リミッター" && el.closest("[data-strip^=track]"))).toBe(false);
  });
});

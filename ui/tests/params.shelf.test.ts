// 第 4 回 区切り3: 棚のエフェクト（「空き ＋」からひらく insertEffect のモード、音源のモードでは見るだけ）。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeScene, reduceMixer } from "../src/bridge/fake";
import RightRail from "../src/components/RightRail.svelte";
import { shelfModel } from "../src/components/shelfList";
import { shelfStepCommand, shelfUi } from "../src/components/tracks.svelte";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";
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
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  Object.assign(shelfUi, { query: "", favoritesOnly: false, open: {}, searchOpen: {}, searchFor: "", keys: false, view: "instruments" });
});
function show(component: Component<any>, state: AppState): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
function press(state: AppState, key: string): Command[] {
  const sent: Command[] = [];
  const deps: KeyDeps = { state: () => state, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0 };
  handleKeydown(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }), deps);
  return sent;
}

describe("棚のエフェクトの行", () => {
  const effects = (query: string) => {
    const s = scene("shelfEffects");
    const look = { query, favoritesOnly: false, open: {}, searchOpen: {}, searchFor: "" };
    return shelfModel("effects", s.shelf, s.library, look).rows.map((r) => (r.effect?.kind === "builtin" ? r.effect.effect : null));
  };
  it("EFFECT_KINDS の順。さがす文字は名前にふくまれる物", () => {
    expect(effects("")).toEqual(["eq4", "compressor", "reverb", "delay", "chorus", "limiter"]);
    expect(effects("ー")).toEqual(["compressor", "reverb", "chorus", "limiter"]);
    expect(effects("eq")).toEqual(["eq4"]);
  });
});

describe("「空き ＋」からひらいた棚（insertEffect）", () => {
  it("見出しは「（ストリップ）にエフェクトを入れる」、一覧はエフェクト、名前の右は「最初から入っている」。▶ はなく、星はある（86-4）", () => {
    const s = scene("shelfEffects");
    show(RightRail, s);
    const track = s.song.tracks.find((x) => x.id === (s.shelf.mode as { strip: { id: number } }).strip.id)!;
    expect(text(q("[data-test=shelf-mode]"))).toBe(`${track.number} ${track.name} にエフェクトを入れる`);
    expect(q("[data-test=shelf-effects]")!.classList.contains("on")).toBe(true);
    expect(qa("[data-effect]").map(text)).toEqual([
      "EQ 4 バンド 最初から入っている",
      "コンプレッサー 最初から入っている",
      "リバーブ 最初から入っている",
      "ディレイ 最初から入っている",
      "コーラス 最初から入っている",
      "リミッター 最初から入っている",
    ]);
    expect(q("[data-effect] [data-test=shelf-sample]")).toBeNull();
    expect(qa("[data-effect] [aria-label=お気に入り]").map((b) => b.getAttribute("aria-pressed"))).toEqual(["false", "false", "true", "true", "false", "false"]);
    expect(qa("[data-effect].selected").map((r) => r.dataset.effect)).toEqual(["compressor"]);
  });

  it("行を押すと ShelfChooseEffect（えらんでいる行では送らない）", () => {
    show(RightRail, scene("shelfEffects"));
    q("[data-effect=compressor]")!.click();
    q("[data-effect=delay]")!.click();
    expect(app.sent).toEqual([{ type: "shelfChooseEffect", effect: { kind: "builtin", effect: "delay" } }]);
  });

  it("↑ ↓ でえらび、Enter で ShelfConfirm", () => {
    const s = scene("shelfEffects");
    show(RightRail, s);
    const choose = (effect: string) => [{ type: "shelfChooseEffect", effect: { kind: "builtin", effect } }];
    expect(press(s, "ArrowDown")).toEqual(choose("reverb"));
    expect(press(s, "ArrowUp")).toEqual(choose("eq4"));
    expect(press(s, "Enter")).toEqual([{ type: "shelfConfirm" }]);
    // F はえらんだエフェクトのお気に入り（仕様書 4 章「棚の中（エフェクト）: お気に入りの印」）。
    expect(press(s, "f")).toEqual([{ type: "toggleFavoriteEffect", effect: { kind: "builtin", effect: "compressor" } }]);
    s.shelf.chosenEffect = null;
    expect(press(s, "Enter")).toEqual([]);
    expect(press(s, "f")).toEqual([]);
    expect(press(s, "ArrowDown")).toEqual(choose("eq4"));
  });

  it("「入れる」のボタン: えらんだ行の下、押すと ShelfConfirm。行をえらんでいないときは一覧の下で、使えない", () => {
    const s = scene("shelfEffects");
    show(RightRail, s);
    const button = () => q<HTMLButtonElement>("[data-test=shelf-insert-confirm]")!;
    expect(text(button())).toBe("入れる Enter");
    expect(button().classList.contains("primary")).toBe(true);
    expect(button().disabled).toBe(false);
    expect(q("[data-effect=compressor]")!.nextElementSibling!.contains(q("[data-test=shelf-insert]"))).toBe(true);
    button().click();
    expect(app.sent).toEqual([{ type: "shelfConfirm" }]);

    app.setState({ ...s, shelf: { ...s.shelf, chosenEffect: null } });
    flushSync();
    expect(qa("[data-test=shelf-insert-confirm]").length).toBe(1);
    expect(button().disabled).toBe(true);
    // 一覧のいちばん下（「あとから入れたもの ・ 0」の行の下）。
    const list = q("[data-test=shelf-list]")!;
    expect([...list.querySelectorAll("[data-effect], [data-test=section\\:added], [data-test=shelf-insert]")].at(-1)).toBe(q("[data-test=shelf-insert]"));
    button().click();
    expect(app.sent.length).toBe(1);
  });

  it("入ったあとは、その子がえらばれ、パラメータの欄が前に出る（偽物の答え）", () => {
    const s = scene("shelfEffects");
    const r = reduceMixer(s, s.shelf, s.song, { type: "shelfConfirm" })!;
    const mode = s.shelf.mode as { strip: { kind: "track"; id: number }; slot: number };
    expect(r.song.tracks.find((x) => x.id === mode.strip.id)!.inserts[mode.slot]).toBe("compressor");
    expect(r.params.selected?.effect).toBe("compressor");
    expect(r.params.selected?.insert).toEqual({ strip: mode.strip, slot: mode.slot });
    expect(r.showParams).toBe(true);
    expect(r.shelf.mode).toEqual({ kind: "newTrack" });
  });

  it("「音源」に切りかえて見ても、行を押してもためし弾きにしない", () => {
    show(RightRail, scene("shelfEffects"));
    q("[data-test=shelf-instruments]")!.click();
    flushSync();
    q("[data-row=grandPiano]")!.click();
    expect(app.sent).toEqual([]);
    expect(shelfStepCommand(scene("shelfEffects"), 1)).toBeNull();
  });
});

describe("音源のモードのエフェクトは見るだけ（A-14）", () => {
  it("「エフェクト 6」で一覧を見られる。押しても ↑ ↓ ・ Enter でも入れない", () => {
    const s = scene("shelfNewTrack");
    show(RightRail, s);
    q("[data-test=shelf-effects]")!.click();
    flushSync();
    expect(qa("[data-effect]").length).toBe(6);
    expect(q("[data-test=shelf-insert-confirm]")).toBeNull();
    expect(text(q("[data-test=shelf-mode]"))).toBe("新しいトラックの音源をえらぶ");
    q("[data-effect=reverb]")!.click();
    flushSync();
    expect(qa("[data-effect].selected")).toEqual([]);
    expect(press(s, "ArrowDown")).toEqual([]);
    expect(press(s, "Enter")).toEqual([]);
    expect(app.sent).toEqual([]);
  });

  it("モードが変わると、そのモードの一覧に戻る", () => {
    const s = scene("shelfNewTrack");
    show(RightRail, s);
    q("[data-test=shelf-effects]")!.click();
    flushSync();
    app.setState({ ...s, shelf: { ...s.shelf, mode: { kind: "replace", track: s.song.tracks[0]!.id } } });
    flushSync();
    expect(shelfUi.view).toBe("instruments");
    expect(q("[data-row]")).not.toBeNull();
  });

  it("棚の数: 見出しと縦の見出しは「音源 11 ・ エフェクト 6」（仕様書 4 章）", () => {
    const s = scene("mixer");
    show(RightRail, s);
    expect(text(q("nav [data-band=shelf]"))).toBe(`▸ 棚 音源 ${s.shelf.available.length} ・ エフェクト 6 P`);
  });
});

// 第 4 回 区切り3: 棚のエフェクト（「空き ＋」からひらく insertEffect のモード、音源のモードでは見るだけ）。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeScene, reduceMixer } from "../src/bridge/fake";
import RightRail from "../src/components/RightRail.svelte";
import { effectShelfCommand, shelfEffects, shelfUi } from "../src/components/tracks.svelte";
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
  Object.assign(shelfUi, { query: "", favoritesOnly: false, folded: [], keys: false, view: "instruments" });
});
function show(component: Component<any>, state: AppState): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
function press(state: AppState, key: string): Command[] {
  const sent: Command[] = [];
  const deps: KeyDeps = { state: () => state, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0 };
  handleKeydown(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }), deps);
  return sent;
}

describe("棚のエフェクトの行", () => {
  it("EFFECT_KINDS の順。さがす文字は名前にふくまれる物", () => {
    expect(shelfEffects("")).toEqual(["eq4", "compressor", "reverb", "delay", "chorus", "limiter"]);
    expect(shelfEffects("ー")).toEqual(["compressor", "reverb", "chorus", "limiter"]);
    expect(shelfEffects("eq")).toEqual(["eq4"]);
  });
});

describe("「空き ＋」からひらいた棚（insertEffect）", () => {
  it("見出しは「（ストリップ）にエフェクトを入れる」、一覧はエフェクト、名前の右は「最初から入っている」。▶ と星はない", () => {
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
    expect(q("[data-effect] [aria-label=お気に入り]")).toBeNull();
    expect(qa("[data-effect].selected").map((r) => r.dataset.effect)).toEqual(["compressor"]);
  });

  it("行を押すと ShelfSelectEffect（えらんでいる行では送らない）", () => {
    show(RightRail, scene("shelfEffects"));
    q("[data-effect=compressor]")!.click();
    q("[data-effect=delay]")!.click();
    expect(app.sent).toEqual([{ type: "shelfSelectEffect", effect: "delay" }]);
  });

  it("↑ ↓ でえらび、Enter で ShelfConfirm", () => {
    const s = scene("shelfEffects");
    show(RightRail, s);
    expect(press(s, "ArrowDown")).toEqual([{ type: "shelfSelectEffect", effect: "reverb" }]);
    expect(press(s, "ArrowUp")).toEqual([{ type: "shelfSelectEffect", effect: "eq4" }]);
    expect(press(s, "Enter")).toEqual([{ type: "shelfConfirm" }]);
    s.shelf.selectedEffect = null;
    expect(press(s, "Enter")).toEqual([]);
    expect(press(s, "ArrowDown")).toEqual([{ type: "shelfSelectEffect", effect: "eq4" }]);
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
    expect(effectShelfCommand("down", scene("shelfEffects"))).toBeNull();
  });
});

describe("音源のモードのエフェクトは見るだけ（A-14）", () => {
  it("「エフェクト 6」で一覧を見られる。押しても ↑ ↓ ・ Enter でも入れない", () => {
    const s = scene("shelfNewTrack");
    show(RightRail, s);
    q("[data-test=shelf-effects]")!.click();
    flushSync();
    expect(qa("[data-effect]").length).toBe(6);
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

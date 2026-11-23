// Effects on the shelf (insertEffect mode opened from 「空き ＋」; in instrument mode, view only).
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeScene, reduceMixer } from "../src/bridge/fake";
import { builtinOf } from "../src/components/params/selected";
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

describe("effect rows on the shelf", () => {
  const effects = (query: string) => {
    const s = scene("shelfEffects");
    const look = { query, favoritesOnly: false, open: {}, searchOpen: {}, searchFor: "" };
    return shelfModel("effects", s.shelf, s.library, look).rows.map((r) => (r.effect?.kind === "builtin" ? r.effect.effect : null));
  };
  it("in EFFECT_KINDS order. The search text matches inside the name", () => {
    expect(effects("")).toEqual(["eq4", "compressor", "reverb", "delay", "chorus", "limiter"]);
    expect(effects("ー")).toEqual(["compressor", "reverb", "chorus", "limiter"]);
    expect(effects("eq")).toEqual(["eq4"]);
  });
});

describe("shelf opened from 「空き ＋」 (insertEffect)", () => {
  it("the heading is 「（ストリップ）にエフェクトを入れる」, the list is effects, right of the name is 「最初から入っている」. No ▶, but a star (86-4)", () => {
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

  it("pressing a row sends ShelfChooseEffect (not for the chosen row)", () => {
    show(RightRail, scene("shelfEffects"));
    q("[data-effect=compressor]")!.click();
    q("[data-effect=delay]")!.click();
    expect(app.sent).toEqual([{ type: "shelfChooseEffect", effect: { kind: "builtin", effect: "delay" } }]);
  });

  it("↑ ↓ choose, Enter sends ShelfConfirm", () => {
    const s = scene("shelfEffects");
    show(RightRail, s);
    const choose = (effect: string) => [{ type: "shelfChooseEffect", effect: { kind: "builtin", effect } }];
    expect(press(s, "ArrowDown")).toEqual(choose("reverb"));
    expect(press(s, "ArrowUp")).toEqual(choose("eq4"));
    expect(press(s, "Enter")).toEqual([{ type: "shelfConfirm" }]);
    // F is the favorite of the chosen effect (spec 4 (In the shelf (effects): favorite mark)).
    expect(press(s, "f")).toEqual([{ type: "toggleFavoriteEffect", effect: { kind: "builtin", effect: "compressor" } }]);
    s.shelf.chosenEffect = null;
    expect(press(s, "Enter")).toEqual([]);
    expect(press(s, "f")).toEqual([]);
    expect(press(s, "ArrowDown")).toEqual(choose("eq4"));
  });

  it("the 「入れる」 button: below the chosen row, pressing sends ShelfConfirm. With no row chosen it is below the list and disabled", () => {
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
    // The bottom of the list (below the 「あとから入れたもの ・ 0」 row).
    const list = q("[data-test=shelf-list]")!;
    expect([...list.querySelectorAll("[data-effect], [data-test=section\\:added], [data-test=shelf-insert]")].at(-1)).toBe(q("[data-test=shelf-insert]"));
    button().click();
    expect(app.sent.length).toBe(1);
  });

  it("once added, that child is chosen and the params panel comes to the front (the fake's answer)", () => {
    const s = scene("shelfEffects");
    const r = reduceMixer(s, s.shelf, s.song, { type: "shelfConfirm" })!;
    const mode = s.shelf.mode as { strip: { kind: "track"; id: number }; slot: number };
    expect(r.song.tracks.find((x) => x.id === mode.strip.id)!.inserts[mode.slot]).toBe("compressor");
    expect(builtinOf(r.params)?.effect).toBe("compressor");
    expect(builtinOf(r.params)?.insert).toEqual({ strip: mode.strip, slot: mode.slot });
    expect(r.showParams).toBe(true);
    expect(r.shelf.mode).toEqual({ kind: "newTrack" });
  });

  it("switching to 「音源」 to look, or pressing a row, does not start a try-out", () => {
    show(RightRail, scene("shelfEffects"));
    q("[data-test=shelf-instruments]")!.click();
    flushSync();
    q("[data-row=grandPiano]")!.click();
    expect(app.sent).toEqual([]);
    expect(shelfStepCommand(scene("shelfEffects"), 1)).toBeNull();
  });
});

describe("effects in instrument mode are view only (A-14)", () => {
  it("「エフェクト 6」 shows the list. Pressing, ↑ ↓ and Enter do not add", () => {
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

  it("when the mode changes, the list of that mode comes back", () => {
    const s = scene("shelfNewTrack");
    show(RightRail, s);
    q("[data-test=shelf-effects]")!.click();
    flushSync();
    app.setState({ ...s, shelf: { ...s.shelf, mode: { kind: "replace", track: s.song.tracks[0]!.id } } });
    flushSync();
    expect(shelfUi.view).toBe("instruments");
    expect(q("[data-row]")).not.toBeNull();
  });

  it("shelf counts: the heading and the vertical heading are 「音源 11 ・ エフェクト 6」 (spec 4)", () => {
    const s = scene("mixer");
    show(RightRail, s);
    expect(text(q("nav [data-band=shelf]"))).toBe(`▸ 棚 音源 ${s.shelf.available.length} ・ エフェクト 6 P`);
  });
});

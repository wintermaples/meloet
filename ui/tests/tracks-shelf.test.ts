// 第 4 回 区切り1 の画面: トラックの見出し（名前 ・ 並べかえ ・ 色 ・ 消す ・ 音源の名前）、棚、キー、偽物の答え。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { INSTRUMENT_GROUPS, type AppState, type Command } from "../src/bridge";
import { createFakeTransport, fakeScene, FakeBands, reduceTracks } from "../src/bridge/fake";
import Arrange from "../src/components/Arrange.svelte";
import { cancelDrag } from "../src/components/pianoroll/view.svelte";
import InputMeter from "../src/components/InputMeter.svelte";
import RightRail from "../src/components/RightRail.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { moveSlot, shelfGroups, shelfStep, shelfUi, trackEdit } from "../src/components/tracks.svelte";
import { handleKeydown, handlePointerdown, type KeyDeps } from "../src/keys/attach";
import { keyAction, type KeyInput } from "../src/keys/keys";
import { t } from "../src/text/ja";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state;
const k = (key: string, mods: Partial<KeyInput> = {}): KeyInput => ({ key, ctrl: false, meta: false, alt: false, shift: false, ...mods });

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
  trackEdit.renaming = null;
  Object.assign(shelfUi, { query: "", favoritesOnly: false, folded: [], keys: false, view: "instruments" });
});

function show(component: Component<any>, state: AppState): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
function pointer(el: Element, type: string, clientY: number): void {
  el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX: 10, clientY, button: 0 }));
  flushSync();
}
function keyOn(el: Element, key: string): KeyboardEvent {
  const ev = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true });
  el.dispatchEvent(ev);
  flushSync();
  return ev;
}

describe("棚の行の決まり（画面だけ）", () => {
  const shelf = () => scene("shelfNewTrack").shelf;

  it("グループの順と数。さがす文字は名前かグループの名前にふくまれる行だけ。出す物がないグループは見出しごと出さない", () => {
    const all = shelfGroups(shelf(), { query: "", favoritesOnly: false, folded: [] });
    expect(all.map((g) => [g.group, g.rows.length])).toEqual([
      ["pianoKeys", 11],
      ["guitar", 4],
      ["bass", 4],
      ["drums", 3],
      ["padStrings", 3],
      ["wind", 4],
      ["synth", 2],
    ]);
    expect(shelfGroups(shelf(), { query: "ベース", favoritesOnly: false, folded: [] }).map((g) => g.rows)).toEqual([
      ["fingerBass", "pickBass", "woodBass", "synthBass"],
    ]);
    // グループの名前に当たれば、そのグループの行はぜんぶ。
    expect(shelfGroups(shelf(), { query: "弦", favoritesOnly: false, folded: [] }).map((g) => g.rows)).toEqual([["strings", "pizzicato", "warmPad"]]);
    expect(shelfGroups(shelf(), { query: "オルガン", favoritesOnly: false, folded: [] }).map((g) => g.rows)).toEqual([
      ["organDrawbar", "organPercussive", "organRock", "organ", "organQuiet", "organChurch", "organReed"],
    ]);
    expect(shelfGroups(shelf(), { query: "シンセ", favoritesOnly: false, folded: [] }).map((g) => g.rows)).toEqual([
      ["synthBass"],
      ["synthLead", "sine"],
    ]);
  });

  it("お気に入りだけ。棚にない音源は出さない（available だけ）", () => {
    const s = { ...shelf(), available: shelf().available.filter((i) => i !== "studioKit") };
    expect(shelfGroups(s, { query: "", favoritesOnly: true, folded: [] }).map((g) => g.rows)).toEqual([["grandPiano"]]);
    expect(shelfGroups({ ...s, available: ["sine"] }, { query: "", favoritesOnly: false, folded: [] }).map((g) => g.group)).toEqual(["synth"]);
  });

  it("↑ ↓ は見えている行の中で動き、はしで止まる。たたんだグループはとばす", () => {
    const groups = shelfGroups(shelf(), { query: "", favoritesOnly: false, folded: ["bass"] });
    expect(shelfStep(groups, null, 1)).toBe("grandPiano");
    expect(shelfStep(groups, null, -1)).toBe("sine");
    expect(shelfStep(groups, "organ", 1)).toBe("organQuiet");
    expect(shelfStep(groups, "marimba", 1)).toBe("guitarNylon");
    expect(shelfStep(groups, "guitarDistortion", 1)).toBe("studioKit");
    expect(shelfStep(groups, "grandPiano", -1)).toBe("grandPiano");
  });

  it("並べかえの線と行き先: まん中が越えた境の線。自分のまわりの線は動かない", () => {
    const h = 64;
    expect(moveSlot(0, 4, 0.5 * h + 10, h)).toEqual({ slot: 1, to: 0 });
    expect(moveSlot(0, 4, 0.5 * h + 40, h)).toEqual({ slot: 1, to: 0 });
    expect(moveSlot(0, 4, 0.5 * h + 100, h)).toEqual({ slot: 2, to: 1 });
    expect(moveSlot(3, 4, 3.5 * h - 200, h)).toEqual({ slot: 0, to: 0 });
    expect(moveSlot(1, 4, 9 * h, h)).toEqual({ slot: 4, to: 3 });
  });
});

describe("キー（仕様書 4 章）", () => {
  it("F2 は文字を打っている最中も効く。棚の中の ↑ ↓ Enter は打っている最中も、Space と F は打っていないときだけ", () => {
    expect(keyAction(k("F2"), true, "other")).toEqual({ kind: "renameTrack" });
    expect(keyAction(k("ArrowDown"), true, "other", "shelf")).toEqual({ kind: "shelfStep", delta: 1 });
    expect(keyAction(k("ArrowUp"), false, "other", "shelf")).toEqual({ kind: "shelfStep", delta: -1 });
    expect(keyAction(k("Enter"), true, "other", "shelf")).toEqual({ kind: "shelfConfirm" });
    expect(keyAction(k(" ", { code: "Space" }), false, "other", "shelf")).toEqual({ kind: "shelfSample" });
    expect(keyAction(k(" ", { code: "Space" }), true, "other", "shelf")).toBeNull();
    expect(keyAction(k("f"), false, "other", "shelf")).toEqual({ kind: "shelfFavorite" });
    expect(keyAction(k("f"), true, "other", "shelf")).toBeNull();
    // 棚にキーが向いていなければ、いままでどおり。
    expect(keyAction(k(" ", { code: "Space" }), false, "other")).toEqual({ kind: "playStop" });
    expect(keyAction(k("f"), false, "other")).toBeNull();
    expect(keyAction(k("ArrowDown"), false, "other")).toEqual({ kind: "nudge", steps: 0, keys: -1 });
  });

  function deps(state: AppState, sent: Command[]): KeyDeps {
    return { state: () => state, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0 };
  }
  const press = (d: KeyDeps, key: string, init: KeyboardEventInit = {}) => {
    const ev = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...init });
    handleKeydown(ev, d);
    return ev;
  };

  it("棚にキーが向いているとき: ↓ で次の行、Enter で入れる、Space でサンプル、F でお気に入り", () => {
    const s = scene("shelfAudition");
    const sent: Command[] = [];
    const d = deps(s, sent);
    shelfUi.keys = true;
    press(d, "ArrowDown");
    expect(press(d, "Enter").defaultPrevented).toBe(true);
    expect(press(d, " ", { code: "Space" }).defaultPrevented).toBe(true);
    press(d, "f");
    expect(sent).toEqual([
      { type: "shelfSelect", instrument: "studioKit" },
      { type: "shelfConfirm" },
      { type: "playSample", instrument: "synthBass" },
      { type: "toggleFavorite", instrument: "synthBass" },
    ]);
  });

  it("棚の外を押すとキーは外れ、Space は再生にもどる。棚を押すとまた向く", () => {
    const s = scene("shelfAudition");
    s.transport.canPlay = true;
    const sent: Command[] = [];
    const d = deps(s, sent);
    const inside = document.createElement("div");
    inside.setAttribute("data-shelf", "");
    const outside = document.createElement("div");
    target.append(inside, outside);
    shelfUi.keys = true;
    handlePointerdown(d, outside);
    expect(shelfUi.keys).toBe(false);
    press(d, " ", { code: "Space" });
    handlePointerdown(d, inside);
    expect(shelfUi.keys).toBe(true);
    expect(sent).toEqual([{ type: "playStop" }]);
  });

  it("Esc: ためし弾き中は ShelfCancel、していなければ Back（棚をとじる）", () => {
    const sent: Command[] = [];
    press(deps(scene("shelfAudition"), sent), "Escape");
    press(deps(scene("shelfNewTrack"), sent), "Escape");
    expect(sent).toEqual([{ type: "shelfCancel" }, { type: "back" }]);
  });

  it("F2 は選んだトラックの名前の入力の枠をひらく。録音中は何もしない", () => {
    const s = scene("tracks6");
    press(deps(s, []), "F2");
    expect(trackEdit.renaming).toBe(12);
    trackEdit.renaming = null;
    s.transport.state = "recording";
    press(deps(s, []), "F2");
    expect(trackEdit.renaming).toBeNull();
  });
});

describe("トラックの見出し（モック R6-Tracks ・ R7-Arrange6）", () => {
  it("名前の入力の枠: Enter で RenameTrack（前後の空白は外す）。空や同じ名前は送らない。Esc でやめる", () => {
    show(Arrange, scene("tracks6"));
    trackEdit.renaming = 12;
    flushSync();
    let field = q("[data-test=track-name-field]") as HTMLInputElement;
    expect(field.value).toBe("ベース");
    field.value = "  ベース A  ";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    keyOn(field, "Enter");
    expect(q("[data-test=track-name-field]")).toBeNull();
    for (const typed of ["   ", "ベース"]) {
      trackEdit.renaming = 12;
      flushSync();
      field = q("[data-test=track-name-field]") as HTMLInputElement;
      field.value = typed;
      field.dispatchEvent(new Event("input", { bubbles: true }));
      keyOn(field, "Enter");
    }
    trackEdit.renaming = 12;
    flushSync();
    field = q("[data-test=track-name-field]") as HTMLInputElement;
    field.value = "やめる名前";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    keyOn(field, "Escape");
    expect(q("[data-test=track-name-field]")).toBeNull();
    expect(app.sent).toEqual([{ type: "renameTrack", track: 12, name: "ベース A" }]);
  });

  it("名前の入力の枠の外を押したら決める。Esc のあとは何も送らない", () => {
    show(Arrange, scene("tracks6"));
    trackEdit.renaming = 12;
    flushSync();
    let field = q("[data-test=track-name-field]") as HTMLInputElement;
    field.value = "外で決める";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    field.dispatchEvent(new FocusEvent("blur"));
    flushSync();
    expect(q("[data-test=track-name-field]")).toBeNull();
    trackEdit.renaming = 12;
    flushSync();
    field = q("[data-test=track-name-field]") as HTMLInputElement;
    field.value = "やめる名前";
    field.dispatchEvent(new Event("input", { bubbles: true }));
    keyOn(field, "Escape");
    field.dispatchEvent(new FocusEvent("blur"));
    expect(app.sent).toEqual([{ type: "renameTrack", track: 12, name: "外で決める" }]);
  });

  it("名前を 2 回押すと入力の枠", () => {
    show(Arrange, scene("tracks6"));
    qa(".track-name")[2]!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    flushSync();
    expect(trackEdit.renaming).toBe(13);
  });

  it("見出しのドラッグ: つまみ上げて accent の線。離すと MoveTrack を 1 つ。Esc でやめる。少しだけなら、ふつうに押した", () => {
    show(Arrange, scene("tracks6"));
    const heads = () => qa("[data-test=track-head]");
    // 1 ピアノ を 2 本ぶん下へ（size.track 64）。
    pointer(heads()[0]!, "pointerdown", 100);
    pointer(heads()[0]!, "pointermove", 130);
    expect(heads()[0]!.classList.contains("lifted")).toBe(true);
    pointer(heads()[0]!, "pointermove", 100 + 64 * 2);
    expect(q("[data-test=track-insert]")).not.toBeNull();
    pointer(heads()[0]!, "pointerup", 100 + 64 * 2);
    expect(heads()[0]!.classList.contains("lifted")).toBe(false);
    // Esc でやめる。
    pointer(heads()[3]!, "pointerdown", 300);
    pointer(heads()[3]!, "pointermove", 100);
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(q("[data-test=track-insert]")).toBeNull();
    pointer(heads()[3]!, "pointerup", 100);
    // space.1（4）より小さい動きは、押しただけ。
    pointer(heads()[4]!, "pointerdown", 300);
    pointer(heads()[4]!, "pointermove", 302);
    pointer(heads()[4]!, "pointerup", 302);
    expect(app.sent).toEqual([
      { type: "moveTrack", track: 11, to: 2 },
      { type: "selectTrack", track: 15, fromArm: false },
    ]);
  });

  it("音源の名前を押すと棚が「入れかえる」でひらく。「＋ トラックを追加」は「新しいトラック」。右の文は「音源」", () => {
    show(Arrange, scene("tracks6"));
    qa("[data-test=track-instrument]")[1]!.click();
    q("[data-test=add-track]")!.click();
    expect(text(q("[data-test=add-track]"))).toBe(`${t("arrange.addTrack")}${t("arrange.addTrackInstrument")}`);
    expect(app.sent).toEqual([
      { type: "openShelf", mode: { kind: "replace", track: 12 } },
      { type: "openShelf", mode: { kind: "newTrack" } },
    ]);
  });

  it("入れかえのためし弾き中は、そのトラックの音源の名前を accent の「（名前）（ためし弾き）」", () => {
    show(Arrange, scene("shelfReplace"));
    const names = qa("[data-test=track-instrument]");
    expect(text(names[0]!)).toBe("エレクトリックピアノ（ためし弾き）");
    expect(names[0]!.classList.contains("auditioned")).toBe(true);
    expect(text(names[1]!)).toBe("フィンガーベース");
  });
});

describe("棚（モック F4-ShelfSample ・ F4-ShelfAudition ・ F4-ShelfReplace）", () => {
  it("見出し、モードの文、さがす枠、すべて ・ お気に入り、グループと行（お気に入りの星）", () => {
    show(RightRail, scene("shelfNewTrack"));
    expect(text(q(".head"))).toBe("▾ 棚 音源 31 エフェクト 6 P");
    expect(text(q("[data-test=shelf-mode]"))).toBe("新しいトラックの音源をえらぶ");
    expect((q("[data-test=shelf-search]") as HTMLInputElement).placeholder).toBe("名前 ・ 種類");
    expect(text(q("[data-test=shelf-all]"))).toBe("すべて 31");
    expect(text(q("[data-test=shelf-favorites]"))).toBe("お気に入り 2");
    expect(qa("[data-group]").map(text)).toEqual([
      "▾ ピアノ ・ 鍵盤 11",
      "▾ ギター 4",
      "▾ ベース 4",
      "▾ ドラム 3",
      "▾ パッド ・ 弦 3",
      "▾ 管楽器 4",
      "▾ シンセ 2",
    ]);
    expect(qa("[data-row]").length).toBe(31);
    expect(qa("[aria-pressed=true][aria-label=お気に入り]").map((b) => b.closest("[data-row]")!.getAttribute("data-row"))).toEqual([
      "grandPiano",
      "studioKit",
    ]);
    // 音源のモードでは「音源」の一覧が前。「エフェクト」も押せる（見るだけ。tests/params.shelf.test.ts）。
    expect(q("[data-test=shelf-instruments]")!.classList.contains("on")).toBe(true);
    expect((q("[data-test=shelf-effects]") as HTMLButtonElement).disabled).toBe(false);
  });

  it("グループの見出しでたたむ（見出しと数は残る）。お気に入りに切りかえ。さがす", () => {
    show(RightRail, scene("shelfNewTrack"));
    q("[data-group=pianoKeys]")!.click();
    flushSync();
    expect(text(q("[data-group=pianoKeys]"))).toBe("▸ ピアノ ・ 鍵盤 11");
    expect(qa("[data-row]").length).toBe(20);
    q("[data-test=shelf-favorites]")!.click();
    flushSync();
    expect(qa("[data-group]").map(text)).toEqual(["▸ ピアノ ・ 鍵盤 1", "▾ ドラム 1"]);
    shelfUi.favoritesOnly = false;
    shelfUi.folded = [];
    shelfUi.query = "あたたか";
    flushSync();
    expect(qa("[data-row]").map((r) => r.getAttribute("data-row"))).toEqual(["warmPad"]);
  });

  it("行を押すと ShelfSelect、星は ToggleFavorite、▶ は PlaySample（行はえらばない）", () => {
    show(RightRail, scene("shelfNewTrack"));
    q("[data-row=organ]")!.click();
    q("[data-row=organ] [aria-label=お気に入り]")!.click();
    q("[data-row=organ] [data-test=shelf-sample]")!.click();
    expect(app.sent).toEqual([
      { type: "shelfSelect", instrument: "organ" },
      { type: "toggleFavorite", instrument: "organ" },
      { type: "playSample", instrument: "organ" },
    ]);
  });

  it("ためし弾き中（新しいトラック）: えらんだ行の下に案内と「トラック 5 を作る Enter」「やめる Esc」", () => {
    show(RightRail, scene("shelfAudition"));
    expect(q("[data-row=synthBass]")!.classList.contains("selected")).toBe(true);
    const box = q("[data-test=shelf-audition]")!;
    expect(box.previousElementSibling).toBe(q("[data-row=synthBass]"));
    expect(text(box)).toBe(`ためし弾き中 ${t("msg.audition")} トラック 5 を作る Enter やめる Esc`);
    q("[data-test=shelf-confirm]")!.click();
    q("[data-test=shelf-cancel]")!.click();
    expect(app.sent).toEqual([{ type: "shelfConfirm" }, { type: "shelfCancel" }]);
  });

  it("入れかえ: 「1 ピアノ の音源を入れかえる」、いまの音源の行に「いまの音源」、ボタンは「入れかえる Enter」", () => {
    show(RightRail, scene("shelfReplace"));
    expect(text(q("[data-test=shelf-mode]"))).toBe("1 ピアノ の音源を入れかえる");
    expect(text(q("[data-row=grandPiano]"))).toBe("グランドピアノ いまの音源");
    expect(text(q("[data-test=shelf-audition]"))).toBe(`ためし弾き中 ${t("msg.auditionReplace")} 入れかえる Enter やめる Esc`);
  });

  it("サンプル再生中: ▶ が accent の停止になり、右に「0:00 / 0:04」", () => {
    const s = scene("shelfNewTrack");
    s.shelf.sample = { instrument: "grandPiano", startedUs: 0, lengthUs: 4_000_000 };
    show(RightRail, s);
    expect(q("[data-row=grandPiano] [data-test=shelf-sample]")!.classList.contains("playing")).toBe(true);
    expect(text(q("[data-test=shelf-sample-time]"))).toMatch(/^0:0\d \/ 0:04$/);
    q("[data-row=grandPiano] [data-test=shelf-sample]")!.click();
    expect(app.sent).toEqual([{ type: "stopSample" }]);
  });

  it("shelf84（モック R8-Shelf84）: 31 の音色、グループ 7 つ。ピアノ ・ 鍵盤とギターのほかをたたむ", () => {
    show(RightRail, scene("shelf84"));
    expect(text(q(".head"))).toBe("▾ 棚 音源 31 エフェクト 6 P");
    expect(text(q("[data-test=shelf-all]"))).toBe("すべて 31");
    for (const g of ["bass", "drums", "padStrings", "wind", "synth"]) q(`[data-group=${g}]`)!.click();
    flushSync();
    expect(qa("[data-group]").map(text)).toEqual([
      "▾ ピアノ ・ 鍵盤 11",
      "▾ ギター 4",
      "▸ ベース 4",
      "▸ ドラム 3",
      "▸ パッド ・ 弦 3",
      "▸ 管楽器 4",
      "▸ シンセ 2",
    ]);
    expect(qa("[data-row]").map((r) => r.getAttribute("data-row"))).toEqual([
      ...INSTRUMENT_GROUPS[0]!.instruments,
      ...INSTRUMENT_GROUPS[1]!.instruments,
    ]);
    shelfUi.folded = [];
  });

  it("とじた棚の縦の見出しの数は、棚に出る音源の数", () => {
    show(RightRail, scene("tracks6"));
    expect(text(q("[data-band=shelf]"))).toContain("音源 31 ・ エフェクト 6");
    q("[data-band=shelf]")!.click();
    expect(app.sent).toEqual([{ type: "openShelf", mode: { kind: "newTrack" } }]);
  });
});

describe("偽物の答え（約束 13）", () => {
  const run = (state: AppState, c: Command) => reduceTracks(state, new FakeBands(800), c)!;

  it("消す: 下のトラックが選ばれ、番号をふりなおし、状態行は消した番号", () => {
    const s = scene("tracks6");
    const r = run(s, { type: "deleteTrack", track: 12 });
    expect(r.song.tracks.map((x) => [x.number, x.name])).toEqual([
      [1, "ピアノ"],
      [2, "ドラム"],
      [3, "パッド"],
      [4, "シンセ"],
      [5, "ベース 2"],
    ]);
    expect(r.song.playTarget).toBe(13);
    expect(r.notice).toEqual({ id: "trackDeleted", track: { number: 2, name: "ベース" } });
  });

  it("並べかえ: 番号はふりなおし、色はトラックについたまま。色は 1〜5 だけ", () => {
    const s = scene("tracks6");
    const r = run(s, { type: "moveTrack", track: 11, to: 2 });
    expect(r.song.tracks.map((x) => [x.number, x.name, x.color])).toEqual([
      [1, "ベース", 2],
      [2, "ドラム", 3],
      [3, "ピアノ", 1],
      [4, "パッド", 4],
      [5, "シンセ", 5],
      [6, "ベース 2", 1],
    ]);
    expect(run(s, { type: "setTrackColor", track: 11, color: 4 }).song.tracks[0]!.color).toBe(4);
    expect(reduceTracks(s, new FakeBands(800), { type: "setTrackColor", track: 11, color: 6 })).toBeNull();
  });

  it("新しいトラック: 選んだトラックのすぐ下、グループの短い名前（同じ名前があれば 2 から空いた番号）、色は場所の色", () => {
    const s = scene("tracks6");
    s.shelf.selected = "fingerBass";
    const r = run(s, { type: "shelfConfirm" });
    expect(r.song.tracks.map((x) => [x.number, x.name, x.color])[2]).toEqual([3, "ベース 3", 3]);
    expect(r.song.playTarget).toBe(r.song.tracks[2]!.id);
    expect(r.notice).toEqual({ id: "trackAdded", track: { number: 3, name: "ベース 3" } });
    expect(r.shelf.selected).toBeNull();
  });

  it("流れ: 棚をひらく → えらぶ（録音できない、状態行はためし弾き）→ やめる → とじる", async () => {
    const transport = createFakeTransport(scene("tracks6"));
    const patches: AppState[] = [];
    let state = (await transport.snapshot()).state;
    await transport.subscribe(
      (m) => {
        if (m.kind !== "patch") return;
        state = { ...state };
        for (const c of m.changes) (state as unknown as Record<string, unknown>)[c.section] = c.value;
        patches.push(state);
      },
      () => {},
    );
    await transport.send({ type: "openShelf", mode: { kind: "replace", track: 11 } });
    expect(state.workspace.layout.rightFront).toBe("shelf");
    expect(state.shelf.current).toBe("grandPiano");
    await transport.send({ type: "shelfSelect", instrument: "organ" });
    expect(state.transport.canRecord).toBe(false);
    expect(state.status.message).toEqual({ id: "auditionReplace", instrument: "organ", track: { number: 1, name: "ピアノ" }, current: "grandPiano" });
    await transport.send({ type: "shelfConfirm" });
    expect(state.song.tracks[0]!.instrument).toBe("organ");
    expect(state.status.message).toEqual({ id: "instrumentReplaced", track: { number: 1, name: "ピアノ" }, instrument: "organ" });
    await transport.send({ type: "toggleBand", band: "shelf" });
    expect(state.workspace.layout.rightOpen).toEqual([]);
    expect(state.shelf.mode).toEqual({ kind: "newTrack" });
    expect(patches.length).toBeGreaterThan(0);
  });
});

describe("状態行と入力計（モック F4-ShelfAudition ・ F4-ShelfReplace ・ R7-Arrange6）", () => {
  const mountOf = (C: Component<any>, state: AppState) => {
    app.setState(state);
    mounted = mount(C, { target });
    flushSync();
  };

  it("ためし弾き中: 左の札は accent の「♪ ためし弾き中」、文は status.auditionNewTrack", () => {
    mountOf(StatusLine, scene("shelfAudition"));
    const badge = q("[data-test=status-badge]")!;
    expect(text(badge)).toBe("♪ ためし弾き中");
    expect(badge.classList.contains("accent")).toBe(true);
    expect(text(q(".message"))).toBe("シンセベース ・ まだトラックには入っていません ・ 入れるには Enter");
  });

  it("入れかえ: status.auditionReplace", () => {
    mountOf(StatusLine, scene("shelfReplace"));
    expect(text(q(".message"))).toBe("エレクトリックピアノ ・ 1 ピアノ の音源は、まだグランドピアノです ・ 入れかえるには Enter");
  });

  it("棚が音源の一覧でひらいていれば、まとめの代わりに「音色 31 ・ …をひらいています」（モック R8-Shelf84）", () => {
    const s = scene("shelf84");
    mountOf(StatusLine, s);
    const say = () => (flushSync(), text(q(".message")));
    shelfUi.folded = ["bass", "drums", "padStrings", "wind", "synth"];
    expect(say()).toBe("音色 31 ・ ピアノ ・ 鍵盤とギターをひらいています");
    shelfUi.folded = ["guitar", "bass", "drums", "padStrings", "wind", "synth"];
    expect(say()).toBe("音色 31 ・ ピアノ ・ 鍵盤をひらいています");
    shelfUi.folded = ["bass", "drums", "padStrings", "synth"];
    expect(say()).toBe("音色 31 ・ ピアノ ・ 鍵盤、ギターと管楽器をひらいています");
    shelfUi.folded = ["pianoKeys", "guitar", "bass", "drums", "padStrings", "wind", "synth"];
    expect(say()).toBe("音色 31");
    // 数は棚に出ている行の数。
    shelfUi.folded = [];
    shelfUi.query = "オルガン";
    expect(say()).toBe("音色 7 ・ ピアノ ・ 鍵盤をひらいています");
    shelfUi.query = "";
    // エフェクトの一覧、ためし弾き、サンプル再生中、棚がとじているときは、いつもの文。
    shelfUi.view = "effects";
    expect(say()).toBe("ループ 5–9 ・ 選択なし");
    shelfUi.view = "instruments";
    app.setState({ ...s, shelf: { ...s.shelf, sample: { instrument: "grandPiano", startedUs: 0, lengthUs: 4_000_000 } } });
    expect(say()).not.toContain("音色");
    app.setState({ ...s, workspace: { ...s.workspace, layout: { ...s.workspace.layout, rightOpen: [], rightFront: null } } });
    expect(say()).toBe("ループ 5–9 ・ 選択なし");
    // ミュートの文は、いつもどおりうしろにつく。
    app.setState({ ...s, song: { ...s.song, tracks: s.song.tracks.map((x, i) => ({ ...x, muted: i === 3 })) } });
    expect(say()).toBe("音色 31 ・ ピアノ ・ 鍵盤、ギター、ベース、ドラム、パッド ・ 弦、管楽器とシンセをひらいています ・ 4 パッド はミュート");
  });

  it("● で別のトラック: 「R を押すと 2 ベース に録ります」", () => {
    mountOf(StatusLine, scene("tracks6"));
    expect(text(q(".message"))).toBe("R を押すと 2 ベース に録ります");
  });

  it("知らせの文: 足した ・ 消した ・ 入れかえた ・ サンプル再生中", () => {
    const s = scene("tracks6");
    mountOf(StatusLine, s);
    const say = (message: AppState["status"]["message"]) => {
      app.setState({ ...s, status: { message } });
      flushSync();
      return text(q(".message"));
    };
    const track = { number: 3, name: "ドラム" };
    expect(say({ id: "trackAdded", track })).toBe("3 ドラム を足しました。");
    expect(say({ id: "trackDeleted", track })).toBe("3 ドラム を消しました。取り消せます（Ctrl Z）");
    expect(say({ id: "instrumentReplaced", track, instrument: "electroKit" })).toBe(
      "3 ドラム の音源を エレクトロキット にしました。取り消せます（Ctrl Z）",
    );
    expect(say({ id: "samplePlaying", instrument: "grandPiano" })).toBe("サンプルを再生中 ・ グランドピアノ。行をえらぶと、鍵盤でためし弾きできます。");
    expect(say({ id: "instrumentLoading", instrument: "grandPiano" })).toBe("グランドピアノを読みこんでいます… 少しだけ待つと、鍵盤で鳴らせます。");
  });

  it("入力計の弾くと鳴る先: ふだんは選んだトラック、ためし弾き中は accent の「ためし弾き ・ シンセベース」", () => {
    mountOf(InputMeter, scene("tracks6"));
    // jsdom は幅を測らないので、音源の名前は引っこめた形になる（引っこめる順は meter-fit の試験で見る）。
    expect(text(q("[data-test=meter-target]"))).toMatch(/^2 ベース/);
    app.setState(scene("shelfAudition"));
    flushSync();
    const el = q("[data-test=meter-target]")!;
    expect(text(el)).toBe("ためし弾き ・ シンセベース");
    expect(el.classList.contains("accent")).toBe(true);
  });
});

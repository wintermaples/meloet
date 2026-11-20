// 第 5 版の「選んだノート」と一覧（design-notes C-6 50 ・ 51 ・ 58 ・ 67）: 長さをそろえる、かける / かけなおす、
// 音の欄の Shift 12、ひらいた「CC の一覧」。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, TakeNotes } from "../src/bridge";
import { CONTROL_CC, CONTROL_PITCH_BEND, decodeTake } from "../src/bridge/take";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import CcListPanel from "../src/components/pianoroll/CcListPanel.svelte";
import { ccGroups, ccLines, ccRow, commitControl, controlFieldSpec, controlName, controlOf, neighborPoint } from "../src/components/pianoroll/ccFields";
import NoteInspector from "../src/components/pianoroll/NoteInspector.svelte";
import { noteFieldSpec, readField, stepField } from "../src/components/pianoroll/noteFields";
import { ccList, inspectorFolds } from "../src/components/pianoroll/view.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import * as app from "./fake-app.svelte";
import * as shown from "./fake-shown.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

const METER = { num: 4, den: 4 };

const proto = HTMLElement.prototype;
const had = Object.getOwnPropertyDescriptor(proto, "clientHeight");
beforeAll(() => Object.defineProperty(proto, "clientHeight", { configurable: true, get: () => 200 }));
afterAll(() => had && Object.defineProperty(proto, "clientHeight", had));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  inspectorFolds.sameLength = false;
  inspectorFolds.velocityAll = false;
  ccList.folded = [];
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
});

const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const key = (el: Element, k: string, init: KeyboardEventInit = {}) =>
  el.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...init }));
function typeInto(input: HTMLInputElement, value: string): void {
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  flushSync();
}

async function scene(name: string, edit?: (s: AppState) => void, editTake?: (t: TakeNotes) => void): Promise<{ s: AppState; take: TakeNotes }> {
  const s = fakeScene(name)!.state;
  edit?.(s);
  const track = s.song.tracks[0]!;
  const clip = track.clips.find((c) => c.id === s.arrange.pianoRollClip)!;
  const take = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
  editTake?.(take);
  app.setState(s);
  shown.setShown({ clip, track, take });
  return { s, take };
}

function show(component: Component): void {
  mounted = mount(component, { target });
  flushSync();
}

describe("長さをそろえる（C-6 51）", () => {
  it("▸ gray.8 でたたんで始まり、押すと ▾ と「まとめて変える行」。ばらばらなら空。Enter かボタンで matchLength", async () => {
    const { take } = await scene("pianoRoll");
    show(NoteInspector);
    const head = q('[data-test="inspector-same-length"]')!;
    expect(text(head)).toBe("▸ 長さをそろえる");
    expect(head.firstElementChild!.classList.contains("m-lbl")).toBe(true);
    expect(q('[data-test="inspector-same-length-field"]')).toBeNull();
    head.click();
    flushSync();
    expect(text(head)).toBe("▾ 長さをそろえる");
    expect(head.firstElementChild!.classList.contains("m-hi")).toBe(true);
    const row = q(".bulk-row")!;
    expect(text(row)).toBe("長さ そろえる");
    const input = q<HTMLInputElement>('[data-test="inspector-same-length-field"]')!;
    const button = q('[data-test="inspector-match-length"]')!;
    expect(input.value).toBe("");
    expect(button.getAttribute("aria-disabled")).toBe("false");

    typeInto(input, "0.240");
    key(input, "Enter");
    expect(app.sent).toEqual([{ type: "matchLength", take: take.id, notes: [14, 15, 16], value: 240 }]);
    expect(input.value).toBe("0.240");
    app.sent.length = 0;

    // 拍 . ティックのティックは 0〜479。ほかの欄と同じく近いはしにして、状態行で言う。
    typeInto(input, "1.999");
    button.click();
    expect(app.sent).toEqual([
      { type: "matchLength", take: take.id, notes: [14, 15, 16], value: 959 },
      { type: "clampValue", field: "tick", value: "1.479", range: "0〜479" },
    ]);
    app.sent.length = 0;

    typeInto(input, "あ");
    expect(input.classList.contains("broken")).toBe(true);
    button.click();
    expect(app.sent).toEqual([{ type: "rejectValue", field: "length", kept: "ばらばら", range: expect.any(String) }]);
  });

  it("みな同じならその値。↑ ↓ は表示のいちばん下のけた。クリップの外まではのばさない。選んでいなければ使えない", async () => {
    const { take } = await scene("pianoRoll", (s) => (s.pianoRoll.selectedNotes = [{ start: 14, end: 15 }]));
    inspectorFolds.sameLength = true;
    show(NoteInspector);
    const input = q<HTMLInputElement>('[data-test="inspector-same-length-field"]')!;
    expect(input.value).toBe("0.360");
    key(input, "ArrowUp");
    flushSync();
    expect(input.value).toBe("0.361");
    key(input, "ArrowDown", { shiftKey: true });
    flushSync();
    expect(input.value).toBe("0.351");
    typeInto(input, "9999");
    key(input, "Enter");
    const sent = app.sent[0] as { type: string; value: number };
    expect(sent.type).toBe("matchLength");
    expect(app.sent[1]).toMatchObject({ type: "clampValue", field: "length" });
    const clip = shown.shown.clip!;
    expect(clip.start + clip.takeOffset + take.starts[14]! + sent.value).toBe(clip.end);
    unmount(mounted!);
    mounted = null;
    app.sent.length = 0;

    await scene("pianoRoll", (s) => (s.pianoRoll.selectedNotes = []));
    show(NoteInspector);
    const off = q<HTMLInputElement>('[data-test="inspector-same-length-field"]')!;
    expect(off.disabled).toBe(true);
    expect(q(".bulk-row .bulk-label")!.classList.contains("off")).toBe(true);
    q('[data-test="inspector-match-length"]')!.click();
    expect(app.sent).toEqual([]);
  });

  it("状態行「3 ノートの長さを 0.240 にそろえました。」（仕様書 6 章「まとめてそろえた」）", async () => {
    await scene("lengthMatched");
    show(StatusLine);
    expect(text(q(".message"))).toBe("3 ノートの長さを 0.240 にそろえました。");
  });
});

describe("まとめて変える行の寸法（C-6 67）", () => {
  it("長さとベロシティの行は同じ部品（字 ・ 入力の枠 ・ そろえる）。寸法は絵で見る", async () => {
    await scene("pianoRoll");
    inspectorFolds.sameLength = true;
    inspectorFolds.velocityAll = true;
    show(NoteInspector);
    const rows = qa(".bulk-row");
    expect(rows.map(text)).toEqual(["長さ そろえる", "ベロシティ そろえる"]);
    for (const r of rows) {
      expect(r.querySelector(".bulk-label")).not.toBeNull();
      expect(r.querySelector("input.bulk-field")).not.toBeNull();
    }
  });
});

describe("かける と かけなおす（C-6 58）", () => {
  it("まだかけていないテイクは「かける」で、もとの位置へ戻すは使えない。かけたあとは「かけなおす」", async () => {
    await scene("pianoRoll", undefined, (take) => (take.origin = 0));
    show(NoteInspector);
    const buttons = qa(".buttons .m-btn");
    expect(buttons.map(text)).toEqual(["かけるQ", "もとの位置へ戻す"]);
    expect(buttons.map((b) => b.getAttribute("aria-disabled"))).toEqual(["false", "true"]);
    unmount(mounted!);
    mounted = null;
    await scene("pianoRoll");
    show(NoteInspector);
    expect(qa(".buttons .m-btn").map(text)).toEqual(["かけなおすQ", "もとの位置へ戻す"]);
  });
});

describe("音の欄の Shift ↑ ↓ は 12 半音（C-6 58）", () => {
  it("音は 12、ほかの欄は 10", async () => {
    const { take } = await scene("pianoRoll");
    const clip = shown.shown.clip!;
    const keySpec = noteFieldSpec("key", take, clip, [14], METER)!;
    expect(keySpec.shift).toBe(12);
    expect(stepField(keySpec, 60, keySpec.shift)).toBe(72);
    for (const name of ["position", "length", "velocity"] as const) expect(noteFieldSpec(name, take, clip, [14], METER)!.shift).toBe(10);
  });
});

// ---- CC の一覧（C-6 50、モック R5-Lists） ----

/** CC の点を 4 つ（サステイン踏む ・ サステイン離す ・ ピッチベンド ・ CC 7）にしたテイク。 */
function withControls(take: TakeNotes): void {
  take.controlTicks = Uint32Array.from([0, (7 * TPQ) / 4, 2 * TPQ, (5 * TPQ) / 2]);
  take.controlKinds = Uint8Array.from([CONTROL_CC, CONTROL_CC, CONTROL_PITCH_BEND, CONTROL_CC]);
  take.controlNumbers = Uint8Array.from([64, 64, 0, 7]);
  take.controlValues = Int16Array.from([127, 0, 2048, 100]);
}

/** tick の順で CC 7 ・ モジュレーション ・ ピッチベンド ・ サステイン ・ CC 11 ・ サステイン ・ ピッチベンド。 */
function mixedControls(take: TakeNotes): void {
  take.controlTicks = Uint32Array.from([0, 1, 2, 3, 4, 5, 6].map((i) => (i * 5 * TPQ) / 24));
  take.controlKinds = Uint8Array.from([CONTROL_CC, CONTROL_CC, CONTROL_PITCH_BEND, CONTROL_CC, CONTROL_CC, CONTROL_CC, CONTROL_PITCH_BEND]);
  take.controlNumbers = Uint8Array.from([7, 1, 0, 64, 11, 64, 0]);
  take.controlValues = Int16Array.from([100, 64, 0, 127, 90, 0, 2048]);
}

describe("CC の一覧の行と欄（DOM なし）", () => {
  it("種類は帯と同じ名前、値は帯の読み値と同じ書きかた", async () => {
    const { take } = await scene("pianoRoll", undefined, withControls);
    const clip = shown.shown.clip!;
    const rows = [0, 1, 2, 3].map((i) => ccRow(take, clip, i, METER));
    expect(rows.map((r, i) => [controlName(controlOf(take, i)), r.value])).toEqual([
      ["サステイン", "踏む（127）"],
      ["サステイン", "OFF（0）"],
      ["ピッチベンド", "+2048"],
      ["CC 7", "100"],
    ]);
  });

  it("種類ごとのまとまりは CC の札と同じ順（サステイン、ピッチベンド、モジュレーション、そのほかは番号の順）。テイクにない種類は出さない", async () => {
    const { take } = await scene("pianoRoll", undefined, mixedControls);
    const groups = ccGroups(take);
    expect(groups.map((g) => [g.name, g.points])).toEqual([
      ["サステイン", [3, 5]],
      ["ピッチベンド", [2, 6]],
      ["モジュレーション", [1]],
      ["CC 7", [0]],
      ["CC 11", [4]],
    ]);
    const { lines, lineOf } = ccLines(groups, ["pitchBend", "cc7"], 7);
    expect([...lines]).toEqual([-1, 3, 5, -2, -3, 1, -4, -5, 4]);
    expect([...lineOf]).toEqual([-1, 5, -1, 1, 8, 2, -1]);
    expect(neighborPoint(lines, 2, 1)).toBe(1);
    expect(neighborPoint(lines, 5, -1)).toBe(5);
    expect(neighborPoint(lines, 8, 1)).toBeNull();
  });

  it("値: 範囲の外は近いはし。オン ・ オフの CC は 0 か 127 にだけ止まる。決めると moveControls 1 つ", async () => {
    const { take } = await scene("pianoRoll", undefined, withControls);
    const clip = shown.shown.clip!;
    const pb = controlFieldSpec("value", take, clip, 2, METER)!;
    expect(pb.initial).toBe("+2048");
    expect(readField(pb, "9000")).toEqual({ value: 8191, clamped: { field: "controlValue", value: "+8191", range: "−8192〜+8191" } });
    expect(readField(pb, "−100")?.value).toBe(-100);
    expect(commitControl("value", pb, -100, take, 2)).toEqual({ type: "moveControls", take: take.id, points: [2], ticks: 0, value: -2148 });
    const sustain = controlFieldSpec("value", take, clip, 1, METER)!;
    expect(commitControl("value", sustain, 50, take, 1)).toBeNull();
    expect(commitControl("value", sustain, 90, take, 1)).toEqual({ type: "moveControls", take: take.id, points: [1], ticks: 0, value: 127 });
    expect(stepField(sustain, 0, 1)).toBe(127);
    const cc7 = controlFieldSpec("value", take, clip, 3, METER)!;
    expect(readField(cc7, "200")).toEqual({ value: 127, clamped: { field: "controlValue", value: "127", range: "0〜127" } });
    const pos = controlFieldSpec("position", take, clip, 3, METER)!;
    const at = clip.start + clip.takeOffset + (5 * TPQ) / 2;
    expect(commitControl("position", pos, at + TPQ, take, 3)).toEqual({ type: "moveControls", take: take.id, points: [3], ticks: TPQ, value: 0 });
  });
});

describe("ひらいた CC の一覧（mount）", () => {
  it("列の見出し 位置 ・ 値、種類ごとの見出しの下に点を位置の順に。選んだ行は selected。押すと selectControls", async () => {
    const { take } = await scene(
      "pianoRoll",
      (s) => (s.pianoRoll.selectedControls = [{ start: 2, end: 3 }]),
      withControls,
    );
    show(CcListPanel);
    expect(text(q(".head"))).toBe("位置 値");
    const rows = qa(".row");
    expect(rows.map((r) => text(r)!.split(" ").slice(1).join(" "))).toEqual(["踏む（127）", "OFF（0）", "+2048", "100"]);
    expect(rows.map((r) => r.classList.contains("selected"))).toEqual([false, false, true, false]);
    // 見出しと点の行は、上から順に同じ高さで重ならない。
    const order = qa(".group, .row").map((el) => [el.dataset.group ?? el.dataset.point, el.style.transform]);
    expect(order).toEqual([
      ["cc64", "translateY(0px)"],
      ["0", "translateY(20px)"],
      ["1", "translateY(40px)"],
      ["pitchBend", "translateY(60px)"],
      ["2", "translateY(80px)"],
      ["cc7", "translateY(100px)"],
      ["3", "translateY(120px)"],
    ]);
    rows[3]!.dispatchEvent(new MouseEvent("click", { bubbles: true, shiftKey: true }));
    expect(app.sent).toEqual([{ type: "selectControls", take: take.id, points: [3], mode: "add" }]);
  });

  it("Enter で位置の枠、Tab で値の枠（種類はとばす）、Enter で決める。Esc でやめる", async () => {
    const { take } = await scene("pianoRoll", undefined, withControls);
    show(CcListPanel);
    const row = qa(".row")[2]!;
    key(row, "Enter");
    flushSync();
    let input = q<HTMLInputElement>("input")!;
    expect(input.dataset.test).toBe("cc-list-field");
    key(input, "Tab");
    flushSync();
    input = q<HTMLInputElement>("input")!;
    expect(input.value).toBe("+2048");
    key(input, "ArrowUp", { shiftKey: true });
    flushSync();
    expect(input.value).toBe("+2058");
    key(input, "Enter");
    flushSync();
    expect(app.sent).toEqual([{ type: "moveControls", take: take.id, points: [2], ticks: 0, value: 10 }]);
    expect(q("input")).toBeNull();
    app.sent.length = 0;

    qa(".row")[0]!.querySelector('[data-field="value"]')!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    flushSync();
    input = q<HTMLInputElement>("input")!;
    expect(input.value).toBe("127");
    typeInto(input, "300");
    key(input, "Escape");
    flushSync();
    expect(q("input")).toBeNull();
    expect(app.sent).toEqual([]);
  });

  it("点が何千あっても、見えている行だけ作る", async () => {
    const n = 5000;
    await scene("pianoRoll", undefined, (take) => {
      take.controlTicks = Uint32Array.from({ length: n }, (_, i) => i);
      take.controlKinds = new Uint8Array(n).fill(CONTROL_CC);
      take.controlNumbers = new Uint8Array(n).fill(1);
      take.controlValues = new Int16Array(n).fill(64);
    });
    show(CcListPanel);
    expect(qa(".row").length).toBeLessThanOrEqual(12);
    expect(q(".rows")!.style.height).toBe(`${(n + 1) * 20}px`);
  });

  it("種類の見出し: ▾ 名前 数。押すとたたみ（▸ gray.8）、もう一度でひらく。たたんだ状態は一覧をとじても覚えている", async () => {
    await scene("pianoRoll", undefined, mixedControls);
    show(CcListPanel);
    const heads = () => qa(".group").map((g) => text(g));
    expect(heads()).toEqual(["▾ サステイン 2", "▾ ピッチベンド 2", "▾ モジュレーション 1", "▾ CC 7 1", "▾ CC 11 1"]);
    expect(qa(".group").every((g) => g.getAttribute("aria-expanded") === "true")).toBe(true);
    expect(q(".rows")!.style.height).toBe(`${12 * 20}px`);

    q('[data-group="pitchBend"]')!.click();
    flushSync();
    const pb = q('[data-group="pitchBend"]')!;
    expect(pb.getAttribute("aria-expanded")).toBe("false");
    expect(pb.firstElementChild!.textContent).toBe("▸");
    expect(pb.firstElementChild!.classList.contains("m-lbl")).toBe(true);
    expect(q('[data-group="cc64"]')!.firstElementChild!.classList.contains("m-hi")).toBe(true);
    expect(qa(".row").map((r) => r.dataset.point)).toEqual(["3", "5", "1", "0", "4"]);
    expect(q(".rows")!.style.height).toBe(`${10 * 20}px`);
    expect(ccList.folded).toEqual(["pitchBend"]);
    expect(app.sent).toEqual([]);

    unmount(mounted!);
    show(CcListPanel);
    expect(heads()[1]).toBe("▸ ピッチベンド 2");
    expect(qa(".row").map((r) => r.dataset.point)).toEqual(["3", "5", "1", "0", "4"]);

    q('[data-group="pitchBend"]')!.click();
    flushSync();
    expect(heads()[1]).toBe("▾ ピッチベンド 2");
    // 窓は 200（10 行）なので、11 行目までを作る。
    expect(qa(".row").map((r) => r.dataset.point)).toEqual(["3", "5", "2", "6", "1", "0"]);
    expect(ccList.folded).toEqual([]);
  });

  it("Tab で行を移るとき、見出しと、たたんだ種類の行はとばす", async () => {
    await scene("pianoRoll", undefined, mixedControls);
    ccList.folded = ["pitchBend"];
    show(CcListPanel);
    const at = () => q<HTMLInputElement>("input")?.closest<HTMLElement>("[data-point]")?.dataset.point;
    q('[data-point="5"] [data-field="value"]')!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    flushSync();
    expect(at()).toBe("5");
    key(q("input")!, "Tab");
    flushSync();
    expect(at()).toBe("1");
    expect(q('[data-point="1"] [data-field="value"]')).not.toBeNull();
    key(q("input")!, "Tab", { shiftKey: true });
    flushSync();
    expect(at()).toBe("5");
    expect(q<HTMLInputElement>("input")!.value).toBe("0");
  });
});

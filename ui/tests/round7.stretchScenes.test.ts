// Screen part: 「のび縮み ・ 音の高さ」 in the panel of the chosen clip (spec 3 (stretch, pitch, while the sound is made),
// mocks F7-Stretch / F7-AudioEdit), the clip mark text, the status line, the place in the context menu, and the copied sources in 「meloet について」.
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, StatusMessage } from "../src/bridge";
import { fakeScene, type FakeState } from "../src/bridge/fake";
import { allParts, partGroups, type LicenseList } from "../src/components/about/licenses";
import { centsText, clipMarks, dragSteps, NO_MARKS, parseSourceBpm, parseWhole, ratioMarkValue } from "../src/components/params/clipFields";
import ParamsPanel from "../src/components/params/ParamsPanel.svelte";
import { cancelDrag, dragReading } from "../src/components/pianoroll/view.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

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
});
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const scene = (name: string) => fakeScene(name)!.state as FakeState;
function panel(state: AppState): void {
  app.setState(state);
  mounted = mount(ParamsPanel, { target, props: { onfold: () => {} } });
  flushSync();
}
const rows = () => ["follow", "source-tempo", "pitch", "keepFormant"].map((k) => text(q(`[data-test="clip-${k}"]`)));
function pointer(el: Element, type: string, clientY: number): void {
  el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX: 0, clientY, button: 0 }));
  flushSync();
}
function type(field: string, value: string): void {
  q(`[data-test="clip-value-${field}"]`)!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
  flushSync();
  const input = q<HTMLInputElement>('[data-test="param-input"]')!;
  input.value = value;
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
  flushSync();
}

describe("the 4 rows of stretch and pitch (protocol 25 scenes)", () => {
  it.each([
    ["stretch", ["曲のテンポについていく オン", "この音のテンポ 76.80", "音の高さ +2 半音 0 セント", "声の感じを保つ オン"]],
    ["stretchRendering", ["曲のテンポについていく オン", "この音のテンポ 76.80", "音の高さ +2 半音 0 セント", "声の感じを保つ オン"]],
    ["stretchPitch", ["曲のテンポについていく オン", "この音のテンポ 96.00", "音の高さ −1 半音 30 セント", "声の感じを保つ オフ"]],
    ["stretchNoTempo", ["曲のテンポについていく オフ", "この音のテンポ さがす —", "音の高さ +0 半音 0 セント", "声の感じを保つ オン"]],
    ["stretchRamp", ["曲のテンポについていく オン", "この音のテンポ 96.00", "音の高さ +0 半音 0 セント", "声の感じを保つ オン"]],
  ])("%s", (name, want) => {
    panel(scene(name));
    expect(rows()).toEqual(want);
    expect(target.querySelectorAll(".row.unusable")).toHaveLength(0);
  });

  it("on / off is pressing the 12×12 square mark, one SetClipStretch", () => {
    const s = scene("stretch");
    panel(s);
    expect(q('[data-test="clip-switch-follow"]')!.getAttribute("aria-checked")).toBe("true");
    q('[data-test="clip-switch-follow"]')!.click();
    q('[data-test="clip-switch-keepFormant"]')!.click();
    expect(app.sent).toEqual([
      { type: "setClipStretch", clip: s.params.selected!.kind === "clip" ? s.params.selected!.clip : -1, change: { kind: "follow", on: false } },
      { type: "setClipStretch", clip: s.params.selected!.kind === "clip" ? s.params.selected!.clip : -1, change: { kind: "keepFormant", on: false } },
    ]);
  });

  it("source tempo: press twice and type. Unreadable text gives RejectValue (the tempo range text). The same number is not sent", () => {
    panel(scene("stretch"));
    type("sourceBpm", "120");
    type("sourceBpm", "76.80");
    type("sourceBpm", "はやい");
    expect(app.sent.map((c) => (c.type === "setClipStretch" ? c.change : c))).toEqual([
      { kind: "sourceBpm", bpm: 120 },
      { type: "rejectValue", field: "tempo", kept: "76.80", range: null },
    ]);
  });

  it("「さがす」 only with 「—」: pressing sends FindSourceTempo. The fake puts in the found number and says so in the status line", () => {
    const s = scene("stretchNoTempo");
    panel(s);
    q('[data-test="clip-find-tempo"]')!.click();
    expect(app.sent).toEqual([{ type: "findSourceTempo", clip: s.params.selected!.kind === "clip" ? s.params.selected!.clip : -1 }]);
    unmount(mounted!);
    mounted = null;
    panel(scene("stretch"));
    expect(q('[data-test="clip-find-tempo"]')).toBeNull();
  });

  it("pitch: dragging semitones up moves only the text first, 1 step on release. Esc cancels and sends nothing", () => {
    panel(scene("stretch"));
    const semis = q('[data-test="clip-value-semitones"]')!;
    pointer(semis, "pointerdown", 100);
    pointer(semis, "pointermove", 100 - 3 * 8);
    expect(text(semis)).toBe("+5");
    expect(app.sent).toEqual([]);
    // The status line and the reading in place (spec 6 「音の高さを動かしている」).
    expect(dragReading.text).toBe("音の高さ +5 半音 +0 セント（ドラッグ中）");
    expect(text(q('[data-test="clip-value-semitones-reading"]'))).toBe("+5 ・ +0");
    pointer(semis, "pointerup", 100 - 3 * 8);
    expect(dragReading.text).toBeNull();
    expect(q('[data-test="clip-value-semitones-reading"]')).toBeNull();
    expect(app.sent.map((c) => (c.type === "setClipStretch" ? c.change : c))).toEqual([{ kind: "semitones", semitones: 5 }]);
    // Stops at the range ends. Esc returns to the original value.
    const cents = q('[data-test="clip-value-cents"]')!;
    pointer(cents, "pointerdown", 0);
    pointer(cents, "pointermove", 1000);
    expect(text(cents)).toBe("−50");
    expect(dragReading.text).toBe("音の高さ +2 半音 −50 セント（ドラッグ中）");
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(text(cents)).toBe("0");
    expect(dragReading.text).toBeNull();
    pointer(cents, "pointerup", 1000);
    expect(app.sent).toHaveLength(1);
  });

  it("pitch: semitones and cents typed separately (also reads −, +). Rust clamps out of range to the ends (same as B-94-29)", () => {
    panel(scene("stretch"));
    type("semitones", "−3");
    type("cents", "+60");
    type("cents", "ふつう");
    expect(app.sent.map((c) => (c.type === "setClipStretch" ? c.change : c))).toEqual([
      { kind: "semitones", semitones: -3 },
      { kind: "cents", cents: 60 },
    ]);
  });

  it("cannot be pressed while recording (not sent)", () => {
    const s = scene("stretch");
    s.transport.state = "recording";
    panel(s);
    q('[data-test="clip-switch-follow"]')!.click();
    pointer(q('[data-test="clip-value-semitones"]')!, "pointerdown", 100);
    pointer(q('[data-test="clip-value-semitones"]')!, "pointermove", 0);
    pointer(q('[data-test="clip-value-semitones"]')!, "pointerup", 0);
    expect(app.sent).toEqual([]);
  });

  it("while the sound is made, a progress bar 「音を作っています ▬ 34 %」 above the panel (only rendering of this clip)", () => {
    panel(scene("stretchRendering"));
    expect(text(q('[data-test="clip-rendering"]'))).toBe("音を作っています 34 %");
    expect(q<HTMLElement>('[data-test="clip-rendering"] .fill')!.style.transform).toBe("scaleX(0.34)");
    unmount(mounted!);
    mounted = null;
    const other = scene("stretchRendering");
    other.audioWork = { jobs: [{ kind: "rendering", audio: 1, clip: 9999, percent: 34 }] };
    panel(other);
    expect(q('[data-test="clip-rendering"]')).toBeNull();
  });
});

describe("text rules", () => {
  it("mark: × has 2 decimals and 1.00 is not shown. Pitch is 「+2」「−1 ・ +30」「+0 ・ −20」 (cents have a sign too; spec 3, how to read the mark), not shown when 0", () => {
    const base = { follow: true, sourceBpm: 96, ratio: null, semitones: 0, cents: 0, keepFormant: true };
    expect(clipMarks(base)).toBe(NO_MARKS);
    expect(clipMarks({ ...base, ratio: 1.25, semitones: 2 })).toEqual(["×1.25", "+2"]);
    expect(clipMarks({ ...base, ratio: 1.004, semitones: -1, cents: 30 })).toEqual(["−1 ・ +30"]);
    expect(clipMarks({ ...base, cents: -5 })).toEqual(["+0 ・ −5"]);
    expect(clipMarks({ ...base, cents: -20 })).toEqual(["+0 ・ −20"]);
    expect(clipMarks({ ...base, semitones: 2, cents: 30 })).toEqual(["+2 ・ +30"]);
    expect([ratioMarkValue(0.996), ratioMarkValue(0.994), ratioMarkValue(1.125), ratioMarkValue(null)]).toEqual([null, "0.99", "1.13", null]);
  });
  it("steps for typing and dragging", () => {
    expect([parseWhole("+2"), parseWhole("−1"), parseWhole("３"), parseWhole("2.6"), parseWhole("a"), parseWhole("")]).toEqual([2, -1, 3, 3, null, null]);
    expect([parseSourceBpm("120"), parseSourceBpm("96.5"), parseSourceBpm("0"), parseSourceBpm("-3"), parseSourceBpm("x")]).toEqual([120, 96.5, null, null, null]);
    expect([dragSteps(24, 8), dragSteps(-9, 8), dragSteps(7, 8), dragSteps(5, 0)]).toEqual([3, -1, 0, 0]);
    expect([centsText(-30), centsText(30), centsText(0)]).toEqual(["−30", "30", "0"]);
  });
});

describe("status line", () => {
  function line(state: AppState): string {
    app.setState(state);
    mounted = mount(StatusLine, { target });
    flushSync();
    return text(q(".message")) ?? "";
  }
  it("while the sound is made, 「歌 テイク 1 の音を作っています 34 % ・ …」 instead of the usual summary (mock F7-Stretch)", () => {
    expect(line(scene("stretchRendering"))).toBe("歌 テイク 1 の音を作っています 34 % ・ できるまでは、もとの音で鳴らします");
  });
  it("other notices (the search answer) come first even while the sound is made", () => {
    const s = scene("stretchRendering");
    s.status.message = { id: "sourceTempoFound", name: "ドラム ループ", bpm: 120, bars: 2, same: false } satisfies StatusMessage;
    expect(line(s)).toBe("この音のテンポは 120.00 くらいです（さがした数を入れました）。");
  });
  it("when not found", () => {
    const s = scene("stretchNoTempo");
    s.status.message = { id: "sourceTempoNotFound", name: "ドラム" };
    expect(line(s)).toBe("この音のテンポは見つかりませんでした。数を打ちこんでください。");
  });
});

describe("「meloet について」: copied sources (Signalsmith Stretch, Linear)", () => {
  it("upstream vendored parts become part rows by name and version (under the MIT heading)", () => {
    const list: LicenseList = {
      meloet: { version: "0.1.0", license: "MIT OR Apache-2.0" },
      sounds: [],
      fonts: [],
      upstream: [
        { crate: "vst3", license: "MIT" },
        { crate: "meloet-stretch", license: "MIT", name: "Signalsmith Stretch", version: "1.4.0", vendored: {} },
      ],
      parts: [{ kind: "crate", name: "serde", version: "1.0.0", license: "MIT" }],
    };
    const parts = allParts(list);
    expect(parts.map((p) => `${p.kind}:${p.name}@${p.version}`)).toEqual(["crate:serde@1.0.0", "vendored:Signalsmith Stretch@1.4.0"]);
    expect(partGroups(parts, "signalsmith").map((g) => [g.group, g.parts.map((p) => p.name)])).toEqual([["MIT", ["Signalsmith Stretch"]]]);
  });
  it("both are in the generated list (generated/licenses.json)", async () => {
    const { licenseList } = await import("../src/components/about/licenses");
    const names = allParts(licenseList)
      .filter((p) => p.kind === "vendored")
      .map((p) => `${p.name} ${p.version} ${p.license}`);
    expect(names).toEqual(["Signalsmith Stretch 1.4.0 MIT", "Signalsmith Linear 0.6.4 MIT"]);
  });
});

// Screen part: the clip gain line scale and dragging many (39-B), the panel for 2 or more chosen (40-A),
// checking before export (41-A) and solo (42-C), new text when latency cannot be measured (spec 6, version 12). Fake scenes multiClip / exportCheck / exportClip.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command, SelectedClips } from "../src/bridge";
import { fakeScene, type FakeState } from "../src/bridge/fake";
import { fakeClipsPanel } from "../src/bridge/fakeRound8";
import { gainLineY, waveArea } from "../src/components/arrange/audioClipGeom";
import AudioClipHandles from "../src/components/arrange/AudioClipHandles.svelte";
import { audioDrag } from "../src/components/arrange/audioDrag.svelte";
import { clipBox, type LaneDims, type View } from "../src/components/arrangeGeom";
import ExportDialog from "../src/components/export/ExportDialog.svelte";
import { soloText } from "../src/components/export/exportText";
import LatencyDialog from "../src/components/LatencyDialog.svelte";
import ParamsPanel from "../src/components/params/ParamsPanel.svelte";
import { cancelDrag, dragReading } from "../src/components/pianoroll/view.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { exportShown } from "../src/state/exportShown.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  audioDrag.now = null;
  dragReading.text = null;
  exportShown.forget();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  exportShown.forget();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState, props: Record<string, unknown> = {}): void {
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}
const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const scene = (name: string) => fakeScene(name)!.state as FakeState;
const BAR = 4 * TPQ;

function pointer(el: Element, type: string, clientX: number, clientY: number): void {
  el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX, clientY, button: 0 }));
  flushSync();
}
function typeInto(input: HTMLInputElement, value: string): void {
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
  flushSync();
}

/** The state with the multiClip panel values changed to the form under test. */
function withClips(over: Partial<SelectedClips>): FakeState {
  const s = scene("multiClip");
  s.params = { ...s.params, clips: { ...s.params.clips!, ...over } };
  return s;
}

describe("the panel when 2 or more audio clips are chosen (spec 3 (40-A), mock F8-MultiClip)", () => {
  it("fake scene: 3 chosen, the panel is clips (selected is empty)", () => {
    const s = scene("multiClip");
    expect(s.params.selected).toBeNull();
    expect(s.params.clips?.clips).toHaveLength(3);
    expect(s.params.clips).toEqual(fakeClipsPanel(s));
  });

  it("on top 「3 クリップを選んでいます」 and track names, 4 rows of the group 「まとめて変える」, and below the single-clip text", () => {
    show(ParamsPanel, scene("multiClip"), { onfold: () => {} });
    expect(text(q('[data-test="params-name"]'))).toBe("3 クリップを選んでいます");
    expect(text(q('[data-test="clips-tracks"]'))).toBe("6 歌 ・ 7 歌 2");
    expect(text(q('[data-group="clips"]'))).toBe("▾ まとめて変える 4");
    const rows = ["gain", "follow", "pitch", "keepFormant"].map((k) => text(q(`[data-test="clips-${k}"]`)));
    expect(rows).toEqual(["音量 —", "曲のテンポについていく オン", "音の高さ +0 半音 0 セント", "声の感じを保つ オン"]);
    expect(text(q('[data-test="clips-only-one"]'))).toBe("位置 ・ 長さ ・ フェード ・ この音のテンポは、1 つだけ選ぶと出ます。");
    expect(q('[data-test="clip-params"]')).toBeNull();
  });

  it("fields with differing values show 「—」. Differing on / off leaves the mark empty. Pitch shows semitones and cents separately, only 「—」 if both differ", () => {
    show(ParamsPanel, withClips({ gainDb: -3, follow: null, keepFormant: false, semitones: null, cents: 20 }), { onfold: () => {} });
    expect(text(q('[data-test="clips-value-gainDb"]'))).toBe("−3.0 dB");
    expect(text(q('[data-test="clips-value-follow"]'))).toBe("—");
    expect(q('[data-test="clips-switch-follow"]')!.classList.contains("on")).toBe(false);
    expect(q('[data-test="clips-switch-follow"]')!.getAttribute("aria-checked")).toBe("mixed");
    expect(text(q('[data-test="clips-value-keepFormant"]'))).toBe("オフ");
    expect(text(q('[data-test="clips-value-pitch"]'))).toBe("— 半音 20 セント");
    unmount(mounted!);
    show(ParamsPanel, withClips({ semitones: null, cents: null }), { onfold: () => {} });
    expect(text(q('[data-test="clips-value-pitch"]'))).toBe("—");
  });

  it("a typed gain sends one SetClipGains (to all chosen). Unreadable text and −∞ send nothing", () => {
    const s = scene("multiClip");
    const ids = s.params.clips!.clips;
    show(ParamsPanel, s, { onfold: () => {} });
    const value = () => q('[data-test="clips-value-gainDb"]')!;
    value().dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    flushSync();
    typeInto(q<HTMLInputElement>('[data-test="clips-gain"] input')!, "−6");
    value().dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    flushSync();
    typeInto(q<HTMLInputElement>('[data-test="clips-gain"] input')!, "あ");
    value().dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    flushSync();
    typeInto(q<HTMLInputElement>('[data-test="clips-gain"] input')!, "−∞");
    expect(app.sent).toEqual([{ type: "setClipGains", clips: ids, db: -6 }] satisfies Command[]);
  });

  it("on / off sends SetClipsStretch (off if all on, on if they differ). Typed semitones / cents too", () => {
    const s = withClips({ follow: true, keepFormant: null });
    const ids = s.params.clips!.clips;
    show(ParamsPanel, s, { onfold: () => {} });
    q('[data-test="clips-switch-follow"]')!.click();
    q('[data-test="clips-switch-keepFormant"]')!.click();
    q('[data-test="clips-value-semitones"]')!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    flushSync();
    typeInto(q<HTMLInputElement>('[data-test="clips-pitch"] input')!, "+2");
    q('[data-test="clips-value-cents"]')!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    flushSync();
    typeInto(q<HTMLInputElement>('[data-test="clips-pitch"] input')!, "−30");
    expect(app.sent).toEqual([
      { type: "setClipsStretch", clips: ids, change: { kind: "follow", on: false } },
      { type: "setClipsStretch", clips: ids, change: { kind: "keepFormant", on: true } },
      { type: "setClipsStretch", clips: ids, change: { kind: "semitones", semitones: 2 } },
      { type: "setClipsStretch", clips: ids, change: { kind: "cents", cents: -30 } },
    ] satisfies Command[]);
  });

  it("not sent while recording", () => {
    const s = scene("multiClip");
    s.transport.state = "recording";
    show(ParamsPanel, s, { onfold: () => {} });
    q('[data-test="clips-switch-follow"]')!.click();
    q('[data-test="clips-value-gainDb"]')!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    flushSync();
    expect(q('[data-test="clips-gain"] input')).toBeNull();
    expect(app.sent).toEqual([]);
  });

  it("status line: 「選択 3 クリップ ・ 欄の「音量」に打つと、3 つともその音量になります」 (with the real count)", () => {
    show(StatusLine, scene("multiClip"));
    expect(text(q(".status"))).toContain("選択 3 クリップ ・ 欄の「音量」に打つと、3 つともその音量になります");
    unmount(mounted!);
    const s = scene("multiClip");
    s.params.clips = { ...s.params.clips!, clips: s.params.clips!.clips.slice(0, 2) };
    show(StatusLine, s);
    expect(text(q(".status"))).toContain("選択 2 クリップ ・ 欄の「音量」に打つと、2 つともその音量になります");
  });
});

describe("clip gain line (spec 3 (39-B)): with many chosen, all move by the same dB", () => {
  const dims: LaneDims = { trackH: 64, inset: 3, hair: 1, mark: 2, nameH: 16, pad: 4, noteH: 2 };
  const view: View = { pxPerTick: 64 / BAR, scrollX: 0, width: 2000 };
  const vocalRows = (s: AppState) => s.song.tracks.filter((tr) => tr.kind.kind === "vocal");
  const areaOf = (s: AppState, id: number) => {
    const tracks = vocalRows(s);
    const row = tracks.findIndex((tr) => tr.clips.some((c) => c.id === id));
    const clip = tracks[row]!.clips.find((c) => c.id === id)!;
    return { clip, area: waveArea(dims, clipBox(view, dims, row, clip.start, clip.end)) };
  };

  it("moving one line moves every chosen line by the same dB (each stops at −60 to +12). The original place is a gray.7 line. Release sends one ShiftClipGains", () => {
    const s = scene("multiClip");
    const ids = s.params.clips!.clips;
    const tracks = vocalRows(s);
    show(AudioClipHandles, s, { view, dims, tracks, selected: new Set(ids), busy: false });
    const lines = qa('[data-test="clip-gain-line"]');
    expect(lines).toHaveLength(3);
    const minus3 = ids.find((id) => areaOf(s, id).clip.audio!.gainDb === -3)!;
    const zero = ids.find((id) => areaOf(s, id).clip.audio!.gainDb === 0)!;
    const grabbed = areaOf(s, minus3);
    const line = lines[ids.indexOf(minus3)]!;
    // −3 → +3 (6 dB up).
    pointer(line, "pointerdown", 450, gainLineY(grabbed.area, -3));
    pointer(line, "pointermove", 450, gainLineY(grabbed.area, 3));
    expect(audioDrag.now).toEqual({ clip: minus3, gainDb: 3, shift: { clips: ids.filter((id) => id !== minus3).sort((a, b) => a - b), byDb: 6 } });
    const top = (id: number) => parseFloat(qa('[data-test="clip-gain-line"]')[ids.indexOf(id)]!.style.top) + 3;
    expect(top(zero)).toBeCloseTo(gainLineY(areaOf(s, zero).area, 6), 6);
    expect(qa('[data-test="clip-gain-origin"]')).toHaveLength(3);
    // Far up: the held line stops at +12, the shift is +15. The 0 dB line also stops at +12.
    pointer(line, "pointermove", 450, grabbed.area.top - 50);
    expect(top(zero)).toBeCloseTo(gainLineY(areaOf(s, zero).area, 12), 6);
    pointer(line, "pointerup", 450, grabbed.area.top - 50);
    expect(app.sent).toEqual([{ type: "shiftClipGains", clips: ids.slice().sort((a, b) => a - b), byDb: 15 }] satisfies Command[]);
  });

  it("Esc cancels, every line goes back, nothing is sent. With only one chosen, SetClipField as before", () => {
    const s = scene("multiClip");
    const ids = s.params.clips!.clips;
    const tracks = vocalRows(s);
    show(AudioClipHandles, s, { view, dims, tracks, selected: new Set(ids), busy: false });
    const { clip, area } = areaOf(s, ids[0]!);
    const line = qa('[data-test="clip-gain-line"]')[0]!;
    const before = qa('[data-test="clip-gain-line"]').map((l) => l.style.top);
    pointer(line, "pointerdown", 450, gainLineY(area, clip.audio!.gainDb));
    pointer(line, "pointermove", 450, gainLineY(area, -12));
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(qa('[data-test="clip-gain-line"]').map((l) => l.style.top)).toEqual(before);
    pointer(line, "pointerup", 450, gainLineY(area, -12));
    expect(app.sent).toEqual([]);
    unmount(mounted!);
    show(AudioClipHandles, s, { view, dims, tracks, selected: new Set([ids[0]!]), busy: false });
    const one = q('[data-test="clip-gain-line"]')!;
    pointer(one, "pointerdown", 450, gainLineY(area, clip.audio!.gainDb));
    pointer(one, "pointermove", 450, gainLineY(area, -12));
    pointer(one, "pointerup", 450, gainLineY(area, -12));
    expect(app.sent).toEqual([{ type: "setClipField", clip: ids[0]!, field: "gainDb", value: -12 }] satisfies Command[]);
  });
});

describe("checking before export (spec 3 (41-A, 42-C), mocks F8-ExportCheck / F8-ExportClip)", () => {
  it("F8-ExportCheck: the check bar and %, the loudest place so far, stopped plugins, the solo text. 「書き出す」 can be pressed while counting", () => {
    show(ExportDialog, scene("exportCheck"));
    expect(qa('[data-test="export-check-line"]').map(text)).toEqual(["たしかめています 38 %", "ここまでのいちばん大きい所 −3.5 dB"]);
    expect(text(q('[data-test="export-stopped"]'))).toBe("止まっているプラグインはありません");
    expect(text(q('[data-test="export-solo"]'))).toBe("いま 2 ベース をソロにしています。書き出しには入れません（曲ぜんぶを書き出します）。");
    const start = q<HTMLButtonElement>('[data-test="export-start"]')!;
    expect([start.disabled, text(start), start.classList.contains("primary")]).toEqual([false, "書き出すEnter", true]);
    start.click();
    expect(app.sent).toEqual([{ type: "startExport" }]);
  });

  it("F8-ExportClip: the loudest place (number in rec), the rec 「3 か所」 text, gray.8 text, 「このまま書き出す」. No row without solo", () => {
    show(ExportDialog, scene("exportClip"));
    expect(qa('[data-test="export-check-line"]').map(text)).toEqual([
      "いちばん大きい所+1.2 dB",
      "0 dB をこえる所が 3 か所あります（いちばん大きい所 +1.2 dB）。音が割れます。",
      "マスターのフェーダーを下げるか、リミッターの上限を見てください。",
    ]);
    expect(q('[data-test="export-solo"]')).toBeNull();
    const start = q<HTMLButtonElement>('[data-test="export-start"]')!;
    expect([text(start), start.classList.contains("primary")]).toEqual(["このまま書き出すEnter", false]);
  });

  it("with 2 or more solos, 「2 ベース ほか 1 本」", () => {
    expect(soloText({ first: { number: 2, name: "ベース" }, others: 1 })).toBe("いま 2 ベース ほか 1 本 をソロにしています。書き出しには入れません（曲ぜんぶを書き出します）。");
    expect(soloText(null)).toBeNull();
    const s = scene("exportCheck");
    s.song.tracks[2]!.soloed = true;
    s.export.dialog = { ...s.export.dialog!, solo: { first: { number: 2, name: "ベース" }, others: 1 } };
    show(ExportDialog, s);
    expect(text(q('[data-test="export-solo"]'))).toContain("2 ベース ほか 1 本");
  });

  it("status line: 「「書き出し」のダイアログをひらいています ・ たしかめています 38 %」 and 「… ・ 0 dB をこえる所があります」", () => {
    show(StatusLine, scene("exportCheck"));
    expect(text(q(".status"))).toContain("「書き出し」のダイアログをひらいています ・ たしかめています 38 %");
    unmount(mounted!);
    exportShown.forget();
    show(StatusLine, scene("exportClip"));
    expect(text(q(".status"))).toContain("「書き出し」のダイアログをひらいています ・ 0 dB をこえる所があります");
    expect(text(q(".status"))).not.toContain("たしかめています");
    unmount(mounted!);
    exportShown.forget();
    show(StatusLine, scene("exportDialog"));
    expect(text(q(".status"))).toContain("「書き出し」のダイアログをひらいています");
    expect(text(q(".status"))).not.toContain(" ・ たしかめています");
  });

  it("mp3 is in the format list (A-23, C-11)", () => {
    show(ExportDialog, scene("exportCheck"));
    expect(qa('[data-test^="export-format-"]').map(text)).toEqual(["wav", "flac", "mp3"]);
  });
});

describe("latency cannot be measured (spec 6, version 12)", () => {
  const failed = (reason: "tooQuiet" | "unsteady" | "noInput") => {
    const s = scene("latencyFailed");
    s.workspace.latencyDialog = { measure: { state: "failed", reason } };
    return s;
  };

  it("spread: the chapter 6 text and 「もう一度」", () => {
    show(LatencyDialog, failed("unsteady"));
    expect(text(q('[data-test="latency-failed"]'))).toBe("測るたびに数が大きくちがったので、決められませんでした。まわりの音を小さくして、もう一度測ってください。");
    q('[data-test="latency-again"]')!.click();
    expect(app.sent).toEqual([{ type: "startLatencyMeasure" }]);
  });

  it("no input: the chapter 6 text and 「音の設定…」 (closes the dialog, goes to advanced sound settings)", () => {
    show(LatencyDialog, failed("noInput"));
    expect(text(q('[data-test="latency-failed"]'))).toBe("音の入口がないので測れません。マイクをつなぐか、入口をえらんでください。");
    expect(q('[data-test="latency-again"]')).toBeNull();
    const button = q('[data-test="latency-settings"]')!;
    expect(text(button)).toBe("音の設定…");
    button.click();
    expect(app.sent).toEqual([{ type: "closeLatencyMeasure" }, { type: "openSettings", advanced: true }] satisfies Command[]);
  });
});

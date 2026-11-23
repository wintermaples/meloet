// The dialog 「書き出しました」 when an export is done (round 12 E9).
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { EXPORT_WAVE_BARS } from "../src/bridge";
import { fakeScene, reduceExport, type FakeState } from "../src/bridge/fake";
import ExportedDialog from "../src/components/export/ExportedDialog.svelte";
import { exportedDetails, exportedLength, exportedLoudness, exportedOpen, wavePath, type ExportedDone } from "../src/components/export/exported";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state as FakeState;
let host: HTMLElement | null = null;
let comp: ReturnType<typeof mount> | null = null;
beforeEach(() => app.reset());
afterEach(() => {
  if (comp) unmount(comp);
  comp = null;
  host?.remove();
  host = null;
});

function show(run: ExportedDone): void {
  host = document.createElement("div");
  document.body.append(host);
  comp = mount(ExportedDialog, { target: host, props: { run } });
  flushSync();
}
const q = (test: string) => host!.querySelector<HTMLElement>(`[data-test="${test}"]`);
const words = (el: Element | null) => (el?.textContent ?? "").replace(/\s+/g, " ").trim();
const done = (s: AppState): ExportedDone => exportedOpen(s)!;
const key = (s: AppState, k: string): Command[] => {
  const sent: Command[] = [];
  const deps: KeyDeps = { state: () => s, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0 };
  handleKeydown(new KeyboardEvent("keydown", { key: k, cancelable: true }), deps);
  return sent;
};

describe("「書き出しました」", () => {
  it("one file: the name, the length and form, the waveform, the loudness, the place and three buttons", () => {
    const run = done(scene("exportDone"));
    show(run);
    expect(words(q("exported-dialog")?.querySelector(".title") ?? null)).toBe("書き出しました");
    expect(words(q("exported-name"))).toBe("はじめての曲.wav");
    expect(words(q("exported-details"))).toBe("3 分 2 秒 ・ 48000 Hz ・ 24 ビット");
    expect(words(q("exported-loudness"))).toBe("大きさ −14.0 LUFS ・ トゥルーピーク −1.0 dBTP");
    expect(words(q("exported-place"))).toBe("~/ミュージック/はじめての曲/はじめての曲.wav");
    expect(run.wave).toHaveLength(EXPORT_WAVE_BARS);
    expect(q("exported-wave")?.getAttribute("viewBox")).toBe(`0 0 ${EXPORT_WAVE_BARS} 255`);
    expect(q("exported-wave")?.querySelector("path")?.getAttribute("d")).toBe(wavePath(run.wave));

    const play = q("exported-play") as HTMLButtonElement;
    expect(words(play)).toBe("PC のプレーヤーで聞く");
    expect(play.disabled).toBe(false);
    play.click();
    q("exported-folder")!.click();
    q("exported-close")!.click();
    q("exported-close-head")!.click();
    expect(app.sent).toEqual([{ type: "playExported" }, { type: "showExported" }, { type: "closeExport" }, { type: "closeExport" }]);
    expect(words(q("exported-folder"))).toBe("フォルダをひらく");
    expect(words(q("exported-close"))).toBe("閉じるEnter");
    expect(words(q("exported-close-head"))).toBe("閉じるEsc");
  });

  it("one file for each track: 「5 つのファイル」, no waveform, no loudness, 「PC のプレーヤーで聞く」 off", () => {
    const run = done(scene("exportDoneTracks"));
    show(run);
    expect(words(q("exported-name"))).toBe("5 つのファイル");
    expect(words(q("exported-details"))).toBe("3 分 2 秒 ・ 48000 Hz ・ 24 ビット");
    expect(q("exported-wave")).toBeNull();
    expect(q("exported-loudness")).toBeNull();
    expect(words(q("exported-place"))).toBe("~/ミュージック/はじめての曲/");
    expect((q("exported-play") as HTMLButtonElement).disabled).toBe(true);
  });

  it("cancelled: not shown (the status line says so)", () => {
    const s = scene("exportCancelled");
    expect(exportedOpen(s)).toBeNull();
    expect(key(s, "Enter")).toEqual([]);
  });

  it("failed: not shown (the status line says so)", () => {
    const s = scene("exportFailed");
    expect(exportedOpen(s)).toBeNull();
    expect(key(s, "Enter")).toEqual([]);
  });

  it("Esc and Enter close it; the fake takes the run away as core does", () => {
    const s = scene("exportDone");
    expect(key(s, "Escape")).toEqual([{ type: "closeExport" }]);
    expect(key(s, "Enter")).toEqual([{ type: "closeExport" }]);
    expect(reduceExport(s, { type: "playExported" })).toEqual([]);
    expect(reduceExport(s, { type: "closeExport" })).toEqual([{ section: "export", value: { dialog: null, run: null } }]);
    expect(reduceExport(scene("exporting"), { type: "closeExport" })).toEqual([]);
  });
});

describe("the text", () => {
  it("lengths, silence and an mp3's place", () => {
    expect(exportedLength(45.9)).toBe("45 秒");
    expect(exportedLength(60)).toBe("1 分 0 秒");
    expect(exportedLength(192)).toBe("3 分 12 秒");
    const run = done(scene("exportDone"));
    expect(exportedLoudness({ ...run, lufs: null, truePeakDb: null })).toBeNull();
    expect(exportedLoudness({ ...run, lufs: null, truePeakDb: -40.2 })).toBe("大きさ −∞ LUFS ・ トゥルーピーク −40.2 dBTP");
    expect(exportedLoudness({ ...run, lufs: -9.5, truePeakDb: 0.3 })).toBe("大きさ −9.5 LUFS ・ トゥルーピーク +0.3 dBTP");
    expect(exportedDetails({ ...run, sampleRate: 44100, bits: "bits16", lengthS: 75 })).toBe("1 分 15 秒 ・ 44100 Hz ・ 16 ビット");
  });

  it("the waveform: a bar for each value that is not 0, around the middle line", () => {
    expect(wavePath([0, 255, 51])).toBe("M1 0h0.75v255h-0.75zM2 102h0.75v51h-0.75z");
  });
});

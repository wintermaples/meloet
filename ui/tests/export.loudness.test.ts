// Screen: 「音の大きさ」 of the export (round 12 F1): 「そのまま」 / 「公開先に合わせる」, the loudness and true peak in the check,
// and the line of what matching does. The fake follows Rust's rules (crates/meloet-core/src/export.rs, export/loudness.rs).
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command, ExportCheck, ExportDialogView } from "../src/bridge";
import { fakeScene, reduceExport, type FakeState } from "../src/bridge/fake";
import { fakeLoudnessOf, fakeMatch } from "../src/bridge/fakeLoudness";
import ExportDialog from "../src/components/export/ExportDialog.svelte";
import { checkLines, loudnessTarget, type CheckLine } from "../src/components/export/exportText";
import { closeContextMenu } from "../src/components/menu/contextMenu.svelte";
import { exportShown } from "../src/state/exportShown.svelte";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state as FakeState;
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const lineText = (line: CheckLine) => (line.kind === "counting" ? `counting ${line.percent}` : line.parts.map((p) => p.text).join(""));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  exportShown.forget();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  closeContextMenu();
  exportShown.forget();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const checkTexts = () => qa('[data-test="export-check-line"]').map(text);

describe("the lines of the check (round 12 F1)", () => {
  const done = (lufs: number | null, truePeakDb: number | null): ExportCheck => ({ state: "done", peakDb: -3.4, clips: false, over: 0, lufs, truePeakDb });
  const shown = (check: ExportCheck, loudness: ExportDialogView["loudness"]) => ({ loudness, loudnessResult: fakeLoudnessOf(check, "song", loudness).loudnessResult });

  it("the right of the row is the target, from the protocol's numbers", () => {
    expect(loudnessTarget()).toBe("−14 LUFS ・ −1 dBTP");
  });

  it("done: the loudness and the true peak after the peak line; the result only when matching", () => {
    const check = done(-18.6, -6.0);
    expect(checkLines(check).map(lineText)).toEqual(["いちばん大きい所 −3.4 dB ・ 音割れはありません", "曲ぜんたいの大きさ −18.6 LUFS ・ トゥルーピーク −6.0 dBTP"]);
    expect(checkLines(check, shown(check, "asIs")).map(lineText)).toHaveLength(2);
    expect(checkLines(check, shown(check, "match")).map(lineText).at(-1)).toBe("合わせると 4.6 dB 大きくなります");
  });

  it("louder, quieter, or stopped at −1 dBTP", () => {
    const result = (lufs: number, peak: number) => checkLines(done(lufs, peak), shown(done(lufs, peak), "match")).map(lineText).at(-1);
    expect(result(-18.6, -6.0)).toBe("合わせると 4.6 dB 大きくなります");
    expect(result(-9.6, 0.8)).toBe("合わせると 4.4 dB 小さくなります");
    expect(result(-17.4, -3.2)).toBe("−1 dBTP を越えないよう、−15.2 LUFS になります");
    // Above 0 dBTP the peak has its +.
    expect(checkLines(done(-9.6, 0.8)).map(lineText).at(-1)).toBe("曲ぜんたいの大きさ −9.6 LUFS ・ トゥルーピーク +0.8 dBTP");
  });

  it("nothing measured (silence, or 「トラックごと」): no loudness line", () => {
    expect(checkLines(done(null, null), { loudness: "asIs", loudnessResult: null }).map(lineText)).toEqual(["いちばん大きい所 −3.4 dB ・ 音割れはありません"]);
    // A true peak without a loudness (shorter than 400 ms, or under the gate): −∞ LUFS.
    expect(checkLines(done(null, -60.0)).map(lineText).at(-1)).toBe("曲ぜんたいの大きさ −∞ LUFS ・ トゥルーピーク −60.0 dBTP");
  });

  it("counting: the numbers so far, the loudness not known yet is —, and no result", () => {
    const counting = (lufs: number | null): ExportCheck => ({ state: "counting", percent: 12, peakDb: -9, clips: false, over: 0, lufs, truePeakDb: -8.2 });
    expect(checkLines(counting(-21.3), { loudness: "match", loudnessResult: null }).map(lineText)).toEqual([
      "counting 12",
      "ここまでのいちばん大きい所 −9.0 dB",
      "ここまでの大きさ −21.3 LUFS ・ トゥルーピーク −8.2 dBTP",
    ]);
    expect(checkLines(counting(null)).map(lineText).at(-1)).toBe("ここまでの大きさ — LUFS ・ トゥルーピーク −8.2 dBTP");
  });
});

describe("the row 「音の大きさ」 in the dialog (round 12 F1)", () => {
  it("after 「形」: 「そのまま」 is on at first, 「公開先に合わせる」 can be chosen, the target on the right", () => {
    show(ExportDialog, scene("exportDialog"));
    const sections = qa("section").map((s) => s.querySelector(".label")?.textContent);
    expect(sections.slice(0, 3)).toEqual(["範囲", "形", "音の大きさ"]);
    expect(qa('[data-test^="export-loudness-"]').map(text)).toEqual(["そのまま", "公開先に合わせる", "−14 LUFS ・ −1 dBTP"]);
    expect(q('[data-test="export-loudness-asIs"]')!.classList.contains("on")).toBe(true);
    expect(q('[data-test="export-loudness-match"]')!.classList.contains("on")).toBe(false);
    expect(q<HTMLButtonElement>('[data-test="export-loudness-match"]')!.disabled).toBe(false);
  });

  it("pressing sends the intent only", () => {
    show(ExportDialog, scene("exportDialog"));
    q('[data-test="export-loudness-match"]')!.click();
    q('[data-test="export-loudness-asIs"]')!.click();
    expect(app.sent).toEqual([
      { type: "setExport", change: { kind: "loudness", loudness: "match" } },
      { type: "setExport", change: { kind: "loudness", loudness: "asIs" } },
    ] satisfies Command[]);
  });

  it("exportLoudness: matched, 4.6 dB louder; the numbers are monospace", () => {
    show(ExportDialog, scene("exportLoudness"));
    expect(q('[data-test="export-loudness-match"]')!.classList.contains("on")).toBe(true);
    expect(checkTexts()).toEqual(["いちばん大きい所 −6.8 dB ・ 音割れはありません", "曲ぜんたいの大きさ −18.6 LUFS ・ トゥルーピーク −6.0 dBTP", "合わせると 4.6 dB 大きくなります"]);
    const loud = qa('[data-test="export-check-line"]')[1]!;
    expect([...loud.querySelectorAll(".mono.strong")].map((e) => e.textContent)).toEqual(["−18.6", "−6.0"]);
  });

  it("exportLoudnessLimited: the true peak stops it", () => {
    show(ExportDialog, scene("exportLoudnessLimited"));
    expect(checkTexts().at(-1)).toBe("−1 dBTP を越えないよう、−15.2 LUFS になります");
  });

  it("exportLoudnessQuieter: a song over 0 dB is turned down and does not clip: 「書き出す」", () => {
    show(ExportDialog, scene("exportLoudnessQuieter"));
    expect(checkTexts()).toEqual(["いちばん大きい所 +0.5 dB ・ 音割れはありません", "曲ぜんたいの大きさ −9.6 LUFS ・ トゥルーピーク +0.8 dBTP", "合わせると 4.4 dB 小さくなります"]);
    expect(text(q('[data-test="export-start"]'))).toBe("書き出すEnter");
  });

  it("exportLoudnessCounting: the loudness so far, no result yet", () => {
    show(ExportDialog, scene("exportLoudnessCounting"));
    expect(checkTexts()).toEqual(["たしかめています 38 %", "ここまでのいちばん大きい所 −3.5 dB", "ここまでの大きさ −19.0 LUFS ・ トゥルーピーク −3.1 dBTP"]);
    expect(q('[data-test="export-loudness-match"]')!.classList.contains("on")).toBe(true);
  });

  it("exportLoudnessSilent: 「公開先に合わせる」 is disabled and 「そのまま」 is on", () => {
    show(ExportDialog, scene("exportLoudnessSilent"));
    expect(q<HTMLButtonElement>('[data-test="export-loudness-match"]')!.disabled).toBe(true);
    expect(q('[data-test="export-loudness-asIs"]')!.classList.contains("on")).toBe(true);
    expect(checkTexts()).toEqual(["いちばん大きい所 −∞ dB ・ 音割れはありません"]);
  });
});

describe("the fake as Rust's rules (round 12 F1)", () => {
  const dialogAfter = (s: FakeState, ...commands: Command[]) => {
    for (const command of commands) {
      const changes = reduceExport(s, command) ?? [];
      for (const c of changes) if (c.section === "export") s.export = c.value;
    }
    return s.export.dialog!;
  };
  const loudness = (l: ExportDialogView["loudness"]): Command => ({ type: "setExport", change: { kind: "loudness", loudness: l } });

  it("matching is chosen and taken back; 「トラックごと」 cannot be matched and has no loudness", () => {
    const s = scene("exportDialog");
    expect(dialogAfter(s, loudness("match")).loudness).toBe("match");
    const tracks = dialogAfter(s, { type: "setExport", change: { kind: "range", range: "tracks" } });
    expect([tracks.canMatch, tracks.loudness, tracks.loudnessResult]).toEqual([false, "asIs", null]);
    expect(tracks.check).toMatchObject({ lufs: null, truePeakDb: null });
    expect(dialogAfter(s, loudness("match")).loudness).toBe("asIs");
    const song = dialogAfter(s, { type: "setExport", change: { kind: "range", range: "song" } });
    expect([song.canMatch, song.check]).toMatchObject([true, { lufs: -12.5, truePeakDb: -0.5 }]);
    expect(dialogAfter(s, loudness("asIs")).loudness).toBe("asIs");
  });

  it("a song near silence cannot be matched: the choice does nothing", () => {
    const s = scene("exportLoudnessSilent");
    expect(reduceExport(s, loudness("match"))).toEqual([]);
    expect(fakeMatch(-70.5, -40)).toBeNull();
    expect(fakeMatch(-16, 2)).toEqual({ gainDb: -3, limited: true, lufs: -19 });
  });
});

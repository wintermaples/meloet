// Screen: MIDI export in the dialog 「書き出し」 (spec 3 "MIDI export", spec 7 A-31): 「入れるトラック」, its number field,
// the rows only for audio hidden, and the done message in the status line without the dialog 「書き出しました」.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command, ExportDialogView } from "../src/bridge";
import { fakeScene, type FakeState } from "../src/bridge/fake";
import ExportDialog from "../src/components/export/ExportDialog.svelte";
import { exportedOpen } from "../src/components/export/exported";
import { midiTakenText } from "../src/components/export/exportText";
import StatusLine from "../src/components/StatusLine.svelte";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";
import { exportShown } from "../src/state/exportShown.svelte";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state as FakeState;
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

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
  exportShown.forget();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState): void {
  if (mounted) unmount(mounted);
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const withDialog = (name: string, over: Partial<ExportDialogView>): FakeState => {
  const s = scene(name);
  s.export.dialog = { ...s.export.dialog!, ...over };
  return s;
};
const key = (el: HTMLElement, k: string, more: KeyboardEventInit = {}) => {
  const e = new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...more });
  el.dispatchEvent(e);
  flushSync();
  return e;
};
const typeInto = (input: HTMLInputElement, value: string) => {
  input.focus();
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  flushSync();
};
const field = () => q<HTMLInputElement>('[data-test="export-midi-text"]');
const start = () => q<HTMLButtonElement>('[data-test="export-start"]')!;

describe("the dialog under 「MIDI」", () => {
  it("exportMidi: 「MIDI」 chosen, 「入れるトラック」 right under 「形」, the audio rows hidden, 「書き出す」 at once", () => {
    show(ExportDialog, scene("exportMidi"));
    expect(q('[data-test="export-format-midi"]')!.classList.contains("on")).toBe(true);
    const sections = qa("section");
    const midi = q('[data-test="export-midi"]')!;
    expect(sections.indexOf(midi)).toBe(2);
    expect(text(midi.querySelector(".label"))).toBe("入れるトラック");
    expect(qa('[data-test^="export-midi-"]').filter((b) => b.tagName === "BUTTON").map(text)).toEqual(["ぜんぶ", "選んでいるトラック", "番号で"]);
    expect(q('[data-test="export-midi-all"]')!.classList.contains("on")).toBe(true);
    expect(field()).toBeNull();
    expect(text(q('[data-test="export-midi-taken"]'))).toBe("1 ピアノ ・ 2 ベース ほか 2 本");
    expect(q('[data-test="export-midi-vocal"]')).toBeNull();
    expect(q('[data-test="export-midi-problem"]')).toBeNull();
    for (const test of ["export-loudness", "export-check", "export-details", "export-adv", "export-song-info"]) expect(q(`[data-test="${test}"]`), test).toBeNull();
    expect(text(q('[data-test="export-span"]'))).toBe("曲ぜんぶ ・ 1. 1. 0000 〜 17. 1. 0000");
    expect(start().disabled).toBe(false);
    expect(text(start())).toBe("書き出すEnter");
  });

  it("「トラックごと」 counts the tracks taken", () => {
    show(ExportDialog, withDialog("exportMidi", { range: "tracks" }));
    expect(text(q('[data-test="export-span"]'))).toBe("トラックごと ・ 4 つのファイル");
  });

  it("the three choices send their intents; 「選んでいるトラック」 is disabled when no keys track is selected", () => {
    show(ExportDialog, scene("exportMidi"));
    for (const choice of ["all", "selected", "numbers"]) q(`[data-test="export-midi-${choice}"]`)!.click();
    expect(app.sent).toEqual([
      { type: "setExport", change: { kind: "midiTracks", tracks: "all" } },
      { type: "setExport", change: { kind: "midiTracks", tracks: "selected" } },
      { type: "setExport", change: { kind: "midiTracks", tracks: "numbers" } },
    ] satisfies Command[]);
    show(ExportDialog, withDialog("exportMidi", { midiSelectedUsable: false }));
    expect(q<HTMLButtonElement>('[data-test="export-midi-selected"]')!.disabled).toBe(true);
  });

  it("the names: one, two, and two with 「ほか N 本」", () => {
    expect(midiTakenText([])).toBeNull();
    expect(midiTakenText([{ number: 1, name: "ピアノ" }])).toBe("1 ピアノ");
    expect(
      midiTakenText([
        { number: 1, name: "ピアノ" },
        { number: 2, name: "ベース" },
      ]),
    ).toBe("1 ピアノ ・ 2 ベース");
  });

  it("exportMidiNumbers: the field with the numbers, two tracks, and the vocal track in gray", () => {
    show(ExportDialog, scene("exportMidiNumbers"));
    expect(q('[data-test="export-midi-numbers"]')!.classList.contains("on")).toBe(true);
    expect(field()!.value).toBe("1-2, 4");
    expect(text(q('[data-test="export-midi-taken"]'))).toBe("1 ピアノ ・ 2 ベース");
    expect(qa('[data-test="export-midi-vocal"]').map(text)).toEqual(["4 歌 は声のトラックなので入りません"]);
    expect(q('[data-test="export-midi-vocal"]')!.classList.contains("rec")).toBe(false);
    expect(q('[data-test="export-midi-problem"]')).toBeNull();
    expect(start().disabled).toBe(false);
  });

  it("an empty field shows the hint 「1-3, 5」", () => {
    show(ExportDialog, withDialog("exportMidiNumbers", { midiNumbers: "", midiTaken: [], midiVocal: [], midiProblem: { kind: "noTrack" }, canExport: false }));
    expect(field()!.value).toBe("");
    expect(field()!.placeholder).toBe("1-3, 5");
    expect(text(q('[data-test="export-midi-problem"]'))).toBe("入れるトラックがありません");
  });

  it("the field sends once, on Enter or when it is left; Esc puts back what Rust has; Enter in the IME is the IME's", () => {
    show(ExportDialog, scene("exportMidiNumbers"));
    typeInto(field()!, "1-3");
    expect(app.sent).toEqual([]);
    expect(key(field()!, "Enter", { isComposing: true }).defaultPrevented).toBe(false);
    expect(app.sent).toEqual([]);
    expect(key(field()!, "Enter").defaultPrevented).toBe(true);
    typeInto(field()!, "5");
    field()!.blur();
    flushSync();
    expect(app.sent).toEqual([
      { type: "setExport", change: { kind: "midiNumbers", text: "1-3" } },
      { type: "setExport", change: { kind: "midiNumbers", text: "5" } },
    ] satisfies Command[]);
    app.sent.length = 0;
    typeInto(field()!, "9");
    key(field()!, "Escape");
    expect(field()!.value).toBe("1-2, 4");
    expect(app.sent).toEqual([]);
  });

  it("problems in rec, and 「書き出す」 disabled", () => {
    show(ExportDialog, scene("exportMidiProblem"));
    const problem = q('[data-test="export-midi-problem"]')!;
    expect(text(problem)).toBe("9 番のトラックはありません");
    expect(problem.classList.contains("rec")).toBe(true);
    expect(text(q('[data-test="export-midi-taken"]'))).toBe("1 ピアノ ・ 2 ベース ほか 1 本");
    expect(start().disabled).toBe(true);
    show(ExportDialog, withDialog("exportMidiProblem", { midiTaken: [], midiProblem: { kind: "unreadable" } }));
    expect(text(q('[data-test="export-midi-problem"]'))).toBe("番号は「1-3, 5」のように書きます");
    expect(q('[data-test="export-midi-taken"]')).toBeNull();
  });

  it("a check that would clip does not turn 「書き出す」 into 「このまま書き出す」 under MIDI", () => {
    show(ExportDialog, withDialog("exportMidi", { check: { state: "done", peakDb: 1.2, clips: true, over: 3, lufs: null, truePeakDb: null } }));
    expect(text(start())).toBe("書き出すEnter");
  });
});

describe("done (MIDI)", () => {
  const keys = (s: AppState, k: string): Command[] => {
    const sent: Command[] = [];
    const deps: KeyDeps = { state: () => s, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0 };
    handleKeydown(new KeyboardEvent("keydown", { key: k, cancelable: true }), deps);
    return sent;
  };

  it("one file: the status line says it with 「フォルダをひらく」; no dialog 「書き出しました」", () => {
    const s = scene("exportDoneMidi");
    expect(exportedOpen(s)).toBeNull();
    expect(keys(s, "Enter")).toEqual([]);
    show(StatusLine, s);
    expect(text(q(".status .message"))).toBe("はじめての曲.mid を書き出しました（MIDI ・ 3 トラック）。");
    const folder = [...target.querySelectorAll<HTMLButtonElement>(".status button")].find((b) => text(b) === "フォルダをひらく")!;
    folder.click();
    expect(app.sent).toEqual([{ type: "showExported" }]);
  });

  it("「トラックごと」: the count of files", () => {
    const s = scene("exportDoneMidiTracks");
    expect(exportedOpen(s)).toBeNull();
    show(StatusLine, s);
    expect(text(q(".status .message"))).toBe("3 つのファイルを書き出しました（MIDI）。");
  });
});

describe("the span line under MIDI", () => {
  it("says no tail: a MIDI file ends where the range ends", () => {
    show(ExportDialog, withDialog("exportMidi", { tail: "untilSilent" }));
    const span = text(q("[data-test=export-span]"))!;
    expect(span).toContain("曲ぜんぶ");
    expect(span).not.toContain("音が消えるまで");
  });
});

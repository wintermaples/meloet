// Screen: mp3 in the export (round 12 F2): 「ビットレート」 in place of 「ビット」 in 「くわしく」, 44100 and 48000 Hz only,
// and the bitrate in 「書き出しました」 and in the status line. The fake follows Rust's rules (crates/meloet-core/src/export.rs).
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeScene, reduceExport, type FakeState } from "../src/bridge/fake";
import ExportDialog from "../src/components/export/ExportDialog.svelte";
import { exportedDetails, exportedOpen } from "../src/components/export/exported";
import { detailsSummary, runShown } from "../src/components/export/exportText";
import { closeContextMenu, contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
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
const rows = () => (contextMenu.open?.entries ?? []).filter((e): e is MenuRow => e.kind === "row");

describe("mp3 in the export dialog (round 12 F2)", () => {
  it("exportMp3: 「ビットレート」 in place of 「ビット」, the four bitrates, 44100 and 48000 Hz", () => {
    show(ExportDialog, scene("exportMp3"));
    expect(q('[data-test="export-format-mp3"]')!.classList.contains("on")).toBe(true);
    expect(q('[data-test="export-bits"]')).toBeNull();
    expect(text(q('[data-test="export-kbps"]'))).toBe("192 kbps▾");
    q('[data-test="export-kbps"]')!.click();
    expect(rows().map((r) => [r.text, r.checked])).toEqual([
      ["128 kbps", false],
      ["192 kbps", true],
      ["256 kbps", false],
      ["320 kbps", false],
    ]);
    rows()[3]!.run();
    q('[data-test="export-rate"]')!.click();
    expect(rows().map((r) => r.text)).toEqual(["44100 Hz", "48000 Hz"]);
    expect(app.sent).toEqual([{ type: "setExport", change: { kind: "mp3Kbps", kbps: 320 } }] satisfies Command[]);
    // An mp3 holds a picture.
    expect(q<HTMLButtonElement>('[data-test="export-choose-picture"]')!.disabled).toBe(false);
  });

  it("folded 「くわしく」 says the bitrate", () => {
    const d = scene("exportMp3").export.dialog!;
    expect(detailsSummary(d)).toBe("48000 Hz ・ 192 kbps ・ しっぽは音が消えるまで（最大 10 秒）");
    expect(detailsSummary({ ...d, tail: "off", mp3Kbps: 128 })).toBe("48000 Hz ・ 128 kbps ・ しっぽなし");
  });

  it("「書き出しました」 and the status line say the bitrate in place of the bits", () => {
    const s = scene("exportDoneMp3");
    expect(exportedDetails(exportedOpen(s)!)).toBe("3 分 2 秒 ・ 48000 Hz ・ 192 kbps");
    const shown = runShown(s.export.run!);
    expect(shown.kind === "message" && shown.parts.map((p) => p.text).join("")).toBe("はじめての曲.mp3 を書き出しました（3:02 ・ 48000 Hz ・ 192 kbps）。");
  });
});

describe("the fake as Rust's rules (round 12 F2, mp3)", () => {
  const dialogAfter = (s: FakeState, ...commands: Command[]) => {
    for (const command of commands) {
      const changes = reduceExport(s, command) ?? [];
      for (const c of changes) if (c.section === "export") s.export = c.value;
    }
    return s.export.dialog!;
  };
  const change = (c: Extract<Command, { type: "setExport" }>["change"]): Command => ({ type: "setExport", change: c });

  it("mp3 keeps the bits chosen for the others, has its rates, and takes only its bitrates", () => {
    const s = scene("exportDialog");
    dialogAfter(s, change({ kind: "sampleRate", hz: 96000 }), change({ kind: "bits", bits: "bits16" }));
    const mp3 = dialogAfter(s, change({ kind: "format", format: "mp3" }));
    expect([mp3.bitsChoices, mp3.bits, mp3.sampleRateChoices, mp3.sampleRate, mp3.place]).toEqual([[], "bits16", [44100, 48000], 48000, "~/音楽/はじめての曲.mp3"]);
    expect(dialogAfter(s, change({ kind: "mp3Kbps", kbps: 160 })).mp3Kbps).toBe(192);
    expect(dialogAfter(s, change({ kind: "mp3Kbps", kbps: 256 })).mp3Kbps).toBe(256);
    expect(dialogAfter(s, change({ kind: "format", format: "wav" })).bits).toBe("bits16");
  });
});

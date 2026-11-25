// Screen: the export dialog asks before it writes over files that are there already (protocol 39; the owner,
// 2026-11-25): the question in rec in the place of the check, 「このまま書き出す」, 「やめる」 and the keys.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeScene, type FakeState } from "../src/bridge/fake";
import ExportDialog from "../src/components/export/ExportDialog.svelte";
import { handleKeydown } from "../src/keys/attach";
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
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const start = () => q<HTMLButtonElement>('[data-test="export-start"]')!;

describe("the question before writing over (protocol 39)", () => {
  it("one file: in rec at the end of the check, and 「このまま書き出す」 in place of 「書き出す」", () => {
    show(ExportDialog, scene("exportReplace"));
    const line = q('[data-test="export-replace"]')!;
    expect(text(line)).toBe("「はじめての曲.wav」はもうあります。上書きします。");
    expect(line.classList.contains("rec")).toBe(true);
    expect(q('[data-test="export-check"]')!.lastElementChild).toBe(line);
    expect([text(start()), start().disabled, start().classList.contains("primary")]).toEqual(["このまま書き出すEnter", false, false]);
    expect(text(q('[data-test="export-cancel"]'))).toBe("やめるEsc");
  });

  it("「トラックごと」: how many files are there already", () => {
    show(ExportDialog, scene("exportReplaceTracks"));
    expect(text(q('[data-test="export-replace"]'))).toBe("同じ名前のファイルが 3 個もうあります。上書きします。");
  });

  it("clipping too: the check's lines, then the question, one 「このまま書き出す」", () => {
    show(ExportDialog, scene("exportReplaceClips"));
    const lines = [...q('[data-test="export-check"]')!.querySelectorAll('[data-test="export-check-line"], [data-test="export-replace"]')].map(text);
    expect(lines.at(-1)).toBe("「はじめての曲.wav」はもうあります。上書きします。");
    expect(lines.some((l) => l?.endsWith("音が割れます。"))).toBe(true);
    expect(text(start())).toBe("このまま書き出すEnter");
  });

  it("MIDI has no check: the question is where it would be", () => {
    show(ExportDialog, scene("exportReplaceMidi"));
    expect(q('[data-test="export-check"]')).toBeNull();
    expect(text(q('[data-test="export-replace-midi"] [data-test="export-replace"]'))).toBe("「はじめての曲.mid」はもうあります。上書きします。");
    expect(text(start())).toBe("このまま書き出すEnter");
  });

  it("no question: 「書き出す」, bright", () => {
    show(ExportDialog, scene("exportDialog"));
    expect(q('[data-test="export-replace"]')).toBeNull();
    expect([text(start()), start().classList.contains("primary")]).toEqual(["書き出すEnter", true]);
  });

  it("「このまま書き出す」 and 「やめる」 send the intents of the dialog; Enter and Esc the same", () => {
    show(ExportDialog, scene("exportReplace"));
    start().click();
    q('[data-test="export-cancel"]')!.click();
    expect(app.sent).toEqual([{ type: "startExport" }, { type: "closeExport" }] satisfies Command[]);
    const key = (init: KeyboardEventInit): Command[] => {
      const sent: Command[] = [];
      handleKeydown(new KeyboardEvent("keydown", { cancelable: true, ...init }), {
        state: () => scene("exportReplace"),
        send: (c) => void sent.push(c),
        nav: { platform: "Linux x86_64" },
      });
      return sent;
    };
    expect(key({ key: "Enter" })).toEqual([{ type: "startExport" }]);
    expect(key({ key: "Escape" })).toEqual([{ type: "closeExport" }]);
  });
});

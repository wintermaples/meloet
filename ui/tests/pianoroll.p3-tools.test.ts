// P3 (round 12): 「割合」 「だんだん」 「ばらつき」 of 「ベロシティをまとめて変える」 and 「次の音までのばす」 of 「長さをそろえる」.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command, TakeNotes } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import { readTool, stepTool, toolCommand } from "../src/components/pianoroll/bulkTools";
import NoteInspector from "../src/components/pianoroll/NoteInspector.svelte";
import { inspectorFolds } from "../src/components/pianoroll/view.svelte";
import * as app from "./fake-app.svelte";
import * as shown from "./fake-shown.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  inspectorFolds.velocityAll = true;
  inspectorFolds.sameLength = true;
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  inspectorFolds.velocityAll = false;
  inspectorFolds.sameLength = false;
  target.remove();
});

const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

async function scene(edit?: (s: AppState) => void): Promise<TakeNotes> {
  const s = fakeScene("pianoRoll")!.state;
  edit?.(s);
  const track = s.song.tracks[0]!;
  const clip = track.clips.find((c) => c.id === s.arrange.pianoRollClip)!;
  const take = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
  app.setState(s);
  shown.setShown({ clip, track, take });
  mounted = mount(NoteInspector as Component<any>, { target, props: {} });
  flushSync();
  return take;
}

function type(sel: string, value: string, key?: string, shiftKey = false): void {
  const input = q<HTMLInputElement>(sel)!;
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  if (key) input.dispatchEvent(new KeyboardEvent("keydown", { key, shiftKey, bubbles: true, cancelable: true }));
  flushSync();
}

describe("the pure parts", () => {
  it("reads and puts values at the ends, and says so for every field", () => {
    expect(readTool("percent", "120")).toEqual({ value: 120, clamped: null });
    expect(readTool("percent", "5")).toEqual({ value: 10, clamped: { field: "percent", value: "10", range: null } });
    expect(readTool("percent", "１２０")).toEqual({ value: 120, clamped: null });
    expect(readTool("spread", "99")).toEqual({ value: 32, clamped: { field: "amount", value: "32", range: null } });
    expect(readTool("spread", "-3")).toEqual({ value: 1, clamped: { field: "amount", value: "1", range: null } });
    expect(readTool("rampTo", "300")).toEqual({ value: 127, clamped: { field: "velocity", value: "127", range: null } });
    expect(readTool("rampFrom", "あ")).toBeNull();
    expect(stepTool("percent", "295", 10)).toBe("300");
    expect(stepTool("spread", "x", -1)).toBe("7");
  });

  it("makes the four intents", () => {
    expect(toolCommand("percent", 5, [1, 2], [120], 0)).toEqual({ type: "scaleVelocity", take: 5, notes: [1, 2], percent: 120 });
    expect(toolCommand("ramp", 5, [1], [40, 110], 0)).toEqual({ type: "rampVelocity", take: 5, notes: [1], from: 40, to: 110 });
    expect(toolCommand("spread", 5, [1], [8], 77)).toEqual({ type: "randomVelocity", take: 5, notes: [1], amount: 8, seed: 77 });
  });
});

describe("the rows", () => {
  it("show the words, the marks and the first values", async () => {
    await scene();
    const row = (test: string) => text(q(`[data-test="${test}"]`)!.closest(".bulk-row"));
    expect(row("inspector-scale-velocity")).toBe("割合 % かける");
    expect(row("inspector-ramp-velocity")).toBe("だんだん → かける");
    expect(row("inspector-random-velocity")).toBe("ばらつき ± かける");
    expect(row("inspector-legato")).toBe("次の音までのばす のばす");
    expect(q<HTMLInputElement>('[data-test="inspector-percent-field"]')!.value).toBe("100");
    expect(q<HTMLInputElement>('[data-test="inspector-ramp-from-field"]')!.value).toBe("40");
    expect(q<HTMLInputElement>('[data-test="inspector-ramp-to-field"]')!.value).toBe("110");
    expect(q<HTMLInputElement>('[data-test="inspector-spread-field"]')!.value).toBe("8");
  });

  it("Enter or the button sends one intent for the selected notes", async () => {
    const take = await scene();
    const notes = [14, 15, 16];
    type('[data-test="inspector-percent-field"]', "120", "Enter");
    expect(app.sent).toEqual([{ type: "scaleVelocity", take: take.id, notes, percent: 120 }]);
    app.sent.length = 0;
    type('[data-test="inspector-ramp-to-field"]', "90");
    q('[data-test="inspector-ramp-velocity"]')!.click();
    expect(app.sent).toEqual([{ type: "rampVelocity", take: take.id, notes, from: 40, to: 90 }]);
    app.sent.length = 0;
    q('[data-test="inspector-random-velocity"]')!.click();
    q('[data-test="inspector-random-velocity"]')!.click();
    const [a, b] = app.sent as Extract<Command, { type: "randomVelocity" }>[];
    expect(a).toMatchObject({ type: "randomVelocity", take: take.id, notes, amount: 8 });
    expect(b!.seed).not.toBe(a!.seed);
    app.sent.length = 0;
    q('[data-test="inspector-legato"]')!.click();
    expect(app.sent).toEqual([{ type: "legato", take: take.id, notes }]);
  });

  it("puts values at the ends and says so; a broken one stays red and is said too", async () => {
    const take = await scene();
    type('[data-test="inspector-ramp-from-field"]', "0", "Enter");
    expect(app.sent).toEqual([
      { type: "rampVelocity", take: take.id, notes: [14, 15, 16], from: 1, to: 110 },
      { type: "clampValue", field: "velocity", value: "1", range: null },
    ]);
    expect(q<HTMLInputElement>('[data-test="inspector-ramp-from-field"]')!.value).toBe("1");
    app.sent.length = 0;
    type('[data-test="inspector-ramp-to-field"]', "あ", "Enter");
    expect(q('[data-test="inspector-ramp-to-field"]')!.classList.contains("broken")).toBe(true);
    expect(app.sent).toEqual([{ type: "rejectValue", field: "velocity", kept: "110", range: null }]);
    app.sent.length = 0;
    type('[data-test="inspector-percent-field"]', "x", "Enter");
    expect(app.sent).toEqual([{ type: "rejectValue", field: "percent", kept: "100", range: null }]);
    app.sent.length = 0;
    type('[data-test="inspector-percent-field"]', "1000", "Enter");
    expect(app.sent).toEqual([
      { type: "scaleVelocity", take: take.id, notes: [14, 15, 16], percent: 300 },
      { type: "clampValue", field: "percent", value: "300", range: null },
    ]);
    app.sent.length = 0;
    type('[data-test="inspector-spread-field"]', "", "Enter");
    expect(app.sent).toEqual([{ type: "rejectValue", field: "amount", kept: "8", range: null }]);
    app.sent.length = 0;
    type('[data-test="inspector-spread-field"]', "40", "Enter");
    expect(app.sent).toEqual([
      { type: "randomVelocity", take: take.id, notes: [14, 15, 16], amount: 32, seed: expect.any(Number) },
      { type: "clampValue", field: "amount", value: "32", range: null },
    ]);
  });

  it("↑ ↓ step (Shift by 10), Esc puts back the last value used", async () => {
    await scene();
    const sel = '[data-test="inspector-spread-field"]';
    type(sel, "8", "ArrowUp");
    expect(q<HTMLInputElement>(sel)!.value).toBe("9");
    type(sel, "9", "ArrowUp", true);
    expect(q<HTMLInputElement>(sel)!.value).toBe("19");
    type(sel, "19", "Escape");
    expect(q<HTMLInputElement>(sel)!.value).toBe("8");
    expect(app.sent).toEqual([]);
  });

  it("are disabled with no notes selected", async () => {
    await scene((s) => (s.pianoRoll.selectedNotes = []));
    expect(q<HTMLInputElement>('[data-test="inspector-percent-field"]')!.disabled).toBe(true);
    for (const test of ["inspector-scale-velocity", "inspector-ramp-velocity", "inspector-random-velocity", "inspector-legato"]) {
      expect(q(`[data-test="${test}"]`)!.getAttribute("aria-disabled")).toBe("true");
      q(`[data-test="${test}"]`)!.click();
    }
    expect(app.sent).toEqual([]);
  });
});

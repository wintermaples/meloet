// The helper that moves the piano roll by itself, only when measuring (ui/src/dev/script.ts). It sends real events to the note area part.
import { describe, expect, it } from "vitest";
import { parseUiScript, SCRIPT_REPORT, UiScript, type Clock, type ScriptRecord, type Target } from "../src/dev/script";

describe("reading MELOET_UI_SCRIPT", () => {
  it("names and counts joined with , . 100 when there is no count", () => {
    expect(parseUiScript("pr-scroll:20")).toEqual([{ kind: "scroll", count: 20 }]);
    expect(parseUiScript("pr-zoom, pr-drag:1000")).toEqual([
      { kind: "zoom", count: 100 },
      { kind: "drag", count: 1000 },
    ]);
  });

  it("does nothing if even one item cannot be read", () => {
    for (const bad of ["", "pr-fly:3", "pr-scroll:0", "pr-scroll:-1", "pr-scroll:1.5", "pr-scroll:2:3", "pr-scroll:x"]) {
      expect(parseUiScript(bad), bad).toBeNull();
    }
  });
});

/** The clock moves by hand. frame and wait return at once. */
function fakeClock(): Clock {
  let t = 0;
  return {
    now: () => (t += 1),
    wall: () => 1000 + t,
    wait: async () => {},
    frame: async () => {},
    settle: async () => {},
    everyFrame: () => () => {},
  };
}

function setup(over: Partial<Target> = {}) {
  const area = document.createElement("div");
  document.body.append(area);
  const events: { type: string; x: number; y: number; shift: boolean; ctrl: boolean; deltaY: number; pointerId?: number; buttons: number }[] = [];
  let scrollTick = 0;
  let zoom = 3;
  let take = 1;
  const record = (e: Event) => {
    const m = e as MouseEvent & { deltaY?: number; pointerId?: number };
    events.push({ type: e.type, x: m.clientX, y: m.clientY, shift: m.shiftKey, ctrl: m.ctrlKey, deltaY: m.deltaY ?? 0, pointerId: m.pointerId, buttons: m.buttons });
  };
  area.addEventListener("wheel", (e) => {
    record(e);
    // horizontally it moves only between 0 and 2 (stops at the ends). Zoom is 0 to 4.
    if (e.shiftKey) scrollTick = Math.min(2, Math.max(0, scrollTick + Math.sign(e.deltaY)));
    if (e.ctrlKey) zoom = Math.min(4, Math.max(0, zoom - Math.sign(e.deltaY)));
  });
  for (const type of ["pointerdown", "pointermove"]) area.addEventListener(type, record);
  area.addEventListener("pointerup", (e) => {
    record(e);
    take += 1;
  });
  const target: Target = {
    area: () => area,
    ready: () => true,
    grab: () => ({ x: 100, y: 50, dx: 12 }),
    scrollTick: () => scrollTick,
    zoom: () => zoom,
    shownTake: () => take,
    reading: () => "+0.120 ・ +0 半音",
    ...over,
  };
  const reports: ScriptRecord[] = [];
  const report = async (name: string, json: string) => {
    expect(name).toBe(SCRIPT_REPORT);
    reports.push(JSON.parse(json) as ScriptRecord);
  };
  return { area, events, target, reports, report };
}

describe("the helper that moves by itself", () => {
  it("scrolling across: sends Shift + wheel to the note area and turns at the ends. Counts only the steps that moved", async () => {
    const s = setup();
    const script = new UiScript("pr-scroll:5", [{ kind: "scroll", count: 5 }], s.target, fakeClock(), s.report);
    await script.run();
    const wheels = s.events.filter((e) => e.type === "wheel");
    expect(wheels.every((e) => e.shift && !e.ctrl)).toBe(true);
    // 0→1→2, end (one turn that does not move), 2→1→0, end, 0→1
    expect(wheels.map((e) => Math.sign(e.deltaY))).toEqual([1, 1, 1, -1, -1, -1, 1]);
    const last = s.reports.at(-1)!;
    expect(last.done).toBe(true);
    expect(last.parts[0]!.steps).toHaveLength(5);
    expect(last.parts[0]!.steps.every((st) => st.changed)).toBe(true);
    expect(last.parts[0]!.marks).toEqual([expect.objectContaining({ done: 5 })]);
  });

  it("zoom: Mod (Ctrl) + wheel", async () => {
    const s = setup();
    await new UiScript("pr-zoom:3", [{ kind: "zoom", count: 3 }], s.target, fakeClock(), s.report).run();
    const wheels = s.events.filter((e) => e.type === "wheel");
    expect(wheels.every((e) => e.ctrl && !e.shift)).toBe(true);
    expect(s.reports.at(-1)!.parts[0]!.steps.every((st) => st.changed)).toBe(true);
  });

  it("vertical steps: with Mod Shift + wheel, moves to the set row height, then the next", async () => {
    const steps = [8, 12, 16, 20, 24];
    let at = 1;
    const s = setup({ rowPx: () => steps[at]! });
    s.area.addEventListener("wheel", (e) => {
      if (e.ctrlKey && e.shiftKey) at = Math.min(4, Math.max(0, at - Math.sign(e.deltaY)));
    });
    expect(parseUiScript("pr-rows:24,pr-scroll:1")).toEqual([
      { kind: "rows", count: 24 },
      { kind: "scroll", count: 1 },
    ]);
    await new UiScript("pr-rows:24", [{ kind: "rows", count: 24 }], s.target, fakeClock(), s.report).run();
    expect(steps[at]).toBe(24);
    const wheels = s.events.filter((e) => e.type === "wheel");
    expect(wheels).toHaveLength(3);
    expect(wheels.every((e) => e.ctrl && e.shift && e.deltaY < 0)).toBe(true);
    expect(s.reports.at(-1)!.done).toBe(true);
  });

  it("drag: press → move 4 times → release. Repeats there and back, and waits until the new take is seen", async () => {
    const s = setup();
    await new UiScript("pr-drag:2", [{ kind: "drag", count: 2 }], s.target, fakeClock(), s.report).run();
    expect(s.events.map((e) => e.type)).toEqual([
      ...["pointerdown", "pointermove", "pointermove", "pointermove", "pointermove", "pointerup"],
      ...["pointerdown", "pointermove", "pointermove", "pointermove", "pointermove", "pointerup"],
    ]);
    expect(s.events.every((e) => e.pointerId === 1)).toBe(true);
    expect(s.events.slice(0, 6).map((e) => e.x)).toEqual([100, 103, 106, 109, 112, 112]);
    expect(s.events.slice(6).map((e) => e.x)).toEqual([100, 97, 94, 91, 88, 88]);
    expect(s.events[0]!.buttons).toBe(1);
    expect(s.events[5]!.buttons).toBe(0);
    const steps = s.reports.at(-1)!.parts[0]!.steps;
    expect(steps.map((st) => st.changed)).toEqual([true, true]);
    expect(steps.every((st) => typeof st.replyMs === "number")).toBe(true);
  });

  it("if the piano roll does not open, sends nothing and writes that down", async () => {
    const s = setup({ ready: () => false });
    let t = 0;
    const clock = { ...fakeClock(), now: () => (t += 10000) };
    await new UiScript("pr-scroll:1", [{ kind: "scroll", count: 1 }], s.target, clock, s.report).run();
    expect(s.events).toEqual([]);
    expect(s.reports.at(-1)).toMatchObject({ done: false, error: expect.stringContaining("did not show") });
  });

  it("after stopping, sends nothing more", async () => {
    const s = setup();
    const script = new UiScript("pr-scroll:5", [{ kind: "scroll", count: 5 }], s.target, fakeClock(), s.report);
    script.stop();
    await script.run();
    expect(s.events).toEqual([]);
  });
});

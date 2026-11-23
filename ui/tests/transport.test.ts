// Mounts and checks the transport band, the status line tag and notices, the input meter's play target while recording, and the hint above the arrange.
import { flushSync, mount, tick, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, StatusMessage, TransportReading } from "../src/bridge";
import InputMeter from "../src/components/InputMeter.svelte";
import SoundHint from "../src/components/SoundHint.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import Transport from "../src/components/Transport.svelte";
import { closeMenu, menuVisible } from "../src/lib/menu";
import { tipView } from "../src/lib/tip.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;

beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (menuVisible()) closeMenu();
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  document.body.innerHTML = "";
});

/** Removes the earlier mount first (if one test calls show many times, what stays reads the state of later tests and fails). */
function show(component: Component<any>, state: AppState): void {
  if (mounted) unmount(mounted);
  mounted = null;
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}

const q = <T extends HTMLElement = HTMLElement>(sel: string) => document.querySelector<T>(sel);
const text = (sel: string) => q(sel)?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const btn = (name: string) => q<HTMLButtonElement>(`[data-test="transport-${name}"]`)!;
const disabled = (name: string) => btn(name).getAttribute("aria-disabled") === "true";

const KEYBOARD = { id: "kb", name: "鍵盤 49", portName: "鍵盤 49 MIDI 1", address: "24:0" };
const PIANO = { number: 1, name: "ピアノ" };

/** With a keyboard and an output, and the piano armed. */
const ready = (edit: (s: AppState) => void = () => {}) =>
  app.stateWith((s) => {
    s.keyboards = { ...s.keyboards, list: [KEYBOARD], active: "kb", lost: null, inputStopped: false };
    s.outputs.chosen = s.outputs.list[0]!.id;
    s.transport.canPlay = true;
    s.transport.canRecord = true;
    edit(s);
  });

const reading = (r: Partial<TransportReading>): TransportReading => ({
  tick: 0,
  ticksPerUs: 0,
  state: "stopped",
  countInBeatsLeft: 0,
  lap: 0,
  ...r,
});

const BAR = 4 * TPQ;

describe("transport buttons", () => {
  it("no output yet: ▶ and ● look disabled and send nothing when pressed. ⏮ ■ loop and metronome work", () => {
    show(Transport, app.stateWith(() => {}));
    expect(disabled("play")).toBe(true);
    expect(disabled("record")).toBe(true);
    for (const b of ["to-start", "stop", "loop", "metronome"]) expect(disabled(b), b).toBe(false);
    btn("play").click();
    btn("record").click();
    expect(app.sent).toEqual([]);
    btn("to-start").click();
    btn("loop").click();
    btn("metronome").click();
    expect(app.sent).toEqual([{ type: "goToStart" }, { type: "toggleLoop" }, { type: "toggleMetronome" }]);
  });

  it("stopped: ■ is bright, ▶ sends playStop, ■ sends nothing. ● is a rec ring", () => {
    show(Transport, ready());
    expect(btn("stop").classList.contains("hi")).toBe(true);
    expect(btn("play").classList.contains("hi")).toBe(false);
    expect(btn("record").classList.contains("recording")).toBe(false);
    btn("stop").click();
    btn("play").click();
    btn("record").click();
    expect(app.sent).toEqual([{ type: "playStop" }, { type: "record" }]);
  });

  it("playing: ▶ is bright, ■ sends playStop, ▶ sends nothing", () => {
    show(Transport, ready((s) => (s.transport.state = "playing")));
    expect(btn("play").classList.contains("hi")).toBe(true);
    expect(btn("stop").classList.contains("hi")).toBe(false);
    btn("play").click();
    btn("stop").click();
    expect(app.sent).toEqual([{ type: "playStop" }]);
  });

  it("no armed track: ● looks disabled but pressing sends record (Rust gives the reason)", () => {
    show(Transport, ready((s) => (s.transport.canRecord = false)));
    expect(disabled("record")).toBe(true);
    btn("record").click();
    expect(app.sent).toEqual([{ type: "record" }]);
  });

  it("while recording and during count-in, ● is filled with rec", () => {
    show(Transport, ready((s) => (s.transport.state = "recording")));
    expect(btn("record").classList.contains("recording")).toBe(true);
    expect(disabled("record")).toBe(false);
    app.setState(ready((s) => (s.transport.state = "countIn")));
    flushSync();
    expect(btn("record").classList.contains("recording")).toBe(true);
  });

  it("loop and metronome on are accent (on)", () => {
    show(
      Transport,
      ready((s) => {
        s.song.loopOn = true;
        s.transport.metronome = true;
      }),
    );
    expect(btn("loop").classList.contains("on")).toBe(true);
    expect(btn("metronome").classList.contains("on")).toBe(true);
    app.setState(ready());
    flushSync();
    expect(btn("loop").classList.contains("on")).toBe(false);
    expect(btn("metronome").classList.contains("on")).toBe(false);
  });
});

describe("position and time", () => {
  it("transport.position with no meter values", () => {
    show(Transport, ready((s) => (s.transport.position = 4 * BAR)));
    expect(text('[data-test="transport-position"]')).toBe("5. 1. 0000");
    expect(text('[data-test="transport-time"]')).toBe("00:00:10.000");
  });

  it("with meter values, that position (7. 3. 240, 00:00:16.562 of mock F-ArrangeRec. Ticks have 4 digits)", () => {
    show(Transport, ready((s) => (s.transport.state = "recording")));
    app.setTelemetry(app.telemetryWith({ position: reading({ tick: 6 * BAR + 2 * TPQ + TPQ / 2, state: "recording" }) }));
    flushSync();
    expect(text('[data-test="transport-position"]')).toBe("7. 3. 0960");
    expect(text('[data-test="transport-time"]')).toBe("00:00:16.562");
  });

  it("during count-in, stays at the position where recording starts", () => {
    show(
      Transport,
      ready((s) => {
        s.transport.state = "countIn";
        s.transport.position = 4 * BAR;
        s.transport.recording = { id: 1, track: 1, from: 4 * BAR, take: 1, clip: null, lap: 0, overdub: false };
      }),
    );
    app.setTelemetry(app.telemetryWith({ position: reading({ tick: 4 * BAR - (3 * TPQ) / 2, state: "countIn", countInBeatsLeft: 2 }) }));
    flushSync();
    expect(text('[data-test="transport-position"]')).toBe("5. 1. 0000");
  });

  it("converts with time signature and tempo (6/8, 120.00)", () => {
    show(
      Transport,
      ready((s) => {
        s.timeline.meterNum = 6;
        s.timeline.meterDen = 8;
        s.timeline.tempoBpm = 120;
        s.transport.position = 3 * TPQ + TPQ / 2 + 80;
      }),
    );
    expect(text('[data-test="transport-position"]')).toBe("2. 2. 0080");
    // 3.5416… beats (quarter notes); the tempo counts quarter notes, so at 120 it is 1.7708… seconds
    expect(text('[data-test="transport-time"]')).toBe("00:00:01.770");
  });
});

describe("count-in", () => {
  // jsdom has no width, so the band is 1920 wide (so that the count-in is not hidden).
  beforeEach(() => {
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(1920);
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("a button when stopped. Bar counts and 「なし」", () => {
    show(Transport, ready());
    expect(text('[data-test="transport-count-in"]')).toBe("カウントイン1 小節");
    app.setState(ready((s) => (s.transport.countInBars = 0)));
    flushSync();
    expect(text('[data-test="transport-count-in"]')).toBe("カウントインなし");
  });

  it("pressing opens the list; choosing sends setCountIn and closes it. It joins 「メニュー」 in the Esc order", async () => {
    show(Transport, ready());
    btn("count-in").click();
    flushSync();
    await tick();
    const menu = q('[data-test="count-in-menu"]')!;
    expect(menu).not.toBeNull();
    expect(menuVisible()).toBe(true);
    const rows = [...menu.querySelectorAll<HTMLButtonElement>("button")];
    expect(rows.map((r) => r.textContent!.trim())).toEqual(["なし", "1 小節", "2 小節"]);
    expect(rows.map((r) => r.getAttribute("aria-checked"))).toEqual(["false", "true", "false"]);
    expect(menu.querySelector(".head")!.textContent!.trim()).toBe("カウントイン");
    rows[2]!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "setCountIn", bars: 2 }]);
    expect(q('[data-test="count-in-menu"]')).toBeNull();
    expect(menuVisible()).toBe(false);
  });

  it("the list can be chosen with ↑ ↓ Enter. Esc (closeMenu) closes it", async () => {
    show(Transport, ready());
    btn("count-in").click();
    flushSync();
    await new Promise((r) => setTimeout(r, 0));
    const menu = q('[data-test="count-in-menu"]')!;
    expect(document.activeElement?.textContent?.trim()).toBe("1 小節");
    menu.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowUp", bubbles: true }));
    expect(document.activeElement?.textContent?.trim()).toBe("なし");
    document.activeElement!.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    flushSync();
    expect(app.sent).toEqual([{ type: "setCountIn", bars: 0 }]);

    btn("count-in").click();
    flushSync();
    expect(q('[data-test="count-in-menu"]')).not.toBeNull();
    closeMenu();
    flushSync();
    expect(q('[data-test="count-in-menu"]')).toBeNull();
  });

  it("during count-in, the on button shows 「カウントイン」 and squares ■■■□. No 「3 / 4」 text (spec 3, mock F-Connect4)", () => {
    show(
      Transport,
      ready((s) => {
        s.transport.state = "countIn";
        s.transport.recording = { id: 1, track: 1, from: 0, take: 1, clip: null, lap: 0, overdub: false };
      }),
    );
    app.setTelemetry(app.telemetryWith({ position: reading({ tick: -(3 * TPQ) / 2, state: "countIn", countInBeatsLeft: 2 }) }));
    flushSync();
    const counting = q('[data-test="transport-counting"]')!;
    expect(text('[data-test="transport-counting"]')).toBe("カウントイン");
    expect(counting.classList.contains("m-btn")).toBe(true);
    expect(counting.classList.contains("on")).toBe(true);
    const boxes = [...counting.querySelectorAll(".box")];
    expect(boxes.map((b) => b.classList.contains("done"))).toEqual([true, true, true, false]);
    expect(q('[data-test="transport-count-in"]')).toBeNull();
  });

  it("the squares are the metronome clicks per bar (22-2): 4 for 12/8, 2 for 6/8, 5 for 5/8, 2 for 2/2", () => {
    for (const [num, den, n] of [
      [12, 8, 4],
      [6, 8, 2],
      [5, 8, 5],
      [2, 2, 2],
    ] as const) {
      show(
        Transport,
        ready((s) => {
          s.timeline.meterNum = num;
          s.timeline.meterDen = den;
          s.transport.state = "countIn";
          s.transport.countInBeats = n;
          s.transport.recording = { id: 1, track: 1, from: 0, take: 1, clip: null, lap: 0, overdub: false };
        }),
      );
      app.setTelemetry(app.telemetryWith({ position: reading({ tick: -100, state: "countIn", countInBeatsLeft: 1 }) }));
      flushSync();
      const boxes = [...q('[data-test="transport-counting"]')!.querySelectorAll(".box")];
      expect(boxes.map((b) => b.classList.contains("done")), `${num}/${den}`).toEqual(Array(n).fill(true));
    }
  });

  it("even with a 2-bar count-in the box is one bar. It counts again each bar", () => {
    show(
      Transport,
      ready((s) => {
        s.transport.state = "countIn";
        s.transport.countInBars = 2;
        s.transport.countInBeats = 8;
        s.transport.recording = { id: 1, track: 1, from: 0, take: 1, clip: null, lap: 0, overdub: false };
      }),
    );
    app.setTelemetry(app.telemetryWith({ position: reading({ tick: -3 * TPQ, state: "countIn", countInBeatsLeft: 3 }) }));
    flushSync();
    const boxes = [...q('[data-test="transport-counting"]')!.querySelectorAll(".box")];
    expect(boxes.map((b) => b.classList.contains("done"))).toEqual([true, true, false, false]);
  });
});

describe("when there is not enough width (spec 1 (1), spec 2 (Song ▾, Metronome ▾), mock F-Menus)", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  /** jsdom has no width, so the width of each part and of the band are set. The key at the right end of a button is width 0 (counted in the button width). */
  function widths(row: number, part: (name: string | undefined) => number): void {
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(row);
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function (this: HTMLElement) {
      if (this.classList.contains("key") || this.dataset.part?.startsWith("key-")) return 0;
      return part(this.dataset.part);
    });
  }
  const folded = () => widths(0, () => 10);

  it("when the song name hides, a 28-wide ▾. Pressing gives the song name list (the mark is empty)", async () => {
    folded();
    show(Transport, ready());
    expect(q('[data-test="transport-song"]')).toBeNull();
    const stub = btn("song-stub");
    expect(stub.textContent).toBe("▾");
    expect(stub.getAttribute("aria-label")).toBe("曲（はじめての曲）");
    stub.click();
    flushSync();
    await tick();
    const menu = q('[data-test="song-menu"]')!;
    const rows = [...menu.querySelectorAll<HTMLButtonElement>("button")];
    expect(rows.map((r) => r.querySelector(".label")!.textContent)).toEqual(["新しい曲", "ひらく…", "保存"]);
    expect(rows.every((r) => r.querySelector(".choice:not(.selected)") !== null)).toBe(true);
    expect(rows.map((r) => r.getAttribute("aria-checked"))).toEqual([null, null, null]);
    rows[2]!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "saveSong" }]);
  });

  it("hovering the song ▾ shows 「曲 ・ （曲の名前）」", () => {
    vi.useFakeTimers();
    folded();
    show(Transport, ready());
    btn("song-stub").dispatchEvent(new PointerEvent("pointerenter"));
    vi.advanceTimersByTime(600);
    flushSync();
    expect(tipView.value?.body).toBe("曲 ・ はじめての曲");
    expect(tipView.value?.term).toBeNull();
  });

  it("the slim master drops 「dB」 (−∞), hover shows 「マスター ・ −∞ dB」. The normal form is 「−∞ dB」 with a term tip (spec 3)", () => {
    vi.useFakeTimers();
    folded();
    show(Transport, ready());
    const db = () => q('[data-test="transport-master-db"]')!;
    expect(q(".master")!.classList.contains("narrow")).toBe(true);
    expect(db().textContent).toBe("−∞");
    db().dispatchEvent(new PointerEvent("pointerenter"));
    vi.advanceTimersByTime(600);
    flushSync();
    expect(tipView.value?.body).toBe("マスター ・ −∞ dB");
    expect(tipView.value?.term).toBeNull();
    db().dispatchEvent(new PointerEvent("pointerleave"));
    unmount(mounted!);
    mounted = null;

    vi.restoreAllMocks();
    widths(1920, () => 10);
    show(Transport, ready());
    expect(q(".master")!.classList.contains("narrow")).toBe(false);
    expect(db().textContent).toBe("−∞ dB");
    db().dispatchEvent(new PointerEvent("pointerenter"));
    vi.advanceTimersByTime(600);
    flushSync();
    expect(tipView.value?.term).toBe("dB");
  });

  it("when the count-in hides, the metronome ▾. When the metronome is on, the ▾ is on too", () => {
    folded();
    show(Transport, ready((s) => (s.transport.metronome = true)));
    expect(q('[data-test="transport-count-in"]')).toBeNull();
    const stub = btn("metronome-stub");
    expect(stub.getAttribute("aria-label")).toBe("メトロノームとカウントイン");
    expect(stub.classList.contains("on")).toBe(true);
    expect(stub.previousElementSibling).toBe(btn("metronome"));
    app.setState(ready());
    flushSync();
    expect(btn("metronome-stub").classList.contains("on")).toBe(false);
  });

  it("the metronome ▾ list: metronome (C), heading 「カウントイン」, none, 1 bar, 2 bars", async () => {
    folded();
    show(Transport, ready((s) => (s.transport.metronome = true)));
    btn("metronome-stub").click();
    flushSync();
    await tick();
    const menu = q('[data-test="metronome-menu"]')!;
    expect(menu.getAttribute("aria-label")).toBe("メトロノームとカウントイン");
    expect([...menu.children].map((c) => c.textContent!.replace(/\s+/g, " ").trim())).toEqual([
      "メトロノーム C",
      "カウントイン",
      "なし",
      "1 小節",
      "2 小節",
    ]);
    expect(menu.children[1]!.classList.contains("head")).toBe(true);
    const rows = [...menu.querySelectorAll<HTMLButtonElement>("button")];
    expect(rows.map((r) => r.getAttribute("aria-checked"))).toEqual(["true", "false", "true", "false"]);
    rows[3]!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "setCountIn", bars: 2 }]);
    expect(q('[data-test="metronome-menu"]')).toBeNull();
    btn("metronome-stub").click();
    flushSync();
    await tick();
    q('[data-test="metronome-menu"]')!.querySelector<HTMLButtonElement>("button")!.click();
    flushSync();
    expect(app.sent.at(-1)).toEqual({ type: "toggleMetronome" });
  });

  it("a list that goes outside the window moves left and up until it fits", async () => {
    folded();
    vi.spyOn(window, "innerWidth", "get").mockReturnValue(1280);
    vi.spyOn(window, "innerHeight", "get").mockReturnValue(800);
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
      // the list is below the button; its right end is outside the window (1200 + 220), and its bottom too (700 + 150).
      const rect = this.classList.contains("menu") ? { left: 1200, top: 700, width: 220, height: 150 } : { left: 0, top: 0, width: 0, height: 0 };
      return { ...rect, right: rect.left + rect.width, bottom: rect.top + rect.height, x: rect.left, y: rect.top, toJSON: () => ({}) } as DOMRect;
    });
    show(Transport, ready());
    btn("metronome-stub").click();
    flushSync();
    await new Promise((r) => setTimeout(r, 0));
    flushSync();
    expect(q<HTMLElement>('[data-test="metronome-menu"]')!.style.transform).toBe("translate(-140px, -50px)");
  });

  it("a list that fits in the window does not move", async () => {
    folded();
    vi.spyOn(window, "innerWidth", "get").mockReturnValue(1280);
    vi.spyOn(window, "innerHeight", "get").mockReturnValue(800);
    show(Transport, ready());
    btn("song-stub").click();
    flushSync();
    await new Promise((r) => setTimeout(r, 0));
    flushSync();
    expect(q<HTMLElement>('[data-test="song-menu"]')!.style.transform).toBe("");
  });

  it("what hides does not change when the count-in starts (the count-in width is the bigger of normal and during count-in)", () => {
    const shown = () =>
      ["transport-song", "transport-song-stub", "transport-metronome-stub"].map((t) => q(`[data-test="${t}"]`) !== null).join();
    const seen = new Set<string>();
    for (let room = 300; room <= 900; room += 10) {
      vi.restoreAllMocks();
      widths(room, (part) => (part === "countInCounting" ? 140 : part === "countIn" ? 100 : 40));
      show(Transport, ready());
      const before = shown();
      seen.add(before);
      app.setState(
        ready((s) => {
          s.transport.state = "countIn";
          s.transport.recording = { id: 1, track: 1, from: 0, take: 1, clip: null, lap: 0, overdub: false };
        }),
      );
      flushSync();
      expect(shown(), String(room)).toBe(before);
      unmount(mounted!);
      mounted = null;
    }
    expect([...seen].sort()).toEqual(["false,true,false", "false,true,true", "true,false,false"]);
  });
});

describe("typing tempo and time signature", () => {
  const field = (name: "tempo" | "meter") => q<HTMLInputElement>(`input[data-test="transport-${name}"]`);
  const press = (el: HTMLElement, key: string, more: KeyboardEventInit = {}) => {
    const ev = new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true, ...more });
    el.dispatchEvent(ev);
    flushSync();
    return ev;
  };
  const type = (el: HTMLInputElement, value: string) => {
    el.value = value;
    el.dispatchEvent(new Event("input", { bubbles: true }));
    flushSync();
  };
  const open = async (name: "tempo" | "meter") => {
    q<HTMLButtonElement>(`button[data-test="transport-${name}"]`)!.click();
    flushSync();
    await tick();
    return field(name)!;
  };

  it("pressing turns it into an input box with the current value selected. Enter sends setTempo", async () => {
    show(Transport, ready());
    expect(field("tempo")).toBeNull();
    const input = await open("tempo");
    expect(input.value).toBe("96.00");
    expect(document.activeElement).toBe(input);
    type(input, "120.5");
    expect(press(input, "Enter").defaultPrevented).toBe(true);
    expect(app.sent).toEqual([{ type: "setTempo", bpm: 120.5 }]);
    await tick();
    expect(field("tempo")).toBeNull();
  });

  it("Esc cancels (sends nothing)", async () => {
    show(Transport, ready());
    const input = await open("tempo");
    type(input, "120");
    press(input, "Escape");
    await tick();
    expect(field("tempo")).toBeNull();
    expect(app.sent).toEqual([]);
  });

  it("a value that cannot be read as a number gets the broken border and is not set even with Enter. Asks the status line to say so (the original value)", async () => {
    show(Transport, ready());
    const input = await open("tempo");
    for (const bad of ["abc", "", "1e2", "12a"]) {
      type(input, bad);
      expect(input.classList.contains("broken"), bad).toBe(true);
      expect(input.getAttribute("aria-invalid")).toBe("true");
      press(input, "Enter");
    }
    expect(app.sent).toEqual(Array(4).fill({ type: "rejectValue", field: "tempo", kept: "96.00", range: null }));
    expect(field("tempo")).not.toBeNull();
    type(input, "29.99");
    expect(input.classList.contains("broken")).toBe(false);
  });

  it("a number outside the range is set to the nearest end, then clampValue (B-68). Decimals are rounded to 2 places", async () => {
    show(Transport, ready());
    for (const [typed, bpm, said] of [
      ["29.99", 30, "30.00"],
      ["999", 300, "300.00"],
      ["-5", 30, "30.00"],
      ["120.555", 120.56, null],
    ] as const) {
      app.sent.length = 0;
      const input = await open("tempo");
      type(input, typed);
      press(input, "Enter");
      expect(app.sent, typed).toEqual([
        { type: "setTempo", bpm },
        ...(said ? [{ type: "clampValue", field: "tempo", value: said, range: null }] : []),
      ]);
      await tick();
    }
  });

  it("↑ ↓ by 1, Shift by 10. Does not go outside the range", async () => {
    show(Transport, ready());
    const input = await open("tempo");
    press(input, "ArrowUp");
    expect(input.value).toBe("97.00");
    press(input, "ArrowDown", { shiftKey: true });
    expect(input.value).toBe("87.00");
    type(input, "295.5");
    press(input, "ArrowUp", { shiftKey: true });
    expect(input.value).toBe("300.00");
    expect(app.sent).toEqual([]);
  });

  it("time signature: Enter sends setMeter, ↑ ↓ change the top number, unreadable values are not set", async () => {
    show(Transport, ready());
    const input = await open("meter");
    expect(input.value).toBe("4/4");
    press(input, "ArrowUp");
    expect(input.value).toBe("5/4");
    type(input, "6/あ");
    expect(input.classList.contains("broken")).toBe(true);
    press(input, "Enter");
    type(input, "4");
    press(input, "Enter");
    expect(app.sent).toEqual([
      { type: "rejectValue", field: "meterDen", kept: "4", range: null },
      { type: "rejectValue", field: "meterNum", kept: "4", range: null },
    ]);
    type(input, "6/8");
    press(input, "Enter");
    expect(app.sent.at(-1)).toEqual({ type: "setMeter", num: 6, den: 8 });
  });

  it("time signature outside the range: top 1 to 12, bottom the nearest of 2, 4, 8, 16 (B-68)", async () => {
    show(Transport, ready());
    for (const [typed, num, den, said] of [
      ["13/4", 12, 4, { field: "meterNum", value: "12" }],
      ["0/4", 1, 4, { field: "meterNum", value: "1" }],
      ["6/7", 6, 8, { field: "meterDen", value: "8" }],
      ["3/3", 3, 4, { field: "meterDen", value: "4" }],
      ["3/99", 3, 16, { field: "meterDen", value: "16" }],
      ["99/1", 12, 2, { field: "meterNum", value: "12" }],
    ] as const) {
      app.sent.length = 0;
      const input = await open("meter");
      type(input, typed);
      expect(input.classList.contains("broken")).toBe(false);
      press(input, "Enter");
      expect(app.sent, typed).toEqual([
        { type: "setMeter", num, den },
        { type: "clampValue", ...said, range: null },
      ]);
      await tick();
    }
  });

  it("position 「1. 1. 0000」 is an input box too (B-61). 「5」「5.2」「5.2.0960」 send locate, ↑ ↓ are not used, Esc cancels", async () => {
    show(Transport, ready());
    const openPos = async () => {
      q<HTMLButtonElement>(`button[data-test="transport-position"]`)!.click();
      flushSync();
      await tick();
      return q<HTMLInputElement>(`input[data-test="transport-position"]`)!;
    };
    let input = await openPos();
    expect(input.value).toBe("1. 1. 0000");
    expect(press(input, "ArrowUp").defaultPrevented).toBe(false);
    expect(input.value).toBe("1. 1. 0000");
    press(input, "Escape");
    await tick();
    expect(app.sent).toEqual([]);
    for (const [typed, tick_] of [
      ["5", 4 * BAR],
      ["5.2", 4 * BAR + TPQ],
      ["5. 2. 0960", 4 * BAR + (3 * TPQ) / 2],
      ["５．２．０９６０", 4 * BAR + (3 * TPQ) / 2],
      ["5.2.960", 4 * BAR + (3 * TPQ) / 2],
    ] as const) {
      input = await openPos();
      type(input, typed);
      press(input, "Enter");
      expect(app.sent.at(-1), typed).toEqual({ type: "locate", tick: tick_ });
      await tick();
    }
    input = await openPos();
    type(input, "5.x");
    expect(input.classList.contains("broken")).toBe(true);
    press(input, "Enter");
    expect(app.sent.at(-1)).toEqual({ type: "rejectValue", field: "playPosition", kept: "1. 1. 0000", range: null });
  });

  it("position outside the range: bar, beat and tick are each rounded, and only the biggest unit is mentioned (B-68)", async () => {
    show(Transport, ready());
    for (const [typed, tick_, said] of [
      ["3.7", 2 * BAR + 3 * TPQ, { field: "beat", value: "3. 4. 0000", range: "1〜4" }],
      ["3.2.9999", 2 * BAR + TPQ + TPQ - 1, { field: "tick", value: "3. 2. 1919", range: "0〜1919" }],
      ["0.9.9999", 3 * TPQ + TPQ - 1, { field: "bar", value: "1. 4. 1919", range: null }],
      ["2.0", BAR, { field: "beat", value: "2. 1. 0000", range: "1〜4" }],
    ] as const) {
      app.sent.length = 0;
      q<HTMLButtonElement>(`button[data-test="transport-position"]`)!.click();
      flushSync();
      await tick();
      const input = q<HTMLInputElement>(`input[data-test="transport-position"]`)!;
      type(input, typed);
      press(input, "Enter");
      expect(app.sent, typed).toEqual([{ type: "locate", tick: tick_ }, { type: "clampValue", ...said }]);
      await tick();
    }
  });

  it("leaving the box cancels", async () => {
    show(Transport, ready());
    const input = await open("meter");
    type(input, "3/4");
    input.blur();
    flushSync();
    expect(field("meter")).toBeNull();
    expect(app.sent).toEqual([]);
  });
});

describe("status line tag and notices", () => {
  const badge = () => q('[data-test="status-badge"]')!;
  const left = () => text("footer.status")!;

  it.each([
    ["stopped", "■ 停止中", false],
    ["playing", "▶ 再生中", false],
    ["recording", "● 録音中", true],
    ["countIn", "● カウントイン中", true],
  ] as const)("%s: 「%s」", (state, label, rec) => {
    show(StatusLine, ready((s) => (s.transport.state = state)));
    expect(badge().textContent).toBe(label);
    expect(badge().classList.contains("rec")).toBe(rec);
  });

  const msg = (m: StatusMessage, edit: (s: AppState) => void = () => {}) =>
    ready((s) => {
      s.status.message = m;
      edit(s);
    });

  it.each([
    [{ id: "recording", track: PIANO, take: 2 }, "1 ピアノ へ録音 ・ 止めると テイク 2 として残ります"],
    [
      { id: "loopRecording", track: PIANO, fromBar: 5, toBar: 9, lap: 2 },
      "1 ピアノ へ録音 ・ ループ 5–9 の 2 周目 ・ 周ごとにテイクを分けています",
    ],
    [
      { id: "takeSwitched", track: PIANO, clip: "Aメロ", take: 2, takes: 3 },
      "1 ピアノ ・ Aメロ をテイク 2 / 3 に切りかえました",
    ],
    [{ id: "noArmedTrack" }, "録るトラックが決まっていません。トラックの ● を押すと、そこに録ります。"],
    [{ id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: true, notes: null }, "ループ 5–9 ・ 選択なし"],
    [{ id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: false, notes: null }, "ループ 5–9"],
    [{ id: "summary", loopBars: null, nothingSelected: true, notes: null }, "選択なし"],
    [
      { id: "outputChosen", output: "アナログヘッドフォン", track: PIANO },
      "出口をアナログヘッドフォンにしました。1 ピアノ が録音待機になっています。",
    ],
    [{ id: "outputChosen", output: "アナログヘッドフォン", track: null }, "出口をアナログヘッドフォンにしました。"],
    [
      { id: "newSong", tempoBpm: 96, meterNum: 4, meterDen: 4, track: PIANO },
      "新しい曲 ・ テンポ 96.00 ・ 4/4 ・ 1 ピアノ が録音待機",
    ],
    [{ id: "newSong", tempoBpm: 120.5, meterNum: 6, meterDen: 8, track: null }, "新しい曲 ・ テンポ 120.50 ・ 6/8"],
  ] as [StatusMessage, string][])("%o", (m, expected) => {
    show(StatusLine, msg(m));
    expect(left()).toContain(`■ 停止中 ${expected}`);
  });

  it.each([
    [{ id: "bandFoldedThird", opened: "history", folded: "mixer" }, "取り消しの履歴をひらいたので、いちばん前にひらいたミキサーをたたみました。下の帯は、2 つまでひらけます。"],
    [{ id: "bandFoldedScale", scale: 150, band: "mixer" }, "拡大率を 150 % にしたので、ミキサーをたたみました。帯札を押すと、またひらきます。"],
    [{ id: "valueRejected", field: "velocity", kept: "77" }, "ベロシティは数字で打ちます。もとの 77 のままです。"],
    [{ id: "valueRejected", field: "meterDen", kept: "4" }, "拍子の下は数字で打ちます。もとの 4 のままです。"],
    [{ id: "valueRejected", field: "tempo", kept: "96.00" }, "テンポは数字で打ちます。もとの 96.00 のままです。"],
    [{ id: "outputMissing", output: "ヘッドホン" }, "ヘッドホンが見つからなくなりました。再生を止めています。 音の設定をひらく"],
  ] as [StatusMessage, string][])("新しい知らせ %o", (m, expected) => {
    show(StatusLine, msg(m));
    expect(left()).toContain(expected);
  });

  it.each([
    [[4], "ループ 5–9 ・ 選択なし ・ 4 パッド はミュート"],
    [[4, 5], "ループ 5–9 ・ 選択なし ・ 4 パッド ・ 5 ベース はミュート"],
  ])("ミュートしたトラックは、止まっているときの左の終わりに（モック Main）: %o", (muted, expected) => {
    show(
      StatusLine,
      msg({ id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: true, notes: null }, (s) => {
        s.song.tracks = [1, 2, 3, 4, 5].map((n) => ({
          ...s.song.tracks[0]!,
          number: n,
          name: ["ピアノ", "ドラム", "ギター", "パッド", "ベース"][n - 1]!,
          muted: muted.includes(n),
        }));
      }),
    );
    expect(left()).toContain(`■ 停止中 ${expected}`);
  });

  it("a muted track is shown even with nothing else to say. Not shown while moving or during other notices", () => {
    const mutePiano = (s: AppState) => (s.song.tracks[0]!.muted = true);
    show(StatusLine, msg({ id: "summary", loopBars: null, nothingSelected: false, notes: null }, mutePiano));
    expect(left()).toContain("■ 停止中 1 ピアノ はミュート");
    unmount(mounted!);
    mounted = null;
    show(StatusLine, msg({ id: "noArmedTrack" }, mutePiano));
    expect(left()).not.toContain("ミュート");
    unmount(mounted!);
    mounted = null;
    show(
      StatusLine,
      msg({ id: "summary", loopBars: null, nothingSelected: true, notes: null }, (s) => {
        mutePiano(s);
        s.transport.state = "playing";
      }),
    );
    expect(left()).not.toContain("ミュート");
  });

  it("with nothing in summary, only the tag", () => {
    show(StatusLine, msg({ id: "summary", loopBars: null, nothingSelected: false, notes: null }));
    expect(text(".status .message")).toBeNull();
  });

  it("during count-in: the spec text with the tag. The beats left come from the meter values", () => {
    show(StatusLine, msg({ id: "countingIn", beats: 4 }, (s) => (s.transport.state = "countIn")));
    expect(left()).toContain("● カウントイン中 あと 4 拍で録音が始まります。止めるには Space");
    app.setTelemetry(app.telemetryWith({ position: reading({ state: "countIn", countInBeatsLeft: 2 }) }));
    flushSync();
    expect(left()).toContain("● カウントイン中 あと 2 拍で録音が始まります。止めるには Space");
  });
});

describe("right of the input meter (play target)", () => {
  it("when stopped, 「1 ピアノ ・ グランドピアノ」", () => {
    show(InputMeter, ready());
    expect(text('[data-test="meter-target"]')).toBe("1 ピアノ ・ グランドピアノ");
  });

  it("during count-in and recording, 「1 ピアノ ● テイク 1」. From ● on is rec", () => {
    show(
      InputMeter,
      ready((s) => {
        s.transport.state = "countIn";
        s.transport.recording = { id: 1, track: 1, from: 0, take: 1, clip: null, lap: 0, overdub: false };
      }),
    );
    expect(text('[data-test="meter-target"]')).toBe("1 ピアノ ● テイク 1");
    expect(q('[data-test="meter-target"] .rec')!.textContent).toBe("● テイク 1");
  });

  it("the take number is recording.take as it is (the meter's pass is not added)", () => {
    show(
      InputMeter,
      ready((s) => {
        s.transport.state = "recording";
        s.transport.recording = { id: 1, track: 1, from: 0, take: 2, clip: 3, lap: 1, overdub: false };
      }),
    );
    app.setTelemetry(app.telemetryWith({ position: reading({ state: "recording", lap: 1 }) }));
    flushSync();
    expect(text('[data-test="meter-target"]')).toBe("1 ピアノ ● テイク 2");
  });
});

describe("hint above the arrange", () => {
  it("sound came out: the second sentence and key only when the record button can be used (mock F-Connect3)", () => {
    show(SoundHint, ready((s) => (s.workspace.hint = "soundCameOut")));
    expect(text('[data-test="sound-hint"]')).toBe(
      "音が出ました。 赤い丸の録音ボタンを押すと、カウントインのあとに録りはじめます。 録音 R ・ 再生と停止 Space",
    );
    app.setState(
      ready((s) => {
        s.workspace.hint = "soundCameOut";
        s.transport.canRecord = false;
      }),
    );
    flushSync();
    expect(text('[data-test="sound-hint"]')).toBe("音が出ました。");
  });

  it("empty arrange: 3 sentences (mock F-Empty). The third only when 「＋ トラックを追加」 can be used", () => {
    show(SoundHint, ready((s) => (s.workspace.hint = "emptyArrange")));
    expect(text('[data-test="empty-hint"]')).toBe(
      "ここに、録った音がならびます。 鍵盤を弾いて音を確かめたら、赤い丸の録音ボタンへ。 トラックを増やすには左下の「＋ トラックを追加」",
    );
    app.setState(
      ready((s) => {
        s.workspace.hint = "emptyArrange";
        s.transport.state = "countIn";
      }),
    );
    flushSync();
    expect(text('[data-test="empty-hint"]')).toBe("ここに、録った音がならびます。 鍵盤を弾いて音を確かめたら、赤い丸の録音ボタンへ。");
    expect(q('[data-test="sound-hint"]')).toBeNull();
  });
});

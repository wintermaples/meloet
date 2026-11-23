// Round 12 R3: the window of the metronome ▾ (on and off, 「音量」, 「音」, 「鳴らすとき」, 「テンポを手拍子で」) and its numbers.
import { flushSync, mount, tick, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { TAP_KEEP, TapTempo, clickDbAt, clickDbText, clickFrac, parseClickDb } from "../src/components/metronome";
import Transport from "../src/components/Transport.svelte";
import { cancelDrag } from "../src/components/pianoroll/view.svelte";
import { closeMenu, menuVisible } from "../src/lib/menu";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

describe("the numbers of the window", () => {
  it("tap: nothing below two taps; the mean gap of the last 8, in whole BPM", () => {
    const tap = new TapTempo();
    expect(tap.tap(1000)).toBeNull();
    expect(tap.tap(1500)).toBe(120);
    expect(tap.tap(2125)).toBe(Math.round(60_000 / 562.5));
    const steady = new TapTempo();
    // Slow at first, then 100 BPM: only the last 8 count.
    let at = 0;
    for (const gap of [900, 900, 900]) steady.tap((at += gap));
    for (let i = 0; i < TAP_KEEP; i++) steady.tap((at += 600));
    expect(steady.bpm()).toBe(100);
  });

  it("tap: a gap of 2 s starts again; the tempo stays in 30 to 300", () => {
    const tap = new TapTempo();
    tap.tap(0);
    expect(tap.tap(500)).toBe(120);
    expect(tap.tap(2500)).toBeNull();
    expect(tap.tap(3499)).toBe(Math.round(60_000 / 999));
    const slow = new TapTempo();
    slow.tap(0);
    expect(slow.tap(1999)).toBe(30);
    const fast = new TapTempo();
    fast.tap(0);
    expect(fast.tap(100)).toBe(300);
    fast.clear();
    expect(fast.bpm()).toBeNull();
  });

  it("the bar goes from −24 dB (left) to +6 dB (right) in whole dB; the text is 「−12 dB」", () => {
    expect([clickDbAt(0), clickDbAt(1), clickDbAt(0.4), clickDbAt(-1), clickDbAt(2)]).toEqual([-24, 6, -12, -24, 6]);
    expect([clickFrac(-24), clickFrac(6), clickFrac(0)]).toEqual([0, 1, 0.8]);
    expect([clickDbText(-12), clickDbText(0), clickDbText(-0.2), clickDbText(3)]).toEqual(["−12 dB", "0 dB", "0 dB", "3 dB"]);
  });

  it("typed volume: 「-6」「−6 dB」「+3」 in whole dB, put into the range; what cannot be read is undefined", () => {
    expect(["-6", "−6 dB", "+3", "2.6", "-40", "12", "abc", ""].map(parseClickDb)).toEqual([-6, -6, 3, 3, -24, 6, undefined, undefined]);
  });
});

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
let frames: FrameRequestCallback[] = [];

beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
  frames = [];
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => frames.push(cb));
  vi.stubGlobal("cancelAnimationFrame", () => {});
});
afterEach(() => {
  if (menuVisible()) closeMenu();
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  document.body.innerHTML = "";
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

const q = <T extends HTMLElement = HTMLElement>(sel: string) => document.querySelector<T>(sel);
const qa = (sel: string) => [...document.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

/** The scene of the spec's mock: on, −12 dB, 「ピッ」, 「録音のときだけ」. */
function scene(edit: (s: AppState) => void = () => {}): AppState {
  const s = structuredClone(fakeScene("metronome")!.state);
  edit(s);
  return s;
}

async function open(state: AppState): Promise<HTMLElement> {
  app.setState(state);
  mounted = mount(Transport, { target });
  flushSync();
  q<HTMLButtonElement>('[data-test="transport-metronome-stub"]')!.click();
  flushSync();
  await tick();
  app.sent.length = 0;
  return q('[data-test="metronome-window"]')!;
}

function nextFrame(): void {
  const run = frames;
  frames = [];
  for (const cb of run) cb(0);
  flushSync();
}
function pointer(el: Element, type: string, clientX: number): void {
  el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX, clientY: 5, button: 0 }));
  flushSync();
}
/** The bar is 0 to 300 px: 10 px a dB. */
function sizeBar(el: HTMLElement): void {
  el.getBoundingClientRect = () => ({ left: 0, right: 300, width: 300, top: 0, bottom: 12, height: 12, x: 0, y: 0, toJSON: () => ({}) });
}
const volume = () => q('[data-test="metronome-volume"]')!;
const volumeText = () => text(q('[data-test="metronome-volume-value"]'));
const click = (volumeDb: number | null, sound: string | null, when: string | null, commit: boolean) => ({ type: "setClick", volumeDb, sound, when, commit });

describe("the metronome window (R3)", () => {
  it("rows as in the mock: on, the volume, the sound, when it clicks, and tapping (the count-in button is there, so no count-in row)", async () => {
    const win = await open(scene());
    const rows = [...win.querySelectorAll(".row")].map((r) => [...r.children].map(text).filter(Boolean).join(" "));
    expect(rows).toEqual([
      "メトロノーム オン",
      "音量 −12 dB",
      "音 ピッ カチ カウベル",
      "鳴らすとき いつも 録音のときだけ",
      "テンポを手拍子で タップ — この速さにする",
    ]);
    const pressed = (sel: string) => qa(sel).map((b) => b.getAttribute("aria-pressed"));
    expect(pressed('[data-test="metronome-on"]')).toEqual(["true"]);
    expect(pressed('[data-test="metronome-sound"]')).toEqual(["true", "false", "false"]);
    expect(pressed('[data-test="metronome-when"]')).toEqual(["false", "true"]);
    expect(q('[data-test="metronome-use-tempo"]')!.getAttribute("aria-disabled")).toBe("true");
    // Off: 「オフ」 without the on color.
    app.setState(scene((s) => (s.transport.metronome = false)));
    flushSync();
    const on = q('[data-test="metronome-on"]')!;
    expect([on.textContent, on.classList.contains("on")]).toEqual(["オフ", false]);
  });

  it("on and off, the sound and when it clicks: one intent each, and the window stays open", async () => {
    await open(scene());
    q<HTMLButtonElement>('[data-test="metronome-on"]')!.click();
    q<HTMLButtonElement>('[data-sound="cowbell"]')!.click();
    q<HTMLButtonElement>('[data-when="always"]')!.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "toggleMetronome" }, click(null, "cowbell", null, true), click(null, null, "always", true)]);
    expect(q('[data-test="metronome-window"]')).not.toBeNull();
  });

  it("a drag of the volume: only the sound at most once a frame, one commit: true on release; the setting is a gray.7 frame meanwhile", async () => {
    await open(scene());
    sizeBar(volume());
    pointer(volume(), "pointerdown", 100);
    pointer(volume(), "pointermove", 110);
    expect(volumeText()).toBe("−13 dB");
    expect(q('[data-test="metronome-volume-from"]')).not.toBeNull();
    nextFrame();
    pointer(volume(), "pointermove", 150);
    pointer(volume(), "pointermove", 160);
    nextFrame();
    nextFrame();
    pointer(volume(), "pointerup", 170);
    expect(app.sent).toEqual([click(-13, null, null, false), click(-8, null, null, false), click(-7, null, null, true)]);
    expect(q('[data-test="metronome-volume-from"]')).toBeNull();
    // Kept until Rust's answer comes.
    expect(volumeText()).toBe("−7 dB");
    app.setState(scene((s) => (s.transport.clickVolumeDb = -7)));
    flushSync();
    expect(volumeText()).toBe("−7 dB");
  });

  it("Esc during the drag puts the sound back to the setting and keeps nothing", async () => {
    await open(scene());
    sizeBar(volume());
    pointer(volume(), "pointerdown", 260);
    nextFrame();
    expect(cancelDrag()).toBe(true);
    flushSync();
    pointer(volume(), "pointerup", 260);
    expect(app.sent).toEqual([click(2, null, null, false), click(-12, null, null, false)]);
    expect(volumeText()).toBe("−12 dB");
    expect(q('[data-test="metronome-window"]')).not.toBeNull();
    // Pressed and let go in the same place: nothing.
    app.sent.length = 0;
    pointer(volume(), "pointerdown", 120);
    pointer(volume(), "pointerup", 120);
    expect(app.sent).toEqual([]);
  });

  it("← → by 1 dB: the sound each time, kept a second after the key is let go", async () => {
    vi.useFakeTimers();
    await open(scene());
    const key = (type: string, k: string) => volume().dispatchEvent(new KeyboardEvent(type, { key: k, bubbles: true, cancelable: true }));
    key("keydown", "ArrowRight");
    key("keydown", "ArrowRight");
    key("keyup", "ArrowRight");
    flushSync();
    expect(volumeText()).toBe("−10 dB");
    expect(app.sent).toEqual([click(-11, null, null, false), click(-10, null, null, false)]);
    vi.advanceTimersByTime(1000);
    expect(app.sent.at(-1)).toEqual(click(-10, null, null, true));
  });

  it("a double click on the number types it; what cannot be read keeps the box", async () => {
    await open(scene());
    q('[data-test="metronome-volume-value"]')!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    flushSync();
    const input = q<HTMLInputElement>('[data-test="param-input"]')!;
    const enter = () => input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }));
    input.value = "abc";
    enter();
    flushSync();
    expect(q('[data-test="param-input"]')).not.toBeNull();
    input.value = "−30";
    enter();
    flushSync();
    expect(app.sent).toEqual([click(-24, null, null, true)]);
    expect(volumeText()).toBe("−24 dB");
  });

  it("「タップ」 twice or more gives a tempo; 「この速さにする」 sends one setTempo", async () => {
    const now = vi.spyOn(performance, "now");
    await open(scene());
    const tap = q('[data-test="metronome-tap"]')!;
    const shown = () => text(q('[data-test="metronome-tap-tempo"]'));
    for (const at of [1000, 1600, 2200, 2800]) {
      now.mockReturnValue(at);
      tap.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, button: 0 }));
    }
    flushSync();
    expect(shown()).toBe("100.00");
    expect(app.sent).toEqual([]);
    // Space taps too, and does not play.
    now.mockReturnValue(3400);
    const space = new KeyboardEvent("keydown", { key: " ", bubbles: true, cancelable: true });
    const toDocument = vi.fn();
    document.addEventListener("keydown", toDocument);
    tap.dispatchEvent(space);
    document.removeEventListener("keydown", toDocument);
    expect(toDocument).not.toHaveBeenCalled();
    flushSync();
    expect(shown()).toBe("100.00");
    const use = q<HTMLButtonElement>('[data-test="metronome-use-tempo"]')!;
    expect(use.getAttribute("aria-disabled")).toBe("false");
    use.click();
    use.click();
    flushSync();
    expect(app.sent).toEqual([{ type: "setTempo", bpm: 100 }]);
    expect(shown()).toBe("—");
  });

  it("「タップ」 and 「この速さにする」 cannot be used while counting in or recording", async () => {
    const now = vi.spyOn(performance, "now");
    await open(scene());
    const tap = q('[data-test="metronome-tap"]')!;
    now.mockReturnValue(0);
    tap.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, button: 0 }));
    now.mockReturnValue(500);
    tap.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, button: 0 }));
    flushSync();
    expect(text(q('[data-test="metronome-tap-tempo"]'))).toBe("120.00");
    for (const state of ["countIn", "recording"] as const) {
      app.setState(
        scene((s) => {
          s.transport.state = state;
          s.transport.recording = { id: 1, track: s.song.tracks[0]!.id, from: 0, take: 1, clip: null, lap: 0, overdub: false };
        }),
      );
      flushSync();
      expect(text(q('[data-test="metronome-tap-tempo"]'))).toBe("—");
      expect(tap.getAttribute("aria-disabled")).toBe("true");
      const use = q<HTMLButtonElement>('[data-test="metronome-use-tempo"]')!;
      expect(use.getAttribute("aria-disabled")).toBe("true");
      now.mockReturnValue(1000);
      tap.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, button: 0 }));
      now.mockReturnValue(1500);
      tap.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, button: 0 }));
      use.click();
      flushSync();
      expect(text(q('[data-test="metronome-tap-tempo"]'))).toBe("—");
    }
    expect(app.sent).toEqual([]);
  });
});

describe("the fake keeps what is committed", () => {
  it("setClick with commit: true changes the transport; commit: false changes nothing that shows", async () => {
    const t = createFakeTransport(fakeScene("metronome")!.state);
    await t.subscribe(
      () => {},
      () => {},
    );
    const now = async () => ((await t.snapshot()).state as AppState).transport;
    await t.send({ type: "setClick", volumeDb: -3, sound: null, when: null, commit: false });
    expect((await now()).clickVolumeDb).toBe(-12);
    await t.send({ type: "setClick", volumeDb: 40, sound: "tick", when: "always", commit: true });
    const after = await now();
    expect([after.clickVolumeDb, after.clickSound, after.clickWhen]).toEqual([6, "tick", "always"]);
  });
});

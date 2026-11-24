// E7 「音の動きを見せる」: the bars of the master's sound left of 「マスター」 in the top band, how they are painted, when they
// fit, the row of the 「表示」 menu, and the fake's answer and scenes.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { createFakeTransport, encodeTelemetry, fakeScene, fakeState } from "../src/bridge/fake";
import { fakeSpectrum } from "../src/bridge/fakeWave3";
import { decodeTelemetry } from "../src/bridge/telemetry";
import { transportWithKeep, transportWithSpectrum } from "../src/components/fit";
import { metrics } from "../src/components/metrics.svelte";
import { topMenus, type ListRow } from "../src/components/menu/menus";
import Transport from "../src/components/Transport.svelte";
import { Batch } from "../src/gl/batch";
import { rgb } from "../src/gl/color";
import { paintSpectrum } from "../src/gl/paint";
import { t } from "../src/text";
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
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  vi.restoreAllMocks();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState): void {
  if (mounted) unmount(mounted);
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}

describe("the wording of E7", () => {
  it("is the spec's", () => {
    expect(t("menu.spectrum")).toBe("音の動きを見せる");
  });
});

describe("the bars as painted", () => {
  const ofColor = (b: Batch, name: Parameters<typeof rgb>[0]) =>
    b
      .rects()
      .filter((r) => r.slice(4).every((v, i) => v === Math.fround(rgb(name)[i]!)))
      .map((r) => r.slice(0, 4));

  it("24 bars on the meter's ground, 5 of each track color from the left (the last 4 of track.5), one device pixel apart, 0..255 the box's height", () => {
    const b = new Batch();
    b.clip(0, 0, 1000, 1000);
    const bands = Uint8Array.from({ length: 24 }, (_, i) => (i === 3 ? 0 : 255 - i * 10));
    paintSpectrum(b, { x0: 10, y0: 20, x1: 106, y1: 48 }, bands);
    expect(ofColor(b, "color.meter.ground")).toEqual([[10, 20, 106, 48]]);
    const tops = (i: number) => 48 - Math.round((28 * bands[i]!) / 255);
    expect(ofColor(b, "color.track.1")).toEqual(
      [0, 1, 2, 4].map((i) => [10 + 4 * i, tops(i), 13 + 4 * i, 48]),
    );
    expect(ofColor(b, "color.track.2")).toHaveLength(5);
    expect(ofColor(b, "color.track.4")).toHaveLength(5);
    expect(ofColor(b, "color.track.5")).toEqual([20, 21, 22, 23].map((i) => [10 + 4 * i, tops(i), 13 + 4 * i, 48]));
    // A band at 0 draws nothing over the ground.
    expect(b.rects()).toHaveLength(1 + 23);
  });

  it("in part.spectrum.width (72 at 100 %) every bar is 2 px with one device pixel of ground after it", () => {
    expect(metrics.px("part.spectrum.width")).toBe(72);
    const b = new Batch();
    b.clip(0, 0, 1000, 1000);
    paintSpectrum(b, { x0: 0, y0: 0, x1: 72, y1: 28 }, new Uint8Array(24).fill(255));
    const bars = b.rects().slice(1);
    expect(bars).toHaveLength(24);
    bars.forEach((r, i) => expect(r.slice(0, 4)).toEqual([3 * i, 0, 3 * i + 2, 28]));
  });

  it("silence is the ground alone", () => {
    const b = new Batch();
    b.clip(0, 0, 1000, 1000);
    paintSpectrum(b, { x0: 0, y0: 0, x1: 144, y1: 42 }, new Uint8Array(24));
    expect(b.rects()).toEqual([[0, 0, 144, 42, ...rgb("color.meter.ground").map(Math.fround)]]);
  });
});

describe("room in the top band", () => {
  const w = { base: 800, keys: 100, songName: 144, load: 60, countIn: 227, positionLabels: 20 };
  const stubs = { songName: 40, countIn: 20 };
  const all = w.base + w.keys + w.songName + w.load + w.countIn + w.positionLabels + 60 + 90;

  it("the bars come only into room nothing else needs; nothing gives way to them; off, never", () => {
    expect(transportWithSpectrum(all + 104, w, stubs, 60, 90, 104).spectrum).toBe(true);
    expect(transportWithSpectrum(all + 103, w, stubs, 60, 90, 104).spectrum).toBe(false);
    expect(transportWithSpectrum(all + 500, w, stubs, 60, 90, null).spectrum).toBe(false);
    for (let width = 600; width <= all + 300; width += 7) {
      const parts = transportWithSpectrum(width, w, stubs, 60, 90, 104);
      expect({ ...parts, spectrum: undefined }).toEqual({ ...transportWithKeep(width, w, stubs, 60, 90), spectrum: undefined });
    }
  });
});

describe("the bars in the top band", () => {
  /** jsdom has no width: every part is 10 wide; the band `row` wide. The master's box is wider by the bars while they are in it. */
  function widths(row: number, part: (name: string | undefined) => number = () => 10): void {
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(row);
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function (this: HTMLElement) {
      if (this.classList.contains("master") && this.querySelector('[data-test="transport-spectrum"]'))
        return part(undefined) + part("spectrum") + metrics.px("space.1");
      return part(this.dataset.part);
    });
  }
  const bars = () => target.querySelector<HTMLElement>('[data-test="transport-spectrum"]');
  const shownParts = () =>
    ['[data-test="transport-song"]', '[data-test="transport-count-in"]', '[data-test="transport-keep-played"]', ".m-lbl.key, .key"].map(
      (sel) => target.querySelector(sel) !== null,
    );

  it("left of 「マスター」 inside the master's box, as a slot of the master surface", () => {
    widths(1920);
    show(Transport, fakeState());
    const el = bars()!;
    expect(el.parentElement!.classList.contains("master")).toBe(true);
    expect(el.nextElementSibling!.classList.contains("master-col")).toBe(true);
    expect(el.nextElementSibling!.textContent).toContain(t("transport.master"));
    expect(el.dataset.glSlot).toBe("spectrum");
    expect(el.closest("[data-gl-region]")?.getAttribute("data-gl-region")).toBe("masterMeter");
  });

  it("the position keeps the width of a 3-digit bar (on screen and in the ruler that measures the row)", () => {
    widths(1920);
    show(Transport, fakeState());
    const shown = target.querySelector<HTMLElement>('[data-test="transport-position"]')!;
    expect(shown.textContent).toBe("1. 1. 0000");
    expect(shown.style.minWidth).toBe("12ch");
    expect(target.querySelector<HTMLElement>('[data-part="readout-0"]')!.style.minWidth).toBe("12ch");
  });

  it("off in 「表示」: no bars", () => {
    widths(1920);
    const s = fakeState();
    s.workspace.spectrum = false;
    show(Transport, s);
    expect(bars()).toBeNull();
    // On again: they come back.
    const on = fakeState();
    app.setState(on);
    flushSync();
    expect(bars()).not.toBeNull();
  });

  it("not enough room: no bars, and the rest is as with them off", () => {
    for (const room of [0, 200, 300, 400, 600]) {
      vi.restoreAllMocks();
      widths(room, (part) => (part === "spectrum" ? 72 : 40));
      const off = fakeState();
      off.workspace.spectrum = false;
      show(Transport, off);
      const without = shownParts();
      show(Transport, fakeState());
      expect(shownParts(), String(room)).toEqual(without);
      if (room === 0) expect(bars()).toBeNull();
    }
  });
});

describe("「表示」 menu", () => {
  const rowOf = (s: AppState, sent: Command[]) =>
    topMenus(s, { state: () => s, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0 }, "other")[2]!.entries.find(
      (e): e is ListRow => e.kind === "row" && e.label === "音の動きを見せる",
    )!;

  it("the last row, checked while on; it sends the other way", () => {
    const sent: Command[] = [];
    const s = fakeState();
    const row = rowOf(s, sent);
    expect([row.checked, row.usable, row.key]).toEqual([true, true, undefined]);
    row.run!();
    s.workspace.spectrum = false;
    const off = rowOf(s, sent);
    expect(off.checked).toBe(false);
    off.run!();
    expect(sent).toEqual([
      { type: "setSpectrum", on: false },
      { type: "setSpectrum", on: true },
    ]);
  });
});

describe("the fake", () => {
  async function opened() {
    const transport = createFakeTransport(fakeState());
    await transport.subscribe(
      () => {},
      () => {},
    );
    return { now: async () => (await transport.snapshot()).state as AppState, send: (c: Command) => transport.send(c) };
  }

  it("setSpectrum is the workspace's switch, not a step", async () => {
    const { now, send } = await opened();
    const before = await now();
    await send({ type: "setSpectrum", on: false });
    const after = await now();
    expect(after.workspace.spectrum).toBe(false);
    expect([after.song.undoCount, after.song.redoCount]).toEqual([before.song.undoCount, before.song.redoCount]);
    await send({ type: "setSpectrum", on: true });
    expect((await now()).workspace.spectrum).toBe(true);
  });

  it("a scene whose master sounds sends the bands of a song; a quiet one none", () => {
    const meters = fakeScene("spectrum")!.meters;
    expect(meters.spectrum).toHaveLength(24);
    const read = decodeTelemetry(encodeTelemetry(meters, 1, 0, 0));
    expect(Array.from(read.spectrum)).toEqual(meters.spectrum);
    expect(Math.max(...meters.spectrum!)).toBeLessThan(255);
    expect(Math.min(...meters.spectrum!)).toBeGreaterThan(0);
    expect(fakeSpectrum(undefined)).toBeUndefined();
    expect(fakeSpectrum([0, 0])).toBeUndefined();
    expect(fakeScene("empty")!.meters.spectrum).toBeUndefined();
    expect(fakeScene("spectrumOff")!.state.workspace.spectrum).toBe(false);
  });
});

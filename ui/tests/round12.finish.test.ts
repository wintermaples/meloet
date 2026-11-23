// M3 「仕上げのメーター」: its wording, its numbers and how often they change, its two bars as painted, the box right of the master
// strip, 「モノで聞く」, and the fake's answer.
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, StatusMessage } from "../src/bridge";
import { createFakeTransport, fakeScene, fakeState } from "../src/bridge/fake";
import { FINISH_TEXT_MS, calmly, finishNumber, finishTarget, finishTexts } from "../src/components/mixer/finish";
import MixerStrips from "../src/components/mixer/MixerStrips.svelte";
import { Batch } from "../src/gl/batch";
import { rgb } from "../src/gl/color";
import { dbToLength } from "../src/gl/level";
import { paintLoudness, paintSpectrum } from "../src/gl/paint";
import { regionOf } from "../src/gl/slot";
import { t } from "../src/text";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  vi.useRealTimers();
});
function show(state: AppState): void {
  app.setState(state);
  mounted = mount(MixerStrips, { target });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

describe("the wording of M3", () => {
  it("is the spec's", () => {
    expect([t("mixer.finish"), t("mixer.finishNow"), t("mixer.finishWhole"), t("mixer.finishTruePeak")]).toEqual([
      "仕上げのメーター",
      "いまの大きさ",
      "曲ぜんたい",
      "トゥルーピーク",
    ]);
    expect([t("mixer.monoListen"), t("msg.monoListening"), t("mixer.finishBands")]).toEqual(["モノで聞く", "モノで聞いています", "低い音 ← → 高い音"]);
    expect(finishTarget()).toBe("公開の目安は −14 LUFS、−1 dBTP");
  });
});

describe("the numbers", () => {
  it("one decimal with a minus sign, a plus above 0, 「−∞」 for silence", () => {
    expect([finishNumber(-13.24), finishNumber(-1.36), finishNumber(0.04), finishNumber(0.3), finishNumber(null)]).toEqual([
      "−13.2",
      "−1.4",
      "0.0",
      "+0.3",
      "−∞",
    ]);
    expect(finishTexts({ lufsShort: -13.2, lufsIntegrated: -15.8, truePeakDb: -1.4 })).toEqual({
      now: "−13.2 LUFS",
      whole: "−15.8 LUFS",
      peak: "−1.4 dBTP",
    });
    expect(finishTexts(null)).toEqual({ now: "−∞ LUFS", whole: "−∞ LUFS", peak: "−∞ dBTP" });
  });

  it("change at most 4 times a second: at once, then once at the end of the quarter however many readings came", () => {
    vi.useFakeTimers();
    vi.setSystemTime(1000);
    const runs: number[] = [];
    const calm = calmly(() => runs.push(Date.now()), FINISH_TEXT_MS, () => Date.now());
    calm.poke();
    expect(runs).toEqual([1000]);
    for (const step of [16, 84, 100]) {
      vi.advanceTimersByTime(step);
      calm.poke();
    }
    expect(runs).toEqual([1000]);
    vi.advanceTimersByTime(50);
    expect(runs).toEqual([1000, 1250]);
    vi.advanceTimersByTime(450);
    calm.poke();
    expect(runs).toEqual([1000, 1250, 1700]);
    vi.advanceTimersByTime(100);
    calm.poke();
    calm.stop();
    vi.advanceTimersByTime(1000);
    expect(runs).toEqual([1000, 1250, 1700]);
  });
});

describe("the bars as painted", () => {
  const rects = (b: Batch, name: Parameters<typeof rgb>[0]) =>
    b
      .rects()
      .filter((r) => r.slice(4).every((v, i) => v === Math.fround(rgb(name)[i]!)))
      .map((r) => r.slice(0, 4));

  it("the loudness grows to the right on the meters' scale, meter.ok and then meter.hot where a meter turns hot", () => {
    const b = new Batch();
    b.clip(0, 0, 1000, 1000);
    paintLoudness(b, { x0: 10, y0: 20, x1: 210, y1: 24 }, -13.2);
    expect(rects(b, "color.meter.ground")).toEqual([[10, 20, 210, 24]]);
    expect(rects(b, "color.meter.ok")).toEqual([[10, 20, 10 + Math.round(200 * dbToLength(-13.2)), 24]]);
    expect(rects(b, "color.meter.hot")).toEqual([]);

    const loud = new Batch();
    loud.clip(0, 0, 1000, 1000);
    paintLoudness(loud, { x0: 0, y0: 0, x1: 200, y1: 4 }, -1);
    expect(rects(loud, "color.meter.ok")).toEqual([[0, 0, 140, 4]]);
    expect(rects(loud, "color.meter.hot")).toEqual([[140, 0, Math.round(200 * dbToLength(-1)), 4]]);
  });

  it("silence is the ground alone", () => {
    const b = new Batch();
    b.clip(0, 0, 1000, 1000);
    paintLoudness(b, { x0: 0, y0: 0, x1: 200, y1: 4 }, null);
    expect(b.rects()).toEqual([[0, 0, 200, 4, ...rgb("color.meter.ground").map(Math.fround)]]);
  });

  it("the 24 bands all in one color when it is given", () => {
    const b = new Batch();
    b.clip(0, 0, 1000, 1000);
    const bands = Uint8Array.from({ length: 24 }, (_, i) => 255 - i * 10);
    paintSpectrum(b, { x0: 0, y0: 0, x1: 192, y1: 56 }, bands, "color.meter.ok");
    expect(rects(b, "color.meter.ok")).toHaveLength(24);
    expect(rects(b, "color.track.1")).toEqual([]);
  });
});

describe("the box right of the master strip", () => {
  it("has the numbers of the telemetry, the two bars as slots of the mixer's region, and the target line", () => {
    app.setTelemetry(app.telemetryWith({ lufsShort: -13.2, lufsIntegrated: -15.8, truePeakDb: -1.4 }));
    show(fakeScene("finishMeters")!.state);
    const box = q("[data-test=mixer-finish]")!;
    expect(box.previousElementSibling?.getAttribute("data-strip")).toBe("master");
    expect(text(box.querySelector(".head"))).toBe("仕上げのメーター モノで聞く");
    expect(text(q("[data-test=mixer-finish-now]"))).toBe("−13.2 LUFS");
    expect(text(q("[data-test=mixer-finish-whole]"))).toBe("−15.8 LUFS");
    expect(text(q("[data-test=mixer-finish-peak]"))).toBe("−1.4 dBTP");
    expect(text(q("[data-test=mixer-finish-target]"))).toBe("公開の目安は −14 LUFS、−1 dBTP");
    const kinds = [...box.querySelectorAll<HTMLElement>("[data-gl-slot]")].map((el) => [el.dataset.glSlot, regionOf(el)?.name]);
    expect(kinds).toEqual([
      ["loudness", "mixerMeters"],
      ["spectrum", "mixerMeters"],
    ]);
  });

  it("silence reads 「−∞」", () => {
    show(fakeScene("finishMetersSilent")!.state);
    expect(text(q("[data-test=mixer-finish-now]"))).toBe("−∞ LUFS");
    expect(text(q("[data-test=mixer-finish-peak]"))).toBe("−∞ dBTP");
  });

  it("「モノで聞く」 sends the switch and is lit while it is on", () => {
    show(fakeScene("finishMeters")!.state);
    const button = q("[data-test=mixer-mono]")!;
    expect(button.getAttribute("aria-pressed")).toBe("false");
    button.click();
    expect(app.sent).toEqual([{ type: "setMonoListen", on: true }]);

    unmount(mounted!);
    show(fakeScene("finishMetersMono")!.state);
    const lit = q("[data-test=mixer-mono]")!;
    expect(lit.getAttribute("aria-pressed")).toBe("true");
    expect(lit.classList.contains("on")).toBe(true);
    lit.click();
    expect(app.sent.at(-1)).toEqual({ type: "setMonoListen", on: false });
  });
});

describe("the fake", () => {
  it("turns 「モノで聞く」 on and off, saying it in the status line while it is on", async () => {
    const transport = createFakeTransport(fakeState());
    await transport.subscribe(
      () => {},
      () => {},
    );
    const now = async () => (await transport.snapshot()).state as AppState;
    const said = async (): Promise<StatusMessage | null> => (await now()).status.message;
    const before = await now();
    await transport.send({ type: "setMonoListen", on: true });
    expect((await now()).workspace.monoListen).toBe(true);
    expect(await said()).toEqual({ id: "monoListening" });
    await transport.send({ type: "setMonoListen", on: true });
    expect(await said()).toEqual({ id: "monoListening" });
    await transport.send({ type: "setMonoListen", on: false });
    const after = await now();
    expect(after.workspace.monoListen).toBe(false);
    expect(after.status.message?.id).toBe("summary");
    expect([after.song.undoCount, after.song.redoCount]).toEqual([before.song.undoCount, before.song.redoCount]);
  });

  it("has the scenes, with the numbers of the spec's picture", () => {
    for (const name of ["finishMeters", "finishMetersMono", "finishMetersSilent"]) expect(fakeScene(name)).not.toBeNull();
    expect(fakeScene("finishMetersMono")!.state.workspace.monoListen).toBe(true);
  });
});

// E2 (round 12): the arpeggio of the track the keyboards play. The input meter's 「アルペジオ」 and its ▾ window, the mixer strip's
// 「アルペジオ」 while on, the fit of the input meter, the wording, and the fake.
import { flushSync, mount, tick, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Arp, Command } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { FAKE_ARP_OFF } from "../src/bridge/fakeWave3";
import InputMeter from "../src/components/InputMeter.svelte";
import MixerStrips from "../src/components/mixer/MixerStrips.svelte";
import { meterFit, type MeterFitInput } from "../src/components/meterFit";
import { closeMenu, menuVisible } from "../src/lib/menu";
import { t } from "../src/text";
import { stepNames } from "../src/text/ja";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state;
const ON: Arp = { ...FAKE_ARP_OFF, on: true };

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
  if (menuVisible()) closeMenu();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState): void {
  if (mounted) unmount(mounted);
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = (sel: string) => document.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...document.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const arpOf = (s: AppState, id: number) => s.song.tracks.find((x) => x.id === id)!.arp;

describe("the wording of E2", () => {
  it("is the spec's", () => {
    expect(t("input.arp")).toBe("アルペジオ");
    expect([t("arp.up"), t("arp.down"), t("arp.upDown"), t("arp.random")]).toEqual(["上へ", "下へ", "上下", "ばらばら"]);
    expect([t("arp.quarter"), t("arp.eighth"), t("arp.sixteenth"), t("arp.thirtySecond"), t("arp.triplet")]).toEqual(["1/4", "1/8", "1/16", "1/32", "三連符"]);
    expect([t("arp.shape"), t("arp.rate"), t("arp.octaves"), t("arp.hold")]).toEqual(["形", "速さ", "オクターブ", "指を離しても続ける"]);
    expect(t("arp.hint")).toBe("和音を押さえるだけで、テンポに合ったフレーズになります");
    expect(stepNames.setArp.text).toBe("アルペジオを変える");
  });
});

describe("the input meter", () => {
  // jsdom lays nothing out: a wide row, so everything has its room.
  beforeEach(() => Object.defineProperty(HTMLElement.prototype, "clientWidth", { configurable: true, get: () => 4000 }));
  afterEach(() => delete (HTMLElement.prototype as { clientWidth?: number }).clientWidth);

  it("「アルペジオ」 is lit while on and turns the keys track's arpeggio off and on (one setArp with the whole setting)", () => {
    const s = scene("arpOn");
    const id = s.song.keysTarget!;
    show(InputMeter, s);
    const button = q('[data-test="arp"]')!;
    expect([text(button), button.classList.contains("on"), button.getAttribute("aria-pressed"), button.getAttribute("aria-disabled")]).toEqual([
      "アルペジオ",
      true,
      "true",
      null,
    ]);
    button.click();
    expect(app.sent).toEqual([{ type: "setArp", track: id, arp: { ...arpOf(s, id), on: false } }]);
    const off = scene("keepPlayedReady");
    show(InputMeter, off);
    expect(q('[data-test="arp"]')!.classList.contains("on")).toBe(false);
    q('[data-test="arp"]')!.click();
    expect(app.sent.at(-1)).toEqual({ type: "setArp", track: off.song.keysTarget!, arp: ON });
  });

  it("is disabled with no keys track, for a vocal track, and while counting in or recording", () => {
    const none = scene("keepPlayedReady");
    none.song.keysTarget = null;
    show(InputMeter, none);
    const disabled = () => [q('[data-test="arp"]')!.getAttribute("aria-disabled"), q('[data-test="arp-open"]')!.getAttribute("aria-disabled")];
    expect(disabled()).toEqual(["true", "true"]);
    q('[data-test="arp"]')!.click();
    q('[data-test="arp-open"]')!.click();
    flushSync();
    expect([app.sent, q('[data-test="arp-window"]')]).toEqual([[], null]);

    const vocal = scene("arpOn");
    const track = vocal.song.tracks[1]!;
    track.kind = { kind: "vocal", input: "mono1", inputReady: true, armed: false, monitor: false };
    vocal.song.keysTarget = track.id;
    show(InputMeter, vocal);
    expect(disabled()).toEqual(["true", "true"]);

    const busy = scene("arpOn");
    busy.transport.state = "recording";
    show(InputMeter, busy);
    // Still lit (it is on), the ▾ still opens, but nothing is sent.
    expect([...disabled(), q('[data-test="arp"]')!.classList.contains("on")]).toEqual(["true", null, true]);
    q('[data-test="arp"]')!.click();
    expect(app.sent).toEqual([]);
  });

  it("the ▾ window: rows as in the mock, each press one setArp with only that field changed", async () => {
    const s = scene("arpSettings");
    const id = s.song.keysTarget!;
    const arp = arpOf(s, id);
    show(InputMeter, s);
    q('[data-test="arp-open"]')!.click();
    flushSync();
    await tick();
    const win = q('[data-test="arp-window"]')!;
    expect(win.parentElement!.parentElement).toBe(document.body);
    expect(q('[data-test="arp-open"]')!.getAttribute("aria-expanded")).toBe("true");
    const rows = [...win.querySelectorAll(".row")].map((r) => [...r.children].map(text).filter(Boolean).join(" "));
    expect(rows).toEqual([
      "アルペジオ オン",
      "形 上へ 下へ 上下 ばらばら",
      "速さ 1/4 1/8 1/16 1/32 三連符",
      "オクターブ 1 2 3",
      "指を離しても続ける オン",
      "和音を押さえるだけで、テンポに合ったフレーズになります",
    ]);
    const pressed = (sel: string) => qa(sel).map((b) => b.getAttribute("aria-pressed"));
    expect(pressed('[data-test="arp-shape"]')).toEqual(["false", "false", "true", "false"]);
    expect(pressed('[data-test="arp-rate"]')).toEqual(["false", "true", "false", "false"]);
    expect(pressed('[data-test="arp-octaves"]')).toEqual(["false", "true", "false"]);
    expect([...pressed('[data-test="arp-triplet"]'), ...pressed('[data-test="arp-hold"]'), ...pressed('[data-test="arp-window-on"]')]).toEqual([
      "true",
      "true",
      "true",
    ]);
    expect(document.activeElement).toBe(q('[data-test="arp-window-on"]'));

    const presses: [string, Partial<Arp>][] = [
      ['[data-test="arp-window-on"]', { on: false }],
      ['[data-test="arp-shape"][data-value="random"]', { shape: "random" }],
      ['[data-test="arp-rate"][data-value="thirtySecond"]', { rate: "thirtySecond" }],
      ['[data-test="arp-triplet"]', { triplet: false }],
      ['[data-test="arp-octaves"][data-value="3"]', { octaves: 3 }],
      ['[data-test="arp-hold"]', { hold: false }],
    ];
    for (const [sel, change] of presses) {
      app.sent.length = 0;
      q(sel)!.click();
      expect(app.sent).toEqual([{ type: "setArp", track: id, arp: { ...arp, ...change } }]);
    }
  });

  it("the window closes with Esc (the menus' closer) and a press outside, and its rows send nothing while recording", async () => {
    const s = scene("arpSettings");
    show(InputMeter, s);
    q('[data-test="arp-open"]')!.click();
    flushSync();
    expect(menuVisible()).toBe(true);
    closeMenu();
    flushSync();
    expect(q('[data-test="arp-window"]')).toBeNull();
    q('[data-test="arp-open"]')!.click();
    flushSync();
    document.body.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
    flushSync();
    expect(q('[data-test="arp-window"]')).toBeNull();

    const busy = scene("arpSettings");
    busy.transport.state = "countIn";
    show(InputMeter, busy);
    q('[data-test="arp-open"]')!.click();
    flushSync();
    await tick();
    for (const b of qa('[data-test="arp-window"] button')) {
      expect(b.getAttribute("aria-disabled")).toBe("true");
      b.click();
    }
    expect(app.sent).toEqual([]);
  });

  it("「アルペジオ」 takes only room left; it comes before 「キーで弾く」 and its octave, which keep their order", () => {
    // Without them: 680 + 108 + 90 + 110 = 988; the arpeggio 100, the switch 70, the octave 50.
    const full: MeterFitInput = { meter: "full", scale: 100, nameChars: 5, available: 1208, rest: 900, name: 60, pitchBend: 90, modulation: 110, typed: [70, 50], arp: 100 };
    expect(meterFit(full)).toMatchObject({ arp: true, typed: 2, modulation: true });
    expect(meterFit({ ...full, available: 1158 })).toMatchObject({ arp: true, typed: 1, modulation: true });
    expect(meterFit({ ...full, available: 1088 })).toMatchObject({ arp: true, typed: 0, modulation: true });
    // Less than the arpeggio: the switch takes the room as it did before.
    expect(meterFit({ ...full, available: 1087 })).toMatchObject({ arp: false, typed: 1, modulation: true });
    expect(meterFit({ ...full, available: 988 })).toMatchObject({ arp: false, typed: 0, modulation: true, pitchBend: true });
    // Modulation hidden as it was without them (988 > 987): the room left (878 + 109) is the arpeggio's; with less, the switch's.
    expect(meterFit({ ...full, available: 987 })).toMatchObject({ arp: true, typed: 0, modulation: false, pitchBend: true });
    expect(meterFit({ ...full, available: 977 })).toMatchObject({ arp: false, typed: 1, modulation: false, pitchBend: true });
    // Slim, without them: 600 + 176 + 90 + 110 = 976.
    const slim: MeterFitInput = { ...full, meter: "slim", name: 200, rest: 820, available: 1196 };
    expect(meterFit(slim)).toMatchObject({ arp: true, typed: 2, modulation: true, nameWidth: "size.meter.name.slim" });
    expect(meterFit({ ...slim, available: 1075 })).toMatchObject({ arp: false, typed: 1, modulation: true });
    // Without the input the arpeggio is not shown and R2's fit is as it was.
    const { arp: _, ...r2 } = full;
    expect(meterFit({ ...r2, rest: 800, available: 1107 })).toMatchObject({ arp: false, typed: 1, modulation: true });
  });
});

describe("the mixer strip", () => {
  beforeEach(() => vi.useFakeTimers({ toFake: ["requestAnimationFrame", "cancelAnimationFrame"] }));
  afterEach(() => vi.useRealTimers());

  it("shows 「アルペジオ」 in the heading only while on; a press turns it off", () => {
    const s = scene("arpOn");
    const [first, second] = s.song.tracks;
    show(MixerStrips, s);
    const chips = qa('[data-test="mixer-arp"]');
    expect(chips.map((c) => [text(c), c.closest('[data-test="mixer-strip"]')!.getAttribute("data-strip")])).toEqual([["アルペジオ", `track-${first!.id}`]]);
    expect(chips[0]!.parentElement!.classList.contains("head")).toBe(true);
    chips[0]!.click();
    expect(app.sent).toEqual([{ type: "setArp", track: first!.id, arp: { ...first!.arp, on: false } }]);
    expect(second!.arp.on).toBe(false);
  });

  it("cannot be pressed while recording", () => {
    const s = scene("arpOn");
    s.transport.state = "recording";
    show(MixerStrips, s);
    const chip = q('[data-test="mixer-arp"]')!;
    expect(chip.getAttribute("aria-disabled")).toBe("true");
    chip.click();
    expect(app.sent).toEqual([]);
  });
});

describe("the fake", () => {
  it("takes setArp as Rust does: one step with its row; octaves 1 to 3; nothing for vocal tracks, unknown ids, the same value, or while recording", async () => {
    const start = scene("arpSettings");
    const transport = createFakeTransport(start);
    await transport.subscribe(
      () => {},
      () => {},
    );
    const keys = start.song.tracks[1]!;
    const before = (await transport.snapshot()).state;
    const send = async (c: Command) => {
      await transport.send(c);
      return (await transport.snapshot()).state;
    };
    let s = await send({ type: "setArp", track: keys.id, arp: { ...ON, shape: "down", octaves: 7 } });
    expect(arpOf(s, keys.id)).toEqual({ ...ON, shape: "down", octaves: 3 });
    expect([s.song.undoCount, s.song.redoCount, s.history.current]).toEqual([before.song.undoCount + 1, 0, before.history.current + 1]);
    const rows = await transport.historyRows(s.history.current, s.history.current);
    expect(rows).toEqual([
      {
        index: s.song.undoCount,
        step: s.song.undoCount,
        name: { kind: "setArp" },
        what: { kind: "track", track: { number: keys.number, name: keys.name } },
        place: { kind: "track", track: { number: keys.number, name: keys.name } },
      },
    ]);
    const undo = s.song.undoCount;
    s = await send({ type: "setArp", track: keys.id, arp: { ...ON, shape: "down", octaves: 3 } });
    s = await send({ type: "setArp", track: 99_999, arp: ON });
    expect(s.song.undoCount).toBe(undo);
    s = await send({ type: "setArp", track: keys.id, arp: { ...ON, octaves: 0 } });
    expect(arpOf(s, keys.id).octaves).toBe(1);

    const vocal = scene("arpSettings");
    vocal.song.tracks[2]!.kind = { kind: "vocal", input: "mono1", inputReady: true, armed: false, monitor: false };
    vocal.transport.state = "stopped";
    const v = createFakeTransport(vocal);
    await v.send({ type: "setArp", track: vocal.song.tracks[2]!.id, arp: ON });
    expect(arpOf((await v.snapshot()).state, vocal.song.tracks[2]!.id).on).toBe(false);

    const busy = scene("arpSettings");
    busy.transport.state = "recording";
    const b = createFakeTransport(busy);
    await b.send({ type: "setArp", track: busy.song.tracks[1]!.id, arp: ON });
    expect(arpOf((await b.snapshot()).state, busy.song.tracks[1]!.id).on).toBe(false);
  });
});

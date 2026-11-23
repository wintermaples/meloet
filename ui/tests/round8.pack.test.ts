// Sound pack on screen (spec 2 (Hint band (sound pack)), (「まだの音色」 row in the shelf), spec 3 43-B, 44-A, 45-A and
// where the sound pack can be fetched later, the 7 sound pack scenes in spec 6. mock F8-PackAsk, F8-PackGet, F8-PackFail, F8-PackSettings, Opt8-PackShow).
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, PackFailure } from "../src/bridge";
import { createFakeTransport, fakeScene, type FakeState } from "../src/bridge/fake";
import AboutDialog from "../src/components/about/AboutDialog.svelte";
import { about, closeAbout, openAbout, showLicenses } from "../src/components/about/about.svelte";
import Arrange from "../src/components/Arrange.svelte";
import InputMeter from "../src/components/InputMeter.svelte";
import { askInStatus, failedLines, fetchingLines, flowHintUp, minutesOf, packBand, packInStatus, percentOf } from "../src/components/pack/pack";
import RightRail from "../src/components/RightRail.svelte";
import SettingsDialog from "../src/components/SettingsDialog.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { t } from "../src/text/ja";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const MB = 1_000_000;
const mounted: Record<string, unknown>[] = [];
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  for (const m of mounted.splice(0)) unmount(m);
  target.remove();
  closeAbout();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState | null = null): void {
  if (state) app.setState(state);
  mounted.push(mount(component, { target }));
  flushSync();
}
const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const scene = (name: string) => fakeScene(name)!.state as FakeState;
const click = (sel: string) => {
  q(sel)!.click();
  flushSync();
};

describe("hint in the input meter band (spec 3 43-B, 44-A)", () => {
  it("asking (F8-PackAsk): 2 lines, 「取りにいく」 (the chosen look) and 「あとで」. Left shows only the input and the held notes. Showing it sends nothing", () => {
    show(InputMeter, scene("packAsk"));
    const band = q("[data-test=pack-band]")!;
    expect(band.dataset.kind).toBe("ask");
    expect(text(q("[data-test=pack-title]"))).toBe("音の素材（約 240 MB）を取りにいきます");
    expect(text(q("[data-test=pack-detail]"))).toBe("グランドピアノなど 31 の音色 ・ 取っている間も弾けます");
    expect(text(q("[data-test=pack-fetch]"))).toBe("取りにいく");
    expect(q("[data-test=pack-fetch]")!.classList.contains("primary")).toBe(true);
    expect(text(q("[data-test=pack-later]"))).toBe("あとで");
    expect(q("[data-test=keyboard-name]")).not.toBeNull();
    expect(q("[data-test=held]")).not.toBeNull();
    for (const gone of ["velocity", "sustain", "pitch-bend", "modulation", "meter-right"]) expect(q(`[data-test=${gone}]`)).toBeNull();
    expect(app.sent).toEqual([]);
    click("[data-test=pack-fetch]");
    click("[data-test=pack-later]");
    expect(app.sent).toEqual([{ type: "fetchPack" }, { type: "packLater" }]);
  });

  it("fetching (F8-PackGet): 2 lines, a bar, 「やめる」. The time left is not shown until known", () => {
    const s = scene("packGet");
    show(InputMeter, s);
    expect(q("[data-test=pack-band]")!.dataset.kind).toBe("fetching");
    expect(text(q("[data-test=pack-title]"))).toBe("音の素材を取りにいっています");
    expect(text(q("[data-test=pack-detail]"))).toBe("38 % ・ 92 / 240 MB ・ のこり 約 2 分");
    expect(q<HTMLElement>("[data-test=pack-bar] .fill")!.style.transform).toBe("scaleX(0.38)");
    expect(text(q("[data-test=pack-stop]"))).toBe("やめる");
    click("[data-test=pack-stop]");
    expect(app.sent).toEqual([{ type: "stopPackFetch" }]);
    expect(fetchingLines({ doneBytes: 92 * MB, totalBytes: 240 * MB, remainingS: null })).toEqual(["音の素材を取りにいっています", "38 % ・ 92 / 240 MB"]);
  });

  const failures: [string, PackFailure, string, string][] = [
    ["packFailOffline", { kind: "offline" }, "インターネットにつながっていません ・ 92 / 240 MB まで取ってあります", "もう一度"],
    ["packFailCutOff", { kind: "cutOff" }, "とちゅうで切れました。92 / 240 MB まで取ってあります。続きから取ります。", "続きから"],
    ["packFailDiskFull", { kind: "diskFull", neededBytes: 160 * MB, freeBytes: 10 * MB }, "ディスクがいっぱいで、音の素材を置けません（あと 150 MB 要ります）。", "もう一度"],
    ["packFailCorrupt", { kind: "corrupt" }, "取った音の素材がこわれていました。取りなおしても直りませんでした。", "もう一度"],
  ];
  it.each(failures)("could not fetch (%s): the text for each reason, the button in the chosen look, 「あとで」", (name, failure, detail, again) => {
    show(InputMeter, scene(name));
    expect(q("[data-test=pack-band]")!.dataset.kind).toBe("failed");
    expect(text(q("[data-test=pack-title]"))).toBe("音の素材を取りにいけませんでした");
    expect(text(q("[data-test=pack-detail]"))).toBe(detail);
    expect(failedLines(failure, 92 * MB, 240 * MB)[1]).toBe(detail);
    expect(text(q("[data-test=pack-again]"))).toBe(again);
    expect(q("[data-test=pack-again]")!.classList.contains("primary")).toBe(true);
    click("[data-test=pack-again]");
    click("[data-test=pack-later]");
    expect(app.sent).toEqual([{ type: "fetchPack" }, { type: "packLater" }]);
  });

  it("while a hint of flows 1 to 4 is shown, asking waits (no keyboard, no output yet). 「音が出ました」 in the arrange does not wait (B-95-12)", () => {
    const base = scene("packAsk");
    expect(flowHintUp(base)).toBe(false);
    const noKeys = structuredClone(base);
    noKeys.keyboards.list = [];
    noKeys.keyboards.active = null;
    const noOutput = structuredClone(base);
    noOutput.outputs.chosen = null;
    const hint = structuredClone(base);
    hint.workspace.hint = "soundCameOut";
    for (const s of [noKeys, noOutput]) {
      expect(flowHintUp(s)).toBe(true);
      expect(packBand(s)).toBeNull();
    }
    expect(flowHintUp(hint)).toBe(false);
    expect(packBand(hint)).toEqual({ kind: "ask" });
    show(InputMeter, noKeys);
    expect(q("[data-test=pack-band]")).toBeNull();
    expect(text(q("[data-test=meter-right]"))).toContain("鍵盤を USB でつなぐと、ここに出ます。");
  });

  it("on an audio track in the split form too, the hint comes first (no mic meter)", () => {
    const s = scene("vocalFit");
    s.sounds.ask = true;
    show(InputMeter, s);
    expect(q("[data-test=pack-band]")!.dataset.kind).toBe("ask");
    expect(q("[data-test=meter-mic-half]")).toBeNull();
  });

  it("on the slim input meter (lower band open), in one line", () => {
    const s = scene("packGet");
    s.workspace.layout.meter = "slim";
    show(InputMeter, s);
    expect(q("[data-test=pack-band]")!.classList.contains("slim")).toBe(true);
    expect(text(q("[data-test=pack-title]"))).toBe("音の素材を取りにいっています");
  });

  it("numbers: % does not reach 100 until done. Time left in minutes (1 even under a minute)", () => {
    expect(percentOf(92 * MB, 240 * MB)).toBe(38);
    expect(percentOf(240 * MB - 1, 240 * MB)).toBe(99);
    expect(percentOf(240 * MB, 240 * MB)).toBe(100);
    expect(minutesOf(120)).toBe(2);
    expect(minutesOf(10)).toBe(1);
  });
});

describe("status line (the 7 sound pack scenes in spec 6, the 44-B form)", () => {
  it("if a flow hint appears while fetching, progress goes to the left of the status line (「音の素材を取りにいっています」, bar, 38 %, 「やめる」)", () => {
    const s = scene("packGet");
    s.workspace.chooser = scene("connect2").workspace.chooser;
    expect(packInStatus(s)).not.toBeNull();
    show(InputMeter, s);
    show(StatusLine);
    expect(q("[data-test=pack-band]")).toBeNull();
    const status = q("[data-test=pack-status]")!;
    expect(text(status.querySelector(".accent"))).toBe("音の素材を取りにいっています");
    expect(text(q("[data-test=pack-percent]"))).toBe("38 %");
    click("[data-test=pack-status-stop]");
    expect(app.sent).toEqual([{ type: "stopPackFetch" }]);
  });

  it("while there is no keyboard, it was pulled out, or input stopped (hints that stay), asking is also in the status line (「取りにいく」「あとで」)", () => {
    const base = scene("packAsk");
    const noKeys = structuredClone(base);
    noKeys.keyboards.list = [];
    noKeys.keyboards.active = null;
    const stopped = structuredClone(base);
    stopped.keyboards.inputStopped = true;
    // At the first start without a keyboard, there is no output yet and the output chooser is not shown. It still asks.
    const noKeysNoOutput = structuredClone(noKeys);
    noKeysNoOutput.outputs.chosen = null;
    for (const s of [noKeys, stopped, noKeysNoOutput]) expect(askInStatus(s)).toBe(true);
    show(InputMeter, noKeys);
    show(StatusLine);
    expect(q("[data-test=pack-band]")).toBeNull();
    expect(text(q("[data-test=pack-status-ask] .accent"))).toBe("音の素材（約 240 MB）を取りにいきます");
    expect(q("[data-test=status-badge]")).toBeNull();
    click("[data-test=pack-status-fetch]");
    click("[data-test=pack-status-later]");
    expect(app.sent).toEqual([{ type: "fetchPack" }, { type: "packLater" }]);
  });

  it("during the output chooser or a one-time hint, the status line also waits without asking (shown in the band after)", () => {
    const base = scene("packAsk");
    expect(askInStatus(base)).toBe(false);
    const chooser = structuredClone(base);
    chooser.keyboards.list = [];
    chooser.keyboards.active = null;
    chooser.outputs.chosen = null;
    chooser.workspace.chooser = scene("connect2").workspace.chooser;
    expect(chooser.workspace.chooser).not.toBeNull();
    const hint = structuredClone(base);
    hint.keyboards.list = [];
    hint.keyboards.active = null;
    hint.workspace.hint = "soundCameOut";
    expect(askInStatus(chooser)).toBe(false);
    show(StatusLine, chooser);
    expect(q("[data-test=pack-status-ask]")).toBeNull();
    expect(askInStatus(hint)).toBe(true);
    const answered = structuredClone(base);
    answered.keyboards.list = [];
    answered.keyboards.active = null;
    answered.sounds.ask = false;
    expect(askInStatus(answered)).toBe(false);
  });

  it("while in the band, no progress in the status line", () => {
    show(StatusLine, scene("packGet"));
    expect(q("[data-test=pack-status]")).toBeNull();
    expect(text(target)).toContain("グランドピアノ が鳴るようになりました ・ のこりの音色を取っています");
  });

  const lines: [string, string, string | null][] = [
    ["packAsk", "音の素材がまだありません ・ いまはサイン波で鳴ります", null],
    ["packFailOffline", "音の素材を取りにいけませんでした（インターネットにつながっていません）。つながったら「もう一度」を押してください。", "もう一度"],
    ["packFailCutOff", "とちゅうで切れました。92 / 240 MB まで取ってあります。続きから取ります。", "続きから"],
    ["packFailDiskFull", "ディスクがいっぱいで、音の素材を置けません（あと 150 MB 要ります）。", "もう一度"],
    ["packFailCorrupt", "取った音の素材がこわれていました。取りなおしても直りませんでした。", "もう一度"],
    ["packNew", "音の素材の新しい包みがあります（12 MB）。", "取りにいく"],
  ];
  it.each(lines)("%s: text and buttons", (name, line, button) => {
    show(StatusLine, scene(name));
    expect(text(q(".message"))).toBe(line);
    const link = q(".link");
    if (button === null) expect(link).toBeNull();
    else {
      expect(text(link)).toBe(button);
      link!.click();
      expect(app.sent).toEqual([{ type: "fetchPack" }]);
    }
  });

  it("all fetched: 「音の素材をぜんぶ取りました。31 の音色が鳴ります。」", () => {
    const s = scene("packGet");
    s.status.message = { id: "packFetched", count: 31 };
    show(StatusLine, s);
    expect(text(q(".message"))).toBe("音の素材をぜんぶ取りました。31 の音色が鳴ります。");
  });
});

describe("「まだの音色」 rows on the shelf (spec 2, spec 3 45-A, mock F8-PackGet)", () => {
  it("name gray.8 and 「取ってきています」「まだ」. No star or ▶, pressing sends nothing. The count is real (31 instruments)", () => {
    show(RightRail, scene("packGet"));
    const waiting = qa("[data-test=shelf-not-yet]");
    expect(waiting.length).toBe(31 - 5 - 4);
    const piano = waiting.find((r) => r.dataset.row === "electricPiano")!;
    expect(text(piano.querySelector("[data-test=shelf-not-yet-state]"))).toBe("取ってきています");
    expect(piano.querySelector(".name")!.classList.contains("gone")).toBe(true);
    for (const r of waiting.filter((r) => r !== piano)) expect(text(r.querySelector("[data-test=shelf-not-yet-state]"))).toBe("まだ");
    for (const r of waiting) {
      expect(r.querySelector("[data-test=shelf-star]")).toBeNull();
      expect(r.querySelector("[data-test=shelf-sample]")).toBeNull();
      r.click();
    }
    expect(app.sent).toEqual([]);
    expect(qa("[data-test=shelf-row]").map((r) => r.dataset.row)).toContain("grandPiano");
    expect(text(q("[data-test=shelf-instruments]"))).toBe("音源 31");
    expect(text(q("[data-test=shelf-all]"))).toBe("すべて 31");
    expect(text(qa("[data-test=shelf-heading]")[0]!)).toBe("▾ ピアノ ・ 鍵盤 11");
  });

  it("even with the shelf folded, the fake keeps the 「まだ」 instruments (「音源 31」 in the vertical heading)", async () => {
    const f = createFakeTransport(scene("packAsk"));
    await f.send({ type: "setViewport", width: 1280, height: 800 });
    const s = (await f.snapshot()).state;
    expect(s.shelf.notYet.length).toBe(26);
    show(RightRail, s);
    expect(text(target)).toContain("音源 31");
  });

  it("once the packs are all there, the 「まだ」 rows go away", () => {
    show(RightRail, scene("shelf"));
    expect(qa("[data-test=shelf-not-yet]")).toEqual([]);
  });
});

describe("「音の素材」 in sound settings ▸ advanced settings (spec 3, mock F8-PackSettings, B-95-7)", () => {
  it("「まだ 12 / 31」 and 「取りにいく」, with the location below", () => {
    const s = scene("packSettings");
    show(SettingsDialog, s);
    const row = q("[data-test=settings-pack]")!;
    expect(text(row.querySelector(".name"))).toBe("音の素材");
    expect(text(q("[data-test=settings-pack-state]"))).toBe("まだ 12 / 31");
    expect(text(q("[data-test=settings-pack-place]"))).toBe(s.sounds.place);
    click("[data-test=settings-fetch-pack]");
    expect(app.sent.at(-1)).toEqual({ type: "fetchPack" });
  });

  it("when all are there, only 「ぜんぶあります ・ 240 MB」", () => {
    const s = scene("packSettings");
    s.sounds.have = s.sounds.total;
    show(SettingsDialog, s);
    expect(text(q("[data-test=settings-pack-state]"))).toBe("ぜんぶあります ・ 240 MB");
    expect(q("[data-test=settings-fetch-pack]")).toBeNull();
  });
});

describe("second level of 「meloet について」 (spec 3, where the sound pack can be fetched later)", () => {
  it("opening the 「音の素材」 heading shows the location inside", () => {
    const s = scene("packAsk");
    app.setState(s);
    openAbout();
    showLicenses();
    show(AboutDialog);
    expect(q("[data-test=about-sounds-place]")).toBeNull();
    click("[data-test=about-sounds]");
    expect(about.sections.sounds).toBe(true);
    expect(text(q("[data-test=about-sounds-place]"))).toBe(s.sounds.place);
  });
});

describe("tracks with instruments not fetched yet (spec 3 45-A, 85)", () => {
  it("the instrument in the head is 「サイン波で代わり」", () => {
    show(Arrange, scene("packAsk"));
    expect(text(target)).toContain(t("track.standIn"));
  });
});

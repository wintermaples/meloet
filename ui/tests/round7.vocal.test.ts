// Screen part: vocal track heads, input meter left and right, recording, status line, sound settings ▸ advanced settings, latency measuring, the 19-A panel.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command, StatusMessage } from "../src/bridge";
import { createFakeTransport, fakeScene, reduceRound7, type FakeState, type FakeVocalMemo } from "../src/bridge/fake";
import Arrange from "../src/components/Arrange.svelte";
import InputMeter from "../src/components/InputMeter.svelte";
import LatencyDialog from "../src/components/LatencyDialog.svelte";
import { closeContextMenu, contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import MixerStrips from "../src/components/mixer/MixerStrips.svelte";
import ParamsPanel from "../src/components/params/ParamsPanel.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { Levels } from "../src/gl/levels";
import { watchSlots, type GlSlot } from "../src/gl/slot";
import { t } from "../src/text/ja";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
const slots = new Map<HTMLElement, GlSlot>();
let stopWatch: () => void = () => {};
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
  slots.clear();
  stopWatch = watchSlots({
    attach: (node, slot) => slots.set(node, slot),
    update: (node, slot) => slots.set(node, slot),
    detach: (node) => slots.delete(node),
  });
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  stopWatch();
  closeContextMenu();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState, props: Record<string, unknown> = {}): void {
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}
const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const scene = (name: string) => fakeScene(name)!.state as FakeState;
const menuRows = () => (contextMenu.open?.entries ?? []).filter((e): e is MenuRow => e.kind === "row");
const vocalOf = (s: AppState) => s.song.tracks.find((x) => x.kind.kind === "vocal")!;

describe("vocal track heads (spec 2 (Vocal track head), mocks F7-VocalTrack / F7-VocalRec)", () => {
  it("the second line is 「入口 1 ・ モノ ▾」 and 「モニター」. No dB. Keyboard tracks stay as they are", () => {
    show(Arrange, scene("vocalTrack"));
    const heads = qa('[data-test="track-head"]');
    expect(heads).toHaveLength(6);
    expect(text(heads[5]!.querySelector('[data-test="track-input"]'))).toBe("入口 1 ・ モノ ▾");
    expect(text(heads[5]!.querySelector('[data-test="track-monitor"]'))).toBe("モニター");
    expect(heads[5]!.textContent).not.toContain("dB");
    expect(heads[0]!.querySelector('[data-test="track-input"]')).toBeNull();
    expect(heads[0]!.textContent).toContain("dB");
  });

  it("with no input, 「入口なし ▾」 (gray.8 class)", () => {
    show(Arrange, scene("vocalNoInput"));
    const input = q('[data-test="track-input"]')!;
    expect(text(input)).toBe("入口なし ▾");
    expect(input.classList.contains("no-input")).toBe(true);
  });

  it("monitor on has the 「えらんでいる」 form. Pressing sends toggleMonitor", () => {
    show(Arrange, scene("vocalMonitorOn"));
    const monitor = q('[data-test="track-monitor"]')!;
    expect(monitor.classList.contains("on")).toBe(true);
    expect(monitor.getAttribute("aria-pressed")).toBe("true");
    monitor.click();
    expect(app.sent).toEqual([{ type: "toggleMonitor", track: vocalOf(scene("vocalMonitorOn")).id }]);
  });

  it("● is red (armed) even when not chosen. Pressing sends toggleVocalArm. ● of a keyboard track is the play target (keysTarget)", () => {
    const s = scene("twoRec");
    s.transport.state = "stopped";
    s.transport.recording = null;
    s.transport.recordingVocals = [];
    show(Arrange, s);
    const arms = qa('[data-test="track-arm"]');
    expect(arms.map((b) => b.classList.contains("on"))).toEqual([true, false, false, false, false, true]);
    arms[5]!.click();
    arms[1]!.click();
    expect(app.sent).toEqual([
      { type: "toggleVocalArm", track: vocalOf(s).id },
      { type: "selectTrack", track: s.song.tracks[1]!.id, fromArm: true },
    ]);
  });

  it("while ● is on, the level bar is the incoming sound (mic). When off, the track bar", () => {
    show(Arrange, scene("vocalTrack"));
    const meter = (head: HTMLElement) => slots.get(head.querySelector<HTMLElement>(".meter")!);
    const heads = qa('[data-test="track-head"]');
    expect(meter(heads[5]!)).toMatchObject({ kind: "meter", source: { mic: vocalOf(scene("vocalTrack")).id } });
    expect(meter(heads[0]!)).toMatchObject({ kind: "meter", source: { track: 1 } });
  });

  it("input list: 3 inputs (mark on the current one), a divider, 「音の設定…」. Rows send setVocalInput, sound settings sends openSettings", () => {
    const s = scene("vocalTrack");
    show(Arrange, s);
    q('[data-test="track-input"]')!.click();
    const entries = contextMenu.open!.entries;
    expect(entries.map((e) => (e.kind === "row" ? [t(e.label), e.checked ?? null] : "—"))).toEqual([
      ["入口 1 ・ モノ", true],
      ["入口 2 ・ モノ", false],
      ["入口 1 ・ 2 ・ ステレオ", false],
      "—",
      ["音の設定…", null],
    ]);
    menuRows()[0]!.run();
    menuRows()[2]!.run();
    menuRows()[3]!.run();
    expect(app.sent).toEqual([
      { type: "setVocalInput", track: vocalOf(s).id, input: "stereo12" },
      { type: "openSettings", advanced: true },
    ]);
  });

  it("「録音待機」 on right-click toggles ● on vocal tracks", () => {
    const s = scene("twoRec");
    s.transport.state = "stopped";
    s.transport.recording = null;
    s.transport.recordingVocals = [];
    show(Arrange, s);
    const head = qa('[data-test="track-head"]')[5]!;
    head.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 10, clientY: 10 }));
    const arm = menuRows().find((r) => r.label === "menu.arm")!;
    expect(arm.checked).toBe(true);
    arm.run();
    expect(app.sent.at(-1)).toEqual({ type: "toggleVocalArm", track: vocalOf(s).id });
  });
});

describe("mixer instrument slot (spec 3, vocal track head)", () => {
  it("a vocal track shows 「入口 1 ・ モノ」. Pressing opens the same input list", () => {
    const s = scene("vocalTrack");
    show(MixerStrips, s);
    const frames = qa('[data-test="mixer-frame"]');
    expect(frames.map(text)).toContain("入口 1 ・ モノ");
    frames.find((f) => text(f) === "入口 1 ・ モノ")!.click();
    expect(menuRows().map((r) => t(r.label))).toEqual(["入口 1 ・ モノ", "入口 2 ・ モノ", "入口 1 ・ 2 ・ ステレオ", "音の設定…"]);
  });
});

describe("input meter band (split left and right; spec 2, spec 3 (31-C), mocks F7-VocalTrack / R11-Fit)", () => {
  it("split left and right when there is a vocal track with ● or a chosen vocal track. The right has the input name and track, scale marks, monitor", () => {
    show(InputMeter, scene("vocalTrack"));
    expect(q('[data-test="meter-keys-half"]')).not.toBeNull();
    const mic = q('[data-test="meter-mic-half"]')!;
    expect(text(mic.querySelector('[data-test="mic-input"]'))).toBe("入口 1 ・ モノ");
    expect(text(mic.querySelector('[data-test="mic-track"]'))).toBe("6 歌");
    expect([...mic.querySelectorAll(".mic-scale span")].map((x) => x.textContent)).toEqual(["−60", "−12", "0"]);
    expect(text(mic.querySelector('[data-test="mic-monitor"]'))).toBe("モニター オフ");
    expect(slots.get(mic.querySelector<HTMLElement>('[data-test="mic-bar"]')!)).toMatchObject({ kind: "meter", mic: { peakDb: -Infinity }, gap: 0 });
    // Pedals and so on are hidden.
    expect(q('[data-test="pitch-bend"]')).toBeNull();
    expect(q('[data-test="velocity-fixed"]')).not.toBeNull();
  });

  it("with no vocal track, as before", () => {
    show(InputMeter, scene("tracks6"));
    expect(q('[data-test="meter-mic-half"]')).toBeNull();
  });

  it("the play target is where keys go (keysTarget; 「1 ピアノ」 even with the vocal chosen; B-94-1). 「—」 if none", () => {
    show(InputMeter, scene("vocalTrack"));
    expect(text(q('[data-test="meter-target"]'))).toBe("1 ピアノ");
    unmount(mounted!);
    const s = scene("vocalTrack");
    s.song.keysTarget = null;
    show(InputMeter, s);
    expect(text(q('[data-test="meter-target"]'))).toBe("—");
  });

  it("the current dB is the latest high value (integer). At 0 dB the text is rec. The text box is under the canvas (cover)", () => {
    const s = scene("vocalTrack");
    show(InputMeter, s);
    const id = vocalOf(s).id;
    app.setTelemetry(app.telemetryWith({ mics: [{ track: id, peak: [0.025, 0.025], clipped: false, stereo: false }] }));
    flushSync();
    expect(text(q('[data-test="mic-db"]'))).toBe("−32 dB");
    expect(q('[data-test="mic-db"]')!.classList.contains("rec")).toBe(false);
    expect(slots.get(q('[data-test="mic-db"]')!.parentElement!)).toMatchObject({ kind: "cover" });
    app.setTelemetry(app.telemetryWith({ mics: [{ track: id, peak: [1, 1], clipped: true, stereo: false }] }));
    flushSync();
    expect(text(q('[data-test="mic-db"]'))).toBe("0 dB");
    expect(q('[data-test="mic-db"]')!.classList.contains("rec")).toBe(true);
  });

  it("「モニター オン」 when monitor is on", () => {
    show(InputMeter, scene("vocalMonitorOn"));
    expect(text(q('[data-test="mic-monitor"]'))).toBe("モニター オン");
  });
});

describe("audio clips being recorded (mocks F7-VocalRec / F7-TwoRec)", () => {
  it("for each vocal track being recorded, the growing head (slot of recordingClip) and the rec name 「歌 テイク 1」", () => {
    show(Arrange, scene("twoRec"));
    expect(qa('[data-test="vocal-rec-tail"]')).toHaveLength(1);
    const label = q('[data-test="vocal-rec-label"]')!;
    expect(text(label)).toBe("歌 テイク 1");
    expect(label.classList.contains("recording")).toBe(true);
    expect(slots.get(q('[data-test="vocal-rec-tail"]')!)).toMatchObject({ kind: "rects" });
  });

  it("not shown when stopped", () => {
    show(Arrange, scene("vocalTrack"));
    expect(q('[data-test="vocal-rec-tail"]')).toBeNull();
  });
});

describe("status line text (spec 6)", () => {
  const say = (message: StatusMessage, edit: (s: AppState) => void = () => {}) => {
    const s = scene("vocalTrack");
    s.status.message = message;
    edit(s);
    show(StatusLine, s);
    return text(q(".message"));
  };
  const six = { number: 6, name: "歌" };

  // Protocol 24 text (positions in 4-digit ticks). The text when dragging in one file is the where-shown column of 「音のファイルを 1 つ持ってきている」 in chapter 6.
  it.each<[string, StatusMessage, string]>([
    ["見つからない音", { id: "audioMissing", count: 2 }, "曲で使っている音のファイルが 2 つ見つかりません。そのクリップは鳴りません。"],
    ["切った", { id: "clipSplit", name: "歌 テイク 1", tick: 6 * 7680, count: 1 }, "歌 テイク 1 を 7. 1. 0000 で切りました。"],
    ["入れた", { id: "audioImported", file: "ドラム.wav", track: six, tick: 4 * 7680, count: 1, skipped: 0 }, "ドラム.wav を入れました（6 歌 ・ 5. 1. 0000）。"],
    ["読めない", { id: "audioUnreadable", file: "声.aac" }, "「声.aac」は読めません（読めるのは wav ・ mp3 ・ flac ・ ogg）。"],
    ["持ってきている", { id: "audioDragging", audio: 8, sf2: 0 }, "音のファイルを 8 つ持ってきています ・ 左に落とすと別々のトラック、右に落とすと 1 本にならべる"],
    ["1 つ持ってきている", { id: "audioDragging", audio: 1, sf2: 0 }, "音のファイルを持ってきています ・ 落とすと、枠の所に入れます"],
  ])("約束 24 の文: %s", (_, message, line) => {
    expect(say(message)).toBe(line);
  });

  it("recording vocals: the leading 「● 録音中 ・ 」 is said by the tag", () => {
    const line = say({ id: "recordingVocals", keys: null, vocals: [{ track: six, input: "mono1" }], monitor: true }, (s) => (s.transport.state = "recording"));
    expect(line).toBe("6 歌（入口 1）・ モニター オン ・ 止めるには Space");
  });

  it("recording keyboard and vocals", () => {
    const line = say(
      { id: "recordingVocals", keys: { number: 1, name: "ピアノ" }, vocals: [{ track: six, input: "mono1" }], monitor: true },
      (s) => (s.transport.state = "recording"),
    );
    expect(line).toBe("1 ピアノ（鍵盤）と 6 歌（入口 1）に録っています ・ 止めるには Space");
  });

  it("recording 2 or more vocals (chapter 6). With monitor off, 「モニター オン ・ 」 is removed", () => {
    const vocals = [{ track: six, input: "mono1" as const }, { track: { number: 7, name: "歌 2" }, input: "mono2" as const }];
    const line = say({ id: "recordingVocals", keys: null, vocals, monitor: false }, (s) => (s.transport.state = "recording"));
    expect(line).toBe("6 歌（入口 1）、7 歌 2（入口 2）・ 止めるには Space");
    unmount(mounted!);
    const on = say({ id: "recordingVocals", keys: null, vocals, monitor: true }, (s) => (s.transport.state = "recording"));
    expect(`${text(q('[data-test="status-badge"]'))} ・ ${on}`).toBe(t("msg.recordingVocals", { vocals: "6 歌（入口 1）、7 歌 2（入口 2）", stopKey: "Space" }));
  });

  it("mic too loud (how long): back to the usual summary 4 s after it last reached 0 dB. Shown again if it reaches it again", () => {
    vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
    const vocal = vocalOf(scene("vocalTrack")).id;
    const mic = (clipped: boolean) => app.setTelemetry(app.telemetryWith({ mics: [{ track: vocal, peak: [1, 1], clipped, stereo: false }] }));
    try {
      expect(say({ id: "micTooLoud" })).toBe(t("msg.micTooLoud"));
      vi.advanceTimersByTime(3000);
      mic(true);
      flushSync();
      vi.advanceTimersByTime(3900);
      flushSync();
      expect(text(q(".message"))).toBe(t("msg.micTooLoud"));
      vi.advanceTimersByTime(200);
      flushSync();
      expect(text(q(".message"))).not.toBe(t("msg.micTooLoud"));
      mic(false);
      flushSync();
      expect(text(q(".message"))).not.toBe(t("msg.micTooLoud"));
      mic(true);
      flushSync();
      expect(text(q(".message"))).toBe(t("msg.micTooLoud"));
    } finally {
      app.setTelemetry(null);
      vi.useRealTimers();
    }
  });

  it("vocal track added / no input (「音の設定…」) / first monitor / too loud / no piano roll / measure latency", () => {
    expect(say({ id: "vocalTrackAdded", track: six, input: "mono1" })).toBe(
      "6 歌 を足しました ・ 入口 1 から録ります ・ 歌のトラックの ● は、選んでいなくても自分でつけます",
    );
    unmount(mounted!);
    expect(say({ id: "noInput" })).toContain("音の入口がありません。マイクをつなぐか、音の設定で入口をえらんでください。");
    expect(text(q("button.link"))).toBe("音の設定…");
    unmount(mounted!);
    expect(say({ id: "monitorFirstOn", latencyMs: 13.2 })).toBe(t("msg.monitorFirstOn", { ms: "13" }));
    unmount(mounted!);
    expect(say({ id: "micTooLoud" })).toBe(t("msg.micTooLoud"));
    unmount(mounted!);
    expect(say({ id: "vocalHasNoPianoRoll" })).toBe("歌のトラックにはピアノロールがありません。波形はアレンジで直します。");
    unmount(mounted!);
    expect(say({ id: "latencyMeasureOpen" })).toBe("「遅れを測る」のダイアログをひらいています");
    unmount(mounted!);
    expect(say({ id: "noParams", sound: { kind: "builtin", instrument: "grandPiano" } })).toBe("グランドピアノには、動かせるパラメータがありません");
  });

  it("bands removed too: 「（そのパラメータの帯 2 本も消えました）」 before the first 「。」", () => {
    const line = say({
      id: "effectRemoved",
      insert: { strip: { kind: "track", id: vocalOf(scene("vocalTrack")).id }, slot: 0 },
      effect: { kind: "builtin", effect: "compressor" },
      lanesRemoved: 2,
    });
    expect(line).toContain("を外しました（そのパラメータの帯 2 本も消えました）。取り消せます");
  });
});

describe("the 19-A params panel (mock F7-Answers92)", () => {
  it("the sound name and 「最初から入っている」, 2 texts, 「音源を入れかえる…」 opens the shelf with 「入れかえる」", () => {
    const s = scene("paramsSound");
    show(ParamsPanel, s, { onfold: () => {} });
    expect(text(q('[data-test="params-name"]'))).toBe("グランドピアノ");
    expect(q(".who")!.textContent).toContain("最初から入っている");
    expect(text(q('[data-test="params-where"]'))).toBe("1 ピアノ ・ 音源");
    const body = q('[data-test="params-no-sound-params"]')!;
    expect(body.textContent).toContain("この音色には、動かせるパラメータがありません。");
    expect(body.textContent).toContain("音量とパンはミキサーで、音色を変えるには入れかえます。");
    q<HTMLButtonElement>('[data-test="params-replace-sound"]')!.click();
    const id = (s.params.selected as { track: number }).track;
    expect(app.sent).toEqual([{ type: "openShelf", mode: { kind: "replace", track: id } }]);
  });

  it("SF2 sounds have the format mark 「SF2」", () => {
    const s = scene("paramsSound");
    s.params.selected = { kind: "sound", track: s.song.tracks[0]!.id, sound: { kind: "sf2", name: "Clean Guitar", file: "FluidR3_GM.sf2" }, knobs: null };
    show(ParamsPanel, s, { onfold: () => {} });
    expect(text(q('[data-test="params-name"]'))).toBe("Clean Guitar");
    expect(text(q('[data-test="params-format"]'))).toBe("SF2");
  });
});

describe("latency measuring (mock F7-Latency)", () => {
  it("while measuring: 「測っています」, a bar and 「3 / 5」, the number the PC says and the measured number (average of 3 so far). Use-this-number is disabled. Stop sends stop", () => {
    show(LatencyDialog, scene("latency"));
    expect(text(q('[data-test="latency-progress"]'))).toBe("測っています 3 / 5");
    expect(text(q('[data-test="latency-reported"]'))).toBe("8.0 ms");
    expect(text(q('[data-test="latency-measured"]'))).toBe("12.4 ms");
    expect(q(".table")!.textContent).toContain("（まだ 3 回の平均）");
    expect(q<HTMLButtonElement>('[data-test="latency-use"]')!.disabled).toBe(true);
    q('[data-test="latency-cancel"]')!.click();
    q('[data-test="latency-close"]')!.click();
    expect(app.sent).toEqual([{ type: "stopLatencyMeasure" }, { type: "closeLatencyMeasure" }]);
  });

  it("after measuring, 「もう一度」 and 「この数を使う」 (spec 3 (94-19))", () => {
    show(LatencyDialog, scene("latencyDone"));
    expect(q('[data-test="latency-progress"]')).toBeNull();
    expect(qa(".foot button").map((b) => b.dataset.test)).toEqual(["latency-cancel", "latency-again", "latency-use"]);
    expect(text(q('[data-test="latency-again"]'))).toBe("もう一度");
    q('[data-test="latency-again"]')!.click();
    q('[data-test="latency-use"]')!.click();
    expect(app.sent).toEqual([{ type: "startLatencyMeasure" }, { type: "useMeasuredLatency" }]);
  });

  it("could not measure: the spec text and 「もう一度」", () => {
    show(LatencyDialog, scene("latencyFailed"));
    expect(text(q('[data-test="latency-failed"]'))).toBe("音が小さくて測れませんでした。ヘッドホンの音量を上げて、もう一度測ってください。");
    q('[data-test="latency-again"]')!.click();
    expect(app.sent).toEqual([{ type: "startLatencyMeasure" }]);
  });
});

describe("fake answers (protocol 23 intents)", () => {
  const memo = (): FakeVocalMemo => ({ monitorSaid: false });
  const apply = (s: FakeState, c: Command, m = memo()): FakeState => {
    const changes = reduceRound7(s, c, m)!;
    const next = structuredClone(s);
    for (const ch of changes) (next as unknown as Record<string, unknown>)[ch.section] = ch.value;
    return next;
  };

  it("adding a vocal track: named 「歌」, input 1, chosen with ● on. The status line says so", () => {
    const s = scene("tracks6");
    s.inputs.chosen = "mic";
    const next = apply(s, { type: "addVocalTrack" });
    const added = next.song.tracks.at(-1)!;
    expect([added.name, added.kind]).toEqual(["歌", { kind: "vocal", input: "mono1", inputReady: true, armed: true, monitor: false }]);
    expect(next.song.playTarget).toBe(added.id);
    expect(next.song.keysTarget).toBe(s.song.playTarget);
    expect(next.status.message).toEqual({ id: "vocalTrackAdded", track: { number: added.number, name: "歌" }, input: "mono1" });
  });

  it("E on a vocal track: the piano roll does not open, the status line says so", () => {
    const s = scene("vocalTrack");
    const next = apply(s, { type: "toggleBand", band: "pianoRoll" });
    expect(next.status.message).toEqual({ id: "vocalHasNoPianoRoll" });
    expect(next.workspace.layout).toEqual(s.workspace.layout);
  });

  it("said only the first time monitor is turned on", () => {
    const s = scene("vocalTrack");
    const m = memo();
    const on = apply(s, { type: "toggleMonitor", track: vocalOf(s).id }, m);
    expect(on.status.message).toEqual({ id: "monitorFirstOn", latencyMs: 8 });
    const off = apply(on, { type: "toggleMonitor", track: vocalOf(s).id }, m);
    const again = apply(off, { type: "toggleMonitor", track: vocalOf(s).id }, m);
    expect(again.status.message).toEqual(on.status.message);
    expect(reduceRound7(off, { type: "toggleMonitor", track: vocalOf(s).id }, m)!.some((c) => c.section === "status")).toBe(false);
  });

  it("pressing the sound of a keyboard track shows the sound panel with no parameters (19-A)", async () => {
    const transport = createFakeTransport(structuredClone(scene("tracks6")));
    const before = (await transport.snapshot()).state;
    let state = before;
    await transport.subscribe(
      (m) => {
        if (m.kind !== "patch") return;
        state = { ...state };
        for (const c of m.changes) (state as unknown as Record<string, unknown>)[c.section] = c.value;
      },
      () => {},
    );
    await transport.send({ type: "selectInstrument", track: before.song.tracks[0]!.id });
    expect(state.params.selected).toMatchObject({ kind: "sound", track: before.song.tracks[0]!.id });
    expect(state.status.message).toMatchObject({ id: "noParams" });
  });
});

describe("WebGL: the mic meter (no more slots or contexts)", () => {
  it("Levels look up vocal track inputs by track id. -1 while not measuring", () => {
    const levels = new Levels();
    expect(levels.micChannel(16)).toBe(-1);
    levels.onTelemetry(app.telemetryWith({ trackPeaks: new Float32Array(4), mics: [{ track: 16, peak: [0.5, 0.25], clipped: true, stereo: true }] }), 0);
    const c = levels.micChannel(16);
    expect(c).toBe(6 + 4);
    expect(levels.bar(c, 0).clipped).toBe(true);
    expect(levels.bar(c, 0).length).toBeGreaterThan(levels.bar(c + 1, 0).length);
  });
});

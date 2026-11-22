// 第 7 回 区切り2 の 2 回目（画面）: 歌のトラックの見出し、入力計の左右、録る、状態行、音の設定 ▸ くわしい設定、遅れを測る、19-A の欄。
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

describe("歌のトラックの見出し（仕様書 2 章、モック F7-VocalTrack ・ F7-VocalRec）", () => {
  it("2 段目は「入口 1 ・ モノ ▾」と「モニター」。dB は出さない。鍵盤のトラックは今のまま", () => {
    show(Arrange, scene("vocalTrack"));
    const heads = qa('[data-test="track-head"]');
    expect(heads).toHaveLength(6);
    expect(text(heads[5]!.querySelector('[data-test="track-input"]'))).toBe("入口 1 ・ モノ ▾");
    expect(text(heads[5]!.querySelector('[data-test="track-monitor"]'))).toBe("モニター");
    expect(heads[5]!.textContent).not.toContain("dB");
    expect(heads[0]!.querySelector('[data-test="track-input"]')).toBeNull();
    expect(heads[0]!.textContent).toContain("dB");
  });

  it("入口がないときは「入口なし ▾」（gray.8 の class）", () => {
    show(Arrange, scene("vocalNoInput"));
    const input = q('[data-test="track-input"]')!;
    expect(text(input)).toBe("入口なし ▾");
    expect(input.classList.contains("no-input")).toBe(true);
  });

  it("モニターのオンは「えらんでいる」の形。押すと toggleMonitor", () => {
    show(Arrange, scene("vocalMonitorOn"));
    const monitor = q('[data-test="track-monitor"]')!;
    expect(monitor.classList.contains("on")).toBe(true);
    expect(monitor.getAttribute("aria-pressed")).toBe("true");
    monitor.click();
    expect(app.sent).toEqual([{ type: "toggleMonitor", track: vocalOf(scene("vocalMonitorOn")).id }]);
  });

  it("● は選んでいなくても赤い（armed）。押すと toggleVocalArm。鍵盤のトラックの ● は弾くと鳴る先（keysTarget）", () => {
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

  it("● がついている間、レベルの棒は入ってくる音（mic）。外れているとトラックの棒", () => {
    show(Arrange, scene("vocalTrack"));
    const meter = (head: HTMLElement) => slots.get(head.querySelector<HTMLElement>(".meter")!);
    const heads = qa('[data-test="track-head"]');
    expect(meter(heads[5]!)).toMatchObject({ kind: "meter", source: { mic: vocalOf(scene("vocalTrack")).id } });
    expect(meter(heads[0]!)).toMatchObject({ kind: "meter", source: { track: 1 } });
  });

  it("入口の一覧: 3 つの入口（いまの入口に印）と、区切り、「音の設定…」。行は setVocalInput、音の設定は openSettings", () => {
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

  it("右クリックの「録音待機」は、歌のトラックでは ● の切りかえ", () => {
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

describe("ミキサーの音源の枠（仕様書 3 章「歌のトラックの見出し」）", () => {
  it("歌のトラックは「入口 1 ・ モノ」。押すと同じ入口の一覧", () => {
    const s = scene("vocalTrack");
    show(MixerStrips, s);
    const frames = qa('[data-test="mixer-frame"]');
    expect(frames.map(text)).toContain("入口 1 ・ モノ");
    frames.find((f) => text(f) === "入口 1 ・ モノ")!.click();
    expect(menuRows().map((r) => t(r.label))).toEqual(["入口 1 ・ モノ", "入口 2 ・ モノ", "入口 1 ・ 2 ・ ステレオ", "音の設定…"]);
  });
});

describe("入力計の帯（左右に分けた形。仕様書 2 章、3 章 31-C、モック F7-VocalTrack ・ R11-Fit）", () => {
  it("● のついた歌のトラックか、選んだ歌のトラックがあると左右に分ける。右は入口の名前とトラック、目盛、モニター", () => {
    show(InputMeter, scene("vocalTrack"));
    expect(q('[data-test="meter-keys-half"]')).not.toBeNull();
    const mic = q('[data-test="meter-mic-half"]')!;
    expect(text(mic.querySelector('[data-test="mic-input"]'))).toBe("入口 1 ・ モノ");
    expect(text(mic.querySelector('[data-test="mic-track"]'))).toBe("6 歌");
    expect([...mic.querySelectorAll(".mic-scale span")].map((x) => x.textContent)).toEqual(["−60", "−12", "0"]);
    expect(text(mic.querySelector('[data-test="mic-monitor"]'))).toBe("モニター オフ");
    expect(slots.get(mic.querySelector<HTMLElement>('[data-test="mic-bar"]')!)).toMatchObject({ kind: "meter", toZero: true, gap: 0 });
    // ペダルなどは引っこめる。
    expect(q('[data-test="pitch-bend"]')).toBeNull();
    expect(q('[data-test="velocity-fixed"]')).not.toBeNull();
  });

  it("歌のトラックがなければ今のまま", () => {
    show(InputMeter, scene("tracks6"));
    expect(q('[data-test="meter-mic-half"]')).toBeNull();
  });

  it("弾くと鳴る先は鍵盤の行き先（keysTarget。歌を選んでいても 1 ピアノ。B-94-1）。なければ「—」", () => {
    show(InputMeter, scene("vocalTrack"));
    expect(text(q('[data-test="meter-target"]'))).toBe("1 ピアノ");
    unmount(mounted!);
    const s = scene("vocalTrack");
    s.song.keysTarget = null;
    show(InputMeter, s);
    expect(text(q('[data-test="meter-target"]'))).toBe("—");
  });

  it("いまの dB は直近の大きい値（整数）。0 dB に届いたら字が rec。字の箱は canvas の下（cover）", () => {
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

  it("モニターがオンなら「モニター オン」", () => {
    show(InputMeter, scene("vocalMonitorOn"));
    expect(text(q('[data-test="mic-monitor"]'))).toBe("モニター オン");
  });
});

describe("録っている歌のクリップ（モック F7-VocalRec ・ F7-TwoRec）", () => {
  it("録っている歌のトラックごとに、のびている先（recordingClip の slot）と rec の名前「歌 テイク 1」", () => {
    show(Arrange, scene("twoRec"));
    expect(qa('[data-test="vocal-rec-tail"]')).toHaveLength(1);
    const label = q('[data-test="vocal-rec-label"]')!;
    expect(text(label)).toBe("歌 テイク 1");
    expect(label.classList.contains("recording")).toBe(true);
    expect(slots.get(q('[data-test="vocal-rec-tail"]')!)).toMatchObject({ kind: "rects" });
  });

  it("止まっていれば出さない", () => {
    show(Arrange, scene("vocalTrack"));
    expect(q('[data-test="vocal-rec-tail"]')).toBeNull();
  });
});

describe("状態行の文（仕様書 6 章）", () => {
  const say = (message: StatusMessage, edit: (s: AppState) => void = () => {}) => {
    const s = scene("vocalTrack");
    s.status.message = message;
    edit(s);
    show(StatusLine, s);
    return text(q(".message"));
  };
  const six = { number: 6, name: "歌" };

  // 約束 24 の文（位置は 4 けたのティック）。1 つだけ持ってきているときの文は 6 章「音のファイルを 1 つ持ってきている」の出る所の列。
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

  it("歌を録っている: 頭の「● 録音中 ・ 」は札が言う", () => {
    const line = say({ id: "recordingVocals", keys: null, vocals: [{ track: six, input: "mono1" }], monitor: true }, (s) => (s.transport.state = "recording"));
    expect(line).toBe("6 歌（入口 1）・ モニター オン ・ 止めるには Space");
  });

  it("鍵盤と歌を録っている", () => {
    const line = say(
      { id: "recordingVocals", keys: { number: 1, name: "ピアノ" }, vocals: [{ track: six, input: "mono1" }], monitor: true },
      (s) => (s.transport.state = "recording"),
    );
    expect(line).toBe("1 ピアノ（鍵盤）と 6 歌（入口 1）に録っています ・ 止めるには Space");
  });

  it("歌 2 本以上を録っている（6 章）。モニターがオフなら「モニター オン ・ 」を外す", () => {
    const vocals = [{ track: six, input: "mono1" as const }, { track: { number: 7, name: "歌 2" }, input: "mono2" as const }];
    const line = say({ id: "recordingVocals", keys: null, vocals, monitor: false }, (s) => (s.transport.state = "recording"));
    expect(line).toBe("6 歌（入口 1）、7 歌 2（入口 2）・ 止めるには Space");
    unmount(mounted!);
    const on = say({ id: "recordingVocals", keys: null, vocals, monitor: true }, (s) => (s.transport.state = "recording"));
    expect(`${text(q('[data-test="status-badge"]'))} ・ ${on}`).toBe(t("msg.recordingVocals", { vocals: "6 歌（入口 1）、7 歌 2（入口 2）", stopKey: "Space" }));
  });

  it("マイクの音が大きすぎる（いつまで）: 最後に 0 dB に届いてから 4 秒で、いつものまとめに戻す。また届けば出す", () => {
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

  it("歌のトラックを足した ・ 入口がない（音の設定…）・ はじめてモニター ・ 大きすぎる ・ ピアノロールはない ・ 遅れを測る", () => {
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

  it("帯もいっしょに消えた: 最初の「。」の前に「（そのパラメータの帯 2 本も消えました）」", () => {
    const line = say({
      id: "effectRemoved",
      insert: { strip: { kind: "track", id: vocalOf(scene("vocalTrack")).id }, slot: 0 },
      effect: { kind: "builtin", effect: "compressor" },
      lanesRemoved: 2,
    });
    expect(line).toContain("を外しました（そのパラメータの帯 2 本も消えました）。取り消せます");
  });
});

describe("19-A のパラメータの欄（モック F7-Answers92）", () => {
  it("音色の名前と「最初から入っている」、文 2 つ、「音源を入れかえる…」は棚を「入れかえる」でひらく", () => {
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

  it("SF2 の音色は形式の印「SF2」", () => {
    const s = scene("paramsSound");
    s.params.selected = { kind: "sound", track: s.song.tracks[0]!.id, sound: { kind: "sf2", name: "Clean Guitar", file: "FluidR3_GM.sf2" } };
    show(ParamsPanel, s, { onfold: () => {} });
    expect(text(q('[data-test="params-name"]'))).toBe("Clean Guitar");
    expect(text(q('[data-test="params-format"]'))).toBe("SF2");
  });
});

describe("遅れを測る（モック F7-Latency）", () => {
  it("測っている間: 「測っています」と棒と「3 / 5」、PC が言う数と測った数（まだ 3 回の平均）。この数を使うは使えない。やめるは stop", () => {
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

  it("測り終わったら「もう一度」と「この数を使う」（仕様書 3 章 94-19）", () => {
    show(LatencyDialog, scene("latencyDone"));
    expect(q('[data-test="latency-progress"]')).toBeNull();
    expect(qa(".foot button").map((b) => b.dataset.test)).toEqual(["latency-cancel", "latency-again", "latency-use"]);
    expect(text(q('[data-test="latency-again"]'))).toBe("もう一度");
    q('[data-test="latency-again"]')!.click();
    q('[data-test="latency-use"]')!.click();
    expect(app.sent).toEqual([{ type: "startLatencyMeasure" }, { type: "useMeasuredLatency" }]);
  });

  it("測れなかった: 仕様書の文と「もう一度」", () => {
    show(LatencyDialog, scene("latencyFailed"));
    expect(text(q('[data-test="latency-failed"]'))).toBe("音が小さくて測れませんでした。ヘッドホンの音量を上げて、もう一度測ってください。");
    q('[data-test="latency-again"]')!.click();
    expect(app.sent).toEqual([{ type: "startLatencyMeasure" }]);
  });
});

describe("偽物の答え（約束 23 の意図）", () => {
  const memo = (): FakeVocalMemo => ({ monitorSaid: false });
  const apply = (s: FakeState, c: Command, m = memo()): FakeState => {
    const changes = reduceRound7(s, c, m)!;
    const next = structuredClone(s);
    for (const ch of changes) (next as unknown as Record<string, unknown>)[ch.section] = ch.value;
    return next;
  };

  it("歌のトラックを足す: 名前は「歌」、入口 1、選んで ● がつく。状態行で言う", () => {
    const s = scene("tracks6");
    s.inputs.chosen = "mic";
    const next = apply(s, { type: "addVocalTrack" });
    const added = next.song.tracks.at(-1)!;
    expect([added.name, added.kind]).toEqual(["歌", { kind: "vocal", input: "mono1", inputReady: true, armed: true, monitor: false }]);
    expect(next.song.playTarget).toBe(added.id);
    expect(next.song.keysTarget).toBe(s.song.playTarget);
    expect(next.status.message).toEqual({ id: "vocalTrackAdded", track: { number: added.number, name: "歌" }, input: "mono1" });
  });

  it("歌のトラックで E: ピアノロールはひらかず、状態行で言う", () => {
    const s = scene("vocalTrack");
    const next = apply(s, { type: "toggleBand", band: "pianoRoll" });
    expect(next.status.message).toEqual({ id: "vocalHasNoPianoRoll" });
    expect(next.workspace.layout).toEqual(s.workspace.layout);
  });

  it("はじめてモニターをオンにしたときだけ言う", () => {
    const s = scene("vocalTrack");
    const m = memo();
    const on = apply(s, { type: "toggleMonitor", track: vocalOf(s).id }, m);
    expect(on.status.message).toEqual({ id: "monitorFirstOn", latencyMs: 8 });
    const off = apply(on, { type: "toggleMonitor", track: vocalOf(s).id }, m);
    const again = apply(off, { type: "toggleMonitor", track: vocalOf(s).id }, m);
    expect(again.status.message).toEqual(on.status.message);
    expect(reduceRound7(off, { type: "toggleMonitor", track: vocalOf(s).id }, m)!.some((c) => c.section === "status")).toBe(false);
  });

  it("鍵盤のトラックの音色を押すと、パラメータのない音色の欄（19-A）", async () => {
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

describe("WebGL: マイクの計（区画も文脈もふやさない）", () => {
  it("Levels は歌のトラックの入口を、トラックの id でひく。計っていない間は -1", () => {
    const levels = new Levels();
    expect(levels.micChannel(16)).toBe(-1);
    levels.onTelemetry(app.telemetryWith({ trackPeaks: new Float32Array(4), mics: [{ track: 16, peak: [0.5, 0.25], clipped: true, stereo: true }] }), 0);
    const c = levels.micChannel(16);
    expect(c).toBe(6 + 4);
    expect(levels.bar(c, 0).clipped).toBe(true);
    expect(levels.bar(c, 0).length).toBeGreaterThan(levels.bar(c + 1, 0).length);
  });
});

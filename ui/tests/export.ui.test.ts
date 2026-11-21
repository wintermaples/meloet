// 第 7 回 区切り5 の 2 回目（画面）: 書き出しのダイアログ、「くわしく」、状態行の進みと「やめる」、「フォルダをひらく」、メニューとキー
// （仕様書 3 章 34-B、2 章「書き出しの進み」、6 章、モック F7-Export ・ F7-Exporting ・ F-Export ・ R11-Fit、design-notes B-94-69〜75）。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command, ExportDialogView, ExportView } from "../src/bridge";
import { fakeScene, type FakeState } from "../src/bridge/fake";
import ExportDialog from "../src/components/export/ExportDialog.svelte";
import { isCut } from "../src/components/export/cutHead";
import { detailsSummary, leftText, lengthText, peakNumber, spanText } from "../src/components/export/exportText";
import { closeContextMenu, contextMenu, type MenuRow } from "../src/components/menu/contextMenu.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { handleKeydown } from "../src/keys/attach";
import { SHOW_EVERY_MS } from "../src/state/audioWork.svelte";
import { ExportShown, exportShown } from "../src/state/exportShown.svelte";
import { mapOf } from "../src/time/bars";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state as FakeState;
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  exportShown.forget();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  closeContextMenu();
  exportShown.forget();
  vi.useRealTimers();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const withDialog = (name: string, over: Partial<ExportDialogView>): FakeState => {
  const s = scene(name);
  s.export.dialog = { ...s.export.dialog!, ...over };
  return s;
};

describe("文の部品（B-94-75）", () => {
  it("いちばん大きい所: 小数 1 けた、音がなければ −∞", () => {
    expect([peakNumber(-0.6), peakNumber(1.44), peakNumber(null), peakNumber(-200)]).toEqual(["−0.6", "1.4", "−∞", "−∞"]);
  });
  it("長さは 3:02、残りは 1 分まで秒、ほかは分。まだわからなければ —", () => {
    expect([lengthText(182), lengthText(59.9), lengthText(3600)]).toEqual(["3:02", "0:59", "60:00"]);
    expect([leftText(40), leftText(0.2), leftText(59), leftText(60), leftText(130), leftText(null)]).toEqual(["40 秒", "1 秒", "59 秒", "1 分", "2 分", "—"]);
  });
  it("範囲の下の行とたたんだ「くわしく」の行（しっぽなし ・ 実時間）", () => {
    const s = scene("exportDialog");
    const d = s.export.dialog!;
    const bars = mapOf(s.timeline);
    expect(spanText(d, bars)).toBe("曲ぜんぶ ・ 1. 1. 0000 〜 17. 1. 0000 と、音が消えるまで");
    expect(spanText({ ...d, tail: "off" }, bars)).toBe("曲ぜんぶ ・ 1. 1. 0000 〜 17. 1. 0000");
    expect(spanText({ ...d, range: "loop", span: d.loopRange! }, bars)).toBe("ループ ・ 5. 1. 0000 〜 9. 1. 0000 と、音が消えるまで");
    expect(spanText({ ...d, range: "tracks" }, bars)).toBe("トラックごと ・ 4 つのファイル");
    expect(detailsSummary(d)).toBe("48000 Hz ・ 24 ビット ・ しっぽは音が消えるまで（最大 10 秒）");
    expect(detailsSummary({ ...d, tail: "off", realtime: true, bits: "bits16" })).toBe("48000 Hz ・ 16 ビット ・ しっぽなし ・ 実時間");
  });
});

describe("書き出しのダイアログ（モック F7-Export、仕様書 3 章 34-B）", () => {
  it("F7-Export: 範囲 3 つ、形は wav と flac（mp3 は出さない）、保存する所、たしかめ、たたんだ「くわしく」、やめる ・ 書き出す", () => {
    show(ExportDialog, scene("exportDialog"));
    expect(text(q(".head"))).toBe("書き出し 閉じるEsc");
    expect(qa('[data-test^="export-range-"]').map(text)).toEqual(["曲ぜんぶ", "ループ 5〜9", "トラックごと"]);
    expect(q('[data-test="export-range-song"]')!.classList.contains("on")).toBe(true);
    expect(text(q('[data-test="export-span"]'))).toBe("曲ぜんぶ ・ 1. 1. 0000 〜 17. 1. 0000 と、音が消えるまで");
    expect(qa('[data-test^="export-format-"]').map(text)).toEqual(["wav", "flac"]);
    expect(text(q('[data-test="export-place"]'))).toBe("~/音楽/はじめての曲.wav");
    // 仕様書 6 章「書き出しの前」の文（モックの「音割れなし」ではなく）。
    expect(qa('[data-test="export-check-line"]').map(text)).toEqual(["いちばん大きい所 −0.6 dB ・ 音割れはありません"]);
    expect(text(q('[data-test="export-stopped"]'))).toBe("止まっているプラグインはありません");
    expect(text(q('[data-test="export-details"]'))).toBe("▸ くわしく 48000 Hz ・ 24 ビット ・ しっぽは音が消えるまで（最大 10 秒）");
    expect(q('[data-test="export-adv"]')).toBeNull();
    expect(text(q('[data-test="export-cancel"]'))).toBe("やめるEsc");
    expect(text(q('[data-test="export-start"]'))).toBe("書き出すEnter");
    expect(q<HTMLButtonElement>('[data-test="export-start"]')!.disabled).toBe(false);
  });

  it("押した物は意図だけを送る", () => {
    show(ExportDialog, scene("exportDialog"));
    for (const sel of ["export-range-loop", "export-range-tracks", "export-format-flac", "export-choose", "export-details", "export-start", "export-cancel", "export-close"])
      q(`[data-test="${sel}"]`)!.click();
    expect(app.sent).toEqual([
      { type: "setExport", change: { kind: "range", range: "loop" } },
      { type: "setExport", change: { kind: "range", range: "tracks" } },
      { type: "setExport", change: { kind: "format", format: "flac" } },
      { type: "chooseExportPlace" },
      { type: "setExport", change: { kind: "advancedOpen", open: true } },
      { type: "startExport" },
      { type: "closeExport" },
      { type: "closeExport" },
    ] satisfies Command[]);
  });

  it("ループのない曲 ・ トラックのない曲: その範囲は使えない（数のない「ループ」）", () => {
    show(ExportDialog, withDialog("exportDialog", { loopRange: null, tracks: 0 }));
    expect(text(q('[data-test="export-range-loop"]'))).toBe("ループ");
    expect(q<HTMLButtonElement>('[data-test="export-range-loop"]')!.disabled).toBe(true);
    expect(q<HTMLButtonElement>('[data-test="export-range-tracks"]')!.disabled).toBe(true);
  });

  it("たしかめの用意: 仮の文で、書き出すは押せない", () => {
    show(ExportDialog, scene("exportWaiting"));
    expect(qa('[data-test="export-check-line"]').map(text)).toEqual(["たしかめの用意をしています"]);
    expect(q<HTMLButtonElement>('[data-test="export-start"]')!.disabled).toBe(true);
  });

  it("数えている途中: % とここまでのいちばん大きい所。書き出すは押せる（B-94-72）", () => {
    show(ExportDialog, scene("exportCounting"));
    expect(qa('[data-test="export-check-line"]').map(text)).toEqual(["数えています 38 % ・ ここまでのいちばん大きい所 −3.5 dB"]);
    expect(q<HTMLButtonElement>('[data-test="export-start"]')!.disabled).toBe(false);
  });

  it("音が割れる: 仕様書 6 章の文と「このまま書き出す」。数えている途中でも", () => {
    show(ExportDialog, scene("exportClips"));
    const clipping = "いちばん大きい所が 0 dB をこえています。このままだと音が割れます。マスターを下げてから書き出すのがおすすめです。";
    expect(qa('[data-test="export-check-line"]').map(text)).toEqual([clipping]);
    expect(text(q('[data-test="export-start"]'))).toBe("このまま書き出すEnter");
    unmount(mounted!);
    show(ExportDialog, withDialog("exportDialog", { check: { state: "counting", percent: 50, peakDb: 0.4, clips: true } }));
    expect(qa('[data-test="export-check-line"]').map(text)).toEqual(["数えています 50 % ・ ここまでのいちばん大きい所 0.4 dB", clipping]);
    expect(text(q('[data-test="export-start"]'))).toBe("このまま書き出すEnter");
  });

  it("止まっているプラグイン（F-Export）: 名前と居場所は rec、とばした音で書き出す", () => {
    show(ExportDialog, scene("exportStopped"));
    const row = q('[data-test="export-stopped"]')!;
    expect(text(row)).toMatch(/^止まっているプラグイン Brick EQ ・ \d .+ この枠をとばした音で書き出します$/);
    expect(text(row.querySelector(".rec"))).toMatch(/^Brick EQ ・ /);
  });

  it("たしかめができない: 仮の文、書き出すは押せない", () => {
    show(ExportDialog, scene("exportCheckFailed"));
    expect(qa('[data-test="export-check-line"]').map(text)).toEqual(["Surge XT を書き出し用に読みこめませんでした"]);
    expect(q<HTMLButtonElement>('[data-test="export-start"]')!.disabled).toBe(true);
    unmount(mounted!);
    show(ExportDialog, withDialog("exportDialog", { check: { state: "failed", reason: { kind: "cannotWrite" } }, canExport: false }));
    expect(qa('[data-test="export-check-line"]').map(text)).toEqual(["作業の場所に書けませんでした"]);
  });

  it("ひらいた「くわしく」: サンプルレート ・ ビット ・ しっぽは一覧で、実時間は選ぶ印", () => {
    show(ExportDialog, scene("exportAdvanced"));
    expect(text(q('[data-test="export-details"]'))).toBe("▾ くわしく");
    expect(qa('[data-test="export-adv"] .row').map(text)).toEqual([
      "サンプルレート 48000 Hz▾",
      "ビット 24 ビット▾",
      "しっぽ 音が消えるまで（最大 10 秒）▾",
      "実時間で書き出す",
    ]);
    const rows = () => (contextMenu.open?.entries ?? []).filter((e): e is MenuRow => e.kind === "row");
    q('[data-test="export-rate"]')!.click();
    expect(rows().map((r) => [r.text, r.checked])).toEqual([
      ["44100 Hz", false],
      ["48000 Hz", true],
      ["88200 Hz", false],
      ["96000 Hz", false],
    ]);
    rows()[0]!.run();
    q('[data-test="export-bits"]')!.click();
    expect(rows().map((r) => r.text)).toEqual(["16 ビット", "24 ビット"]);
    rows()[0]!.run();
    q('[data-test="export-tail"]')!.click();
    expect(rows().map((r) => r.text)).toEqual(["音が消えるまで（最大 10 秒）", "なし"]);
    rows()[1]!.run();
    q('[data-test="export-realtime"]')!.click();
    q('[data-test="export-details"]')!.click();
    expect(app.sent).toEqual([
      { type: "setExport", change: { kind: "sampleRate", hz: 44100 } },
      { type: "setExport", change: { kind: "bits", bits: "bits16" } },
      { type: "setExport", change: { kind: "tail", tail: "off" } },
      { type: "setExport", change: { kind: "realtime", on: true } },
      { type: "setExport", change: { kind: "advancedOpen", open: false } },
    ] satisfies Command[]);
  });

  it("速く回せないプラグインがあれば、実時間は外せない（使えない形でオン）", () => {
    show(ExportDialog, withDialog("exportAdvanced", { realtime: true, realtimeForced: true }));
    const pick = q<HTMLButtonElement>('[data-test="export-realtime"]')!;
    expect([pick.disabled, pick.getAttribute("aria-checked")]).toEqual([true, "true"]);
  });

  it("R11-Fit: 中身はスクロールする箱の中、見出しと下のボタンはその外", () => {
    show(ExportDialog, scene("exportFit"));
    const body = q('[data-test="export-body"]')!;
    expect(body.contains(q('[data-test="export-check"]'))).toBe(true);
    expect(body.contains(q(".head"))).toBe(false);
    expect(body.contains(q('[data-test="export-start"]'))).toBe(false);
  });
});

describe("キー（仕様書 4 章、3 章の Esc の順）", () => {
  function key(s: AppState, init: KeyboardEventInit): Command[] {
    const sent: Command[] = [];
    handleKeydown(new KeyboardEvent("keydown", { cancelable: true, ...init }), {
      state: () => s,
      send: (c) => void sent.push(c),
      nav: { platform: "Linux x86_64" },
    });
    return sent;
  }

  it("Esc でとじる、Enter で書き出す（押せないときは何もしない）", () => {
    expect(key(scene("exportDialog"), { key: "Escape" })).toEqual([{ type: "closeExport" }]);
    expect(key(scene("exportDialog"), { key: "Enter" })).toEqual([{ type: "startExport" }]);
    expect(key(scene("exportWaiting"), { key: "Enter" })).toEqual([]);
  });

  it("棚でためし弾き中でも、ダイアログが先", () => {
    const s = scene("shelfAudition");
    s.export.dialog = scene("exportDialog").export.dialog;
    expect(key(s, { key: "Escape" })).toEqual([{ type: "closeExport" }]);
  });

  it("Mod Shift E でひらく。録音中 ・ カウントイン中 ・ 書き出している間 ・ もうひらいているときは送らない（B-94-74）", () => {
    const modShiftE = { key: "E", code: "KeyE", ctrlKey: true, shiftKey: true };
    expect(key(scene("tracks6"), modShiftE)).toEqual([{ type: "openExport" }]);
    for (const st of ["recording", "countIn"] as const) {
      const s = scene("tracks6");
      s.transport.state = st;
      expect(key(s, modShiftE), st).toEqual([]);
    }
    expect(key(scene("exporting"), modShiftE)).toEqual([]);
    expect(key(scene("exportDialog"), modShiftE)).toEqual([]);
    expect(key(scene("exportDone"), modShiftE)).toEqual([{ type: "openExport" }]);
  });
});

describe("保存する所の欄: 入りきらない道は、頭を省いて終わり（ファイルの名前）を見せる", () => {
  const LONG = "~/ミュージック/とても長い名前のフォルダ/もっと長い名前のフォルダ/k7m5-song16.wav";
  const BOX = 300;
  const CHAR = 8;
  const proto = HTMLElement.prototype;
  const hadOffset = Object.getOwnPropertyDescriptor(proto, "offsetWidth");
  let observed: { fire: () => void; nodes: Element[] }[] = [];
  beforeEach(() => {
    // jsdom はレイアウトをしない: 字は 1 字 8、欄は 300 とする。
    Object.defineProperty(proto, "offsetWidth", {
      configurable: true,
      get(this: HTMLElement) {
        return this.hasAttribute("data-cut-text") ? [...(this.textContent ?? "")].length * CHAR : 0;
      },
    });
    Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => BOX });
    observed = [];
    vi.stubGlobal(
      "ResizeObserver",
      class {
        nodes: Element[] = [];
        constructor(fire: () => void) {
          observed.push({ fire, nodes: this.nodes });
        }
        observe(node: Element): void {
          this.nodes.push(node);
        }
        disconnect(): void {}
      },
    );
  });
  afterEach(() => {
    if (hadOffset) Object.defineProperty(proto, "offsetWidth", hadOffset);
    delete (proto as { clientWidth?: number }).clientWidth;
    vi.unstubAllGlobals();
  });

  it("幅は内がわのすきまを引いてくらべる", () => {
    expect([isCut(284, 300, 8, 8), isCut(285, 300, 8, 8), isCut(300, 300, 0, 0)]).toEqual([false, true, false]);
  });

  it("入りきる道には印がない。入りきらない道は data-cut がつき、字はまるごと「終わりをそろえる箱」の中にある", () => {
    show(ExportDialog, scene("exportDialog"));
    const short = q('[data-test="export-place"]')!;
    expect(short.hasAttribute("data-cut")).toBe(false);
    unmount(mounted!);

    show(ExportDialog, withDialog("exportDialog", { place: LONG }));
    const field = q('[data-test="export-place"]')!;
    expect(field.hasAttribute("data-cut")).toBe(true);
    // 字は切らずに置く（終わりのファイルの名前まで）。頭を隠すのは CSS（data-cut の欄の「…」と、箱の終わりそろえ）。
    const inner = field.querySelector(".path > [data-cut-text]")!;
    expect(inner.textContent).toBe(LONG);
    expect(inner.textContent!.endsWith("k7m5-song16.wav")).toBe(true);
    expect(text(field)).toBe(LONG);
  });

  it("道や幅が変わると測りなおす（欄と字の大きさを見はる）", () => {
    const s = withDialog("exportDialog", { place: LONG });
    show(ExportDialog, s);
    const field = q('[data-test="export-place"]')!;
    const watch = observed.find((o) => o.nodes.includes(field))!;
    expect(watch.nodes).toContain(field.querySelector("[data-cut-text]"));
    app.setState({ ...s, export: { ...s.export, dialog: { ...s.export.dialog!, place: "~/音楽/はじめての曲.wav" } } });
    flushSync();
    watch.fire();
    expect(field.hasAttribute("data-cut")).toBe(false);
    expect(text(field)).toBe("~/音楽/はじめての曲.wav");
  });
});

describe("状態行（仕様書 2 章「書き出しの進み」、6 章、モック F7-Export ・ F7-Exporting）", () => {
  it("ダイアログをひらいている間は「「書き出し」のダイアログをひらいています」", () => {
    show(StatusLine, scene("exportDialog"));
    expect(text(q(".message"))).toBe("「書き出し」のダイアログをひらいています");
  });

  it("書き出している間: 札の代わりに「書き出しています」、棒、%、ファイルと残り、「やめる」は cancelExport", () => {
    show(StatusLine, scene("exporting"));
    const run = q('.status [data-test="export-run"]')!;
    expect(q('.status [data-test="status-badge"]')).toBeNull();
    expect(text(run.querySelector(".accent"))).toBe("書き出しています");
    expect(text(run.querySelector('[data-test="export-percent"]'))).toBe("46 %");
    expect(run.querySelector<HTMLElement>(".fill")!.style.transform).toBe("scaleX(0.46)");
    expect(text(run.querySelector('[data-test="export-rest"]'))).toBe("はじめての曲.wav ・ 残り 約 40 秒 ・ 弾けます（書き出しの音は聞こえません）");
    run.querySelector<HTMLElement>('[data-test="export-cancel"]')!.click();
    expect(app.sent).toEqual([{ type: "cancelExport" }]);
  });

  it("実時間で書き出している: 仕様書 6 章の文", () => {
    show(StatusLine, scene("exportingRealtime"));
    expect(text(q('.status [data-test="export-rest"]'))).toBe("速く書き出せないプラグインがあるので、実時間で書き出します（3:02）。");
  });

  it("人が「実時間で書き出す」を選んだだけのときは、プラグインのせいにしない", () => {
    const s = scene("exportingRealtime");
    s.export.run = { state: "running", percent: 12, file: "はじめての曲.wav", remainingS: 160, realtime: true, realtimeForced: false, lengthS: 182 };
    show(StatusLine, s);
    expect(text(q('.status [data-test="export-rest"]'))).toBe("実時間で書き出しています（3:02）。");
  });

  it("終わった: 1 つのファイルは「書き出した」の文と「フォルダをひらく」（showExported）。トラックごとはフォルダ", () => {
    show(StatusLine, scene("exportDone"));
    expect(text(q('.status [data-test="status-badge"]'))).toBe("■ 停止中");
    expect(text(q(".status .message"))).toBe("はじめての曲.wav を書き出しました（3:02 ・ 48000 Hz ・ 24 ビット）。");
    const folder = [...target.querySelectorAll<HTMLButtonElement>(".status button")].find((b) => text(b) === "フォルダをひらく")!;
    folder.click();
    expect(app.sent).toEqual([{ type: "showExported" }]);
    unmount(mounted!);
    const s = scene("exportDone");
    s.export.run = { state: "done", file: "ピアノ.wav", files: 4, place: "~/ミュージック/はじめての曲/", lengthS: 40, sampleRate: 48000, bits: "bits24" };
    show(StatusLine, s);
    expect(text(q(".status .message"))).toBe("書き出しました: ~/ミュージック/はじめての曲/");
  });

  it("やめた ・ できなかった", () => {
    show(StatusLine, scene("exportCancelled"));
    expect(text(q(".status .message"))).toBe("書き出しをやめました。途中までのファイルは消しました。");
    unmount(mounted!);
    show(StatusLine, scene("exportFailed"));
    expect(text(q(".status .message"))).toBe("書き出せませんでした。保存する所に書けません。");
    unmount(mounted!);
    const s = scene("exportFailed");
    s.export.run = { state: "failed", reason: { kind: "plugin", name: "Surge XT" } };
    show(StatusLine, s);
    expect(text(q(".status .message"))).toBe("書き出せませんでした。Surge XT が止まりました。途中までのファイルは消しました。");
  });

  it("% は 1 秒に 2 回まで（22-22 と同じ）。終わった時はすぐ", () => {
    vi.useFakeTimers();
    const s = scene("exporting");
    show(StatusLine, s);
    const percent = () => text(q('.status [data-test="export-percent"]'));
    const running = (p: number): ExportView => ({ dialog: null, run: { ...(s.export.run as Extract<typeof s.export.run, { state: "running" }>), percent: p } });
    app.setState({ ...s, export: running(47) });
    flushSync();
    expect(percent()).toBe("46 %");
    vi.advanceTimersByTime(SHOW_EVERY_MS);
    flushSync();
    expect(percent()).toBe("47 %");
    app.setState({ ...s, export: { dialog: null, run: { state: "cancelled" } } });
    flushSync();
    expect(text(q(".status .message"))).toBe("書き出しをやめました。途中までのファイルは消しました。");
  });
});

describe("ExportShown（見せる数）", () => {
  let now = 1000;
  let view: ExportView | null = null;
  let shown: ExportShown;
  beforeEach(() => {
    vi.useFakeTimers();
    now = 1000;
    shown = new ExportShown(
      () => view,
      () => now,
    );
  });
  afterEach(() => shown.forget());
  const later = (ms: number) => {
    now += ms;
    vi.advanceTimersByTime(ms);
  };
  const counting = (percent: number, over: Partial<ExportDialogView> = {}): ExportView => ({
    dialog: { ...scene("exportCounting").export.dialog!, check: { state: "counting", percent, peakDb: -3, clips: false }, ...over },
    run: null,
  });

  it("たしかめの % だけなら 500 ms に 1 回。ほかの所が変わったらすぐ", () => {
    view = counting(10);
    const first = shown.current;
    later(100);
    view = counting(20);
    expect(shown.current).toBe(first);
    later(100);
    view = counting(30, { format: "flac" });
    expect(shown.current?.dialog?.format).toBe("flac");
    later(100);
    view = counting(40, { format: "flac" });
    expect(shown.current?.dialog?.check).toMatchObject({ percent: 30 });
    later(SHOW_EVERY_MS);
    expect(shown.current?.dialog?.check).toMatchObject({ percent: 40 });
  });
});

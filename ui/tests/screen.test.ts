import { afterEach, describe, expect, it, vi } from "vitest";
import { FakeBands, SCENES, encodeTelemetry, fakeScene, fakeState, unfitScales } from "../src/bridge/fake";
import { decodeTelemetry, keyIsDown } from "../src/bridge/telemetry";
import type { AppState, Command } from "../src/bridge";
import { bandCountsShown, bandTabWidth, transportParts } from "../src/components/fit";
import { PeakHold, dbNumber, msText, peakText, textParts } from "../src/components/format";
import { mock, tokensAt } from "../src/design/tokens";
import { handleKeydown, handlePointerdown } from "../src/keys/attach";
import { hideTip, term, tipVisible } from "../src/lib/tip.svelte";

// docs/ は ui/ の外なので、design-sources.ts と同じく Node の fs で読む。
const fs = (globalThis as unknown as { process: { getBuiltinModule(id: "node:fs"): { readFileSync(p: URL, e: "utf8"): string } } }).process.getBuiltinModule("node:fs");
const here = import.meta.url;
const readMock = (board: string) => fs.readFileSync(new URL(`../../docs/design/mock/${board}.dc.html`, here), "utf8");

describe("再生まわりの引っこめかた（仕様書 1 章 (1)。幅は測った物）: ▾ を残す", () => {
  const w = { base: 800, songName: 144, load: 60, countIn: 227, positionLabels: 20 };
  const stubs = { songName: 40, countIn: 20 };
  const all = w.base + w.songName + w.load + w.countIn + w.positionLabels;
  const at = (room: number) => transportParts(room, w, stubs);

  it("曲の名前は ▾ のぶんだけ、カウントインはメトロノームの ▾ のぶんだけ、へる幅が少ない", () => {
    expect(at(all - 1)).toEqual({ songName: false, load: true, countIn: true, positionLabels: true });
    const afterName = all - w.songName + stubs.songName;
    expect(at(afterName)).toEqual({ songName: false, load: true, countIn: true, positionLabels: true });
    expect(at(afterName - 1)).toEqual({ songName: false, load: false, countIn: true, positionLabels: true });
    const afterLoad = afterName - w.load;
    expect(at(afterLoad - 1)).toEqual({ songName: false, load: false, countIn: false, positionLabels: true });
    expect(at(afterLoad - w.countIn + stubs.countIn)).toEqual({ songName: false, load: false, countIn: false, positionLabels: true });
    expect(at(afterLoad - w.countIn + stubs.countIn - 1)).toEqual({ songName: false, load: false, countIn: false, positionLabels: false });
  });
});

describe("再生まわりの引っこめかた（仕様書 1 章 (1)。幅は測った物）", () => {
  const w = { base: 800, songName: 144, load: 60, countIn: 227, positionLabels: 20 };
  const all = w.base + w.songName + w.load + w.countIn + w.positionLabels;
  const at = (room: number) => transportParts(room, w);

  it("入れば全部出す", () => {
    expect(at(all)).toEqual({ songName: true, load: true, countIn: true, positionLabels: true });
  });

  it("曲の名前、処理の負荷、カウントイン、位置のラベルの順に引っこめる", () => {
    expect(at(all - 1)).toEqual({ songName: false, load: true, countIn: true, positionLabels: true });
    expect(at(all - w.songName - 1)).toEqual({ songName: false, load: false, countIn: true, positionLabels: true });
    expect(at(all - w.songName - w.load - 1)).toEqual({ songName: false, load: false, countIn: false, positionLabels: true });
    expect(at(w.base)).toEqual({ songName: false, load: false, countIn: false, positionLabels: false });
    expect(at(10)).toEqual({ songName: false, load: false, countIn: false, positionLabels: false });
  });
});

describe("帯札の件数（仕様書 3 章: 右の帯の件数から消す）", () => {
  const tabs = [
    bandTabWidth("アレンジ", "トラック 1", "A"),
    bandTabWidth("ピアノロール", "ノート 0", "E"),
    bandTabWidth("ミキサー", "マスター 0.0 dB", "M"),
    bandTabWidth("取り消しの履歴", "0 手", "H"),
  ];
  const full = tabs.reduce((s, t) => s + t.fixed + t.count, 0);

  it("入りきれば全部", () => {
    expect(bandCountsShown(full, 100, tabs)).toEqual([true, true, true, true]);
  });
  it("右から消す。名前とキーの分は消さない", () => {
    expect(bandCountsShown(full - 1, 100, tabs)).toEqual([true, true, true, false]);
    expect(bandCountsShown(full - tabs[3]!.count - 1, 100, tabs)).toEqual([true, true, false, false]);
    expect(bandCountsShown(1, 100, tabs)).toEqual([false, false, false, false]);
  });
  it("右はしの案内の分と拡大率も数える", () => {
    expect(bandCountsShown(full + 100, 100, tabs, 100)).toEqual([true, true, true, true]);
    expect(bandCountsShown(full, 150, tabs)).toContain(false);
  });
});

describe("数の見せかた", () => {
  it("dB は 0.1 で、マイナスは −", () => {
    expect(dbNumber(0)).toBe("0.0");
    expect(dbNumber(-0.01)).toBe("0.0");
    expect(dbNumber(-4.84)).toBe("−4.8");
    expect(peakText(1)).toBe("0.0 dB");
    expect(peakText(10 ** (-8.4 / 20))).toBe("−8.4 dB");
    expect(peakText(10 ** (-89.9 / 20))).toBe("−89.9 dB");
    expect(peakText(10 ** (-90.1 / 20))).toBe("−∞ dB");
    expect(peakText(0)).toBe("−∞ dB");
  });
  it("直近 1 秒のいちばん大きい値", () => {
    const h = new PeakHold(1000);
    h.push(0.5, 0);
    h.push(0.2, 500);
    expect(h.max(900)).toBe(0.5);
    expect(h.max(1200)).toBe(0.2);
    expect(h.max(1600)).toBe(0);
  });
  it("遅れの ms: 20 ms 未満は小数 1 けた、20 ms 以上は整数（仕様書 6 章の「約 53 ms」「約 13.5 ms」）", () => {
    expect(msText(13.3333)).toBe("13.3");
    expect(msText(13.54)).toBe("13.5");
    expect(msText(13)).toBe("13.0");
    expect(msText(5)).toBe("5.0");
    expect(msText(19.96)).toBe("20");
    expect(msText(20)).toBe("20");
    expect(msText(53.3333)).toBe("53");
  });
  it("文の穴", () => {
    expect(textParts("status.bufferMismatch", { actual: 1024, requested: 256 })).toEqual([
      { text: "バッファ ", hole: null },
      { text: "1024", hole: "actual" },
      { text: " ・ 頼んだ数 ", hole: null },
      { text: "256", hole: "requested" },
    ]);
  });
});

describe("mock.*（モックにしかない寸法）", () => {
  // この画面の骨組みが足した物だけを見る（ほかの部品の mock.* は、その部品のテストで）。
  const mine = Object.entries(mock).filter(([k]) => /^mock\.(transport|arrange|track|overview)\./.test(k));
  it("見る物がある", () => expect(mine.length).toBe(12));
  it.each(mine)("%s の値はモックに書いてあり、率をかけた値は整数（四捨五入）", (_, tok) => {
    const html = readMock(tok.board);
    expect(html).toContain(`${tok.value}px`);
    for (const s of [100, 125, 150] as const) expect(Number.isInteger(tokensAt(s, 1)[_] as number)).toBe(true);
  });
});

describe("偽物の場面", () => {
  // crates/meloet-core/src/layout.rs と同じ数字になるか。
  it("窓 800、ピアノロール: 入力計は細く、アレンジ 40、ロールは残りぜんぶ", () => {
    const l = fakeScene("pianoRoll")!.state.workspace.layout;
    expect(l).toMatchObject({ meter: "slim", arrangeHeight: 40, arrangeOverview: true, showBackHint: true });
    expect(l.lower).toEqual([{ band: "pianoRoll", height: 800 - 24 - 48 - 24 - 24 - 24 - 40 }]);
  });
  it("窓 800 のミキサーはアレンジを 40 にして残りぜんぶ。履歴は 248 で、残りはアレンジ", () => {
    expect(fakeScene("mixer")!.state.workspace.layout).toMatchObject({ arrangeHeight: 40, lower: [{ band: "mixer", height: 656 - 40 }] });
    expect(fakeScene("history")!.state.workspace.layout).toMatchObject({ arrangeHeight: 656 - 248, lower: [{ band: "history", height: 248 }] });
  });
  // 仕様書 3 章「縦の分けかたの例」。order は古い順。高さは アレンジ、帯（上から）。
  it.each([
    { order: ["pianoRoll"], fits: 452, grows: 920, rows: 1016, at800: [40, 616], at1080: [200, 736] },
    { order: ["mixer"], fits: 700, grows: 844, rows: 940, at800: [40, 616], at1080: [276, 660] },
    { order: ["history"], fits: 432, grows: 432, rows: 528, at800: [408, 248], at1080: [688, 248] },
    { order: ["mixer", "pianoRoll"], fits: 968, grows: 1580, rows: 1676, at800: null, at1080: [40, 380, 516] },
    { order: ["history", "mixer"], fits: 948, grows: 1092, rows: 1188, at800: null, at1080: [40, 648, 248] },
  ] as const)("縦の分けかたの例: $order", ({ order, fits, grows, rows, at800, at1080 }) => {
    const at = (height: number) => {
      const b = new FakeBands(height);
      for (const band of order) b.toggle(band);
      return b.layout();
    };
    const heights = (l: ReturnType<typeof at>) => [l.arrangeHeight, ...l.lower.map((x) => x.height)];
    expect(at(fits).lower).toHaveLength(order.length);
    expect(at(fits).arrangeHeight).toBe(40);
    if (order.length > 1) expect(at(fits - 1).lower).toHaveLength(1);
    expect(at(grows).arrangeHeight).toBe(40);
    expect(at(grows + 1).arrangeHeight).toBe(41);
    expect(at(rows)).toMatchObject({ arrangeHeight: 136, arrangeOverview: false });
    expect(at(rows - 1).arrangeOverview).toBe(true);
    if (at800) expect(heights(at(800))).toEqual(at800);
    else expect(at(800).lower).toHaveLength(1);
    expect(heights(at(1080))).toEqual(at1080);
    expect(at(1080).meter).toBe("slim");
  });
  it("窓を高くしても、帯の組は変わらず、どの高さもへらない", () => {
    for (const order of [["pianoRoll"], ["mixer", "pianoRoll"], ["pianoRoll", "mixer"], ["history", "mixer"], ["pianoRoll", "history"]] as const) {
      const b = new FakeBands(944);
      for (const band of order) b.toggle(band);
      let last = b.layout();
      for (let h = 945; h <= 2000; h++) {
        expect(b.setHeight(h)).toBeNull();
        const l = b.layout();
        expect(l.lower.map((x) => x.band)).toEqual(last.lower.map((x) => x.band));
        expect(l.arrangeHeight).toBeGreaterThanOrEqual(last.arrangeHeight);
        l.lower.forEach((x, i) => expect(x.height).toBeGreaterThanOrEqual(last.lower[i]!.height));
        last = l;
      }
    }
  });
  it("たたんだ帯と理由を言う。3 つ目の帯で押し出したあと、入りきらずにもう 1 つたたんだら、あとのほう", () => {
    const low = new FakeBands(800);
    expect(low.toggle("mixer")).toBeNull();
    expect(low.toggle("pianoRoll")).toEqual({ band: "mixer", why: "tooLow" });
    const tall = new FakeBands(1080);
    tall.toggle("history");
    tall.toggle("mixer");
    expect(tall.toggle("pianoRoll")).toEqual({ band: "history", why: "third" });
    const mid = new FakeBands(960);
    mid.toggle("history");
    mid.toggle("mixer");
    expect(mid.toggle("pianoRoll")).toEqual({ band: "mixer", why: "tooLow" });
    expect(tall.setHeight(967)).toBe("mixer");
    expect(tall.setHeight(2000)).toBeNull();
    expect(tall.layout().lower.map((x) => x.band)).toEqual(["pianoRoll"]);
  });
  it("入らない拡大率: 画面 1200×700 では 150 だけ。100 はいつも入る", () => {
    expect(unfitScales({ width: 1200, height: 700 })).toEqual([150]);
    expect(unfitScales({ width: 1280, height: 672 })).toEqual([]);
    expect(unfitScales({ width: 800, height: 400 })).toEqual([125, 150]);
    const zoom = fakeScene("zoom")!.state.workspace;
    expect(zoom).toMatchObject({ scale: 125, unfitScales: [150] });
  });
  it("帯がたたまれた場面: ピアノロールだけがひらき、状態行が知らせる", () => {
    const s = fakeScene("bandFolded")!.state;
    expect(s.workspace.layout.lower.map((x) => x.band)).toEqual(["pianoRoll"]);
    expect(s.status.message).toEqual({ id: "bandFoldedToOpen", opened: "pianoRoll", folded: "mixer" });
  });
  it("アレンジだけなら入力計は太く、アレンジが残りぜんぶ", () => {
    expect(fakeScene("empty")!.state.workspace.layout).toMatchObject({ meter: "full", arrangeHeight: 800 - 24 - 48 - 52 - 24 - 24, lower: [] });
  });
  it("拡大率は 100 % の px にしてから決める", () => {
    const l = fakeScene("empty", 125, { width: 1280, height: 800 })!.state.workspace.layout;
    expect(l.arrangeHeight).toBe(640 - 24 - 48 - 52 - 24 - 24);
  });
  it("下の帯は 2 つまで。入りきらなければ古いほうをたたむ", () => {
    const b = new FakeBands(800);
    b.toggle("pianoRoll");
    b.toggle("history");
    expect(b.layout().lower.map((x) => x.band)).toEqual(["pianoRoll", "history"]);
    b.toggle("history");
    b.toggle("mixer");
    expect(b.layout().lower.map((x) => x.band)).toEqual(["mixer"]);
    const tall = new FakeBands(1400);
    tall.toggle("mixer");
    tall.toggle("history");
    tall.toggle("pianoRoll");
    expect(tall.layout().lower.map((x) => x.band)).toEqual(["pianoRoll", "history"]);
  });
  it("右の欄はあとからひらいたほうが前", () => {
    const b = new FakeBands(800);
    b.toggle("shelf");
    b.toggle("params");
    expect(b.layout()).toMatchObject({ rightOpen: ["shelf", "params"], rightFront: "params" });
    b.back();
    expect(b.layout().rightFront).toBe("shelf");
  });
  it("場面はみな作れる", () => {
    for (const name of Object.keys(SCENES)) expect(fakeScene(name), name).not.toBeNull();
    expect(fakeScene("nope")).toBeNull();
    const c2 = fakeScene("connect2")!.state;
    expect(c2.outputs.list.map((o) => o.unusable)).toEqual([
      null,
      null,
      "switchedByPc",
      ...Array(7).fill("stoppedByPc"),
    ]);
    expect(c2.workspace.chooser).toEqual({ highlighted: "out-headphones" });
  });
  it("計器のバイナリは telemetry.ts で読める", () => {
    const m = SCENES.connect3!.meters;
    const t = decodeTelemetry(encodeTelemetry(m, 7, 1234, 1));
    expect([60, 64, 67].every((n) => keyIsDown(t, n))).toBe(true);
    expect(keyIsDown(t, 61)).toBe(false);
    expect(t).toMatchObject({ seq: 7, sentUs: 1234, lastVelocity: 87, sustain: 127, modulation: 12, loadPermille: 60 });
    expect(t.masterPeak[0]).toBeCloseTo(10 ** (-8.4 / 20), 5);
    expect(t.trackPeaks).toHaveLength(2);
  });
});

describe("キーを画面につなぐ", () => {
  let sent: Command[] = [];
  let state: AppState = fakeState();
  const deps = { state: () => state, send: (c: Command) => void sent.push(c), nav: { platform: "Linux x86_64" } };
  const key = (k: string, more: KeyboardEventInit = {}) => {
    const ev = new KeyboardEvent("keydown", { key: k, cancelable: true, ...more });
    handleKeydown(ev, deps);
    return ev;
  };
  afterEach(() => {
    sent = [];
    state = fakeState();
    hideTip();
    document.body.innerHTML = "";
    vi.useRealTimers();
  });

  it("A・E・M・H・P・U と Mod+, と Mod+Alt+数字", () => {
    for (const k of ["a", "e", "m", "h", "p", "u"]) key(k);
    key(",", { ctrlKey: true });
    key("2", { ctrlKey: true, altKey: true, code: "Digit2" });
    expect(sent).toEqual([
      { type: "goArrange" },
      { type: "toggleBand", band: "pianoRoll" },
      { type: "toggleBand", band: "mixer" },
      { type: "toggleBand", band: "history" },
      { type: "toggleBand", band: "shelf" },
      { type: "toggleBand", band: "params" },
      { type: "openSettings", advanced: false },
      { type: "setScale", percent: 125 },
    ]);
  });

  const withOutput = (canRecord: boolean) => {
    state = { ...state, transport: { ...state.transport, canPlay: true, canRecord } };
  };

  it("再生まわりのキー: Space、R、Home、L、C、T、Shift T、Delete", () => {
    withOutput(true);
    const evs = [key(" ", { code: "Space" }), key("r"), key("Home"), key("l"), key("c"), key("t"), key("T", { shiftKey: true }), key("Delete")];
    expect(evs.every((e) => e.defaultPrevented)).toBe(true);
    expect(sent).toEqual([
      { type: "playStop" },
      { type: "record" },
      { type: "goToStart" },
      { type: "toggleLoop" },
      { type: "toggleMetronome" },
      { type: "stepTake", delta: 1 },
      { type: "stepTake", delta: -1 },
      { type: "deleteSelected" },
    ]);
  });

  it("音の出口がまだのあいだ、Space と R は何もしない。ほかは効く", () => {
    for (const k of [" ", "r"]) expect(key(k, { code: k === " " ? "Space" : "KeyR" }).defaultPrevented).toBe(false);
    expect(sent).toEqual([]);
    key("Home");
    key("l");
    key("c");
    expect(sent).toEqual([{ type: "goToStart" }, { type: "toggleLoop" }, { type: "toggleMetronome" }]);
  });

  it("録音待機のトラックがなくても、R は record を送る（Rust が理由を出す）", () => {
    withOutput(false);
    key("r");
    expect(sent).toEqual([{ type: "record" }]);
  });

  it("動いているあいだは、出口がなくなっても Space で止められる", () => {
    state = { ...state, transport: { ...state.transport, state: "playing", canPlay: false } };
    key(" ", { code: "Space" });
    expect(sent).toEqual([{ type: "playStop" }]);
  });

  it("文字を打っているあいだの Home は、先頭へ戻り、枠の中のカーソルも動かす", () => {
    const input = document.createElement("input");
    document.body.append(input);
    input.focus();
    expect(key("Home").defaultPrevented).toBe(false);
    key(" ", { code: "Space" });
    key("Delete");
    expect(sent).toEqual([{ type: "goToStart" }]);
  });

  it("文字を打っているあいだ、一文字のキーは効かない", () => {
    const input = document.createElement("input");
    document.body.append(input);
    input.focus();
    key("m");
    expect(sent).toEqual([]);
  });

  it("Esc: 文字の枠 → 用語の説明 → Rust の back", () => {
    vi.useFakeTimers();
    const span = document.createElement("span");
    const input = document.createElement("input");
    document.body.append(span, input);
    term(span, "ループ");
    span.dispatchEvent(new PointerEvent("pointerenter"));
    vi.advanceTimersByTime(600);
    input.focus();
    expect(tipVisible()).toBe(true);

    key("Escape");
    expect(document.activeElement).not.toBe(input);
    expect(tipVisible()).toBe(true);
    key("Escape");
    expect(tipVisible()).toBe(false);
    expect(sent).toEqual([]);
    key("Escape");
    expect(sent).toEqual([{ type: "back" }]);
  });

  it("出口えらびでは ↑ ↓ Enter、音の設定では Enter", () => {
    state = { ...state, workspace: { ...state.workspace, chooser: { highlighted: null } } };
    key("ArrowDown");
    key("ArrowUp");
    key("Enter");
    state = fakeState();
    state = { ...state, workspace: { ...state.workspace, settings: { draftOutput: null, advancedOpen: false } } };
    key("Enter");
    expect(sent).toEqual([
      { type: "chooserMove", delta: 1 },
      { type: "chooserMove", delta: -1 },
      { type: "chooserConfirm" },
      { type: "settingsApply" },
    ]);
  });

  it("「音が出ました」は、キーの操作やマウスのボタンで消える", () => {
    state = { ...state, workspace: { ...state.workspace, hint: "soundCameOut" } };
    key("Shift");
    expect(sent).toEqual([]);
    key("x");
    handlePointerdown(deps);
    expect(sent).toEqual([{ type: "dismissHint" }, { type: "dismissHint" }]);
  });
});

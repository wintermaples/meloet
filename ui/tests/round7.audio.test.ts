// 第 7 回 区切り3 の 2 回目（画面の HTML）: 160 の行、フェードの取っ手と音量の線、選んだクリップの欄、見つからない音と読みこんでいる途中の文、
// 音のファイルを持ってきている間（入る所の枠、分かれた箱、落としたときの DropAudio）、メニュー、S、落ちたあとの歌の行。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeScene, type FakeState } from "../src/bridge/fake";
import { audioDrag } from "../src/components/arrange/audioDrag.svelte";
import AudioClipHandles from "../src/components/arrange/AudioClipHandles.svelte";
import AudioClipNotes from "../src/components/arrange/AudioClipNotes.svelte";
import { fileDrag, heardFileEvent, setDropResolver } from "../src/components/arrange/fileDrag.svelte";
import { audioShapeOf, fadeHandleBox, handleRects, waveArea, gainLineY } from "../src/components/arrange/audioClipGeom";
import { clipBox, type LaneDims, type View } from "../src/components/arrangeGeom";
import type { GlRect } from "../src/gl/slot";
import { arrangeRows } from "../src/components/automation/autoGeom";
import FileDropNotice from "../src/components/FileDropNotice.svelte";
import { topMenus } from "../src/components/menu/menus";
import type { ListRow } from "../src/components/menu/menus";
import { clipLengthLabel, fadeLengthText, parseClipLength, parseClipPosition } from "../src/components/params/clipFields";
import ParamsPanel from "../src/components/params/ParamsPanel.svelte";
import { cancelDrag, dragReading } from "../src/components/pianoroll/view.svelte";
import RecoveryHint from "../src/components/RecoveryHint.svelte";
import TrackHeads from "../src/components/TrackHeads.svelte";
import { trackContextMenu } from "../src/components/trackMenu";
import { keyCommand } from "../src/keys/attach";
import { keyAction, type KeyInput } from "../src/keys/keys";
import { t } from "../src/text/ja";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  audioDrag.now = null;
  dragReading.text = null;
  fileDrag.at = null;
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
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
const BAR = 4 * TPQ;

function pointer(el: Element, type: string, clientX: number, clientY: number): void {
  el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX, clientY, button: 0 }));
  flushSync();
}

describe("選んだクリップの欄（仕様書 3 章 93-4、モック F7-AudioEdit）", () => {
  it("名前と「選んだクリップ」、ファイルの行、クリップの 4 行、のび縮みの 4 行", () => {
    show(ParamsPanel, scene("audioEdit"), { onfold: () => {} });
    expect(text(q('[data-test="params-name"]'))).toBe("歌 テイク 1");
    expect(text(q('[data-test="clip-params"]'))).toContain("選んだクリップ");
    expect(text(q('[data-test="clip-file"]'))).toBe("6 歌 ・ 歌 テイク 1.wav ・ 48000 Hz ・ 24 ビット");
    expect(text(q('[data-test="clip-value-start"]'))).toBe("7. 1. 0000");
    expect(text(q('[data-test="clip-value-length"]'))).toBe("2. 0. 0000");
    expect(text(q('[data-test="clip-value-gainDb"]'))).toBe("−3.0 dB");
    // モックの「前 0.120 ・ 後 0.000」を 4 けたのティックで（B-94-28）。
    expect(text(q('[data-test="clip-value-fade"]'))).toBe("前 0.0480 ・ 後 0.0000");
    const stretch = ["follow", "source-tempo", "pitch", "keepFormant"].map((k) => text(q(`[data-test="clip-${k}"]`)));
    expect(stretch).toEqual(["曲のテンポについていく オン", "この音のテンポ 96.00", "音の高さ +0 半音 0 セント", "声の感じを保つ オン"]);
    expect(qa(".row.unusable")).toEqual([]);
  });

  it("サンプルレートがちがうファイルは「44100 Hz（曲に合わせて鳴らします）」、ビットのない形は出さない", () => {
    const s = scene("audioLoading");
    s.params.selected = { kind: "clip", track: vocalRow(s).vocal.id, clip: 70, missing: null };
    show(ParamsPanel, s, { onfold: () => {} });
    expect(text(q('[data-test="clip-file"]'))).toBe("6 歌 ・ ドラム.mp3 ・ 44100 Hz（曲に合わせて鳴らします）");
    expect(text(q('[data-test="clip-source-tempo"]'))).toBe("この音のテンポ さがす —");
  });

  it("値を 2 回押して打つと SetClipField を 1 つ。読めない位置は RejectValue", () => {
    show(ParamsPanel, scene("audioEdit"), { onfold: () => {} });
    const type = (field: string, value: string) => {
      q(`[data-test="clip-value-${field}"]`)!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
      flushSync();
      const input = q<HTMLInputElement>('[data-test="param-input"]')!;
      input.value = value;
      input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
      flushSync();
    };
    type("gainDb", "−6");
    type("fadeOut", "1.0000");
    type("start", "8. 1. 0000");
    type("length", "1.2");
    type("start", "あ");
    expect(app.sent).toEqual([
      { type: "setClipField", clip: 72, field: "gainDb", value: -6 },
      { type: "setClipField", clip: 72, field: "fadeOut", value: TPQ },
      { type: "setClipField", clip: 72, field: "start", value: 7 * BAR },
      { type: "setClipField", clip: 72, field: "length", value: BAR + 2 * TPQ },
      { type: "rejectValue", field: "position", kept: "7. 1. 0000", range: null },
    ]);
  });

  it("見つからない音: 知らせの文と「さがす…」「曲から外す」（モック F7-AudioMissing）", () => {
    show(ParamsPanel, scene("audioMissing"), { onfold: () => {} });
    expect(text(q('[data-test="clip-missing"]'))).toBe(
      "歌 テイク 1.wav が見つかりません。前はここにありました: はじめての曲/音/歌 テイク 1.wav さがす… 曲から外す",
    );
    expect(q('[data-test="clip-value-start"]')).toBeNull();
    q('[data-test="clip-find"]')!.click();
    q('[data-test="clip-remove"]')!.click();
    expect(app.sent).toEqual([
      { type: "findAudio", audio: 64 },
      { type: "removeMissing", missing: { kind: "audio", audio: 64 } },
    ]);
  });
});

describe("欄の字", () => {
  const meter = { num: 4, den: 4 };
  it("長さは「2. 0. 0000」、フェードは「0.0480」。打った字は空白も全角も読む", () => {
    expect(clipLengthLabel(2 * BAR, meter)).toBe("2. 0. 0000");
    expect(clipLengthLabel(BAR + TPQ + 5, meter)).toBe("1. 1. 0005");
    expect(fadeLengthText(TPQ / 4, meter)).toBe("0.0480");
    expect(parseClipLength("2. 0. 0000", meter)).toBe(2 * BAR);
    expect(parseClipLength("１.３", meter)).toBe(BAR + 3 * TPQ);
    expect(parseClipLength("1.4", meter)).toBeNull();
    expect(parseClipLength("0.0480", meter, true)).toBe(480);
    expect(parseClipPosition("7. 1. 0000", meter)).toBe(6 * BAR);
    expect(parseClipPosition("x", meter)).toBeNull();
  });
});

const dims: LaneDims = { trackH: 64, inset: 3, hair: 1, mark: 2, nameH: 16, pad: 4, noteH: 2 };
const view: View = { pxPerTick: 64 / BAR, scrollX: 0, width: 2000 };
/** audioEdit の 6 本目（歌）だけを 1 行にした形。 */
function vocalRow(s: AppState) {
  const vocal = s.song.tracks.find((tr) => tr.kind.kind === "vocal")!;
  return { vocal, tracks: [vocal] };
}

describe("フェードの取っ手と音量の線（仕様書 3 章 27-A ・「クリップの音量」）", () => {
  it("どの歌のクリップにも左右の取っ手（絵は WebGL の四角）。音量の線は選んだクリップだけ。クリップごとの HTML は置かない", () => {
    const s = scene("audioEdit");
    const { tracks } = vocalRow(s);
    show(AudioClipHandles, s, { view, dims, tracks, selected: new Set([72]), busy: false });
    expect(qa('[data-test="clip-gain-line"]')).toHaveLength(1);
    expect(qa('[data-test="clip-fade-grip"]')).toHaveLength(0);
    const rects: GlRect[] = [];
    for (const clip of tracks[0]!.clips) handleRects(rects, view, dims, clipBox(view, dims, 0, clip.start, clip.end), audioShapeOf(clip)!, 6);
    expect(rects.map((r) => [r.w, r.h, r.color])).toEqual(Array(4).fill([6, 6, "color.gray.10"]));
    // フェードがある側の取っ手は、フェードの切れ目がまん中。ない側は、クリップの角。
    const right = tracks[0]!.clips[1]!;
    const box = clipBox(view, dims, 0, right.start, right.end);
    expect(rects[2]!.x + 3).toBeCloseTo((right.start + right.audio!.fadeIn) * view.pxPerTick);
    expect(rects[3]!.x + 6).toBeCloseTo(waveArea(dims, box).x1);
  });

  it("ポインタが取っ手の上に来たら、その 1 つだけ HTML（カーソルの形）。はなれたら消える。録音中は出ない", () => {
    const s = scene("audioEdit");
    const { tracks } = vocalRow(s);
    show(AudioClipHandles, s, { view, dims, tracks, selected: new Set(), busy: false });
    const clip = tracks[0]!.clips[1]!;
    const b = fadeHandleBox(view, dims, clipBox(view, dims, 0, clip.start, clip.end), audioShapeOf(clip)!, "fadeOut", 6);
    pointer(target, "pointermove", b.x0 + 3, b.y0 + 3);
    const grip = q('[data-test="clip-fade-grip"]')!;
    expect([parseFloat(grip.style.left), parseFloat(grip.style.top), parseFloat(grip.style.width)]).toEqual([b.x0, b.y0, 6]);
    pointer(target, "pointermove", b.x0 - 20, b.y0 + 3);
    expect(q('[data-test="clip-fade-grip"]')).toBeNull();
    unmount(mounted!);
    show(AudioClipHandles, s, { view, dims, tracks, selected: new Set(), busy: true });
    pointer(target, "pointermove", b.x0 + 3, b.y0 + 3);
    expect(q('[data-test="clip-fade-grip"]')).toBeNull();
    pointer(target, "pointerdown", b.x0 + 3, b.y0 + 3);
    pointer(target, "pointermove", b.x0 - 20, b.y0 + 3);
    expect(audioDrag.now).toBeNull();
  });

  it("線を上下にドラッグ: 画面だけが先に動き（audioDrag.now）、離して SetClipField を 1 つ。状態行は「… −3.0 dB（ドラッグ中）…」", () => {
    const s = scene("audioEdit");
    const { tracks } = vocalRow(s);
    show(AudioClipHandles, s, { view, dims, tracks, selected: new Set([72]), busy: false });
    const clip = tracks[0]!.clips[1]!;
    const area = waveArea(dims, clipBox(view, dims, 0, clip.start, clip.end));
    const y = gainLineY(area, -3);
    const line = q('[data-test="clip-gain-line"]')!;
    pointer(line, "pointerdown", 450, y);
    pointer(line, "pointermove", 450, area.bottom + 10);
    expect(audioDrag.now).toEqual({ clip: 72, gainDb: -60 });
    expect(dragReading.text).toBe(t("msg.clipGainDragging", { clip: "歌 テイク 1", value: "−60.0 dB", cancelKey: "Esc" }));
    expect(text(q('[data-test="clip-gain-reading"]'))).toBe("−60.0 dB");
    pointer(line, "pointerup", 450, area.bottom + 10);
    expect(app.sent).toEqual([{ type: "setClipField", clip: 72, field: "gainDb", value: -60 }]);
    expect(dragReading.text).toBeNull();
  });

  it("Esc はドラッグをやめて、何も送らない。2 回押すと 0.0 dB", () => {
    const s = scene("audioEdit");
    const { tracks } = vocalRow(s);
    show(AudioClipHandles, s, { view, dims, tracks, selected: new Set([72]), busy: false });
    const line = q('[data-test="clip-gain-line"]')!;
    pointer(line, "pointerdown", 450, 20);
    pointer(line, "pointermove", 450, 40);
    expect(audioDrag.now).not.toBeNull();
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(audioDrag.now).toBeNull();
    pointer(line, "pointerup", 450, 40);
    expect(app.sent).toEqual([]);
    line.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    expect(app.sent).toEqual([{ type: "setClipField", clip: 72, field: "gainDb", value: 0 }]);
  });

  it("取っ手を横にひっぱるとフェード（ティック）。もとの位置に gray.7 の枠", () => {
    const s = scene("audioEdit");
    const { tracks } = vocalRow(s);
    show(AudioClipHandles, s, { view, dims, tracks, selected: new Set(), busy: false });
    const clip = tracks[0]!.clips[1]!;
    const b = fadeHandleBox(view, dims, clipBox(view, dims, 0, clip.start, clip.end), audioShapeOf(clip)!, "fadeIn", 6);
    const x0 = b.x0 + 3;
    const y = b.y0 + 3;
    // 押した出来事は、クリップの面（親）のほかの係には届かない。
    const reached: string[] = [];
    target.addEventListener("pointerdown", () => reached.push("down"));
    pointer(target, "pointerdown", x0, y);
    expect(reached).toEqual([]);
    pointer(target, "pointermove", x0 + 16, y);
    expect(audioDrag.now).toEqual({ clip: 72, fadeIn: TPQ / 4 + TPQ });
    expect(q(".origin-handle")).not.toBeNull();
    // 状態行とその場の読み値（仕様書 6 章「フェードを動かしている」）。
    expect(dragReading.text).toBe("歌 テイク 1 の前のフェード 0. 1. 0480（ドラッグ中）・ 離すと決まります ・ やめるには Esc");
    expect(text(q('[data-test="clip-gain-reading"]'))).toBe("0. 1. 0480");
    // ひっぱっている間、HTML の取っ手は値についていく。
    expect(parseFloat(q('[data-test="clip-fade-grip"]')!.style.left)).toBeCloseTo(b.x0 + 16);
    pointer(target, "pointerup", x0 + 16, y);
    expect(app.sent).toEqual([{ type: "setClipField", clip: 72, field: "fadeIn", value: TPQ / 4 + TPQ }]);
    expect(dragReading.text).toBeNull();
    // 取っ手のない所を押しても、何もしない（クリップの面の係に届く）。
    pointer(target, "pointerdown", x0 + 100, y + 20);
    expect(reached).toEqual(["down"]);
  });

  it("録音中は取っ手も線も押せない", () => {
    const s = scene("audioEdit");
    const { tracks } = vocalRow(s);
    show(AudioClipHandles, s, { view, dims, tracks, selected: new Set([72]), busy: true });
    pointer(q('[data-test="clip-gain-line"]')!, "pointerdown", 450, 20);
    pointer(q('[data-test="clip-gain-line"]')!, "pointermove", 450, 40);
    expect(audioDrag.now).toBeNull();
  });
});

describe("歌のクリップの中の文", () => {
  it("見つからない音は「音のファイルが見つかりません」、読みこんでいる途中は「読みこんでいます 40 %」", () => {
    const missing = scene("audioMissing");
    show(AudioClipNotes, missing, { view, dims, tracks: vocalRow(missing).tracks });
    expect(qa('[data-test="audio-clip-missing"]').map(text)).toEqual(["音のファイルが見つかりません", "音のファイルが見つかりません"]);
    unmount(mounted!);
    const loading = scene("audioLoading");
    show(AudioClipNotes, loading, { view, dims, tracks: vocalRow(loading).tracks });
    expect(qa('[data-test="audio-clip-loading"]').map(text)).toEqual(["読みこんでいます 40 %"]);
  });
});

describe("160 の行（仕様書 3 章「歌のトラックでピアノロールをひらこうとしたとき」）", () => {
  it("tallTrack の行だけ高く、下の行と帯はその分さがる", () => {
    const s = scene("audioTall");
    const rows = arrangeRows(s.song.tracks, [], { trackH: 64, laneH: 48, rowH: 24, tall: { track: s.arrange.tallTrack, h: 160 } });
    const at = s.song.tracks.findIndex((tr) => tr.id === s.arrange.tallTrack);
    expect(rows.heights[at]).toBe(160);
    expect(rows.heights.filter((h) => h === 64)).toHaveLength(s.song.tracks.length - 1);
    expect(rows.height - arrangeRows(s.song.tracks, [], { trackH: 64, laneH: 48, rowH: 24 }).height).toBe(96);
  });

  it("見出しも同じ高さ", () => {
    show(TrackHeads, scene("audioTall"));
    const heads = qa('[data-test="track-head"]');
    expect(heads[5]!.style.height).toBe("160px");
    expect(heads[0]!.style.height).toBe("");
  });
});

describe("音のファイルを持ってきている間（仕様書 3 章 29-A ・ 30-C、モック F7-AudioDrop）", () => {
  it("2 つ以上: 箱を左右に分け、ポインタのいる側を「乗っている」に", () => {
    const s = scene("audioDrop");
    app.setState(s);
    window.innerWidth = 1280;
    fileDrag.at = { x: 520, y: 385 };
    mounted = mount(FileDropNotice, { target, props: { drop: s.workspace.fileDrop! } });
    flushSync();
    expect(text(q('[data-test="file-drop-separate"]'))).toBe("別々のトラックに 8 つのファイルを 8 本の歌のトラックにして、落とした位置から入れます。");
    expect(text(q('[data-test="file-drop-in-row"]'))).toBe("1 本にならべる 落とした所のトラックに、順にうしろへ。");
    expect(q('[data-test="file-drop-separate"]')!.classList.contains("on")).toBe(true);
    fileDrag.at = { x: 760, y: 385 };
    flushSync();
    expect(q('[data-test="file-drop-in-row"]')!.classList.contains("on")).toBe(true);
    expect(q('[data-test="file-drop-separate"]')!.classList.contains("on")).toBe(false);
  });

  it("SF2 とまぜたら「SF2 は 1 つ棚へ ・ 音のファイル 3 つはアレンジへ」", () => {
    const s = scene("audioDropMixed");
    show(FileDropNotice, s, { drop: s.workspace.fileDrop });
    expect(text(q('[data-test="file-drop-mixed"]'))).toBe("SF2 は 1 つ棚へ ・ 音のファイル 3 つはアレンジへ");
  });

  it("1 つ: 名前、入る所（6 章「音のファイルを 1 つ持ってきている」）、やめかた。SF2 の「足せる」文は出さない", () => {
    const s = scene("audioDropOne");
    fileDrag.at = null;
    show(FileDropNotice, s, { drop: s.workspace.fileDrop });
    expect(text(q('[data-test="file-drop"]'))).toBe(`ドラム.wav ${t("sf2.dropCancel")}`);
    const stop = setDropResolver((_x, y) => ({ track: y < 300 ? 6 : null, tick: 0 }));
    try {
      fileDrag.at = { x: 400, y: 200 };
      flushSync();
      expect(text(q('[data-test="file-drop"]'))).toBe(`ドラム.wav ここに落とすと、そのトラックの落とした位置に入れます。 ${t("sf2.dropCancel")}`);
      fileDrag.at = { x: 400, y: 500 };
      flushSync();
      expect(text(q('[data-test="file-drop-audio-say"]'))).toBe("新しい歌のトラックに入れます。");
    } finally {
      stop();
      fileDrag.at = null;
    }
  });

  it("fileDrag で場所を覚え、fileDrop で DropAudio を 1 つ（左右は窓の半分、1 つなら落とした所のトラックへ）", () => {
    const s = scene("audioDrop");
    app.setState(s);
    window.innerWidth = 1280;
    const stop = setDropResolver((x, _y, mode) => ({ track: mode === "separate" ? null : 6, tick: x * 10 }));
    heardFileEvent({ type: "fileDrag", x: 520, y: 385 });
    expect(fileDrag.at).toEqual({ x: 520, y: 385 });
    heardFileEvent({ type: "fileDrop", drop: 3, x: 520, y: 385 });
    heardFileEvent({ type: "fileDrop", drop: 4, x: 900, y: 385 });
    app.setState(scene("audioDropOne"));
    heardFileEvent({ type: "fileDrop", drop: 5, x: 100, y: 385 });
    stop();
    expect(app.sent).toEqual([
      { type: "dropAudio", drop: 3, track: null, tick: 5200, mode: "separate" },
      { type: "dropAudio", drop: 4, track: 6, tick: 9000, mode: "inRow" },
      { type: "dropAudio", drop: 5, track: 6, tick: 1000, mode: "inRow" },
    ]);
    expect(fileDrag.at).toBeNull();
  });
});

describe("メニュー ・ 右クリック ・ S", () => {
  it("ファイル ▸ 音のファイルを入れる…: 選んだ歌のトラックへ（鍵盤のトラックなら null）", () => {
    const s = scene("audioEdit");
    const deps = { state: () => s, send: (c: Command) => void app.sent.push(c), playTick: () => 0 };
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const row = topMenus(s, deps as any, "other")[0]!.entries.find((e): e is ListRow => e.kind === "row" && e.label === "音のファイルを入れる…")!;
    expect(row.usable).toBe(true);
    row.run!();
    s.song.playTarget = s.song.tracks[0]!.id;
    row.run!();
    expect(app.sent.slice(-1)).toEqual([{ type: "importAudio", track: null }]);
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const fresh = topMenus(scene("audioEdit"), deps as any, "other")[0]!.entries.find((e): e is ListRow => e.kind === "row" && e.label === "音のファイルを入れる…")!;
    fresh.run!();
    expect(app.sent.slice(-1)).toEqual([{ type: "importAudio", track: vocalRow(s).vocal.id }]);
  });

  it("歌のトラックの右クリックだけに「音のファイルを入れる…」", () => {
    const s = scene("audioEdit");
    const send = (c: Command) => void app.sent.push(c);
    const vocal = s.song.tracks.find((tr) => tr.kind.kind === "vocal")!;
    const rows = trackContextMenu(s, vocal, false, send, (l) => l).filter((e) => e.kind === "row");
    const row = rows.find((e) => e.kind === "row" && e.label === "menu.importAudio")!;
    expect(row).toBeDefined();
    if (row.kind === "row") row.run();
    expect(app.sent).toEqual([{ type: "importAudio", track: vocal.id }]);
    const keys = trackContextMenu(s, s.song.tracks[0]!, false, send, (l) => l);
    expect(keys.some((e) => e.kind === "row" && e.label === "menu.importAudio")).toBe(false);
  });

  it("S は splitClips。打っている最中と録音中は効かない", () => {
    const k = (key: string): KeyInput => ({ key, ctrl: false, meta: false, alt: false, shift: false });
    expect(keyAction(k("s"), false, "other")).toEqual({ kind: "splitClips" });
    expect(keyAction(k("s"), true, "other")).toBeNull();
    const s = scene("audioEdit");
    const deps = { state: () => s, send: () => {}, playTick: () => 0 };
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect(keyCommand({ kind: "splitClips" }, s, deps as any)).toEqual({ type: "splitClips" });
    s.transport.state = "recording";
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    expect(keyCommand({ kind: "splitClips" }, s, deps as any)).toBeNull();
  });
});

describe("落ちたあとの案内の歌の行（区切り2 の測りで見つけた事 2）", () => {
  it("「録っていた歌 1 つを、止まる直前（9. 3. 0000 ごろ）まで残しました。」", () => {
    show(RecoveryHint, scene("recoverVocal"));
    expect(text(q('[data-test="recovery-vocals"]'))).toBe("録っていた歌 1 つを、止まる直前（9. 3. 0000 ごろ）まで残しました。");
  });

  it("歌がなければ出さない", () => {
    show(RecoveryHint, scene("recover1"));
    expect(q('[data-test="recovery-vocals"]')).toBeNull();
  });
});


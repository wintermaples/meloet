// ドラムのトラックのピアノロール（モック R7-Drum）を mount して見る: 鍵の列の楽器の名前、スケール固定は「使えない」、
// ノートの一覧の列、右の欄、状態行と入力計の音の名前。ドラムでないトラックに切りかえると、もとのピアノロールに戻る。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, ClipView, TakeNotes, TrackView } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import InputMeter from "../src/components/InputMeter.svelte";
import NoteInspector from "../src/components/pianoroll/NoteInspector.svelte";
import NoteListPanel from "../src/components/pianoroll/NoteListPanel.svelte";
import Roll from "../src/components/pianoroll/Roll.svelte";
import Toolbar from "../src/components/pianoroll/Toolbar.svelte";
import { drumView, prPanels, prView } from "../src/components/pianoroll/view.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import * as app from "./fake-app.svelte";
import * as shown from "./fake-shown.svelte";
import { take } from "./pianoroll-fixture";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

// jsdom は大きさを測らない。ノートの場所をモック R7-Drum の 668 × 440 にする（行の高さ 36）。
const proto = HTMLElement.prototype;
const hadW = Object.getOwnPropertyDescriptor(proto, "clientWidth");
const hadH = Object.getOwnPropertyDescriptor(proto, "clientHeight");
beforeAll(() => {
  Object.defineProperty(proto, "clientWidth", { configurable: true, get: () => 668 });
  Object.defineProperty(proto, "clientHeight", { configurable: true, get: () => 440 });
});
afterAll(() => {
  if (hadW) Object.defineProperty(proto, "clientWidth", hadW);
  if (hadH) Object.defineProperty(proto, "clientHeight", hadH);
});

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  prPanels.list = true;
  prPanels.inspector = true;
  prView.tool = "select";
  prView.shownClip = null;
  prView.drums = false;
  drumView.rowPx = null;
  drumView.topRow = 0;
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
});

const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const qa = (sel: string) => [...target.querySelectorAll<HTMLElement>(sel)];
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

interface Song {
  s: AppState;
  clip: ClipView;
  track: TrackView;
  take: TakeNotes;
}

/** モック R7-Drum の曲。見せているのは「3 ドラム」の「8ビート」。 */
async function drumSong(edit?: (s: AppState) => void): Promise<Song> {
  const s = fakeScene("drumRoll")!.state;
  edit?.(s);
  const track = s.song.tracks.find((x) => x.clips.some((c) => c.id === s.arrange.pianoRollClip))!;
  const clip = track.clips.find((c) => c.id === s.arrange.pianoRollClip)!;
  const t = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
  app.setState(s);
  shown.setShown({ clip, track, take: t });
  return { s, clip, track, take: t };
}

function show(component: Component): void {
  mounted = mount(component, { target });
  flushSync();
}

describe("鍵の列（モック R7-Drum）", () => {
  it("12 の音の名前を上から。叩いている行は明るく、使っていない行も出す。幅は part.drum.keys の列", async () => {
    await drumSong();
    show(Roll);
    expect(q(".roll")!.classList.contains("drums")).toBe(true);
    const names = qa("[data-test=pr-drum-names] .drum-name");
    expect(names.map(text)).toEqual([
      "クラッシュ",
      "ライド",
      "オープンハイハット",
      "ペダルハイハット",
      "クローズハイハット",
      "ハイタム",
      "ミッドタム",
      "ロータム",
      "クラップ",
      "スネア",
      "サイドスティック",
      "キック",
    ]);
    const used = names.filter((n) => n.classList.contains("used")).map(text);
    expect(used).toEqual(["クラッシュ", "クローズハイハット", "ハイタム", "ミッドタム", "ロータム", "スネア", "キック"]);
    expect(names.map((n) => n.style.height)).toEqual(Array(12).fill("36px"));
    expect(names[11]!.style.top).toBe(`${11 * 36}px`);
    // 用語の説明は名前の中の言葉にだけ（「ハイタム」の「タム」）。
    expect(text(names[5]!.querySelector("span"))).toBe("タム");
    expect(q(".key-names")).toBeNull();
  });

  it("ドラムでないトラックのクリップに切りかえると、もとの鍵盤の列と行の高さに戻る", async () => {
    const { s } = await drumSong();
    show(Roll);
    expect(prView.drums).toBe(true);
    const piano = s.song.tracks[0]!;
    const clip = piano.clips[0]!;
    const next = structuredClone(s);
    next.arrange = { ...next.arrange, pianoRollClip: clip.id, selectedClip: clip.id, selectedClips: [clip.id] };
    app.setState(next);
    shown.setShown({ clip, track: piano, take: take([[0, 480, 60]], clip.takeId) });
    flushSync();
    expect(prView.drums).toBe(false);
    expect(q(".roll")!.classList.contains("drums")).toBe(false);
    expect(q("[data-test=pr-drum-names]")).toBeNull();
    expect(q(".key-names")).not.toBeNull();
  });

  it("縦の − ＋ はドラムの行の高さを 4 ずつ（ふつうの行の高さは変えない）", async () => {
    await drumSong();
    show(Roll);
    const rowPx = prView.rowPx;
    q("[data-test=pr-row-zoom-in]")!.click();
    flushSync();
    expect(drumView.rowPx).toBe(40);
    expect(qa(".drum-name")[0]!.style.height).toBe("40px");
    expect(prView.rowPx).toBe(rowPx);
  });
});

describe("道具の列のスケール固定", () => {
  it("「使えない」で「スケール固定 —」。押しても何も送らない", async () => {
    await drumSong();
    show(Toolbar);
    const lock = q("[data-test=pr-scale-lock]")!;
    expect(text(lock)).toBe("スケール固定 —");
    expect(lock.getAttribute("aria-disabled")).toBe("true");
    expect(q("[data-test=pr-scale-menu]")!.getAttribute("aria-disabled")).toBe("true");
    lock.click();
    q("[data-test=pr-scale-menu]")!.click();
    flushSync();
    expect(app.sent).toEqual([]);
  });
});

describe("ノートの一覧", () => {
  it("「長さ」の列を出さない: 位置 ・ 音（楽器の名前）・ Vel", async () => {
    await drumSong();
    show(NoteListPanel);
    expect(text(q(".head"))).toBe("位置 音 Vel");
    expect(q(".head")!.classList.contains("drums")).toBe(true);
    expect(qa(".row").slice(0, 3).map(text)).toEqual(["1.1.000 キック 104", "1.1.000 クローズハイハット 88", "1.1.000 クラッシュ 110"]);
    expect(qa(".row.selected").map(text)).toEqual(["1.2.000 スネア 96", "2.2.000 スネア 96"]);
  });

  it("キットにない鍵のノートは音名で", async () => {
    const song = await drumSong();
    const odd = take([[0, 120, 60, 90], [0, 120, 36, 104]], song.clip.takeId);
    shown.setShown({ clip: song.clip, track: song.track, take: odd });
    show(NoteListPanel);
    expect(qa(".row").map(text)).toEqual(["1.1.000 C4 90", "1.1.000 キック 104"]);
  });
});

describe("右の欄（選んだノート）", () => {
  it("音は楽器の名前。スケールは音階「—」、固定「使えない」", async () => {
    await drumSong();
    show(NoteInspector);
    expect(text(q("[data-test=inspector-keys]"))).toBe("スネア スネア");
    expect(text(q("[data-test=inspector-scale]"))).toBe("—");
    expect(text(q("[data-test=inspector-lock]"))).toBe("使えない");
    expect(q("[data-test=inspector-lock]")!.tagName).toBe("SPAN");
  });
});

describe("状態行と入力計", () => {
  it("選んだノートの音名は楽器の名前", async () => {
    await drumSong();
    show(StatusLine);
    expect(text(q(".status"))).toContain("選択 2 ノート ・ スネア スネア");
  });

  it("入力計の「いま押している音」: 弾くと鳴る先がドラムなら楽器の名前（「キック スネア」）", async () => {
    await drumSong();
    show(InputMeter);
    app.setTelemetry(app.telemetryWith({ keys: [38, 36] }));
    flushSync();
    expect(text(q('[data-test="held"]'))).toBe("キック スネア");
  });

  it("鳴る先がドラムでなければ音名", async () => {
    await drumSong((s) => (s.song.playTarget = s.song.tracks[0]!.id));
    show(InputMeter);
    app.setTelemetry(app.telemetryWith({ keys: [38, 36] }));
    flushSync();
    expect(text(q('[data-test="held"]'))).toBe("C2 D2");
  });
});

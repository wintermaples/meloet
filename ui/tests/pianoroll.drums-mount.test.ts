// Mounts the piano roll of a drum track (mock R7-Drum) and checks: the instrument names in the key column, scale lock disabled,
// the note list columns, the right panel, and the sound names in the status line and input meter. Switching to a non-drum track goes back to the normal piano roll.
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
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { take } from "./pianoroll-fixture";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

// jsdom does not measure sizes. The note area is 668 × 440 as in mock R7-Drum (row height 36).
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

/** The song of mock R7-Drum. It shows 「8ビート」 of 「3 ドラム」. */
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

describe("key column (mock R7-Drum)", () => {
  it("12 sound names from the top. Rows being hit are bright, unused rows are shown too. The width is the part.drum.keys column", async () => {
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
      "ミッドタム 2",
      "ロータム",
      "クラップ",
      "スネア",
      "サイドスティック",
      "キック",
    ]);
    const used = names.filter((n) => n.classList.contains("used")).map(text);
    expect(used).toEqual(["クラッシュ", "クローズハイハット", "ハイタム", "ミッドタム 2", "ロータム", "スネア", "キック"]);
    expect(names.map((n) => n.style.height)).toEqual(Array(12).fill("36px"));
    expect(names[11]!.style.top).toBe(`${11 * 36}px`);
    // term tips are only on words inside the name (「タム」 in 「ハイタム」).
    expect(text(names[5]!.querySelector("span"))).toBe("タム");
    expect(q(".key-names")).toBeNull();
  });

  it("switching to a clip on a non-drum track goes back to the keyboard column and row height", async () => {
    const { s } = await drumSong();
    show(Roll);
    expect(prView.drums).toBe(true);
    const piano = s.song.tracks[0]!;
    const clip = piano.clips[0]!;
    const next = structuredClone(s);
    next.arrange = { ...next.arrange, pianoRollClip: clip.id, selectedClip: clip.id, selectedClips: [clip.id] };
    app.setState(next);
    shown.setShown({ clip, track: piano, take: take([[0, TPQ, 60]], clip.takeId) });
    flushSync();
    expect(prView.drums).toBe(false);
    expect(q(".roll")!.classList.contains("drums")).toBe(false);
    expect(q("[data-test=pr-drum-names]")).toBeNull();
    expect(q(".key-names")).not.toBeNull();
  });

  it("the vertical − ＋ change the drum row height by 4 (the normal row height does not change)", async () => {
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

describe("scale lock in the toolbar", () => {
  it("disabled, with 「スケール固定 —」. Pressing sends nothing", async () => {
    await drumSong();
    show(Toolbar);
    const lock = q("[data-test=pr-scale-lock]")!;
    expect(text(lock)).toBe("スケール固定 —");
    expect(lock.getAttribute("aria-disabled")).toBe("true");
    expect(q("[data-test=pr-scale-menu]")!.getAttribute("aria-disabled")).toBe("true");
    expect(q("[data-test=pr-scale-highlight]")!.getAttribute("aria-disabled")).toBe("true");
    lock.click();
    q("[data-test=pr-scale-menu]")!.click();
    q("[data-test=pr-scale-highlight]")!.click();
    flushSync();
    expect(app.sent).toEqual([]);
  });
});

describe("note list", () => {
  it("no 「長さ」 column: position, sound (instrument name), Vel", async () => {
    await drumSong();
    show(NoteListPanel);
    expect(text(q(".head"))).toBe("位置 音 Vel");
    expect(q(".head")!.classList.contains("drums")).toBe(true);
    expect(qa(".row").slice(0, 3).map(text)).toEqual(["1.1.0000 キック 104", "1.1.0000 クローズハイハット 88", "1.1.0000 クラッシュ 110"]);
    expect(qa(".row.selected").map(text)).toEqual(["1.2.0000 スネア 96", "2.2.0000 スネア 96"]);
  });

  it("notes on keys not in the kit use note names", async () => {
    const song = await drumSong();
    const odd = take([[0, TPQ / 4, 60, 90], [0, TPQ / 4, 36, 104]], song.clip.takeId);
    shown.setShown({ clip: song.clip, track: song.track, take: odd });
    show(NoteListPanel);
    expect(qa(".row").map(text)).toEqual(["1.1.0000 C4 90", "1.1.0000 キック 104"]);
  });
});

describe("right panel (selected notes)", () => {
  it("the sound is the instrument name. The scale is 「—」, scale lock disabled", async () => {
    await drumSong();
    show(NoteInspector);
    expect(text(q("[data-test=inspector-keys]"))).toBe("スネア ×2");
    expect(text(q("[data-test=inspector-scale]"))).toBe("—");
    expect(text(q("[data-test=inspector-lock]"))).toBe("使えない");
    expect(q("[data-test=inspector-lock]")!.tagName).toBe("SPAN");
    expect(text(q("[data-test=inspector-highlight]"))).toBe("使えない");
    expect(q("[data-test=inspector-highlight]")!.tagName).toBe("SPAN");
  });
});

describe("status line and input meter", () => {
  it("the note name of the selected note is the instrument name", async () => {
    await drumSong();
    show(StatusLine);
    expect(text(q(".status"))).toContain("選択 2 ノート ・ スネア ×2");
  });

  it("「いま押している音」 in the input meter: instrument names when the play target is drums (「キック スネア」)", async () => {
    await drumSong();
    show(InputMeter);
    app.setTelemetry(app.telemetryWith({ keys: [38, 36] }));
    flushSync();
    expect(text(q('[data-test="held"]'))).toBe("キック スネア");
  });

  it("note names when the play target is not drums", async () => {
    await drumSong((s) => (s.song.playTarget = s.song.keysTarget = s.song.tracks[0]!.id));
    show(InputMeter);
    app.setTelemetry(app.telemetryWith({ keys: [38, 36] }));
    flushSync();
    expect(text(q('[data-test="held"]'))).toBe("C2 D2");
  });
});

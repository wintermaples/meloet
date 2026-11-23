// Mounts the piano roll band and checks the toolbar (text, take tags, tools, − ＋, grid list, triplets, scale lock), folding the left and right panels, and the CC tags.
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import LowerBand from "../src/components/LowerBand.svelte";
import { ccLanes, prPanels, prView } from "../src/components/pianoroll/view.svelte";
import * as app from "./fake-app.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { take } from "./pianoroll-fixture";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  takeNotes: (id: number) =>
    Promise.resolve(
      take([[0, TPQ, 60]], id, {
        controlTicks: Uint32Array.from([0, TPQ / 48, TPQ / 24]),
        controlKinds: Uint8Array.from([0, 0, 1]),
        controlNumbers: Uint8Array.from([64, 64, 0]),
        controlValues: Int16Array.from([127, 0, 300]),
      }),
    ),
}));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  prPanels.list = true;
  prPanels.inspector = true;
  prView.tool = "select";
  prView.quarterPx = 40;
  ccLanes.open = [];
  ccLanes.other = null;
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  document.querySelectorAll("[data-test=pr-grid-menu]").forEach((n) => n.parentElement?.remove());
});

function show(state: AppState): void {
  app.setState(state);
  mounted = mount(LowerBand, { target, props: { band: "pianoRoll", height: 640 } });
  flushSync();
}
const scene = () => fakeScene("pianoRoll")!.state;
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const settle = async () => {
  for (let i = 0; i < 3; i++) {
    await new Promise((r) => setTimeout(r, 0));
    flushSync();
  }
};

describe("toolbar", () => {
  it("clip text (mock A-PianoRoll). The selected count is accent", () => {
    show(scene());
    expect(text(q("[data-test=pr-info]"))).toBe("1 ピアノ ・ イントロ ・ 小節 1–4 ・ ノート 23（選択 3）");
    expect(text(q("[data-test=pr-info] .selected"))).toBe("3");
    expect(q("[data-test=pr-take]")).toBeNull();
  });

  it("with 2 or more takes, a switch tag. ◂ ▸ is StepTake", () => {
    const s = scene();
    s.song.tracks[0]!.clips[0] = { ...s.song.tracks[0]!.clips[0]!, take: 2, takes: 3 };
    show(s);
    expect(text(q("[data-test=pr-take]"))).toBe("◂ テイク 2 / 3 ▸ T");
    q("[data-test=pr-take] button")!.click();
    expect(app.sent).toEqual([{ type: "stepTake", delta: -1 }]);
  });

  it("the tool (select, draw, erase) is a screen-only state", () => {
    show(scene());
    q("[data-test=pr-tool-draw]")!.click();
    flushSync();
    expect(prView.tool).toBe("draw");
    expect(q("[data-test=pr-tool-draw]")!.classList.contains("on")).toBe(true);
    expect(app.sent).toEqual([]);
  });

  it("− ＋ zooms one step sideways", () => {
    show(scene());
    q("[data-test=pr-zoom-in]")!.click();
    expect(prView.quarterPx).toBe(57);
    q("[data-test=pr-zoom-out]")!.click();
    q("[data-test=pr-zoom-out]")!.click();
    expect(prView.quarterPx).toBe(28);
  });

  it("grid list (「1 小節」, 1/2, 1/4, 1/8, 1/16, 1/32), triplets, scale lock", () => {
    show(scene());
    expect(text(q("[data-test=pr-grid]"))).toBe("グリッド 1/16");
    q("[data-test=pr-grid]")!.click();
    flushSync();
    const menu = document.querySelector("[data-test=pr-grid-menu]")!;
    const rows = [...menu.querySelectorAll("button")];
    expect(rows.map((b) => text(b))).toEqual(["1 小節", "1/2", "1/4", "1/8", "1/16", "1/32"]);
    expect(rows[4]!.getAttribute("aria-checked")).toBe("true");
    rows[3]!.click();
    flushSync();
    expect(document.querySelector("[data-test=pr-grid-menu]")).toBeNull();
    q("[data-test=pr-triplet]")!.click();
    q("[data-test=pr-scale-lock]")!.click();
    expect(text(q("[data-test=pr-scale-lock]"))).toBe("スケール固定 C メジャー");
    expect(app.sent).toEqual([
      { type: "setGrid", division: 8, triplet: false },
      { type: "setGrid", division: 16, triplet: true },
      { type: "setMusicalScale", root: 0, mode: "major", lock: false },
    ]);
  });
});

describe("where there is no clip (mock F-PREmpty, spec 3 and 6)", () => {
  it("the toolbar is「1 ピアノ ・ クリップなし」(the number is Mono). Two hint sentences at the top left of the note area (「2」is Mono)", async () => {
    show(fakeScene("pianoRollEmpty")!.state);
    await settle();
    expect(text(q("[data-test=pr-info]"))).toBe("1 ピアノ ・ クリップなし");
    expect(text(q("[data-test=pr-info] .m-num"))).toBe("1");
    const hint = q("[data-test=pr-empty-hint]")!;
    expect(hint.closest(".area")).not.toBeNull();
    expect([...hint.children].map(text)).toEqual([
      "ここにはクリップがありません。",
      "書く道具（2）で押すと、そこから 4 小節のクリップを作って書きます。鍵盤で弾いて録っても作られます。",
    ]);
    expect(text(hint.querySelector(".m-num"))).toBe("2");
  });

  it("when the draw tool makes a clip, the hint goes and the toolbar shows the clip text", async () => {
    const s = fakeScene("pianoRollEmpty")!.state;
    show(s);
    await settle();
    expect(q("[data-test=pr-empty-hint]")).not.toBeNull();
    const made = fakeScene("pianoRoll")!.state;
    app.setState({ ...s, song: { ...s.song, tracks: made.song.tracks }, arrange: made.arrange });
    await settle();
    expect(q("[data-test=pr-empty-hint]")).toBeNull();
    expect(text(q("[data-test=pr-info]"))).toContain("1 ピアノ ・ イントロ");
  });

  it("no hint when there is a clip", async () => {
    show(scene());
    await settle();
    expect(q("[data-test=pr-empty-hint]")).toBeNull();
  });
});

describe("left and right panels and CC tags", () => {
  it("one heading per panel (「▾ ノートの一覧 23」, 「▾ 選んだノート 3」). Pressing folds it to a vertical heading; pressing again opens it", () => {
    show(scene());
    expect(text(q("[data-test=pr-list-fold]"))).toBe("▾ ノートの一覧 23");
    expect(text(q("[data-test=pr-inspector-fold]"))).toBe("▾ 選んだノート 3");
    const band = q("[data-band=pianoRoll]")!;
    expect(band.textContent!.split("選んだノート").length - 1).toBe(1);
    expect(band.textContent!.split("ノートの一覧").length - 1).toBe(1);
    q("[data-test=pr-list-fold]")!.click();
    q("[data-test=pr-inspector-fold]")!.click();
    flushSync();
    expect(text(q("[data-test=pr-list-open]"))).toBe("▸ ノートの一覧 23");
    expect(text(q("[data-test=pr-inspector-open]"))).toBe("▸ 選んだノート 3");
    q("[data-test=pr-list-open]")!.click();
    flushSync();
    expect(q("[data-testid=pr-note-list]")).not.toBeNull();
    expect(prPanels).toEqual({ list: true, inspector: false });
  });

  it("CC tags show real counts and can be pressed (jsdom has width 0, so「そのほかの CC」is not shown)", async () => {
    show(scene());
    await settle();
    const tags = [...target.querySelectorAll<HTMLElement>(".cc-row button")];
    expect(tags.map((b) => b.getAttribute("aria-disabled"))).toEqual([null, null, null]);
    expect(tags.map((b) => text(b))[0]).toMatch(/^▸ ?サステイン ?(CC 64 ・ 踏む )?1$/);
    expect(tags.map((b) => text(b))[1]).toMatch(/^▸ ?ピッチベンド ?(点 )?1$/);
  });

  it("pressing a tag opens the band with ▾, pressing again folds it. Open bands are remembered (in opening order)", async () => {
    show(scene());
    await settle();
    q("[data-test=pr-cc-pitchBend]")!.click();
    q("[data-test=pr-cc-sustain]")!.click();
    flushSync();
    expect(ccLanes.open).toEqual(["pitchBend", "sustain"]);
    expect(text(q("[data-test=pr-cc-sustain]"))).toMatch(/^▾ ?サステイン/);
    // The band order is the same as the tags (sustain, pitch bend).
    expect([...target.querySelectorAll("[data-test^=pr-lane-]:not([data-test^=pr-lane-fold])")].map((l) => l.getAttribute("data-test"))).toEqual([
      "pr-lane-sustain",
      "pr-lane-pitchBend",
    ]);
    expect(text(q("[data-test=pr-lane-fold-pitchBend]"))).toBe("▾ ピッチベンド");
    q("[data-test=pr-cc-sustain]")!.click();
    flushSync();
    expect(ccLanes.open).toEqual(["pitchBend"]);
    expect(q("[data-test=pr-lane-sustain]")).toBeNull();
  });
});

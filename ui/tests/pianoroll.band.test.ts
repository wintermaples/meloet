// ピアノロールの帯を mount して、道具の列（文、テイクの札、道具、− ＋、グリッドの一覧、三連符、スケール固定）、左右の欄のたたみ、CC の札を見る。
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import LowerBand from "../src/components/LowerBand.svelte";
import { prPanels, prView } from "../src/components/pianoroll/view.svelte";
import * as app from "./fake-app.svelte";
import { take } from "./pianoroll-fixture";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  takeNotes: (id: number) =>
    Promise.resolve(
      take([[0, 480, 60]], id, {
        controlTicks: Uint32Array.from([0, 10, 20]),
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

describe("道具の列", () => {
  it("クリップの文（モック A-PianoRoll）。選んだ数は accent", () => {
    show(scene());
    expect(text(q("[data-test=pr-info]"))).toBe("1 ピアノ ・ イントロ ・ 小節 1–4 ・ ノート 23（選択 3）");
    expect(text(q("[data-test=pr-info] .selected"))).toBe("3");
    expect(q("[data-test=pr-take]")).toBeNull();
  });

  it("テイクが 2 つ以上なら切りかえの札。◂ ▸ は StepTake", () => {
    const s = scene();
    s.song.tracks[0]!.clips[0] = { ...s.song.tracks[0]!.clips[0]!, take: 2, takes: 3 };
    show(s);
    expect(text(q("[data-test=pr-take]"))).toBe("◂ テイク 2 / 3 ▸ T");
    q("[data-test=pr-take] button")!.click();
    expect(app.sent).toEqual([{ type: "stepTake", delta: -1 }]);
  });

  it("道具（選ぶ ・ 書く ・ 消す）は画面だけの状態", () => {
    show(scene());
    q("[data-test=pr-tool-draw]")!.click();
    flushSync();
    expect(prView.tool).toBe("draw");
    expect(q("[data-test=pr-tool-draw]")!.classList.contains("on")).toBe(true);
    expect(app.sent).toEqual([]);
  });

  it("− ＋ で横の拡大を 1 段", () => {
    show(scene());
    q("[data-test=pr-zoom-in]")!.click();
    expect(prView.quarterPx).toBe(57);
    q("[data-test=pr-zoom-out]")!.click();
    q("[data-test=pr-zoom-out]")!.click();
    expect(prView.quarterPx).toBe(28);
  });

  it("グリッドの一覧（1 小節 ・ 1/2 ・ 1/4 ・ 1/8 ・ 1/16 ・ 1/32）、三連符、スケール固定", () => {
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

describe("左右の欄と CC の札", () => {
  it("見出しを押すとたたんで縦の見出し（ノートの一覧 23 ・ 選んだノート 3）、押すとひらく", () => {
    show(scene());
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

  it("CC の札は本物の数で、押せない（使えない）", async () => {
    show(scene());
    await settle();
    const tags = [...target.querySelectorAll<HTMLElement>(".cc-row button")];
    expect(tags.every((b) => b.getAttribute("aria-disabled") === "true")).toBe(true);
    expect(tags.map((b) => text(b))[0]).toMatch(/^▸ ?サステイン ?(CC 64 ・ 踏む )?1$/);
    expect(tags.map((b) => text(b))[1]).toMatch(/^▸ ?ピッチベンド ?(点 )?1$/);
  });
});

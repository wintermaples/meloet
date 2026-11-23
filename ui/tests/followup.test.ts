// Mounts and checks fixes from docs/history/design-notes.ja.md B-64, 65, 72–75: folding the velocity band, the mark of the band keys go to,
// keys on buttons, folding by an open band's heading, the scale lock ▾, Esc while dragging the loop range and the overview frame, keys for the strength and swing bars, term tips.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import Arrange from "../src/components/Arrange.svelte";
import BandRow from "../src/components/BandRow.svelte";
import LowerBand from "../src/components/LowerBand.svelte";
import PercentSlider from "../src/components/pianoroll/PercentSlider.svelte";
import { cancelDrag, ccLanes, prPanels, prView, velocityLane } from "../src/components/pianoroll/view.svelte";
import RightRail from "../src/components/RightRail.svelte";
import Transport from "../src/components/Transport.svelte";
import { tokensAt, type Scale } from "../src/design/tokens";
import { closeMenu, menuVisible } from "../src/lib/menu";
import { hideTip, tipView } from "../src/lib/tip.svelte";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";
import { take } from "./pianoroll-fixture";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/bridge", async (original) => ({
  ...(await original<typeof import("../src/bridge")>()),
  takeNotes: (id: number) => Promise.resolve(take([[0, TPQ, 60]], id)),
}));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  prPanels.list = true;
  prPanels.inspector = true;
  prView.focus = "arrange";
  ccLanes.open = [];
  velocityLane.open = true;
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (menuVisible()) closeMenu();
  hideTip();
  if (mounted) unmount(mounted);
  mounted = null;
  vi.useRealTimers();
  vi.restoreAllMocks();
  document.body.innerHTML = "";
});

function show(component: Component<any>, state: AppState, props: Record<string, unknown> = {}): void {
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}
const showRoll = (state: AppState = fakeScene("pianoRoll")!.state) => show(LowerBand, state, { band: "pianoRoll", height: 640 });
const q = <T extends HTMLElement = HTMLElement>(sel: string) => document.querySelector<T>(sel);
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const click = (sel: string) => {
  q(sel)!.click();
  flushSync();
};

/** The term shown when the mouse rests on el for 0.6 s. */
function termAt(el: Element): string | null {
  vi.useFakeTimers();
  el.dispatchEvent(new PointerEvent("pointerenter"));
  vi.advanceTimersByTime(600);
  flushSync();
  const found = tipView.value?.term ?? null;
  el.dispatchEvent(new PointerEvent("pointerleave"));
  vi.useRealTimers();
  return found;
}

describe("folding the velocity band (spec 3 band table「ベロシティ」, the example in spec 1)", () => {
  it("folding with「▾ ベロシティ」shows「▸ ベロシティ」at the left of the CC tag row. Pressing opens it. It is remembered", () => {
    showRoll();
    expect(q('[data-test="pr-velocity"]')).not.toBeNull();
    expect(q('[data-test="pr-velocity-open"]')).toBeNull();
    click('[data-test="pr-velocity-fold"]');
    expect(q('[data-test="pr-velocity"]')).toBeNull();
    const tag = q('[data-test="pr-velocity-open"]')!;
    expect(text(tag)).toBe("▸ベロシティ");
    expect(tag.parentElement!.firstElementChild).toBe(tag);
    expect(velocityLane.open).toBe(false);

    // Folding and opening the band again keeps it folded.
    unmount(mounted!);
    mounted = null;
    showRoll();
    expect(q('[data-test="pr-velocity"]')).toBeNull();
    click('[data-test="pr-velocity-open"]');
    expect(q('[data-test="pr-velocity"]')).not.toBeNull();
    expect(q('[data-test="pr-velocity-open"]')).toBeNull();
  });

  // The spec 1 example is 18 rows and 24 rows. The menu bar (36 at 150 %) takes 2 rows from each (docs/history/design-notes.ja.md B-59).
  // The chord row under the tool row (36 at 150 %) takes 2 more rows.
  it("at 150 % in a 1280×800 window, 14 note rows. Folded, 20 rows (the folded 72 goes to the note area)", () => {
    const scale: Scale = 150;
    const scene = fakeScene("pianoRoll150")!;
    const band = scene.state.workspace.layout.lower.find((l) => l.band === "pianoRoll")!.height;
    const px = tokensAt(scale, 1);
    const n = (name: string) => px[name] as number;
    // The note area is the band height minus the toolbar, the key's chord row, the ruler, the velocity band and the CC tag row (the vertical order in Roll.svelte and PianoRoll.svelte).
    const around = n("size.toolbar") + n("size.row") + n("mock.pianoroll.ruler") + n("size.row");
    const rows = (velocity: boolean) => Math.floor((Math.round((band * scale) / 100) - around - (velocity ? n("size.lane.vel") : 0)) / n("size.note"));
    expect(scene.state.workspace.viewportWidth).toBe(1280);
    expect(scene.state.workspace.viewportHeight).toBe(800);
    expect(rows(true)).toBe(14);
    expect(rows(false)).toBe(20);
  });
});

describe("mark of the band keys go to (B-72)", () => {
  it("when keys go to the piano roll, the top edge of the toolbar. When they go to the arrange, the arrange heading row", () => {
    const state = fakeScene("pianoRoll")!.state;
    showRoll(state);
    expect(q('[data-test="pr-focus-mark"]')).toBeNull();
    prView.focus = "pianoRoll";
    flushSync();
    expect(q('[data-test="pr-focus-mark"]')).not.toBeNull();
    unmount(mounted!);
    mounted = null;

    show(Arrange, state);
    expect(q('[data-test="arrange-focus-mark"]')).toBeNull();
    prView.focus = "arrange";
    flushSync();
    expect(q('[data-test="arrange-focus-mark"]')).not.toBeNull();
  });

  it("with the piano roll closed, always the arrange", () => {
    prView.focus = "pianoRoll";
    show(Arrange, app.stateWith(() => {}));
    expect(q('[data-test="arrange-focus-mark"]')).not.toBeNull();
  });
});

describe("keys on buttons (B-73)", () => {
  const ready = (edit: (s: AppState) => void = () => {}) =>
    app.stateWith((s) => {
      s.transport.canPlay = true;
      edit(s);
    });

  it("transport: ⏮ Home, ▶ Space, ● R, loop L, metronome C. Not written on ■ and count-in (F-WideArrange)", () => {
    show(Transport, ready());
    const keyOf = (name: string) => text(q(`[data-test="transport-${name}"] .key`));
    expect(["to-start", "play", "stop", "record", "loop", "metronome", "count-in"].map(keyOf)).toEqual([
      "Home",
      "Space",
      null,
      "R",
      "L",
      "C",
      null,
    ]);
  });

  it("piano roll tools:「選ぶ」1, 「書く」2, 「消す」3", () => {
    showRoll();
    expect(["select", "draw", "erase"].map((tool) => text(q(`[data-test="pr-tool-${tool}"]`)))).toEqual(["選ぶ1", "書く2", "消す3"]);
  });
});

describe("pressing an open band's heading folds it (B-74)", () => {
  it("「▾ ピアノロール」is toggleBand pianoRoll", () => {
    showRoll();
    click('[data-test="pr-fold"]');
    expect(app.sent).toEqual([{ type: "toggleBand", band: "pianoRoll" }]);
  });

  it("「▾ ミキサー」「▾ 取り消しの履歴」", () => {
    show(LowerBand, app.stateWith(() => {}), { band: "mixer", height: 516 });
    click('[data-test="band-fold"]');
    unmount(mounted!);
    mounted = null;
    show(LowerBand, app.stateWith(() => {}), { band: "history", height: 248 });
    click('[data-test="band-fold"]');
    expect(app.sent).toEqual([
      { type: "toggleBand", band: "mixer" },
      { type: "toggleBand", band: "history" },
    ]);
  });

  it("「▾ 棚」「▾ パラメータ」", () => {
    show(
      RightRail,
      app.stateWith((s) => {
        s.workspace.layout.rightOpen = ["shelf"];
        s.workspace.layout.rightFront = "shelf";
      }),
    );
    expect(text(q('[data-test="rail-fold"]'))).toBe("▾ 棚");
    click('[data-test="rail-fold"]');
    expect(app.sent).toEqual([{ type: "toggleBand", band: "shelf" }]);
  });
});

describe("scale lock ▾ (B-65)", () => {
  it("▾ to the right of the button. Opens the list of scales; choosing sends setMusicalScale with lock: true", async () => {
    showRoll();
    const lock = q('[data-test="pr-scale-lock"]')!;
    const stub = q('[data-test="pr-scale-menu"]')!;
    expect(lock.nextElementSibling).toBe(stub);
    expect(text(stub)).toBe("▾");
    click('[data-test="pr-scale-menu"]');
    await new Promise((r) => setTimeout(r, 0));
    const menu = q('[data-test="scale-menu"]')!;
    expect(menu).not.toBeNull();
    expect(menuVisible()).toBe(true);
    const rows = [...menu.querySelectorAll<HTMLButtonElement>("button")];
    rows.find((b) => text(b) === "マイナー")!.click();
    flushSync();
    const pr = fakeScene("pianoRoll")!.state.pianoRoll;
    expect(app.sent).toEqual([{ type: "setMusicalScale", root: pr.scale.root, mode: "minor", lock: true }]);
    expect(q('[data-test="scale-menu"]')).toBeNull();
  });
});

describe("Esc while dragging (spec 4)", () => {
  it("cancels the loop range drag and returns to the old range", () => {
    const s = app.stateWith(() => {});
    show(Arrange, s);
    const ruler = q(".ruler.bars")!;
    const pointer = (type: string, x: number) => {
      ruler.dispatchEvent(new MouseEvent(type, { bubbles: true, clientX: x, clientY: 10, button: 0 }));
      flushSync();
    };
    pointer("pointerdown", 256);
    pointer("pointermove", 512);
    expect(q(".loop-mark")).not.toBeNull();
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(q(".loop-mark")).toBeNull();
    pointer("pointerup", 512);
    expect(app.sent).toEqual([]);
    expect(cancelDrag()).toBe(false);
  });

  it("strength and swing bars: cancel the drag and return to the value before. onchange is not sent", () => {
    const changes: number[] = [];
    show(PercentSlider, app.stateWith(() => {}), { value: 75, label: "強さ", test: "s", onchange: (v: number) => changes.push(v) });
    const bar = q('[data-test="s"]')!;
    bar.getBoundingClientRect = () => ({ left: 0, width: 100, top: 0, height: 12, right: 100, bottom: 12, x: 0, y: 0, toJSON() {} });
    bar.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, clientX: 20, button: 0 }));
    flushSync();
    expect(text(q('[data-test="s-value"]'))).toBe("20 %");
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(text(q('[data-test="s-value"]'))).toBe("75 %");
    bar.dispatchEvent(new MouseEvent("pointerup", { bubbles: true, clientX: 20, button: 0 }));
    expect(changes).toEqual([]);
  });
});

describe("strength and swing bars are reached with Tab and move with ← → (spec 4)", () => {
  it("tabindex 0. ← → by 10, with Shift by 1. Not passed to the document key handler", () => {
    const changes: number[] = [];
    show(PercentSlider, app.stateWith(() => {}), { value: 75, label: "強さ", test: "s", onchange: (v: number) => changes.push(v) });
    const bar = q('[data-test="s"]')!;
    expect(bar.tabIndex).toBe(0);
    let reached = 0;
    const onDoc = () => reached++;
    document.addEventListener("keydown", onDoc);
    const key = (k: string, shiftKey = false) => {
      bar.dispatchEvent(new KeyboardEvent("keydown", { key: k, shiftKey, bubbles: true }));
      flushSync();
    };
    key("ArrowRight");
    key("ArrowLeft", true);
    key("ArrowRight");
    key("ArrowRight");
    document.removeEventListener("keydown", onDoc);
    expect(changes).toEqual([85, 84, 94, 100]);
    expect(reached).toBe(0);
  });
});

describe("term tips (spec 5)", () => {
  it("band tabs: the names arrange, piano roll and mixer, and「ノート」in the piano roll count", () => {
    // jsdom has no width, so give a width that fits the count.
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(1280);
    show(BandRow, app.stateWith(() => {}));
    const name = (band: string) => q(`[data-band="${band}"] .m-name`)!;
    expect(termAt(name("arrange"))).toBe("アレンジ");
    expect(termAt(name("pianoRoll"))).toBe("ピアノロール");
    expect(termAt(name("mixer"))).toBe("ミキサー");
    const note = [...q('[data-band="pianoRoll"]')!.querySelectorAll("span")].find((s) => s.textContent === "ノート")!;
    expect(termAt(note)).toBe("ノート");
  });

  it("piano roll: heading,「ノート」in the left and right panels, velocity, CC tags", () => {
    showRoll(fakeScene("pianoRollCc")!.state);
    expect(termAt(q('[data-test="pr-fold"] .m-name')!)).toBe("ピアノロール");
    const word = (root: string, w: string) => [...q(root)!.querySelectorAll("span")].find((s) => s.textContent === w)!;
    expect(termAt(word('[data-test="pr-list-fold"]', "ノート"))).toBe("ノート");
    expect(termAt(word('[data-test="pr-inspector-fold"]', "ノート"))).toBe("ノート");
    expect(termAt(word('[data-test="pr-velocity-fold"]', "ベロシティ"))).toBe("ベロシティ");
    expect(termAt(word('[data-test="pr-cc-sustain"]', "サステイン"))).toBe("サステイン");
    expect(termAt(word('[data-test="pr-cc-pitchBend"]', "ピッチベンド"))).toBe("ピッチベンド");
    expect(termAt(word('[data-test="pr-cc-modulation"]', "モジュレーション"))).toBe("モジュレーション");
  });

  it("arrange: heading, track names, track dB", () => {
    show(Arrange, app.stateWith(() => {}));
    expect(termAt(q(".title-row .m-name")!)).toBe("アレンジ");
    expect(termAt(q(".track-name")!)).toBe("トラック");
    const db = [...document.querySelectorAll(".track-head .m-num")].find((s) => s.textContent?.endsWith("dB"))!;
    expect(termAt(db)).toBe("dB");
  });
});

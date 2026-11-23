// Round 12, M7 「音程を寄せる」: the two rows in the panel of the chosen clip (the strength, the scale with
// 「いまのスケールにする」), the tooltip, the disabled form, the fake's copy of the intents and the scenes.
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, AudioClipView, StretchChange, TrackView } from "../src/bridge";
import { TUNE_STRENGTH_MAX } from "../src/bridge";
import { fakeScene, reduceAudio, type FakeState } from "../src/bridge/fake";
import ParamsPanel from "../src/components/params/ParamsPanel.svelte";
import { cancelDrag, dragReading } from "../src/components/pianoroll/view.svelte";
import { hideTip, TIP_DELAY_MS, tipView } from "../src/lib/tip.svelte";
import { t } from "../src/text/ja";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  hideTip();
  vi.useRealTimers();
});
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const scene = (name: string) => fakeScene(name)!.state as FakeState;
function panel(state: AppState): void {
  app.setState(state);
  mounted = mount(ParamsPanel, { target, props: { onfold: () => {} } });
  flushSync();
}
function pointer(el: Element, type: string, clientY: number): void {
  el.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, clientX: 0, clientY, button: 0 }));
  flushSync();
}
function type(value: string): void {
  q('[data-test="clip-value-tune"]')!.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
  flushSync();
  const input = q<HTMLInputElement>('[data-test="param-input"]')!;
  input.value = value;
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
  flushSync();
}
const changes = () => app.sent.map((c) => (c.type === "setClipStretch" ? c.change : c));
const strength = () => q('[data-test="clip-value-tune"]')!;
const toScale = () => q<HTMLButtonElement>('[data-test="clip-tune-to-scale"]')!;

describe("the rows 「音程を寄せる」", () => {
  it("wording as the spec has it", () => {
    expect([t("clipParams.tune"), t("clipParams.tuneToScale"), t("clipParams.tuneTip")]).toEqual([
      "音程を寄せる",
      "いまのスケールにする",
      "ひとりの声や、音が1つずつの楽器に使えます",
    ]);
    expect(TUNE_STRENGTH_MAX).toBe(100);
  });

  it("at strength 0: 「0 %」, the song's scale as where it would go, and nothing to take", () => {
    panel(scene("stretchPitch"));
    expect(text(q('[data-test="clip-tune"]'))).toBe("音程を寄せる 0 %");
    expect(text(q('[data-test="clip-tune-scale-name"]'))).toBe("C メジャー");
    expect(toScale().disabled).toBe(true);
    expect(target.querySelectorAll(".row.unusable")).toHaveLength(0);
    // The group counts the row.
    expect(text(q('[data-group="stretch"]'))).toBe("▾ のび縮み ・ 音の高さ 5");
  });

  it("a tuned clip shows its own scale, not the song's; 「いまのスケールにする」 is one SetClipStretch", () => {
    const s = scene("stretchTune");
    panel(s);
    expect(text(q('[data-test="clip-tune"]'))).toBe("音程を寄せる 60 %");
    expect(text(q('[data-test="clip-tune-scale-name"]'))).toBe("A マイナー");
    expect(s.pianoRoll.scale).toMatchObject({ root: 0, mode: "major" });
    expect(toScale().disabled).toBe(false);
    toScale().click();
    expect(app.sent).toEqual([{ type: "setClipStretch", clip: s.params.selected!.kind === "clip" ? s.params.selected!.clip : -1, change: { kind: "tuneScale" } }]);
  });

  it("with the song's scale already, there is nothing to take", () => {
    const s = scene("stretchTune");
    s.pianoRoll.scale = { ...s.pianoRoll.scale, root: 9, mode: "minor" };
    panel(s);
    expect(toScale().disabled).toBe(true);
  });

  it("dragging up moves only the text first (2 px a percent), one intent on release; it stops at 0 and 100", () => {
    panel(scene("stretchTune"));
    pointer(strength(), "pointerdown", 500);
    pointer(strength(), "pointermove", 500 - 2 * 15);
    expect(text(strength())).toBe("75 %");
    expect(app.sent).toEqual([]);
    expect(dragReading.text).toBe("音程を寄せる 75 %（ドラッグ中）");
    expect(text(q('[data-test="clip-value-tune-reading"]'))).toBe("75 %");
    pointer(strength(), "pointermove", -5000);
    expect(text(strength())).toBe("100 %");
    pointer(strength(), "pointerup", -5000);
    expect(dragReading.text).toBeNull();
    expect(changes()).toEqual([{ kind: "tuneStrength", percent: 100 }]);
    pointer(strength(), "pointerdown", 0);
    pointer(strength(), "pointermove", 5000);
    expect(text(strength())).toBe("0 %");
    pointer(strength(), "pointerup", 5000);
    expect(changes()).toEqual([
      { kind: "tuneStrength", percent: 100 },
      { kind: "tuneStrength", percent: 0 },
    ]);
  });

  it("Esc stops the drag and sends nothing; released where it began sends nothing", () => {
    panel(scene("stretchTune"));
    pointer(strength(), "pointerdown", 100);
    pointer(strength(), "pointermove", 0);
    expect(text(strength())).toBe("100 %");
    expect(cancelDrag()).toBe(true);
    flushSync();
    expect(text(strength())).toBe("60 %");
    pointer(strength(), "pointerup", 0);
    pointer(strength(), "pointerdown", 100);
    pointer(strength(), "pointermove", 101);
    pointer(strength(), "pointerup", 101);
    expect(app.sent).toEqual([]);
  });

  it("typed after pressing twice: 「35」 and 「35 %」; out of range goes to the ends; the same number and unreadable text send nothing", () => {
    panel(scene("stretchTune"));
    type("35");
    type("80 %");
    type("250");
    type("60");
    type("つよく");
    expect(changes()).toEqual([
      { kind: "tuneStrength", percent: 35 },
      { kind: "tuneStrength", percent: 80 },
      { kind: "tuneStrength", percent: 100 },
    ]);
  });

  it("the label says what it is for after the mouse rests on it", () => {
    vi.useFakeTimers();
    panel(scene("stretchTune"));
    const label = q('[data-test="clip-tune"] .label')!;
    label.dispatchEvent(new PointerEvent("pointerenter", { bubbles: true }));
    vi.advanceTimersByTime(TIP_DELAY_MS + 1);
    expect(tipView.value?.body).toBe("ひとりの声や、音が1つずつの楽器に使えます");
  });

  it("not usable (tuneUsable false): both rows stay, disabled, and nothing is sent", () => {
    const s = scene("stretchTuneUnusable");
    panel(s);
    expect([...target.querySelectorAll(".row.unusable")].map((r) => r.getAttribute("data-test"))).toEqual(["clip-tune", "clip-tune-scale"]);
    expect(text(q('[data-test="clip-tune"]'))).toBe("音程を寄せる 0 %");
    expect(text(q('[data-test="clip-tune-scale-name"]'))).toBe("C メジャー");
    expect(toScale().disabled).toBe(true);
    pointer(strength(), "pointerdown", 100);
    pointer(strength(), "pointermove", 0);
    pointer(strength(), "pointerup", 0);
    strength().dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
    flushSync();
    expect(q('[data-test="param-input"]')).toBeNull();
    expect(app.sent).toEqual([]);
    // The rows of the same group that can be used still are.
    expect(q<HTMLButtonElement>('[data-test="clip-switch-follow"]')!.disabled).toBe(false);
  });

  it("a tuned clip that cannot be tuned any more keeps showing its strength, disabled", () => {
    const s = scene("stretchTune");
    if (s.params.selected?.kind === "clip") s.params.selected.tuneUsable = false;
    panel(s);
    expect(text(q('[data-test="clip-tune"]'))).toBe("音程を寄せる 60 %");
    expect(toScale().disabled).toBe(true);
  });

  it("cannot be changed while recording", () => {
    const s = scene("stretchTune");
    s.transport.state = "recording";
    panel(s);
    pointer(strength(), "pointerdown", 100);
    pointer(strength(), "pointermove", 0);
    pointer(strength(), "pointerup", 0);
    expect(toScale().disabled).toBe(true);
    toScale().click();
    expect(app.sent).toEqual([]);
  });

  it("a clip whose file is not found shows the notice, not the rows", () => {
    panel(scene("audioMissing"));
    expect(q('[data-test="clip-missing"]')).not.toBeNull();
    expect(q('[data-test="clip-tune"]')).toBeNull();
  });
});

describe("the fake's copy of the intents", () => {
  type S = NonNullable<ReturnType<typeof fakeScene>>["state"];
  const CLIP = 70;
  const vocalOf = (s: AppState): TrackView => s.song.tracks.find((tr) => tr.kind.kind === "vocal")!;
  const audioOf = (s: AppState): AudioClipView => vocalOf(s).clips.find((c) => c.id === CLIP)!.audio!;
  const stretch = (s: S, change: StretchChange, clip = CLIP) => reduceAudio(s, { type: "setClipStretch", clip, change })!;
  const after = (changes: ReturnType<typeof stretch>, s: S): S => {
    const song = changes.find((c) => c.section === "song");
    return song ? { ...s, song: song.value as S["song"] } : s;
  };

  it("leaving 0 takes the song's scale; another strength keeps the clip's; to 0 the scale goes. One step each", () => {
    const s = fakeScene("stretchPitch")!.state;
    expect(audioOf(s).stretch.tune).toBeNull();
    expect(stretch(s, { kind: "tuneStrength", percent: 0 })).toEqual([]);
    expect(stretch(s, { kind: "tuneScale" })).toEqual([]);
    s.pianoRoll.scale = { ...s.pianoRoll.scale, root: 2, mode: "dorian" };
    const on = after(stretch(s, { kind: "tuneStrength", percent: 59.6 }), s);
    expect(audioOf(on).stretch.tune).toEqual({ root: 2, mode: "dorian", strength: 60 });
    expect(on.song.undoCount).toBe(s.song.undoCount + 1);
    on.pianoRoll = { ...on.pianoRoll, scale: { ...on.pianoRoll.scale, root: 0, mode: "blues" } };
    const more = after(stretch(on, { kind: "tuneStrength", percent: 400 }), on);
    expect(audioOf(more).stretch.tune).toEqual({ root: 2, mode: "dorian", strength: 100 });
    expect(stretch(more, { kind: "tuneStrength", percent: 100 })).toEqual([]);
    expect(stretch(more, { kind: "tuneStrength", percent: Number.NaN })).toEqual([]);
    const taken = after(stretch(more, { kind: "tuneScale" }), more);
    expect(audioOf(taken).stretch.tune).toEqual({ root: 0, mode: "blues", strength: 100 });
    expect(taken.song.undoCount).toBe(s.song.undoCount + 3);
    expect(stretch(taken, { kind: "tuneScale" })).toEqual([]);
    const off = after(stretch(taken, { kind: "tuneStrength", percent: -1 }), taken);
    expect(audioOf(off).stretch.tune).toBeNull();
    expect(stretch(s, { kind: "tuneStrength", percent: 50 }, 999)).toEqual([]);
  });

  it("the scenes: tuned toward A minor in a song in C major; and the rows disabled", () => {
    const tuned = fakeScene("stretchTune")!.state;
    expect(audioOf(tuned).stretch.tune).toEqual({ root: 9, mode: "minor", strength: 60 });
    expect(tuned.params.selected).toMatchObject({ kind: "clip", tuneUsable: true });
    const unusable = fakeScene("stretchTuneUnusable")!.state;
    expect(unusable.params.selected).toMatchObject({ kind: "clip", tuneUsable: false });
    expect(audioOf(unusable).stretch.tune).toBeNull();
  });
});

// テンポ ・ 拍子の変更とマーカーのキー（仕様書 4 章）を、キーの係（handleKeydown）の順で見る。
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import { onDragCancel, prView } from "../src/components/pianoroll/view.svelte";
import { lastPick, pickClips, pickTimeline } from "../src/components/timeline/pick";
import { trackEdit } from "../src/components/tracks.svelte";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";

describe("handleKeydown（テンポ ・ 拍子の変更とマーカー）", () => {
  let state: AppState;
  let sent: Command[];
  let deps: KeyDeps;
  const press = (key: string, init: KeyboardEventInit = {}) => {
    const ev = new KeyboardEvent("keydown", { key, cancelable: true, ...init });
    handleKeydown(ev, deps);
    return ev;
  };

  beforeEach(() => {
    state = fakeScene("markerMoving")!.state;
    sent = [];
    deps = { state: () => state, send: (c) => void sent.push(c), nav: { platform: "Linux" } };
    prView.focus = "arrange";
    pickClips();
    trackEdit.renaming = null;
  });
  afterEach(() => {
    document.body.innerHTML = "";
    trackEdit.renaming = null;
  });

  it("K: 再生の線の所に「マーカー 4」。送ったあとは Delete がマーカーの側へ", () => {
    const ev = press("k", { code: "KeyK" });
    expect(sent).toEqual([{ type: "addMarker", name: "マーカー 4" }]);
    expect(ev.defaultPrevented).toBe(true);
    expect(lastPick.kind).toBe("timeline");
  });

  it("K は文字を打っている最中は効かない。Alt → ・ Alt ← は効く", () => {
    const input = document.createElement("input");
    document.body.append(input);
    input.focus();
    press("k");
    expect(sent).toEqual([]);
    press("ArrowRight", { altKey: true });
    press("ArrowLeft", { altKey: true });
    expect(sent).toEqual([
      { type: "stepMarker", forward: true },
      { type: "stepMarker", forward: false },
    ]);
  });

  it("F2: マーカーをえらんでいればマーカーの名前、そうでなければトラックの名前", () => {
    press("F2");
    expect(sent).toEqual([{ type: "openMarkerName", id: 2 }]);
    expect(trackEdit.renaming).toBeNull();
    state.timeline = { ...state.timeline, selected: null };
    state.song.playTarget = state.song.tracks[0]!.id;
    press("F2");
    expect(sent).toHaveLength(1);
    expect(trackEdit.renaming).toBe(state.song.tracks[0]!.id);
  });

  it("Delete: 最後にえらんだのがマーカーならマーカーを、クリップならクリップを消す", () => {
    pickTimeline();
    press("Delete");
    expect(sent.at(-1)).toEqual({ type: "deleteTimeline", target: { kind: "marker", id: 2 } });
    pickClips();
    press("Delete");
    expect(sent.at(-1)).toEqual({ type: "deleteSelected" });
  });

  it("Delete はキーがピアノロールに向いているときはピアノロールの物", () => {
    pickTimeline();
    state.workspace.layout = { ...state.workspace.layout, lower: [{ band: "pianoRoll", height: 640 }] };
    prView.focus = "pianoRoll";
    press("Delete");
    expect(sent.some((c) => c.type === "deleteTimeline")).toBe(false);
  });

  it("Esc はドラッグをやめるのがいちばん前（テンポ ・ マーカーのキーを通らない）", () => {
    let canceled = 0;
    const off = onDragCancel(() => (canceled += 1) > 0);
    press("Escape");
    off();
    expect(canceled).toBe(1);
    expect(sent).toEqual([]);
  });

  it("録音中は K も Alt → も送らない", () => {
    state.transport = { ...state.transport, state: "recording" };
    press("k");
    press("ArrowRight", { altKey: true });
    expect(sent.filter((c) => c.type === "addMarker" || c.type === "stepMarker")).toEqual([]);
  });
});

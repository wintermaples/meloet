// Keys for tempo / time signature changes and markers (spec 4), in the order of the key handler (handleKeydown).
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import { onDragCancel, prView } from "../src/components/pianoroll/view.svelte";
import { lastPick, pickClips, pickTimeline } from "../src/components/timeline/pick";
import { trackEdit } from "../src/components/tracks.svelte";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";

describe("handleKeydown (tempo / time signature changes and markers)", () => {
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

  it("K: 「マーカー 4」 at the play line. After sending, Delete goes to the marker", () => {
    const ev = press("k", { code: "KeyK" });
    expect(sent).toEqual([{ type: "addMarker", name: "マーカー 4", tick: null }]);
    expect(ev.defaultPrevented).toBe(true);
    expect(lastPick.kind).toBe("timeline");
  });

  it("K has no effect while typing. Alt → and Alt ← work", () => {
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

  it("F2: the marker name if a marker is chosen, otherwise the track name", () => {
    press("F2");
    expect(sent).toEqual([{ type: "openMarkerName", id: 2 }]);
    expect(trackEdit.renaming).toBeNull();
    state.timeline = { ...state.timeline, selected: null };
    state.song.playTarget = state.song.tracks[0]!.id;
    press("F2");
    expect(sent).toHaveLength(1);
    expect(trackEdit.renaming).toBe(state.song.tracks[0]!.id);
  });

  it("Delete: deletes the marker if a marker was chosen last, the clip if a clip", () => {
    pickTimeline();
    press("Delete");
    expect(sent.at(-1)).toEqual({ type: "deleteTimeline", target: { kind: "marker", id: 2 } });
    pickClips();
    press("Delete");
    expect(sent.at(-1)).toEqual({ type: "deleteSelected" });
  });

  it("Delete belongs to the piano roll when keys go to the piano roll", () => {
    pickTimeline();
    state.workspace.layout = { ...state.workspace.layout, lower: [{ band: "pianoRoll", height: 640 }] };
    prView.focus = "pianoRoll";
    press("Delete");
    expect(sent.some((c) => c.type === "deleteTimeline")).toBe(false);
  });

  it("Esc cancels a drag first (does not reach the tempo / marker keys)", () => {
    let canceled = 0;
    const off = onDragCancel(() => (canceled += 1) > 0);
    press("Escape");
    off();
    expect(canceled).toBe(1);
    expect(sent).toEqual([]);
  });

  it("while recording, neither K nor Alt → is sent", () => {
    state.transport = { ...state.transport, state: "recording" };
    press("k");
    press("ArrowRight", { altKey: true });
    expect(sent.filter((c) => c.type === "addMarker" || c.type === "stepMarker")).toEqual([]);
  });
});

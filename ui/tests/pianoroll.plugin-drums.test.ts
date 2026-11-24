// A plugin instrument that names its keys (LV2 midnam; the owner, 2026-11-25) gets the drum piano roll as an SF2 kit does:
// its named keys as rows (TrackView.drumKeys), named as the plugin names them. Without names, the normal piano roll.
// The names may come in a later state (DrumGizmo names its keys after its kit has loaded), and may go away.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, TrackView } from "../src/bridge";
import { createFakeTransport, fakeScene, type FakeState } from "../src/bridge/fake";
import { decodeTake } from "../src/bridge/take";
import NoteInspector from "../src/components/pianoroll/NoteInspector.svelte";
import NoteListPanel from "../src/components/pianoroll/NoteListPanel.svelte";
import Roll from "../src/components/pianoroll/Roll.svelte";
import Toolbar from "../src/components/pianoroll/Toolbar.svelte";
import { drumKeyOf, drumKeysOf, drumName, isPluginKit, keyLabel, keysDrums, keysLabel, pianoRollDrums, trackDrums } from "../src/components/pianoroll/drums";
import { ghostCandidates } from "../src/components/pianoroll/ghosts";
import { drumView, prPanels, prView } from "../src/components/pianoroll/view.svelte";
import { placeText } from "../src/components/shelfPatterns";
import StatusLine from "../src/components/StatusLine.svelte";
import * as app from "./fake-app.svelte";
import * as shown from "./fake-shown.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));
vi.mock("../src/components/pianoroll/shown.svelte", () => import("./fake-shown.svelte"));

const KIT_ROWS = [49, 57, 55, 52, 51, 59, 53, 46, 44, 42, 50, 48, 47, 45, 43, 41, 39, 38, 40, 37, 36, 54, 56, 58, 60, 61];

const named = () => fakeScene("drumRollPlugin")!.state;
const unnamed = () => fakeScene("drumRollPluginUnnamed")!.state;
const kitOf = (s: AppState): TrackView => s.song.tracks[2]!;

describe("drums.ts for a plugin that names its keys", () => {
  it("drumRollPlugin: Black Pearl's 26 names; the rows are drumKeys in the order of an SF2 kit", () => {
    const kit = kitOf(named());
    expect(kit.sound.kind).toBe("plugin");
    expect(kit.keyNames).toHaveLength(26);
    expect(kit.keyNames![0]).toEqual({ key: 36, name: "Kick Drum" });
    expect(kit.keyNames!.at(-1)).toEqual({ key: 61, name: "Maracas" });
    const drums = trackDrums(kit);
    expect(isPluginKit(drums)).toBe(true);
    expect(drumKeysOf(kit, drums)).toEqual(KIT_ROWS);
    // The same names give the same value, so a derived value that did not change stays the same.
    expect(trackDrums(kit)).toBe(drums);
  });

  it("names as the plugin gives them; keys it does not name get note names; a typed name gives its key", () => {
    const drums = trackDrums(kitOf(named()));
    expect(drumName(36, drums)).toBe("Kick Drum");
    expect(drumName(43, drums)).toBe("Floor Tom Edge/Alt");
    expect(drumName(62, drums)).toBeNull();
    expect(keyLabel(62, drums)).toBe("D4");
    expect(keysLabel([38, 36], drums)).toBe("Kick Drum Snare Center");
    expect(drumKeyOf(" Closed Hat ", drums)).toBe(42);
    expect(drumKeyOf("キック", drums)).toBeNull();
  });

  it("a plugin with no names, or an empty list, is not drums", () => {
    expect(trackDrums(kitOf(unnamed()))).toBe(false);
    expect(trackDrums({ ...kitOf(named()), keyNames: [] })).toBe(false);
  });

  it("the piano roll and the keys follow the track; an audition of a plugin on the shelf is not drums", () => {
    const s = named();
    expect(isPluginKit(pianoRollDrums(s))).toBe(true);
    expect(isPluginKit(keysDrums(s))).toBe(true);
    const sound = kitOf(s).sound;
    s.shelf.selectedSound = { kind: "plugin", key: sound.kind === "plugin" ? sound.key : "" };
    expect(keysDrums(s)).toBe(false);
  });

  it("not offered as a ghost track, as other kits; a pattern goes on a new studio kit, as core places it", () => {
    const s = named();
    expect(ghostCandidates(s.song.tracks, s.song.tracks[0]!.id).map((x) => x.id)).not.toContain(kitOf(s).id);
    expect(placeText(s)).not.toContain(kitOf(s).name);
  });
});

// jsdom does not measure sizes: a note area of 668 × 440, as in drumRoll.
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

/** Puts the scene's state in and shows its clip of 「3 ドラム」. */
async function showScene(s: FakeState): Promise<void> {
  const track = kitOf(s);
  const clip = track.clips.find((c) => c.id === s.arrange.pianoRollClip)!;
  const take = decodeTake(await createFakeTransport(s).takeNotes(clip.takeId));
  app.setState(s);
  shown.setShown({ clip, track, take });
}

function show(component: Component): void {
  mounted = mount(component, { target });
  flushSync();
}

const rowNames = () => qa("[data-test=pr-drum-names] .drum-name").map(text);

describe("mounted", () => {
  it("the key column shows the plugin's names from the top row, with no term tips", async () => {
    await showScene(named());
    show(Roll);
    expect(prView.drums).toBe(true);
    expect(q(".roll")!.classList.contains("drums")).toBe(true);
    const names = rowNames();
    expect(names.length).toBeGreaterThan(0);
    expect(names.slice(0, 4)).toEqual(["Crash 1", "Crash 2", "Splash", "Ride Choked"]);
    expect(q("[data-test=pr-drum-names] .drum-name span")).toBeNull();
    expect(q(".key-names")).toBeNull();
  });

  it("note list, right panel, status line and scale lock as for a kit", async () => {
    await showScene(named());
    show(NoteListPanel);
    expect(q(".head")!.classList.contains("drums")).toBe(true);
    expect(qa(".row").slice(0, 3).map(text)).toEqual(["1.1.0000 Kick Drum 104", "1.1.0000 Closed Hat 88", "1.1.0000 Crash 1 110"]);
    unmount(mounted!);
    show(NoteInspector);
    expect(text(q("[data-test=inspector-keys]"))).toBe("Snare Center ×2");
    unmount(mounted!);
    show(StatusLine);
    expect(text(q(".status"))).toContain("Snare Center ×2");
    unmount(mounted!);
    show(Toolbar);
    expect(q("[data-test=pr-scale-lock]")!.getAttribute("aria-disabled")).toBe("true");
    unmount(mounted!);
    mounted = null;
  });

  it("names that come in a later state switch to drum rows, and back when they go away", async () => {
    const before = unnamed();
    await showScene(before);
    show(Roll);
    expect(prView.drums).toBe(false);
    expect(q("[data-test=pr-drum-names]")).toBeNull();
    expect(q(".key-names")).not.toBeNull();

    const after = named();
    await showScene(after);
    flushSync();
    expect(prView.drums).toBe(true);
    expect(rowNames().slice(0, 2)).toEqual(["Crash 1", "Crash 2"]);
    expect(drumView.rows).toBe(26);

    await showScene(before);
    flushSync();
    expect(prView.drums).toBe(false);
    expect(q("[data-test=pr-drum-names]")).toBeNull();
    expect(q(".key-names")).not.toBeNull();
  });
});

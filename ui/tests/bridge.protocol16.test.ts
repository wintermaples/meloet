// Protocol 16: plugin scan, the list of stopped ones, plugin locations.
import { describe, expect, it } from "vitest";
import { createFakeTransport, fakeScene, fakeState } from "../src/bridge/fake";
import { PROTOCOL_VERSION, type AppState, type Command, type ServerMessage } from "../src/bridge/types";
import { mockScreenText } from "./design-sources";

const state = (scene: string) => fakeScene(scene)!.state;

async function open(scene: string) {
  const start = state(scene);
  const transport = createFakeTransport(structuredClone(start));
  const held = { s: structuredClone(start) as AppState };
  await transport.subscribe(
    (m: ServerMessage) => {
      if (m.kind !== "patch") return;
      held.s = { ...held.s };
      for (const c of m.changes) (held.s as Record<string, unknown>)[c.section] = c.value;
    },
    () => {},
  );
  return { held, send: async (c: Command) => void (await transport.send(c)) };
}

describe("protocol 16 shape", () => {
  it("version is 23 (after 22). In a new song, no scan runs, nothing stopped, and the locations are only the usual LV2 and VST3 places", () => {
    expect(PROTOCOL_VERSION).toBeGreaterThanOrEqual(26);
    const p = fakeState().plugins;
    expect(p.scan).toEqual({ state: "idle" });
    expect(p.stopped).toEqual([]);
    expect(p.places.map((x) => [x.shownPath, x.added])).toEqual([
      ["~/.lv2", false],
      ["/usr/lib/lv2", false],
      ["/usr/local/lib/lv2", false],
      ["~/.vst3", false],
      ["/usr/lib/vst3", false],
      ["/usr/local/lib/vst3", false],
    ]);
    expect(p.places[0]!.path).toBe("/home/yachiyo/.lv2");
    expect([p.stoppedOpen, p.placesOpen]).toEqual([false, false]);
  });
});

describe("protocol 16 fake scenes", () => {
  it("scanning (mock R9-Scan): shelf open, 37 / 112", () => {
    const s = state("scanning");
    expect(s.plugins.scan).toEqual({ state: "running", done: 37, total: 112 });
    expect(s.workspace.layout.rightOpen).toContain("shelf");
    expect(mockScreenText("R9-Scan")).toContain("37 / 112");
  });

  it("scanFinished and pluginsFound: status line notice and the count under the shelf", () => {
    const s = state("scanFinished");
    expect(s.plugins.scan).toEqual({ state: "finished", added: 667, stopped: 4 });
    expect(s.plugins.stopped).toHaveLength(4);
    expect(s.status.message).toEqual({ id: "scanFinished", added: 667, stopped: 4 });
    expect(state("pluginsFound").status.message).toEqual({ id: "pluginsFound", count: 3 });
  });

  it("pluginPlaces (mock R9-Scan): advanced settings open, 2 added locations. No CLAP location", () => {
    const s = state("pluginPlaces");
    expect(s.workspace.settings).toMatchObject({ advancedOpen: true });
    expect(s.plugins.placesOpen).toBe(true);
    const mock = mockScreenText("R9-Scan");
    // /usr/local/lib/vst3 is not in the mock (added as a usual VST3 place; docs/history/research/plugin-formats.md section 8).
    for (const p of s.plugins.places.filter((x) => x.shownPath !== "/usr/local/lib/vst3")) expect(mock, p.shownPath).toContain(p.shownPath);
    expect(s.plugins.places.filter((p) => p.added).map((p) => p.shownPath)).toEqual(["~/音楽/プラグイン", "/opt/surge-xt/lib/vst3"]);
    expect(s.plugins.places.some((p) => p.path.includes("clap"))).toBe(false);
  });

  it("scanStopped (mock F5-ScanStopped): 4 lines of name, format, reason, location", () => {
    const s = state("scanStopped");
    expect(s.plugins.stoppedOpen).toBe(true);
    expect(s.status.message).toEqual({ id: "scanStoppedOpen" });
    expect(s.plugins.stopped.map((p) => [p.name, p.format, p.reason, p.shownPath])).toEqual([
      ["Old Chorus", "vst3", "crashed", "~/.vst3/OldChorus.vst3"],
      ["Granular Fog", "lv2", "timedOut", "/usr/lib/lv2/granfog.lv2"],
      ["Tape Wobble", "lv2", "broken", "~/.lv2/TapeWobble.lv2"],
      ["Brick EQ 1.0", "vst3", "crashed", "/usr/lib/vst3/BrickEQ.vst3"],
    ]);
    const mock = mockScreenText("F5-ScanStopped");
    for (const p of s.plugins.stopped) expect(mock, p.name).toContain(p.name);
    expect(state("scanStoppedRetrying").plugins.stopped.map((p) => p.retrying)).toEqual([false, true, false, false]);
    expect(state("scanRetryAdded").plugins.stopped.map((p) => p.name)).not.toContain("Granular Fog");
    expect(state("scanRetryStopped").status.message).toEqual({ id: "scanRetryStopped", name: "Granular Fog" });
  });
});

describe("protocol 16 intents (fake)", () => {
  it("stopped-plugins dialog: open, retry, close. Esc closes too", async () => {
    const { held, send } = await open("scanFinished");
    await send({ type: "openScanStopped" });
    expect(held.s.plugins.stoppedOpen).toBe(true);
    await send({ type: "retryStoppedPlugin", path: "/usr/lib/lv2/granfog.lv2", id: "urn:other" });
    expect(held.s.plugins.stopped.map((p) => p.retrying)).toEqual([false, false, false, false]);
    await send({ type: "retryStoppedPlugin", path: "/usr/lib/lv2/granfog.lv2", id: "urn:granfog:fog" });
    expect(held.s.plugins.stopped.map((p) => p.retrying)).toEqual([false, true, false, false]);
    await send({ type: "closeScanStopped" });
    expect(held.s.plugins.stoppedOpen).toBe(false);
    await send({ type: "openScanStopped" });
    await send({ type: "back" });
    expect(held.s.plugins.stoppedOpen).toBe(false);
  });

  it("「プラグインの場所…」: closes the dialog and opens sound settings with advanced settings and locations shown", async () => {
    const { held, send } = await open("scanStopped");
    await send({ type: "openPluginPlaces" });
    expect(held.s.plugins).toMatchObject({ stoppedOpen: false, placesOpen: true });
    expect(held.s.workspace.settings).toMatchObject({ advancedOpen: true });
    await send({ type: "togglePluginPlaces" });
    expect(held.s.plugins.placesOpen).toBe(false);
  });

  it("removing a location: only added ones. Usual places cannot be removed", async () => {
    const { held, send } = await open("pluginPlaces");
    await send({ type: "removePluginPlace", path: "/usr/lib/lv2" });
    expect(held.s.plugins.places).toHaveLength(8);
    await send({ type: "removePluginPlace", path: "/opt/surge-xt/lib/vst3" });
    expect(held.s.plugins.places.filter((p) => p.added).map((p) => p.shownPath)).toEqual(["~/音楽/プラグイン"]);
    // The fake does not scan.
    await send({ type: "rescanPlugins" });
    await send({ type: "addPluginPlace" });
    expect(held.s.plugins.scan).toEqual({ state: "idle" });
  });
});

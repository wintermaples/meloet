// 約束 16: プラグインのスキャン、止まった物の一覧、プラグインの場所。
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

describe("約束 16 の形", () => {
  it("版は 23（22 より後）。新しい曲では、スキャンは動いておらず、止まった物はなく、場所は LV2 と VST3 のふつうの場所だけ", () => {
    expect(PROTOCOL_VERSION).toBe(25);
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

describe("約束 16 の偽物の場面", () => {
  it("scanning（モック R9-Scan）: 棚をひらいたまま、37 / 112", () => {
    const s = state("scanning");
    expect(s.plugins.scan).toEqual({ state: "running", done: 37, total: 112 });
    expect(s.workspace.layout.rightOpen).toContain("shelf");
    expect(mockScreenText("R9-Scan")).toContain("37 / 112");
  });

  it("scanFinished と pluginsFound: 状態行の知らせと、棚の下の数", () => {
    const s = state("scanFinished");
    expect(s.plugins.scan).toEqual({ state: "finished", added: 667, stopped: 4 });
    expect(s.plugins.stopped).toHaveLength(4);
    expect(s.status.message).toEqual({ id: "scanFinished", added: 667, stopped: 4 });
    expect(state("pluginsFound").status.message).toEqual({ id: "pluginsFound", count: 3 });
  });

  it("pluginPlaces（モック R9-Scan）: くわしい設定がひらき、足した場所が 2 つ。CLAP の場所はない", () => {
    const s = state("pluginPlaces");
    expect(s.workspace.settings).toMatchObject({ advancedOpen: true });
    expect(s.plugins.placesOpen).toBe(true);
    const mock = mockScreenText("R9-Scan");
    // /usr/local/lib/vst3 はモックにない（VST3 のふつうの場所として足した。docs/research/plugin-formats.md 8 章）。
    for (const p of s.plugins.places.filter((x) => x.shownPath !== "/usr/local/lib/vst3")) expect(mock, p.shownPath).toContain(p.shownPath);
    expect(s.plugins.places.filter((p) => p.added).map((p) => p.shownPath)).toEqual(["~/音楽/プラグイン", "/opt/surge-xt/lib/vst3"]);
    expect(s.plugins.places.some((p) => p.path.includes("clap"))).toBe(false);
  });

  it("scanStopped（モック F5-ScanStopped）: 名前 ・ 形式 ・ わけ ・ 場所の 4 行", () => {
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

describe("約束 16 の意図（偽物）", () => {
  it("止まった物のダイアログ: ひらく、ためす、閉じる。Esc でも閉じる", async () => {
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

  it("「プラグインの場所…」: ダイアログを閉じ、音の設定をくわしい設定と場所をひらいて出す", async () => {
    const { held, send } = await open("scanStopped");
    await send({ type: "openPluginPlaces" });
    expect(held.s.plugins).toMatchObject({ stoppedOpen: false, placesOpen: true });
    expect(held.s.workspace.settings).toMatchObject({ advancedOpen: true });
    await send({ type: "togglePluginPlaces" });
    expect(held.s.plugins.placesOpen).toBe(false);
  });

  it("場所を外す: 足した場所だけ。ふつうの場所は外れない", async () => {
    const { held, send } = await open("pluginPlaces");
    await send({ type: "removePluginPlace", path: "/usr/lib/lv2" });
    expect(held.s.plugins.places).toHaveLength(8);
    await send({ type: "removePluginPlace", path: "/opt/surge-xt/lib/vst3" });
    expect(held.s.plugins.places.filter((p) => p.added).map((p) => p.shownPath)).toEqual(["~/音楽/プラグイン"]);
    // 偽物はスキャンをしない。
    await send({ type: "rescanPlugins" });
    await send({ type: "addPluginPlace" });
    expect(held.s.plugins.scan).toEqual({ state: "idle" });
  });
});

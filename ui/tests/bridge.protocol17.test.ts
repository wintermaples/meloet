// Protocol 17: 「あとから入れたもの」 on the shelf (SF2 and plugins), plugin instruments and effects, missing things.
import { describe, expect, it } from "vitest";
import { SCENES, createFakeTransport, fakeScene, fakeState, keyCounts, type FakeState } from "../src/bridge/fake";
import { FLUID, fakePluginRows, fluidFile, lv2Key } from "../src/bridge/fakeLibrary";
import {
  GM_FAMILIES,
  PLUGIN_EFFECT_CATEGORIES,
  PLUGIN_FORMATS,
  PLUGIN_INSTRUMENT_CATEGORIES,
  PROTOCOL_VERSION,
  SF2_DRUM_BANK,
  type AppState,
  type Command,
  type ServerMessage,
  type SoundRef,
} from "../src/bridge/types";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { mockScreenText as mockText, specDocument } from "./design-sources";

/** Mocks and the spec put spaces at part borders or not, so compare with all spaces removed. */
const squash = (s: string) => s.replace(/\s+/g, "");
const mockScreenText = (board: string) => squash(mockText(board));
/** Screen names in the spec's English text: 「a」, 「b」, … */
const quotedList = (names: string[]) => squash(names.map((n) => `「${n}」`).join(", "));

let spec: string | null = null;
/** All text of the spec. */
const specText = () => (spec ??= squash(specDocument().body.textContent ?? ""));

const state = (scene: string) => fakeScene(scene)!.state;

async function open(scene: string) {
  const start = state(scene);
  const transport = createFakeTransport(structuredClone(start));
  const held = { s: structuredClone(start) as FakeState };
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

const SURGE: SoundRef = { kind: "plugin", key: lv2Key("Surge XT") };

describe("protocol 17 shape", () => {
  it("version is 23 (after 22). In a new song, 「あとから入れたもの」 is empty and nothing is missing", () => {
    expect(PROTOCOL_VERSION).toBeGreaterThanOrEqual(26);
    const s = fakeState();
    expect(s.library).toEqual({ sf2: [], plugins: [], openedSf2: null, packMissing: false });
    expect(s.workspace.fileDrop).toBeNull();
    expect(s.shelf.missing).toEqual([]);
    expect(s.shelf.favoriteEffects).toEqual([
      { kind: "builtin", effect: "reverb" },
      { kind: "builtin", effect: "delay" },
    ]);
    expect(s.shelf.favoriteSounds).toEqual(s.shelf.favorites.map((instrument) => ({ kind: "builtin", instrument })));
    const t = s.song.tracks[0]!;
    expect([t.sound, t.standIn, t.drumKeys]).toEqual([{ kind: "builtin", instrument: "grandPiano" }, null, null]);
    expect(t.slots).toEqual([null, null, null, null]);
    expect(s.song.master.slots[0]).toEqual({ id: 3, effect: { kind: "builtin", effect: "limiter" }, bypassed: false });
  });

  it("heading order follows spec 3 (16 SF2 families + drum kits, the kinds of 14-A)", () => {
    expect(GM_FAMILIES).toHaveLength(17);
    expect(GM_FAMILIES.at(-1)).toBe("drumKit");
    expect(PLUGIN_EFFECT_CATEGORIES).toEqual(["eq", "dynamics", "reverb", "delay", "modulation", "distortion", "filter", "pitch", "spatial", "analysis", "other"]);
    expect(PLUGIN_INSTRUMENT_CATEGORIES).toEqual(["synth", "sampler", "drum", "keys", "other"]);
    expect(PLUGIN_FORMATS).toEqual(["vst3", "lv2"]);
    expect(SF2_DRUM_BANK).toBe(128);
    // As many as the headings in the spec (names are screen wording).
    const spec = specText();
    expect(spec).toContain(quotedList(["ピアノ", "マレット", "オルガン", "ギター", "ベース", "弦", "アンサンブル", "金管", "木管", "笛", "シンセリード", "シンセパッド", "シンセの効果", "民族楽器", "打楽器", "効果音", "ドラムのキット"]));
    expect(spec).toContain(quotedList(["EQ", "ダイナミクス", "リバーブ", "ディレイ", "モジュレーション", "ひずみ", "フィルター", "ピッチ", "空間", "分析", "そのほか"]));
    expect(spec).toContain(quotedList(["シンセ", "サンプラー", "ドラム", "ピアノ ・ 鍵盤", "そのほか"]));
  });

  it("count for grouping the same sound (85): from the low notes, up to 5", () => {
    expect(keyCounts([38, 36, 38, 36, 36, 36])).toEqual([
      { key: 36, count: 4 },
      { key: 38, count: 2 },
    ]);
    expect(keyCounts([60, 62, 64, 65, 67, 69])).toHaveLength(5);
    const drums = state("drumRoll").status.message;
    expect(drums?.id === "summary" && drums.notes?.names).toEqual([{ key: 38, count: 2 }]);
  });
});

describe("fake contents (the mock's numbers)", () => {
  it("FluidR3_GM.sf2 has 137 sounds (128 + 9 drum kits). 8 per family, in GM number order", () => {
    const file = fluidFile();
    expect(file.file).toBe("FluidR3_GM.sf2");
    expect(file.presets).toHaveLength(137);
    expect(mockScreenText("F5-SF2Shelf")).toContain(squash("FluidR3_GM.sf2 SF2 137"));
    for (const family of GM_FAMILIES.slice(0, 16)) expect(file.presets.filter((p) => p.family === family)).toHaveLength(8);
    const kits = file.presets.filter((p) => p.family === "drumKit");
    expect(kits).toHaveLength(9);
    expect(kits.every((p) => p.bank === SF2_DRUM_BANK)).toBe(true);
    const guitars = file.presets.filter((p) => p.family === "guitar");
    expect(guitars.map((p) => `${p.name} ${p.program + 1}`)).toEqual([
      "Nylon String Guitar 25",
      "Steel String Guitar 26",
      "Jazz Guitar 27",
      "Clean Guitar 28",
      "Palm Muted Guitar 29",
      "Overdrive Guitar 30",
      "Distortion Guitar 31",
      "Guitar Harmonics 32",
    ]);
    const mock = mockScreenText("F5-SF2Shelf");
    for (const g of guitars) expect(mock).toContain(squash(`${g.name} ${g.program + 1}`));
    expect(file.presets.map((p) => GM_FAMILIES.indexOf(p.family) * 1000 + p.program)).toEqual(
      [...file.presets].map((p) => GM_FAMILIES.indexOf(p.family) * 1000 + p.program).sort((a, b) => a - b),
    );
  });

  it("plugins are the 506 effects of F5-ShelfFx and the 34 instruments of F5-PluginLoading. In name order", () => {
    const rows = fakePluginRows();
    const effects = rows.filter((r) => r.kind.role === "effect");
    const instruments = rows.filter((r) => r.kind.role === "instrument");
    expect([effects.length, instruments.length]).toEqual([506, 34]);
    const count = (category: string) => rows.filter((r) => r.kind.category === category && r.kind.role === "effect").length;
    expect(PLUGIN_EFFECT_CATEGORIES.map(count)).toEqual([64, 88, 23, 31, 52, 40, 36, 9, 14, 70, 79]);
    const fx = mockScreenText("F5-ShelfFx");
    for (const heading of ["EQ 64", "ダイナミクス 88", "リバーブ 23", "ディレイ 31", "そのほか 79"]) expect(fx).toContain(squash(heading));
    expect(PLUGIN_INSTRUMENT_CATEGORIES.map((c) => instruments.filter((r) => r.kind.category === c).length)).toEqual([21, 6, 4, 2, 1]);
    expect(mockScreenText("F5-PluginLoading")).toContain(squash("シンセ 21"));
    const names = rows.map((r) => r.name.toLowerCase());
    expect(names).toEqual([...names].sort());
    // Every row has the key of the format it uses in formats, and keys differ per row.
    for (const r of rows) expect(r.formats.map((f) => f.key)).toContain(r.key);
    const keys = rows.flatMap((r) => r.formats.map((f) => f.key));
    expect(new Set(keys).size).toBe(keys.length);
    // Mock rows. The + counts are 1 less, as CLAP is not included.
    const dragonfly = rows.find((r) => r.name === "Dragonfly Hall Reverb")!;
    expect([dragonfly.vendor, dragonfly.kind, dragonfly.formats.map((f) => f.format)]).toEqual(["Michael Willis", { role: "effect", category: "reverb" }, ["vst3", "lv2"]]);
    expect(dragonfly.key).toBe(lv2Key("Dragonfly Hall Reverb"));
    expect(fx).toContain(squash("Dragonfly Hall Reverb Michael Willis LV2 ＋2"));
    const vital = rows.find((r) => r.name === "Vital")!;
    expect(vital.key.startsWith("vst3:")).toBe(true);
    expect(vital.key).toMatch(/^vst3:[0-9A-F]{32}$/);
  });
});

describe("protocol 17 fake scenes", () => {
  it("in every scene, the new song and shelf fields match the old ones", () => {
    for (const name of Object.keys(SCENES)) {
      const s = state(name);
      const strips = [...s.song.tracks, ...s.song.returns, s.song.master];
      for (const strip of strips) {
        strip.inserts.forEach((effect, i) => {
          const slot = strip.slots[i]!;
          if (effect !== null) expect(slot?.effect, name).toEqual({ kind: "builtin", effect });
          else expect(slot === null || slot.effect.kind === "plugin", name).toBe(true);
        });
      }
      for (const t of s.song.tracks) {
        if (t.sound.kind === "builtin") expect(t.sound.instrument, name).toBe(t.instrument);
        else expect(t.instrument, name).toBe("sine");
      }
      const sh = s.shelf;
      if (sh.selected !== null) expect(sh.selectedSound, name).toEqual({ kind: "builtin", instrument: sh.selected });
      if (sh.selectedEffect !== null) expect(sh.chosenEffect, name).toEqual({ kind: "builtin", effect: sh.selectedEffect });
      expect(sh.favoriteSounds.filter((x) => x.kind === "builtin"), name).toEqual(sh.favorites.map((instrument) => ({ kind: "builtin", instrument })));
    }
  });

  it("sf2Shelf (mock F5-SF2Shelf): the added file's heading opens, auditioning Clean Guitar", () => {
    const s = state("sf2Shelf");
    expect(s.library.sf2.map((f) => [f.file, f.presets.length])).toEqual([[FLUID, 137]]);
    expect(s.library.openedSf2).toBe(FLUID);
    expect(s.library.plugins).toEqual([]);
    expect(s.shelf.selectedSound).toEqual({ kind: "sf2", file: FLUID, bank: 0, program: 27 });
    expect(s.shelf.selected).toBeNull();
    expect(s.transport.canRecord).toBe(false);
    expect(s.status.message).toEqual({ id: "auditionNewTrackSound", sound: { kind: "sf2", name: "Clean Guitar", file: FLUID } });
    expect(mockScreenText("F5-SF2Shelf")).toContain(squash("Clean Guitar ・ FluidR3_GM.sf2 ・ まだトラックには入っていません ・ 入れるには Enter"));
    expect(state("sf2Added").status.message).toEqual({ id: "sf2Added", file: FLUID, presets: 128, kits: 9, skipped: 0 });
    expect(specText()).toContain(squash("FluidR3_GM.sf2 を足しました。音色 128 ・ ドラムのキット 9"));
    expect(state("sf2Unreadable").status.message).toEqual({ id: "sf2Unreadable", file: FLUID });
    expect(state("sf2Unreadable").library.sf2).toEqual([]);
  });

  it("sf2DropAddable / sf2DropNotAddable (mock F5-SF2Drop): can add / cannot add. Esc cancels", async () => {
    const s = state("sf2DropAddable");
    expect(s.workspace.fileDrop).toEqual({ name: FLUID, addable: true, files: 1, sf2: 1, audio: 0 });
    expect(s.status.message).toEqual({ id: "sf2Dragging" });
    expect(s.library.sf2).toEqual([]);
    const mock = mockScreenText("F5-SF2Drop");
    expect(mock).toContain(squash("SF2 のファイルを持ってきています"));
    expect(mock).toContain(squash("FluidR3_GM.sf2 ここに落とすと、棚に SF2 を足します。"));
    expect(state("sf2DropNotAddable").workspace.fileDrop?.addable).toBe(false);
    expect(state("sf2DropNotAddable").status.message).toEqual({ id: "notSf2Dragging" });
    expect(state("sf2DropMany").workspace.fileDrop).toEqual({ name: FLUID, addable: true, files: 3, sf2: 2, audio: 0 });
    expect(specText()).toContain(squash("SF2 のファイルではないので、足せません。"));
    const { held, send } = await open("sf2DropAddable");
    await send({ type: "cancelFileDrop" });
    expect(held.s.workspace.fileDrop).toBeNull();
  });

  it("shelfFx (mock F5-ShelfFx): inserts with plugins, a stopped slot, the chosen effect", () => {
    const s = state("shelfFx");
    const surge = s.song.tracks[4]!;
    expect([surge.number, surge.name, surge.instrument]).toEqual([5, "Surge XT", "sine"]);
    expect(surge.sound).toEqual({ kind: "plugin", key: lv2Key("Surge XT"), name: "Surge XT", format: "lv2", state: "ready" });
    expect(surge.standIn).toBeNull();
    expect(s.shelf.mode).toEqual({ kind: "insertEffect", strip: { kind: "track", id: surge.id }, slot: 1 });
    expect(s.shelf.chosenEffect).toEqual({ kind: "plugin", key: lv2Key("Dragonfly Hall Reverb") });
    expect(s.shelf.selectedEffect).toBeNull();
    const names = (i: number) => s.song.tracks[i]!.slots.map((x) => (x === null ? null : x.effect.kind === "builtin" ? x.effect.effect : `${x.effect.name}:${x.effect.state}`));
    expect(names(0)).toEqual(["eq4", "compressor", null, null]);
    expect(names(2)).toEqual(["compressor", "Brick EQ:stopped", null, null]);
    expect(names(3)).toEqual(["MVerb:ready", "eq4", null, null]);
    expect(names(4)).toEqual(["Calf Reverb:ready", null, null, null]);
    expect(s.song.tracks[3]!.inserts).toEqual([null, "eq4", null, null]);
    const mock = mockScreenText("F5-ShelfFx");
    for (const text of ["5 Surge XT にエフェクトを入れる", "Brick EQ 止", "5 Surge XT のインサート 2 に入れます。", "エフェクト 512"]) expect(mock).toContain(squash(text));
    expect(s.library.plugins.filter((r) => r.kind.role === "effect").length + 6).toBe(512);
    // F5-ShelfSearch is the same shelf. The search text is held by the screen.
    const search = state("shelfSearch");
    expect(search.shelf.chosenEffect).toBeNull();
    expect(search.library).toEqual(s.library);
    expect(search.library.plugins.filter((r) => r.vendor.includes("Michael Willis") && r.name.includes("Dragonfly"))).toHaveLength(4);
    expect(mockScreenText("F5-ShelfSearch")).toContain(squash("さがす「Dragonfly」・ 合う物 4 個"));
  });

  it("pluginLoading / pluginAudition / pluginUnreadable (mocks F5-PluginLoading / F5-PluginAudition)", () => {
    const loading = state("pluginLoading");
    expect([loading.shelf.selectedSound, loading.shelf.loadingSound]).toEqual([SURGE, SURGE]);
    expect(loading.status.message).toEqual({ id: "pluginLoading", name: "Surge XT" });
    expect(mockScreenText("F5-PluginLoading")).toContain(squash("Surge XT を読みこんでいます… ・ やめるには Esc"));
    const audition = state("pluginAudition");
    expect([audition.shelf.selectedSound, audition.shelf.loadingSound]).toEqual([SURGE, null]);
    expect(audition.transport.canRecord).toBe(false);
    expect(audition.status.message).toEqual({ id: "auditionNewTrackSound", sound: { kind: "plugin", name: "Surge XT" } });
    const mock = mockScreenText("F5-PluginAudition");
    expect(mock).toContain(squash("Surge XT ・ まだトラックには入っていません ・ 入れるには Enter"));
    expect(mock).toContain(squash("窓をひらく"));
    const failed = state("pluginUnreadable");
    expect([failed.shelf.selectedSound, failed.shelf.unreadable]).toEqual([null, SURGE]);
    expect(specText()).toContain(squash("Surge XT を読みこめませんでした。"));
  });

  it("pluginTrack (mock F5-PluginTrack): the name is the plugin name, the chosen track, 2 steps undone", () => {
    const s = state("pluginTrack");
    const surge = s.song.tracks[4]!;
    expect(s.song.playTarget).toBe(surge.id);
    expect(surge.name).toBe("Surge XT");
    expect([s.song.undoCount, s.song.redoCount, s.song.file]).toEqual([14, 2, "changed"]);
    expect(s.workspace.layout.lower.map((l) => l.band)).toEqual(["history"]);
    expect(mockScreenText("F5-PluginTrack")).toContain(squash("5 Surge XT"));
  });

  it("missingPack (mock R8-Missing): the name stays, sine stand-in, the shelf has only the sine", () => {
    const s = state("missingPack");
    expect(s.library.packMissing).toBe(true);
    expect(s.shelf.available).toEqual(["sine"]);
    expect(s.song.tracks.map((t) => [t.instrument, t.standIn])).toEqual([
      ["grandPiano", "packMissing"],
      ["fingerBass", "packMissing"],
      ["studioKit", "packMissing"],
      ["strings", "packMissing"],
    ]);
    expect(s.status.message).toEqual({ id: "soundsMissing", tracks: 4 });
    const mock = mockScreenText("R8-Missing");
    expect(mock).toContain(squash("音の素材が見つからないので、4 本のトラックをサイン波で鳴らしています。"));
    expect(mock).toContain(squash("サイン波で代わり"));
  });

  it("missingSf2 / missingPlugin: notice and buttons. 「曲から外す」 turns it into a sine and the notice goes away", async () => {
    const sf2 = state("missingSf2");
    expect(sf2.shelf.missing).toEqual([{ kind: "sf2", file: FLUID, tracks: 2 }]);
    expect(sf2.song.tracks.filter((t) => t.standIn === "sf2Missing").map((t) => t.sound.kind)).toEqual(["sf2", "sf2"]);
    expect(sf2.status.message).toEqual({ id: "sf2Missing", file: FLUID, tracks: 2 });
    expect(specText()).toContain(squash("曲で使っている FluidR3_GM.sf2 が見つかりません。2 本のトラックはサイン波で鳴らしています。"));
    const a = await open("missingSf2");
    await a.send({ type: "removeMissing", missing: { kind: "sf2", file: FLUID } });
    expect(a.held.s.shelf.missing).toEqual([]);
    expect(a.held.s.song.tracks.map((t) => [t.sound.kind, t.instrument, t.standIn])).toEqual([
      ["builtin", "sine", null],
      ["builtin", "fingerBass", null],
      ["builtin", "studioKit", null],
      ["builtin", "sine", null],
    ]);
    expect(a.held.s.song.tracks[0]!.name).toBe("ギター");

    const plugin = state("missingPlugin");
    expect(plugin.shelf.missing.map((m) => m.kind === "plugin" && [m.name, m.role, m.shownPath])).toEqual([
      ["Surge XT", "instrument", "/usr/lib/lv2/Surge XT.lv2"],
      ["MVerb", "effect", "~/.lv2/MVerb.lv2"],
    ]);
    expect(plugin.song.tracks[4]!.standIn).toBe("pluginMissing");
    expect(plugin.song.tracks[4]!.sound).toMatchObject({ kind: "plugin", state: "missing" });
    expect(plugin.song.tracks[3]!.slots[0]).toMatchObject({ effect: { kind: "plugin", name: "MVerb", state: "missing" } });
    expect(specText()).toContain(squash("前はここにありました: （場所）。曲の中の設定は残しています。"));
    const b = await open("missingPlugin");
    await b.send({ type: "removeMissing", missing: { kind: "plugin", key: lv2Key("MVerb") } });
    expect(b.held.s.shelf.missing).toHaveLength(1);
    expect(b.held.s.song.tracks[3]!.slots).toEqual([null, expect.objectContaining({ effect: { kind: "builtin", effect: "eq4" }, bypassed: false }), null, null]);
    expect(b.held.s.song.tracks[4]!.standIn).toBe("pluginMissing");
  });

  it("drumPlay (mock R8-DrumPlay) and drumRollSf2: the position while playing, the SF2 kit rows", () => {
    const play = fakeScene("drumPlay")!;
    expect(play.state.transport.state).toBe("playing");
    expect(play.meters.position).toMatchObject({ tick: 4 * TPQ + TPQ, state: "playing" });
    expect(play.state.arrange.pianoRollClip).not.toBeNull();
    expect(mockScreenText("R8-DrumPlay")).toContain(squash("鳴っている音 クローズハイハット スネア"));
    const kit = state("drumRollSf2").song.tracks[2]!;
    expect(kit.sound).toEqual({ kind: "sf2", file: FLUID, bank: 128, program: 0, name: "Standard", family: "drumKit" });
    expect(kit.drumKeys).toHaveLength(47);
    expect(kit.drumKeys!.slice(0, 10)).toEqual([49, 57, 55, 52, 51, 59, 53, 46, 44, 42]);
    expect(kit.drumKeys!.slice(18, 25)).toEqual([40, 37, 36, 35, 54, 56, 58]);
    expect(specText()).toContain(squash("35 「キック 2」, 36 「キック」, 37 「サイドスティック」, 38 「スネア」, 39 「クラップ」, 40 「スネア 2」"));
  });
});

describe("protocol 17 intents (fake)", () => {
  it("shelfSelectSound: built-in sounds are the same as shelfSelect. SF2 and plugins only if on the shelf", async () => {
    const { held, send } = await open("shelfPlugins");
    await send({ type: "shelfSelectSound", sound: { kind: "builtin", instrument: "synthBass" } });
    expect([held.s.shelf.selected, held.s.shelf.selectedSound]).toEqual(["synthBass", { kind: "builtin", instrument: "synthBass" }]);
    expect(held.s.status.message).toEqual({ id: "auditionNewTrack", instrument: "synthBass" });
    await send({ type: "shelfSelectSound", sound: SURGE });
    expect([held.s.shelf.selected, held.s.shelf.selectedSound]).toEqual([null, SURGE]);
    expect(held.s.transport.canRecord).toBe(false);
    expect(held.s.status.message).toEqual({ id: "auditionNewTrackSound", sound: { kind: "plugin", name: "Surge XT" } });
    const guitar: SoundRef = { kind: "sf2", file: FLUID, bank: 0, program: 27 };
    await send({ type: "shelfSelectSound", sound: guitar });
    expect(held.s.shelf.selectedSound).toEqual(guitar);
    expect(held.s.status.message).toEqual({ id: "auditionNewTrackSound", sound: { kind: "sf2", name: "Clean Guitar", file: FLUID } });
    // Things not on the shelf (an effect plugin, a missing preset) are the same as canceling.
    await send({ type: "shelfSelectSound", sound: { kind: "plugin", key: lv2Key("MVerb") } });
    expect(held.s.shelf.selectedSound).toBeNull();
    await send({ type: "shelfSelectSound", sound: guitar });
    await send({ type: "shelfCancel" });
    expect(held.s.shelf.selectedSound).toBeNull();
    expect(held.s.transport.canRecord).toBe(true);
    expect(held.s.status.message?.id).toBe("summary");
    // Closing the shelf ends the audition. The stars stay.
    await send({ type: "toggleFavoriteSound", sound: SURGE });
    await send({ type: "shelfSelectSound", sound: SURGE });
    await send({ type: "toggleBand", band: "shelf" });
    expect(held.s.shelf.selectedSound).toBeNull();
    expect(held.s.shelf.favoriteSounds).toContainEqual(SURGE);
  });

  it("favorites: instruments (any) and effects (86-4). The star stays on the row when the format changes", async () => {
    const { held, send } = await open("shelfPlugins");
    await send({ type: "toggleFavoriteSound", sound: { kind: "builtin", instrument: "organ" } });
    expect(held.s.shelf.favorites).toEqual(["grandPiano", "organ", "studioKit"]);
    await send({ type: "toggleFavoriteSound", sound: SURGE });
    expect(held.s.shelf.favoriteSounds.at(-1)).toEqual(SURGE);
    expect(held.s.shelf.favoriteSounds.slice(0, 3)).toEqual(held.s.shelf.favorites.map((instrument) => ({ kind: "builtin", instrument })));
    const row = held.s.library.plugins.find((r) => r.name === "Surge XT")!;
    const vst3 = row.formats.find((f) => f.format === "vst3")!.key;
    await send({ type: "usePluginFormat", key: vst3 });
    expect(held.s.library.plugins.find((r) => r.name === "Surge XT")!.key).toBe(vst3);
    expect(held.s.shelf.favoriteSounds.at(-1)).toEqual({ kind: "plugin", key: vst3 });
    await send({ type: "usePluginFormat", key: "vst3:NOPE" });
    expect(held.s.library.plugins.find((r) => r.name === "Surge XT")!.key).toBe(vst3);

    await send({ type: "toggleFavoriteEffect", effect: { kind: "builtin", effect: "reverb" } });
    await send({ type: "toggleFavoriteEffect", effect: { kind: "plugin", key: lv2Key("MVerb") } });
    expect(held.s.shelf.favoriteEffects).toEqual([
      { kind: "builtin", effect: "delay" },
      { kind: "plugin", key: lv2Key("MVerb") },
    ]);
    await send({ type: "toggleFavoriteEffect", effect: { kind: "plugin", key: "lv2:urn:nowhere" } });
    expect(held.s.shelf.favoriteEffects).toHaveLength(2);
  });

  it("shelfChooseEffect: only for the effect list. Built-in effects are the same as shelfSelectEffect", async () => {
    const { held, send } = await open("shelfFx");
    await send({ type: "shelfChooseEffect", effect: { kind: "builtin", effect: "chorus" } });
    expect([held.s.shelf.selectedEffect, held.s.shelf.chosenEffect]).toEqual(["chorus", { kind: "builtin", effect: "chorus" }]);
    const mverb = { kind: "plugin", key: lv2Key("MVerb") } as const;
    await send({ type: "shelfChooseEffect", effect: mverb });
    expect([held.s.shelf.selectedEffect, held.s.shelf.chosenEffect]).toEqual([null, mverb]);
    await send({ type: "shelfChooseEffect", effect: { kind: "plugin", key: lv2Key("Surge XT") } });
    expect(held.s.shelf.chosenEffect, "an instrument is not in the effect list").toBeNull();
    await send({ type: "shelfChooseEffect", effect: mverb });
    await send({ type: "shelfChooseEffect", effect: null });
    expect(held.s.shelf.chosenEffect).toBeNull();
    const other = await open("shelfPlugins");
    await other.send({ type: "shelfChooseEffect", effect: mverb });
    expect(other.held.s.shelf.chosenEffect).toBeNull();
  });

  it("intents with no content yet change nothing", async () => {
    const { held, send } = await open("pluginAudition");
    const before = structuredClone(held.s);
    for (const command of [{ type: "openPluginWindow", slot: null }, { type: "addSf2" }, { type: "playSampleOf", sound: SURGE }] satisfies Command[]) {
      await send(command);
      expect(held.s).toEqual(before);
    }
  });
});

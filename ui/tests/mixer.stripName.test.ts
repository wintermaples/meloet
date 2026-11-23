import { describe, expect, it } from "vitest";
import type { EffectKind, LibraryView, PluginRowView, SongView } from "../src/bridge/types";
import { stripName } from "../src/components/mixer/stripName";

const slots = (effects: (EffectKind | null)[]) =>
  effects.map((effect) => (effect === null ? null : { effect: { kind: "builtin", effect }, bypassed: false }));
const song = (returnA: (EffectKind | null)[]) =>
  ({
    tracks: [{ id: 7, number: 2, name: "ベース" }],
    returns: [
      { gainDb: 0, pan: 0, muted: false, slots: slots(returnA) },
      { gainDb: 0, pan: 0, muted: false, slots: slots(["delay", null, null, null]) },
    ],
  }) as unknown as SongView;

const library = { sf2: [], plugins: [] } as unknown as LibraryView;
const row = (key: string, name: string, category: string) =>
  ({ name, vendor: "", kind: { role: "effect", category }, key, formats: [{ format: "lv2", key }] }) as unknown as PluginRowView;

describe("strip names (B-86)", () => {
  it("a track is its number and name", () => {
    expect(stripName(song([null, null, null, null]), { kind: "track", id: 7 }, library)).toBe("2 ベース");
    expect(stripName(song([null, null, null, null]), { kind: "track", id: 99 }, library)).toBe("");
  });
  it("a return is its letter and the top effect. Only the letter when empty", () => {
    expect(stripName(song(["reverb", null, null, null]), { kind: "return", bus: "a" }, library)).toBe("A リバーブ");
    expect(stripName(song([null, "chorus", "reverb", null]), { kind: "return", bus: "a" }, library)).toBe("A コーラス");
    expect(stripName(song([null, null, null, null]), { kind: "return", bus: "a" }, library)).toBe("A");
    expect(stripName(song([null, null, null, null]), { kind: "return", bus: "b" }, library)).toBe("B ディレイ");
    // a plugin keeps its name (only the new slots field is read).
    const s = song([null, null, null, null]);
    s.returns[0].slots[1] = { id: 8, effect: { kind: "plugin", key: "lv2:x", name: "MVerb", format: "lv2", state: "ready" }, bypassed: false };
    expect(stripName(s, { kind: "return", bus: "a" }, library)).toBe("A MVerb");
  });
  it("when the top of a return is a plugin, the kind name of 14-A. Only when the kind is 「そのほか」, the plugin name (user:2026-11-19)", () => {
    const s = song([null, null, null, null]);
    s.returns[0].slots[0] = { id: 8, effect: { kind: "plugin", key: "lv2:dragonfly", name: "Dragonfly Reverb", format: "lv2", state: "ready" }, bypassed: false };
    const withRows = (category: string) => ({ ...library, plugins: [row("lv2:dragonfly", "Dragonfly Reverb", category)] }) as LibraryView;
    expect(stripName(s, { kind: "return", bus: "a" }, withRows("reverb"))).toBe("A リバーブ");
    expect(stripName(s, { kind: "return", bus: "a" }, withRows("other"))).toBe("A Dragonfly Reverb");
  });
  it("master", () => {
    expect(stripName(song([null, null, null, null]), { kind: "master" }, library)).toBe("マスター");
  });
});

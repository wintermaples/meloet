// Protocol 37 (the owner, 2026-11-25): the keys a plugin instrument names (TrackView.keyNames), and its drum rows (drumKeys).
import { describe, expect, it } from "vitest";
import { PROTOCOL_VERSION, type PluginKeyName, type TrackView } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";

const tracks = (scene: string): TrackView[] => fakeScene(scene)!.state.song.tracks;

describe("shape of protocol 37", () => {
  it("version 37; a named key is its key and the plugin's name", () => {
    expect(PROTOCOL_VERSION).toBe(37);
    const name: PluginKeyName = { key: 36, name: "Kick Drum" };
    expect(name).toEqual({ key: 36, name: "Kick Drum" });
  });

  it("no names on tracks that are not a plugin naming its keys; names and rows go together", () => {
    for (const scene of ["drumRoll", "drumRollSf2", "drumRollPluginUnnamed"]) {
      for (const t of tracks(scene)) expect(t.keyNames, `${scene} ${t.name}`).toBeNull();
    }
    const kit = tracks("drumRollPlugin")[2]!;
    expect(kit.sound.kind).toBe("plugin");
    expect([...kit.drumKeys!].sort((a, b) => a - b)).toEqual(kit.keyNames!.map((n) => n.key));
    expect(tracks("drumRollPluginUnnamed")[2]!.drumKeys).toBeNull();
  });
});

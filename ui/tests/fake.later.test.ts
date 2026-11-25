// The fake answers as Rust does (UI-BRIDGE #6, FIX-fix-ui-text #1 of the later list).
import { describe, expect, it } from "vitest";
import { fakeScene } from "../src/bridge/fake";
import { FAKE_TABLES, FakeParamTables } from "../src/bridge/fakeParams";

describe("the fake", () => {
  it("a vocal clip lists its shown take in audio.takes", () => {
    const clip = fakeScene("stretch")!.state.song.tracks.flatMap((t) => t.clips).find((c) => c.audio !== null)!;
    expect(clip.audio!.takes.map((x) => [x.takeId, x.number, x.audio])).toEqual([[clip.takeId, 1, clip.audio!.audio]]);
  });

  it("a typed plugin value past an end gives its range joined as in the language", () => {
    const tables = new FakeParamTables();
    const id = tables.table(FAKE_TABLES.dragonfly)!.groups[0]!.params[0]!.id;
    expect(tables.type(FAKE_TABLES.dragonfly, id, "200", "ja").clamped!.range).toContain("〜");
    const en = tables.type(FAKE_TABLES.dragonfly, id, "200", "en").clamped!.range;
    expect([en.includes("–"), en.includes("〜")]).toEqual([true, false]);
  });
});

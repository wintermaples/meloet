import { describe, expect, it } from "vitest";
import type { SongView } from "../src/bridge/types";
import { stripName } from "../src/components/mixer/stripName";

const song = (returnA: SongView["returns"][0]["inserts"]) =>
  ({
    tracks: [{ id: 7, number: 2, name: "ベース" }],
    returns: [
      { gainDb: 0, pan: 0, muted: false, inserts: returnA },
      { gainDb: 0, pan: 0, muted: false, inserts: ["delay", null, null, null] },
    ],
  }) as unknown as SongView;

describe("ストリップの名前（B-86）", () => {
  it("トラックは番号と名前", () => {
    expect(stripName(song([null, null, null, null]), { kind: "track", id: 7 })).toBe("2 ベース");
    expect(stripName(song([null, null, null, null]), { kind: "track", id: 99 })).toBe("");
  });
  it("戻りは文字といちばん上のエフェクト。空なら文字だけ", () => {
    expect(stripName(song(["reverb", null, null, null]), { kind: "return", bus: "a" })).toBe("A リバーブ");
    expect(stripName(song([null, "chorus", "reverb", null]), { kind: "return", bus: "a" })).toBe("A コーラス");
    expect(stripName(song([null, null, null, null]), { kind: "return", bus: "a" })).toBe("A");
    expect(stripName(song([null, null, null, null]), { kind: "return", bus: "b" })).toBe("B ディレイ");
  });
  it("マスター", () => {
    expect(stripName(song([null, null, null, null]), { kind: "master" })).toBe("マスター");
  });
});

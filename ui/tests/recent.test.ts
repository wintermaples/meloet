// A5 「ファイル ▸ 最近ひらいた曲 ▸」: the fake's copy of what Rust does with the list.
import { describe, expect, it } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";

async function opened() {
  const t = createFakeTransport(fakeScene("recentSongs")!.state);
  await t.subscribe(
    () => {},
    () => {},
  );
  const now = async () => (await t.snapshot()).state as AppState;
  return { now, send: (command: Command) => t.send(command) };
}

describe("recent songs in the fake (A5)", () => {
  it("the scene has songs newest first, by name and place; a row opens nothing in the fake", async () => {
    const { now, send } = await opened();
    const before = await now();
    expect(before.recent.map((r) => r.name)).toEqual(["はじめての曲", "夜のピアノ", "ドラムの練習"]);
    expect(before.recent.every((r) => r.place.startsWith("~/") && r.place.endsWith("/"))).toBe(true);
    await send({ type: "openRecent", id: before.recent[1]!.id });
    expect((await now()).recent).toEqual(before.recent);
  });

  it("一覧を消す empties the list", async () => {
    const { now, send } = await opened();
    await send({ type: "clearRecent" });
    expect((await now()).recent).toEqual([]);
  });
});

// 約束 29（第 10 回）: トラックの見出しの「ピアノロールでひらく」と、「AI とつなぐ」の切りかえ。形と偽物の意図と場面。
import { describe, expect, it } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { PROTOCOL_VERSION } from "../src/bridge";
import { createFakeTransport, fakeScene, fakeState } from "../src/bridge/fake";

async function opened(scene = "tempoFolded") {
  const t = createFakeTransport(fakeScene(scene)!.state);
  await t.subscribe(
    () => {},
    () => {},
  );
  const now = async () => (await t.snapshot()).state as AppState;
  const send = async (command: Command) => {
    await t.send(command);
    return now();
  };
  return { now, send };
}

const pianoRollOpen = (s: AppState) => s.workspace.layout.lower.some((b) => b.band === "pianoRoll");

describe("約束 29 の形", () => {
  it("版は 29。新しい曲では「AI とつなぐ」は切れていて、口は閉じている", () => {
    expect(PROTOCOL_VERSION).toBe(29);
    expect(fakeState().aiLink).toEqual({ on: false, port: "closed" });
  });
});

describe("偽物の意図", () => {
  it("「AI とつなぐ」: オンで「待っています」、オフで「閉じています」。曲は変わらない", async () => {
    const { now, send } = await opened();
    const before = (await now()).song;
    let s = await send({ type: "setAiLink", on: true });
    expect(s.aiLink).toEqual({ on: true, port: "waiting" });
    expect(s.song).toEqual(before);
    s = await send({ type: "setAiLink", on: false });
    expect(s.aiLink).toEqual({ on: false, port: "closed" });
  });

  it("「ピアノロールでひらく」: 再生の線のクリップを見せ、なければクリップなし。帯はひらいたまま。手ではない", async () => {
    const { now, send } = await opened();
    const first = await now();
    const at = first.transport.position;
    const keys = first.song.tracks.filter((t) => t.kind.kind === "keys");
    const withClip = keys.find((t) => t.clips.some((c) => c.start <= at && at < c.end))!;
    const without = keys.find((t) => !t.clips.some((c) => c.start <= at && at < c.end))!;
    expect(withClip && without).toBeTruthy();
    expect(pianoRollOpen(first)).toBe(false);

    let s = await send({ type: "openPianoRoll", track: withClip.id });
    const clip = withClip.clips.find((c) => c.start <= at && at < c.end)!;
    expect(pianoRollOpen(s)).toBe(true);
    expect([s.arrange.pianoRollClip, s.arrange.selectedClips, s.pianoRoll.take]).toEqual([clip.id, [clip.id], clip.takeId]);
    expect([s.song.playTarget, s.song.keysTarget]).toEqual([withClip.id, withClip.id]);

    s = await send({ type: "openPianoRoll", track: without.id });
    expect(pianoRollOpen(s)).toBe(true);
    expect([s.arrange.pianoRollClip, s.arrange.selectedClips, s.pianoRoll.take]).toEqual([null, [], null]);
    expect(s.song.keysTarget).toBe(without.id);
    expect(s.song.undoCount).toBe(first.song.undoCount);
  });

  it("歌のトラックでは何もしない", async () => {
    const { now, send } = await opened("multiClip");
    const first = await now();
    const vocal = first.song.tracks.find((t) => t.kind.kind === "vocal")!;
    expect(vocal).toBeTruthy();
    const s = await send({ type: "openPianoRoll", track: vocal.id });
    expect(s).toEqual(first);
  });
});

describe("場面", () => {
  it("くわしい設定の 3 つの状態", () => {
    expect(fakeScene("aiLinkOff")!.state.aiLink).toEqual({ on: false, port: "closed" });
    expect(fakeScene("aiLinkWaiting")!.state.aiLink).toEqual({ on: true, port: "waiting" });
    const connected = fakeScene("aiLinkConnected")!.state;
    expect(connected.aiLink).toEqual({ on: true, port: "connected" });
    expect(connected.workspace.settings?.advancedOpen).toBe(true);
  });
});

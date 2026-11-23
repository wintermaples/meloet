// Protocol 29: 「ピアノロールでひらく」 in the track head and the 「AI とつなぐ」 toggle. The shapes, fake intents and scenes.
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

describe("shape of protocol 29", () => {
  it("version 29. In a new song 「AI とつなぐ」 is off and the door is closed", () => {
    expect(PROTOCOL_VERSION).toBe(29);
    expect(fakeState().aiLink).toEqual({ on: false, port: "closed" });
  });
});

describe("fake intents", () => {
  it("「AI とつなぐ」: on gives 「待っています」, off gives 「閉じています」. The song does not change", async () => {
    const { now, send } = await opened();
    const before = (await now()).song;
    let s = await send({ type: "setAiLink", on: true });
    expect(s.aiLink).toEqual({ on: true, port: "waiting" });
    expect(s.song).toEqual(before);
    s = await send({ type: "setAiLink", on: false });
    expect(s.aiLink).toEqual({ on: false, port: "closed" });
  });

  it("「ピアノロールでひらく」: shows the clip under the play line, or no clip if there is none. The band stays open. Not a step", async () => {
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

  it("does nothing on a vocal track", async () => {
    const { now, send } = await opened("multiClip");
    const first = await now();
    const vocal = first.song.tracks.find((t) => t.kind.kind === "vocal")!;
    expect(vocal).toBeTruthy();
    const s = await send({ type: "openPianoRoll", track: vocal.id });
    expect(s).toEqual(first);
  });
});

describe("scenes", () => {
  it("the three states in the advanced settings", () => {
    expect(fakeScene("aiLinkOff")!.state.aiLink).toEqual({ on: false, port: "closed" });
    expect(fakeScene("aiLinkWaiting")!.state.aiLink).toEqual({ on: true, port: "waiting" });
    const connected = fakeScene("aiLinkConnected")!.state;
    expect(connected.aiLink).toEqual({ on: true, port: "connected" });
    expect(connected.workspace.settings?.advancedOpen).toBe(true);
  });
});

// Protocol 39 (bug sweep, 2026-11-25): the kind of the first clip kept for 「貼りつけ」 (ArrangeView.clipboardKind).
import { describe, expect, it } from "vitest";
import { PROTOCOL_VERSION, type ArrangeView, type ClipKind } from "../src/bridge";
import { fakeScene, reduceClips } from "../src/bridge/fake";

describe("shape of protocol 39", () => {
  it("version 39; the kind is keys or vocal, null with nothing kept", () => {
    expect(PROTOCOL_VERSION).toBe(39);
    const kinds: ClipKind[] = ["keys", "vocal"];
    expect(kinds).toHaveLength(2);
    const kept: ArrangeView = fakeScene("arrangeSelect")!.state.arrange;
    expect(kept.clipboardKind).toBe("keys");
    expect(fakeScene("autoRec")!.state.arrange).toMatchObject({ clipboardClips: 0, clipboardKind: null });
  });
  it("the fake answers copy with the kind of the earliest clip (the upper one when they start together), as Rust", () => {
    const s = fakeScene("audioEdit")!.state;
    const vocalTrack = s.song.tracks.find((t) => t.kind.kind === "vocal")!;
    const keysTrack = s.song.tracks.find((t) => t.kind.kind === "keys")!;
    const vocal = vocalTrack.clips[0]!;
    const at = (start: number) => [{ ...vocal, id: 900, start, end: start + 1920, audio: null }];
    expect(reduceClips(s, { type: "copyClips", clips: [vocal.id] })!.arrange.clipboardKind).toBe("vocal");
    keysTrack.clips = at(vocal.start + 1);
    expect(reduceClips(s, { type: "copyClips", clips: [900, vocal.id] })!.arrange).toMatchObject({ clipboardClips: 2, clipboardKind: "vocal" });
    keysTrack.clips = at(vocal.start);
    const upper = s.song.tracks.indexOf(keysTrack) < s.song.tracks.indexOf(vocalTrack) ? "keys" : "vocal";
    expect(reduceClips(s, { type: "copyClips", clips: [vocal.id, 900] })!.arrange.clipboardKind).toBe(upper);
  });
});

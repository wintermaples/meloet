// 「音が大きすぎます」 (round 12, M2): the status line with 「全部のトラックを N dB 下げる」 and 「閉じる」, and the fake's
// copy of the core's intents.
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { createFakeTransport, fakeScene, reduceMixer, type FakeState } from "../src/bridge/fake";
import StatusLine from "../src/components/StatusLine.svelte";
import { t } from "../src/text";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
});

function show(state: AppState): void {
  if (mounted) unmount(mounted);
  app.setState(state);
  mounted = mount(StatusLine, { target, props: {} });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const scene = (name: string) => fakeScene(name)!.state as FakeState;

async function opened(state: FakeState) {
  const transport = createFakeTransport(state);
  await transport.subscribe(
    () => {},
    () => {},
  );
  const send = async (command: Command) => {
    await transport.send(command);
    return (await transport.snapshot()).state as AppState;
  };
  return { send };
}

describe("wording (user:2026-11-23)", () => {
  it("is the spec's, with the spec's numbers", () => {
    expect(t("msg.tooLoudLimiter", { db: "6" })).toBe("音が大きすぎます。リミッターがずっと 6 dB おさえています");
    expect(t("msg.tooLoudOver", { db: "2.1" })).toBe("音が大きすぎます。マスターが 2.1 dB 越えています");
    expect(t("button.lowerAllTracks", { db: 6 })).toBe("全部のトラックを 6 dB 下げる");
    expect(t("button.close")).toBe("閉じる");
  });
});

describe("the status line", () => {
  it("says the measured amount to a tenth and lowers by the whole dB of the button, each button one intent", () => {
    show(scene("tooLoudLimiter"));
    expect(text(q(".message"))).toBe("音が大きすぎます。リミッターがずっと 5.6 dB おさえています");
    const buttons = [...target.querySelectorAll<HTMLElement>("footer.status button.link")];
    expect(buttons.map((b) => text(b))).toEqual(["全部のトラックを 6 dB 下げる", "閉じる"]);
    buttons[0]!.click();
    buttons[1]!.click();
    expect(app.sent).toEqual([{ type: "lowerAllTracks", db: 6 }, { type: "dismissTooLoud" }]);
  });

  it("without a limiter says how far the master goes over 0 dB", () => {
    show(scene("tooLoudOver"));
    expect(text(q(".message"))).toBe("音が大きすぎます。マスターが 2.1 dB 越えています");
    expect(text(q("footer.status button.link"))).toBe("全部のトラックを 3 dB 下げる");
  });
});

describe("the fake's intents", () => {
  it("「下げる」 lowers every track's fader, −∞ staying and none below −60 dB, and puts the line away", async () => {
    const start = scene("tooLoudLimiter");
    const { send } = await opened(start);
    const s = await send({ type: "lowerAllTracks", db: 6 });
    const before = start.song.tracks.map((x) => x.gainDb);
    expect(s.song.tracks.map((x) => x.gainDb)).toEqual(before.map((db) => (db === null ? null : Math.max(-60, db - 6))));
    expect([s.song.master.gainDb, s.song.returns[0].gainDb]).toEqual([start.song.master.gainDb, start.song.returns[0].gainDb]);
    expect(s.status.message?.id).not.toBe("tooLoud");
  });

  it("「閉じる」 only puts the line away", async () => {
    const start = scene("tooLoudOver");
    const { send } = await opened(start);
    const s = await send({ type: "dismissTooLoud" });
    expect(s.status.message).toBeNull();
    expect(s.song).toEqual(start.song);
  });

  it("lowering is refused while recording, and keeps −∞ and the −60 dB floor", () => {
    const state = scene("tooLoudLimiter");
    state.song.tracks[0]!.gainDb = null;
    state.song.tracks[1]!.gainDb = -57;
    const lowered = reduceMixer(state, state.shelf, state.song, { type: "lowerAllTracks", db: 6 })!;
    expect(lowered.song.tracks.slice(0, 2).map((x) => x.gainDb)).toEqual([null, -60]);
    state.transport.state = "recording";
    const refused = reduceMixer(state, state.shelf, state.song, { type: "lowerAllTracks", db: 6 })!;
    expect(refused.song.tracks.map((x) => x.gainDb)).toEqual(state.song.tracks.map((x) => x.gainDb));
  });
});

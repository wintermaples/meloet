// Protocol 35 (the owner, 2026-11-24): 「このテイク以外を消す」 in the right-click menu of a clip, its step name, and the
// fake's copy of the intent.
import { afterEach, describe, expect, it } from "vitest";
import type { AppState, Command, HistoryRow } from "../src/bridge";
import { PROTOCOL_VERSION, STEP_NAME_KINDS } from "../src/bridge";
import { COMP_CLIP, createFakeTransport, fakeScene } from "../src/bridge/fake";
import { historyRowText, stepNameText } from "../src/components/history/stepText";
import { t, useLanguage } from "../src/text";
import { stepNames, texts } from "../src/text/ja";

afterEach(() => useLanguage("ja"));

describe("shape of protocol 35", () => {
  it("version 35; the intent names the clip", () => {
    expect(PROTOCOL_VERSION).toBe(35);
    const command: Command = { type: "deleteOtherTakes", clip: 4 };
    expect(command).toEqual({ type: "deleteOtherTakes", clip: 4 });
  });

  it("the step name and the menu row, in Japanese and in English", () => {
    expect(STEP_NAME_KINDS).toContain("deleteOtherTakes");
    expect(stepNames.deleteOtherTakes).toMatchObject({ text: "このテイク以外を消す", source: "user:2026-11-24" });
    expect(texts["menu.deleteOtherTakes"]).toMatchObject({ text: "このテイク以外を消す", source: "user:2026-11-24" });
    const row: HistoryRow = { index: 3, step: 7, name: { kind: "deleteOtherTakes" }, what: { kind: "take", number: 2 }, place: null };
    const ctx = fakeScene("compTakes")!.state;
    expect(historyRowText(row, ctx)).toBe("「このテイク以外を消す」・ テイク 2");
    useLanguage("en");
    expect([stepNameText(row.name), t("menu.deleteOtherTakes")]).toEqual(["Delete all other takes", "Delete all other takes"]);
  });
});

describe("the fake", () => {
  const clipOf = (s: AppState) => s.song.tracks.flatMap((x) => x.clips).find((c) => c.id === COMP_CLIP)!;
  const [A, B, D] = [COMP_CLIP + 1, COMP_CLIP + 2, COMP_CLIP + 3];

  it("keeps the shown take as take 1; nothing while the clip has a comp, or with one take", async () => {
    const f = createFakeTransport(fakeScene("compTakes")!.state);
    const send = async (c: Command) => {
      await f.send(c);
      return (await f.snapshot()).state as AppState;
    };
    const shape = (s: AppState) => {
      const clip = clipOf(s);
      return [clip.take, clip.takes, clip.takeId, clip.audio!.takes.map((x) => x.takeId)];
    };
    let s = await send({ type: "deleteOtherTakes", clip: COMP_CLIP });
    expect(shape(s)).toEqual([3, 3, D, [A, B, D]]);
    s = await send({ type: "clearComp", clip: COMP_CLIP });
    s = await send({ type: "deleteOtherTakes", clip: COMP_CLIP });
    expect(shape(s)).toEqual([1, 1, D, [D]]);
    const kept = shape(s);
    s = await send({ type: "deleteOtherTakes", clip: COMP_CLIP });
    expect(shape(s)).toEqual(kept);
  });
});

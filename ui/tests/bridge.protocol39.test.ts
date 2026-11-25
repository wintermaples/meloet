// Protocol 39 (bug sweep, 2026-11-25): right after 「戻さない」, the song dialog asks whether the recovery file goes too.
import { describe, expect, it } from "vitest";
import { PROTOCOL_VERSION, type AppState, type Command, type SongDialogView } from "../src/bridge";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";

describe("shape of protocol 39", () => {
  it("version 39; the dialog kind and its two answers", () => {
    expect(PROTOCOL_VERSION).toBe(39);
    const dialog: SongDialogView = { id: 1, kind: "deleteRecovery", title: "はじめての曲", previous: false };
    const answers: Command[] = [
      { type: "answerSongDialog", dialog: dialog.id, answer: "delete" },
      { type: "answerSongDialog", dialog: dialog.id, answer: "keep" },
    ];
    expect(answers.map((c) => c.type === "answerSongDialog" && c.answer)).toEqual(["delete", "keep"]);
    expect(fakeScene("deleteRecovery")!.state.workspace.songDialog).toEqual(dialog);
  });

  it("the fake asks after 「戻さない」, and an answer puts the dialog away", async () => {
    const t = createFakeTransport(fakeScene("recover1")!.state);
    await t.subscribe(
      () => {},
      () => {},
    );
    const now = async () => (await t.snapshot()).state as AppState;
    await t.send({ type: "discardRecovered" });
    const asked = (await now()).workspace.songDialog;
    expect(asked?.kind).toBe("deleteRecovery");
    await t.send({ type: "answerSongDialog", dialog: asked!.id, answer: "keep" });
    expect((await now()).workspace.songDialog).toBeNull();
  });
});

// Protocol 38 (bug sweep, 2026-11-25): what the status line says when the files of a recording, or a file brought in, cannot be written.
import { describe, expect, it } from "vitest";
import { PROTOCOL_VERSION, type StatusMessage } from "../src/bridge";
import { t } from "../src/text";

describe("shape of protocol 38", () => {
  it("version 38 or later; three messages, each with its text", () => {
    expect(PROTOCOL_VERSION).toBeGreaterThanOrEqual(38);
    const messages: StatusMessage[] = [
      { id: "vocalsNotRecorded", keys: true },
      { id: "recordingStoppedDisk", reason: "cannotWrite" },
      { id: "recordingStoppedDisk", reason: "fileFull" },
      { id: "audioNotCopied", file: "声.wav" },
    ];
    expect(new Set(messages.map((m) => m.id)).size).toBe(3);
    expect(t("msg.vocalsNotRecorded")).toBe("歌を録れません。作業の場所に書けません。");
    expect(t("msg.audioNotCopied", { file: "声.wav" })).toBe("「声.wav」を入れられませんでした。作業の場所に書けません。");
  });
});

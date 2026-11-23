// The measuring hook (when MELOET_MEASURE_DIR is set): time per frame, and which sections were drawn how many times.
import { describe, expect, it } from "vitest";
import { FrameProfile, PROFILE_NAME } from "../src/gl/profile";

describe("FrameProfile", () => {
  it("records per frame, counts draws per section, and writes them out together", async () => {
    const written: [string, string][] = [];
    let now = 100;
    const p = new FrameProfile(
      async (name, json) => void written.push([name, json]),
      () => now,
    );
    const f = p.begin();
    f.measureMs = 1;
    p.drew("recordingClip", 12);
    p.drew("recordingClip", 3);
    p.drew("masterMeter", 4);
    now = 120;
    p.begin();
    await p.flush();
    await p.flush();
    expect(written).toHaveLength(1);
    const d = JSON.parse(written[0]![1]);
    expect(written[0]![0]).toBe(PROFILE_NAME);
    expect(d.frames).toBe(2);
    expect(d.draws).toEqual({ recordingClip: 2, masterMeter: 1 });
    expect(d.kept[0]).toMatchObject({ atMs: 100, measureMs: 1, drawn: { recordingClip: 15, masterMeter: 4 } });
    p.stop();
  });
});

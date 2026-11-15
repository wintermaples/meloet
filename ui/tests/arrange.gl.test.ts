// 偽物の場面で画面ぜんたいを組み立て、WebGL の文脈の数が予算の中にあるかを見る（gl.surface.test.ts の形）。
// 場面ごとに、モジュールを読みなおす（橋と状態はモジュールの中にあるので、前の場面の物が残る。残ると、ピアノロールが前の場面の録っている物の帳面をつかむ）。
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/** にせの WebGL2 と、手で回す rAF。 */
function fakePage() {
  const frames: FrameRequestCallback[] = [];
  vi.stubGlobal("requestAnimationFrame", (f: FrameRequestCallback) => frames.push(f));
  vi.stubGlobal("cancelAnimationFrame", () => {});
  const gl = new Proxy(
    {},
    {
      get: (_, name: string) => {
        if (name === "getShaderParameter" || name === "getProgramParameter") return () => true;
        return typeof name === "string" && /^[A-Z_0-9]+$/.test(name) ? 1 : () => ({});
      },
    },
  );
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(gl as never);
  return () => {
    for (const f of frames.splice(0)) f(0);
  };
}

const settle = async (frame: () => void, flushSync: () => void) => {
  for (let i = 0; i < 10; i++) {
    await new Promise((r) => setTimeout(r, 0));
    flushSync();
    frame();
  }
};

/** canvas を持つ区画の名前。 */
const regions = () =>
  [...document.querySelectorAll("canvas")].map((c) => (c.parentElement as HTMLElement).dataset.glRegion).sort();

describe("場面ごとの WebGL の文脈の数", () => {
  beforeEach(() => {
    vi.resetModules();
  });
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    history.replaceState(null, "", "/");
    document.body.innerHTML = "";
  });

  it.each([
    ["recording", ["arrangeLanes", "inputKeys", "inputVelocity", "masterMeter", "recordingClip", "trackMeters"]],
    ["takes", ["inputKeys", "masterMeter", "overview", "pianoRoll", "pianoRollKeys"]],
    ["takesArrange", ["arrangeLanes", "inputKeys", "inputVelocity", "masterMeter", "trackMeters"]],
    ["tracks16", ["arrangeLanes", "inputKeys", "inputVelocity", "masterMeter", "trackMeters"]],
    ["pianoRollRec", ["inputKeys", "masterMeter", "overview", "pianoRoll", "pianoRollKeys", "pianoRollLive"]],
  ])("%s: 区画ごとに 1 枚で、予算をこえない", async (scene, expected) => {
    const { flushSync, mount, unmount } = await import("svelte");
    const { default: App } = await import("../src/App.svelte");
    const { CONTEXT_BUDGET } = await import("../src/gl/budget");
    const { contextsMade, resetPool } = await import("../src/gl/pool");
    resetPool();
    const frame = fakePage();
    history.replaceState(null, "", `/?scene=${scene}&scale=100`);
    const app = mount(App, { target: document.body });
    await settle(frame, flushSync);
    expect(regions()).toEqual(expected);
    expect(contextsMade()).toBe(expected.length);
    expect(contextsMade()).toBeLessThanOrEqual(CONTEXT_BUDGET);
    unmount(app);
  });
});

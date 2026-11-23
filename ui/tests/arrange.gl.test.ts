// Builds the whole screen from fake scenes and checks that the number of WebGL contexts is in the budget (the form of gl.surface.test.ts).
// Each scene loads the modules again (the bridge and state live in modules, so the previous scene's things stay. If they stay, the piano roll holds the previous scene's recording book).
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/** A fake WebGL2 and a rAF turned by hand. */
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

/** Names of the regions that have a canvas. */
const regions = () =>
  [...document.querySelectorAll("canvas")].map((c) => (c.parentElement as HTMLElement).dataset.glRegion).sort();

describe("number of WebGL contexts per scene", () => {
  beforeEach(() => {
    vi.resetModules();
  });
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    history.replaceState(null, "", "/");
    document.body.innerHTML = "";
  });

  // The first scene loads App's modules for the first time. On a busy PC that runs other files at the same time, it can go over the default 5 seconds.
  it.each([
    ["recording", ["arrangeLanes", "inputMeter", "masterMeter", "recordingClip", "trackMeters"]],
    ["takes", ["inputMeter", "masterMeter", "overview", "pianoRoll", "pianoRollKeys", "pianoRollLive"]],
    ["takesArrange", ["arrangeLanes", "inputMeter", "masterMeter", "recordingClip", "trackMeters"]],
    ["arrangeTrimmed", ["arrangeLanes", "inputMeter", "masterMeter", "recordingClip", "trackMeters"]],
    ["tracks16", ["arrangeLanes", "inputMeter", "masterMeter", "recordingClip", "trackMeters"]],
    ["pianoRollRec", ["inputMeter", "masterMeter", "overview", "pianoRoll", "pianoRollKeys", "pianoRollLive"]],
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
  }, 60_000);
});

// The helper that moves by itself is loaded only when measuring (MELOET_MEASURE_DIR) and Rust passes a script. Usually it does nothing.
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const bridge = vi.hoisted(() => ({
  measureEnabled: vi.fn<() => Promise<boolean>>(),
  uiScript: vi.fn<() => Promise<string | null>>(),
  measureReport: vi.fn(async () => {}),
}));
const dev = vi.hoisted(() => ({ startUiScript: vi.fn(() => () => {}) }));

vi.mock("../src/bridge", async (original) => ({ ...(await original<typeof import("../src/bridge")>()), ...bridge }));
vi.mock("../src/dev/start", () => dev);

const Surface = (await import("../src/gl/Surface.svelte")).default;

async function mounted(): Promise<() => void> {
  const surface = mount(Surface, { target: document.body });
  flushSync();
  for (let i = 0; i < 10; i++) await Promise.resolve();
  await new Promise((r) => setTimeout(r, 0));
  return () => unmount(surface);
}

beforeEach(() => {
  vi.stubGlobal("requestAnimationFrame", () => 0);
  vi.stubGlobal("cancelAnimationFrame", () => {});
  bridge.uiScript.mockResolvedValue("pr-scroll:3");
  bridge.measureReport.mockClear();
  bridge.uiScript.mockClear();
  dev.startUiScript.mockClear();
});

afterEach(() => vi.unstubAllGlobals());

describe("start of the helper that moves by itself", () => {
  it("when not measuring, does not ask for a script and does not load the helper", async () => {
    bridge.measureEnabled.mockResolvedValue(false);
    const stop = await mounted();
    expect(bridge.uiScript).not.toHaveBeenCalled();
    expect(dev.startUiScript).not.toHaveBeenCalled();
    stop();
  });

  it("even when measuring, does not move without a script", async () => {
    bridge.measureEnabled.mockResolvedValue(true);
    bridge.uiScript.mockResolvedValue(null);
    const stop = await mounted();
    expect(bridge.uiScript).toHaveBeenCalledTimes(1);
    expect(dev.startUiScript).not.toHaveBeenCalled();
    stop();
  });

  it("when measuring with a script, moves with that script, and stops when the surface is removed", async () => {
    bridge.measureEnabled.mockResolvedValue(true);
    const stopScript = vi.fn();
    dev.startUiScript.mockReturnValue(stopScript);
    const stop = await mounted();
    expect(dev.startUiScript).toHaveBeenCalledWith("pr-scroll:3", bridge.measureReport);
    stop();
    expect(stopScript).toHaveBeenCalledTimes(1);
  });
});

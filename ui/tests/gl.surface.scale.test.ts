// UI-BRIDGE #1 (later): the engine hears the scale only when the scale changes, not after every patch.
import { flushSync, mount, unmount } from "svelte";
import { afterEach, expect, it, vi } from "vitest";
import { fakeState } from "../src/bridge/fake";
import Surface from "../src/gl/Surface.svelte";
import { Engine } from "../src/gl/engine";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  app.reset();
});

it("setScale runs once per scale, not after a patch of another section", () => {
  vi.stubGlobal("requestAnimationFrame", () => 0);
  vi.stubGlobal("cancelAnimationFrame", () => {});
  const scales: number[] = [];
  vi.spyOn(Engine.prototype, "setScale").mockImplementation((s: number) => void scales.push(s));
  vi.spyOn(Engine.prototype, "setTheme").mockImplementation(() => {});
  vi.spyOn(Engine.prototype, "setTracks").mockImplementation(() => {});
  const s = fakeState();
  app.setState(s);
  const surface = mount(Surface, { target: document.body });
  flushSync();
  app.setState({ ...s, transport: { ...s.transport } });
  flushSync();
  app.setState({ ...s, workspace: { ...s.workspace, scale: 125 } });
  flushSync();
  expect(scales).toEqual([s.workspace.scale, 125]);
  unmount(surface);
});

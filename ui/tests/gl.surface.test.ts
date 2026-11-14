// WebGL2 のない所（jsdom）でも、面を置いた画面がこわれない。
import { flushSync, mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import Surface from "../src/gl/Surface.svelte";

describe("WebGL の面", () => {
  afterEach(() => vi.restoreAllMocks());

  it("WebGL2 が取れなければ、何も描かずに 1 回だけ知らせる", () => {
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const surface = mount(Surface, { target: document.body });
    flushSync();
    const canvas = document.querySelector("canvas")!;
    expect(canvas.getAttribute("aria-hidden")).toBe("true");
    expect(error).toHaveBeenCalledTimes(1);
    expect(String(error.mock.calls[0]![0])).toContain("WebGL2");
    unmount(surface);
    expect(document.querySelector("canvas")).toBe(null);
  });
});

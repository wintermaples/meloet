// 面は、slot の箱の中に 1 枚ずつ。WebGL2 のない所（jsdom）でも、画面がこわれない。
import { flushSync, mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import Surface from "../src/gl/Surface.svelte";
import { Engine } from "../src/gl/engine";
import { glSlot } from "../src/gl/slot";

class FakeResizeObserver {
  observe() {}
  unobserve() {}
  disconnect() {}
}

/** 呼ばれた事だけを数える、にせの WebGL2。 */
function fakeGl() {
  const calls = { drawArrays: 0, clear: 0 };
  const gl = new Proxy(
    {},
    {
      get: (_, name: string) => {
        if (name === "getShaderParameter" || name === "getProgramParameter") return () => true;
        if (name === "drawArrays" || name === "clear") return () => void (calls[name] += 1);
        return typeof name === "string" && /^[A-Z_0-9]+$/.test(name) ? 1 : () => ({});
      },
    },
  );
  return { gl, calls };
}

describe("WebGL の面", () => {
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    document.body.innerHTML = "";
  });

  it("WebGL2 が取れなければ、箱を空のままにして 1 回だけ知らせる", () => {
    vi.stubGlobal("ResizeObserver", FakeResizeObserver);
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const a = document.body.appendChild(document.createElement("div"));
    const b = document.body.appendChild(document.createElement("div"));
    const slots = [glSlot(a, { kind: "velocity" }), glSlot(b, { kind: "overview" })];
    const surface = mount(Surface, { target: document.body });
    flushSync();
    expect(document.querySelector("canvas")).toBe(null);
    expect(error).toHaveBeenCalledTimes(1);
    expect(String(error.mock.calls[0]![0])).toContain("WebGL2");
    unmount(surface);
    for (const s of slots) s.destroy();
  });

  it("箱ごとに、箱の大きさの面を 1 枚。中を全部ぬる面は不透明。変わった面だけを描く", () => {
    vi.stubGlobal("ResizeObserver", FakeResizeObserver);
    const frames: FrameRequestCallback[] = [];
    vi.stubGlobal("requestAnimationFrame", (f: FrameRequestCallback) => frames.push(f));
    vi.stubGlobal("cancelAnimationFrame", () => {});
    const frame = () => {
      for (const f of frames.splice(0)) f(0);
    };
    const made: { canvas: HTMLCanvasElement; alpha: boolean; calls: { drawArrays: number; clear: number } }[] = [];
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockImplementation(function (this: HTMLCanvasElement, _id: string, options?: unknown) {
      const { gl, calls } = fakeGl();
      made.push({ canvas: this, alpha: (options as { alpha: boolean }).alpha, calls });
      return gl as never;
    });

    const keys = document.body.appendChild(document.createElement("div"));
    const bar = document.body.appendChild(document.createElement("div"));
    // 画素の間から始まる箱: 面は、はしを画素に合わせる。
    keys.getBoundingClientRect = () => new DOMRect(10.4, 20, 349, 32);
    bar.getBoundingClientRect = () => new DOMRect(500, 20, 64, 4);
    const slots = [
      glSlot(keys, { kind: "keyboard", size: "full", firstNote: 36, lastNote: 84 }),
      glSlot(bar, { kind: "meter", source: "master", gap: 1 }),
    ];
    const engine = new Engine();
    frame();

    expect(made.map((m) => m.alpha)).toEqual([false, true]);
    const [k, m] = made as [(typeof made)[0], (typeof made)[0]];
    expect(k.canvas.parentElement).toBe(keys);
    expect(k.canvas.getAttribute("aria-hidden")).toBe("true");
    expect([k.canvas.width, k.canvas.height]).toEqual([349, 32]);
    expect(parseFloat(k.canvas.style.left)).toBeCloseTo(-0.4);
    expect(k.canvas.style.width).toBe("349px");
    expect([m.canvas.width, m.canvas.height]).toEqual([64, 4]);
    expect([k.calls.clear, m.calls.clear]).toEqual([1, 1]);

    // 鍵: 鍵盤の面だけ。同じ絵なら描かない。
    engine.onEvent({ type: "note", note: 60, on: true, velocity: 90, tUs: 1 });
    frame();
    expect([k.calls.clear, m.calls.clear]).toEqual([2, 1]);
    engine.invalidate();
    frame();
    expect([k.calls.clear, m.calls.clear]).toEqual([2, 1]);
    expect(engine.stats).toEqual({ frames: 3, draws: 3 });

    slots[0]!.destroy();
    expect(keys.querySelector("canvas")).toBe(null);
    engine.destroy();
    expect(document.querySelector("canvas")).toBe(null);
    slots[1]!.destroy();
  });
});

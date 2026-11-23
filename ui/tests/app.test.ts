// Builds the whole screen from a fake scene, and checks the frame's text and the commands sent on a press.
import { flushSync, mount, unmount } from "svelte";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import App from "../src/App.svelte";
import { current } from "../src/state/app.svelte";

// jsdom has no WebGL2. The surfaces are checked in the WebGL tests.
vi.mock("../src/gl/Surface.svelte", () => ({ default: () => {} }));

const settle = async () => {
  for (let i = 0; i < 5; i++) await new Promise((r) => setTimeout(r, 0));
  flushSync();
};

describe("screen frame (?scene=bufferFixed)", () => {
  let app: ReturnType<typeof mount>;
  beforeAll(async () => {
    history.replaceState(null, "", "/?scene=bufferFixed&scale=100");
    app = mount(App, { target: document.body });
    await settle();
  });
  afterAll(() => {
    unmount(app);
    history.replaceState(null, "", "/");
  });

  const text = (sel: string) => document.querySelector(sel)?.textContent?.replace(/\s+/g, " ").trim();

  it("once connected, sends the current window size (does not lose size changes from before connecting)", () => {
    expect(current.failure).toBeNull();
    expect(current.state!.workspace.viewportWidth).toBe(window.innerWidth);
    expect(current.state!.workspace.viewportHeight).toBe(window.innerHeight);
  });

  it("from the top: transport, arrange, band tabs, status line", () => {
    // jsdom has width 0, so the song name and such are hidden.
    expect(text(".transport")).toContain("96.00");
    expect(document.querySelector("[data-gl-slot=gridLines]")).not.toBeNull();
    expect(document.querySelectorAll("[data-gl-slot=meter]")).toHaveLength(2);
    expect(text("nav.row")).toContain("アレンジ A");
  });

  it("status line: the text that the buffer is fixed, 「くわしく」, and the input, output and buffer on the right", () => {
    const s = text("footer.status")!;
    expect(s).toContain("バッファが PC 全体の設定で 1024 に固定されています ・ 音が出るまで 約 53 ms（256 なら 約 13.5 ms）");
    expect(s).toContain("出口: アナログヘッドフォン");
    expect(s).toContain("入口: Keystation 49 MK3");
    expect(s).toContain("バッファ 1024 ・ 頼んだ数 256");
  });

  it("「くわしく」 opens the sound settings in detail", async () => {
    const button = [...document.querySelectorAll("footer.status button")].find((b) => b.textContent === "くわしく") as HTMLButtonElement;
    button.click();
    await settle();
    expect(document.querySelector(".dialog")).not.toBeNull();
  });

  it("pressing a band tab opens the band and shows the Esc hint", async () => {
    (document.querySelector("nav.row [data-band=history]") as HTMLButtonElement).click();
    await settle();
    expect(document.querySelector("section.band[data-band=history]")).not.toBeNull();
    expect(text("nav.row")).toContain("Esc ・ ひとつ戻る。何度か押すとアレンジへ");
  });
});

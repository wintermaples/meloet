// 画面ぜんたいを偽物の場面で組み立てて、骨組みの文と押した時のコマンドを見る。
import { flushSync, mount, unmount } from "svelte";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import App from "../src/App.svelte";

// jsdom に WebGL2 はない。面は WebGL の係のテストで見る。
vi.mock("../src/gl/Surface.svelte", () => ({ default: () => {} }));

const settle = async () => {
  for (let i = 0; i < 5; i++) await new Promise((r) => setTimeout(r, 0));
  flushSync();
};

describe("画面の骨組み（?scene=bufferFixed）", () => {
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

  it("上から、再生まわり・アレンジ・帯札・状態行がある", () => {
    // jsdom では幅が 0 なので、曲の名前などは引っこんでいる。
    expect(text(".transport")).toContain("96.00");
    expect(document.querySelector("[data-gl-slot=arrangeGrid]")).not.toBeNull();
    expect(document.querySelectorAll("[data-gl-slot=meter]")).toHaveLength(2);
    expect(text("nav.row")).toContain("アレンジ A");
  });

  it("状態行: バッファが固定されている文と、くわしく、右の入口・出口・バッファ", () => {
    const s = text("footer.status")!;
    expect(s).toContain("バッファが PC 全体の設定で 1024 に固定されています ・ 音が出るまで 約 53 ms（256 なら 約 13.5 ms）");
    expect(s).toContain("出口: アナログヘッドフォン");
    expect(s).toContain("入口: Keystation 49 MK3");
    expect(s).toContain("バッファ 1024 ・ 頼んだ数 256");
  });

  it("くわしく で音の設定の奥がひらく", async () => {
    const button = [...document.querySelectorAll("footer.status button")].find((b) => b.textContent === "くわしく") as HTMLButtonElement;
    button.click();
    await settle();
    expect(document.querySelector(".dialog")).not.toBeNull();
  });

  it("帯札を押すと帯がひらき、Esc の案内が出る", async () => {
    (document.querySelector("nav.row [data-band=history]") as HTMLButtonElement).click();
    await settle();
    expect(document.querySelector("section.band[data-band=history]")).not.toBeNull();
    expect(text("nav.row")).toContain("Esc ・ ひとつ戻る。何度か押すとアレンジへ");
  });
});

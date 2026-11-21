// 約束 25 の偽物の場面を、今の「選んだクリップ」の欄に出してもこわれない（欄を使える形にするのは区切り4 の 2 回目）。
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { fakeScene, type FakeState } from "../src/bridge/fake";
import ParamsPanel from "../src/components/params/ParamsPanel.svelte";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
});
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

describe("約束 25 の場面と、今の「選んだクリップ」の欄", () => {
  it.each([
    ["stretch", ["曲のテンポについていく オン", "この音のテンポ 76.80", "音の高さ +2 半音 0 セント", "声の感じを保つ オン"]],
    ["stretchRendering", ["曲のテンポについていく オン", "この音のテンポ 76.80", "音の高さ +2 半音 0 セント", "声の感じを保つ オン"]],
    ["stretchPitch", ["曲のテンポについていく オン", "この音のテンポ 96.00", null, "声の感じを保つ オフ"]],
    ["stretchNoTempo", ["曲のテンポについていく オフ", "この音のテンポ —", "音の高さ +0 半音 0 セント", "声の感じを保つ オン"]],
    ["stretchRamp", ["曲のテンポについていく オン", "この音のテンポ 96.00", "音の高さ +0 半音 0 セント", "声の感じを保つ オン"]],
  ])("%s: のび縮み ・ 音の高さの 4 行", (name, rows) => {
    app.setState(fakeScene(name)!.state as FakeState);
    mounted = mount(ParamsPanel, { target, props: { onfold: () => {} } });
    flushSync();
    const got = [...target.querySelectorAll<HTMLElement>(".row.unusable")].map((r) => [...r.children].map(text).join(" "));
    expect(got.length).toBe(4);
    rows.forEach((row, i) => {
      if (row !== null) expect(got[i], name).toBe(row);
    });
    // 負の半音とセントの見せかたは、今の欄の形のまま（2 回目に決める）。
    if (name === "stretchPitch") expect(got[2]).toMatch(/1 半音 .*30 セント$/);
  });
});

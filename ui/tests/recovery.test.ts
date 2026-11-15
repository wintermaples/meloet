// なおすためのファイルの案内と状態行（仕様書 3 章・6 章、モック F-Recover1〜3）。
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import RecoveryHint from "../src/components/RecoveryHint.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { dayAndTime, termParts } from "../src/components/format";
import { TIP_DELAY_MS, tipView } from "../src/lib/tip.svelte";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;

beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
  vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockReturnValue(1920);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  document.body.innerHTML = "";
  vi.restoreAllMocks();
  vi.useRealTimers();
});

function show(component: Component<any>, state: AppState): void {
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}

const scene = (name: string, change: (s: AppState) => void = () => {}) => {
  const s = fakeScene(name)!.state;
  change(s);
  return s;
};
const q = <T extends HTMLElement = HTMLElement>(sel: string) => document.querySelector<T>(sel);
const text = (sel: string) => q(sel)?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const lines = () => [...q('[data-test="recovery-hint"]')!.children].slice(0, 2).map((e) => e.textContent);
const buttons = () => [...document.querySelectorAll<HTMLButtonElement>('[data-test="recovery-hint"] button')];

describe("戻した曲の案内（F-Recover1）", () => {
  it("PC が終わったとき: 仕様書 6 章の 2 文と、保存する Ctrl S ・ 戻さない", () => {
    show(RecoveryHint, scene("recover1"));
    expect(lines()).toEqual([
      "PC が終わる前の演奏を戻しました。まだ保存していません。",
      "11 月 15 日 22:41 に PC が終わったときの「新しい曲」です。テイク 2 つ。",
    ]);
    // 文とキーの間は、ボタンの gap（space.1）。
    expect(buttons().map((b) => b.textContent)).toEqual(["保存するCtrl S", "戻さない"]);
    expect(q('[data-test="recovery-save"] .key')!.textContent).toBe("Ctrl S");
    buttons()[0]!.click();
    buttons()[1]!.click();
    expect(app.sent).toEqual([{ type: "saveSong" }, { type: "discardRecovered" }]);
  });

  it("ふだん書いた物: ヤチヨさんの 2 文。保存した曲は名前", () => {
    show(
      RecoveryHint,
      scene("recover1", (s) => {
        const r = s.workspace.recovery!;
        if (r.kind === "recovered") Object.assign(r, { cause: "sudden", song: "はじめての曲", takes: 5 });
      }),
    );
    expect(lines()).toEqual([
      "meloet が急に終わる前の演奏を戻しました。まだ保存していません。",
      "11 月 15 日 22:41 の「はじめての曲」です。テイク 5 つ。",
    ]);
  });

  it("テイクが 0 なら、2 文目は「…です。」で終わる", () => {
    show(
      RecoveryHint,
      scene("recover1", (s) => {
        const r = s.workspace.recovery!;
        if (r.kind === "recovered") r.takes = 0;
      }),
    );
    expect(lines()[1]).toBe("11 月 15 日 22:41 に PC が終わったときの「新しい曲」です。");
  });

  it("状態行: なおすためのファイルから戻しました ・ まだ保存していません（用語の説明つき）", () => {
    vi.useFakeTimers();
    show(StatusLine, scene("recover1"));
    expect(text(".status .message")).toBe("なおすためのファイルから戻しました ・ まだ保存していません");
    const word = [...document.querySelectorAll<HTMLElement>(".status .message span")].find((s) => s.textContent === "なおすためのファイル")!;
    word.dispatchEvent(new Event("pointerenter"));
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipView.value?.term).toBe("なおすためのファイル");
    word.dispatchEvent(new Event("pointerleave"));
  });
});

describe("戻さないを押した（F-Recover2）", () => {
  it("状態行: 戻した変更をすてて、最後に保存した形にしました。取り消せます（Ctrl Z）", () => {
    show(StatusLine, scene("recover2"));
    expect(text(".status .message")).toBe("戻した変更をすてて、最後に保存した形にしました。取り消せます（Ctrl Z）");
  });
});

describe("読めないとき（F-Recover3）", () => {
  it("2 文と、ファイルの場所をひらく ・ 閉じる。用語に説明がつく", () => {
    vi.useFakeTimers();
    show(RecoveryHint, scene("recover3"));
    expect(lines()).toEqual([
      "前に保存していない変更がありましたが、なおすためのファイルが読めませんでした。",
      "最後に保存した「はじめての曲」をひらいています。ファイルは消さずに残してあります。",
    ]);
    expect(buttons().map((b) => b.textContent)).toEqual(["ファイルの場所をひらく", "閉じる"]);
    buttons()[0]!.click();
    buttons()[1]!.click();
    expect(app.sent).toEqual([{ type: "showRecoveryFile" }, { type: "dismissHint" }]);
    const word = q('[data-test="recovery-hint"] .strong span')!;
    expect(word.textContent).toBe("なおすためのファイル");
    word.dispatchEvent(new Event("pointerenter"));
    vi.advanceTimersByTime(TIP_DELAY_MS);
    expect(tipView.value?.body).toBe(
      "PC が急に終わったときのために、保存していない変更を急いで書いておくファイル。次に起動したとき、ここから戻せる。",
    );
    word.dispatchEvent(new Event("pointerleave"));
  });

  it("保存したことのない曲なら、2 文目は「新しい曲をひらいています。…」", () => {
    show(RecoveryHint, scene("recover3", (s) => (s.workspace.recovery = { kind: "unreadable", song: null })));
    expect(lines()[1]).toBe("新しい曲をひらいています。ファイルは消さずに残してあります。");
  });

  it.each([
    [true, "なおすためのファイルが読めませんでした ・ 最後に保存した曲をひらいています"],
    [false, "なおすためのファイルが読めませんでした ・ 新しい曲をひらいています"],
  ])("状態行（最後に保存した曲 %s）", (savedSong, expected) => {
    show(StatusLine, scene("recover3", (s) => (s.status.message = { id: "recoveryUnreadable", savedSong })));
    expect(text(".status .message")).toBe(expected);
  });
});

describe("時刻と用語の分けかた", () => {
  it("地元の時刻で、分は 2 けた", () => {
    expect(dayAndTime(new Date(2026, 0, 2, 9, 5).getTime())).toEqual({ month: 1, day: 2, time: "9:05" });
  });
  it("用語のある所だけを分ける", () => {
    expect(termParts("あ語い語", "語")).toEqual([
      { text: "あ", term: false },
      { text: "語", term: true },
      { text: "い", term: false },
      { text: "語", term: true },
    ]);
    expect(termParts("なし", "語")).toEqual([{ text: "なし", term: false }]);
  });
});

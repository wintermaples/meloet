// 測るときだけ動く、取り消しの履歴の帯を自動で動かす係（ui/src/dev/historyScript.ts）と、キーの読みかた。
import { describe, expect, it } from "vitest";
import type { HistoryView } from "../src/bridge";
import { HistoryScript, type HistoryTarget } from "../src/dev/historyScript";
import { isHistoryPart, parseUiScript, type Clock, type ScriptRecord } from "../src/dev/script";
import { keyAction } from "../src/keys/keys";

const k = (key: string, shift = false) => ({ key, ctrl: false, meta: false, alt: false, shift });

describe("帯にフォーカスがあるときのキー（仕様書 4 章）", () => {
  it("↑ は新しい手、↓ は古い手、Home はいまの手、Enter は戻す", () => {
    expect(keyAction(k("ArrowUp"), false, "other", "history")).toEqual({ kind: "historyStep", delta: 1 });
    expect(keyAction(k("ArrowDown"), false, "other", "history")).toEqual({ kind: "historyStep", delta: -1 });
    expect(keyAction(k("Home"), false, "other", "history")).toEqual({ kind: "historyHome" });
    expect(keyAction(k("Enter"), false, "other", "history")).toEqual({ kind: "historyConfirm" });
    expect(keyAction(k("Escape"), false, "other", "history")).toEqual({ kind: "back" });
  });
  it("帯の外では今までどおり", () => {
    expect(keyAction(k("Home"), false, "other")).toEqual({ kind: "toStart" });
    expect(keyAction(k("ArrowUp"), false, "other")).toEqual({ kind: "nudge", steps: 0, keys: 1 });
    expect(keyAction(k("Enter"), false, "other")).toBeNull();
  });
});

describe("MELOET_UI_SCRIPT の hist-*", () => {
  it("読める", () => {
    const parts = parseUiScript("hist-make:5000,hist-open:3,hist-scroll,hist-jump:4")!;
    expect(parts).toEqual([
      { kind: "histMake", count: 5000 },
      { kind: "histOpen", count: 3 },
      { kind: "histScroll", count: 100 },
      { kind: "histJump", count: 4 },
    ]);
    expect(parts.every(isHistoryPart)).toBe(true);
    expect(parseUiScript("pr-scroll")!.every(isHistoryPart)).toBe(false);
  });

  it("ひらく ・ スクロール ・ 500 手とぶを順に行い、画面に出るまでの時間を書く", async () => {
    let t = 0;
    const clock: Clock = {
      now: () => (t += 1),
      wall: () => 1000 + t,
      wait: async () => {},
      frame: async () => {},
      settle: async () => {},
      everyFrame: () => () => {},
    };
    let history: HistoryView = { total: 1000, redo: 0, current: 1000, newest: 1000 };
    let open = false;
    const made: number[] = [];
    const viewport = document.createElement("div");
    const row = document.createElement("div");
    row.className = "row";
    row.getBoundingClientRect = () => ({ height: 24 }) as DOMRect;
    viewport.append(row);
    // jsdom の scrollTop は、はしで止まらない。0〜600 の間にする。
    let top = 0;
    Object.defineProperty(viewport, "scrollTop", { get: () => top, set: (v: number) => (top = Math.max(0, Math.min(600, v))) });
    const jumps: number[] = [];
    const target: HistoryTarget = {
      ready: () => true,
      history: () => history,
      bandOpen: () => open,
      toggleBand: () => (open = !open),
      make: async (i) => void made.push(i),
      viewport: () => (open ? viewport : null),
      filled: () => true,
      jumpTo: async (index) => {
        jumps.push(index);
        history = { ...history, current: index, redo: history.total - index };
      },
      isNow: (index) => history.current === index,
    };
    const reports: string[] = [];
    const parts = parseUiScript("hist-make:3,hist-open:2,hist-scroll:4,hist-jump:3")!;
    const script = new HistoryScript("x", parts, target, clock, async (_, json) => void reports.push(json));
    await script.run();
    const record = JSON.parse(reports.at(-1)!) as ScriptRecord;
    expect(record.error).toBeNull();
    expect(record.done).toBe(true);
    expect(made).toEqual([0, 1, 2]);
    expect(record.parts.map((p) => [p.kind, p.steps.length])).toEqual([
      ["histMake", 3],
      ["histOpen", 2],
      ["histScroll", 4],
      ["histJump", 3],
    ]);
    expect(record.parts[1]!.steps.every((s) => s.replyMs !== null)).toBe(true);
    expect(record.parts[2]!.steps.every((s) => s.changed)).toBe(true);
    // 240 → 480 → 600（はし）→ 折りかえして 360。
    expect(top).toBe(360);
    expect(jumps).toEqual([500, 1000, 500]);
  });
});

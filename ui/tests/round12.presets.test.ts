// Round 12 M1: 「プリセット ▾」 of a built-in effect, its list, keeping and removing, the fake's copy of Rust, and the wording.
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command, SelectedInsert } from "../src/bridge";
import { EFFECT_KINDS, EFFECT_PARAMS, FACTORY_PRESETS } from "../src/bridge/types";
import { createFakeTransport, fakeScene } from "../src/bridge/fake";
import { presetName as cleanName, presetOf } from "../src/bridge/fakePresets";
import { presetName } from "../src/components/params/presetName";
import RightRail from "../src/components/RightRail.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { contextMenu } from "../src/components/menu/contextMenu.svelte";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";
import { texts } from "../src/text/ja";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state;
const builtin = (s: AppState) => s.params.selected as SelectedInsert;
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

describe("the presets that come with meloet", () => {
  it("the spec's names, in its order; every name in ja.ts; two each for the gate, de-esser and saturation", () => {
    const names = (kind: keyof typeof FACTORY_PRESETS) => FACTORY_PRESETS[kind].map((p) => presetName(kind, { kind: "factory", id: p.id }));
    expect(names("eq4")).toEqual(["歌をくっきり", "こもりを取る", "低い音をすっきり", "ベースを太く"]);
    expect(names("compressor")).toEqual(["歌をそろえる", "ドラムをはっきり", "ベースをどっしり", "やさしくまとめる"]);
    expect(names("reverb")).toEqual(["小さな部屋", "ホール", "歌のうしろに"]);
    expect(names("delay")).toEqual(["やまびこ", "短いはね返り", "左右にとぶ"]);
    expect(names("chorus")).toEqual(["うすく広げる", "深くゆらす"]);
    expect(names("limiter")).toEqual(["仕上げ", "やさしく止める"]);
    expect(names("gate")).toEqual(["雑音を消す", "余韻を短く切る"]);
    expect(names("deEsser")).toEqual(["サ行をやわらかく", "サ行をしっかりおさえる"]);
    expect(names("saturation")).toEqual(["あたたかく", "荒くひずませる"]);
    for (const kind of EFFECT_KINDS)
      for (const p of FACTORY_PRESETS[kind]) {
        expect(`preset.${kind}.${p.id}` in texts, `${kind} ${p.id}`).toBe(true);
        p.values.forEach((v, i) => {
          const spec = EFFECT_PARAMS[kind][i]!;
          expect(v >= spec.min && v <= spec.max, `${kind} ${p.id} ${spec.id}`).toBe(true);
        });
      }
  });

  it("a name is trimmed and at most 40 characters, as Rust keeps it", () => {
    expect(cleanName("  いつもの  ")).toBe("いつもの");
    expect(cleanName("   ")).toBeNull();
    expect([...cleanName("あ".repeat(45))!].length).toBe(40);
  });

  it("the preset the values are: the factory ones first; the mix of a reverb is not looked at", () => {
    const view = { user: [] };
    expect(presetOf("compressor", FACTORY_PRESETS.compressor[1]!.values, view)).toEqual({ kind: "factory", id: "drumPunch" });
    const hall = [...FACTORY_PRESETS.reverb[1]!.values];
    hall[EFFECT_PARAMS.reverb.findIndex((p) => p.id === "mix")] = 30;
    expect(presetOf("reverb", hall, view)).toEqual({ kind: "factory", id: "hall" });
    expect(presetOf("compressor", EFFECT_PARAMS.compressor.map((p) => p.default), view)).toBeNull();
  });
});

describe("the fake takes the intents as Rust", () => {
  async function opened(name: string) {
    const t = createFakeTransport(scene(name));
    await t.subscribe(
      () => {},
      () => {},
    );
    const now = async () => (await t.snapshot()).state as AppState;
    return { now, send: (command: Command) => t.send(command) };
  }

  it("use, keep under the same name, keep a new one, remove", async () => {
    const { now, send } = await opened("presets");
    const insert = builtin(await now()).insert;
    await send({ type: "applyPreset", insert, preset: { kind: "factory", id: "bassSolid" } });
    let s = await now();
    expect(builtin(s).values).toEqual(FACTORY_PRESETS.compressor[2]!.values);
    expect(builtin(s).preset).toEqual({ kind: "factory", id: "bassSolid" });

    await send({ type: "savePreset", insert, name: "いつものピアノ" });
    s = await now();
    expect(s.status.message).toEqual({ id: "presetOverwritten", name: "いつものピアノ" });
    expect(builtin(s).preset).toEqual({ kind: "factory", id: "bassSolid" });

    await send({ type: "setParam", insert, effect: "compressor", param: 1, value: 9, commit: true });
    await send({ type: "savePreset", insert, name: "  強め  " });
    s = await now();
    expect(s.presets.user).toEqual([{ effect: "compressor", names: ["いつものピアノ", "強め"] }]);
    expect(builtin(s).preset).toEqual({ kind: "user", name: "強め" });

    await send({ type: "applyPreset", insert, preset: { kind: "user", name: "いつものピアノ" } });
    expect(builtin(await now()).values).toEqual(FACTORY_PRESETS.compressor[2]!.values);

    await send({ type: "deletePreset", effect: "compressor", name: "強め" });
    await send({ type: "deletePreset", effect: "compressor", name: "いつものピアノ" });
    expect((await now()).presets.user).toEqual([]);
  });
});

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  contextMenu.open = null;
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
  vi.stubGlobal("requestAnimationFrame", () => 0);
  vi.stubGlobal("cancelAnimationFrame", () => {});
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  vi.unstubAllGlobals();
});
function show(state: AppState): void {
  app.setState(state);
  mounted = mount(RightRail, { target });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(`[data-test="${sel}"]`);
const all = (sel: string) => [...target.querySelectorAll<HTMLElement>(`[data-test="${sel}"]`)];

describe("「プリセット ▾」 in the params panel", () => {
  it("shows the name of the preset the values are, on; 「プリセット ▾」 otherwise", () => {
    show(scene("presets"));
    expect(text(q("preset-button"))).toBe("歌をそろえる ▾");
    expect(q("preset-button")!.classList.contains("on")).toBe(true);
    unmount(mounted!);
    show(scene("presetsNone"));
    expect(text(q("preset-button"))).toBe("プリセット ▾");
    expect(q("preset-button")!.classList.contains("on")).toBe(false);
  });

  it("the list: the two headings, the rows, the one the values are on gray.3; a row uses it and closes the list", () => {
    show(scene("presets"));
    q("preset-button")!.click();
    flushSync();
    const menu = q("preset-menu")!;
    expect([...menu.children].map(text)).toEqual([
      "はじめから入っているもの",
      "歌をそろえる",
      "ドラムをはっきり",
      "ベースをどっしり",
      "やさしくまとめる",
      "自分で残したもの",
      "いつものピアノ",
      "いまの設定を残す…",
    ]);
    expect(all("preset-factory").map((b) => b.classList.contains("now"))).toEqual([true, false, false, false]);
    all("preset-factory")[1]!.click();
    flushSync();
    const s = app.current.state!;
    expect(app.sent.at(-1)).toEqual({ type: "applyPreset", insert: builtin(s).insert, preset: { kind: "factory", id: "drumPunch" } });
    expect(q("preset-menu")).toBeNull();
  });

  it("a gate has its two factory rows; the rows are disabled while recording", () => {
    const s = scene("presetsNone");
    s.transport.state = "recording";
    show(s);
    q("preset-button")!.click();
    flushSync();
    expect(all("preset-factory").map(text)).toEqual(["雑音を消す", "余韻を短く切る"]);
    expect([...q("preset-menu")!.children].map(text)).toEqual(["はじめから入っているもの", "雑音を消す", "余韻を短く切る", "自分で残したもの", "いまの設定を残す…"]);
    unmount(mounted!);
    const r = scene("presets");
    r.transport.state = "recording";
    show(r);
    q("preset-button")!.click();
    flushSync();
    expect(all("preset-factory").every((b) => (b as HTMLButtonElement).disabled)).toBe(true);
  });

  it("UI-MIX-004: Esc closes the list wherever the focus is; in the name box the first Esc puts the row back", () => {
    show(scene("presets"));
    const deps: KeyDeps = { state: () => app.current.state, send: app.send, nav: { platform: "Linux" } };
    const on = (ev: KeyboardEvent) => handleKeydown(ev, deps);
    document.addEventListener("keydown", on);
    const esc = (el: Element) => {
      el.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }));
      flushSync();
    };
    try {
      q("preset-button")!.click();
      flushSync();
      esc(document.body);
      expect(q("preset-menu")).toBeNull();
      q("preset-button")!.click();
      flushSync();
      q("preset-save")!.click();
      flushSync();
      esc(q("preset-name")!);
      expect(q("preset-name")).toBeNull();
      expect(q("preset-menu")).not.toBeNull();
      esc(document.activeElement ?? document.body);
      expect(q("preset-menu")).toBeNull();
      expect(app.sent.filter((c) => c.type === "back")).toEqual([]);
    } finally {
      document.removeEventListener("keydown", on);
    }
  });

  it("FIX-fix-ui-keys #2: with the focus in the open list, Mod S, Mod Z and Space still work", () => {
    show(scene("presets"));
    const deps: KeyDeps = { state: () => app.current.state, send: app.send, nav: { platform: "Linux" } };
    const on = (ev: KeyboardEvent) => handleKeydown(ev, deps);
    document.addEventListener("keydown", on);
    try {
      q("preset-button")!.click();
      flushSync();
      const row = all("preset-factory")[0]!;
      row.focus();
      for (const init of [{ key: "s", code: "KeyS", ctrlKey: true }, { key: "z", code: "KeyZ", ctrlKey: true }, { key: " ", code: "Space" }])
        row.dispatchEvent(new KeyboardEvent("keydown", { bubbles: true, cancelable: true, ...init }));
      expect(app.sent.map((c) => c.type)).toEqual(["saveSong", "undo", "playStop"]);
      expect(q("preset-menu")).not.toBeNull();
    } finally {
      document.removeEventListener("keydown", on);
    }
  });

  it("「いまの設定を残す…」 is a name box: Enter keeps, Esc puts the row back, then Esc closes the list", () => {
    show(scene("presets"));
    q("preset-button")!.click();
    flushSync();
    q("preset-save")!.click();
    flushSync();
    const box = q("preset-name") as HTMLInputElement;
    expect(box.maxLength).toBe(40);
    box.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    flushSync();
    expect(q("preset-name")).toBeNull();
    q("preset-save")!.click();
    flushSync();
    const again = q("preset-name") as HTMLInputElement;
    again.value = "ピアノ 2";
    again.dispatchEvent(new Event("input", { bubbles: true }));
    again.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    flushSync();
    const s = app.current.state!;
    expect(app.sent.at(-1)).toEqual({ type: "savePreset", insert: builtin(s).insert, name: "ピアノ 2" });
    expect(q("preset-menu")).toBeNull();
    q("preset-button")!.click();
    flushSync();
    q("preset-menu")!.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    flushSync();
    expect(q("preset-menu")).toBeNull();
  });

  it("right click on a preset of the PC: 「消す」", () => {
    show(scene("presets"));
    q("preset-button")!.click();
    flushSync();
    all("preset-user")[0]!.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, clientX: 10, clientY: 10 }));
    const rows = contextMenu.open!.entries;
    expect(rows.length).toBe(1);
    const row = rows[0]!;
    if (row.kind !== "row") throw new Error("a row");
    expect(texts[row.label].text).toBe("消す");
    row.run();
    expect(app.sent.at(-1)).toEqual({ type: "deletePreset", effect: "compressor", name: "いつものピアノ" });
  });
});

describe("the status line", () => {
  it("written over, lanes kept, too many", () => {
    const say = (message: AppState["status"]["message"]) => {
      const s = scene("presets");
      s.status.message = message;
      app.setState(s);
      const m = mount(StatusLine, { target });
      flushSync();
      const shown = text(target.querySelector('[data-test="status-left"]') ?? target);
      unmount(m);
      return shown;
    };
    expect(say({ id: "presetOverwritten", name: "いつものピアノ" })).toContain("「いつものピアノ」を上書きしました");
    expect(say({ id: "presetLanesKept", count: 2 })).toContain("レーンが動かしている 2 つのパラメータは、そのままにしました");
    expect(say({ id: "tooManyPresets", effect: "compressor" })).toContain("コンプレッサーのプリセットは 100 個までなので、これ以上残せません。");
  });
});

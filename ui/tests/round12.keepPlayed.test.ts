// R1 「さっきのを残す」: the button right of 「重ねる」, Shift R, the row of the play menu, the status line, the fake's answer.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { createFakeTransport, fakeScene, fakeState, type FakeState } from "../src/bridge/fake";
import { transportWithKeep, transportWithOverdub } from "../src/components/fit";
import { keyNames, topMenus, type ListRow } from "../src/components/menu/menus";
import StatusLine from "../src/components/StatusLine.svelte";
import Transport from "../src/components/Transport.svelte";
import { keyCommand, type KeyDeps } from "../src/keys/attach";
import { ACTION_ROWS, keyListGroups, WAVE2_ROWS } from "../src/keys/keyList";
import { keyAction, type KeyInput } from "../src/keys/keys";
import { stepNames, t } from "../src/text/ja";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state;
const k = (key: string, mods: Partial<KeyInput> = {}): KeyInput => ({ key, ctrl: false, meta: false, alt: false, shift: false, ...mods });
const depsOf = (state: AppState, sent: Command[]): KeyDeps => ({ state: () => state, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0 });

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  target = document.createElement("div");
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState): void {
  if (mounted) unmount(mounted);
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}

describe("the wording of R1", () => {
  it("is the spec's", () => {
    expect(t("transport.keepPlayed")).toBe("さっきのを残す");
    expect(t("menu.keepPlayed")).toBe("さっきの演奏を残す");
    expect(t("msg.nothingToKeep")).toBe("残せる演奏がありません");
    expect(stepNames.keepPlayed.text).toBe("さっきの演奏を残す");
  });
});

describe("Shift R", () => {
  it("is 「さっきのを残す」; R alone still records; not while typing; read from the code on a layout with no Latin letters", () => {
    expect(keyAction(k("R", { shift: true }), false, "other")).toEqual({ kind: "keepPlayed" });
    expect(keyAction(k("r"), false, "other")).toEqual({ kind: "record" });
    expect(keyAction(k("R", { shift: true }), true, "other")).toBeNull();
    expect(keyAction(k("す", { shift: true, code: "KeyR" }), false, "other")).toEqual({ kind: "keepPlayed" });
    expect(keyAction(k("r", { shift: true, ctrl: true }), false, "other")).toBeNull();
  });

  it("sends keepPlayed with nothing to keep too (the status line then says so)", () => {
    const s = fakeState();
    expect(s.transport.canKeepPlayed).toBe(false);
    expect(keyCommand({ kind: "keepPlayed" }, s, depsOf(s, []))).toEqual({ type: "keepPlayed" });
  });

  it("is in the key list under 「再生と録音」's group, and no longer among the keys to come", () => {
    expect(ACTION_ROWS.keepPlayed).toHaveLength(1);
    expect(WAVE2_ROWS.map((r) => r.label)).not.toContain("keyList.keepLast");
    const play = keyListGroups(keyNames("other")).find((g) => g.group === "play")!;
    const rows = play.rows.filter((r) => r.label === "さっきのを残す");
    expect(rows).toEqual([{ label: "さっきのを残す", keys: "Shift R" }]);
  });
});

describe("the button in the top band", () => {
  const button = () => target.querySelector<HTMLElement>('[data-test="transport-keep-played"]')!;

  it("is right of 「重ねる」, disabled while there is nothing to keep, and sends nothing then", () => {
    show(Transport, scene("nothingToKeep"));
    expect(button().previousElementSibling?.getAttribute("data-test")).toBe("transport-overdub");
    expect(button().textContent?.trim()).toBe("さっきのを残す");
    expect([button().getAttribute("aria-disabled"), button().classList.contains("on")]).toEqual(["true", false]);
    button().click();
    expect(app.sent).toEqual([]);
  });

  it("with something played it is lit and sends keepPlayed", () => {
    show(Transport, scene("keepPlayedReady"));
    expect([button().getAttribute("aria-disabled"), button().classList.contains("on")]).toEqual(["false", true]);
    button().click();
    expect(app.sent).toEqual([{ type: "keepPlayed" }]);
  });

  it("in a narrow window it is the first thing to hide: nothing that was there before gives way to it", () => {
    const w = { base: 800, keys: 100, songName: 144, load: 60, countIn: 227, positionLabels: 20 };
    const stubs = { songName: 40, countIn: 20 };
    const all = w.base + w.keys + w.songName + w.load + w.countIn + w.positionLabels;
    const shown = { keys: true, songName: true, load: true, countIn: true, positionLabels: true };
    expect(transportWithKeep(all + 60 + 90, w, stubs, 60, 90)).toEqual({ ...shown, overdub: true, keepPlayed: true });
    // One px short: only the button goes, and everything else is as without it.
    expect(transportWithKeep(all + 60 + 89, w, stubs, 60, 90)).toEqual({ ...shown, overdub: true, keepPlayed: false });
    for (const width of [all + 60, all + 59, all, 1100, 900]) {
      expect(transportWithKeep(width, w, stubs, 60, 90)).toEqual({ ...transportWithOverdub(width, w, stubs, 60), keepPlayed: false });
    }
  });
});

describe("「再生」 menu", () => {
  const row = (s: AppState, sent: Command[] = []) => {
    const entries = topMenus(s, depsOf(s, sent), "other").flatMap((m) => m.entries);
    const rows = entries.filter((e): e is ListRow => e.kind === "row");
    const at = rows.findIndex((r) => r.label === "さっきの演奏を残す");
    return { row: rows[at]!, before: rows[at - 1]!.label };
  };

  it("「さっきの演奏を残す」 is below 「録音」 with Shift R, usable only with something to keep", () => {
    const nothing = row(scene("nothingToKeep"));
    expect([nothing.before, nothing.row.key, nothing.row.usable]).toEqual(["録音", "Shift R", false]);
    const sent: Command[] = [];
    const ready = row(scene("keepPlayedReady"), sent);
    expect(ready.row.usable).toBe(true);
    ready.row.run!();
    expect(sent).toEqual([{ type: "keepPlayed" }]);
  });
});

describe("the status line", () => {
  it("「残せる演奏がありません」", () => {
    show(StatusLine, scene("nothingToKeep"));
    expect(target.querySelector(".message")?.textContent?.trim()).toBe("残せる演奏がありません");
  });
});

describe("the fake's 「さっきのを残す」", () => {
  const opened = async (state: FakeState) => {
    let now: AppState = state;
    const transport = createFakeTransport(state);
    await transport.subscribe(
      (m) => {
        if (m.kind !== "patch") return;
        for (const c of m.changes) now = { ...now, [c.section]: c.value };
      },
      () => {},
    );
    return { now: () => now, transport };
  };

  it("with nothing played says so and changes no song", async () => {
    const s = scene("nothingToKeep");
    s.status.message = null;
    const { now, transport } = await opened(s);
    await transport.send({ type: "keepPlayed" });
    expect(now().status.message).toEqual({ id: "nothingToKeep" });
    expect(now().song).toEqual(s.song);
  });

  it("puts a clip of two bars on the keys track at the bar of the play line, selected, as one step; then there is nothing left", async () => {
    const s = scene("keepPlayedReady");
    const keys = s.song.tracks.find((x) => x.id === s.song.keysTarget)!;
    keys.clips = [];
    s.transport.position += 700;
    const { now, transport } = await opened(s);
    await transport.send({ type: "keepPlayed" });
    const after = now();
    const track = after.song.tracks.find((x) => x.id === s.song.keysTarget)!;
    expect(track.clips).toHaveLength(1);
    const clip = track.clips[0]!;
    const bar = 4 * 1920;
    expect([clip.start, clip.end - clip.start, clip.noteCount]).toEqual([Math.floor(s.transport.position / bar) * bar, 2 * bar, 4]);
    expect(after.arrange.selectedClip).toBe(clip.id);
    expect(after.song.undoCount).toBe(s.song.undoCount + 1);
    expect(after.transport.canKeepPlayed).toBe(false);
    expect(after.transport.state).toBe("stopped");
    const rows = await transport.historyRows(after.song.undoCount, after.song.undoCount);
    expect(rows[0]).toMatchObject({ name: { kind: "keepPlayed" }, what: { kind: "take", number: 1 } });
    await transport.send({ type: "keepPlayed" });
    expect(now().status.message).toEqual({ id: "nothingToKeep" });
    expect(now().song.undoCount).toBe(after.song.undoCount);
  });

  it("does nothing while recording", async () => {
    const s = scene("keepPlayedReady");
    s.transport.state = "recording";
    const { now, transport } = await opened(s);
    await transport.send({ type: "keepPlayed" });
    expect(now().song).toEqual(s.song);
    expect(now().transport.canKeepPlayed).toBe(true);
  });
});

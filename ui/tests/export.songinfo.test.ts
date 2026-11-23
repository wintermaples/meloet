// Screen: 「曲の情報」 of the export (round 12 F2): 「題名」 and 「作者」 typed and sent once, 「絵」 with 「えらぶ…」 and
// 「はずす」 (not for a wav), and the status lines of a picture that cannot be used. The fake follows Rust's rules
// (crates/meloet-core/src/export.rs).
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command, StatusMessage } from "../src/bridge";
import { fakeScene, reduceExport, type FakeState } from "../src/bridge/fake";
import ExportDialog from "../src/components/export/ExportDialog.svelte";
import { closeContextMenu } from "../src/components/menu/contextMenu.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { exportShown } from "../src/state/exportShown.svelte";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string) => fakeScene(name)!.state as FakeState;
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;

let mounted: Record<string, unknown> | null = null;
let target: HTMLElement;
beforeEach(() => {
  app.reset();
  exportShown.forget();
  target = document.createElement("div");
  target.className = "app";
  document.body.append(target);
});
afterEach(() => {
  if (mounted) unmount(mounted);
  mounted = null;
  target.remove();
  closeContextMenu();
  exportShown.forget();
});
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState): void {
  if (mounted) unmount(mounted);
  app.setState(state);
  mounted = mount(component, { target });
  flushSync();
}
const q = <T extends HTMLElement = HTMLElement>(sel: string) => target.querySelector<T>(sel);
const key = (el: HTMLElement, k: string, more: KeyboardEventInit = {}) => {
  const e = new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...more });
  el.dispatchEvent(e);
  flushSync();
  return e;
};
const typeInto = (input: HTMLInputElement, value: string) => {
  input.focus();
  input.value = value;
  input.dispatchEvent(new Event("input", { bubbles: true }));
  flushSync();
};

describe("「曲の情報」 in 「くわしく」 (round 12 F2)", () => {
  it("exportSongInfo: after 「実時間で書き出す」, 「題名」 is the song's name, 「作者」, and 「絵」 with its name", () => {
    show(ExportDialog, scene("exportSongInfo"));
    expect(text(q('[data-test="export-song-info"]'))).toBe("曲の情報");
    expect(q<HTMLInputElement>('[data-test="export-title"]')!.value).toBe("はじめての曲");
    expect(q<HTMLInputElement>('[data-test="export-artist"]')!.value).toBe("わたし");
    expect(text(q('[data-test="export-picture"]'))).toBe("表紙.png");
    expect(text(q('[data-test="export-remove-picture"]'))).toBe("はずす");
    expect(text(q('[data-test="export-choose-picture"]'))).toBe("えらぶ…");
    expect(q<HTMLButtonElement>('[data-test="export-choose-picture"]')!.disabled).toBe(false);
    const names = [...q('[data-test="export-adv"]')!.querySelectorAll(".row .name")].map(text);
    expect(names.slice(-3)).toEqual(["題名", "作者", "絵"]);
  });

  it("exportSongInfoWav: the row 「絵」 is there and cannot be used; no picture says 「なし」", () => {
    show(ExportDialog, scene("exportSongInfoWav"));
    expect(q('[data-test="export-picture-row"]')!.classList.contains("off")).toBe(true);
    expect(q<HTMLButtonElement>('[data-test="export-choose-picture"]')!.disabled).toBe(true);
    expect(text(q('[data-test="export-picture"]'))).toBe("なし");
    expect(q('[data-test="export-remove-picture"]')).toBeNull();
  });

  it("what is typed is sent once, when Enter is pressed or the box is left, and not the export", () => {
    show(ExportDialog, scene("exportSongInfo"));
    const title = q<HTMLInputElement>('[data-test="export-title"]')!;
    typeInto(title, "はじめての");
    typeInto(title, "はじめての歌");
    expect(app.sent).toEqual([]);
    const enter = key(title, "Enter");
    expect(enter.defaultPrevented).toBe(true);
    const artist = q<HTMLInputElement>('[data-test="export-artist"]')!;
    typeInto(artist, "ぼく");
    artist.blur();
    flushSync();
    expect(app.sent).toEqual([
      { type: "setExport", change: { kind: "title", title: "はじめての歌" } },
      { type: "setExport", change: { kind: "artist", artist: "ぼく" } },
    ] satisfies Command[]);
  });

  it("Esc puts back what Rust has and sends nothing; Enter while the IME composes is the IME's", () => {
    show(ExportDialog, scene("exportSongInfo"));
    const title = q<HTMLInputElement>('[data-test="export-title"]')!;
    typeInto(title, "べつの");
    expect(key(title, "Enter", { isComposing: true }).defaultPrevented).toBe(false);
    key(title, "Escape");
    expect(title.value).toBe("はじめての曲");
    expect(app.sent).toEqual([]);
    // The same text again sends nothing.
    typeInto(title, "はじめての曲");
    title.blur();
    flushSync();
    expect(app.sent).toEqual([]);
  });

  it("「えらぶ…」 and 「はずす」 send their intents", () => {
    show(ExportDialog, scene("exportSongInfo"));
    q('[data-test="export-choose-picture"]')!.click();
    q('[data-test="export-remove-picture"]')!.click();
    expect(app.sent).toEqual([
      { type: "setExport", change: { kind: "choosePicture" } },
      { type: "setExport", change: { kind: "removePicture" } },
    ] satisfies Command[]);
  });
});

describe("the status lines of a picture that is not taken (round 12 F2)", () => {
  const said = (message: StatusMessage) => {
    show(
      StatusLine,
      app.stateWith((s) => {
        s.status.message = message;
      }),
    );
    return target.querySelector("footer.status")?.textContent?.replace(/\s+/g, " ").trim() ?? "";
  };
  it("too large, and not a png or a jpg", () => {
    expect(said({ id: "exportPictureTooLarge", file: "表紙.png" })).toContain("「表紙.png」は大きすぎるので使えません（絵は 5 MB まで）。");
    expect(said({ id: "exportPictureUnreadable", file: "表紙.bmp" })).toContain("「表紙.bmp」は絵として読めません（使えるのは png ・ jpg）。");
  });
  it("the scene exportPictureUnreadable says it under the dialog", () => {
    expect(fakeScene("exportPictureUnreadable")!.state.status.message).toEqual({ id: "exportPictureUnreadable", file: "表紙.bmp" });
  });
});

describe("the fake as Rust's rules (round 12 F2)", () => {
  const dialogAfter = (s: FakeState, ...commands: Command[]) => {
    for (const command of commands) {
      const changes = reduceExport(s, command) ?? [];
      for (const c of changes) if (c.section === "export") s.export = c.value;
    }
    return s.export.dialog!;
  };
  const change = (c: Extract<Command, { type: "setExport" }>["change"]): Command => ({ type: "setExport", change: c });

  it("the title and the artist are taken as typed; a wav cannot hold the picture, a flac can", () => {
    const s = scene("exportSongInfo");
    expect(dialogAfter(s, change({ kind: "title", title: "歌" }), change({ kind: "artist", artist: "" }))).toMatchObject({ title: "歌", artist: "" });
    expect(dialogAfter(s, change({ kind: "format", format: "wav" })).pictureUsable).toBe(false);
    expect(dialogAfter(s, change({ kind: "format", format: "flac" })).pictureUsable).toBe(true);
    expect(dialogAfter(s, change({ kind: "removePicture" })).picture).toBeNull();
  });
});

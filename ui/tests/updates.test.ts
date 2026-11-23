// 「新しい版をたしかめる」 (round 12, D1): the status line, the line in 「meloet について」, the row of the advanced
// settings, and the fake's intents.
import { flushSync, mount, unmount, type Component } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { createFakeTransport, fakeScene, fakeState, type FakeState } from "../src/bridge/fake";
import AboutDialog from "../src/components/about/AboutDialog.svelte";
import { closeAbout, openAbout } from "../src/components/about/about.svelte";
import type { LicenseList } from "../src/components/about/licenses";
import SettingsDialog from "../src/components/SettingsDialog.svelte";
import StatusLine from "../src/components/StatusLine.svelte";
import { t } from "../src/text/ja";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const LIST: LicenseList = { meloet: { version: "0.3.0", license: "MIT OR Apache-2.0" }, sounds: [], fonts: [], parts: [] };

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
  closeAbout();
  target.remove();
});

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function show(component: Component<any>, state: AppState, props: Record<string, unknown> = {}): void {
  if (mounted) unmount(mounted);
  app.setState(state);
  mounted = mount(component, { target, props });
  flushSync();
}
const q = (sel: string) => target.querySelector<HTMLElement>(sel);
const text = (el: Element | null) => el?.textContent?.replace(/\s+/g, " ").trim() ?? null;
const scene = (name: string) => fakeScene(name)!.state as FakeState;

async function opened(state: FakeState) {
  const transport = createFakeTransport(state);
  await transport.subscribe(
    () => {},
    () => {},
  );
  const send = async (command: Command) => {
    await transport.send(command);
    return (await transport.snapshot()).state as AppState;
  };
  return { send };
}

describe("wording (user:2026-11-23)", () => {
  it("is the spec's", () => {
    expect(t("update.newVersion", { version: "0.3.0" })).toBe("新しい版 0.3.0 があります");
    expect([t("button.openReleasePage"), t("button.newVersionLater"), t("settings.checkUpdates")]).toEqual(["ページをひらく", "あとで", "新しい版をたしかめる"]);
  });
});

describe("the status line", () => {
  it("says the version with 「ページをひらく」 and 「あとで」, each sending one intent", () => {
    show(StatusLine, scene("newVersion"));
    expect(text(q(".message"))).toBe("新しい版 0.4.0 があります");
    const buttons = [...target.querySelectorAll<HTMLElement>("footer.status button.link")];
    expect(buttons.map((b) => text(b))).toEqual(["ページをひらく", "あとで"]);
    buttons[0]!.click();
    buttons[1]!.click();
    expect(app.sent).toEqual([{ type: "openReleasePage" }, { type: "dismissNewVersion" }]);
  });
});

describe("「meloet について」", () => {
  it("has the line only while a newer version was found", () => {
    openAbout();
    show(AboutDialog, scene("newVersion"), { list: LIST, load: () => Promise.resolve(null) });
    expect(text(q('[data-test="about-new-version"]'))).toBe("新しい版 0.4.0 があります");
    show(AboutDialog, fakeState(), { list: LIST, load: () => Promise.resolve(null) });
    expect(q('[data-test="about-new-version"]')).toBeNull();
  });
});

describe("the row 「新しい版をたしかめる」 of the advanced settings", () => {
  const row = () => q('[data-test="settings-updates"]')!;
  const pick = () => q('[data-test="settings-check-updates"]')!;

  it("comes after 「AI とつなぐ」 and its note, with the choice mark", () => {
    show(SettingsDialog, scene("checkUpdatesOff"));
    expect(row().previousElementSibling).toBe(q('[data-test="settings-ai-note"]'));
    expect(text(row().querySelector(".name"))).toBe("新しい版をたしかめる");
    expect([text(pick()), pick().getAttribute("aria-checked"), row().querySelector(".mark")!.classList.contains("on")]).toEqual(["オフ", "false", false]);
    show(SettingsDialog, scene("aiLinkOff"));
    expect([text(pick()), pick().getAttribute("aria-checked"), row().querySelector(".mark")!.classList.contains("on")]).toEqual(["オン", "true", true]);
  });

  it("pressing sends one setCheckUpdates; the state comes from the core", () => {
    show(SettingsDialog, scene("checkUpdatesOff"));
    pick().click();
    flushSync();
    expect(app.sent).toEqual([{ type: "setCheckUpdates", on: true }]);
    expect(text(pick())).toBe("オフ");
  });
});

describe("the fake's intents", () => {
  it("「あとで」 and 「ページをひらく」 take the status line's notice away and keep the version for 「meloet について」", async () => {
    let { send } = await opened(scene("newVersion"));
    let s = await send({ type: "dismissNewVersion" });
    expect(s.updates).toEqual({ check: true, newVersion: "0.4.0" });
    expect(s.status.message).toBeNull();
    ({ send } = await opened(scene("newVersion")));
    s = await send({ type: "openReleasePage" });
    expect([s.updates.newVersion, s.status.message]).toEqual(["0.4.0", null]);
  });

  it("off takes the found version away; on again finds nothing by itself", async () => {
    const { send } = await opened(scene("newVersion"));
    let s = await send({ type: "setCheckUpdates", on: false });
    expect(s.updates).toEqual({ check: false, newVersion: null });
    expect(s.status.message).toBeNull();
    s = await send({ type: "setCheckUpdates", on: true });
    expect(s.updates).toEqual({ check: true, newVersion: null });
  });

  it("with nothing found, 「ページをひらく」 and 「あとで」 change nothing", async () => {
    const before = fakeState();
    const { send } = await opened(fakeState());
    expect((await send({ type: "openReleasePage" })).updates).toEqual(before.updates);
    expect((await send({ type: "dismissNewVersion" })).status).toEqual(before.status);
  });
});

// Protocol 30: the color themes (「表示 ▸ テーマ ▸」). The shapes, the fake's intent and the scenes in a theme.
import { afterEach, describe, expect, it } from "vitest";
import type { AppState, Command, Theme } from "../src/bridge";
import { createFakeTransport, fakeScene, fakeState, type FakeState } from "../src/bridge/fake";
import { DEFAULT_THEME, THEME_IDS, type ThemeId } from "../src/design/themes";

async function opened(initial?: FakeState) {
  const t = createFakeTransport(initial);
  await t.subscribe(
    () => {},
    () => {},
  );
  const now = async () => (await t.snapshot()).state as AppState;
  const send = async (command: Command) => {
    await t.send(command);
    return now();
  };
  return { now, send };
}

afterEach(() => history.replaceState(null, "", "/"));

describe("shape of protocol 30", () => {
  it("a new state is in the default theme, dark", () => {
    expect(fakeState().workspace.theme).toBe("dark");
    expect(DEFAULT_THEME).toBe("dark");
  });

  it("the names of the contract are the theme ids of the screen", () => {
    const ids: readonly Theme[] = THEME_IDS;
    const back: readonly ThemeId[] = ["dark", "light", "pinkDark", "pinkLight", "mintDark", "mintLight"] satisfies Theme[];
    expect(ids).toEqual(back);
  });
});

describe("fake intent", () => {
  it("setTheme changes only the theme of the workspace. Not a step", async () => {
    const { now, send } = await opened(fakeScene("tempoFolded")!.state);
    const before = await now();
    const s = await send({ type: "setTheme", theme: "mintLight" });
    expect(s.workspace).toEqual({ ...before.workspace, theme: "mintLight" });
    expect(s.song).toEqual(before.song);
    expect(s.song.undoCount).toBe(before.song.undoCount);
  });
});

describe("scenes in a theme", () => {
  it("fakeScene takes a theme; without one the scene is dark", () => {
    expect(fakeScene("empty")!.state.workspace.theme).toBe("dark");
    for (const id of THEME_IDS) expect(fakeScene("empty", 150, undefined, id)!.state.workspace.theme).toBe(id);
    expect(fakeScene("empty", 150, undefined, "pinkDark")!.state.workspace.scale).toBe(150);
  });

  it("?theme= in the URL, with a scene or without; a name not known is left out", async () => {
    history.replaceState(null, "", "/?scene=empty&scale=100&theme=pinkLight");
    expect((await (await opened()).now()).workspace.theme).toBe("pinkLight");
    history.replaceState(null, "", "/?theme=mintDark");
    expect((await (await opened()).now()).workspace.theme).toBe("mintDark");
    history.replaceState(null, "", "/?scene=empty&theme=blue");
    expect((await (await opened()).now()).workspace.theme).toBe("dark");
  });
});

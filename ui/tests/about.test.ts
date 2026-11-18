// meloet について（仕様書 3 章、モック R7-About ・ R7-AboutLicense）と、オープンソースのライセンスの一覧。
import { flushSync, mount, unmount } from "svelte";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command } from "../src/bridge";
import { fakeState } from "../src/bridge/fake";
import AboutDialog from "../src/components/about/AboutDialog.svelte";
import { about, aboutBack, closeAbout, openAbout } from "../src/components/about/about.svelte";
import {
  forgetTexts,
  fullText,
  licenseGroup,
  licenseList,
  licenseText,
  loadTexts,
  partGroups,
  soundLicenseText,
  type LicenseList,
  type Part,
  type TextsFile,
} from "../src/components/about/licenses";
import { topMenus, type ListRow } from "../src/components/menu/menus";
import { handleKeydown, type KeyDeps } from "../src/keys/attach";
import { texts } from "../src/text/ja";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const fs = (globalThis as unknown as { process: { getBuiltinModule(id: "node:fs"): { readFileSync(p: URL, e: "utf8"): string } } }).process.getBuiltinModule("node:fs");
const here = import.meta.url;
const readRepoFile = (path: string) => fs.readFileSync(new URL(`../../${path}`, here), "utf8");

const part = (name: string, license: string, version = "1.0.0"): Part => ({ kind: "crate", name, version, license });

const LIST: LicenseList = {
  meloet: { version: "9.8.7", license: "MIT OR Apache-2.0" },
  sounds: [
    {
      id: "salamander-grand-piano",
      title: "Salamander Grand Piano V3",
      creator: "Alexander Holm",
      license: "CC-BY-3.0",
      source: "https://archive.org/details/SalamanderGrandPianoV3",
      licenseUri: "http://creativecommons.org/licenses/by/3.0/",
      also: null,
      changes: "Cut.",
    },
  ],
  fonts: [
    { name: "IBM Plex Sans JP", package: "@fontsource/ibm-plex-sans-jp", version: "5.3.1", license: "OFL-1.1" },
    { name: "IBM Plex Mono", package: "@fontsource/ibm-plex-mono", version: "5.3.0", license: "OFL-1.1" },
  ],
  parts: [part("serde", "MIT OR Apache-2.0"), part("tokio", "MIT"), part("toml", "Apache-2.0 OR MIT"), part("alloc-stdlib", "BSD-3-Clause")],
};
const TEXTS: TextsFile = {
  parts: {
    "serde@1.0.0": [{ file: "LICENSE-MIT", text: "a" }],
    "toml@1.0.0": [{ file: "LICENSE-APACHE", text: "b" }],
  },
  texts: { a: "MIT License\n\nCopyright serde", b: "Apache License" },
};

let host: HTMLElement | null = null;
let comp: ReturnType<typeof mount> | null = null;
function show(): HTMLElement {
  host = document.createElement("div");
  document.body.append(host);
  comp = mount(AboutDialog, { target: host, props: { list: LIST, load: () => Promise.resolve(TEXTS) } });
  flushSync();
  return host;
}
const settle = async () => {
  for (let i = 0; i < 3; i++) await new Promise((r) => setTimeout(r, 0));
  flushSync();
};
const q = (sel: string) => host!.querySelector<HTMLElement>(sel);
const all = (sel: string) => [...host!.querySelectorAll<HTMLElement>(sel)];
const words = (el: Element | null) => (el?.textContent ?? "").replace(/\s+/g, " ").trim();
const click = (el: HTMLElement | null) => {
  el!.click();
  flushSync();
};

afterEach(() => {
  if (comp) unmount(comp);
  host?.remove();
  comp = host = null;
  closeAbout();
});

describe("一覧の決まり", () => {
  it("ただの OR は名前の順に 1 つの見出し。AND ・ WITH ・ かっこはそのまま", () => {
    expect(licenseGroup("MIT OR Apache-2.0")).toBe("Apache-2.0 OR MIT");
    expect(licenseGroup("Apache-2.0 OR MIT")).toBe("Apache-2.0 OR MIT");
    expect(licenseGroup("(MIT OR Apache-2.0) AND Unicode-3.0")).toBe("(MIT OR Apache-2.0) AND Unicode-3.0");
  });

  it("OR は「または」（モックの書きかた）。CC のライセンスはモックの書きかた", () => {
    expect(licenseText("Apache-2.0 OR MIT")).toBe("Apache-2.0 または MIT");
    expect(licenseText("0BSD OR Apache-2.0 OR MIT")).toBe("0BSD または Apache-2.0 または MIT");
    expect(soundLicenseText("CC-BY-3.0")).toBe("CC BY 3.0");
    expect(soundLicenseText("CC-BY-4.0")).toBe("CC BY 4.0");
    expect(soundLicenseText("CC0-1.0")).toBe("CC0 1.0");
  });

  it("ライセンスごとに、数の多い順。さがす枠は名前とライセンスでしぼる", () => {
    expect(partGroups(LIST.parts, "").map((g) => [g.label, g.parts.map((p) => p.name)])).toEqual([
      ["Apache-2.0 または MIT", ["serde", "toml"]],
      ["BSD-3-Clause", ["alloc-stdlib"]],
      ["MIT", ["tokio"]],
    ]);
    expect(partGroups(LIST.parts, "TOK").map((g) => g.parts.map((p) => p.name))).toEqual([["tokio"]]);
    expect(partGroups(LIST.parts, "bsd").map((g) => g.group)).toEqual(["BSD-3-Clause"]);
    expect(partGroups(LIST.parts, "または").map((g) => g.group)).toEqual(["Apache-2.0 OR MIT"]);
  });

  it("全文はファイルの名前の行と本文。ない部品は null", () => {
    expect(fullText(TEXTS, "serde@1.0.0")).toBe("LICENSE-MIT\n\nMIT License\n\nCopyright serde");
    expect(fullText(TEXTS, "nope@1")).toBeNull();
  });

  it("全文は 1 回だけ読む。読めなければ null", async () => {
    forgetTexts();
    const fetcher = vi.fn(() => Promise.resolve(new Response("{}", { status: 404 })));
    expect(await loadTexts(fetcher)).toBeNull();
    expect(await loadTexts(fetcher)).toBeNull();
    expect(fetcher).toHaveBeenCalledTimes(1);
    expect(fetcher).toHaveBeenCalledWith("licenses/texts.json");
    forgetTexts();
  });
});

describe("作った一覧（generated/licenses.json）", () => {
  it("バージョンは crates/meloet-app/Cargo.toml の版", () => {
    const version = /^version\s*=\s*"([^"]+)"/m.exec(readRepoFile("crates/meloet-app/Cargo.toml"))![1];
    expect(licenseList.meloet.version).toBe(version);
    expect(licenseList.meloet.license).toBe("MIT OR Apache-2.0");
  });

  it("音の素材は tools/sounds/sources.json と同じ並び。どの素材にも「手を加えたこと」の文がある", () => {
    const sources = (JSON.parse(readRepoFile("tools/sounds/sources.json")) as { sources: { id: string }[] }).sources;
    expect(licenseList.sounds.map((s) => s.id)).toEqual(sources.map((s) => s.id));
    for (const s of licenseList.sounds) expect(Object.keys(texts), s.id).toContain(`about.changes.${s.id}`);
  });

  it("書体は IBM Plex Sans JP と IBM Plex Mono（OFL-1.1）。部品には書体を入れない", () => {
    expect(licenseList.fonts.map((f) => [f.name, f.license]).sort()).toEqual([
      ["IBM Plex Mono", "OFL-1.1"],
      ["IBM Plex Sans JP", "OFL-1.1"],
    ]);
    expect(licenseList.parts.some((p) => p.name.startsWith("@fontsource/"))).toBe(false);
    expect(licenseList.parts.length).toBeGreaterThan(100);
  });
});

describe("ヘルプのメニュー", () => {
  const deps = (s: AppState, sent: Command[]): KeyDeps => ({ state: () => s, send: (c) => sent.push(c), playTick: () => 0 });
  const aboutRow = (s: AppState) => topMenus(s, deps(s, []), "other")[6]!.entries[0] as ListRow;

  it("「meloet について」でひらく。ほかのダイアログがひらいている間は使えない", () => {
    const s = fakeState();
    expect(aboutRow(s).label).toBe("meloet について");
    expect(aboutRow(s).usable).toBe(true);
    aboutRow(s).run!();
    expect(about.open).toBe(true);
    expect(about.stage).toBe(1);
    s.workspace.settings = { draftOutput: null, advancedOpen: false } as unknown as AppState["workspace"]["settings"];
    expect(aboutRow(s).usable).toBe(false);
  });
});

describe("1 段目（R7-About）", () => {
  it("名前、ひとこと、バージョン（例の字は出さない）、ライセンス、2 段目へ進む行に本当の数", () => {
    openAbout();
    show();
    expect(words(q(".head"))).toBe("meloet について 閉じるEsc");
    expect(words(q(".name-box"))).toBe("meloet MIDI 鍵盤ではじめる、音楽づくりのアプリ");
    expect(words(q(".facts"))).toBe("バージョン 9.8.7 meloet のライセンス MIT または Apache-2.0");
    expect(words(q("[data-test=about-licenses]"))).toBe("オープンソースのライセンス 音の素材 ・ 書体 ・ 部品 約 4 ▸");
    expect(host!.textContent).not.toContain("（例）");
  });

  it("閉じるでとじる", () => {
    openAbout();
    show();
    click(q("[data-test=about-close]"));
    expect(about.open).toBe(false);
  });
});

describe("2 段目（R7-AboutLicense）", () => {
  async function second() {
    openAbout();
    show();
    click(q("[data-test=about-licenses]"));
    await settle();
  }

  it("見出しは「◂ meloet について ・ オープンソースのライセンス」。音の素材も書体も部品も、はじめはたたんでいる", async () => {
    await second();
    expect(about.stage).toBe(2);
    expect(words(q(".head"))).toBe("◂ meloet について ・ オープンソースのライセンス もどるEsc");
    expect(words(q("[data-test=about-sounds]"))).toBe("▸ 音の素材 1");
    expect(q("[data-test=about-sound]")).toBeNull();
    expect(words(q("[data-test=about-fonts]"))).toBe("▸ 書体 2");
    expect(all("[data-test=about-part]")).toHaveLength(0);
    expect(words(q(".parts-label"))).toBe("部品（ライセンスごと）");
    expect(all("[data-test=about-group]").map(words)).toEqual(["▸ Apache-2.0 または MIT 2", "▸ BSD-3-Clause 1", "▸ MIT 1"]);
  });

  it("音の素材と書体の見出しを押すとひらく", async () => {
    await second();
    click(q("[data-test=about-sounds]"));
    click(q("[data-test=about-fonts]"));
    expect(words(q("[data-test=about-sounds]"))).toBe("▾ 音の素材 1");
    expect(all("[data-test=about-sound] .field").map((f) => [...f.children].map((c) => c.textContent))).toEqual([
      ["名前", "Salamander Grand Piano V3"],
      ["作った人", "Alexander Holm"],
      ["ライセンス", "CC BY 3.0"],
      ["出どころ", "https://archive.org/details/SalamanderGrandPianoV3"],
      ["手を加えたこと", texts["about.changes.salamander-grand-piano"].text],
    ]);
    expect(words(q("[data-test=about-fonts]"))).toBe("▾ 書体 2");
    expect(all("[data-test=about-part]").map(words)).toEqual(["IBM Plex Sans JP OFL-1.1", "IBM Plex Mono OFL-1.1"]);
  });

  it("見出しを押すとひらき、行を押すとその下に全文の枠（1 つずつ）", async () => {
    await second();
    click(all("[data-test=about-group]")[0]!);
    expect(words(all("[data-test=about-group]")[0]!)).toBe("▾ Apache-2.0 または MIT 2");
    const serde = () => q("[data-key='serde@1.0.0']");
    const toml = () => q("[data-key='toml@1.0.0']");
    expect(words(serde())).toBe("serde 1.0.0");
    click(serde());
    expect(words(serde())).toBe("▾ serde 1.0.0");
    expect(all("[data-test=about-text]")).toHaveLength(1);
    expect(serde()!.nextElementSibling?.getAttribute("data-test")).toBe("about-text");
    expect(words(q("[data-test=about-text]"))).toBe("LICENSE-MIT MIT License Copyright serde");
    click(toml());
    expect(all("[data-test=about-text]")).toHaveLength(1);
    expect(words(q("[data-test=about-text]"))).toBe("LICENSE-APACHE Apache License");
    click(toml());
    expect(all("[data-test=about-text]")).toHaveLength(0);
  });

  it("音の素材と書体の見出しでたためる", async () => {
    await second();
    click(q("[data-test=about-sounds]"));
    click(q("[data-test=about-sounds]"));
    expect(words(q("[data-test=about-sounds]"))).toBe("▸ 音の素材 1");
    expect(q("[data-test=about-sound]")).toBeNull();
  });

  it("さがす枠でしぼる。合う物のある見出しはひらいて見せる", async () => {
    await second();
    about.query = "tok";
    flushSync();
    expect(q("[data-test=about-sounds]")).toBeNull();
    expect(q("[data-test=about-fonts]")).toBeNull();
    expect(all("[data-test=about-group]").map(words)).toEqual(["▾ MIT 1"]);
    expect(all("[data-test=about-part]").map(words)).toEqual(["tokio 1.0.0"]);
    about.query = "OFL";
    flushSync();
    expect(all("[data-test=about-part]").map(words)).toEqual(["IBM Plex Sans JP OFL-1.1", "IBM Plex Mono OFL-1.1"]);
    about.query = "Salamander";
    flushSync();
    expect(words(q("[data-test=about-sounds]"))).toBe("▾ 音の素材 1");
    expect(all("[data-test=about-sound]")).toHaveLength(1);
  });

  it("「◂ meloet について」で 1 段目へ", async () => {
    await second();
    click(q("[data-test=about-back]"));
    expect(about.stage).toBe(1);
    expect(about.open).toBe(true);
  });
});

describe("Esc（ほかのダイアログと同じ番）", () => {
  const esc = (deps: KeyDeps) => handleKeydown(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }), deps);

  it("2 段目では 1 段目へ、1 段目ではとじる。とじていれば Rust の back へ", () => {
    const s = fakeState();
    const sent: Command[] = [];
    const deps: KeyDeps = { state: () => s, send: (c) => sent.push(c), nav: { platform: "Linux x86_64" }, playTick: () => 0 };
    openAbout();
    about.stage = 2;
    esc(deps);
    expect([about.open, about.stage]).toEqual([true, 1]);
    esc(deps);
    expect(about.open).toBe(false);
    expect(sent).toEqual([]);
    esc(deps);
    expect(sent).toEqual([{ type: "back" }]);
  });

  it("aboutBack はひらいていなければ何もしない", () => {
    expect(aboutBack()).toBe(false);
  });
});

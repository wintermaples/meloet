#!/usr/bin/env node
// The parts meloet ships, their licenses, and the texts that came with them.
//
//   node tools/licenses/inventory.mjs write        NOTICE and ui/src/components/about/generated/licenses.json
//   node tools/licenses/inventory.mjs texts DIR    DIR/texts.json: every license text, for the about dialog (npm run build)
//   node tools/licenses/inventory.mjs check        exit 1 when NOTICE or licenses.json no longer match the parts
//
// Rust: the crates inside the binaries meloet ships (BINARIES: the app, the plugin host process and the AI bridge), for
// the target of this PC: cargo tree from each over normal dependencies only (as `cargo build -p` resolves
// features); dev and build dependencies, proc-macro crates (they run in the compiler, nothing of them is
// linked) and the workspace crates are left out. Licenses and folders come from cargo metadata.
// A crate made from somebody else's work whose text the crate does not carry (bindings generated from
// headers) has that text in licenses/upstream.json; it goes into NOTICE and beside the crate's own texts.
// npm: the packages whose modules end up in the bundle that `vite build` writes (the dependencies of
// ui/package.json that are really used; the fonts come in through their CSS).
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export const ROOT = join(here, "..", "..");
export const LIST_PATH = join("ui", "src", "components", "about", "generated", "licenses.json");
export const NOTICE_PATH = "NOTICE";

/** The workspace crates that are shipped as programs. */
export const BINARIES = ["meloet-app", "meloet-plughost", "meloet-mcp"];
export const UPSTREAM_PATH = join("licenses", "upstream.json");

const TEXT_FILE = /^(licen[cs]e|copying|copyright|notice|unlicense)([-._].*)?$/i;
const FONT_PACKAGE = /^@fontsource\//;

/** "Apache-2.0/MIT" (old crates.io style) to "Apache-2.0 OR MIT". */
export function normalizeLicense(expr) {
  return String(expr ?? "")
    .replace(/\s*\/\s*/g, " OR ")
    .replace(/\s+/g, " ")
    .trim();
}

/** The group a part goes in: a plain OR of ids is sorted, so "MIT OR Apache-2.0" and "Apache-2.0 OR MIT" meet. */
export function licenseGroup(expr) {
  const e = normalizeLicense(expr);
  if (/[()]|\bAND\b|\bWITH\b/.test(e)) return e;
  return e.split(" OR ").sort((a, b) => a.localeCompare(b, "en")).join(" OR ");
}

function hostTarget() {
  const out = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  return /^host: (.+)$/m.exec(out)[1].trim();
}

function cargoMetadata(root, target) {
  const json = execFileSync(
    "cargo",
    ["metadata", "--format-version", "1", "--locked", "--offline", "--filter-platform", target],
    { cwd: root, encoding: "utf8", maxBuffer: 256 * 1024 * 1024 },
  );
  return JSON.parse(json);
}

/**
 * name@version of the crates in a `cargo build -p <binary>` of each of BINARIES for this target. cargo tree,
 * not the resolve of cargo metadata: the resolve joins the features of every workspace member (spikes, dev
 * and build dependencies), and lists about 60 crates more than the binaries have.
 */
function builtCrates(root, target) {
  const set = new Set();
  for (const binary of BINARIES) {
    const out = execFileSync(
      "cargo",
      ["tree", "-p", binary, "-e", "normal,no-proc-macro", "--target", target, "--prefix", "none", "-f", "{p}", "--locked", "--offline"],
      { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
    );
    for (const line of out.split("\n")) {
      const m = /^(\S+) v(\S+)/.exec(line.trim());
      if (m) set.add(`${m[1]}@${m[2]}`);
    }
  }
  return set;
}

/**
 * The work of others inside a crate that the crate's own files do not name:
 * [{ crate, title, copyright, license, source, text }], `text` a file under licenses/.
 */
export function upstreamParts(root = ROOT) {
  const path = join(root, UPSTREAM_PATH);
  if (!existsSync(path)) return [];
  return JSON.parse(readFileSync(path, "utf8")).upstream;
}

/**
 * The name and version of copied source files (`vendored`), read from the title ("Signalsmith Stretch 1.4.0 (...)"):
 * the about dialog shows them as a part of their own, and its full text is keyed by `name@version`.
 */
export function vendoredPart(u) {
  const m = /^(.+?) v?(\d+(?:\.\d+)+) \(/.exec(u.title);
  if (!m) throw new Error(`upstream.json: no name and version at the head of the title "${u.title}"`);
  return { name: m[1], version: m[2] };
}

/** Crates linked into the shipped binaries: [{ name, version, license, repository, dir }]. Details from cargo metadata. */
export function rustParts(root = ROOT) {
  const target = hostTarget();
  const meta = cargoMetadata(root, target);
  const built = builtCrates(root, target);
  const workspace = new Set(meta.workspace_members);
  for (const binary of BINARIES)
    if (!built.has(`${binary}@${meta.packages.find((p) => p.name === binary)?.version}`)) throw new Error(`cargo tree did not list ${binary}`);
  const seen = meta.packages.filter((p) => !workspace.has(p.id) && built.has(`${p.name}@${p.version}`)).map((p) => p.id);
  const pkgs = new Map(meta.packages.map((p) => [p.id, p]));
  return seen
    .map((id) => pkgs.get(id))
    .map((p) => ({
      name: p.name,
      version: p.version,
      license: normalizeLicense(p.license) || "UNKNOWN",
      repository: p.repository ?? null,
      dir: dirname(p.manifest_path),
      licenseFile: p.license_file ? join(dirname(p.manifest_path), p.license_file) : null,
    }))
    .sort(byNameVersion);
}

function byNameVersion(a, b) {
  return a.name.localeCompare(b.name, "en") || a.version.localeCompare(b.version, "en", { numeric: true });
}

/** The package that owns a module path inside node_modules: "/…/node_modules/@scope/name/…" → its folder. */
export function packageDirOf(id) {
  const path = id.split("?")[0].replace(/\\/g, "/");
  const at = path.lastIndexOf("/node_modules/");
  if (at < 0) return null;
  const rest = path.slice(at + "/node_modules/".length).split("/");
  const n = rest[0].startsWith("@") ? 2 : 1;
  if (rest.length < n) return null;
  return path.slice(0, at) + "/node_modules/" + rest.slice(0, n).join("/");
}

/** npm packages in the screen's bundle: [{ name, version, license, repository, dir }]. */
export async function npmParts(root = ROOT) {
  const ui = join(root, "ui");
  const require = createRequire(join(ui, "package.json"));
  const { build } = await import(pathToFileURL(require.resolve("vite")).href);
  const dirs = new Set();
  await build({
    root: ui,
    configFile: join(ui, "vite.config.ts"),
    logLevel: "silent",
    build: { write: false },
    plugins: [
      {
        name: "meloet-license-inventory",
        generateBundle(_options, bundle) {
          for (const chunk of Object.values(bundle)) {
            if (chunk.type !== "chunk") continue;
            for (const id of chunk.moduleIds) {
              const dir = packageDirOf(id);
              if (dir) dirs.add(dir);
            }
          }
        },
      },
    ],
  });
  return [...dirs]
    .map((dir) => {
      const j = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
      const lic = typeof j.license === "string" ? j.license : j.license?.type ?? (j.licenses || []).map((l) => l.type).join(" OR ");
      const repo = typeof j.repository === "string" ? j.repository : j.repository?.url ?? null;
      return { name: j.name, version: j.version, license: normalizeLicense(lic) || "UNKNOWN", repository: repo, dir };
    })
    .sort(byNameVersion);
}

function cargoField(toml, section, key) {
  const lines = toml.split("\n");
  let inSection = false;
  for (const line of lines) {
    const head = /^\s*\[([^\]]+)\]\s*$/.exec(line);
    if (head) {
      inSection = head[1].trim() === section;
      continue;
    }
    const kv = new RegExp(`^\\s*${key.replace(".", "\\.")}\\s*=\\s*(.+?)\\s*$`).exec(line);
    if (inSection && kv) return kv[1];
  }
  return null;
}

/** meloet itself: the version of crates/meloet-app and the license of the workspace. */
export function appInfo(root = ROOT) {
  const app = readFileSync(join(root, "crates", "meloet-app", "Cargo.toml"), "utf8");
  const ws = readFileSync(join(root, "Cargo.toml"), "utf8");
  const unquote = (v) => (v ? JSON.parse(v) : null);
  let version = cargoField(app, "package", "version");
  version = version === "{ workspace = true }" || cargoField(app, "package", "version.workspace") ? cargoField(ws, "workspace.package", "version") : version;
  let license = cargoField(app, "package", "license");
  if (!license || cargoField(app, "package", "license.workspace")) license = cargoField(ws, "workspace.package", "license");
  return { version: unquote(version), license: normalizeLicense(unquote(license)) };
}

/** CHANGES of tools/sounds/build.py: what the pack builder does to each source, one English sentence. */
export function soundChanges(root = ROOT) {
  const py = readFileSync(join(root, "tools", "sounds", "build.py"), "utf8");
  const block = /^CHANGES = \{\n([\s\S]*?)^\}/m.exec(py);
  if (!block) throw new Error("tools/sounds/build.py: no CHANGES table");
  const out = {};
  for (const m of block[1].matchAll(/^\s*"([^"]+)":\s*"((?:[^"\\]|\\.)*)",?\s*$/gm)) out[m[1]] = JSON.parse(`"${m[2]}"`);
  return out;
}

/** The credits of the sampled instruments (tools/sounds/sources.json). */
export function soundParts(root = ROOT) {
  const sources = JSON.parse(readFileSync(join(root, "tools", "sounds", "sources.json"), "utf8")).sources;
  const changes = soundChanges(root);
  for (const s of sources) if (!changes[s.id]) throw new Error(`tools/sounds/build.py: CHANGES has no entry for ${s.id}`);
  return sources.map((s) => ({
    id: s.id,
    title: s.attribution.title,
    creator: s.attribution.creator,
    license: s.license,
    source: s.attribution.source_uri,
    licenseUri: s.attribution.license_uri,
    also: s.attribution.also ?? null,
    changes: changes[s.id],
  }));
}

/** The list the about dialog reads (committed; small). Texts are not in it. */
export async function makeList(root = ROOT) {
  const npm = await npmParts(root);
  const rust = rustParts(root);
  const fonts = npm
    .filter((p) => FONT_PACKAGE.test(p.name))
    .map((p) => {
      const meta = JSON.parse(readFileSync(join(p.dir, "metadata.json"), "utf8"));
      return { name: meta.family, package: p.name, version: p.version, license: p.license };
    });
  // The source location is kept only where a license asks for it (MPL-2.0: NOTICE says where the source is).
  const strip = ({ name, version, license, repository }) => (/\bMPL-/.test(license) ? { name, version, license, repository } : { name, version, license });
  return {
    meloet: appInfo(root),
    sounds: soundParts(root),
    fonts,
    // `vendored`: files of others kept inside one of our own crates (no cargo part stands for them).
    upstream: upstreamParts(root)
      .filter((u) => u.vendored || rust.some((p) => p.name === u.crate))
      .map((u) => (u.vendored ? { ...u, ...vendoredPart(u) } : u)),
    parts: [
      ...rust.map((p) => ({ kind: "crate", ...strip(p) })),
      ...npm.filter((p) => !FONT_PACKAGE.test(p.name)).map((p) => ({ kind: "npm", ...strip(p) })),
    ].sort(byNameVersion),
  };
}

/** NOTICE at the top of the repository. Goes out with meloet. */
export function noticeText(list) {
  const out = [];
  const line = (s = "") => out.push(s);
  line("meloet");
  line(`Version ${list.meloet.version}. Licensed under ${list.meloet.license.replace(" OR ", " or ")} (LICENSE-MIT, LICENSE-APACHE).`);
  line();
  line("This file lists the work of others that ships with meloet. The about dialog of meloet");
  line("(Help > About meloet > Open source licenses) shows the same list with the full license texts.");
  line("This file is made by tools/licenses/inventory.mjs; do not edit it by hand.");
  line();
  line("== Sound samples");
  line();
  for (const s of list.sounds) {
    line(`${s.title}`);
    line(`  by ${s.creator}`);
    line(`  source: ${s.source}`);
    if (s.also) line(`  also: ${s.also}`);
    line(`  license: ${s.license}, ${s.licenseUri}`);
    line(`  changes made for meloet: ${s.changes}`);
    line();
  }
  line("The license texts that came with each sample set are in licenses/sounds/.");
  line();
  line("== Fonts");
  line();
  for (const f of list.fonts) line(`${f.name} (${f.package} ${f.version}): ${f.license}. Copyright IBM Corp. The woff2 files are used without change.`);
  line('The license text of IBM (github.com/IBM/plex, LICENSE.txt) names "Plex" as a Reserved Font Name.');
  line();
  const mpl = list.parts.filter((p) => /\bMPL-2\.0\b/.test(p.license));
  line("== Source code of the MPL-2.0 parts");
  line();
  line("meloet uses these parts without changing them. Their source code is here:");
  for (const p of mpl) line(`${p.name} ${p.version}: ${p.repository ?? `https://crates.io/crates/${p.name}/${p.version}`} (also https://crates.io/crates/${p.name}/${p.version})`);
  line();
  if (list.upstream.length) {
    line("== Parts made from the work of others");
    line();
    for (const u of list.upstream) {
      line(`${u.crate} (Rust): ${u.title}`);
      line(`  ${u.copyright}`);
      line(`  source: ${u.source}`);
      line(`  license: ${u.license}; the text is in ${u.text}`);
      line();
    }
  }
  line(`== Parts (${list.parts.length}: ${list.parts.filter((p) => p.kind === "crate").length} Rust crates, ${list.parts.filter((p) => p.kind === "npm").length} npm packages)`);
  line();
  for (const p of list.parts) line(`${p.name} ${p.version} (${p.kind === "crate" ? "Rust" : "npm"}): ${p.license}`);
  return out.join("\n") + "\n";
}

function readTexts(dir, extra) {
  const files = [];
  if (extra && existsSync(extra) && statSync(extra).isFile()) files.push(extra);
  for (const f of readdirSync(dir)) {
    const p = join(dir, f);
    if (!TEXT_FILE.test(f)) continue;
    if (statSync(p).isFile()) files.push(p);
    else if (statSync(p).isDirectory()) for (const g of readdirSync(p)) if (statSync(join(p, g)).isFile()) files.push(join(p, g));
  }
  return [...new Set(files)].sort().map((p) => ({ file: relative(dir, p).split(sep).join("/"), text: readFileSync(p, "utf8") }));
}

/** SPDX standard texts, from the spdx crate in the cargo registry (the crate cargo-deny uses). */
function spdxTexts() {
  const home = process.env.CARGO_HOME ?? join(process.env.HOME ?? "", ".cargo");
  const src = join(home, "registry", "src");
  const found = [];
  if (existsSync(src))
    for (const reg of readdirSync(src))
      for (const d of readdirSync(join(src, reg))) {
        const m = /^spdx-(\d+)\.(\d+)\.(\d+)$/.exec(d);
        if (m && existsSync(join(src, reg, d, "src", "text", "licenses"))) found.push({ dir: join(src, reg, d), v: m.slice(1).map(Number), name: d });
      }
  found.sort((a, b) => b.v[0] - a.v[0] || b.v[1] - a.v[1] || b.v[2] - a.v[2]);
  const best = found[0];
  if (!best) return null;
  const listVersion = /VERSION: &str = "([^"]+)"/.exec(readFileSync(join(best.dir, "src", "identifiers.rs"), "utf8"))?.[1];
  return {
    from: `SPDX License List ${listVersion} (${best.name}, src/text/licenses/)`,
    text(id) {
      const p = join(best.dir, "src", "text", "licenses", id);
      if (!existsSync(p)) return null;
      return readFileSync(p, "utf8").replace(/^r#*"/, "").replace(/"#*\s*$/, "");
    },
  };
}

/** Every text, keyed by its hash (most crates carry the same Apache-2.0 text). */
export async function makeTexts(root = ROOT) {
  const rust = rustParts(root);
  const npm = await npmParts(root);
  const spdx = spdxTexts();
  const texts = {};
  const parts = {};
  const missing = [];
  const keep = (text) => {
    const h = createHash("sha256").update(text).digest("hex").slice(0, 16);
    texts[h] = text;
    return h;
  };
  const upstream = upstreamParts(root);
  for (const p of [...rust, ...npm]) {
    let found = readTexts(p.dir, p.licenseFile);
    for (const u of upstream.filter((u) => rust.includes(p) && u.crate === p.name))
      found.push({ file: `${u.text} (${u.title})`, text: readFileSync(join(root, u.text), "utf8") });
    if (!found.length) {
      const ids = [...new Set(p.license.replace(/[()]/g, " ").split(/\s+(?:OR|AND|WITH)\s+|\s+/).filter(Boolean))];
      found = ids.flatMap((id) => {
        const t = spdx?.text(id);
        return t ? [{ file: `${id} (${spdx.from}; the part ships no license file)`, text: t }] : [];
      });
    }
    if (!found.length) missing.push(`${p.name}@${p.version}`);
    parts[`${p.name}@${p.version}`] = found.map((f) => ({ file: f.file, text: keep(f.text) }));
  }
  for (const u of upstream.filter((u) => u.vendored)) {
    const { name, version } = vendoredPart(u);
    parts[`${name}@${version}`] = [{ file: `${u.text} (${u.title})`, text: keep(readFileSync(join(root, u.text), "utf8")) }];
  }
  return { texts, parts, missing };
}

/** One record per line: small to keep in the repository, and a diff shows which part changed. */
function listJson(list) {
  const rows = (a) => "[\n" + a.map((r) => "  " + JSON.stringify(r)).join(",\n") + "\n ]";
  const body = Object.entries(list).map(([k, v]) => ` ${JSON.stringify(k)}: ${Array.isArray(v) ? rows(v) : JSON.stringify(v)}`);
  return "{\n" + body.join(",\n") + "\n}\n";
}

/** Names of the files that no longer match the parts (empty when all is well). */
export async function stale(root = ROOT) {
  const list = await makeList(root);
  const out = [];
  const same = (path, want) => existsSync(join(root, path)) && readFileSync(join(root, path), "utf8") === want;
  if (!same(LIST_PATH, listJson(list))) out.push(LIST_PATH);
  if (!same(NOTICE_PATH, noticeText(list))) out.push(NOTICE_PATH);
  return out;
}

async function main(argv) {
  const [cmd, arg] = argv;
  if (cmd === "write") {
    const list = await makeList(ROOT);
    mkdirSync(dirname(join(ROOT, LIST_PATH)), { recursive: true });
    writeFileSync(join(ROOT, LIST_PATH), listJson(list));
    writeFileSync(join(ROOT, NOTICE_PATH), noticeText(list));
    const crates = list.parts.filter((p) => p.kind === "crate").length;
    console.log(`wrote ${NOTICE_PATH} and ${LIST_PATH}: ${crates} crates, ${list.parts.length - crates} npm packages, ${list.fonts.length} fonts, ${list.sounds.length} sound sources`);
  } else if (cmd === "texts" && arg) {
    const { texts, parts, missing } = await makeTexts(ROOT);
    if (missing.length) {
      console.error(`no license text for: ${missing.join(", ")}`);
      process.exit(1);
    }
    mkdirSync(arg, { recursive: true });
    writeFileSync(join(arg, "texts.json"), JSON.stringify({ parts, texts }));
    console.log(`wrote ${join(arg, "texts.json")}: ${Object.keys(parts).length} parts, ${Object.keys(texts).length} distinct texts`);
  } else if (cmd === "check") {
    const bad = await stale(ROOT);
    if (bad.length) {
      console.error(`out of date: ${bad.join(", ")} (run: node tools/licenses/inventory.mjs write)`);
      process.exit(1);
    }
    console.log("license list and NOTICE match the parts");
  } else {
    console.error("usage: node tools/licenses/inventory.mjs write | texts DIR | check");
    process.exit(2);
  }
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) await main(process.argv.slice(2));

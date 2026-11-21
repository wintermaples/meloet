#!/usr/bin/env node
// License watchdog. Exit code 1 when a forbidden license shows up.
//  1. Rust crates: `cargo deny check licenses` against deny.toml (allow list).
//  2. npm packages: every package.json under each ui/node_modules, written to npm-inventory.json.
//  3. Things no package manager knows about (system libs, test plugins): system.json.
import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..");
const ALLOW = new Set([
  "MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause",
  "ISC", "Zlib", "0BSD", "Unicode-3.0", "Unicode-DFS-2016", "CC0-1.0", "MPL-2.0",
  "BSL-1.0", "Unlicense", "CDLA-Permissive-2.0", "BlueOak-1.0.0", "Python-2.0", "CC-BY-4.0",
  "OFL-1.1", // fonts (IBM Plex via @fontsource); the font files keep their own license
]);
const FORBIDDEN = /\b(A?GPL)\b/i; // LGPL is matched separately
let bad = 0;
const fail = (msg) => { bad++; console.log("NG  " + msg); };

// An SPDX expression passes when some OR-branch has only allowed ids.
function spdxOk(expr, extra = new Set()) {
  if (!expr) return false;
  const e = String(expr).replace(/[()]/g, " ").trim();
  return e.split(/\s+OR\s+|\//).some((branch) =>
    branch.split(/\s+AND\s+/).every((id) => ALLOW.has(id.trim()) || extra.has(id.trim())));
}

// 1. Rust
if (existsSync(join(root, "Cargo.toml"))) {
  try {
    execFileSync("cargo", ["deny", "--log-level", "error", "check", "licenses"], { cwd: root, stdio: "inherit" });
    console.log("OK  cargo deny check licenses");
  } catch { fail("cargo deny check licenses (see output above)"); }
}

// 2. npm
const npmRows = [];
function walk(nm) {
  if (!existsSync(nm)) return;
  for (const d of readdirSync(nm)) {
    if (d.startsWith(".")) continue;
    const p = join(nm, d);
    if (d.startsWith("@")) { walk(p); continue; }
    const pj = join(p, "package.json");
    if (!existsSync(pj)) continue;
    const j = JSON.parse(readFileSync(pj, "utf8"));
    const lic = typeof j.license === "string" ? j.license : j.license?.type ?? (j.licenses || []).map((l) => l.type).join(" OR ");
    npmRows.push({ name: j.name, version: j.version, license: lic || "UNKNOWN" });
    walk(join(p, "node_modules"));
  }
}
walk(join(root, "ui", "node_modules"));
for (const s of readdirSync(join(root, "spikes"))) walk(join(root, "spikes", s, "ui", "node_modules"));
const seen = new Map();
for (const r of npmRows) seen.set(`${r.name}@${r.version}`, r);
const npmList = [...seen.values()].sort((a, b) => a.name.localeCompare(b.name));
writeFileSync(join(here, "npm-inventory.json"), JSON.stringify(npmList, null, 1) + "\n");
for (const r of npmList) if (!spdxOk(r.license)) fail(`npm ${r.name}@${r.version}: ${r.license}`);
console.log(`OK  npm packages checked: ${npmList.length}`);

// 3. system / test targets
const sys = JSON.parse(readFileSync(join(here, "system.json"), "utf8"));
for (const c of sys.components) {
  if (c.role === "test-target") continue; // loaded by the user's choice, never shipped or linked by us
  // An OS library loaded dynamically may also be LGPL, or AFL-2.1 (libdbus is "AFL-2.1 OR GPL-2.0-or-later"; we take AFL-2.1).
  const dynamicOk = c.role === "system-dynamic" ? new Set(["LGPL-2.0-or-later", "LGPL-2.1-or-later", "LGPL-2.1-only", "LGPL-3.0-or-later", "LGPL-3.0-only", "AFL-2.1"]) : new Set();
  // spdxOk only passes a branch whose every id is allowed, so a GPL id can only sit in a branch we do not take.
  if (!spdxOk(c.license, dynamicOk) || (FORBIDDEN.test(c.license) && !/LGPL|\bOR\b/.test(c.license))) fail(`system ${c.name}: ${c.license} (${c.role})`);
}
console.log(`OK  system components checked: ${sys.components.length}`);

// 4. sound sources (tools/sounds/sources.json): only CC0, CC BY or MIT, credits complete, license texts kept
const SOUND_LICENSES = new Set(["CC0-1.0", "CC-BY-3.0", "CC-BY-4.0", "MIT"]);
const soundsPath = join(root, "tools", "sounds", "sources.json");
if (existsSync(soundsPath)) {
  const sounds = JSON.parse(readFileSync(soundsPath, "utf8")).sources;
  for (const s of sounds) {
    if (!SOUND_LICENSES.has(s.license)) { fail(`sound ${s.id}: ${s.license}`); continue; }
    const a = s.attribution || {};
    if (s.license.startsWith("CC-BY-")) {
      for (const k of ["creator", "title", "source_uri", "license_uri"]) if (!a[k]) fail(`sound ${s.id}: attribution.${k} missing`);
      const ver = s.license.slice("CC-BY-".length);
      if (a.license_uri && !a.license_uri.includes(`/licenses/by/${ver}`)) fail(`sound ${s.id}: license_uri ${a.license_uri} is not CC BY ${ver}`);
    }
    const dir = s.license_copies ? join(root, s.license_copies) : "";
    if (!s.license_copies || !s.license_copies.startsWith("licenses/sounds/") || !existsSync(dir)) { fail(`sound ${s.id}: no license texts in licenses/sounds/`); continue; }
    const texts = readdirSync(dir).map((f) => readFileSync(join(dir, f), "utf8"));
    if (s.license === "MIT") {
      // MIT asks that the copyright notice and the permission notice go with every copy.
      if (!a.creator || !a.title || !a.source_uri) fail(`sound ${s.id}: attribution.creator, title or source_uri missing`);
      const mit = texts.find((t) => /Permission is hereby granted, free of charge/.test(t) && /above copyright notice and this permission notice shall be\s+included/.test(t));
      if (!mit) fail(`sound ${s.id}: no MIT permission notice in ${s.license_copies}`);
      else if (!/Copyright \(c\)/i.test(mit)) fail(`sound ${s.id}: no copyright notice with the MIT text in ${s.license_copies}`);
      continue;
    }
    const mark = s.license === "CC0-1.0" ? /\bCC0\b|CC 0|publicdomain\/zero/i : /Creative\s+Commons|creativecommons\.org\/licenses\/by\//i;
    if (!texts.some((t) => mark.test(t))) fail(`sound ${s.id}: no ${s.license} text in ${s.license_copies}`);
  }
  console.log(`OK  sound sources checked: ${sounds.length}`);
}

// 4b. the work of others inside crates (licenses/upstream.json): license allowed, text kept, vendored files untouched
{
  const { createHash } = await import("node:crypto");
  const { statSync } = await import("node:fs");
  const up = JSON.parse(readFileSync(join(here, "upstream.json"), "utf8")).upstream;
  const sha = (p) => createHash("sha256").update(readFileSync(p)).digest("hex");
  const filesUnder = (dir, rel = "") => readdirSync(join(dir, rel)).flatMap((f) => (statSync(join(dir, rel, f)).isDirectory() ? filesUnder(dir, join(rel, f)) : [join(rel, f)]));
  let vendored = 0;
  for (const u of up) {
    const name = `upstream ${u.crate}: ${u.title}`;
    if (!spdxOk(u.license)) fail(`${name}: ${u.license}`);
    if (!u.text || !existsSync(join(root, u.text))) { fail(`${name}: no license text at ${u.text}`); continue; }
    const v = u.vendored;
    if (!v) continue;
    const dir = join(root, v.dir);
    if (!existsSync(dir)) { fail(`${name}: ${v.dir} is missing`); continue; }
    const there = filesUnder(dir).map((f) => f.split("\\").join("/")).sort();
    for (const f of there) {
      if (!(f in v.files)) fail(`${name}: ${v.dir}/${f} is not listed in upstream.json`);
      else if (sha(join(dir, f)) !== v.files[f]) fail(`${name}: ${v.dir}/${f} was changed (sha256 differs)`);
      else vendored++;
    }
    for (const f of Object.keys(v.files)) if (!there.includes(f)) fail(`${name}: ${v.dir}/${f} is missing`);
    if (existsSync(join(dir, v.license_file)) && sha(join(dir, v.license_file)) !== sha(join(root, u.text))) fail(`${name}: ${u.text} is not the license file that came with it`);
    if (there.some((f) => /\bL?A?GPL\b|General Public License/.test(readFileSync(join(dir, f), "utf8")))) fail(`${name}: a file under ${v.dir} names the GPL`);
  }
  console.log(`OK  parts inside crates checked: ${up.length} (${vendored} vendored files match their sha256)`);
}

// 5. NOTICE and the about dialog's list (tools/licenses/inventory.mjs) still match what ships
{
  const { stale } = await import("../tools/licenses/inventory.mjs");
  try {
    const old = await stale(root);
    if (old.length) fail(`out of date: ${old.join(", ")} (run: node tools/licenses/inventory.mjs write)`);
    else console.log("OK  NOTICE and the about dialog's list match the parts");
  } catch (e) {
    fail(`license list: ${e.message}`);
  }
}
process.exit(bad ? 1 : 0);

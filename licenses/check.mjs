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
  const lgplOk = c.role === "system-dynamic" ? new Set(["LGPL-2.0-or-later", "LGPL-2.1-or-later", "LGPL-2.1-only", "LGPL-3.0-or-later", "LGPL-3.0-only"]) : new Set();
  if (!spdxOk(c.license, lgplOk) || (FORBIDDEN.test(c.license) && !/LGPL/.test(c.license))) fail(`system ${c.name}: ${c.license} (${c.role})`);
}
console.log(`OK  system components checked: ${sys.components.length}`);
process.exit(bad ? 1 : 0);

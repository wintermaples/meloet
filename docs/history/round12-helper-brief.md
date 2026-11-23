# Round 12: common brief for feature helpers

You build ONE feature (or a short chain of features) of round 12. The lead's request names the feature id(s). Other helpers build other features at the same time in their own worktrees, on the same base commit; the lead merges.

## Start

1. Your worktree was made from `main`. Run the `git reset -q --hard <commit>` the request gives (the head of `fea/round12`).
2. `source ~/.cargo/env; export PATH=$HOME/.local/node/bin:$PATH`
3. `ln -s /srv/meloet-agent/meloet/ui/node_modules ui/node_modules` (do not run `npm install`), and `mkdir -p ui/dist` if clippy of `meloet-app` needs it (do not add `ui/dist` to git).
4. Always build with `CARGO_TARGET_DIR=target/<your name>` (the request gives the name) and `cargo -p <crate>`; never plain `cargo build` of the whole workspace unless needed. Never use the real tree's `target/`.
5. Scratch files: start their names with your name.

## Read

- `CLAUDE.md` (the rules of the project; they all apply).
- The spec (Japanese HTML), `docs/history/round12-spec.ja.html`: lines 48-158 (what the owner decided, common rules, acceptance conditions for every feature) and the section(s) of your feature (line numbers in the request). The mock-ups inside are HTML; when a picture and the text disagree, the text is right.
- The spec calls itself "round 11". It is **round 12** (round 11 was the AI link). Read "round 11" as "round 12" everywhere. Source tag for wording: `user:2026-11-23`.
- Code line numbers in the spec are old. Find things by name. Where the spec says "(未確認)", read the code and make sure first.
- The chapters of `docs/architecture.md` and `docs/spec/` that cover the part you touch.

## Hard rules for this round

- **Do not touch** `PROTOCOL_VERSION`, the shape of any existing protocol type, or `ui/src/bridge/generated/*`. The shapes for your feature are already in (protocol 31 and later; the request lists them). Core stubs are marked `// round12 stub: <ID>`; replace the stubs of your feature. If a shape is truly missing or wrong, do NOT add it: finish everything else, and say exactly what is needed in your report (the lead adds it).
- Do not widen the scope. Build only what the spec section says. No new AI-link (MCP) tools.
- Wording only in `ui/src/text/ja.ts`, taken as written inside 「」 in the spec, source `user:2026-11-23`. Small wording the spec lacks (tool tips, status lines): make it in the style of the neighbours and list it in your notes file (below).
- No new design tokens. Use only named values from `ui/src/design/tokens.ts`.
- No new WebGL regions (`ui/tests/gl.surface.test.ts` pins the maximum at 10). No Canvas 2D for large faces.
- Every song change is one undo step; refuse while `busy()` (count-in, recording); intents point by id, an old id does nothing; a drag moves the screen first and sends one intent on release, Esc cancels.
- Audio thread: no allocation, no lock waits; values move over 20 ms; swaps overlap 10 ms; stop NaN where signals are summed; denormals to 0 in loops.
- Exhaustive matches: fix them all (`ExportEngine::tell`, `service.rs` `Audio::op`, effect kinds, `crates/meloet-ai` tables, `ui/src/bridge/fake.ts`).
- Add Rust tests and screen tests (the `fake.ts` copy and scenes). New scenes must work with `python3 -I tools/design/shoot_ui.py`.
- Step names are already in all 6 places for the steps the lead listed. If you need another step name, treat it as a missing shape (report it).
- Files several helpers touch (`ui/src/text/ja.ts`, `ui/src/bridge/fake.ts`, `crates/meloet-core/src/state.rs`, `ui/src/keys/keys.ts`, `settings.rs`, menus): read again before editing, add only your own lines, keep the edits small and local, do not reformat or reorder what is there.
- **Do not edit the shared documents** (`docs/architecture.md`, `docs/progress.md`, `docs/spec/*`, `README*.md`, `docs/user/*`, `CLAUDE.md`). Instead write ONE notes file `docs/round12/<ID>.md` (English, plain words) with these headings; a document helper merges them later:
  - `## Architecture` — how it is built and why (what will go into architecture chapter 25), the traps you met.
  - `## Spec rules` — the rules to add to `docs/spec/3-bands.md` ("Rules added in round 12") and other spec files, including keys (`4-keys.md`) and step names.
  - `## Readings` — every decision you made where the spec said nothing, every wording you made up (Japanese text + where), anything you changed from the spec and why.
  - `## User documents` — the lines for the feature lists of `README.md` (English), `README.ja.md` and `docs/user/README.ja.md` (Japanese).
  - `## Measurements` — numbers you measured, with conditions (or "none").
  - `## Left for later` — pre-existing problems you saw and did not fix, things you could not check.
- Do not commit. Do not push. Do not use sudo. Do not change PC-wide settings.
- Do not use a background wait (a loop like `until ! pgrep -f …; do sleep …`): `pgrep -f` matches its own loop and never ends. Run things in the foreground. Leave no process running when you report.
- Do not measure speed or latency yourself (other helpers are building; the PC is not quiet). Say in the notes what should be measured; the lead measures after the merge.
- Do not run `npm run dev` (port 1420 belongs to the owner). GTK apps: start with `dbus-run-session -- ` in front. Things that need a screen or sound: `source ~/.config/meloet-agent/env.sh`.

## Checks before you report

Run them yourself, in the foreground, and fix what fails:

- `cargo fmt -p <crates you touched>` (never plain `cargo fmt`: it rewrites `spikes/`).
- `CARGO_TARGET_DIR=target/<name> cargo clippy -p <crates you touched> --all-targets --features dev-hooks -- -D warnings` (leave out `--features dev-hooks` for crates that do not have the feature).
- `CARGO_TARGET_DIR=target/<name> cargo test -p <crates you touched and crates that depend on what you changed> --features dev-hooks` (same note).
- Screen: in `ui/`, `npm run check` and `npm test`.
- `python3 -I tools/design/spec_check.py` if you touched wording, and `node licenses/check.mjs` if you added a crate (then also `node tools/licenses/inventory.mjs write`).
- If you touched Python tools: `python3 -c "import ast; ast.parse(open('<file>').read())"`.
- You do not have to run the whole `scripts/check-all.sh` (the lead runs it after the merge), but everything above must pass.

## Report (short, Japanese is fine)

1. What works now, feature by feature, against the spec's "テストと受け入れ" list (done / not done, honestly).
2. Files changed (short).
3. Commands run and results (counts; the output of anything that failed).
4. Missing shapes or anything the lead must add.
5. What the owner should try by hand or by ear.

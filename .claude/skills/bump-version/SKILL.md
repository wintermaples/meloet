---
name: bump-version
description: Raise meloet's app version (for example to 0.2.0) everywhere it is written, check nothing was missed, verify, and build the deb. Use when the owner asks to change the version or to prepare a release.
---

# Raising the meloet version

The version is written in many places. Do not edit them by hand: `scripts/bump-version.sh` changes all of them and then checks that the old version is gone.

## Where the version lives (what the script changes)

- `version = "X.Y.Z"` in every `crates/*/Cargo.toml` (all product crates carry the same version). `crates/meloet-app/tauri.conf.json` has no version: Tauri takes the app crate's version, so the deb name `meloet_X.Y.Z_amd64.deb` follows it.
- `Cargo.lock`, workspace members only (`cargo update --workspace --offline`).
- `NOTICE` (line 2) and `ui/src/components/about/generated/licenses.json` (shown in 「meloet について」), rebuilt with `node tools/licenses/inventory.mjs write`.
- The deb name and the current version in `README.md`, `README.ja.md`, `docs/development/ubuntu.md`, `docs/install-check.md`, `docs/user/README.ja.md` and the versions table of `docs/progress.md`.
- The app version also reaches the song files (`"app": "meloet X.Y.Z"`) and the sound pack download's user agent through `CARGO_PKG_VERSION`; nothing to edit there.
- Left as they are on purpose: `docs/history/` (old records), the note on the first shipped version in `docs/architecture.md` 23, examples in docs, and fake data in `ui/tests/`.

## Steps

1. Do this last, after all other work of the round is merged into `foundation`, and with no helper running.
2. `source ~/.cargo/env; export PATH=$HOME/.local/node/bin:$PATH`, then `scripts/bump-version.sh X.Y.Z`. It stops with an error if any place still says the old version; fix that place (or the script, if a new place appeared) and run it again.
3. Look at `git diff --stat`: about 20 files, each with only the version lines changed.
4. If a new place that names the version was added in this round, add it to `scripts/bump-version.sh` (both the change and the check) and to the list above.
5. `scripts/check-all.sh --release` must pass.
6. Commit: `Version X.Y.Z` (small, on its own).
7. Build the deb: `MELOET_DOCKER_SG=1 scripts/build-deb.sh` (Ubuntu 24.04 container; needs the docker group, through `sg docker`). Check `target/deb/meloet_X.Y.Z_amd64.deb`: `dpkg-deb -I` (control is ASCII only), `dpkg-deb -c` (the binaries `meloet`, `meloet-plughost`, `meloet-mcp`, the MIME XML, the .desktop, the icons), `sha256sum`, and that the binaries carry no development hook names (`scripts/check-release.sh` does this for the release build).
8. Write the version, the deb size and its sha256 in `docs/progress.md`.
9. Do not tag, push or upload a Release without asking the owner.

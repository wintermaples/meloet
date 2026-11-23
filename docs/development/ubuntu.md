# Developing on Ubuntu

Ubuntu 24.04 or later (amd64). Checked on Ubuntu Studio 26.04; the deb is built in an Ubuntu 24.04 container. What is the same on every OS (checks, the sound pack for development, development hooks, documents) is in [`README.md`](README.md).

## Setup

```bash
sudo apt install build-essential pkg-config curl \
  libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev \
  libasound2-dev libpipewire-0.3-dev libspa-0.2-dev clang libclang-dev libudev-dev \
  liblilv-dev libdbus-1-dev
# Rust (rustup, 1.100) and Node.js (26) on PATH
cargo install cargo-deny          # used by the license check
(cd ui && npm ci)
```

`packaging/Dockerfile` installs the same packages and the same Rust and Node.js versions (Rust 1.100.0, Node.js 26.13.1).

## Run

```bash
(cd ui && npm run build)          # builds the screen; meloet-app takes in ui/dist when it is built
cargo run --release -p meloet-app
```

- Settings are kept in `~/.config/meloet/settings.json` (under `$XDG_CONFIG_HOME/meloet` if it is set).
- Unsaved changes are written to `~/.local/state/meloet/recovery.json` (under `$XDG_STATE_HOME/meloet` if it is set).
- The sound comes through PipeWire, and keyboards through the ALSA sequencer.

To see only the screen in a browser (no Tauri, fake state):

```bash
(cd ui && npm run dev)            # http://localhost:1420/?scene=connect2&scale=100
```

The scene names are in `ui/src/bridge/fake.ts`. Add `&theme=light` (or another theme id of `ui/src/design/themes.ts`) to see a scene in another theme.

## VS Code

`.vscode/` has the settings (the CodeLLDB extension is needed):

- "meloet (debug)" starts vite, then builds Rust for debugging and runs it. The screen comes from vite, not from `ui/dist`, so changes in `ui/` show in the window at once.
- "UI only (browser, fake scenes)" opens a scene at a scale you choose in Chrome.
- "Build the deb (docker)" runs `scripts/build-deb.sh`.
- Tasks such as "meloet: run release", "Check: all" and "Write the protocol types (Rust → TS)".

## Build the deb

```bash
MELOET_DOCKER_SG=1 scripts/build-deb.sh   # needs docker; puts the deb in target/deb/
sudo apt install ./target/deb/meloet_0.1.0_amd64.deb
```

- `scripts/build-deb.sh` builds in an Ubuntu 24.04 container (`packaging/Dockerfile`). It copies the files git knows (committed, changed, and new files that are not ignored) to `target/deb-work/src` and builds `-p meloet-app`, `-p meloet-plughost` and `-p meloet-mcp` only, with the default features.
- `MELOET_DOCKER_SG=1` runs docker through `sg docker` (for a user who is in the docker group only for the moment). Leave it out if you can run `docker` directly.
- The tauri-cli is `@tauri-apps/cli` in the devDependencies of `ui`. `meloet-plughost` and `meloet-mcp` are added as `bundle.externalBin` only while the deb is built, and go to `/usr/bin/` in the deb. They are not written in `tauri.conf.json`: there, a normal `cargo build` would overwrite `target/*/meloet-plughost` with an old copy.
- The steps to test the deb on a real PC are in [`docs/install-check.md`](../install-check.md).
- To raise the version everywhere it is written, use `scripts/bump-version.sh X.Y.Z`.

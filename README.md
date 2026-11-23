<p align="center"><img src="crates/meloet-app/icons/128x128.png" width="128" alt="meloet"></p>

# meloet

meloet is a DAW for people who have just bought a MIDI keyboard. It is written in Rust, Tauri 2 and Svelte 5 with TypeScript.

What it can do now:

- Record from a USB MIDI keyboard (with count-in, loop recording into takes, and the recorded keys placed where they were heard).
- Edit notes in the piano roll: write, move, quantize, velocity and CC lanes (sustain, pitch bend, modulation, …).
- Arrange clips on tracks; tempo and time signature changes, markers and automation.
- A mixer with six built-in effects (4-band EQ, compressor, reverb, delay, chorus, limiter).
- 31 built-in sounds. The 26 sampled ones come in a sound pack that meloet downloads from GitHub Releases on first start; the rest are made by calculation.
- VST3 and LV2 plugins, run outside meloet in plugin host processes (`meloet-plughost`; copies of the same plugin share one process), and SF2 files.
- Vocal tracks: record audio, import audio files, stretch and pitch.
- Export, an undo history band, and a recovery file for changes that were not saved.

It runs on Linux only for now: Ubuntu 24.04 and 26.04 with PipeWire.

For people who use meloet (installing, keyboards, the sound pack, plugins, removing), there is a guide in Japanese: [`docs/user/README.ja.md`](docs/user/README.ja.md).

## Install

meloet is handed out as a deb. There is no app release on GitHub yet, so for now you make the deb yourself:

```bash
MELOET_DOCKER_SG=1 scripts/build-deb.sh   # needs docker; puts the deb in target/deb/
sudo apt install ./target/deb/meloet_0.1.0_amd64.deb
```

- `scripts/build-deb.sh` builds in an Ubuntu 24.04 container (`packaging/Dockerfile`). It copies the files git knows (committed, changed, and new files that are not ignored) to `target/deb-work/src` and builds `-p meloet-app` and `-p meloet-plughost` only, with the default features.
- `MELOET_DOCKER_SG=1` runs docker through `sg docker` (for a user who is in the docker group only for the moment). Leave it out if you can run `docker` directly.
- The tauri-cli is `@tauri-apps/cli` in the devDependencies of `ui`. `meloet-plughost` is added as `bundle.externalBin` only while the deb is built, and goes to `/usr/bin/` in the deb. It is not written in `tauri.conf.json`: there, a normal `cargo build` would overwrite `target/*/meloet-plughost` with an old copy.
- After installing, meloet asks to download the sound pack (about 240 MB). How that goes is in [`docs/user/README.ja.md`](docs/user/README.ja.md).
- The steps to test the deb on a real PC are in [`docs/install-check.md`](docs/install-check.md).

## Development

### Setup (checked on Ubuntu Studio 26.04)

```bash
sudo apt install libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev \
  libasound2-dev libpipewire-0.3-dev libspa-0.2-dev clang libclang-dev libudev-dev \
  liblilv-dev libdbus-1-dev
# Rust (rustup, 1.100) and Node.js (26) on PATH
cargo install cargo-deny          # used by the license check
(cd ui && npm ci)
```

`packaging/Dockerfile` installs the same packages and versions.

### Run

```bash
(cd ui && npm run build)          # builds the screen; meloet-app takes in ui/dist when it is built
cargo run --release -p meloet-app
```

- Settings are kept in `~/.config/meloet/settings.json` (under `$XDG_CONFIG_HOME/meloet` if it is set).
- Unsaved changes are written to `~/.local/state/meloet/recovery.json` (under `$XDG_STATE_HOME/meloet` if it is set).
- The keys are in [`docs/spec/4-keys.md`](docs/spec/4-keys.md).

To see only the screen in a browser (no Tauri, fake state):

```bash
(cd ui && npm run dev)            # http://localhost:1420/?scene=connect2&scale=100
```

The scene names are in `ui/src/bridge/fake.ts`.

In VS Code (`.vscode/`, needs the CodeLLDB extension):

- "meloet (debug)" starts vite, then builds Rust for debugging and runs it. The screen comes from vite, not from `ui/dist`, so changes in `ui/` show in the window at once.
- "UI only (browser, fake scenes)" opens a scene at a scale you choose in Chrome.
- Tasks such as "meloet: run release", "Check: all" and "Write the protocol types (Rust → TS)".

### The sound pack for development

Builds of meloet look for the sound pack in `sounds/` beside the executable first. To build the pack:

1. Download the sources listed in `tools/sounds/sources.json` (each entry says how to get the exact same bytes) into `~/.cache/meloet/sounds-src`.
2. Build the pack and link it beside the debug build (do the same for `target/release`):

```bash
python3 -I tools/sounds/build.py --out target/sounds/dev
ln -sfn ../sounds/dev target/debug/sounds
```

The sounds found there are not downloaded. `build.py` uses ffmpeg, ffprobe and numpy.

### Checks

```bash
scripts/check-all.sh             # formatting, clippy, Rust tests, type checks, UI tests, licenses
scripts/check-all.sh --devices   # also the tests that need a keyboard, PipeWire and an output
scripts/check-all.sh --release   # also scripts/check-release.sh: no development hooks in the shipped build (slow)
```

### Development hooks and measuring

The `MELOET_*` variables below exist **only in development builds** (`cargo build --release -p meloet-app --features dev-hooks`, and the screen built with `MELOET_DEV_HOOKS=1 npm run build`). `tools/measure/build-dev.sh` builds both (`--keep-ui` builds only the programs). In the shipped meloet (no features) they do nothing, and their names are not in the executable ([`docs/architecture.md`](docs/architecture.md) 23-11). The plugin host's `MELOET_PLUGHOST_FAKE`, `MELOET_PLUGHOST_RT` and `MELOET_PLUGHOST_NO_DUMP` are the same (`-p meloet-plughost --features dev-hooks`).

| Variable (development builds only) | What it does |
|---|---|
| `MELOET_WINDOW=1280x800` | The inner size of the window |
| `MELOET_SNAPSHOT=out.png`, `MELOET_SNAPSHOT_AFTER_MS=1500` | Writes the screen to a PNG and ends |
| `MELOET_COMMANDS='[{"afterMs":800,"command":{"type":"chooserConfirm"}}]'` | Sends intents at set times |
| `MELOET_EVAL='[{"afterMs":900,"js":"…"}]'` | Runs JavaScript in the page at set times |
| `MELOET_PIPE=fifo` | Keeps reading lines from that named pipe: `{"command":{…}}` is an intent, `{"js":"…"}` is JavaScript in the page (`tools/measure/remote.py`) |
| `MELOET_EXIT_AFTER_MS=10000` | Ends after that time |
| `MELOET_CLOSE_AFTER_MS=10000` | Closes the window after that time, the same way as the close button |
| `MELOET_CONFIG_DIR=dir` | Where the settings file is |
| `MELOET_STATE_DIR=dir` | Where the recovery file is |
| `MELOET_SONGS_DIR=dir` | The "Music" folder where songs go |
| `MELOET_SF2_DIR=dir` | Where copied SF2 files go |
| `MELOET_SOUNDS_DIR=dir` | The sound pack folder (with `pack.json`). When set, no other place is searched and nothing is downloaded |
| `MELOET_PACK_ORIGIN=http://127.0.0.1:8000/` | For tests: where sounds and the online catalog (`<origin>catalog.json`) are fetched from (`http://` is allowed) |
| `MELOET_PACK_CATALOG=file` | For tests: reads that file instead of the built-in catalog (`tools/sounds/catalog.json`) |
| `MELOET_OPEN_SONG=file.meloet` | Opens that song at start |
| `MELOET_MEASURE_DIR=dir` | Writes the key → sound and key → screen records |
| `MELOET_UI_SCRIPT=pr-scroll\|pr-zoom\|pr-drag:N` | Only with `MELOET_MEASURE_DIR`: repeats scrolling, zooming or dragging in the page and measures |
| `MELOET_ACCEPT_VIRTUAL=1` | Also takes the test keyboard (`meloet-vkeys`) as a keyboard |
| `MELOET_ONLY_VIRTUAL=1` | Ignores real keyboards and sees only virtual ones |
| `MELOET_VKEYS_NAME=name[,name…]` | Takes only the virtual keyboards with those names (`meloet-vkeys --name`) |
| `MELOET_MIDI_FAIL_AFTER=N` | For tests: stops the MIDI thread after N keys |
| `MELOET_INPUT_SINK_MONITOR=sink` | For measuring: uses the monitor of that output as the input |
| `MELOET_PCWINDOW_SHOT=out.png` | 1.5 s after the PC file dialog opens, draws it to a PNG |
| `MELOET_DEBUG_SCREEN=1` | Prints the work area, window and inner size to stderr each time they are measured |

`JSC_useGenerationalGC` is not a hook: meloet sets it to 0 at start in every build (so the web process memory does not keep growing). A value set from outside wins.

The full list of tools (latency, long recording, screenshots, mocks, plugins, export, …) is in 「よく使うコマンド」 in [`CLAUDE.md`](CLAUDE.md).

### Documents

- [`docs/progress.md`](docs/progress.md): where the work is now, and what is left to do.
- [`docs/architecture.md`](docs/architecture.md): the structure of meloet, its rules and the reasons.
- [`docs/spec/`](docs/spec/): the design spec (the source of truth for the screen). [`docs/spec/glossary.md`](docs/spec/glossary.md) gives the English words.
- [`docs/design/mock/`](docs/design/mock/): the screen mocks. When a mock and the spec do not agree, the spec is right.
- [`docs/history/`](docs/history/): older records, kept in Japanese as they were (progress by round, the Japanese architecture, design notes, measurements, research, and the report on the `spikes/`).
- `spikes/`: early tests of the base ideas, made before the real code. They are kept, but not copied into the real code.

## License

The code is under either of these, at your choice:

- MIT ([`LICENSE-MIT`](LICENSE-MIT))
- Apache License 2.0 ([`LICENSE-APACHE`](LICENSE-APACHE))

The font IBM Plex is under OFL-1.1 (the woff2 files of @fontsource are used without change). The credits for the sound pack and the parts are in [`NOTICE`](NOTICE). `node licenses/check.mjs` checks the licenses of the parts and the sounds.

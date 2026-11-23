# Developing meloet

This folder is for people who build and change meloet. For people who use it, see [`README.md`](../../README.md) (English) or [`README.ja.md`](../../README.ja.md) (Japanese).

## Supported development OS

| OS | State | Guide |
|---|---|---|
| Ubuntu 24.04 or later (amd64) | Supported | [`ubuntu.md`](ubuntu.md) |
| Windows | Not yet | — |
| Mac | Not yet | — |

The guide for each OS covers the setup, running meloet, the browser-only screen, VS Code and building the deb. The rest of this page is the same on every OS.

## Checks

```bash
scripts/check-all.sh             # formatting, clippy, Rust tests, type checks, UI tests, licenses
scripts/check-all.sh --devices   # also the tests that need a keyboard, PipeWire and an output
scripts/check-all.sh --release   # also scripts/check-release.sh: no development hooks in the shipped build (slow)
```

## Licenses of the parts

The code is under MIT or Apache-2.0. Do not bring in GPL or AGPL parts. After adding a dependency, run `node licenses/check.mjs`; it checks the licenses of the parts and of the sound material (only CC0, CC BY or MIT).

## The sound pack for development

Builds of meloet look for the sound pack in `sounds/` beside the executable first. To build the pack:

1. Download the sources listed in `tools/sounds/sources.json` (each entry says how to get the exact same bytes) into `~/.cache/meloet/sounds-src`.
2. Build the pack and link it beside the debug build (do the same for `target/release`):

```bash
python3 -I tools/sounds/build.py --out target/sounds/dev
ln -sfn ../sounds/dev target/debug/sounds
```

The sounds found there are not downloaded. `build.py` uses ffmpeg, ffprobe and numpy.

## Development hooks and measuring

The `MELOET_*` variables below exist **only in development builds** (`cargo build --release -p meloet-app --features dev-hooks`, and the screen built with `MELOET_DEV_HOOKS=1 npm run build`). `tools/measure/build-dev.sh` builds both (`--keep-ui` builds only the programs). In the shipped meloet (no features) they do nothing, and their names are not in the executable ([`docs/architecture.md`](../architecture.md) 23-11). The plugin host's `MELOET_PLUGHOST_FAKE`, `MELOET_PLUGHOST_RT` and `MELOET_PLUGHOST_NO_DUMP` are the same (`-p meloet-plughost --features dev-hooks`).

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
| `MELOET_UPDATE_URL=http://127.0.0.1:8000/latest` | For tests: where 「新しい版をたしかめる」 reads the newest release (a JSON with `tag_name`; `http://` is allowed). A build with the hooks never looks for a new version without it, and remembers nothing of what it read |
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

The full list of tools (latency, long recording, screenshots, mocks, plugins, export, …) is in "Common commands" in [`CLAUDE.md`](../../CLAUDE.md).

## Documents

- [`docs/progress.md`](../progress.md): where the work is now, and what is left to do.
- [`docs/architecture.md`](../architecture.md): the structure of meloet, its rules and the reasons.
- [`docs/spec/`](../spec/): the design spec (the source of truth for the screen). [`docs/spec/glossary.md`](../spec/glossary.md) gives the English words; [`docs/spec/4-keys.md`](../spec/4-keys.md) lists the keys.
- [`docs/design/mock/`](../design/mock/): the screen mocks. When a mock and the spec do not agree, the spec is right.
- [`docs/install-check.md`](../install-check.md): the steps to test the deb on a real PC.
- [`apt-repo.md`](apt-repo.md): the apt repository (the signing key, `scripts/make-apt-repo.sh`, GitHub Pages, and what a user writes).
- [`ai-tools.md`](ai-tools.md): the MCP tools of the AI link, with the arguments and answers of those added or changed in round 11.
- [`docs/user/README.ja.md`](../user/README.ja.md): the detailed guide for users (Japanese).
- [`docs/history/`](../history/): older records, kept in Japanese as they were (progress by round, the Japanese architecture, design notes, measurements, research, and the report on the `spikes/`).
- `spikes/`: early tests of the base ideas, made before the real code. They are kept, but not copied into the real code.

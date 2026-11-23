# meloet — to you, who come next

meloet is a DAW for people who have just bought a MIDI keyboard. Requests come from the owner (the person who decides the design and the direction), each time in a new conversation. Yachiyo sometimes writes the request text, but the person who decides is the owner (「ヤチヨさん」 in older docs means the owner).
Read this document first. Then read [`docs/progress.md`](docs/progress.md) (where things are, open work, notes for the next round), [`docs/architecture.md`](docs/architecture.md) (the structure, the promises and the reasons), and [`docs/spec/README.md`](docs/spec/README.md) (the design spec, the source of truth). The old records (rounds 1 to 9, in Japanese) are in [`docs/history/`](docs/history/); read them only when you need the past.

Reply in Japanese, in plain words.

You are not the first one here, and you will not be the last. Leave things so the next one can pick them up.

## How to work

- You are the lead. You design, split the work, ask helpers, receive and check their work, and put it together. **Do not write large pieces of code yourself.**
- The helpers are the five in `.claude/agents/` (four for building, one for research; the owner, 2026-11-23). Helper files and the commands helpers use to consult another model pin the full model name (`model: claude-opus-5-5`, `claude -p --model claude-fable-5-2` …); in the prose of documents the short names (`opus`, `sonnet`, `haiku`, `fable`) are fine (the owner, 2026-11-23).
  - `meloet-light` (haiku, effort medium): light, mechanical work with exact instructions: running checks and returning only what failed, gathering facts from the code, small edits whose form is fully given, screenshots and counts. Never designs or decides. When stuck it asks sonnet, then opus, then returns to the lead.
  - `meloet-impl` (opus, effort medium): screen parts, band frames, settings, wording, tests, tools and scripts, documents and translations, plain Rust with no tricky logic. When it is stuck, it may ask Fable how to implement or fix.
  - `meloet-hard` (opus, effort xhigh): the audio thread, unsafe and FFI, communication between threads and processes, WebGL, the core of the promise between Rust and the screen, the data shapes of core, measuring speed and memory. As soon as it feels even a little stuck, it asks Fable how to implement or fix.
  - `meloet-very-hard` (fable, effort medium): work whose logic is complex, not easy Rust code (timing and position rules, recording and take rules, undo and merge rules over several clips, real-time engine changes with subtle invariants, signal processing algorithms, concurrency protocols, tricky state machines).
  - `meloet-research` (sonnet, effort high): searches the internet. Writes no code.
  - Helpers ask Fable through the Claude Code command line, read-only, always with effort medium: `claude -p --model claude-fable-5-2 --effort medium --tools "Read,Grep,Glob" --allowedTools "Read,Grep,Glob" "<question>"` from the root of their worktree (the owner, 2026-11-23). The helper files say how.
  - Choosing: if you cannot tell whether a job is normal or hard, choose hard; if a hard job also holds complex logic, choose very-hard.
  - When Fable hits its usage limit, ask the owner; then call `meloet-very-hard` by name with `model: opus` (and tell the hard and impl helpers that Fable cannot be asked). Past limits: 2026-11-15 to 18, 2026-11-20, 2026-11-22 to 24 (Fable came back on 11-24 at 21:00).
  - If you create or change a helper file in the middle of a conversation, you cannot call it by name in that conversation. Then call the general helper with the same model and effort, and have it read the helper file first.
- Helpers do not know what was said before. When you ask, write everything: which files, under which rules, and how to check. When several work at the same time, give each a separate directory, and use `CARGO_TARGET_DIR=target/<name>` and `cargo -p` so builds do not clash. One `target/<name>` grows to 4 to 9 GB. After you merge a helper's work, delete its target (on 2026-11-18 the disk became full and 166 GB were deleted; keep `debug/` and `release/`). Check after a merge in the real working tree's `target/` (no `CARGO_TARGET_DIR`). Do not reuse a helper's `target/<name>` (even with a different path, the same crate is taken as the same thing, and things built on the helper's old base are used as if new; on 2026-11-22 the engine was tested in its old form).
- When several work at the same time, put each helper in its own worktree (the Agent tool's `isolation: "worktree"`). Worktrees are made **from main**, so write `git reset -q --hard <foundation now>` at the start of the request. Have the helper make a symlink for `ui/node_modules`, and do not let helpers commit. When they finish, the lead checks in the helper's worktree, commits, and merges into foundation.
- After a merge, look over the whole thing, not only the places that clashed. git sometimes puts an arm added at the end of an enum into a different enum next to it (it happened twice in round 3). `scripts/check-all.sh` does not read the Python tools (`tools/`), so if you touched them, also run `python3 -c "import ast; ast.parse(open('<file>').read())"`.
- When two screen helpers build at the same time, the lead first puts the seam in place, small, and commits it, before asking (the seam: a state module both read, or a part where one makes the container and the other makes the content; in round 3, `ui/src/components/pianoroll/view.svelte.ts`, `shown.svelte.ts`, an empty `NoteListPanel.svelte`, and so on).
- Check what you get from helpers yourself (build, test, run). Do not take "done" as it is.
- In requests to helpers, write: "do not use a background wait (a loop like `until ! pgrep -f …; do sleep …`)". `pgrep -f "<text>"` also matches the line of the `bash -c` that runs the loop itself, so it never ends. The helper stops there, waiting (2026-11-18, it did not come back for about an hour; it also happened in an earlier round). When a helper does not come back, or after it ends, look with `ps -u meloet-agent -o pid,ppid,etime,args | grep -E "sleep|until"` and stop any loops left (do not stop the parent `claude`).
- When several work at the same time, half-written changes get mixed in the same files. Write in each request which files that helper may touch ("read it again first, and add to it", too). When they get mixed, make a scratch worktree with `git worktree add --detach <dir> HEAD`, copy in only that helper's files, check there, commit there, then `git update-ref refs/heads/foundation <commit>` and `git reset -q` (do not touch the other helpers' half-written files). A symlink for `ui/node_modules` is enough.
- Ask for large work that changes the promise in two passes: first, have one helper make only the shape of the promise, the TS types and the fake scenes, and commit; second, ask core and the screen of two different helpers at the same time.
- Tests that open plugin windows put windows on the owner's screen, but you may go ahead without asking (the owner, 2026-11-19).
- When you measure speed, make the PC quiet (stop other helpers' builds and apps; check with `ps -u wintermaples` that the owner's meloet is not running). Write down numbers taken in unclean conditions together with the conditions. Measurements play sound in the headphones, but you may go ahead without asking (the owner, 2026-11-18). The memory trend (RSS) can be read even if a little unclean.
- To measure things that move only on the screen (scrolling, zoom, drag) in the real app, use `MELOET_UI_SCRIPT` (an automatic helper inside the page that runs only when measuring) and `MELOET_OPEN_SONG` (start with a song open) (`tools/measure/ui-script.sh`).
- When a helper says it "was stopped", ask the owner before starting a new helper.
- When a helper stops at the usage limit, call it back after the limit resets with `SendMessage` (the helper's id): "continue from where you stopped". Its worktree and conversation stay as they were. **While you may call it back, do not clean up its worktree** (if you do, you cannot call it back; it happened once on 2026-11-20). If the owner asks, make a one-time `CronCreate` booking for the time the limit resets (it works only during this conversation).
- Helpers' scratch files can end up in a place shared with other helpers. In the request, have each helper put its own mark at the start of file names (`k3core2-` and so on; in round 5 they wrote over each other's files).
- Research goes to meloet-research. Primary sources first. Have it return the source and the confidence (A, B, C).
- Do not write from memory. For fast-moving parts such as Tauri, Svelte, cpal, pipewire and alsa, read the docs or the source of the installed version first (`~/.cargo/registry/src/`, `ui/node_modules/`).

## When to stop and ask, and what you may decide

- Ask: looks or wording that are in neither the spec nor the mocks. Places where you cannot go on without breaking a rule. sudo. When you want to make the scope wider. Write questions for the owner in `docs/spec/open-questions.md`.
- You may decide: inner structure, names, how to write tests, how to split work among helpers. When you decide, leave one line with the reason (in chapter 10 of `docs/architecture.md` or in `docs/progress.md`). Decisions about how to read the spec, or made where the spec and the mocks say nothing, go in `docs/spec/7-readings.md`.

## Rules

- Rust, Tauri 2, Svelte 5 and TypeScript.
- Code is under both MIT and Apache-2.0. Do not bring in GPL or AGPL parts. Keep watching with `node licenses/check.mjs`. Do not change files of MPL-2.0 parts. The font (IBM Plex) stays OFL-1.1; use the @fontsource woff2 files as they are, without changes.
- The screen runs at 60 Hz. Large faces (piano roll, arrange clips, meters) use WebGL. Do not use Canvas 2D for large faces. Text and small parts are plain HTML.
- While dragging, only the screen moves first, and on release one intent is sent (one drag is one step). The original position is shown with a gray.7 border. Esc is first in the key order and stops the drag (`cancelDrag` in `ui/src/components/pianoroll/view.svelte.ts`). Only the mixer changes the sound during a drag: it sends a "change only the sound" intent (`commit: false`, at most one per frame), "decides" on release (`commit: true`, one step), and Esc sends `CancelMixPreview` (`ui/src/components/mixer/mixDrag.ts`).
- Only `ui/src/bridge/` (and `crates/meloet-app` on the Rust side) knows Tauri.
- The promise between Rust and the screen is 7-3 of the report (`docs/history/report.md`): greeting, get ready to listen and then fetch the whole state, rev, the screen sends only intents, Channel, telemetry in binary, urgent news at once. Details are in chapter 3 of `docs/architecture.md`.
- The crate split is 7-1 of the report with `meloet-core` added (chapter 1 of `docs/architecture.md`).
- Keyboards are plugged and unplugged through the ALSA sequencer. Output and input names come from PipeWire Routes. Sound goes through cpal.
- Show the buffer that is really running. PC-wide settings (such as PipeWire's quantum) are not changed, neither by meloet nor by you. Only read them.
  - But only for measurements and tests, you may change `clock.force-quantum` for a short time (the owner, 2026-11-21). Before changing, read the old value with `pw-metadata -n settings 0` (now 256), put it back when you are done, and tell the owner that you changed it and put it back. meloet still never changes it.
- On the audio thread, do not allocate memory and do not wait on locks. Tests watch this by machine (`crates/meloet-engine`, `crates/meloet-sound`). Values move smoothly over 20 ms; when swapping effects or instruments, overlap 10 ms. Stop NaN where signals are summed, and set denormal small values to 0 in loops (do not set FTZ or DAZ).
- Write the built-in effects ourselves (`crates/meloet-sound/src/effects/`). Sound material is only CC0, CC BY or MIT (not CC BY-SA). Credits are in `NOTICE` and in 「meloet について」; `node licenses/check.mjs` watches the material too. How the material is made lives in the repository (`tools/sounds/`); the built packs go on GitHub Releases (ask the owner before uploading).
- Plugins run in a separate process (`meloet-plughost`; the same plugin goes in one process). Do not use stdout or stderr for communication. Watch for a stop by time (chapter 20 of `docs/architecture.md`). Start child processes one at a time with `meloet_plugin::launch::spawn` (because of LLDB in VS Code). Formats are VST3 and LV2 (CLAP is not in).
- Use numbers by their names in the spec (`color.gray.4`, `size.meter.slim`, `space.3` …), as they are, from `ui/src/design/tokens.ts`. Do not add colors, text sizes or spaces on your own.
- Screen wording lives in one place, `ui/src/text/ja.ts`. Do not change a single character of the spec's wording (`docs/spec/`). Text found only in a mock is used as written in the mock (with its source). Text the owner decided has the source `user:<date>`.
- When the mocks and the spec disagree, the spec is right. Make a list of what you fixed.
- Parts with no content yet are not removed; show them in the "disabled" state.
- Positions in a song are in ticks (a quarter note = 1920). Tempo is always quarter notes per minute (whatever the time signature). A recorded key is put at "the position that was reaching the ear at the moment it was pressed". Quantize is only an edit (chapters 11 and 12 of `docs/architecture.md`).
- Every operation that changes the song is one "undoable step" (`meloet_model::History`). A take, once made, never changes its content. Editing notes puts a new take with a new id in the same place, and the number of takes does not grow. Intents point at a take by its id and an index inside it; an intent with an old id does nothing. The selection is held by core and comes back with undo (chapter 16 of `docs/architecture.md`).
- The screen keeps only the takes in use and the 16 newest (`keepOnly` in `ui/src/bridge/take.ts`). What undo can go back to is held by Rust's undo history.
- The song file is versioned JSON (now version 6. Versions 5 and older cannot be read. During round 7 the shape grows while staying at version 6. Plugin state is base64. Fields for plugins or SF2 that are not found are written back as they were read). Keep the safe way of writing and `.prev` (chapter 14). When you change the shape, decide how the version is handled.
- Unsaved changes are written to the recovery file (when the PC shuts down, and 2 seconds after a change), with the same safe way of writing (chapter 15).
- The WebGL context budget is 12 (WebKitGTK allows up to 16, and dropped canvases count until GC). One per region, reused (chapter 7). Contexts are made when a region's box arrives (at start, when a band opens) (`getContext` stops for 50 to 200 ms each time, so do not make one while a person is playing). Things that change at 60 Hz go in small regions; no layout every frame. There are at most 10 regions now (8 in the real app).
- meloet sets `WEBKIT_DISABLE_COMPOSITING_MODE=1` at start and draws the page on the CPU (from WebKitGTK `2.52.6-0ubuntu0.26.04.2`, without it the page goes the compositing-layer way and key → screen is 8 to 10 ms longer; see the note in chapter 7 of architecture). On the CPU way, when one frame has 11 or more dirty rectangles, or 25 % or more of the bounding rectangle, the whole bounding rectangle is drawn again ("WebKit drawing on the CPU" in chapter 7 of `docs/architecture.md`). When you add something that changes at 60 Hz: in the top band, put it under the `masterMeter` canvas with `use:glSlot={{ kind: "cover" }}`; elsewhere, put it under the same canvas as things that change in the same frame, or line up its top and bottom edges with them. After adding, check with `tools/measure/webkit-damage.sh`.
- Comments only where the intent is hard to see. Do not write why something changed or its history. Commits small and often.
- Documents still in use, and code comments, are written in English (the owner, 2026-11-23). Use the words of `docs/spec/glossary.md`. The old records in `docs/history/` stay in Japanese and are not changed any more.
- Do not delete `spikes/`. Do not copy it into the product as it is. Bring over only what was learned.
- When sudo is needed, show the command and ask. If you run as another user and someone opened a way in for you (ACL, xhost), close it at the end and say that you closed it.

## Structure (at a glance)

```
crates/
  meloet-model     song, track, clip, take, note (pure data), the tempo map (tempo_map.rs), undoable steps (History), the rules that turn recorded events into takes (Recorder), editing notes (notes.rs), quantize (quantize.rs), CC points (controls.rs), moving clips (arrange.rs), sounds and groups (instrument.rs), drum kits (drums.rs), mixer and effect parameters (mixer.rs, effects.rs)
  meloet-protocol  the promise between Rust and the screen (version 30). TS types are written out to ui/src/bridge/generated/. Plugin windows and parameter shapes in params.rs, things added later in added.rs, tempo, time signature and markers in timeline.rs
  meloet-engine    audio thread priority (priority.rs), the sound flow, the instrument interface (Instrument), track seats, the mixer signal path (mixer.rs), meters, the cpal output, transport (transport.rs), the position reaching the ear (clock.rs)
  meloet-sound     sound parts: the sample player (sampler/), SF2 reader (sf2/), 6 effects (effects/), sounds made by calculation (synth/), the phrases of a sample's ▶ (phrases.rs)
  meloet-plugin    plugin keys and the keys that group them, kinds, the scan record, shared memory and futex, communication with the plugin host (wire.rs), starting children (launch.rs)
  meloet-plughost  the plugin host process (host and scan). VST3 and LV2 (formats/), windows (window.rs, X11), the audio thread (rt.rs), the main thread (control.rs)
  meloet-stretch   stretch and pitch (the Signalsmith Stretch C++ headers copied into vendor/ without changes, a C interface and a safe Stretcher, Follower that follows step by step, and render). C++ and unsafe only here
  meloet-midi      ALSA sequencer. Plugging and unplugging keyboards. meloet-vkeys (a virtual keyboard for tests. --name)
  meloet-devices   reads PipeWire Routes (human names) and settings (the fixed quantum)
  meloet-core      the owner of the state. Plugin scanning and plugin host processes (plugins/: scanner.rs, hosts.rs, housed.rs), the shelf's "added later" (catalog.rs), SF2 copies (sf2store.rs), loading plugins (play/plugs.rs), windows and 15-C (play/windows.rs), plugin parameters (play/params.rs), command → state → diff with rev. Band rules (layout.rs), intents for play, record, undo and the piano roll (play.rs), the song file (songfile.rs), the save flow (state/songs.rs), export (export.rs, export/sink.rs, plugins/export.rs), the recovery file (recovery.rs, state/recovery.rs), connecting again (service.rs), sound packs and loading them (sounds.rs, instruments.rs), the sound pack fetcher (sounds/fetch.rs, catalog.rs, untar.rs, state/pack.rs), engine seats (play/seats.rs), the shelf (play/shelf.rs), mixer intents and sending them to the engine (play/mixer.rs, play/mixsync.rs, mixbook.rs)
  meloet-app       the Tauri shell. The web process watcher (webprocess.rs), the PC file window (pcwindow.rs), the logind delay lock (logind.rs), hooks only for development (devhooks.rs)
ui/
  src/bridge/      the only place that knows Tauri
  src/design/      numbers (by their names in the spec)
  src/text/        wording
  src/gl/          the WebGL helper (regions, reuse, budget)
  src/time/        the screen's tempo map (tempoMap.ts. Tested with the table of Rust's answers, tempoMap.table.json)
  src/components/pianoroll/  the piano roll band (the range in view view.svelte.ts, the take shown shown.svelte.ts, drag drag.ts, CC lanes cc.ts)
  src/components/mixer/      the mixer band (strips, faders, pan, sends, inserts, drag mixDrag.ts)
  src/components/params/     the params panel on the right (built-in effects BuiltinList, plugins PluginList. The table is bridge/paramTable.ts)
  src/components/menu/       the menu bar and context menus (menus.ts, contextMenu.svelte.ts)
  src/dev/         an automatic helper inside the page that runs only when measuring (MELOET_UI_SCRIPT)
docs/
  spec/            the design spec, the source of truth (README.md, chapters 1-6, 7-readings.md, open-questions.md, glossary.md; checked by tools/design/spec_check.py)
  design/mock/     copies of the mocks
  architecture.md  the structure and the reasons
  progress.md      where things are, open work, notes for the next round
  install-check.md steps to install and try on a real PC
  development/     for developers: setup, run, checks, the dev sound pack, development hooks, building the deb (README.md for every OS, ubuntu.md)
  screenshots/     pictures of the real app, side by side with the mocks
  user/            documents for users (README.ja.md)
  measurements/    measured numbers from round 10 on (English; make it when the first one is written)
  research/        research documents with sources and confidence, from round 10 on (English; the same)
  history/         old records, in Japanese, not changed any more: progress.ja.md, design-notes.ja.md (how the spec and mocks were read), architecture.ja.md, report.md (the first study), notes-interim.md, spec-v12.html, spec-v12-changes.ja.md, measurements/ (measured numbers), research/ (research documents with sources and confidence)
tools/sounds/      makes the sound packs (URLs and sha256 in sources.json, build.py, the SF2 reader sf2.py, split.py that splits by sound, the built-in catalog catalog.json)
packaging/         the container that builds the deb we ship (Dockerfile. Used by scripts/build-deb.sh)
licenses/          the license watcher (check.mjs) and copies of the material licenses
spikes/            test prototypes (do not delete, do not copy)
```

## Common commands

```bash
source ~/.cargo/env; export PATH=$HOME/.local/node/bin:$PATH
scripts/check-all.sh             # fmt, clippy, Rust tests, type check, screen tests, license watcher
scripts/check-all.sh --devices   # plus tests that need a keyboard or PipeWire
scripts/check-all.sh --release   # plus: no development hooks left in the shipped form (scripts/check-release.sh. Heavy)
tools/measure/build-dev.sh [--keep-ui]  # before measuring: build release with the development hooks (feature dev-hooks + MELOET_DEV_HOOKS=1). With only `cargo build --release -p meloet-app`, MELOET_* does not work
cargo test --features dev-hooks  # Rust tests (check-all uses this form. They pass without the feature too, but the devhooks tests do not run)
MELOET_DOCKER_SG=1 scripts/build-deb.sh  # build the deb we ship in an Ubuntu 24.04 container (target/deb/. Needs the docker group)
scripts/bump-version.sh X.Y.Z    # raise the app version everywhere and check nothing still says the old one (the skill .claude/skills/bump-version has the whole release steps)
python3 -I tools/design/render_mocks.py target/mock-png   # mocks to PNG
python3 -I tools/design/shoot_ui.py <output> [--jobs=N] [--theme=a,b|all] [--build] <scene>  # fake scenes side by side with mocks (N at a time, default 8; builds ui/ only when it changed)
python3 -I tools/design/shoot_app.py <state>                # take pictures of the real app (docs/screenshots/)
tools/measure/latency.sh --state stopped|playing|recording  # key → screen, key → sound
tools/measure/latency.sh --pattern mixed --piano-roll      # chords and fast keys, with the piano roll open
tools/measure/record-timing.sh private                      # error of recorded positions (our own PipeWire)
tools/measure/latency.sh --mixer --song <song> --state playing  # with the mixer open (show the virtual keyboard before start)
tools/measure/long-record.sh [--piano-roll] [--mixer --song <song>]  # record for 10 minutes
tools/measure/ui-script.sh                                  # scroll, zoom and drag automatically in the real app (frame times and memory)
tools/measure/app-watch.sh                                  # watch frames (busyMs) and memory of the real app
tools/measure/webkit-damage.sh                              # rectangles WebKit drew again (under gdb)
python3 -I tools/measure/remote.py ...                      # drive the real app from outside one step at a time (MELOET_PIPE, fake X events, record the output)
tools/measure/plugin-stop.sh / open-play-save.sh            # tests that stop plugins, tests that save and open
python3 -I tools/measure/window-rounds.py ...               # plugin windows dozens of times, 15-C hundreds of times
python3 -I tools/measure/make_song.py ... [--tracks N] [--instruments plugin,sf2,builtin --plugin <name> --sf2 <file> --effect-plugin <name>]  # songs for tests
python3 -I tools/measure/audio_app.py play|load|same|record-save|position|arrange ...  # audio clips (play 16, loading speed, same sound after save, recorded position, arrange frames)
python3 -I tools/measure/stretch_app.py position|prepare|load|kept|splice ...          # stretch (position, load while making, no remaking, swapping)
python3 -I tools/measure/export_app.py same|speed|flow ...                              # export (same as the output's sound, speed, the check flow)
python3 -I tools/measure/arrange_parts.py ...                                           # arrange frames part by part (--how task draws every frame)
python3 -I tools/sounds/build.py --out target/sounds/dev    # build the sound pack (material in ~/.cache/meloet/sounds-src, sources in sources.json)
ln -sfn ../sounds/dev target/debug/sounds                   # the development build finds the pack (release too)
python3 -I tools/sounds/split.py target/sounds/dev --out <dir> --pack <version> [--previous <previous catalog>] [--catalog tools/sounds/catalog.json]  # the shipped form: a tar per sound and the catalog (architecture 23-1, 23-9)
```

How to set up and run it: [`docs/development/`](docs/development/README.md) (per OS: [`ubuntu.md`](docs/development/ubuntu.md)). The READMEs for users: [`README.md`](README.md) (English) and [`README.ja.md`](README.ja.md) (Japanese); keep the two in step.

## Things about this PC (not about real users)

- The agent runs as a separate user, `meloet-agent`. Screen and sound come in with `source ~/.config/meloet-agent/env.sh` (the owner opens the way in).
- GTK apps sometimes stop for 30 seconds at start. Start them with `dbus-run-session -- ` in front.
- When cpal does not find PipeWire: `PIPEWIRE_RUNTIME_DIR=/run/user/1000`.
- The owner has the same working tree open in VS Code, and vite may be running on port 1420. Do not run `npm run dev` on 1420. Do not delete `ui/node_modules/.vite/`. The owner may also be running a debug build of meloet.
- Tests that bring PipeWire down use our own PipeWire and WirePlumber (`crates/meloet-engine/tests/private_pw/`). Do not stop the owner's PipeWire. Socket paths are limited to 108 bytes, so put them under `/run/user/1001/`.
- The WebKitWebProcess SIGSEGV on close (NVIDIA's EGL) was avoided by stopping the web process first. apport does not write a new report for the same program while there is a `.crash` not yet looked at, so "it did not crash" cannot be checked with `/var/crash` (`docs/history/progress.ja.md`, round 2, step 0).
- "Hundreds of GC threads made and gone" is the GTK title bar icon being read again (glycin). No effect (same record as above).
- In the shell, writing `a && b && ( … ) & c` makes `&` send the whole list before it into the background. Put the line that sets variables and the line that runs in the background on separate lines.
- `cargo fmt` with no options also rewrites `spikes/`. List crates with `-p` (the same as `scripts/check-all.sh`). `cargo clippy --workspace` also fails on spikes.
- Helper worktrees are made in `.claude/worktrees/` (left out by `.git/info/exclude`). The clippy of `meloet-app` needs `ui/dist`, so in a worktree make an empty `ui/dist` for a short time.
- When the PC shuts down, the app's scope gets SIGKILL 5 seconds after SIGTERM (Ubuntu 26.04, GNOME 50 is Wayland only). Before power off, logind's PrepareForShutdown comes (if you hold a delay lock).
- The JavaScriptCore of WebKitGTK 2.52 runs only Eden GC when idle, and Full GC rarely comes. On a page that moves at 60 Hz, the WebKitWebProcess memory keeps growing. meloet sets `JSC_useGenerationalGC=0` at the very start of startup and does Full every time (`crates/meloet-app/src/main.rs`, g in `docs/history/measurements/2026-11-15-round3-fix-memory.md`). If the same variable is set from outside, that one wins. GC can be watched with `JSC_logGC=1` (writing the log is heavy, so stops look a little longer).
- Before WebKitGTK `2.52.6-0ubuntu0.26.04.2`, this PC's WebKitGTK drew pages on the CPU with no compositing layers (variables such as `WEBKIT_FORCE_COMPOSITING_MODE=1` did not change it). From `.2` (2026-11-21) it takes the compositing-layer way unless `WEBKIT_DISABLE_COMPOSITING_MODE=1` is set, and meloet sets it (see the rules). WebKit debug info is in `~/.cache/debuginfod_client/` (`webkit-damage.sh` uses it).
- The output chooser appears only when "no output has been decided yet + at least one keyboard". Measuring tools show a virtual keyboard (`meloet-vkeys appear`) before start.
- Even in debug builds, symphonia and `meloet-sound` are opt-level 3 (`Cargo.toml`). Without it, loading the grand piano takes 8 seconds.
- `env.sh` has no `DISPLAY`. For things that need X (plugin window tests, the real `--ignored` tests), add `DISPLAY=:0` (XWayland; windows appear on the owner's screen).
- This PC has plugins such as Surge XT (VST3 and LV2), LSP, ZamComp, Dragonfly and Calf (`/usr/lib/vst3`, `/usr/lib/lv2`). Where there is an X screen, the first scan takes about 40 seconds (LSP's VST3 connects to X every time it is asked for the window type); with no screen, 11 to 19 seconds.
- VS Code's 「meloet（デバッグ）」 uses CodeLLDB. When several child processes were started at the same time, lldb-server kept spinning with meloet stopped (no window appeared). Fixed by starting children one at a time and starting the scan after the screen's greeting (2026-11-19). If either is removed, it happens again. A stopped meloet shows as STAT `t` in `ps` and in TracerPid of `/proc/<pid>/status`.

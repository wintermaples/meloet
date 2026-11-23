# Windows and macOS port: research results and plan

Date: 2026-11-23. Round 12, feature D3. This document holds no code and nothing was built for it.

- The facts about other people's software come from the research notes [`2026-11-23-windows-macos-port-sources.md`](2026-11-23-windows-macos-port-sources.md). Source ids `[S1]` to `[S66]` and their ranks point into the list at the end of that file.
- The facts about meloet come from reading the working tree (branch `fea/round12`, 2026-11-23): `[L1]`.
- A few facts were checked again in the crate sources installed on this PC (`~/.cargo/registry/src/`): cpal 0.18.2 `[L2]`, midir 0.11.1 `[L3]`, vst3 0.3.0 `[L4]`, audio_thread_priority 0.35.1 `[L5]`. These are primary sources (A).
- Ranks: A = primary source, B = secondary source that cites a primary one, C = unknown source, memory, or our own reasoning. A design proposal is always C (it is our reasoning), even when every input is A. "Not confirmed" means nobody found a source; it is then listed as something to check or to prototype.
- Sizes: **S** = one helper job. **M** = a few helper jobs, no prototype needed. **L** = many jobs, or a prototype must come first.

---

## 1. Summary

**Can it be done?** Yes, with no change to the model, the protocol, the core's rules or the screen's code (C: our reading of `[L1]`). The OS-bound code sits in a small number of files (section 2). No licence blocks the port: every part we need is MIT, Apache-2.0, 0BSD or the OS's own API (section 6). Nobody has run meloet on either OS, so every speed number is unknown until measured.

**What is already in our favour** `[L1, A]`:

- A plugin window is a top-level window of the plugin host process (`crates/meloet-plughost/src/window.rs`, architecture 20-14). The meloet window is not its parent. So "embed a window of another process" (D3 item 2 in the request) is **not needed**. The plugin host makes its own HWND or NSWindow and the plugin's view goes inside it, in one process.
- The sleep and wake between the engine and the plugin host is behind four functions (`wait`, `wake`, `wait_until`, `store_and_wake` in `crates/meloet-plugin/src/ipc/futex.rs`). The callers already loop after a wake ("the word may still hold the old value; look again") and every sleep has a time limit. A slower or less exact wake-up on another OS changes speed, not correctness.
- A key's time is taken by meloet when the MIDI thread reads the key, not from the driver (architecture 12). The same rule works with any MIDI library.
- Keys already read <kbd>Mod</kbd> as Cmd on a Mac (`ui/src/keys/keys.ts`, spec 4). Icons `.ico` and `.icns` exist. The sound pack fetcher (ureq, rustls, the OS's certificate store) is not Linux-bound. `sync_dir` already has a `cfg(not(unix))` arm.

**The biggest blockers**

| | Windows | macOS |
|---|---|---|
| 1 | **Output latency.** cpal's WASAPI host is shared mode only, with the default period of about 10 ms `[L2, S4, S49, A]`. To come near the Linux key → sound number we need our own small WASAPI output with `IAudioClient3` (prototype). ASIO is not usable (GPLv3 or a signed agreement) `[S12, A]`. | **The screen.** WKWebView is not WebKitGTK's CPU path. rAF is capped at 60 fps and drops to 30 fps in Low Power Mode `[S27, B]`; one user report of jitter `[S27, C]`. Nothing measured. |
| 2 | **The audio-block handshake.** `WaitOnAddress` does not cross processes `[S47, A]`. Kernel events or semaphores must replace the futex, and the wait's time limit (a part of one block, under 1 ms) may be coarser than on Linux (C). Prototype. | **The plugin host as a windowed helper.** It needs an AppKit main loop, must sit inside the signed app bundle, and needs the "disable library validation" entitlement to load other people's plugins `[S39, A]`. Whether it can show windows without its own bundle is not confirmed. Prototype. |
| 3 | **Signing.** Without it SmartScreen warns and Smart App Control blocks `[S29, S34, S35, A]`. Azure Artifact Signing takes organisations in Japan but individuals only in the US and Canada `[S32, A]`. | **Money and a Mac.** Developer ID and notarization need the Apple Developer Program, 99 USD a year `[S36, S30, A]`, and builds need a Mac `[S30, A]`. |
| 4 | The screen on WebView2: frame pacing and key → screen have no source at all (not confirmed). | Minimum OS: cpal 0.18 needs macOS 14.2 `[L2, S1, A]`; the cross-process wait needs 14.4 `[S42, A]`. |

**Rough order of work** (section 4): stage 0, seams on Linux with no change of behaviour → five prototypes on real machines → stage 1, sound and keyboards with built-in sounds → stage 2, the screen's speed → stage 3, plugins → stage 4, packages and signing. Our advice is macOS first up to stage 3, then Windows (section 5, decision 1), because macOS keeps the POSIX parts (descriptors, `posix_spawn`, Unix sockets) and has a direct futex replacement; Windows needs each of those built anew.

**What cannot be answered from a desk.** Sections 3 and 4 name five prototypes. Until they are run on real hardware, the plan's sizes for audio on Windows, the handshake on Windows, and the screen on both are guesses.

---

## 2. Inventory of the Linux-only parts

From `[L1]`: grep for `libc::`, `std::os::unix`, `std::os::fd`, `cfg(target_os`, `cfg(unix)`, `/proc`, `/run`, `XDG_`, `dbus`, `alsa`, `pipewire`, `x11rb`, and the `Cargo.toml` of every crate. Today only `meloet-app` has `cfg(target_os)` gates and a target-specific dependency table; every other crate depends on its Linux parts without a gate, so **nothing but `meloet-model`, `meloet-protocol` and (most likely) `meloet-stretch` compiles on another OS today** (`meloet-sound` has no Linux-only code of its own that is not gated, but it depends on `meloet-engine`, which pulls in `dbus` and cpal's `pipewire` feature).

Risk: low = known API, no speed question. Mid = known API, behaviour must be measured. High = needs a prototype before the design is fixed.

### 2-1. Sound and keyboards

| File | What it does now | Windows | macOS | Size | Risk |
|---|---|---|---|---|---|
| `crates/meloet-engine/src/output.rs`, `input.rs` | cpal streams on `HostId::PipeWire`, the device found by PipeWire `node.name`; `BufferSize::Fixed` is only a wish (`node.latency`); block times from `OutputCallbackInfo::timestamp()` | cpal WASAPI host first; then our own output with `IAudioClient3` if the prototype says so | cpal CoreAudio host | M (L with our own WASAPI) | High (Windows), mid (macOS) |
| `crates/meloet-engine/src/engine.rs` (`monotonic_us`), `crates/meloet-midi/src/lib.rs`, `crates/meloet-app/src/clock.rs`, `crates/meloet-plugin/src/ipc/futex.rs` (`monotonic_ns`) | four copies of `clock_gettime(CLOCK_MONOTONIC)`; "the one clock every time in the protocol uses" | one function, on the clock cpal stamps with: `QueryPerformanceCounter` `[L2, A]` | `mach_absolute_time` scaled to ns `[L2, A]` (not `CLOCK_MONOTONIC`: see 3-1) | S | Mid (a wrong clock moves every recorded key) |
| `crates/meloet-engine/src/priority.rs`, `crates/meloet-plughost/src/rt.rs` (priority part) | `sched_setscheduler(SCHED_FIFO | SCHED_RESET_ON_FORK, 20)`, `RLIMIT_RTPRIO`, `RLIMIT_RTTIME`, rtkit over D-Bus, `gettid` | MMCSS `AvSetMmThreadCharacteristics` ("Pro Audio"), called by the thread itself `[S48, A]` | `thread_policy_set` time constraint `[S46, A]`, and join the output device's audio workgroup `[S41, S43, A]` | M | Mid |
| `crates/meloet-midi/src/input.rs`, `rules.rs`, `vkeys.rs`, `bin/` | ALSA sequencer: join as `meloet`, subscribe to System:Announce for plug in and out, pick the lowest readable port of a HARDWARE client, stamp keys at read; a virtual keyboard for tests | midir (WinMM) `[S7, A]`; plug in and out by looking at the port list on a timer (midir has no notice) `[S8, L3, A]`; no virtual ports `[S7, A]` | midir (CoreMIDI) `[S7, A]`; the same timer first, CoreMIDI's notify block later `[S56, A]`; virtual ports work `[S7, A]` | M | Mid |
| `crates/meloet-devices/` (all: `watcher.rs`, `facts.rs`, `route_pod.rs`) | PipeWire Routes for human names, plugged or not, `switchedByPc` / `stoppedByPc`, the PC-wide quantum | names and ids from cpal (`FriendlyName` `[S2, A]`); list changes from our own `IMMNotificationClient` (C); the running period from `IAudioClient3::GetCurrentSharedModeEnginePeriod` `[S49, A]` | names and ids from cpal; list changes from a CoreAudio property listener (C); the running buffer from the device's buffer frame size `[S5, A]` | M | Mid |
| `crates/meloet-engine/tests/private_pw/`, `tools/measure/record-timing.sh private` and the tools that call `pw-*` | a PipeWire of our own for tests that take the output down, and to fix the quantum | no equal; tests that need a device run by hand on a real PC | the same | S (gate them) | Low |

### 2-2. Plugins

| File | What it does now | Windows | macOS | Size | Risk |
|---|---|---|---|---|---|
| `crates/meloet-plugin/src/ipc/mod.rs` (mapping) | `memfd_create`, seals (`F_SEAL_SHRINK`, `F_SEAL_GROW`…), `mmap(MAP_SHARED | MAP_POPULATE)`, `mlock`, size check with `fstat` | `CreateFileMapping` on the paging file + `MapViewOfFile` `[S51, A]`; a section cannot be made smaller by the other side (C, memory) | `shm_open` with a name of at most 31 bytes `[S45, A]`, unlinked at once, descriptor handed to the child; no seals (see 3-6) | M | Mid |
| `crates/meloet-plugin/src/ipc/futex.rs` | shared futex wait and wake, time limit in ns | kernel event or semaphore per word + a high-resolution timer for the limit (C) | `os_sync_wait_on_address_with_timeout` and `os_sync_wake_by_address_*` with the SHARED flag, macOS 14.4+ `[S42, A]` | M (macOS), L (Windows) | High (Windows), mid (macOS) |
| `crates/meloet-plugin/src/launch.rs` | hands the control socket as stdin and the memory as stdout; `take_stdio` (`dup2`, `/dev/null`); `PR_SET_PDEATHSIG`; `pidfd_open` and `pidfd_send_signal`; `setpriority` for scans; `posix_spawn` on purpose; one child at a time | pipes or named objects (3-6); a Job object with `KILL_ON_JOB_CLOSE` `[S50, A]`; wait on and end the process handle; `BELOW_NORMAL_PRIORITY_CLASS` (C) | the same descriptors as Linux; a pipe the child watches, or kqueue, in place of `PDEATHSIG` (C; no primary source); `kill` on the `Child`; `setpriority` works | M | Mid |
| `crates/meloet-core/src/plugins/hosts.rs`, `scanner.rs`, `crates/meloet-plughost/src/main.rs`, `control.rs` | `UnixStream::pair()` as the control channel; `poll` on the socket, the X connection and the plugin's descriptors; `recv(MSG_DONTWAIT)` | a channel type from the seam (3-6); the main thread becomes a Win32 message loop | Unix socket stays; the main thread becomes an AppKit run loop | M | Mid |
| `crates/meloet-plughost/src/window.rs` | X11 top windows through x11rb: make, title, size hints, front, focus in and out by asking X, RandR for the monitor, close | HWND top window + message loop; DPI per monitor (VST3 sizes are physical pixels on Windows `[S16, A]`) | NSWindow + NSView; VST3 sizes are logical units on macOS `[S16, A]` | L | High (macOS), mid (Windows) |
| `crates/meloet-plughost/src/formats/vst3.rs`, `vst3/runloop.rs` | `dlopen`, `ModuleEntry`/`ModuleExit`, `attached` with `kPlatformTypeX11EmbedWindowID`, the Linux-only `IRunLoop` (descriptors and timers) | load the DLL in `Contents/x86_64-win/` `[S17, A]`, `attached` with `kPlatformTypeHWND` `[S16, A]`; no `IRunLoop` (the interface is `Steinberg::Linux` `[L1, A]`) | load the bundle's `Contents/MacOS/` binary `[S17, A]`, `attached` with `kPlatformTypeNSView` `[S16, A]`; no `IRunLoop` | M | Mid |
| `crates/meloet-plughost/src/formats/lv2.rs`, `lv2/*.rs`, `crates/meloet-ai/src/presets/lv2.rs` | LV2 through `livi` (lilv), X11 UI, `dlopen` of the UI | **left out** (gate the module; LV2 plugins show as missing things) | **left out** | S | Low |
| `crates/meloet-plugin/src/scan.rs`, `crates/meloet-core/src/plugins.rs`, `crates/meloet-ai/src/presets.rs`, `surge/presets.rs` | plugin places `~/.vst3`, `/usr/lib/vst3`, `~/.lv2`, `/usr/lib/lv2`, `LV2_PATH`; preset places under `$HOME` and `/usr/share`; `~` in shown paths | `%LOCALAPPDATA%/Programs/Common/VST3`, `Program Files/Common Files/VST3` `[S18, A]` | `~/Library/Audio/Plug-ins/VST3`, `/Library/Audio/Plug-ins/VST3` `[S18, A]` | S | Low |
| `crates/meloet-plughost/src/crashlog.rs`, `formats/fake.rs`, `main.rs` (`PR_SET_DUMPABLE`, `_exit`) | development only: signal handler with a backtrace; no core dumps for the fake plugin | gate to Linux; nothing in its place at first | gate to Linux | S | Low |
| `crates/meloet-plughost/tests/engine.rs`, `host.rs`, `crates/meloet-plugin/tests/ipc_fork.rs` | tests with `fork`, `SIGSTOP`, `SIGSEGV`, `waitid` | gate; new tests that end or freeze the child the Windows way | most work as they are (POSIX) | M | Low |

### 2-3. The shell, files and the AI link

| File | What it does now | Windows | macOS | Size | Risk |
|---|---|---|---|---|---|
| `crates/meloet-app/src/main.rs` (`set_jsc_options`: `JSC_useGenerationalGC=0`, `WEBKIT_DISABLE_COMPOSITING_MODE=1`; `decorations`; `to_front` with the activation token) | tuning that belongs to WebKitGTK; GTK window calls | not set; WebView2 has its own compositor and GC (3-4) | not set | S | Low (the code), high (what the screen then does) |
| `crates/meloet-app/src/webprocess.rs` | kills the WebKit web process before exit (NVIDIA EGL crash), reloads the page when it dies, exits on SIGTERM / SIGINT / SIGHUP | already has an empty `cfg(not(linux))` arm; later: WebView2 "process failed" event (not researched) | already empty; later: WKWebView "content process did terminate" (not researched) | S | Mid |
| `crates/meloet-app/src/logind.rs` | logind "delay" lock, writes the recovery file on `PrepareForShutdown` | `WM_QUERYENDSESSION` + `ShutdownBlockReasonCreate` `[S58, A/B]` | `applicationShouldTerminate` `[S57, A]` (that logout and power-off go this way: not confirmed) | M | Mid |
| `crates/meloet-app/src/pcwindow.rs` | GTK `FileChooserNative`; opens the file manager; has a `cfg(not(linux))` arm | Tauri's dialog plugin or the OS's own dialog (not researched) | the same | S | Low |
| `crates/meloet-app/src/single.rs` | `flock` in `$XDG_RUNTIME_DIR/meloet/`, a Unix socket beside it, `SO_PEERCRED`, hands over the song path and the activation token | a named mutex + a named pipe (C) | `flock` + Unix socket in a short path under the user's own folder; the OS already sends "open this file" to the running app (C; not researched) | M | Mid |
| `crates/meloet-app/src/devhooks.rs` | cairo snapshots, GTK calls (already gated) | gate stays; the measuring hooks need new back ends | the same | M | Low |
| `crates/meloet-app/Cargo.toml`, `tauri.conf.json`, `packaging/`, `scripts/build-deb.sh`, `scripts/check-release.sh` | deb only; `.desktop`, MIME xml; `externalBin` for `meloet-plughost` and `meloet-mcp` | NSIS installer, file type in the installer (section 3-11) | `.app` in a DMG, file type in `Info.plist`; identifier `dev.meloet.app` must change ("ends with .app", `docs/progress.md` 7) | M | Mid |
| `crates/meloet-ai/src/port.rs`, `lib.rs` | Unix socket `$XDG_RUNTIME_DIR/meloet/ai.sock`, folder 0700, socket 0600, peer uid check, tokio `net` | named pipe that only this user may open (C) | Unix socket in a short path (at most 104 bytes, C) under the user's folder; modes and uid check as now | M | Mid |
| `crates/meloet-mcp/src/place.rs`, `main.rs` | looks in `$XDG_RUNTIME_DIR` and `/run/user/<uid>`; uid from `/proc/self`; `UnixStream` | the pipe's fixed name | the fixed path | S | Low |
| `crates/meloet-ai/src/scratch.rs` | `<temp>/meloet-ai-<uid>`, mode 0700, `geteuid`, `localtime_r` | `%TEMP%` is already per user; no mode bits | works (POSIX) | S | Low |
| `crates/meloet-core/src/settings.rs` (`XDG_CONFIG_HOME`, `HOME/.config`), `recovery.rs` (`dirs::state_dir`, else `~/.local/state`), `crates/meloet-plugin/src/scan.rs` (`XDG_CACHE_HOME`), `sounds.rs`, `sf2store.rs`, `peaks.rs`, `audiofiles.rs` | hand-made XDG reading in three places, `dirs` in the others | `dirs` everywhere: `RoamingAppData` for settings, `LocalAppData` for data and caches `[S61, A]`; `state_dir` is `None` there (C: from the `dirs` design), so the recovery file needs a named place | `~/Library/Application Support`, `~/Library/Caches` `[S61, A]` | S | Low |
| `crates/meloet-core/src/settings.rs:569`, `service/export.rs:88` | "is that pid still alive" by `/proc/<pid>` | `OpenProcess` (C) | `kill(pid, 0)` (C) | S | Low |
| `crates/meloet-core/src/sounds/fetch.rs:486` | free disk space with `rustix::fs::statvfs` | `GetDiskFreeSpaceExW` (C) | `statvfs` works (C) | S | Low |
| `crates/meloet-core/src/songfile.rs`, `recovery.rs`, `sf2store.rs`, `audiofiles.rs` (safe writing: temp file, `sync_all`, rename, `.prev`, `sync_dir`) | rename over an existing file; folder sync under `cfg(unix)` | rename over a file that is open elsewhere fails on Windows (C, memory); must be tried with the song open in a virus scanner's hands | works | S | Mid |
| `crates/meloet-sound/src/memory.rs` | `malloc_trim` (already gated to Linux + glibc), `mallinfo2` | nothing; measure the memory after closing a big song | nothing; the same | S | Low |
| `crates/meloet-stretch/build.rs` | C++17 through `cc` | MSVC; untried (`docs/progress.md` 7) | clang; untried | S | Mid |
| `tools/measure/*` (25 of the files name `pw-*`, gdb, `/proc` or X), `tools/design/shoot_app.py` | measuring and pictures on Linux | only the parts inside the app (`MELOET_UI_SCRIPT`, the latency records of the engine, `MELOET_PIPE`) carry over; the shells around them do not | the same | L | Mid |

### 2-4. What changes shape, or cannot be taken over (D3 item 4)

- **"Show the buffer that is really running."** Kept. The number is the WASAPI engine period `[S49, A]` or the CoreAudio device buffer `[S5, A]` in place of PipeWire's quantum.
- **"PC-wide settings are never changed."** The rule needs a new reading there (section 5, decision 8): on Windows a small shared period asked by one app becomes the period of every app on that output `[S49, A]`; on macOS the buffer size is a property of the device, not of our stream `[S5, A]`. That is the normal way an app asks on those systems, and it is close to what meloet already does on Linux (it asks `node.latency` and PipeWire decides).
- **PipeWire names and the two "cannot be chosen" states** (`switchedByPc`, `stoppedByPc`; the easy names such as 「ヘッドホンの穴」 from `port.type`): they come from PipeWire Routes. On Windows and macOS the list is simply the devices the OS shows. The states stay in the protocol and are never sent. Whether the easy names can be made from what Windows and macOS tell (jack kind, form factor) is not researched.
- **logind and the 5 s rule at power-off**: replaced by each OS's own "the session ends" question (3-8). The 2 s autosave of the recovery file does not change and is the real safety net.
- **rtkit, `SCHED_FIFO`, the number 20, the 15-process limit of rtkit**: gone. One reason for "one plugin host process per plugin kind" (rtkit's limits, architecture 20-1) does not exist there; the other reasons (memory, one crash takes one plugin's instances) stay, so the shape stays.
- **memfd seals**: no equal on macOS (3-6).
- **`posix_spawn` shares memory until exec, so starting a child never copies meloet's page tables**: true on macOS; Windows has no fork at all, so the worry is gone.
- **`WEBKIT_DISABLE_COMPOSITING_MODE`, `JSC_useGenerationalGC`, the redraw rules of architecture 7 (11 rectangles, 25 %, `cover`), `webkit-damage.sh`**: WebKitGTK only (C). The WebGL budget of 12 stays (3-4).
- **The virtual keyboard `meloet-vkeys`**: not on Windows (midir has no virtual ports there `[S7, A]`). Measuring needs another way to press keys (3-2).
- **LV2**: not ported (section 5, decision 6).

---

## 3. Topic by topic

Each topic: the facts, the proposed design, what was rejected, what to measure or prototype first. Every "proposed design" is rank C.

### 3-1. Audio

**Facts**

- cpal 0.18.2 (Apache-2.0) is what meloet has; its WASAPI host opens **shared mode only** (`AUDCLNT_SHAREMODE_SHARED` in both builders), event-driven, output with `AUTOCONVERTPCM`; no `IAudioClient3`, no MMCSS call `[L2 `src/host/wasapi/device.rs`, S4, A]`. The buffer range it reports is `GetBufferSizeLimits` or else just the device period `[S4, A]`.
- Windows: shared mode's default buffer is 10 ms; `IAudioClient3` can ask for less, down to 128 frames (2.67 ms at 48 kHz) with the inbox HD Audio driver; the driver must support it, and all apps on that output then run at the small period `[S49, A]`. The Windows 10+ audio engine itself adds about 1.3 ms `[S49, A]`.
- ASIO: GPLv3 or a proprietary agreement since 2025-10-15 `[S12, S13, A]`; that GPLv3 does not fit an MIT OR Apache-2.0 binary is our reading (C).
- cpal's times, which meloet's "position reaching the ear" rule is built on (architecture 12) `[L2, A]`: on WASAPI `callback` is `QueryPerformanceCounter` and `playback = callback + (frames written − frames the device consumed) + GetStreamLatency`; on CoreAudio `callback` is the callback's `mHostTime` (`mach_absolute_time` scaled to ns) and `playback = callback + device buffer frames + device latency + safety offset`.
- macOS: the fixed buffer is the device property `kAudioDevicePropertyBufferFrameSize`, checked against its range `[S5, A]`. cpal 0.18.2's README gives macOS 14.2 as the minimum `[L2, S1, A]`. Recording needs the audio-input entitlement and `NSMicrophoneUsageDescription` `[S39, A; the plist key B]`; playing needs no permission.
- No primary number for real CoreAudio or WASAPI key → sound latency was found. Do not quote one.

**Proposed design**

1. Split `meloet-engine`'s way out and way in behind one seam: "open this endpoint with this wish for the block size; give blocks with (`callback_us`, `playback_us`) on the engine's clock; say what really runs". `output.rs` and `input.rs` become the Linux back end. The engine above it does not change.
2. One clock function for the whole product, `meloet_engine::monotonic_us` (the other three copies call it): Linux `CLOCK_MONOTONIC`, Windows QPC, macOS `mach_absolute_time`. **Rule to keep: a key's time and a block's `playback` time are read on the same clock.** On macOS that is not `CLOCK_MONOTONIC` (which, from memory, keeps counting while the Mac sleeps; C), so the clock must be chosen per OS, not left to libc.
3. Windows, first: cpal's WASAPI host as it is. This gives a working product with about 10 ms of buffer (C, from the A facts above). Then, if prototype P1 shows a real gain: our own output behind the same seam, written with the `windows` crate (MIT OR Apache-2.0 `[S6, A]`): `IAudioClient3::InitializeSharedAudioStream` with the smallest period the driver gives, event-driven, the thread in MMCSS "Pro Audio". The running period is read back and shown.
4. macOS: cpal's CoreAudio host; ask for a fixed buffer (start with 256 frames as on Linux; the measured number decides).
5. Audio input comes after output on each OS (it adds the permission question on macOS and the "two devices, two clocks" question on both: on Linux PipeWire runs input and output in one cycle (architecture 22-5); nothing promises that there).

**Rejected**

- ASIO through cpal's `asio` feature: the licence `[S12, A]`.
- WASAPI exclusive mode: it takes the output away from every other app `[S49, A]`, against the spirit of "meloet does not change the PC".
- Patching cpal in place: we would carry a fork of a fast-moving part. A patch offered upstream is fine, but the plan must not wait for it.
- cpal's `realtime` feature for priority: brings in `audio_thread_priority`, MPL-2.0 `[L5, A]`, and joins MMCSS "Audio", not "Pro Audio" `[S64, A]`; meloet already decided against it on Linux (architecture 23-10).

**Measure or prototype first**

- **P1 (Windows)**: key → sound with cpal shared mode against a 100-line `IAudioClient3` output, on at least two PCs (one with the inbox HD Audio driver, one with a USB audio box). Also: does the small period hold when another app plays?
- Both OSes: how true cpal's `playback` time is. Run the loop-cable measurement (「遅れを測る」, architecture 22-5) and compare the measured `X` with Linux on the same audio box. A large or changing `X` means the "PC-reported" part is wrong there and recorded keys will sit off.
- macOS: the smallest buffer that runs without lost blocks on built-in output and on a USB box; whether changing the device buffer disturbs other apps' sound.

### 3-2. MIDI

**Facts**

- midir 0.11.1 is MIT; back ends WinMM, WinRT (feature), CoreMIDI; virtual ports everywhere but Windows `[S6, S7, A]`.
- midir has no plug-in or plug-out notice on any back end; ports are listed anew on each call; a removed device gives `InvalidPort` `[S8, A]`.
- Timestamps `[L3, S8, A]`: WinMM gives the driver's **millisecond** count times 1000 (`handler.rs`: "milliseconds -> microseconds"); CoreMIDI gives host time in µs (the same clock family as cpal's CoreAudio times).
- CoreMIDI can tell about setup changes through the notify block of `MIDIClientCreate`, on any thread `[S56, A]`; which message types exist was not read (not confirmed).
- Windows MIDI Services (new stack, MIT, has a device watcher and better timestamps) is Windows 11 only, the SDK is in preview, and no Rust binding was found `[S9, S10, S11, A]`. Old WinMM apps go through the new service on such PCs and every port becomes multi-client `[S9, S10, A]`. On Windows 10 a WinMM port can be opened by one app at a time (C, general knowledge).

**Proposed design**

1. `meloet-midi` keeps its public face (`MidiInput::start`, the sink contract, `MidiMessage::{Added, Removed, Ready, Stopped, …}`, `Keyboard` ids and names). `input.rs` becomes the ALSA back end; a second back end uses midir. `rules.rs` (what counts as a keyboard) is ALSA-shaped and gets a sibling per back end.
2. **Key time**: stamp with `monotonic_us()` at the start of midir's callback, as the ALSA back end does at read. Do not use WinMM's millisecond stamp (too coarse for a rule whose rounding error is ± 0.52 ms). On macOS the packet's host time is on the engine's clock and is taken earlier than our callback, so it may be better; prototype P4 decides, and the rule in architecture 12 would then need one new line.
3. **Plug in and out**: a thread looks at the port list every 500 ms and turns the difference into `Added` and `Removed`. A port that gives `InvalidPort` is `Removed` at once. All the rules that hang on `Removed` (stop all sound, clear held keys and pedals) stay as they are.
4. **Keyboard identity**: ALSA gives a client id and name; midir gives a port name (plus the CoreMIDI unique id on macOS). Two keyboards of the same model may get the same name on Windows. The id only has to be stable while plugged; settings do not store keyboards (to check in `settings.rs` before building).
5. Later, not in the first port: CoreMIDI notices through the `coremidi` crate (MIT `[S6, A]`; whether it wraps the notify block is not confirmed); Windows MIDI Services when a Rust binding exists.

**Rejected**

- WinRT back end of midir first: no notice either `[S8, A]`, and one more API family for no gain we can name.
- Our own WinMM or CoreMIDI code first: midir is small and MIT; write our own only where midir lacks something we measured a need for.

**Measure or prototype first**

- **P4**: recorded position error on each OS (the job of `tools/measure/record-timing.sh`). On macOS a virtual CoreMIDI port can play the keys. On Windows there are no virtual ports: use a hardware loop (USB-MIDI out to in), or add a development hook that feeds keys into the MIDI thread's sink with a time (then the measure starts at "Rust has the key", which is where the Linux "Rust → DAC" number starts too).
- How long a 500 ms look at the port list takes on WinMM with several devices, and that it never blocks the callback.
- Windows 10: what the person sees when another app holds the keyboard (a wording may be needed; section 5, decision 12).

### 3-3. Device names and hot-plug (the job of `meloet-devices`)

**Facts**

- cpal 0.18: stable device ids (`DeviceTrait::id`, `HostTrait::device_by_id`) `[S2, A]`; names from `FriendlyName` on Windows `[S2, A]`. cpal has no public "the device list changed" notice (`[S3, A]`, read from an excerpt, so not fully confirmed). It does re-route a default-device stream by itself since 0.18.0 and reports `DeviceNotAvailable`, `StreamInvalidated` `[S2, A]`, which meloet's error callback already counts as "the stream is lost" `[L1, A]`.
- The running buffer can be read on both (2-4).

**Proposed design**

- `meloet-devices` keeps `DevicesPicture { outputs, inputs, clock }` and `DevicesEvent`; a new back end fills it from cpal's device list. `Endpoint.id` is cpal's device id; `availability` is "plugged" for everything listed; `friendly` (the easy names) is `None` at first; `clock` holds the running period, not a PC-wide fixed quantum.
- List changes: first a timer (every 1 s, off the audio thread; the core already starts the watcher again when it stops, architecture 2). Then the OS's own notices (`IMMNotificationClient`, a CoreAudio property listener) once the rest works. Both are C: the interfaces are public but were not read this round.
- meloet always opens the device the person chose by id, never "the default", so cpal's automatic re-route does not move the sound without the screen knowing.

**Rejected**: showing only "the default output" (simple, but it breaks 「出口をえらぶ」 and the saved choice).

**Measure first**: that a device id from cpal survives unplug and replug, a reboot, and a driver update on Windows (the saved setting depends on it). Whether listing devices on a timer disturbs a running stream (on some systems listing is not free; not confirmed).

### 3-4. Webview and WebGL

**Facts**

- Tauri 2: WebView2 (Chromium) on Windows, WKWebView on macOS `[S22, A]`. WebView2 is on Windows 11 and on "the vast majority" of Windows 10 PCs, not all `[S23, A]`; Tauri's installer can fetch it (default, needs the internet), carry the small bootstrapper (about 1.8 MB) or the whole offline installer (about 127 MB) `[S28, A]`.
- WKWebView only gets WebKit updates with the OS `[S22, A]`.
- WebGL contexts: WebKit's limit is 16 active (`maxActiveContexts`), the oldest is lost beyond it `[S25, A]`. Chromium also drops the oldest; the number 16 is from secondary sources and the default was not found in the source `[S26, B]`.
- WKWebView: rAF capped to 60 fps on macOS 13 to 15 `[S27, B]`; 30 fps in Low Power Mode `[S27, B]`.
- WebView2: GPU on by default, separate browser, renderer and GPU processes `[S24, A]`. Frame pacing and input latency: **no source found**.

**Proposed design**

- Keep the screen's code as it is: one WebGL context per region, budget 12, contexts made when a region's box arrives. 12 of 16 holds on all three engines (C from `[S25, A]` and `[S26, B]`).
- Set none of the WebKitGTK variables there. Treat each engine as new: measure first, then tune. The 60 Hz target stays; on a Mac in Low Power Mode the screen will run at 30 fps and nothing in meloet can change that (C from `[S27, B]`); say so in the user document.
- Windows installer: carry the bootstrapper (`embedBootstrapper`), so a PC without WebView2 gets it with one small download and a PC with it needs no internet (C).
- The measuring that lives inside the page (`MELOET_UI_SCRIPT`, frame times, `busyMs`) is engine-neutral and is the tool for stage 2. `webkit-damage.sh` and its relatives stay Linux-only.
- The web process watcher: leave empty at first; add "the page's process died → load the page again and take the whole state" (the promise already allows it: hello → subscribe → snapshot) when the events of each webview have been read.

**Rejected**

- A different shell (winit + wgpu, or another webview) for those OSes: it would throw away the whole screen. Only on the table if stage 2 fails on an OS.
- Fixed-version WebView2 inside the installer: 180 to over 250 MB `[S28, S23, A]`, larger than the app and the sound pack fetcher together.

**Measure or prototype first**

- **P3**: the real screen (built from `ui/` as it is, with the fake scenes if the Rust side does not run yet) in WebView2 and WKWebView: frame times with the piano roll and the mixer open, context loss with 10 regions, memory over 10 minutes of 60 Hz, key → screen. On a Mac also with Low Power Mode on.
- Whether Tauri's `Channel` with binary telemetry at 60 Hz costs more there (on Windows it crosses WebView2's process border; not researched).
- Text and font drawing against the mocks (`tools/design/shoot_ui.py` runs a browser on Linux; the fake scenes can be opened in Edge and Safari by hand).

### 3-5. Plugin host and plugin windows

**Facts**

- VST3 SDK 3.8 is MIT `[S13, S14, S15, A]`. The `vst3` crate 0.3.0 that meloet uses is MIT OR Apache-2.0 and has Windows-specific code in it `[L4, A]`; that it builds and works on macOS was not checked (not confirmed).
- Bundle layout and places per OS: `[S17, S18, A]` (in table 2-2).
- `IPlugView::attached` takes `HWND` on Windows, `NSView` on macOS; sizes are physical pixels on Windows and logical units on macOS `[S16, A]`.
- macOS: a notarized app must use the hardened runtime, and loading other people's plugins needs the "Disable Library Validation" exception `[S39, A]`.
- Audio Units: AUv2 load into the host's process, AUv3 are out of process by default `[S40, A]`. Rust bindings for AudioToolbox: not confirmed.
- AppKit windows on the main thread only: general rule (C). A helper executable without its own bundle showing windows: not confirmed.
- LV2 on Windows and macOS: rare (C); lilv there not researched.

**Proposed design**

1. The shape stays: one `meloet-plughost` per plugin kind; audio threads sleep on the bell; the main thread owns windows and the control channel; the audio threads never wait for the main thread.
2. `control.rs`'s loop ("sleep in `poll` on the socket, the X connection and the plugin's descriptors, at most one tick") becomes a seam: **"wait for the next thing, at most this long"**, with three back ends. Linux: `poll` as now. Windows: `MsgWaitForMultipleObjectsEx` on the control channel's event plus the thread's message queue. macOS: the AppKit run loop with the socket as a run-loop source and a timer for the tick. This is the largest single change in the plugin host, and it can be put in on Linux first with no change of behaviour.
3. `window.rs` becomes a seam with the calls `control.rs` uses today (make, title, size, limits, front, focus in and out, close, the monitor's size). The "ask X where the focus is every 0.1 s" trick is replaced by each OS's own focus notices.
4. `formats/vst3.rs`: the loader and the `attached` type per OS; `runloop.rs` only on Linux.
5. macOS start-up: `meloet-plughost` lives inside `meloet.app` (Tauri `externalBin`), is signed with the app, and carries the library-validation exception itself; the main app does not. It runs as an "accessory" (no Dock icon) if P2 shows that works.
6. Formats: section 5, decision 6.

**Rejected**

- Putting the plugin's view inside the meloet window: not in the spec; across processes it is "legal but hard to manage" on Windows `[S52, B]`, and no primary source exists for macOS `[S66, C]`.
- AUv3 out-of-process hosting as a way to drop our own plugin host on macOS: adds XPC between us and the plugin with unknown real-time cost (C), and user reports name missing callbacks and blank views (C).
- Plugins inside the meloet process "only on macOS, to save work": one bad plugin would take the song down; the spec's 「（名前）が止まりました…」 flow depends on the separate process.

**Measure or prototype first**

- **P2**: a tiny host process, started by another process, that loads one real VST3 (Surge XT is on all three OSes), opens its window, and survives open/close 50 times (the job of `tools/measure/window-rounds.py`). On Windows: DPI on two monitors with different scale. On macOS: without its own bundle, signed with the hardened runtime and the exception, started from inside a signed `.app`; where the keyboard focus goes; that the window comes to the front.
- Scan time for a real plugin folder on each OS (Linux: 11 to 40 s for about 300 bundles on this PC).
- That a VST3 state saved on Linux loads in the same plugin on Windows and macOS (song files travel between PCs). Not confirmed; try with Surge XT.

### 3-6. Shared memory, wake-ups and child processes

**Facts**

- The handshake `[L1, A]`: per block, per slot, the engine stores a number in `request`, bumps the header's `bell` and wakes the host if `plugin_waiting` says it sleeps; the host answers in `done` and wakes the engine if `engine_waiting` says so. The engine's sleep has a limit taken from the time left in the block (`PLUGIN_WAIT_SHARE` 0.9). Several host audio threads (up to 4) may sleep on one bell. On Linux a round trip is p50 8 to 13 µs with both sides real-time (architecture 20-2).
- Windows: `WaitOnAddress` is in-process only `[S47, A]`. Named shared memory through `CreateFileMapping` / `OpenFileMapping`; the `Global\` prefix needs a privilege, the session's own names do not `[S51, A]`. Job objects end the children when the last handle closes `[S50, A]`. Windows AF_UNIX sockets are stream-only and cannot pass handles `[S62, B]`.
- macOS 14.4+: `os_sync_wait_on_address` with `OS_SYNC_WAIT_ON_ADDRESS_SHARED`, with timeout and deadline forms `[S42, A]`. Before 14.4 only the private `__ulock_wait` `[S44, A that it exists]`. POSIX shm names are at most 31 bytes `[S45, A]`.
- Kernel events and semaphores cross processes on Windows: general knowledge (C), not read this round. Their wake-up time is not measured.

**Proposed design**

1. **The words stay the truth; the OS object is only an alarm.** Keep the layout of the mapping and every rule in `ipc/mod.rs` and `ends.rs` (stale answers, the progress counter, `HANG_MS`). Keep the four functions of `futex.rs` and add a small "sleeper" value to them, made when the mapping is made:
   - Linux: nothing in it (the address is enough).
   - macOS: nothing in it; the functions call `os_sync_*` with the SHARED flag on the same address.
   - Windows: one kernel object per word that someone sleeps on: a **semaphore for the bell** (several threads sleep; `wake(n)` releases n) and an **auto-reset event per slot for `done`**. `wait(word, expected, limit)` is: read the word; if it is not `expected`, return; else wait on the object with the limit; the caller looks again.
2. **Why this is still correct on Windows.** A futex checks the word and sleeps in one step; an event does not. But an event and a semaphore remember a wake that came before the wait, so a wake between "read the word" and "wait" is not lost: the wait returns at once. What can happen instead is a wake left over from an earlier block, which makes one wait return early. The callers already look at the word again after every wake `[L1, A]`, so the cost is one extra turn of the loop, never a wrong block. The `plugin_waiting` / `engine_waiting` words keep their meaning (skip the kernel call when nobody sleeps).
3. **The time limit on Windows.** A plain wait's limit is in whole milliseconds and may follow the system timer (C, memory). The engine's limit is a part of one block (under 2.67 ms at 128 frames), so a coarse limit would turn "the plugin is late, go on without it" into a lost block of our own. Design: the engine thread owns one high-resolution waitable timer, made on the core thread, and waits on (event, timer) together. That such a timer is exact enough at real-time priority is C (memory) and is the heart of prototype P5. We do not lower the system timer resolution `[S48, A]`.
4. **Handing things to the child.**
   - macOS: as on Linux. `shm_open` (short random name, unlinked at once) in place of `memfd_create`; the descriptor as the child's stdout and a `UnixStream::pair()` end as its stdin; `take_stdio` works as it is.
   - Windows: the mapping, the events and the semaphore get random names in the session's own name space, passed on the command line; the control channel is a named pipe with a random name, opened by this user only. No handle inheritance (simple to get wrong with std's `Command`; not researched). `take_stdio` becomes "point the three standard handles at `NUL` or the log".
5. **No seals on macOS.** The seals stop the other side from shrinking the memory under the engine's feet (a `SIGBUS` on the audio thread). On macOS there is no seal. What we know: the child already gets its lengths and positions range-checked by the engine `[L1, A]`; whether a child can shrink a POSIX shm object after it was sized is not confirmed (from memory macOS allows `ftruncate` on it only once; C). To check before stage 3; if it can, the risk is a plugin that does it on purpose, which is far from the cases the plugin host exists for (crashes and hangs).
6. **Child control.** A small "child" seam in `launch.rs`: "has it ended, within this time" and "end it now". Linux: pidfd as now. Windows: the process handle, and one Job object with `KILL_ON_JOB_CLOSE` for all children `[S50, A]`. macOS: `Child::try_wait` / `kill`, and for "die with the parent" the child watches the control socket, which it already does ("the process ends when the socket closes", `control.rs` `[L1, A]`); a scan child watches its output pipe the same way. A frozen child that never reads its socket is ended by the watcher, as now.
7. The rule "start children one at a time" stays on every OS (it costs nothing and the debugger question is open there too).

**Rejected**

- `interprocess` (0BSD OR Apache-2.0 `[S6, S63, A]`) for the control channel: it brings its own types and, for async, only tokio; the control channel is blocking code with our own framing. Use the OS calls behind our small seam. (It may still serve the AI link, 3-10.)
- `shared_memory` crate: no release since 2022 `[S6, A]`.
- Spinning in place of sleeping: measured worse without real-time on Linux (architecture 20-2), and on battery it is the wrong thing to do. A short spin before the sleep stays as a fallback idea if P5 shows slow wake-ups (C).
- macOS below 14.4 with `__ulock_wait`: private API `[S44]`; a release of macOS may change it under us.
- A different IPC shape per OS (for example one block of delay on Windows to avoid the round trip): 5.33 ms per plugin crossing was the reason the round trip exists (architecture 20-2).

**Measure or prototype first**

- **P5**: port `spikes/plugins/ipcbench/` in spirit (a new spike, not a copy into the product): two processes, the real ring layout, both ends at the OS's audio priority, 80 round trips in a row. Numbers to bring back: round trip p50 / p99 / max; the same with the PC busy; how late a 500 µs time limit fires. Compare with the Linux numbers in `docs/history/measurements/2026-11-19-round5-ipc.md`.
- macOS: that `os_sync_wait_on_address` with SHARED works on memory from `shm_open` mapped at different addresses in the two processes (the header says "the exact same address" for wait and wake `[S42, A]`; whether that means the same shared object or the same number is not clear from the notes). **If it means the same virtual address, the design for macOS changes** (map at a fixed address, or fall back to Mach semaphores). Check this first; it is one afternoon.

### 3-7. Real-time priority

**Facts**: MMCSS tasks and the "Pro Audio" class (thread priority 23 to 26), called by the thread that wants it `[S48, A]`. macOS time-constraint policy fields `[S46, A]`; audio workgroups, which "span multiple processes, including… plug-ins", joined only by real-time threads, macOS 11+ `[S41, S43, A]`. How to get the device's workgroup (`kAudioDevicePropertyIOThreadOSWorkgroup`) is from memory only (C). On macOS and Windows the audio callback thread comes from the OS's audio system, and on macOS it is already real-time and in the workgroup `[S41, A]`.

**Proposed design**

- A seam "make this thread an audio thread" with one call, used by `priority.rs` and `rt.rs`. Linux keeps today's code (including the helper thread, since the callback must not call D-Bus).
- Windows: the thread calls MMCSS itself at its start (the call is cheap and local, no helper needed; C). For cpal's WASAPI thread this happens in the first callback. To check: that the first call does not block for long (it is once per stream).
- macOS: the engine's callback thread needs nothing. The plugin host's audio threads set the time-constraint policy from the block length and then join the output device's workgroup, whose handle the engine side reads and hands over at start (how a workgroup crosses a process border is not confirmed; without it the threads are still real-time, only not counted together with the device's deadline).
- The rule "the engine's thread never stands below the threads it waits for" has no number to set there. It becomes something to see in P5 (a slow wake-up of the engine after the host's answer would be the sign).

**Rejected**: `audio_thread_priority` (MPL-2.0 `[L5, A]`, "Audio" not "Pro Audio" `[S64, A]`; architecture 23-10 already says no on Linux). `THREAD_PRIORITY_TIME_CRITICAL` alone (no MMCSS): cpal's fallback `[S4, A]`, weaker than what the OS offers audio apps.

**Measure first**: blocks that missed a plugin's answer with 16 Surge XT on a busy PC, with and without the priority (the same test as `docs/history/measurements/2026-11-22-round8-rt.md`).

### 3-8. Shutdown hooks

**Facts**: Windows asks with `WM_QUERYENDSESSION`, the app may hold the shutdown with `ShutdownBlockReasonCreate` and must clean up in `WM_ENDSESSION` `[S58, A for the API pages, read only as snippets; B for the recipe]`. macOS: `applicationShouldTerminate`, which may answer later `[S57, A]`; that power-off and logout come this way is not confirmed. How Tauri 2 passes these on: not researched.

**Proposed design**: `logind.rs` already has the right shape (a `watch(core)` with an empty arm for other OSes). Fill the arms: on the question, tell the core "write the recovery file now", wait for it (it is the same call the logind path makes), then let the OS go on. Never show a dialog from here. The 2 s autosave (chapter 15) already bounds the loss to 2 s of work if the hook never runs, so this topic can come late (stage 4).

**Rejected**: blocking shutdown to ask 「保存しますか」: the OS shows its own "this app prevents shutdown" screen; the recovery file exists so we do not have to ask.

**Check first**: what Tauri 2.12 exposes (`main.rs` uses `WindowEvent::CloseRequested` and `RunEvent::Exit` today `[L1, A]`; whether the OS's end-of-session reaches them on Windows and macOS must be read in the installed Tauri and tao sources).

### 3-9. File locations

**Facts**: `dirs` gives `RoamingAppData`, `LocalAppData`, `~/Library/Application Support`, `~/Library/Caches`, and no runtime folder on either OS `[S61, A]`.

**Proposed design**: one small module in `meloet-core` that names every place (settings, recovery, data, caches, the plugin scan record, logs), built on `dirs`, with today's XDG behaviour kept bit for bit on Linux (the hand-made `XDG_CONFIG_HOME` and `XDG_CACHE_HOME` readers move there with their tests). `meloet-plugin/src/scan.rs` gets its cache folder passed in instead of reading the environment. Paths shown on the screen: `~` stays on Linux and macOS; on Windows show the path as it is.

Song files: check that nothing in a song is an absolute Linux path that must work elsewhere (SF2 files are copied into meloet's own store by hash, audio lives in the song folder; plugin keys are format + id) and that a path with `\` and a drive letter survives the song file and the protocol (paths cross to the screen as strings).

**Rejected**: keeping XDG-style folders (`~/.config/meloet`) on macOS and Windows: works, but it is not where people or backup tools look.

### 3-10. The AI link socket

**Facts**: no runtime folder there `[S61, A]`; Unix socket paths are short (104 bytes on macOS; C); named pipes are the Windows way and `interprocess` wraps both, tokio only `[S63, A]`; meloet-ai already uses tokio `[L1, A]`.

**Proposed design**

- macOS: the same code as Linux with another folder: a 0700 folder under the user's own temp or Application Support folder, as short as possible, socket 0600, peer uid check kept (the macOS call for the peer's uid differs from `SO_PEERCRED`; not researched).
- Windows: a named pipe `\\.\pipe\meloet-ai-<user sid>` whose security lets only this user connect, through tokio's own named-pipe support if it has what we need (not checked) or `interprocess`. `meloet-mcp` (blocking std code, no tokio) opens the pipe as a file.
- The same seam serves `single.rs` (one meloet per user): a lock (flock, or a named mutex on Windows) plus the same kind of channel. On macOS the OS already keeps one running copy of an app and sends "open this file" to it; Tauri's way to receive that must be read first.

**Rejected**: TCP on localhost (any user and any web page's helper could try to connect; the Unix socket's file modes are the whole access rule today). AF_UNIX on Windows (stream-only, Windows 10 1803+, and Claude-side tools do not expect it; `[S62, B]`).

**Check first**: where Claude Desktop and Claude Code on Windows and macOS start an MCP stdio server and with which environment (on Linux the missing `XDG_RUNTIME_DIR` was the surprise, `place.rs` `[L1, A]`); with a fixed pipe name or path, `meloet-mcp` needs no environment at all.

### 3-11. Packaging, signing, updates

**Facts**

- Windows bundles: NSIS (`-setup.exe`, per-user without admin by default) and MSI (needs Windows to build) `[S28, A]`.
- Unsigned on Windows: SmartScreen warns; Smart App Control on Windows 11 blocks, and the user can only turn the whole feature off `[S29, S35, A]`. Signed apps still warn until the publisher has reputation ("several weeks and hundreds of clean installs"); EV certificates no longer skip this; there is no way to ask Microsoft for reputation by hand `[S34, A]`. A new certificate starts from zero `[S34, A]`.
- Azure Artifact Signing: from 9.99 USD a month, works in CI, identity check takes 1 to 30 working days; organisations in Japan can use it, individuals only in the US and Canada `[S32, S33, S34, A]`. The price was read from the SmartScreen page, not from the pricing page (which showed no number). Price of a classic OV certificate: not confirmed.
- macOS: Developer ID + notarization need the paid program, 99 USD a year; only the Account Holder can make the Developer ID certificate; ad-hoc signing still makes users approve the app by hand `[S30, S36, A]`. The exact notarization commands are from memory (C).
- Updates: Tauri's updater handles NSIS, MSI and `.app.tar.gz`, with its own signing key that cannot be turned off `[S31, A]`.

**Proposed design**

- Windows: NSIS, per-user install, bootstrapper for WebView2 inside. `meloet-plughost.exe` and `meloet-mcp.exe` as `externalBin`. The `.meloet` file type registered by the installer. Sign every exe and the installer.
- macOS: `meloet.app` in a DMG, universal (arm64 + x86_64) if the build time allows, else Apple Silicon first. The plugin host and `meloet-mcp` inside the bundle, all signed with the same Developer ID, hardened runtime, the library-validation exception only on the plugin host, the audio-input entitlement on the main app, then notarize and staple. Change the identifier (`dev.meloet.app` warns on macOS, `docs/progress.md` 7) before the first release there, because it names the settings folder.
- Where `meloet-mcp` lives matters to users (they type its path into the AI app): on Linux `/usr/bin/meloet-mcp`; on macOS inside the bundle (`/Applications/meloet.app/Contents/MacOS/meloet-mcp`); on Windows under the per-user install folder. The 「AI とつなぐ」 help text shows the path; it must show the real one per OS (wording: section 5, decision 12).
- Updates: follow what round 12's D2 decides for Linux; the updater's key is the same on all OSes.
- `scripts/bump-version.sh`, `node licenses/check.mjs` and `scripts/check-release.sh` (no development hooks in the shipped form) must run for all three targets; the last one reads Linux binaries today.

**Rejected**: shipping unsigned on macOS for real users (every user must go into Privacy & Security `[S30, A]`; beginners will not). Cross-building the Windows installer on Linux ("a last resort" in Tauri's own words `[S28, A]`). EV certificate for SmartScreen (`[S34, A]`: no longer helps).

**Prototype first**: **P6**, the whole signing flow for a "hello" Tauri app with a second executable inside, on both OSes, before stage 4 starts, because the identity checks take weeks and the owner must start them.

### 3-12. CI

**Facts**: standard GitHub runners are free for public repositories; for private ones the Free plan has 12 USD a month, and a minute costs 0.006 (Linux 2-core), 0.010 (Windows) and 0.062 USD (macOS) `[S59, A]`. Runner labels and image changes: `[S60, B]`. meloet has no `.github/` folder today; all checks are `scripts/check-all.sh`, run by hand `[L1, A]`. Hosted runners have no sound device and no keyboard (C).

**Proposed design**

- From stage 0 on: a workflow that runs `cargo build`, `cargo clippy -- -D warnings` and `cargo test` for the product crates on `windows-latest` and `macos-latest`, plus `npm run check` and `npm test` once (on Linux). It is the only way to keep "it still compiles there" true while daily work goes on under Linux. `scripts/check-all.sh` stays the Linux check; the workflow calls cargo directly (the script is bash and names Linux tools).
- Device tests, measurements and plugin window tests run by hand on real machines, as now on Linux.
- Release builds and signing: on tags only, in stage 4.
- If the repository is private: run the macOS job only by hand and on the work branch's pull requests. At 0.062 USD a minute a 20-minute job is 1.24 USD (our arithmetic from `[S59, A]`; the 20 minutes is a guess, C), so the 12 USD is about 9 runs a month.

**Rejected**: self-hosted runners on the owner's PCs (a runner executes whatever a branch says; not worth it for one developer). Cross-compiling from Linux for checks (does not cover MSVC C++ for `meloet-stretch` or Apple frameworks).

---

## 4. Staged plan

The stage names follow the owner's request (D3 item 3). The Linux numbers that the checks point at were measured on this PC in round 8 (`docs/history/measurements/2026-11-22-round8-rt.md`, `-quiet.md`): key → sound (Rust has the key → DAC) p95 15.5 to 15.9 ms at a 256-frame block; key → screen (Rust has the key → drawn) p50 0.48 ms, p95 1.48 ms, 99 to 100 % within 2 frames; recorded position rounding ± 0.52 ms (architecture 11). A different PC and audio stack will not give the same numbers; each check below says what "good" means there.

### Prototypes (before or beside stage 0; each a spike, not product code)

| | What | Machine | Answers |
|---|---|---|---|
| P5 | The handshake: events/semaphores on Windows, `os_sync` SHARED on macOS, with the real ring layout | both | Is the design of 3-6 fast and exact enough? Does macOS need the same address? |
| P1 | cpal shared mode against `IAudioClient3` | Windows, 2 PCs | Do we write our own WASAPI output? |
| P3 | The real `ui/` in WebView2 and WKWebView | both | Does the screen hold 60 Hz and 2 frames? |
| P2 | A helper process that opens a real VST3 window | both | The plugin host's main loop and, on macOS, its bundle and signing shape |
| P4 | midir: look at the port list on a timer; key time | both | Plug in/out delay; recorded position |
| P6 | Sign and notarize a hello app with a helper inside | both | Stage 4's steps, and starts the identity checks |

P5 and P3 decide the most and should run first. If P3 fails on an OS (for example under 90 % of key → screen within 2 frames with the mixer open, and no setting that fixes it), stop and ask the owner before spending on that OS.

### Stage 0: seams, on Linux, no change of behaviour (size M, risk low)

Work (all on Linux; every step keeps `scripts/check-all.sh --devices` green and changes no measured number):

1. Move every Linux dependency under `[target.'cfg(target_os = "linux")'.dependencies]` (`alsa`, `pipewire`, `dbus`, `x11rb`, `livi`, `lv2_raw`, `rustix`, cpal's `pipewire` feature) and gate the modules that use them.
2. Put in the seams named in section 3, each with the Linux code as its first back end and a second back end that compiles and says "not here yet": the clock (3-1), the way out and in (3-1), audio thread priority (3-7), `meloet-midi` back end (3-2), `meloet-devices` back end (3-3), the IPC sleeper, the mapping, the child and the control channel (3-6), the plugin host's "wait for the next thing" and its window (3-5), the places module (3-9), the local channel for the AI link and `single.rs` (3-10), and the shutdown arms (3-8).
3. Gate LV2, `crashlog.rs`, the fork tests, `malloc_trim`.
4. Add the CI workflow of 3-12.

Acceptance:

- Linux: `scripts/check-all.sh --devices` passes; `tools/measure/latency.sh --state playing` and `record-timing.sh private` give numbers inside the run-to-run spread of the round 8 numbers; `tools/measure/plugin-stop.sh` passes (the IPC seam did not change the handshake).
- Windows and macOS: `cargo build` and `cargo clippy -- -D warnings` pass for every product crate in CI; `cargo test` passes for `meloet-model`, `meloet-protocol`, `meloet-sound`, `meloet-stretch` and the parts of `meloet-core` that touch no device. The app starts, shows the screen and says "no output" and "no keyboard" with the existing wording.

Risk: the seams are drawn before the other back ends exist, so some will be wrong. Keep them small and inside the crates (no new crate), so moving one later is cheap. Do P5 and P2 before fixing the IPC and plugin host seams.

### Stage 1: sound and keyboards, built-in sounds only (size L on Windows, M on macOS; risk high on Windows)

Work: the cpal back ends and the clock; priority; midir back end with the timer; devices back end; the places; the sound pack fetcher and SF2 on the new paths; safe writing on Windows; recording a voice (audio input) last.

Acceptance (on a real machine per OS, quiet, numbers written to `docs/measurements/` with the machine and the conditions):

- **Key → sound**: the engine's own latency records (`LatencyRecord`: `playback_us − key_us`) with 120 keys, as `latency.sh` does. Good means: p95 no more than **one block + what the OS reports as its output delay + 1 ms**, and stable from run to run (the Linux number is of this kind: one 256-frame block plus PipeWire's delay). Also write the plain number next to Linux's 15.9 ms. On Windows give it for cpal shared mode and, if built, for our own output.
- **Recorded position**: the `record-timing` method (keys at known times against the click) at two block sizes and at tempo 120 and 30: the middle error within ± 1 ms of zero and not moving with the block size. A shift that follows the block size means the `playback` time is wrong there (the kind of error 22-26 fixed on Linux).
- **The loop-cable measurement** (「遅れを測る」) gives an `X` that does not change with the block size.
- **10 minutes of recording** (`long-record.sh` in spirit): no lost blocks on a quiet machine; the audio thread guards (no allocation, no lock) pass in `meloet-engine` and `meloet-sound` tests on that OS.
- **Keyboards**: plug in and out 20 times: each is seen within 1 s, no sound or light stays after a pull with a key held, two keyboards are both heard.
- **Outputs**: pull the chosen output while playing: the existing "stopped, connecting again" flow (architecture 4-2) runs and comes back when it is plugged again. The settings show the buffer that really runs.

### Stage 2: the screen's speed (size M; risk high until P3 is done)

Work: measure, then tune per webview. No change to Linux behaviour.

Acceptance:

- **Key → screen** (`latency.sh` method, 120 spaced keys, stopped / playing / recording, then with the piano roll and with the mixer open): **at least 95 % within 2 frames** in every state, and p95 written next to Linux's 1.48 ms. (Linux on the compositing path gave 92.5 to 95 % with the mixer open and was judged not good enough; the same bar holds.)
- `MELOET_UI_SCRIPT` runs (scroll, zoom, drag): frame gaps over 25 ms no more often than on Linux's run of the same script.
- No WebGL context loss in an hour of opening and closing bands (10 regions).
- Memory of the webview's processes flat (within the spread) over 10 minutes at 60 Hz. (Not the same number as WebKitWebProcess RSS: the process shapes differ `[S24, A]`.)
- macOS: the same list once more with Low Power Mode on, written down as "what the user gets", not as pass or fail.

### Stage 3: plugins (size L; risk high)

Work: the Windows sleeper and mapping, the control channel, child control, the plugin host's main loop and window per OS, the VST3 loader per OS, scan places, the AI link's plugin tools on the new paths.

Acceptance:

- **Handshake** (the P5 measure, now in the product): round trip p99 under one tenth of a 128-frame block, and with 16 Surge XT + a compressor on a busy PC the count of blocks that missed an answer in the same range as Linux with priority (0 to 20 per run in `2026-11-22-round8-rt.md`).
- **Key → sound with a plugin instrument**: no more than the built-in number of stage 1 plus the plugin's own reported latency (the round trip is inside the block).
- `tools/measure/plugin-stop.sh` in spirit: a plugin that crashes, one that hangs, one whose process is frozen: each gives 「（名前）が止まりました…」 for that plugin only, the song keeps playing, no child is left behind when meloet ends or is killed.
- Windows 50 times open/close (`window-rounds.py` in spirit) with no leak of windows or processes; sizes right on a 150 % monitor (Windows) and a Retina screen (macOS).
- Save, close, open with plugin state: the same sound (`open-play-save.sh`); a song saved on Linux with VST3 Surge XT opens with the same patch.
- Export with plugins gives the same samples as the output (`export_app.py same`).

### Stage 4: packages and signing (size M; risk mid, with waiting time outside our hands)

Work: 3-11, 3-8, 3-10's install paths, the user documents (`README.md`, `README.ja.md`, `docs/user/`), `docs/development/` pages for each OS, `docs/install-check.md` for each OS.

Acceptance:

- A fresh Windows 11 with Smart App Control on and a fresh macOS at the minimum version: download, install, start, play the piano with the fetched sound pack, with no step that needs the user to switch off a protection. (SmartScreen may still warn until reputation builds `[S34, A]`; write what the user sees.)
- A double click on a `.meloet` file opens it, also when meloet already runs.
- The AI link works from Claude Desktop on that OS with the path the help shows.
- Power-off with unsaved changes: the recovery file is there after the restart.
- `scripts/check-release.sh` (no development hooks in the shipped form) and `node licenses/check.mjs` pass for each target; `NOTICE` and 「meloet について」 list the new parts.

---

## 5. Decisions for the owner

| # | Decision | Our advice | Why (rank of the inputs) |
|---|---|---|---|
| 1 | Which OS first? | **macOS first** through stage 3, Windows right after. If reaching more beginners sooner matters more than risk, Windows first. | macOS keeps descriptors, `posix_spawn`, Unix sockets and has a futex-like call `[S42, A]`; Windows needs the handshake, the channel and maybe the output written anew, and its signing is harder for a person in Japan `[S32, A]`. Which OS has more new MIDI-keyboard owners: we have no source (C). |
| 2 | Real machines | A Mac (Apple Silicon, macOS 14.4+) and a Windows 11 PC that helpers can build and measure on, each with the audio box and keyboard used on Linux. | Hosted CI has no sound; every acceptance number needs a real machine (C). Without them only stage 0 can be done. |
| 3 | Minimum Windows | **Windows 11, and Windows 10 22H2, x64 only**; ARM64 later. | WebView2 is in Windows 11 and on most Windows 10 `[S23, A]`; Job objects and the APIs we need are older than that `[S50, A]`. Supporting Windows 10 costs the single-client WinMM case (C). |
| 4 | Minimum macOS | **14.4**. | cpal 0.18 needs 14.2 `[S1, L2, A]`; the cross-process wait needs 14.4 `[S42, A]`; older needs a private call `[S44]`. |
| 5 | ASIO | **No.** Prototype our own `IAudioClient3` output; decide on it from P1's numbers. | GPLv3 or a signed agreement `[S12, A]`; that GPLv3 does not fit is our reading (C; a lawyer would have to say otherwise). |
| 6 | Plugin formats | **Windows: VST3. macOS: VST3 first, Audio Units (v2, in the plugin host) in a later round. LV2 stays Linux only. CLAP stays out** (architecture 20-9). | VST3 SDK is MIT `[S13, S15, A]`; AU needs bindings nobody checked; LV2 is rare there (C). A song with LV2 plugins opens with the notice of missing things and keeps their fields. |
| 7 | Money for signing | **Apple: yes, 99 USD a year, as an individual** (start early; only the Account Holder can make the certificate). **Windows: Azure Artifact Signing if the owner has an organisation in a listed country; if not, decide between** (a) testers only, unsigned, with the warning written in the README, (b) a classic OV certificate (price not confirmed), (c) the Microsoft Store (not researched). | `[S36, S30, S32, S34, A]`. We do not know the owner's country or legal form. |
| 8 | How to read "PC-wide settings are never changed" there | **Asking the OS for a small buffer for our stream is allowed** (it is the same kind of wish as `node.latency`); exclusive mode and writing system settings are not. Show the buffer that really runs. | The wish changes what other apps on that output get while meloet plays `[S49, S5, A]`. Whether everything returns when meloet closes is from memory (C); check in P1. |
| 9 | Is up to 1 s between plugging a keyboard and seeing it acceptable in the first port? | **Yes** for the first port (a 500 ms timer); real notices later. | midir has none `[S8, A]`. On Linux it is immediate. |
| 10 | Is a Windows release with about 10 ms more key → sound than it could have acceptable, if our own output slips? | **No for a public release, yes for testers.** Key → sound is the product's first promise. | C (from `[S49, A]` and `[L2, A]`); P1 gives the real gap. |
| 11 | Repository public or private (CI cost) | If public: CI on every push. If private: macOS jobs by hand and on pull requests only. | `[S59, A]`. We did not check which it is. |
| 12 | New wording | The owner writes it; we list the places when they are known. Seen so far: the microphone permission refused (macOS); WebView2 missing and no internet (Windows installer, Tauri's own text); another app holds the keyboard (Windows 10); Low Power Mode; the path of `meloet-mcp` in the AI help; the top menu bar on a Mac (spec 3, Y11); shown paths without `~` on Windows. | Wording is the spec's (`ui/src/text/ja.ts`); none of these is in it. |
| 13 | Is the plugin window allowed to stay a separate window on those OSes? | **Yes** (it is the spec's shape today). | Embedding across processes is the costliest single item we avoided `[S52, B]`. |
| 14 | Universal macOS build (Intel + Apple Silicon)? | Apple Silicon first; Intel when someone asks. | Intel-only plugins on Apple Silicon and Rosetta were not researched (C). |

---

## 6. Licence check of new dependencies

Rule: MIT or Apache-2.0 (0BSD and the OS's own APIs are fine; no GPL or AGPL; MPL-2.0 only unchanged, and better not at all). **Every row must still pass `node licenses/check.mjs` on the real dependency tree when it is added: a crate's own licence says nothing about what it pulls in**, and nobody has resolved these trees for Windows or macOS yet.

| Part | Use | Licence | Rank | Verdict |
|---|---|---|---|---|
| midir 0.11.1 | keyboards on Windows and macOS | MIT | `[S6, L3, A]` | OK. Its Windows and CoreMIDI dependencies (`windows`, `coremidi` 0.9.x MIT `[S6, A]`) to be checked by the tool. |
| cpal 0.18.2 (already in) | WASAPI and CoreAudio hosts | Apache-2.0 | `[S6, L2, A]` | OK. Drop the `pipewire` feature outside Linux. The crates its hosts pull in on Windows and macOS: not checked. |
| `windows` 0.62 | Win32 calls: WASAPI, MMCSS, events, mapping, jobs, pipes, windows | MIT OR Apache-2.0 | `[S6, A]` | OK. (One version in the tree: cpal and midir bring their own; watch for two copies.) |
| `objc2` 0.6.5 | AppKit windows and run loop in the plugin host | MIT | `[S6, A]` | OK. Its framework crates (`objc2-app-kit`, `objc2-foundation`, `objc2-audio-toolbox` …): **not confirmed**, check before use. |
| `mach2` or plain `extern` declarations | `mach_absolute_time`, `thread_policy_set`, `os_sync_*`, `os_workgroup_*` | system API; `mach2` licence not confirmed | | Prefer our own few `extern "C"` lines in one file (as `futex.rs` does with `libc`). |
| `libc` (already in) | `shm_open`, `kill`, `statvfs` on macOS | MIT OR Apache-2.0 (already passed) | `[L1, A]` | OK. |
| VST3 interfaces (`vst3` 0.3.0, already in) | HWND and NSView paths | crate MIT OR Apache-2.0 `[L4, A]`; SDK MIT since 3.8 `[S13, S15, A]` | A | OK. The VST name and logo follow Steinberg's trademark rules if used `[S14, A]`. |
| `interprocess` 2.4.9 | maybe the AI link's channel on Windows | 0BSD OR Apache-2.0 | `[S6, S63, A]` | OK under Apache-2.0, if used at all (3-6, 3-10). |
| `memmap2` 0.9.12 | not needed (we map ourselves) | MIT OR Apache-2.0 | `[S6, A]` | Not added. |
| Tauri plugins (dialog, updater, single-instance) | file windows, updates, one meloet | not read this round (Tauri itself is MIT OR Apache-2.0, already passed) | C | Check each before use. |
| Windows MIDI Services SDK | later, hot-plug and timestamps on Windows 11 | MIT | `[S11, A]` | OK in licence; no Rust binding found. |
| Audio Units | later | Apple system frameworks | | No licence to carry; bindings not confirmed. |
| **ASIO SDK** | | GPLv3 or proprietary | `[S12, A]` | **Rejected.** |
| **`audio_thread_priority`** | | MPL-2.0 | `[L5, S6, A]` | **Not added** (we write the few calls ourselves; keep cpal's `realtime` feature off). |
| **`shared_memory`** | | MIT OR Apache-2.0, last release 2022 | `[S6, A]` | **Not added** (unmaintained, C). |
| lilv / `livi` on Windows and macOS | | `livi` MIT `[L1, A]`; lilv there not researched | | Not built there (LV2 stays Linux only). |
| Signalsmith Stretch (already in, vendored) | C++17 with MSVC and clang | already passed | | No licence change; the build is untried there. |
| Fonts (IBM Plex, OFL-1.1) and sound material | | no change | | The same files ship on every OS. |

---

## 7. Confidence and sources

**Overall rank: B.**

How it is worked out, by the project's rule (the overall rank is the highest rank among the sources that directly support the conclusion; our own reasoning takes the rank of its inputs):

- "The port can be done, and these are the Linux-only parts" rests on reading meloet's code `[L1]` and the installed crates `[L2]`–`[L5]`, and on official API and licence pages `[S4, S8, S12–S18, S42, S47–S51 …]`: **A**.
- "No licence blocks it" rests on licence texts and crates.io: **A** for the parts named, with the dependency trees still unchecked (section 6).
- The designs, the sizes, the order of stages and the advice in section 5 are our reasoning: **C** as proposals, even where every input is A. Several rest on memory and are marked so: Windows wait limits and high-resolution timers, events and semaphores across processes, rename over an open file on Windows, macOS `CLOCK_MONOTONIC` and sleep, `ftruncate` on POSIX shm, the workgroup property, AppKit's main-thread rule.
- The screen on WKWebView rests on **B** sources `[S27]`; the screen on WebView2 has **no source** for speed.
- No speed number in this document was measured on Windows or macOS. Every Linux number quoted is from meloet's own measurements on one PC.

So: A for what must be replaced and that replacements exist; C for how long it takes and how fast it will be. The document is rated B as a whole so that nobody reads the plan's sizes as facts; the prototypes of section 4 are what turns the C parts into A.

**Things the research notes left open, in one list** (each is also named where it matters): the default WebGL context limit in Chromium's source; WebView2 frame pacing and input latency; whether `os_sync` SHARED needs the same virtual address; how a workgroup crosses to another process; CoreMIDI notice types and the `coremidi` wrapper; a Rust binding for Windows MIDI Services; an equal of `PR_SET_PDEATHSIG` on macOS; whether power-off reaches `applicationShouldTerminate`; what Tauri 2 passes on of end-of-session and "open this file"; a helper executable with windows and no bundle on macOS; AU bindings; VST3 state across OSes; the proprietary ASIO agreement's text; the price of an OV certificate; the Microsoft Store route; whether hosted Windows runners have WebView2; the licences of `objc2-*` framework crates and of the Tauri plugins; lilv outside Linux.

**Sources**: the list `[S1]`–`[S66]` with URLs, dates and ranks is section 11 of [`2026-11-23-windows-macos-port-sources.md`](2026-11-23-windows-macos-port-sources.md). Local sources (all A, read 2026-11-23):

| ID | Source |
|---|---|
| L1 | meloet working tree, branch `fea/round12`: the files named in section 2, `docs/architecture.md` chapters 2, 5, 6, 11, 12, 20, 22-5, 23-10, `docs/progress.md` section 7, `Cargo.toml` of the workspace and of every crate, `crates/meloet-app/tauri.conf.json`, `scripts/build-deb.sh` |
| L2 | cpal 0.18.2, `~/.cargo/registry/src/*/cpal-0.18.2/`: `README.md` (minimum OS table), `Cargo.toml`, `src/host/wasapi/device.rs` and `stream.rs` (share mode, flags, `output_timestamp`), `src/host/coreaudio/mod.rs` and `macos/device.rs` (host time, `playback`) |
| L3 | midir 0.11.1, same place: `Cargo.toml`, `src/backend/winmm/handler.rs` (timestamp), `src/backend/coremidi/mod.rs` (timestamp) |
| L4 | vst3 0.3.0, same place: `Cargo.toml` (licence), `cfg(target_os = "windows")` in the source |
| L5 | audio_thread_priority 0.35.1, same place: `Cargo.toml` (MPL-2.0) |

Where this document and the notes differ:

- The notes say meloet "may use" cpal's PipeWire host later; meloet already uses it (`cpal` with the `pipewire` feature, `HostId::PipeWire` in `output.rs` and `input.rs`) `[L1, A]`.
- The notes read the WASAPI facts in cpal's master (0.19.0) and ask for a check against 0.18.2; `[L2]` confirms them for 0.18.2.
- The notes leave the WinMM timestamp open; `[L3]` shows it is the driver's millisecond count.

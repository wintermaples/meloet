# Windows and macOS port: research notes

Date: 2026-11-23. All pages below were read on 2026-11-23 unless a different date is given.
Purpose: facts for the design document in round 12, section D3. No code was written or built.

Confidence ranks: A = primary source (official docs, source code, licence text, official announcement). B = secondary source that cites a primary one, or a reliable organisation. C = unknown source, forum post, own inference, or memory. Facts about versions, prices and laws that are older than 2 years are lowered by one level (marked "older than 2 years").
`[Sxx]` points to the source list at the end.
"Not confirmed" means I looked and did not find it, or I could only find C.

Read this first (the short list of what changes the plan):

1. meloet already gives every plugin its own top-level window in the plugin host process (`crates/meloet-plughost/src/window.rs`; `docs/architecture.md` line 890). So the plan does NOT need cross-process window embedding into the main window. On Windows and macOS the plugin host makes its own top window (HWND or NSWindow/NSView) and the plugin view is embedded inside the same process. This removes the biggest risk named in D3 item 2 [local code, A]. See section 4.
2. cpal on Windows uses WASAPI shared mode only, with no `IAudioClient3`, no exclusive mode and no MMCSS call (cpal master source) [S4, A]. Realistic latency with cpal is the default device period (about 10 ms) unless meloet uses its own WASAPI code. Section 1.
3. ASIO SDK is GPLv3 or a proprietary agreement [S12, A]. The GPLv3 side does not fit an MIT/Apache app (own reading, C). Section 1.
4. `WaitOnAddress` works only inside one process [S47, A]. The audio-block handshake between the engine and the plugin host (futex now) needs a new design on Windows (named events or semaphores, not measured). On macOS 14.4+ `os_sync_wait_on_address` has a SHARED flag [S42, A]. Section 5.
5. Windows MIDI hot-plug: midir has none; it is polling only [S8, A]. Same for CoreMIDI in midir. Section 2.
6. macOS needs the Apple Developer Program (99 USD per year) for Developer ID and notarization [S36, S30, A]. Windows signing: Azure Artifact Signing from 9.99 USD per month, but individual developers must be in the US or Canada [S32, S34, A]. Section 8.
7. VST3 SDK is MIT since 3.8 [S13, S15, A]. Good for us. Audio Units (macOS only) and the Windows/macOS plugin world are mostly VST3 and AU; LV2 is rare there (C).

---

## 0. Where the current Linux code depends on the OS (checked in the repo)

I counted the OS calls in `crates/` with grep (local code, A, working tree `fea/round12`, 2026-11-23). This list is a start for D3 item 1; the lead should check the full list.

- `libc::memfd_create` (shared memory for the plugin host, `meloet-plugin/src/ipc`), `SYS_futex` (wake-up between engine and host), `SYS_pidfd_open` and `pidfd_send_signal` (watch and stop a child), `prctl(PR_SET_PDEATHSIG)` (child dies with parent), `prctl(PR_SET_DUMPABLE)`.
- `sched_setscheduler(SCHED_FIFO)` and `SCHED_RESET_ON_FORK` (audio threads), `mlock`.
- `malloc_trim`, `CLOCK_MONOTONIC`, `statvfs`.
- 33 lines with `cfg(target_os ...)` or `cfg(unix)`.
- The ring in `meloet-plugin/src/ipc/mod.rs` wakes the plugin host audio thread once per audio block and waits for the answer on a second futex word. That is two wake-ups per block per plugin slot. Any replacement must be fast for this pattern.

---

## 1. Audio output and input

### 1.1 cpal (the audio library meloet uses)

Facts:
- cpal 0.18.2 is the newest release on crates.io (published 2026-08-16), licence Apache-2.0, MSRV 1.85 [S6, S2, A]. The git master branch already says version 0.19.0 [S65, A]. The numbers below come from the changelog and master source; check them against 0.18.2 before use.
- README: Windows default is WASAPI, ASIO and JACK optional; macOS default is CoreAudio, JACK optional; macOS minimum is "macOS 14.2 (loopback recording requires 14.6+)" [S1, A]. Conflict: Tauri says macOS 10.15 is its minimum [S21, A]. The higher one (cpal, 14.2) binds a product that uses cpal 0.18.
- cpal now also has PipeWire and PulseAudio hosts (feature flags `pipewire`, `pulseaudio` in `Cargo.toml`; `src/host/pipewire/`, `src/host/pulseaudio/`) [S65, A]. Not a port question, but meloet-devices may use it on Linux later.
- Device IDs: since 0.17 there is `DeviceTrait::id` and `HostTrait::device_by_id`; since 0.18 a device must have a stable ID plus `Eq`, `Hash`, `Display` [S2, A].
- Buffer size: `BufferSize::Default` (system default) or `BufferSize::Fixed(n)`; `SupportedBufferSize` gives a min and max range [S1, S3, A]. The README warns that a very deep buffer makes audio run fast [S1, A].
- cpal has no public API for "device list changed" (the docs list none; errors come through the stream error callback) [S3, A; docs excerpt only, so not fully confirmed]. Stream error kinds include `DeviceChanged`, `DeviceNotAvailable`, `StreamInvalidated`, `BufferUnderrun` [S2, A].

Windows (WASAPI) facts, from cpal master source `src/host/wasapi/` [S4, A]:
- Only shared mode: `let share_mode = Audio::AUDCLNT_SHAREMODE_SHARED;` in both input and output stream builders. No exclusive-mode path. (The word "exclusive" appears only in comments and an error mapping.) This confirms the D3 table.
- `IAudioClient3`, `InitializeSharedAudioStream`, `AvSetMmThreadCharacteristics`/Avrt: not found in the wasapi files (grep of master, 0.19.0).
- Stream uses event callback mode (`AUDCLNT_STREAMFLAGS_EVENTCALLBACK`) and waits with `WaitForMultipleObjectsEx`. Output uses `AUTOCONVERTPCM`, so any sample rate and format is accepted and Windows converts.
- Buffer range: `IAudioClient2::GetBufferSizeLimits` if it works; otherwise it reports min = max = the device period (`GetDevicePeriod`). So on a normal shared stack the buffer is not really adjustable.
- Realtime priority: only with the cpal `realtime` feature. It calls the `audio_thread_priority` crate and falls back to `THREAD_PRIORITY_TIME_CRITICAL` [S4, A]. `audio_thread_priority` 0.38.0 is MPL-2.0 [S6, A] and on Windows it joins the MMCSS task named "Audio" (not "Pro Audio") [S64, A]. CHANGELOG 0.16.0: "WASAPI: Set realtime priority for stream threads" [S2, A].
- Device change: cpal registers an `IMMNotificationClient` only for default-device changes; add/remove events are ignored unless the default is lost [S4, A]. 0.18.0 added automatic re-route when the default device changes [S2, A].
- Device names: `FriendlyName` is preferred over `DeviceDesc` since 0.18.0 [S2, A].

Windows system facts:
- Windows 10 and later: the audio engine adds about 1.3 ms; before Windows 10 about 12 ms (float) [S49, A; page dated 2024-12 and updated 2025-03, within 2 years].
- Default shared-mode buffer is 10 ms. With `IAudioClient3` an app can ask for a smaller period; the inbox HD Audio driver supports 128 frames (2.66 ms at 48 kHz) up to 480 frames (10 ms). The driver must support small buffers, and when one app asks for a small buffer, all apps on that endpoint switch to it [S49, A].
- Exclusive mode and ASIO give low latency but block other apps on the endpoint [S49, A].
- Conclusion (own reasoning from the facts above, C): cpal gives about 10 ms buffering in shared mode. To get 3 ms the project would need its own thin WASAPI backend using `IAudioClient3` (the `windows` crate 0.62 is MIT OR Apache-2.0 [S6, A]) or a patch to cpal. This is a prototype item.

ASIO:
- cpal feature `asio` is Windows only and needs LLVM/Clang for bindgen; the build script downloads the SDK from steinberg.net [S1, S65, A].
- Licence: Steinberg moved ASIO to a dual licence on 2025-10-15: proprietary and GPLv3 [S12, A]. A second announcement (2025-10-29) says the ASIO SDK is available under GPLv3 "allowing both proprietary and open-source licensing options to be offered side by side" [S13, A]. The proprietary route needs a signed SDK agreement; forum posts from 2026 say Steinberg answers slowly [S20, C]. A GPLv3 SDK linked into an MIT/Apache binary would make the whole binary GPLv3 (own reading, C; ask a lawyer before shipping). The D3 table said "GPL reading is C"; that still holds.
- Not confirmed: the exact text of the proprietary agreement (whether it allows hosting ASIO drivers in a free MIT app) — no primary source was found for the host side.

macOS (CoreAudio) facts [S5, S2, A]:
- Fixed buffer size is set with `kAudioDevicePropertyBufferFrameSize` (scope Global, element Output) after checking against `kAudioDevicePropertyBufferFrameSizeRange`. The source comment says: "Buffer frame size is a DEVICE-LEVEL property, not stream-specific." So changing it affects every app using the device (own reading of the comment: C).
- 0.18.0: error callback gets `StreamInvalidated` on any sample-rate change; default-output streams that lose their device re-route or report `DeviceNotAvailable`; timestamps include device latency and safety offset [S2, A].
- Device-list notifications: cpal has a generic `AudioObjectAddPropertyListener` wrapper but I could not confirm that cpal exposes "device added/removed" to users [S5, A].
- Microphone/audio input needs the `com.apple.security.device.audio-input` entitlement with the hardened runtime, and `NSMicrophoneUsageDescription` in `Info.plist` [S39, A for entitlement; B for the plist key: Apple forum answers]. Playing sound needs no permission. cpal 0.17.0 fixed "unnecessary microphone permission requests" [S2, A].
- Realistic latency: not measured here. No primary number found for CoreAudio built-in output buffer sizes. Do not quote a number; measure in a prototype.

How device names and hot-plug are obtained (replacement for `meloet-devices`, PipeWire Routes):
- Windows: cpal gives names (FriendlyName). Full device add/remove needs `IMMNotificationClient` (cpal uses it internally for the default device only) — meloet would need its own listener (C, own inference; the COM interface is public API, not read here).
- macOS: cpal gives names and IDs. Add/remove needs a CoreAudio property listener on the system object (inference, C; not read from Apple docs).
- "Show the buffer that is really running" (a project rule): Windows shared mode — the engine period can be read with `IAudioClient3::GetCurrentSharedModeEnginePeriod` [S49, A]. macOS — the device buffer frame size property [S5, A]. Both are readable, but neither is PipeWire's quantum and neither is a PC-wide number one can leave alone: asking for a small period changes it for other apps [S49, A].

Open questions:
- Does the owner accept 10 ms shared-mode latency on Windows in the first release? Or must a own WASAPI backend be written (stage 1 cost)?
- Is ASIO needed at all (proprietary agreement route)?
- Is per-device hot-plug needed for audio devices, or only for MIDI keyboards (the current rule is about keyboards)?

Risks:
- cpal 0.18 changed many APIs (device ID rules, error kinds). meloet-engine uses cpal today; a version check is needed.
- On macOS, changing the device buffer size is system-wide for that device.

---

## 2. MIDI

midir facts:
- midir 0.11.1 (published 2026-10-10), licence MIT [S6, A].
- Backends: ALSA (Linux), WinMM (Windows), CoreMIDI (macOS, iOS), WinRT (Windows 8+, feature `winrt`), JACK (feature `jack`) [S7, A]. Virtual ports work except on Windows [S7, A].
- Port listing is on demand (no cache). WinMM: `midiInGetNumDevs`, then `midiInGetDevCapsW` per index; a port is matched again by name and interface ID when connecting. There is no hot-plug callback; clients must poll `port_count()`. A removed device gives `InvalidPort` [S8, A].
- CoreMIDI backend: ports from `Sources`/`Destinations`; `id()` is the CoreMIDI `unique_id`; `Client::new` is created without a notification callback, so midir does not report hot-plug either [S8, A]. Timestamps: host time converted to nanoseconds, then divided by 1000, so microseconds; if the packet timestamp is 0, the current host time is used [S8, A].
- WinRT backend: `DeviceInformation::FindAllAsyncAqsFilter` per call, no `DeviceWatcher` [S8, A]. Timestamp is `Duration` divided by 10 (so microseconds from 100 ns units) [S8, A]. The WinMM timestamp source: not confirmed (the file was not in the part I read).
- coremidi crate 0.9.2, MIT [S6, A]. Whether it offers a notification wrapper: not confirmed.

CoreMIDI hot-plug (Apple): `MIDIClientCreate` takes a notify block (`MIDINotifyBlock`): "The system calls this block when some aspect of the current MIDI setup changes… on an arbitrary thread" [S56, A]. The list of message types (object added/removed) was not read from the page; not confirmed.

Windows MIDI Services (the new Windows MIDI stack):
- Microsoft announced general availability in February 2026; the core is built into Windows 11 and is rolling out in phases to in-support retail releases of Windows 11 [S10, A]. The project page says "Windows 11 25H2 and later" [S9, A]. Conflict between the two pages: same-direction (both Windows 11 only); the blog is later and says "in-support retail releases", so 25H2 may be the safe floor. Windows 10 is not covered by either page (not confirmed; no support statement found).
- Old WinMM and WinRT MIDI 1.0 APIs "now point to the new service" and every port is multi-client; "existing MIDI 1.0 software just got even better, without needing any app updates" [S9, S10, A].
- The new SDK has `MidiEndpointDeviceWatcher` for hot-plug [S9, A]. The SDK is in preview; the App SDK Runtime and Tools were announced for "the coming months" (as of 2026-02-17) [S10, A]. Source on GitHub, MIT licence [S11, A]. Rust projection: not mentioned on any page I read (not confirmed).
- Timestamps and scheduled messages need the new SDK [S10, A].
- meloet rule "a recorded key is put at the position that was reaching the ear at the moment it was pressed" needs a good event timestamp. WinMM timestamp precision: not confirmed. CoreMIDI timestamps are host time (A above). This is a prototype item (`record-timing`).

Open questions:
- Hot-plug design: polling in `meloet-midi` (for example every 1 s) is the only option with midir on both OSes (C, own inference from the A facts above). Is a 1 s delay OK?
- Use midir (WinMM) first and move to Windows MIDI Services later?

Risks:
- WinMM on Windows 10 without the new stack: one app can open a port at a time (C, general knowledge; the "multi-client" statement above is only about the new stack).
- SysEx and other exotic uses are untested.

---

## 3. Webview

Tauri facts:
- Tauri 2 uses WebView2 on Windows ("based on Microsoft Edge and therefore Chromium"; updates itself), WKWebView on macOS and iOS ("webview that comes preinstalled with macOS"; WebKit updates only come with OS updates, so unsupported macOS versions get no WebKit updates), WebKitGTK on Linux [S22, A].
- Tauri's prerequisites page: Windows 7 and later (WebView2 on Windows 10 1803 and newer is already installed; otherwise use the Evergreen Bootstrapper); macOS 10.15 and later [S21, A]. Conflict with the spec (D3 table says WebView2 comes with Windows 10 from 1803): Microsoft says "the vast majority of Windows 10 devices have the WebView2 Runtime installed already… a small number don't" and the Evergreen runtime "will be included as part of the Windows 11 operating system" [S23, A]. Use Microsoft's wording: install check needed.
- WebView2 install modes in Tauri (`webviewInstallMode`): `downloadBootstrapper` (default, adds 0 MB), `embedBootstrapper` (about 1.8 MB), `offlineInstaller` (about 127 MB), fixed runtime (about 180 MB), `skip` [S28, A]. Microsoft says Fixed Version binaries are "over 250 MB" [S23, A] (conflict in size; Microsoft's figure is for the current download; Tauri's table is older text, so take the larger one).
- WebView2 uses the GPU by default, and each WebView2 control starts its own browser, renderer and GPU processes [S24, A]. WebView2 "performance characteristics are like running a site in Edge" [S24, A].

WebGL context limits:
- WebKit (WKWebView, WebKitGTK use the same WebCore code): `static constexpr size_t maxActiveContexts = 16;` and `maxActiveWorkerContexts = 4`; when the limit is reached "the oldest context will be lost" with a console error [S25, A, read in `WebGLRenderingContextBase.cpp` on `main`]. This matches the 16 in `CLAUDE.md`.
- Chromium (WebView2): the oldest context is lost when the limit is exceeded; WebKit's own change log says "other browsers have a limit of 16" [S26, B]. A Chromium developer thread (2021) says a start flag `max-active-webgl-contexts` exists and that the team has no plan to raise the default [S26, B]. The Blink source reads the limit from the GPU context provider (`max_active_webgl_contexts_` in `webgl_rendering_context_base.cc`) but I could not find the default number in the source [S26, A for the code, number not confirmed]. Whether a WebView2 app can pass that flag: not confirmed.
- Result: budget 12 of 16 is still a safe rule on all three engines (own reasoning, C).

Frame pacing and input latency:
- WKWebView: Tauri plugin notes say rAF is capped to 60 fps on macOS 13–15 regardless of display rate, and the cause is the WebKit preference `PreferPageRenderingUpdatesNear60FPSEnabled`; the Tauri issue (#13978) was closed "not planned" [S27, B]. A 60 Hz target (the project's target) is therefore fine.
- WebKit throttles rAF to 30 fps in Low Power Mode, on purpose (WebKit bugzilla) [S27, B]. A laptop on battery in Low Power Mode may drop to 30 fps (own conclusion from the source, C).
- A user report says Tauri on WKWebView looks jittery on an M2 Pro even with a 60 fps counter [S27, C, anecdote].
- WebView2 frame pacing and input latency: no primary or secondary source found. Not confirmed. Needs a measurement with `tools/measure/` ideas (key to screen).
- The `WEBKIT_DISABLE_COMPOSITING_MODE` and `JSC_*` tuning is for WebKitGTK only. WKWebView and WebView2 have their own compositors and GC; none of the Linux-specific numbers (dirty rectangles, Full GC every time) carry over (own conclusion; C). The "webkit-damage" tool needs gdb and WebKitGTK debug info, so it is Linux-only.
- Memory: on Chromium (WebView2) the process model has separate browser, renderer and GPU processes [S24, A], so RSS numbers are not comparable with the WebKitWebProcess numbers in the old measurements.

Open questions:
- Does the WebGL region approach (one context per region, up to 12) work in WebView2 without context loss? Needs a prototype.
- Which minimum WebView2 version does Tauri 2 / the current UI need? Not confirmed (no number found).

Risks:
- WKWebView on older macOS gets no WebKit update (A above); the app may meet old WebKit bugs on macOS 14 or older.
- Tauri docs say the Windows installer's WebView2 download step needs internet by default (A, from the install mode table).

---

## 4. Plugin hosting (VST3, Audio Units, LV2)

VST3 licence and format facts:
- VST 3.8 (released 2025-10-29) is under the MIT licence [S13, A]. The SDK README says "Welcome to VST SDK 3.8.x" and the dev portal says "Since version 3.8 VST 3 is licensed under MIT license"; using the VST name or logo is optional and must follow Steinberg trademark rules if used [S14, A]. `vst3_pluginterfaces` LICENSE.txt: "MIT License, Copyright (c) 2026, Steinberg Media Technologies GmbH" [S15, A]. The press release says the 3.8.0 SDK is available at the developer portal "under the existing licensing terms" [S13, A] (unclear what exactly that phrase means; the licence files say MIT).
- GPLv3 and the proprietary licence "are no longer available" for the SDK [S14/S19, B].
- Windows bundle: folder `MyPlugin.vst3/Contents/x86_64-win/MyPlugin.vst3` (also `x86`, Arm64EC, Arm64X); a single `.vst3` DLL is deprecated since VST 3.6.10; optional `Contents/Resources/moduleinfo.json` [S17, A].
- macOS bundle: `MyPlugin.vst3/Contents/MacOS/` (universal Mach-O), `Contents/Info.plist`, optional `moduleinfo.json` [S17, A].
- Locations. Windows: `%LOCALAPPDATA%/Programs/Common/VST3/`, `/Program Files/Common Files/VST3/`, and `/Program Files (x86)/Common Files/VST3/` for 32-bit. macOS: `~/Library/Audio/Plug-ins/VST3/`, `/Library/Audio/Plug-ins/VST3/`, `/Network/Library/Audio/Plug-ins/VST3/`. The first plugin found per Processor UID wins [S18, A].
- Window embedding (`IPlugView::attached`): `kPlatformTypeHWND = "HWND"` (Windows), `kPlatformTypeNSView = "NSView"` (macOS), `kPlatformTypeHIView = "HIView"` (old macOS), `kPlatformTypeX11EmbedWindowID` (Linux) [S16, A]. Size units: on macOS "logical units (independent of the screen scale factor)", on Windows and Linux physical pixels [S16, A]. This matters for scaling code in `window.rs`.

How meloet shows plugin windows today (local code, A): `meloet-plughost` makes its own top-level X11 window and calls `attached` with the X11 window ID, all inside the plugin host process. The main meloet window is not the parent [local: `crates/meloet-plughost/src/window.rs` header; `docs/architecture.md:890`]. D3 item 2 ("embed a plugin window of another process into the main window") therefore does not apply to the current design. The port needs:
- Windows: a plugin host top-level window with its own message loop (HWND); DPI awareness per monitor (own inference, C: VST3 gives physical pixels on Windows).
- macOS: the plugin host process needs an `NSApplication` main loop, an `NSWindow`, and an `NSView` for `attached`. AppKit windows must be created on the main thread (general rule, C; not read from Apple docs here). Whether an executable without an app bundle can show windows reliably (activation policy) is not confirmed; it needs a prototype.

If the owner later wants the plugin window inside the main window (not in the current spec): cross-process embedding facts below.
- Windows: cross-process parent/child windows are "technically legal, but the consequences can be very difficult to manage"; input queues of the two threads become implicitly attached [S52, B (Microsoft engineer blog, 2013, older than 2 years: stays B)]. A Microsoft Q&A answer says cross-process `SetParent` "was not officially supported" and that DWM may draw the window as a separate top-level one on Windows 11; UIPI blocks `SetParent` silently if the child is elevated and the parent is not [S53, C: Q&A moderator answer, not documented].
- Linux comparison: yabridge embeds a Wine window into a host window by XEmbed-like reparenting and acts as a minimal X11 window manager to fix events, focus is a heuristic [S54, A].
- macOS: no primary source found for embedding an NSView from another process. A 2012 Apple mailing list suggestion was a borderless always-on-top window kept in sync with the host window [S66, C]. Bitwig's user guide describes five plug-in hosting modes (Within Bitwig, Together, By manufacturer, By plug-in, Individually) but says nothing about plugin windows [S55, A]. Not confirmed how Bitwig or REAPER show windows of out-of-process plugins on macOS.

Audio Units (macOS only):
- Apple: "AUv2 audio units are loaded into the host's process, whereas AUv3 audio units are loaded out-of-process by default. In macOS only, if supported by the AUv3 audio unit, you can request loading it in-process by passing the `loadInProcess` option" and check `isLoadedInProcess` [S40, A].
- Finding and starting: `AVAudioUnitComponentManager` (find), `AVAudioUnit.instantiate(with:options:completionHandler:)` (start), `requestViewController(completionHandler:)` (UI; callback is on a background thread, so the view must be added from the main queue) [S40, A]. AUv3 API can also host AUv2 through a bridging layer [S40, A].
- Known problems for out-of-process hosts (Apple forum user reports, C): an out-of-process AUv3 host cannot see other AUv3 units; the render observer block is not called; some views render blank.
- Rust bindings for AudioToolbox/AVFoundation (`objc2` family): `objc2` 0.6.5 is MIT [S6, A]; the specific crates for AudioToolbox (`objc2-audio-toolbox`) and their coverage: not confirmed.
- The meloet plugin host design (one separate process per plugin, shared memory ring with futex, own audio thread) fits AUv2 (in-process in plughost). With AUv3 out-of-process the system adds a second process and XPC between host and plugin (own conclusion from Apple's text, C); real-time cost not confirmed.
- Architecture on Apple Silicon: an Apple forum thread title "Hosting x86 Audio Units on Silicon Mac" exists; content not read. Own inference (C): a separate plugin host process could be built as x86_64 and run under Rosetta to load Intel-only plugins. Not confirmed.

Hardened runtime and plugins (macOS):
- Apple: "Apple only notarizes macOS apps that enable the Hardened Runtime." The exception "Disable Library Validation" is for "Load frameworks and plug-ins that are written by third-party developers." "Audio Input" is the resource entitlement for "Core Audio APIs" input. Apple warns to use the narrowest set of exceptions [S39, A].
- Result: meloet's plugin host (a separate executable) would need `com.apple.security.cs.disable-library-validation` to load third-party VST3 and AU bundles. The main app may not need it. Own reading, C.

LV2:
- On Windows and macOS LV2 is rare (C, from the spec; I found no primary statistic). I did not research lilv licence or Windows/macOS builds. Not confirmed. Suggest: keep LV2 Linux-only in the first port (design choice for the lead).

CLAP: out of scope.

Open questions:
- Hosting only VST3 on Windows, VST3 + AU on macOS?
- Own plugin windows (current) are fine; confirm the owner does not want embedded windows.
- How is `meloet-plughost` started on macOS so it can show windows (helper inside the app bundle)?

Risks:
- 32-bit plugins on Windows: not covered; skip.
- A plugin that blocks the main thread of the host process hangs its own window only (the current design isolates this).

---

## 5. Inter-process communication, threads, priority

### 5.1 Shared memory

- Windows: `CreateFileMapping` with `INVALID_HANDLE_VALUE` and a name gives paging-file-backed shared memory; the second process uses `OpenFileMapping` and `MapViewOfFile`; the `Global\` prefix works across sessions but needs `SeCreateGlobalPrivilege` [S51, A]. Handle inheritance or `DuplicateHandle` is the unnamed equivalent of passing a memfd (own inference from the standard API, C; not read in this round).
- macOS: POSIX `shm_open` is available; the xnu source defines `#define PSHMNAMLEN 31 /* maximum name segment length we bother with */` [S45, A]. So a name like `/meloet-<pid>-<n>` must stay short. macOS has no `memfd_create` (C, general knowledge). An unlinked `shm_open` fd can be passed to a child through inheritance or a socket (AF_UNIX fd passing is a general Unix feature; not read from a macOS doc here, C).
- `memmap2` 0.9.12 is MIT OR Apache-2.0 [S6, A]. `shared_memory` crate 0.12.4: last update 2022-03-01 (old; MIT OR Apache-2.0) [S6, A] — looks unmaintained (own reading, C).

### 5.2 Wake-up (futex replacement)

- Windows: `WaitOnAddress`: "does not return until another thread **in the same process** signals…" [S47, A]. Minimum Windows 8. It cannot wake a thread in another process. (This confirms the D3 table.)
- Alternatives on Windows (own reasoning, C; no source read this round): named or inherited auto-reset event objects, or semaphores (kernel objects work across processes). Cost per wake: one kernel call each way. For two wake-ups per block per plugin slot, the number of calls per second is 2 x (48000 / frames per block) x slots; for example 2 x 375 = 750 per second per slot at 128 frames, small, but latency jitter of the kernel wake-up is not measured. Needs a prototype.
- macOS 14.4 and newer: `os_sync_wait_on_address` and the wake functions have a flag `OS_SYNC_WAIT_ON_ADDRESS_SHARED` (and `OS_SYNC_WAKE_BY_ADDRESS_SHARED`) "used when synchronizing among multiple processes by placing the addr in a shared memory region"; the wake must use the same flag and the exact same address; availability `macos(14.4)` [S42, A]. There are `_with_deadline` and `_with_timeout` variants [S42, A]. The engine currently waits "with a time limit", so the timeout variant fits.
- macOS before 14.4: the private function `__ulock_wait` with `UL_COMPARE_AND_WAIT_SHARED = 3` exists in the xnu source [S44, A for existence]; it is not public SDK API (so risky; own reading, C). Other options (Mach semaphores, named POSIX semaphores) were not researched. Not confirmed that macOS `sem_timedwait` exists (C, from memory it does not).
- The macOS minimum to choose: if cpal is 0.18 (macOS 14.2) the gap to 14.4 is small; decide with the owner whether 14.4 is the floor.
- `interprocess` crate 2.4.9 (published 2026-11-18), licence "0BSD OR Apache-2.0"; local sockets are Unix domain sockets on Unix and named pipes on Windows; only Tokio for async [S6, S63, A].
- AF_UNIX on Windows: added in Windows 10 Insider build 17063 (blog 2017-12-19); only stream sockets, no fd passing [S62, B (older than 2 years)].
- Socket and file locations: the `dirs` crate (MIT or Apache-2.0) returns `runtime_dir = None` on Windows and macOS; config on Windows is `RoamingAppData`, cache/local data is `LocalAppData`, macOS uses `~/Library/Application Support` and `~/Library/Caches` [S61, A]. So `meloet-ai/src/port.rs`, `meloet-mcp/src/place.rs` (which use `$XDG_RUNTIME_DIR`) need a new place: named pipe on Windows; on macOS a path inside a temp dir or `~/Library/Application Support` (short path; a Unix socket path is limited to 104 bytes on macOS and 108 on Linux — general knowledge, C).

### 5.3 Child process control

- Windows: a Job object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` ends all processes when the last handle to the job closes ("closing the last job object handle terminates all associated processes") [S50, A]. This replaces `PR_SET_PDEATHSIG`. A process stays in the job and nested jobs need Windows 8 or newer [S50, A]. A handle to a process (HANDLE) can be waited on, replacing `pidfd`.
- macOS: no direct equivalent of `PR_SET_PDEATHSIG` found in a primary source (not confirmed); common options are a pipe whose read end the child watches, or kqueue process events (C, own reasoning).
- `launch.rs` rule "start children one at a time because of LLDB in VS Code": VS Code on Windows would use another debugger (cppvsdbg or CodeLLDB); not researched.

### 5.4 Real-time thread priority

- Windows MMCSS: tasks include `Audio`, `Capture`, `Distribution`, `Games`, `Playback`, `Pro Audio`, `Window Manager`; threads call `AvSetMmThreadCharacteristics` or `AvSetMmMaxThreadCharacteristics`, then optionally `AvSetMmThreadPriority`. Scheduling category High (Pro Audio) uses thread priority 23–26 (lower than certain system tasks); the `SystemResponsiveness` registry value reserves CPU for low-priority tasks (value 100 disables MMCSS) [S48, A; page dated 2025-07]. Microsoft also tells WASAPI apps to use the Real-Time Work Queue API with the task class "Audio" or "ProAudio" [S49, A]. The `audio_thread_priority` crate uses task "Audio" [S64, A]; "Pro Audio" is a choice for the lead.
- Windows timer resolution: Microsoft says apps should not lower the system timer resolution unless needed [S48, A]. Not researched: the effect on futex-like waits.
- macOS: the Mach `thread_time_constraint_policy` fields are `period`, `computation`, `constraint` and `preemptible` (the last "IGNORED"); a computation under constraint/2 is forced to constraint/2 [S46, A (xnu header)]. The `audio_thread_priority` crate calls `thread_policy_set` with `THREAD_TIME_CONSTRAINT_POLICY` on macOS [S64, A].
- macOS audio workgroups (macOS 11 and newer): "An audio workgroup is a collection of real-time threads that work together to produce audio by a common deadline while spanning multiple processes, including the audio server and client apps and plug-ins"; "Only real-time threads can join an audio workgroup"; the system joins the audio thread that the audio framework gives you automatically, but apps that make their own real-time threads should adopt the API [S41, A]. The C headers show `os_workgroup_join` and `os_workgroup_interval_create`, availability `macos(10.16)` (that is macOS 11) [S43, A]. Meloet's plugin host audio threads are its own threads in another process, so they should join the output device's workgroup. How to get that workgroup (a device property named `kAudioDevicePropertyIOThreadOSWorkgroup`) is from memory only: not confirmed from a primary source.
- Local note: `meloet-engine/src/priority.rs` and `meloet-plughost/src/rt.rs` hold the Linux code (SCHED_FIFO plus rtkit). The `audio_thread_priority` crate is MPL-2.0; the repo rule says do not change MPL-2.0 files and the licence check (`node licenses/check.mjs`) must accept it.

Open questions:
- Pro Audio or Audio MMCSS class for engine and host threads?
- macOS floor: 14.4 (os_sync) or lower with a fallback?
- Measure the wake-up latency of named events vs `os_sync_wait_on_address(SHARED)` with the same ring layout.

Risks:
- The per-block handshake is the most timing-sensitive piece of the plugin design. If Windows events turn out to be too slow or jittery, the design may need a spin-then-wait mix (own idea, C).

---

## 6. File locations and shutdown

- Config, data and cache paths: see `dirs` facts in 5.2 [S61, A]. The Linux code uses XDG paths; `dirs` covers Windows and macOS. Songs and recovery files should go under the user's data dir. Saving safely ("write then rename, keep `.prev`"): rename over an existing file on Windows needs `MoveFileEx` with replace (ReplaceFile/`rename` semantics differ from POSIX; not researched, C).
- Windows shutdown: `WM_QUERYENDSESSION` / `WM_ENDSESSION`; Microsoft says to answer quickly and clean up in `WM_ENDSESSION`, and "if your application must block or postpone system shutdown, use the ShutdownBlockReasonCreate function" [S58, A: Microsoft Learn, I read the quotes in search snippets only, not the full pages]. The recipe from Microsoft's engineers: call `ShutdownBlockReasonCreate` inside the `WM_QUERYENDSESSION` handler, then return FALSE [S58, B]. Apps without a visible window get a short time limit [S58, B]. Fit with meloet's current rule (the logind delay lock, 2 s autosave, and 5 s SIGKILL after SIGTERM on Linux): not researched in detail.
- macOS quit and shutdown: `applicationShouldTerminate(_:)` "is called after the application's Quit menu item has been selected, or after the `terminate(_:)` method has been called"; the app can delay it and later call `reply(toApplicationShouldTerminate:)` [S57, A]. The page does not say that logout and shutdown go through this path (not confirmed). Using `NSProcessInfo` activity APIs or "sudden termination": not researched.
- How Tauri 2 exposes these events (for example an exit-requested event): not researched. Not confirmed.
- Single-instance (`single.rs` uses flock + socket): not researched for Windows/macOS. A named mutex or the `interprocess` local socket would be the first choices (C).
- `malloc_trim`, `statvfs`, `/proc/<pid>`: the replacement for each is small, but only Windows `GetDiskFreeSpaceEx` and macOS `statfs` are my guesses (C; not researched).

Open questions:
- Is a delay lock equivalent needed at all, or is a quick save in `WM_ENDSESSION` / `applicationShouldTerminate` enough?

---

## 7. Packaging, signing, auto-update

Tauri bundles:
- Windows: `.msi` (WiX Toolset v3, needs a Windows machine and the VBSCRIPT optional feature) and NSIS `-setup.exe`. NSIS can be cross-built from Linux or macOS, but Tauri calls that less tested and a last resort; signing a cross-built installer needs an external signing tool [S28, A]. NSIS default install mode is per-user without admin rights; `perMachine` needs admin [S28, A]. Building for ARM64 needs the C++ ARM64 build tools and the `aarch64-pc-windows-msvc` target [S28, A].
- macOS: bundles are `.app` and DMG (the DMG format itself: not read this round). Updater artifact is `.app.tar.gz` [S31, A].
- Plugin host as a second executable: Tauri's sidecar feature (already cited in the spec). On macOS every executable inside the app must be signed with the same Developer ID and notarized together (own inference, C; not read this round).

Windows signing:
- Without signing the app can run after the user accepts the SmartScreen warning; Windows 11 Smart App Control "blocks unknown unsigned apps even if they weren't downloaded from the browser" and the user can only turn the feature off completely [S29, A]. Smart App Control: signed apps can run when the cloud service cannot decide; unsigned or invalidly signed apps are untrusted and blocked; applies to Windows 11 [S35, A].
- SmartScreen: two signals, publisher reputation (certificate) and file hash reputation. "EV certificates no longer bypass SmartScreen" and paying extra for EV only for that reason "is no longer justified"; unsigned files must build reputation again for each new version; a signed app can still show a warning until it has "several weeks and hundreds of clean installs from a wide audience"; there is no manual submission process for consumer reputation [S34, A; page updated 2026-08]. Tauri's page says a manual submission to Microsoft "may reduce the warning, though not guaranteed" [S29, A] — conflict: the SmartScreen page (newer, Microsoft) says there is no mechanism for consumer endpoints; take Microsoft's statement.
- Microsoft Store apps are never subject to SmartScreen download warnings [S34, A]. MSIX/Store distribution: not researched.
- Azure Artifact Signing (renamed from Trusted Signing): "Starts at $9.99/month", no hardware token, works in CI, identity validation required, reputation builds up with downloads [S34, A]. The Azure pricing page itself showed "$-" placeholders for the two tiers (Basic 5,000 signatures per month, Premium 100,000) so I could not read prices there [S59-pricing, A, price not confirmed from the pricing page]. Billing is the full SKU amount, not pro-rated [S33, A]; a paid Azure subscription is required (free, trial, sponsored subscriptions do not work) [S33, A]. No EV certificates from this service and none planned [S33, A]. Certificates are never given to you; signing happens in the service [S33, A]. Identity validation: 1 to 30 business days (can be longer) [S32, A].
- Eligibility (page updated 2026-11-20): Public Trust certificates are for organisations in the US, Canada, the EU, the UK, Australia, New Zealand, Japan, South Korea, Singapore, Switzerland, Norway and Israel; individual developers must be in the United States or Canada [S32, A]. The owner's country and whether the owner has a legal organisation: unknown to me — this is an owner question.
- Tauri signing recipes: OV/EV certificates (the Tauri OV guide applies only to certificates bought before 2023-06-01), Azure Key Vault with `relic`, a custom `signCommand`, and `artifact-signing-cli` for Azure Artifact Signing [S29, A].
- Price of an OV code-signing certificate from a commercial CA: no primary source found. Not confirmed.

macOS signing and notarization:
- Apple Developer Program: "99 USD per membership year" (prices may vary by region) [S36, A]. Individuals can enrol with a legal name and 2FA; organisations need a legal entity, a D-U-N-S number, a public website and a work email on the domain [S36, A]. Fee waivers exist for nonprofits, schools and government [S36, A].
- Tauri: "Notarization is required when using a Developer ID Application certificate"; only the Account Holder can create the Developer ID Application certificate; notarization uses either App Store Connect API variables (`APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_PATH`) or Apple ID variables (`APPLE_ID`, `APPLE_PASSWORD` as an app-specific password, `APPLE_TEAM_ID`); CI imports a base64 `.p12` into a temporary keychain via `APPLE_CERTIFICATE` and `APPLE_CERTIFICATE_PASSWORD` [S30, A]. Ad-hoc signing (`"signingIdentity": "-"`) is possible on Apple Silicon (code-signing is required there for apps from the Internet) but "does not prevent macOS from requiring users to approve" the app in Privacy & Security [S30, A]. A paid account is needed to distribute; the free account is "only for testing and development" [S30, A].
- Notarization steps (`codesign --options runtime --timestamp`, `notarytool submit --wait`, `stapler staple`) were not read from Apple's page (the page text did not load) — I only have them from memory: C. Apple's hardened-runtime page does confirm the hardened runtime is required [S39, A]. Whether notarization itself has a separate fee: not confirmed (it is included in the membership as far as I know — C).
- Entitlements for meloet: hardened runtime; `com.apple.security.cs.disable-library-validation` for the plugin host [S39, A]; `com.apple.security.device.audio-input` only if audio input is used [S39, A]. JIT: WKWebView runs its JIT in its own web process, which Apple signs; whether the host app needs `allow-jit` is not confirmed.
- Both OSes: build machines for macOS must be a Mac (GitHub macOS runners work; see section 9).

Auto-update:
- Tauri updater: bundles NSIS, MSI (Windows), `.app.tar.gz` (macOS), AppImage (Linux); signing "cannot be disabled" — you make your own key pair with the Tauri CLI and put the public key in `tauri.conf.json`; delivery by a static JSON file or a dynamic server; Windows install modes `passive` (default), `basicUi`, `quiet` [S31, A]. No deb updater [S31, A]. The update key is separate from the OS code-signing certificate.

Open questions (owner): legal form and country for Azure Artifact Signing; whether to join the Apple Developer Program as an individual; Microsoft Store as an alternative to signing; who holds the signing keys.

Risks:
- Without Windows signing, Smart App Control (Windows 11) can block the app completely [S29, S35, A].
- New signing identity or certificate resets publisher reputation [S34, A].

---

## 8. CI

- Public repositories: standard GitHub-hosted runners are free ("Standard GitHub-hosted runners in public repositories" do not count against included usage) [S59, A]. Larger runners are billed even on public repos [S60, B].
- Private repositories: the Free plan has 12 USD of compute per month (not a fixed number of minutes). Rates per minute: Linux 1-core 0.002 USD, Linux 2-core 0.006, Windows 2-core 0.010, macOS 3 or 4-core 0.062 [S59, A]. (Plan limits and rates change; read again before relying on them.)
- Labels: `macos-latest`, `macos-14`, `macos-15`, `macos-26` are standard Apple Silicon (arm64, M1 class) runners; `macos-14` image retirement was announced for 2026-11-02 [S60, B (search summary of GitHub docs and changelog)]. `windows-latest` moved to Visual Studio 2026 in June 2026 [S60, B].
- Whether meloet's repository is public: not known to me. It decides the cost.
- Cross-compile limits: Tauri MSI cannot be built off Windows; NSIS from Linux works but is a last resort [S28, A]. macOS builds and signing need an Apple device (Tauri: "you need an Apple device to sign") [S30, A]. So the practical plan is native runners: `windows-latest` and `macos-latest`.
- Real-time and device tests (`--devices` in `scripts/check-all.sh`) cannot run on hosted runners: no audio devices, no MIDI keyboard (own inference, C). Only build, unit tests and the non-device tests fit.
- WebView2 is present on Windows hosted runners: not confirmed (it comes with Edge, and runner images include Edge — C).

---

## 9. What cannot be taken over as it is (D3 item 4)

- "Show the buffer that is really running" and PipeWire names and quantum: replaced by WASAPI engine period / CoreAudio buffer size; both readable (see 1). The "never change PC-wide settings" rule matters more on Windows: asking for a small shared period changes it for all apps on that endpoint [S49, A].
- logind delay lock: replaced by `ShutdownBlockReasonCreate` and `applicationShouldTerminate` (see 6).
- rtkit and `SCHED_FIFO`: replaced by MMCSS and Mach time constraint with audio workgroups (see 5.4).
- memfd, futex, pidfd, `PR_SET_PDEATHSIG`: replaced as in 5.1–5.3.
- X11 window code in `meloet-plughost`: replaced by HWND and NSView/NSWindow code (see 4).
- `WEBKIT_DISABLE_COMPOSITING_MODE`, `JSC_useGenerationalGC`, `webkit-damage.sh`, WebKit web process watcher (`webprocess.rs`): WebKitGTK only; WebView2 and WKWebView need their own measurements (see 3).
- GTK file chooser (`pcwindow.rs`): Tauri's dialog plugin or native dialogs (not researched).
- `$XDG_RUNTIME_DIR` sockets: replaced by named pipes (Windows) or a short path (macOS) (see 5.2).

---

## 10. Items that need a prototype (D3 item 2)

1. Wake-up latency for the audio-block handshake: Windows named events vs macOS `os_sync_wait_on_address` (SHARED). Target: no worse than the futex numbers in `docs/history/measurements/`.
2. Plugin window in the plugin host: a VST3 `attached(HWND)` on Windows; a VST3 `attached(NSView)` and an AU view on macOS, with a plugin host that has no app bundle.
3. WebGL regions (up to 12 contexts) and 60 Hz drawing in WebView2 and WKWebView; check context loss and Low Power Mode on macOS.
4. MIDI hot-plug by polling with midir (WinMM and CoreMIDI), and timestamp precision for recording position.
5. Windows low-latency output with `IAudioClient3` (own backend or patch for cpal) vs cpal shared mode.
6. Signing flow end to end on both OSes (including the plugin host as a sidecar).

---

## 11. Source list

Local code read (A, working tree `fea/round12`, 2026-11-23): `/srv/meloet-agent/meloet/crates/meloet-plughost/src/window.rs`, `/srv/meloet-agent/meloet/crates/meloet-plugin/src/ipc/mod.rs`, `/srv/meloet-agent/meloet/docs/architecture.md` (line 890), grep of `libc::` use in `/srv/meloet-agent/meloet/crates`.

| ID | Source | URL | Rank |
|----|--------|-----|------|
| S1 | cpal README | https://github.com/RustAudio/cpal (read as raw README, master) | A |
| S2 | cpal CHANGELOG (0.16.0 to 0.18.2) | https://raw.githubusercontent.com/RustAudio/cpal/master/CHANGELOG.md | A |
| S3 | cpal 0.18.2 docs | https://docs.rs/cpal/0.18.2/cpal/ | A (excerpt only) |
| S4 | cpal WASAPI source (`src/host/wasapi/device.rs`, `stream.rs`, `wasapi.rs`), master = 0.19.0 | https://github.com/RustAudio/cpal/tree/master/src/host/wasapi | A |
| S5 | cpal CoreAudio source (`macos/device.rs`, `property_listener.rs`) | https://github.com/RustAudio/cpal/tree/master/src/host/coreaudio/macos | A |
| S6 | crates.io API for cpal, midir, coremidi, interprocess, memmap2, shared_memory, windows, objc2, audio_thread_priority | https://crates.io/api/v1/crates/<name> | A (versions, licences, dates) |
| S7 | midir README | https://github.com/Boddlnagg/midir | A |
| S8 | midir backend sources (winmm, winrt, coremidi) | https://github.com/Boddlnagg/midir/tree/master/src/backend | A |
| S9 | Windows MIDI Services site | https://microsoft.github.io/MIDI/ | A |
| S10 | Microsoft Windows blog, 2026-02-17, "making music with MIDI just got a real boost in Windows 11" | https://blogs.windows.com/windowsexperience/2026/02/17/making-music-with-midi-just-got-a-real-boost-in-windows-11/ | A |
| S11 | microsoft/MIDI repository (MIT licence) | https://github.com/microsoft/MIDI | A |
| S12 | Steinberg press release 2025-10-15 (ASIO dual licence) | https://ocl-steinberg-live.steinberg.net/_storage/asset/808575/storage/master/Press%20Release%20-%202025-10-15%20-%20OBS%20Partnership-%20EN.pdf | A |
| S13 | Steinberg press release 2025-10-29 (VST 3.8, MIT) | https://ocl-steinberg-live.steinberg.net/_storage/asset/819253/storage/master/Press%20Release%20-%202025-10-29%20-%20VST%203.8%20-%20EN.pdf | A |
| S14 | VST3 dev portal, licensing, and SDK README ("VST SDK 3.8.x") | https://steinbergmedia.github.io/vst3_dev_portal/pages/VST+3+Licensing/Index.html ; https://github.com/steinbergmedia/vst3sdk | A |
| S15 | vst3_pluginterfaces LICENSE.txt (MIT, 2026) | https://github.com/steinbergmedia/vst3_pluginterfaces/blob/master/LICENSE.txt | A |
| S16 | `iplugview.h` (platform types, size units) | https://github.com/steinbergmedia/vst3_pluginterfaces/blob/master/gui/iplugview.h | A |
| S17 | VST3 plugin format (bundle layout) | https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Locations+Format/Plugin+Format.html | A |
| S18 | VST3 plugin locations | https://steinbergmedia.github.io/vst3_dev_portal/pages/Technical+Documentation/Locations+Format/Plugin+Locations.html | A |
| S19 | LibreArts, 2025-11-04, "Steinberg relicenses VST3 and ASIO" | https://librearts.org/2025/11/steinberg-relicenses-vst3-and-asio/ | B |
| S20 | Steinberg forum threads on ASIO SDK agreement delays (search summary only) | https://forums.steinberg.net/t/proprietary-asio-sdk-license-agreement-no-response-after-5-weeks-odemi-daw/1043608 and related | C |
| S21 | Tauri 2 prerequisites | https://v2.tauri.app/start/prerequisites/ | A |
| S22 | Tauri 2 webview versions | https://v2.tauri.app/reference/webview-versions/ | A |
| S23 | WebView2 distribution (page updated 2026-10-02) | https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution | A |
| S24 | WebView2 performance best practices (updated 2026-11-02) | https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/performance | A |
| S25 | WebKit source `WebGLRenderingContextBase.cpp` (main) | https://github.com/WebKit/WebKit/blob/main/Source/WebCore/html/canvas/WebGLRenderingContextBase.cpp | A |
| S26 | Chromium source `webgl_rendering_context_base.cc`; Chromium graphics-dev thread; WebKit change log (limit of 16 in other browsers) | https://chromium.googlesource.com/chromium/src/+/refs/heads/main/third_party/blink/renderer/modules/webgl/webgl_rendering_context_base.cc ; https://groups.google.com/a/chromium.org/g/graphics-dev/c/fmNedEEAYpA ; https://webkit.googlesource.com/WebKit/+/70e10861eb2fbd9d0862fd8702874c25ae5d8bfd%5E%21 | A for code; B for the number 16 and the flag (default value in Chromium source not found) |
| S27 | tauri-plugin-macos-fps README; WebKit bugzilla 215745 / 294338; Tauri issue 13978 (via the plugin); Babylon.js forum post | https://docs.rs/crate/tauri-plugin-macos-fps/0.1.0 ; https://www2.webkit.org/show_bug.cgi?id=215745 ; https://forum.babylonjs.com/t/performance-between-safari-and-wkwebview-tauri/60811 | B (plugin and bugzilla), C (forum) |
| S28 | Tauri 2 Windows installer | https://v2.tauri.app/distribute/windows-installer/ | A |
| S29 | Tauri 2 Windows code signing | https://v2.tauri.app/distribute/sign/windows/ | A |
| S30 | Tauri 2 macOS code signing | https://v2.tauri.app/distribute/sign/macos/ | A |
| S31 | Tauri 2 updater plugin | https://v2.tauri.app/plugin/updater/ | A |
| S32 | Azure Artifact Signing quickstart (updated 2026-11-20) | https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart | A |
| S33 | Azure Artifact Signing FAQ (updated 2026-11-20) | https://learn.microsoft.com/en-us/azure/artifact-signing/faq | A |
| S34 | SmartScreen reputation for Windows app developers (updated 2026-08-17) | https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation | A |
| S35 | Smart App Control (Microsoft support) | https://support.microsoft.com/en-us/topic/what-is-smart-app-control-285ea03d-fa88-4d56-882e-6698afdb7003 | A |
| S36 | Apple Developer Program enrolment | https://developer.apple.com/programs/enroll/ | A |
| S37 | Apple membership comparison (no price on the page I read) | https://developer.apple.com/support/compare-memberships/ | A (no data) |
| S38 | Apple notarization page (text did not load; steps from memory) | https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution | C for steps |
| S39 | Apple "Configuring the hardened runtime" | https://developer.apple.com/documentation/xcode/configuring-the-hardened-runtime | A |
| S40 | Apple "Migrating your audio unit host to the AUv3 API" | https://developer.apple.com/documentation/audiotoolbox/migrating-your-audio-unit-host-to-the-auv3-api | A |
| S41 | Apple "Understanding audio workgroups" | https://developer.apple.com/documentation/audiotoolbox/understanding-audio-workgroups | A |
| S42 | Apple libplatform header `os/os_sync_wait_on_address.h` | https://github.com/apple-oss-distributions/libplatform/blob/main/include/os/os_sync_wait_on_address.h | A |
| S43 | Apple libdispatch headers `os/workgroup_object.h`, `workgroup_interval.h` | https://github.com/apple-oss-distributions/libdispatch/tree/main/os | A |
| S44 | xnu `bsd/sys/ulock.h` | https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/ulock.h | A (source exists; not public SDK API) |
| S45 | xnu `bsd/sys/posix_shm.h` (PSHMNAMLEN 31) | https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/posix_shm.h | A |
| S46 | xnu `osfmk/mach/thread_policy.h` | https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/mach/thread_policy.h | A |
| S47 | Microsoft `WaitOnAddress` (page dated 2024-02-05) | https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-waitonaddress | A |
| S48 | Microsoft MMCSS (page dated 2025-07-14) | https://learn.microsoft.com/en-us/windows/win32/procthread/multimedia-class-scheduler-service | A |
| S49 | Microsoft "Low Latency Audio" (page dated 2024-12-13) | https://learn.microsoft.com/en-us/windows-hardware/drivers/audio/low-latency-audio | A |
| S50 | Microsoft Job Objects (2025-07-14) | https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects | A |
| S51 | Microsoft "Creating named shared memory" (2024-07-30) | https://learn.microsoft.com/en-us/windows/win32/memory/creating-named-shared-memory | A |
| S52 | Raymond Chen, "Is it legal to have a cross-process parent/child or owner/owned window relationship?", 2013-04-12 | https://devblogs.microsoft.com/oldnewthing/20130412-00/?p=4683 | B (older than 2 years) |
| S53 | Microsoft Q&A answer on cross-process SetParent | https://learn.microsoft.com/en-us/answers/a/12736602 | C |
| S54 | yabridge architecture notes | https://github.com/robbert-vdh/yabridge/blob/master/docs/architecture.md | A (project doc, Linux) |
| S55 | Bitwig user guide, plug-in handling | https://www.bitwig.com/userguide/latest/vst_plug-in_handling_and_options | A |
| S56 | Apple CoreMIDI `MIDINotifyBlock` | https://developer.apple.com/documentation/coremidi/midinotifyblock | A |
| S57 | Apple `applicationShouldTerminate(_:)` | https://developer.apple.com/documentation/appkit/nsapplicationdelegate/applicationshouldterminate(_:) | A |
| S58 | Microsoft `WM_QUERYENDSESSION`, `ShutdownBlockReasonCreate`, and Raymond Chen posts (read only as search snippets) | https://learn.microsoft.com/en-us/windows/win32/shutdown/wm-queryendsession ; https://learn.microsoft.com/en-us/windows/desktop/api/Winuser/nf-winuser-shutdownblockreasoncreate ; https://devblogs.microsoft.com/oldnewthing/?p=7373 | A for Microsoft Learn text as quoted; B because only snippets were read |
| S59 | GitHub Actions billing | https://docs.github.com/en/billing/concepts/product-billing/github-actions | A |
| S60 | GitHub runner reference and changelog items (macOS 14 retirement, Windows runner VS 2026) via search summary | https://docs.github.com/en/actions/reference/runners/github-hosted-runners ; https://github.blog/changelog/2026-10-01-github-actions-macos-14-runner-image-retirement | B |
| S61 | `dirs` README | https://github.com/soc/dirs-rs | A |
| S62 | Microsoft blog, "AF_UNIX comes to Windows", 2017-12-19 | https://devblogs.microsoft.com/commandline/af_unix-comes-to-windows/ | B (older than 2 years) |
| S63 | `interprocess` README | https://github.com/kotauskas/interprocess | A |
| S64 | `audio_thread_priority` source (`rt_win.rs`, `rt_mach.rs`) | https://github.com/padenot/audio_thread_priority | A |
| S65 | cpal `Cargo.toml` (master = 0.19.0) | https://github.com/RustAudio/cpal/blob/master/Cargo.toml | A |
| S66 | Apple Core Audio mailing list, 2012 (suggestion of a synced always-on-top window), seen only as a search result | https://lists.apple.com/archives/coreaudio-api/2012/Jul/msg00145.html | C (older than 2 years) |
| S59-pricing | Azure Artifact Signing pricing page (prices shown as "$-") | https://azure.microsoft.com/en-us/pricing/details/artifact-signing/ | A (no price read) |

Overall rank of the notes: A for the facts about APIs, licences, versions and source code that were read directly; B or C where marked. Conclusions that join several facts (for example "10 ms with cpal on Windows", "ASIO GPLv3 does not fit") are own reasoning and carry the rank of their weakest input.

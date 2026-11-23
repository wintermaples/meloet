# meloet: progress

Where meloet is now, the work still open, and notes for the next round. Read this after [`CLAUDE.md`](../CLAUDE.md).

The history of rounds 1 to 9 (what each round did, in order, with every number) is in [`history/progress.ja.md`](history/progress.ja.md). It is in Japanese and is not changed any more. This file does not repeat that history; it only says what is true now. When something here and the history disagree, this file is newer.

Last written: 2026-11-24, at the end of round 12 (branch `fea/round12`, not merged into `main`; section 6, "Round 12").

Branch: each piece of work is done on its own branch made from `main` (`fea/`, `fix/`, `doc/`, `refactor/`, `chore/`; the rule is in `CLAUDE.md`), and goes into `main` when the owner asks. `foundation` is no longer used (the owner: the basic build is done); older lines that name it are records of that time.

## 1. Versions

| What | Version | Where |
|---|---|---|
| Contract between Rust and the screen (protocol) | 33 | `crates/meloet-protocol/src/lib.rs` (`PROTOCOL_VERSION`), architecture 24-5, 24-11, 24-15, 25-1, 25-2 and 25-3 |
| Telemetry binary | 5 | architecture 3-3 and 25-2 |
| Take content binary | 2 | architecture 3-4 |
| Peaks answer | kind 4, version 1 | `crates/meloet-protocol/src/audio.rs` |
| Lane points binary | 1 | `crates/meloet-protocol/src/automation.rs` |
| Song file | 7 (reads 6 too; versions 5 and older cannot be read) | `crates/meloet-core/src/songfile.rs`, architecture 14, 22-3, 25-1 and 25-3 |
| Recovery file | 3 (reads 2 too) | `crates/meloet-core/src/recovery.rs`, architecture 15 and 25-1 |
| Wire to the plugin host | 7 | `crates/meloet-plugin/src/wire.rs`, architecture 24-16 E and I |
| Sound pack catalog | format `meloet-sounds-catalog`, version 1; pack `2026.11.2` | `tools/sounds/catalog.json`, architecture 23-1 and 23-9 |
| The person's effect presets (`presets.json`) | 1 | `crates/meloet-core/src/presets.rs`, architecture 25-22 |
| Drum patterns | format `meloet-drum-patterns`, version 1 | `tools/patterns/drums.json`, architecture 25-20 |
| Settings | 1 (fields added in round 12) | `crates/meloet-core/src/settings.rs`, architecture 8 |
| App | 0.3.0 (released as v0.x.y) | `crates/meloet-app/Cargo.toml` |

Time base: a quarter note is 1920 ticks. Tempo is always quarter notes per minute, whatever the time signature (architecture 22-1, 22-2). The tempo map is `crates/meloet-model/src/tempo_map.rs` and `ui/src/time/tempoMap.ts`.

## 2. What works now

Short lists. The numbers after "arch" are chapters of [`architecture.md`](architecture.md).

- **Keyboards and recording** (arch 5, 11, 12): MIDI keyboards through the ALSA sequencer, plug and unplug while running, two keyboards heard together. Play, record with count-in, loop recording into takes. A recorded key is put where it was heard at the moment it was pressed. Quantize is only an edit. Round 12: keep what was just played (Shift R, 25-16), the PC keyboard as a keyboard (Mod K, 25-17), the arpeggio in the engine (25-26), metronome volume, sounds, "only while recording" and tap tempo (25-8).
- **Outputs and inputs** (arch 4, 6, 8-1): sound through cpal; output and input names from PipeWire Routes; the buffer shown is the one really running. meloet never changes PC-wide settings.
- **Undo, saving, recovery** (arch 13, 14, 15): every song change is one undo step (`meloet_model::History`); the selection is in the core and comes back with undo. Song file version 7 (reads 6) with safe writing and `.prev`. The recovery file (version 3) is written when the PC shuts down and 2 seconds after a change. File ▸ 「最近ひらいた曲」 (25-7).
- **Piano roll** (arch 16): notes, velocity, CC lanes, drums, quantize. A drag moves only the screen first and sends one intent on release. Editing notes makes a new take with a new id in the same place. Round 12: nine scales and 「色をつける」 (25-9), velocity tools and legato (25-10), chord progressions (25-11), other tracks' notes faintly (25-18), sounding notes lit in the track color (25-19).
- **Arrange** (arch 17, 18, 23-13): moving clips, takes, mute and solo, fixed velocity, save as. Scrolling is tuned for the CPU path of WebKit. Round 12: duplicate a track (25-6), repeat clips (25-21), whole sections by marker (25-30), keys into audio (25-31), comping of vocal clips (25-33).
- **Tempo, time signature, markers, automation, undo history band** (arch 21). Round 12: the master's volume lane and the fade-out (25-32).
- **Mixer** (arch 19-11, 21-11): faders, pan, sends, returns, inserts, master. While dragging, the sound changes too (`commit: false`, at most one per frame); release commits one step; Esc sends `CancelMixPreview`. Round 12: presets of the built-in effects (25-22), 「音が大きすぎます」 with 「全部のトラックを下げる」 (25-24), the finishing meters and 「モノで聞く」 (25-29), the spectrum in the top band (25-28).
- **Sounds and the shelf** (arch 19, 23-1, 23-9): 31 built-in sounds (26 from samples, 5 made by calculation) with four knobs each (25-27), 9 built-in effects written by us (`crates/meloet-sound/src/effects/`; gate, de-esser and saturation are new, 25-23), audition on the shelf, twelve drum patterns on the shelf (25-20). The sound pack is fetched one sound at a time from the GitHub Release; a sound plays as soon as it has arrived. Song notes always sound the same from wherever play starts (round robin and randomness come from the place in the song; arch 22-25).
- **Plugins and SF2** (arch 20): VST3 and LV2 (no CLAP) in a separate process `meloet-plughost`, one process per plugin, watched by time. Scanning, the "added later" part of the shelf, plugin windows (X11), parameters, latency compensation. SF2 instruments.
- **Vocal tracks and audio** (arch 22): audio input, measuring the latency (「遅れを測る」), recording a voice, audio clips with waveforms, fades and gain, dragging in audio files, the song folder, stretch and pitch (`crates/meloet-stretch`, preset `cheaper`, ±12 semitones, keep formant on from the start). Round 12: reverse and normalize (25-12), 「音程を寄せる」 (25-34).
- **Export** (arch 22-11, 23-16): export (wav, flac), with plugins, in the same path as playing. The export check counts places over 0 dB before the final cut, and tells about solo. Round 12: loudness and true peak, 「公開先に合わせる」 (25-13), the 「書き出しました」 dialog (25-14), title, artist and picture in the file (25-25). mp3 is built but not shipped (25-25, B-98-1).
- **Shipping** (arch 23): a deb for Ubuntu 24.04 and 26.04 (`scripts/build-deb.sh`), development hooks only in `dev-hooks` builds (`scripts/check-release.sh`), audio threads at SCHED_FIFO 20 when the PC allows it (arch 23-10), memory targets (arch 23-12), the app icon (`crates/meloet-app/icons/meloet.svg`). Round 12: the notice of a new version (25-15), the apt repository script (`scripts/make-apt-repo.sh`, not published), the AppImage trial (25-35, not published).
- **Screen** (arch 7): WebGL for the large faces, one context per region, budget 12 (still at most 10 regions after round 12). WebKit draws on the CPU path (`WEBKIT_DISABLE_COMPOSITING_MODE=1`) and runs a Full GC every time (`JSC_useGenerationalGC=0`). Help ▸ 「キーの一覧」 and 「使い方をひらく」 (25-5).

Tests at the end of round 8: Rust 1364 pass, 0 fail, 106 ignored; screen 3281 (`scripts/check-all.sh --release`).

## 3. Open work

Items from rounds 7 and 8 that are still open. Older items that rounds 7 and 8 did not touch are in the round 6 list of [`history/progress.ja.md`](history/progress.ja.md) (section 「第 6 回の宿題」).

### Tests that fail on time

- `readahead::tests::other_audio_for_the_same_clips_comes_in_over_ten_ms_where_the_ring_ends` (only on a busy PC, the number of swaps is 3 where the test waits for 2).
- `meloet-midi`: `plugging_and_unplugging_after_start_is_reported_with_the_same_id` (the port name came out as `…:portet`).
- Seen in round 12 on a busy PC (they pass alone): `meloet-ai --test analyze` and `--test link` ("another measurement is running"; perhaps a race of `Worker::ai_jobs` between two measurements of one test), `meloet-ai` `track_ids::lists_take_numbers_and_ids_together`, `meloet-core` `sounds::fetch_tests::a_stopped_fetch_keeps_its_part_and_goes_on_later`.

### Sound and timing

- **Key → DAC read as exactly 1 quantum** (the 10.4 ms of round 7). It did not come back. `tools/measure/summarize.py` marks it if it does. Guess (C): cpal gets no answer from `pw_stream_get_time_n`.
- **Key → screen while the sound pack is being fetched**: p95 22.49 ms, 94 % within 2 frames (load 13, so the numbers are not clean). It bunches when the fetch starts and when the piano loads. Measure again on a quiet PC.
- **Surge XT**: the first chord loses 80 to 120 ms per track while Surge XT makes its first sound (idea: pass one silent block just after loading; not tried). Not checked whether the two stops in the quiet measurement were the "stopped host" mistake fixed in arch 23-14. On a PC where rtkit raises the priority, the plugin host's `RLIMIT_RTTIME` of 200 ms may kill it on a long block.
- With the mixer open while playing, one frame of 23 to 29 ms every 25 seconds (round 7; not measured again).
- 「いちばん大きい所」 (the loudest place) uses the engine's sample rate (writing at 44100 changes it a little).

### Screen and WebKit (CPU path)

- Arrange zoom: busyMs 19 to 20 ms (frame gaps stay inside 60 Hz). The WebGL canvas read back is fixed inside WebKit.
- Arrange with 192 audio clips: zoom 30.6 ms (waveforms made again on every zoom). While the 192 clips are being made, an 11 ms frame each time the state arrives (cause not known). Making the selected clip while playing with the panel open redraws everything every 0.5 s.
- Pan and send value text change 15 times a second, not every frame (every frame made the damage cover the whole box on the CPU path). This may not match the spec's "the mixer's value text every frame".
- **Mixer with 64 tracks and 4 lanes each: 47 to 49 frames a second** (round 7: 56 to 60). Keep "value text every frame" from the spec and win it back by making the fonts lighter: `CSSFontAccessor::isLoading` walks the 123 `@font-face` of IBM Plex Sans JP for each drawn text (about 10 %). Another idea (not taken): 30 Hz only when heavy.
- Fonts: fewer `@font-face` while keeping the `@fontsource` pieces as they are (expected about 2 ms less busyMs while playing, with no pixel change). See `refs/archive/k7f3-font-trial` and `docs/history/measurements/2026-11-21-round7-step3-fix.md`.
- While playing, the CPU path draws 60 or more frames a second even when the window is hidden or the screen is off. Stop drawing while not seen?
- WebKit memory in play longer than 2 hours.
- While dragging a clip's gain, the other clips' waveforms are drawn with the same dB; not yet measured with `tools/measure/webkit-damage.sh`.
- Small screen items from the round 7 end pictures (design item B-94, 88 to 92; 89 and 92 were done in round 8, the rest not checked).
- 12 differences from the mocks found in the round 8 end pictures, not fixed (`docs/history/measurements/2026-11-22-round8-end.md`; for example the □ in the FirstLane list, and the solo line of ExportCheck wrapping to 2 lines).

### Sound pack

- While fetching, the shelf's 「お気に入り」 count is 0.
- After 「やめる」 the status line says nothing.
- The folder name: the mock says 「音の素材」, the code uses `sounds` (arch 23-1).
- The swap of a held sound at the moment its sound arrives (10 ms overlap) has not been recorded.

### Other

- **Typing out of range in 「手で決める…」**: Rust quietly puts the value at the end of the range. Spec chapter 3 (number field) says the status line tells it.
- **deb**: the bundler puts `128x128@2x.png` in `hicolor/256x256@2` (not the usual name; 512 and 128 are fine). The identifier `dev.meloet.app` gives a warning for macOS. No automatic update (round 12 added only a notice of a new version, and an apt repository script that is not published yet).
- **Clean-up (the owner does this)**: there are two functions that raise a thread to real time, in the plugin host (`crates/meloet-plughost/src/rt.rs`) and in the engine (`crates/meloet-engine/src/priority.rs`).

### Removed from the open list (done in later rounds)

- The voice placed one block late after only the output dropped a callback: fixed in round 8 (arch 22-26).
- The PC's latency one block too large on real devices: fixed in round 8 (arch 22-26).
- xruns with 16 Surge XT tracks and a compressor: 6 of 6 runs had 0 xruns at the end of round 8 (real-time audio thread, arch 23-10).
- The second start 40 to 60 ms slower: round 8 end measured 733 to 753 ms (round 7: 739 to 781).
- Exporting while playing makes more missed ZamComp answers: under nice 19, export does not add missed answers (round 8 quiet measurement).
- The time-based tests `the_ring_holds_the_clips_in_the_order_they_sound_with_silence_between`, `a_host_process_crashed_from_outside_stops_its_slots_at_the_second_block`, the `--ignored` window test, the leftovers in `/tmp`, the three copies of the wav head, the audiofiles test in release: fixed in round 8 (k8tc).

## 4. Questions to the owner

Still open:

1. **Install and try on a real PC** (`docs/install-check.md`): on a real PC or VM with Ubuntu 24.04 and 26.04, install `target/deb/meloet_0.3.0_amd64.deb` → open → keyboard → fetch the sound pack → the grand piano sounds → the plugin shelf → save and open → remove. Also the "audio thread priority" line when started from a terminal (`(limit)`, `(rtkit)`, or could not raise). The agent on this PC cannot check rtkit (a session with no seat).
2. **Measure 「遅れを測る」 again**: round 8 fixed the formula (real inputs were one block too large). If 「この数を使う」 was pressed before, that number is no longer read (`measured_latency_2`).
3. **Clean-up**: delete `/var/crash/_usr_lib_x86_64-linux-gnu_webkit2gtk-4.1_WebKitWebProcess.1001.crash` (made when a helper stopped WebKit under gdb; while it is there, apport writes no new report). Remove meloet-agent from the docker group (`sudo gpasswd -d meloet-agent docker`; the image `meloet-deb-build` is already deleted).
4. **Put the deb on a Release?** Make an app Release (for example `v0.1.0`) apart from the sound pack, and when.
5. **Round 12**: merge `fea/round12` into `main`? The 34 questions B-98-1 to B-98-34 in `docs/spec/open-questions.md` (mp3, the AppImage's licenses, controls hidden at 1280 px, the wording the developers made, …), and what to listen to (section 6, "Round 12").

A note given to the owner: the VS Code launch 「meloet（デバッグ）」 uses `--no-default-features`, so the development hooks (`MELOET_*`) do not work. Add `--features dev-hooks` to use them while debugging.

Answered and removed: `foundation` into `main` (fast-forwarded, 2026-11-23); cutting the sample tails (not cut: only −50 dB sounds right and it saves only 56.5 MiB over all sounds, 2026-11-23; the next steps if memory runs short are the rest of arch 19-7).

## 5. Round 9 (stopped part way, 2026-11-23, the owner's weekly limit)

The owner's six requests: 1. clean `target/` and make `target/debug` small; 2. resting on text cut with 「…」 shows the whole text; 3. double click a number to type it; 4. keep the spec in the repository as the source of truth (Markdown, English); 5. tidy the documents and make them English (only the ones still in use; old records stay Japanese in `docs/history/`); 6. tidy the comments and make them English. README.md is rewritten in this order: icon, summary, install, how to develop, license (the owner). Debug info for outside libraries is line numbers only (the owner).

Done:

- 1: `Cargo.toml` `[profile.dev.package."*"] debug = "line-tables-only"` (`target/debug` 98 → 13 GB).
- 2: `ui/src/lib/tip.svelte.ts` `watchCutText`, `data-tip-through`.
- 3: mixer, the head's volume, percent sliders, latency, tempo point values.
- 4, steps 1 and 2: `docs/spec/1-tokens.md` to `docs/spec/6-messages.md` in English, `docs/spec/glossary.md`. `tools/design/spec_check.py --strings` misses only 17 strings (the step names).
- 5: the records of rounds 1 to 8 moved to `docs/history/` unchanged; `docs/architecture.md` in English with the same chapter numbers (the Japanese text is `docs/history/architecture.ja.md`); this file.
- 6: comments in Rust, tools and `.vscode` in English.
- A VS Code launch configuration that builds the deb.
- Spec step 3, part: `docs/spec/open-questions.md` (questions with no answer yet), the step names written out one by one (`spec_check.py --strings` now misses 0), `spec_check.py` no longer reads the old spec.html.
- `README.md` (English, in the order above) and `docs/install-check.md` in English.
- `CLAUDE.md` in English.
- Spec step 3, the rest: `docs/spec/7-readings.md` (the developers' readings and decisions, from part A of `docs/history/design-notes.ja.md`; 269 of 384 rows kept). Three new questions found on the way: B-96-1 to 3 in `docs/spec/open-questions.md`.

Still to do:

- **Screen comments in English: after the round 10 improvements** (the owner, 2026-11-23), because the improvements touch the same files. The rules are in `target/k9-comments-rules.md`. Of the four helper worktrees that had half-done work, only `.claude/worktrees/agent-aed06ffb08658acbb` (pianoroll, automation, timeline, history) was still there on 2026-11-23.
- `.claude/agents/` in English: last.
- At the end: `scripts/check-all.sh --release`, and push (ask the owner).

Documents in English now: `CLAUDE.md`, `README.md`, `docs/progress.md`, `docs/architecture.md`, `docs/install-check.md`, `docs/spec/` (README, chapters 1 to 6, glossary, 7-readings, open-questions).

## 6. Plan for round 10: answers to the improvement notes (user:2026-11-23)

What was asked and decided about the owner's `改善ノート.md` (14 items). New wording, including the context menu proposals, has the source `user:2026-11-23`.

- **Order**: first finish the half-done work of round 9 (such as the screen comments in English), then start the improvements (they touch the same files). Changed later the same day, when it turned out that the comment work had to start from zero (the helpers' worktrees held almost nothing): the screen comments wait until after the round 10 improvements (the owner, 2026-11-23; see section 5).
- **Helper models**: until Fable 5.2 comes back on 11-24 21:00, the hard helper also uses Opus 5.5.
- **New looks** (the overdub toggle, the chord stamp, the table of chords in the key, the clip name box, the AI connection setting, and so on): show mock pictures and let the owner decide before building. Only values with names from the spec.

1. **Clip names**: can be changed from these 4 places. Each shows an input box in place (Enter decides, Esc cancels).
   - Press the name twice. Pressing the wave of an audio clip twice still makes the row taller, as now.
   - F2. With a clip selected, the clip's name; with only a track selected, the track's name.
   - 「名前を変える」 in the context menu.
   - The name in the right panel 「選んだクリップ」.
2. **Overdub recording**: put a 「重ねる」 toggle in the transport band.
   - While it is on, notes recorded on a keys track are added to the current clip.
   - When recording in a loop, notes are added pass by pass.
   - It edits the current take. The number of takes does not grow; to go back, use undo.
   - Vocal tracks record takes as now.
3. **The Studio Kit crash cymbal is cut at the end of the note** (bug). In the pack's `instrument.json` the crash region is `oneShot: true`, so the cause is somewhere else.
4. **Joining clips**: keys clips only.
   - Usable when 2 or more clips on the same track are selected. Gaps between them go into the one clip as empty parts.
   - For takes, the ones now shown are joined. Other takes are not kept (undo brings them back).
   - The key is Mod J.
5. **Pasting notes** (bug): with Mod C → Mod V or with the context menu, notes cannot be pasted into a clip on another track.
6. **Chord help**: build these two.
   - Chord stamp: choose a chord shape in the draw tool; the key pressed becomes the root and the chord is written.
   - Table of chords in the key: the chords of the key fixed by the scale lock, listed by name. Pressing one plays it, and the draw tool can then place that chord.
   - Do not build "one-finger chords" or "showing chord names".
7. **Keys tracks with no clip**: add these ways in.
   - Press an empty place in the arrange twice, or 「ここにクリップを作る」 in the context menu of an empty place. Both make an empty clip of 4 bars from that bar and open it in the piano roll.
   - 「ピアノロールでひらく」 in the context menu of the track head. It opens even with no clip, and inside it shows the present hint 「クリップがありません」.
8. **Context menus** (as proposed):
   - Keys clip:
     - 「ピアノロールでひらく」 E
     - 「切り取り」, 「コピー」, 「貼りつけ」, 「複製」, 「消す」
     - 「ここで切る」 (at the place right-clicked, snapped to the lines shown on the ruler), 「つなげる」 Mod J (when 2 or more are selected), 「名前を変える」 F2
     - 「次のテイク」, 「前のテイク」, 「テイクを消す」
   - Audio clip: the list above without 「ピアノロールでひらく」 and 「つなげる」. 「逆にする」 and 「ノーマライズ」 stay as disabled rows.
   - Empty place in the arrange: 「貼りつけ」, 「ここにクリップを作る」 (keys track), 「音のファイルを入れる…」 (vocal track).
   - Track head: at the very top, 「ピアノロールでひらく」 E (keys track). The other rows as now.
   - Empty place in the marker row: 「ここにマーカーを足す」.
   - Menu bar 「編集」: 「クリップを切る」 S (at the play line), 「クリップをつなげる」 Mod J.
9. **Markers**: add 2 ways to add one.
   - Press an empty place in the marker row twice (snapped to the beat).
   - 「ここにマーカーを足す」 in the context menu of the marker row.
   - Do not add it to the menu bar or to the arrange's context menu.
10. **Opening the piano roll while the loop is on**: only when the loop overlaps that clip, use the zoom where the loop range just fits and show it from the loop start. When it does not overlap, fit to the clip as now (`fitQuarterPx`).
11. **To the start** (Home and the button): when the loop is on, move to the loop start. Pressing it again while at the loop start moves to the start of the song.
12. **Keyboard buttons**: recorded on a real Keystation 49 MK3 (in the Mackie Control form).
    - On the second port 「(Transport)」 they come as ch1 notes: play 94, stop 93, record 95, up 96, down 97, left 98, right 99, center 100.
    - The fader comes on the first port as CC 7 (ch16 when recorded).
    - There are no rewind, fast forward or loop buttons.
    - Decided:
      - Support play, stop and record. For other keyboards, also support MCU rewind 91, fast forward 92 (one bar each) and Cycle 86 (loop).
      - Up and down choose the track. Left and right move one bar. Center turns the loop on and off.
      - The fader changes the selected track's volume. It starts moving after it passes the present volume position, and becomes one step where it stops.
      - ALL OFF and RESET (CC 123 and 121) stop notes left sounding.
      - CANCEL, CH, OCTAVE, TRANSPOSE, DAW, ENTER and WHEEL are handled inside the keyboard; nothing reaches meloet.
      - Do not build changing sounds with PROGRAM #.
    - Sources: Keystation 49 MK3 User Guide v1.6 (A), meloet-research's study (2026-11-23).
13. **AI**: connect to a running meloet with MCP. The AI app starts `meloet-mcp`, which connects to meloet through a local socket on the PC.
    - What the AI does:
      - Reads the song and gives advice.
      - Writes when asked. Each change is one undo step and shows in the undo history.
      - Gives advice from exported sound: exports a range and hands it over with numbers such as loudness.
      - Do not build a teacher for what was played.
    - The socket opens only when turned on in the settings. It is placed where only the same user can use it.

### Round 10 so far (2026-11-23)

Done and merged into `foundation` (every check of `scripts/check-all.sh` passed after the step 1 merges); how each is built is in architecture chapter 24:

- Round 9 docs: `docs/architecture.md`, `README.md`, `docs/install-check.md`, `CLAUDE.md`, this file, spec chapter 7 and `docs/spec/open-questions.md` (B-96-1 to 3 are new) in English.
- 3 (crash): not reproduced on any path; the owner said it was a mistake. Guard tests kept.
- 5 (paste): fixed; paste grows the clip (the owner's decision), Mod V pastes what was copied last.
- 10, 11 (loop): the piano roll opens on the loop; Home goes to the loop start, then the song start.
- 12 (keyboard buttons): Mackie Control buttons on the second port, the fader on the selected track's volume (picks up), CC 123 / 121.
- 1, 4, 7, 8, 9 (clip names, join, empty tracks, split here, context menus, markers): protocol 28 and 29, core and screen.
- 2 (overdub): the 「重ねる」 switch; loop passes add to the take, one step per pass.
- 6 (chord help): the chord stamp and the chord table of the key (the piano roll band is 24 taller at both ends).
- The five proposal mocks are in `docs/design/proposals/` (the owner chose Overdub A, ChordTable A, AiLink B).

- 13 (the AI link): protocol 29, the `meloet-ai` socket and `meloet-mcp` bridge with 70 tools (mixer, sounds, vocal audio, note edits and quantize, automation, meters, tracks and clips, save / open / export; audio for the AI goes to a temp folder, and tools that need it answer "not available" when the folder cannot be used), the settings row, the user guide. Research: `docs/research/2026-11-23-mcp.md`; spike: `spikes/s10-mcp/`.
- Later requests of the owner: screen comments and `.claude/agents/` in English; five helpers (light, impl, hard, very-hard, research); the deb opens `.meloet` files (in the running meloet if there is one); the `bump-version` skill and `scripts/bump-version.sh`; `改善ノート.md` deleted.
- READMEs for users in English (`README.md`) and Japanese (`README.ja.md`), in the owner's order: logo, title, overview (with the main features), supported environment and install, quick start, the AI link and its notes (use the AI as a tool for your own music; music data is sent), development, bug reports and contact, license, disclaimer. The development part moved to `docs/development/` (English; `README.md` for every OS, `ubuntu.md`). The picture at the top is the real app (`python3 -I tools/design/shoot_app.py --only 1280x800-100 readmeApp`: six built-in tracks playing C - Am - F - G, the first clip in the piano roll).
- Themes (protocol 30): six themes, WCAG AA (`tools/design/contrast.py`, 172 pairs per theme), the dark table made stronger, key and row names, the hatch on unwritable rows (12 apart, anti-aliased, `color.row.hatch` exactly 3:1), line-only names (the owner's "B plan 2"), gray.6 a little stronger in the dark themes, meter grounds and marks so meters show on the light themes, loop edges gray.8 (architecture 24-15). No brownish colors and softer pastel pink and mint (the owner's choices): light accent blue #014AD1, pinkDark #F9B8CF, pinkLight #853868, mintDark #AAEBD1, mintLight #205F47. The owner's decisions on 2026-11-23 are in spec 1 (source `user:2026-11-23`); the developers' readings in spec 7. Pictures of all six themes: `python3 -I tools/design/shoot_ui.py target/theme-shots --theme=all pianoRoll drumRoll tracks6 mixer settingsAdv ccLanes` (24 s).

Screenshot tools: `shoot_ui.py` shoots in parallel (`--jobs=N`, default 8; 16 got flaky under load), takes several themes in one run (`--theme=a,b|all`) and builds `ui/` only when its inputs changed (`ui/dist/.shoot-stamp`, `--build` forces). Most of the old 18 to 26 s per shot was headless Chrome waiting for the desktop keyring; `--password-store=basic` (also in `render_mocks.py`) removed it with the same pictures. Six themes × five scenes: about 42 minutes before, 39 s now.

Release 0.2.1 (2026-11-23): fix for `meloet-mcp` (the bridge of 「AI とつなぐ」). Claude Desktop starts it without `XDG_RUNTIME_DIR`, so 0.2.0 never looked for the socket and always answered 「…をオンにしてください。」. It now also tries `/run/user/<uid>/meloet/ai.sock`, a failed call adds the step, the paths tried with their errors and how the bridge was started, and `meloet_status` is answered by the bridge while meloet is there. VS Code: "meloet-mcp (debug)". The owner checked the test deb on the real PC and it worked. `scripts/check-all.sh --release` passed; `MELOET_DOCKER_SG=1 scripts/build-deb.sh` built `target/deb/meloet_0.3.0_amd64.deb`: 19 617 852 bytes (18.71 MiB), sha256 `a0805960f16a0e9c1e03e35d7d84d1a0cf0cf7733b33c9f862d9ba315cf7243b`; control ASCII only, `/usr/bin/meloet`, `meloet-plughost`, `meloet-mcp`, the MIME XML and the .desktop are in it.

Release 0.2.0 (2026-11-23): `foundation` was fast-forwarded into `main` after the README picture (the owner's order), then `scripts/check-all.sh --release` passed (Rust 1504 passed, screen 3443 passed), the version went to 0.2.0 (`scripts/bump-version.sh`, 22 files) and passed again, and `MELOET_DOCKER_SG=1 scripts/build-deb.sh` built `target/deb/meloet_0.2.0_amd64.deb`: 19 606 918 bytes (18.70 MiB), sha256 `2b44399ed38a2f46d41b72818921f6a8bebb85afb38ce083bdafe133a902bb2d`. Checked: control ASCII only; `/usr/bin/meloet`, `meloet-plughost`, `meloet-mcp`; the MIME XML and the .desktop with `MimeType=application/x-meloet;` and `Exec=meloet %f`; the icons; LICENSE-MIT, LICENSE-APACHE and NOTICE; no development hook names in the binaries. With the owner's go-ahead: `main` and `foundation` pushed, tag `v0.2.0`, and the GitHub release 「meloet 0.2.0」 with the deb (https://github.com/wintermaples/meloet/releases/tag/v0.2.0; the downloaded asset has the same sha256). The sound pack is not affected: the online catalog is read from `main` (unchanged) and the sounds from their own `sounds-*` tags.

Left for later in round 10: a real-app check of the new features and themes by the owner, measuring key → sound and key → screen while overdubbing, and the light themes showing dark for a moment at start.

### Round 11 (2026-11-23): the AI link, from the owner's 13 requirements

The owner let an AI make music in meloet through the MCP tools and listed 13 things that went badly. Branch `fea/mcp-improvements`; eleven helpers, one part each. How each is built: architecture 24-16 (letters below). Every new and changed tool with its arguments and answers: [`development/ai-tools.md`](development/ai-tools.md). 70 → 94 tools. No protocol change; the plugin host wire is now 7; the song file stays version 6.

1. **Plugin presets**: done (I, J). `list_presets`, `load_preset` (one step), `save_preset` for LV2 presets (through lilv in the plugin host), VST3 `.vstpreset` files and Surge XT patches. VST3 program lists are not read (decision below).
2. **A plugin's whole state**: done (G, J). `get_plugin_state` / `set_plugin_state` (one step; a refused or crashing state leaves the song as it was), Surge XT's patch as XML with `set_plugin_state_text`. The plugin host waits for the audio blocks after a state is put, so a put while the sound runs is one step and the song equals the plugin (before, Surge XT VST3 answered "no step" while the plugin changed).
3. **Many notes with little work**: done (C). Notes as `[start, length, key, velocity]`, `origin`, `create_clip` with `bars` / `end`, `copy_notes` (another track, transposed), `import_midi` and `export_midi` (our own reader and writer, 1920 ticks a quarter note).
4. **Plugin parameters that can be used**: done (H). Choices by name (probing the plugin's texts), `text` in `set_param`, `set_params` as one step, pages and filters. The cause of "no step came within 5 s" was found (DPF plugins take a value on their next audio block) and fixed in the plugin host.
5. **Several changes as one undo step**: done (A). `batch` of up to 200 operations; a failure takes all of it back; the person's intents wait while it runs.
6. **Measuring muted candidates**: done (D). `measure_mix` gains `alone`, `ignore_mute` and LUFS.
7. **What sounds where in frequency**: done (D). `analyze_bands`: levels per band, two tracks sounding in the same band at once, sections by markers or bars.
8. **Which key is which drum**: done (E) for built-in kits, SF2 kits (sample names) and LV2 midnam (checked with Black Pearl); VST3 gives no names (a GM guess is marked as a guess).
9. **Tracks that do not move under the AI**: done (K). Every track argument takes the number or `"id:7"`; answers give ids.
10. **Notes that went missing (266 sent, 265 written)**: done (C). The cause was the 4-bar clip `write_notes` made; now the new clip is long enough, and every write answers `not_written` and `cut` with reasons.
11. **"No change" is not an error**: done (B). `changed: false` as a success; real failures got real reasons (`ai_why_not`).
12. **Finding song files**: done (F). `list_songs` (the song open now, the songs opened or saved lately, the songs folder, or a given folder). Nothing on the screen yet (B-97-4; built in round 12 as A5).
13. **Mute sets**: done for the AI (F). `save_mute_set`, `list_mute_sets`, `recall_mute_set`, `delete_mute_set`; one step each; kept in the song (`muteSets`). Nothing on the screen yet (B-97-3).

New questions for the owner: B-97-1 to B-97-7 in `docs/spec/open-questions.md` (history rows of a batch, of mute sets, of a plugin state and of several parameters; where mute sets and recent songs go on the screen; renaming a mute set). New readings: spec 7, A-28.

Release 0.3.0 (2026-11-23; built on the branch `fea/mcp-improvements`): `scripts/check-all.sh --release` passed before and after the version went to 0.3.0 (Rust 1687 passed, screen 3447 passed, 22 files changed by `scripts/bump-version.sh`). The real-plugin tests of the AI link passed (`conditions` with the sound running, `plugstate`, `presets`, `surge`, `keynames`; Surge XT VST3 and LV2, ZamComp, ZynAddSubFX, Black Pearl). `MELOET_DOCKER_SG=1 scripts/build-deb.sh` built `target/deb/meloet_0.3.0_amd64.deb`: 20 462 852 bytes (19.51 MiB), sha256 `3da24b6bbbd1c8b2b59d08a309d1d8b4169c6623e5b4002bd3e3951d4844b087`; control ASCII only; `/usr/bin/meloet`, `meloet-plughost`, `meloet-mcp`, the MIME XML, the .desktop, 16 icons, the licenses and NOTICE are in it; no development hook names in the binaries. The deb was built from the tree of the commit `Version 0.3.0` (before it was committed). Not checked: the real app on the owner's screen with an AI app connected. Then, with the owner's go-ahead: `fea/mcp-improvements` fast-forwarded into `main`, `main` pushed, tag `v0.3.0`, and the GitHub release 「meloet 0.3.0」 with the deb (https://github.com/wintermaples/meloet/releases/tag/v0.3.0; the downloaded asset has the same sha256). `git push` needed `-c credential.helper='!gh auth git-credential'` (the agent's git has no credential helper; `gh` is logged in).

The owner's completion conditions are tests: requirements 1 and 2 in `crates/meloet-ai/tests/conditions.rs` (ignored; real Surge XT VST3 and LV2 with the sound running), requirement 3 in `crates/meloet-ai/tests/notes_io.rs`.

**The lead's decisions** (one line each, with the reason):

- `get_plugin_params` and `list_effects` give 50 a page by default, so an old call without arguments now gets a page instead of everything: the old answers were about 400 000 and 190 000 characters, too large for an AI app.
- `write_notes` into a track makes its new clip 4 bars (or up to the next clip) and longer when the notes need it, not exactly as long as the notes: an AI that writes a bar at a time would line up 1-bar clips. When another clip is in the way the whole call is refused naming it: all or nothing, as `import_midi` and `copy_notes`, so a batch never succeeds with half a write.
- `ignore_mute` ignores the mute button only; a moving mute lane is still followed: the lane is part of the song as written, the button is the candidate being tried.
- `tracks` and `alone` together are refused: they say different things about the mix.
- Surge XT modulation is edited in the patch XML inside the plugin's state, not over OSC: OSC is off by default and cannot add or remove routings.
- VST3 program lists are not read: on this PC they are empty (Zam) or a single empty name (Surge XT); Zam's presets are a hidden parameter, and the same presets come through the LV2 versions.
- An empty plugin state is an error, and the same state as the song's is "no change" (not a failed put).

**Left for later** (from the helpers' reports; not fixed in round 11):

- Batch
  - The history row of a batch is the last operation's name (B-97-1).
  - `BatchLost` can still come from inputs that are not held but make a step: an automation write while playing ending in `sync_automation`, and a song handed over at start opening on `ScreenUp`.
  - When the core thread stops for more than 2 s (for example while an output opens), a live batch is taken back by the idle limit; the tool then says nothing was changed.
  - No limit on the time of a whole batch.
  - A held intent's answer waits until it has run; whether Tauri's synchronous commands run on the main thread was not checked (if so, the window could freeze up to 2 s in the worst case).
  - Not checked in the real app with a person working while a batch runs.
  - Old doc comments: `AiAsk::InBatch` / `Rollback` in `crates/meloet-core/src/ai.rs`, and `HistoryView` in `meloet-protocol` ("only the last row changes"; now rows above the lower of the two `current` may change when `newest` changes).
  - Intents of the screen path still change the selection when the AI sends them (`MoveClips`, `DuplicateClips`, `SplitClipAt`, `JoinClips` choose the clips they make; `timeline_step` chooses tempo points, meters and markers; `AddVocalTrack` changes the selected track), against "the AI does not change the selection". Their results also show on the person's status line (`TrackDeleted` and so on).
  - `EditError::Unchanged` of the model still mixes "cannot" (`join_clips` with one clip, other tracks or a clip between; `split_clips` outside the clip; empty names); `ai_why_not` and the tools give the AI the right reason, the model was not changed.
  - `play/params.rs` still answers an empty or non-finite plugin value with `AiRefusal::Unchanged` (which the tools now show as a success); the tools refuse such values first, but it should be `Cannot`.
- Notes and MIDI
  - Not tried with MIDI files written by other DAWs; track names in Shift-JIS come out garbled; a note fully inside another of the same key swaps ends on a round trip (a limit of the format).
  - `import_midi` / `copy_notes` with another clip in the way refuse; they do not spread the notes over several clips.
  - With 200 notes listed, an answer is about 30 KB (text and `structuredContent` both hold the JSON; `MAX_TOLD`).
  - The `anyOf` note schema was not shown to a real AI app.
  - `AiNoteWhy::PastClipEnd` and `at_clip_end` no longer come on the AI path.
  - No test for a next clip that starts in the middle of a bar with notes ending before it.
  - Sending a take of 50 000 notes to the screen was not measured.
- Measuring
  - Each measuring pass still writes a temporary WAV and deletes it (600 s × 65 passes would write about 15 GB).
  - Overlap sums left and right, so two tracks panned apart still count as overlapping; no true peak.
  - Not checked through the real app and the `meloet-mcp` bridge; 44.1 and 96 kHz only in unit tests.
- Key names
  - VST3 `getProgramPitchName` is not built (no VST3 drum plugin that uses it on this PC).
  - The midnam update feature is not offered; with several name lists, no choice by patch or channel.
  - Built-in kit keys from a loaded bank are checked only by unit tests (no pack in the tests).
  - Answers a tool gave up on stay until the next ask (as `ai_lists`).
  - Idea: the SF2 reading thread could make the key names with the bank (no second open, names always match the sound).
- Mute sets and recent songs
  - Nothing on the screen (B-97-3; the recent songs came in round 12, A5); no rename tool (B-97-5).
  - A set does not remember whether a lane was resting.
  - A song restored from the recovery file is not added to the recent songs.
  - meloet 0.2.x opens a song with mute sets but drops them when it saves.
  - `deleted_track_ids` are plain integers, not `"id:N"`.
- Presets
  - Loading the same LV2 preset again after the song was opened again ends in "says it has the state it had" (the in-memory record is gone); a typed outcome of a state put would fix it.
  - An LV2 state without `appliesTo` put through `set_plugin_state` may make the plugin host panic (in livi's `restore_state`; not tried).
  - LV2 presets that point to files (samples): relative paths not checked.
  - The class id letter form of `.vstpreset` checked only against our own round trip and vst3-host 0.11.1; no files from other hosts on this PC.
  - ZynAddSubFX preset names come as the bundle gives them ("Bass: 0001-Bass 1.xiz").
- Surge XT
  - On LV2 with the sound running, a put whose result equalled the song's state still made a step (suspected empty step; not looked into).
  - `set_plugin_state_text` with attributes Surge XT does not write back (such as `deactivated` on `a_pitch`) ends in "says it has the state it had".
  - Routings with `scene` on parameters of the whole patch were tried only in unit tests; `source_index` is always 0.
  - After a patch swap on LV2, whether `get_plugin_params` shows the new patch's values was not checked.
- Parameters
  - `Handle::touch` in `crates/meloet-plughost/src/rt.rs` does not ring the bell when it leaves a seat (as `params_taken` did before its fix).
  - VST3 values set in blocks while the gate is closed are lost (`vst3.rs`, around line 746).
  - LV2 ports that are continuous but have scale points get no names.
  - `list_instruments` has the same size problem as `list_effects` had.
  - Each plugin set reads the whole list once (about 100 ms for Surge XT); a cache would help.
  - `changed_only` lists about 320 parameters of a Surge XT just loaded (its first patch differs from the plugin's defaults).
- Plugin host
  - `Hosts::request` in `crates/meloet-core/src/plugins/hosts.rs` answers `Gone` on a broken read without killing the process, so no `Stopped` comes: the AI is told the plugin was reloaded, but it is not (one `pidfd.kill()` would fix it).
  - Esc on a panel drag that started during a state put puts the old value into the plugin (a window of a few ms).
  - The 2 s revive window also reloads plugins of the same key that stopped for another reason.
  - A large plugin state makes the recovery file large on every change.
  - A Surge XT VST3 state cut in half crashes the plugin host (the song is safe; the instructions to the AI say to put back only states it read).
  - A state put answers `step: null` (the step comes later through a ticket); there is no way yet to hand the row to the AI.
- Tracks
  - The id is turned into a number inside the tool; many core asks take numbers, so a change in that short time can still move them. `add_track` reads the song again after the write for its `track_id` (a `track_id` in `AiDone` would be exact).
  - `add_vocal_track` answers no track number or id.
- Tests that fail now and then
  - `readahead::tests::other_audio_for_the_same_clips_comes_in_over_ten_ms_where_the_ring_ends` (2 of 5 runs on a busy PC: 3 swaps where 2 are waited for).
  - `link.rs` `a_note_is_rendered_into_the_private_folder` and `the_export_writes_over_a_file_only_when_asked` failed once right after a build, then passed 7 times.
  - `tests/common` `scratch` in `crates/meloet-ai` does not remove its folders: `/tmp/k10ai2-link-*` piles up (619 seen).

### Round 12 (2026-11-23 to 24): the owner's list of features

The owner's request of 2026-11-23 (`docs/history/round12-spec.ja.html`; it calls itself round 11, read it as 12): 33 items in four waves, with the owner's decisions at its head (looks and wording as written, source `user:2026-11-23`; one branch; no merge into `main`, push, release or publishing unless asked). Each wave first got its shapes in one commit (protocol 31, 32, then 33 for waves 3 and 4 together; telemetry 5; song file 7 and recovery file 3 in wave 1), then the features in parallel in helper worktrees. How each is built: architecture chapter 25 (25-4 to 25-35). The rules: spec 3, "Rules added in round 12". Readings and every text the developers made up: spec 7, A-29. Questions: B-98-1 to B-98-34.

**Branch**: `fea/round12` (from `main` at 0.3.0). **Not merged into `main` and not pushed** (the owner decides). The mp3 work is on `fea/round12-f2-mp3`. App version still 0.3.0.

**Built** (31 features and D3's documents): G1 the key list and the user guide link, A3 duplicate a track, A5 recently opened songs, R3 metronome settings and tap tempo, P1 scale highlight and nine scales, P3 velocity tools and legato, P6 chord progressions, A9 reverse and normalize, F1 loudness of the export, E9 「書き出しました」, D1 the new-version notice and the apt repository script, D3 the Windows / macOS study (documents only, `docs/research/2026-11-23-windows-macos-port.md`), R1 keep what was just played, R2 play with the PC keyboard, P2 other tracks' notes faintly, E1 sounding notes light up, P4 drum patterns, A1 repeat clips, M1 effect presets, M6 gate / de-esser / saturation, M2 「音が大きすぎます」 (and the analysis tap), F2 song information in the export, E2 arpeggio, E3 the knobs of built-in sounds, E7 the spectrum in the top band, M3 finishing meters and mono, A2 whole sections, A7 keys into audio, M5 the master's volume lane and fade-out, A8 comping, M7 pitch toward the scale, D2 the AppImage trial. D4 (English wording) came last: `ui/src/text/index.ts` and `en.ts`, 「表示 ▸ ことば / Language ▸」 (a setting, not a step, refused while busy), a change reloads the screen, new songs and tracks are named in the language of the moment, `shoot_ui.py --lang=en`, `docs/user/README.en.md`.

**After the features** (2026-11-24): the measurements on a quiet PC are in `docs/measurements/2026-11-24-round12.md` (latency, recorded positions, export = playback all hold; E7 holds the redraw rule). E1 broke the redraw rule with 10 000 notes under `webkit-damage.sh`; the lit box now stays in place while its notes sound, the default stays on, and the question of the tool is B-98-33. At the edge: P2 scroll, M3 with the mixer open, the export's speed (B-98-34). D4 was checked only with the fake and unit tests: the reload in the real app, the GTK window titles and the choice from the real environment were not tried; all scenes were loaded in both languages and no text is cut only in English (about 45 scenes looked at by eye). Not measured: the click sounds other than the default, the seams of comping, the delay of typed keys, the arpeggio. Open questions of the round: B-98-1 to B-98-34.

**Not shipped**: **mp3** (F2). Built with `rusty_mp3` 1.2.2, but the acceptance condition "20 dB below the source at 192 kbps for pink noise" failed (14.4 / 13.4 dB; LAME 3.101 13.7 / 12.8 dB on the same signal, so the rule may be too strict). The owner decides (B-98-1). **The AppImage** is built but must not be published until the LGPL source offer and the patched WebKit library are settled (B-98-2). The apt repository is not published (the owner publishes; apt instructions go into the READMEs after that).

**Measurements**: done once at the end on a quiet PC, in [`measurements/2026-11-24-round12.md`](measurements/2026-11-24-round12.md). Helpers measured nothing (the PC was not quiet); what each feature asks to be measured is in its architecture section and in that file.

**To listen to** (nothing of these was heard by a person yet):

- E2 arpeggio: `target/r12-e2-ear/` (shapes, rates, hold, stopped, note repeat).
- E3 knobs: `target/r12-e3-ear/` (grand piano, synth lead, studio kit, electro kit; each knob at its ends).
- M6 gate, de-esser, saturation: `target/r12-m6-ear/` (the de-esser on a made-up voice, not a real one).
- M7 pitch toward the scale: `target/r12-m7-ear/` (speech and singing, dry, 50 %, 100 %).
- F2 mp3 (the side branch): `target/r12-f2-ear/` (192 and 320 kbps against the 24-bit wav).
- R3 「カチ」 and 「カウベル」, M1's 24 factory presets (also switching while playing), P4's twelve patterns: in the app only.
- The AppImage: `target/appimage/meloet_0.3.0_amd64.AppImage` (not to be published).

**The lead's decisions** (the reasons are in spec 7 A-29, "How the round was run"): protocol 31 → 32 → 33 (waves 3 and 4 share 33); notes files per feature instead of editing the shared documents, merged at the end; measurements once at the end; a new control folds away before any part the spec already shows (P2's label and R1's button are hidden at 1280 px); P6 reads Dorian and Mixolydian in the major with the same notes; `ebur128`'s FTZ inside its own call on non-audio threads is accepted (architecture 19-2); D2's line for the release skill was not added (the helper had no permission; the text is in A-29 for the owner).

**Left for later** (from the helpers' notes; not fixed in round 12):

- Not checked in the real app by hand: almost every feature was checked with Rust tests, screen tests and fake scenes in six themes only. Worth trying first: the arpeggio and typed keys (with Japanese input on), comping on real takes, a fade-out exported, the finishing meters with a real song, 「キーで弾く」 when another window takes the keys.
- Sound and engine
  - The master limiter's gain does not come back to exactly 1 after it has held the sound down (`gain += step × (1 − gain)` stops one step under 1 in f32, `effects/limiter.rs`), so two plays of a song that drove the limiter differ in the last bits.
  - An effect that follows the tempo (the delay with 「テンポに合わせる」) reads the tempo per rendered stretch, and stretches are cut at the next note of every track: inside a tempo ramp it sounds slightly different solo than in the mix (found by A7; up to 5e-3).
  - A track's lanes deleted (or rested) while no stream runs stay in the engine until that track's lanes change again (`Audio::op` drops automation without a stream; the master's lane is always sent now, the tracks' are not).
  - A hand-set pan under a moving volume lane jumps within 32 frames instead of 20 ms (since round 6; now also the master).
  - Knob lanes and effect lanes glide 20 ms at a jump of the play line while playing.
  - The arpeggio: `stream_gone` does not call `drop_after` (a take can be a note or two longer); the settings' measured latency counts MIDI NoteOns that the arpeggio delays; no look-ahead for tracks whose plugins delay the sound.
  - The saturation aliases at high drive on high notes (no oversampling, by the no-delay rule). The gate reads single frames (a sound with tops more than 30 ms apart would open and close). The EQ still has its own copy of `dsp::Svf`.
  - The spectrum at 96 kHz barely tells the lowest bands apart and shows a DC offset in the lowest one; the analysis (spectrum, M3's loudness) runs while the screen hides it (B-98-26). M3's numbers start anew after a ring overflow even after a stop; a loud key in the 10 ms before 「再生」 can reach the play's true peak; ebur128 keeps 3 s of frames (2.6 MB at 48 kHz) while the mixer is open.
  - M7: octave or fifth mistakes are not repaired; the pitch is listened for again on every strength change (a cache would help); every tuned clip goes through the phase vocoder whole.
- Audio files
  - `AudioTakeView` has no sample rate, so a take of another rate is drawn at the wrong scale in the comp band and the clip (the sound is right).
  - A comp change while playing a stretched clip can be heard up to two seconds late (as T on a stretched clip).
  - No progress and no stop for a long 「逆にする」; the keeper thread does one ask at a time (imports wait); no message when a file cannot be read or written (`clipWorkFailed` has no wording, B-98-29).
  - A `.bounce-*.wav` can stay in the work place if meloet ends between the job's move and the step; a bounce is not dropped at once when the song is replaced.
  - Without the mp3 branch, meloet cannot open an mp3 whose ID3v2 tag is over 1 MB (symphonia's `id3v2` feature is off; pre-existing).
  - The song's title and artist have no length limit.
- Screen
  - The inspector's 「そろえる」 buttons are cut at the right edge of the 240 px panel at 1280 px (pre-existing); P3's new rows are below the fold at 150 %.
  - The mixer's master strip is cut at the right at 150 % in 1280 × 800 (pre-existing).
  - The repeat count box has no `ValueField` (out-of-range counts are put at the end without words), nor do P3's 「割合」 and 「ばらつき」 (the shapes exist, B-98-22).
  - The top band has 3 px to spare for the spectrum at 1280 × 800 100 %: anything added there hides it.
  - Lists moved to `body` (`ChoiceMenu`, zoom lists) stop key events with `stopPropagation`, which does not stop the document key handler on the same node (R3 found it; not checked whether it does harm). `ChoiceMenu` lists do not scroll (more than about 25 tracks in P2's list).
  - E1: a CC lane drag while playing shares the live canvas with the glow; the eraser's hover mark is covered by a lit note; `StatusLine.svelte` still makes a `Set` per frame for 「鳴っている音」. `GLOW_AREA` may need tuning after `webkit-damage.sh`.
  - The fake: no undo for most new intents; M3's mixer scenes show 「−∞」; the loop summary is not made again after A2 moves the loop; A1's selection after a repeat is not core's.
  - The mixer band's tab shows the master's hand value also while its lane moves it. Selected points on a volume lane are lost after 「下げる」 (the lane gets a new id). A very long folder path makes the 「最近ひらいた曲」 list wide.
  - The key list (`keyList.ts`) is written by hand; nothing checks it row by row against spec 4.
  - While 「meloet について」 or 「キーの一覧」 is open, one-letter keys still work (as before for About).
- Core and the AI link
  - `screen_allowed` (the screen intents the AI may send) has `DuplicateClips` but not `RepeatClips`; no new AI tools were made in this round (presets, arpeggio, comps … are screen only).
  - `TooManyTracks` is said by three ways of adding a track but not the shelf's or 「＋ 歌のトラック」 (B-98-27).
  - A duplicate waits while a plugin of the track hangs (up to 5 s); a second 「トラックを複製」 meanwhile does nothing.
  - `Recent` items dropped for ring headroom are not told to the core; a keys track still loading its instrument keeps keys that made no sound.
  - `repeat_clips` compares every copy with every clip of its track ((clips × 99)² at worst; not measured).
  - A2: whether a phrase waiting to be kept (R1) should move with a section was not looked at; `Doc::check_lanes` order of the master's lanes; a helper for "apply and set the selection after" would remove repetition.
  - `ParamFile::Step` writing indexes the names with no bound check (values are clamped first).
  - Tools: `tools/measure/*` and `shoot_app.py` cannot start an AppImage; there is no tool yet for key → sound of a typed key.
- D1 and D2: the real GitHub answer and the real app against a test server were not tried; the AppImage was not tried on other distributions, AMD / Intel graphics or X11 sessions, nor with the sound pack download or the first-start chooser.

## 7. Notes for Windows and Mac

From the end of round 8 (only what was noticed).

- **Linux-only parts** (replace when moving): keyboards through the ALSA sequencer (`meloet-midi`); output and input names from PipeWire Routes (`meloet-devices`); the futex and shared memory between meloet and the plugin host (`meloet-plugin`); plugin windows on X11 (`crates/meloet-plughost/src/window.rs`); audio thread priority through `sched_setscheduler` and rtkit (D-Bus; Windows uses MMCSS, Mac the thread time constraint); `malloc_trim` (glibc) to give memory back; `statvfs` for free space; XDG places (replace with `dirs`); the logind delay lock; the web process watcher (`crates/meloet-app/src/webprocess.rs`); `pidfd`. The PC file window is GTK's `FileChooserNative` (arch 10).
- **Screen**: the redraw rules of arch 7 (11 rectangles, 25 %, `cover`), `WEBKIT_DISABLE_COMPOSITING_MODE` and `JSC_useGenerationalGC` belong to WebKitGTK. Measure again on WebView2 (Windows) and WKWebView (Mac). The limit on WebGL contexts is different too.
- **Packages**: `icon.ico` and `icon.icns` are made. The identifier `dev.meloet.app` gives a warning on macOS ("ends with .app"; change it). On Mac the menu bar moves to the top of the screen (spec 3, Y11). Signing and notarizing (Mac), an installer (Windows). Build the stretch C++ (Signalsmith) with MSVC and clang.
- **Sound pack**: the fetcher is ureq + rustls + ring (works everywhere). Certificates come from `rustls-platform-verifier` (on Windows and Mac it reads the OS's store).
- **Development hooks**: `tools/measure/build-dev.sh`, `scripts/check-release.sh`. Many measuring tools depend on PipeWire, gdb and X.

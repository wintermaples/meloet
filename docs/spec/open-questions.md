# Open questions

This is the list of things for the owner to decide. Each item says what is asked, what the screen does now (the developers' provisional form), and where it comes from. When an item is decided, put the answer into the body of the spec (chapters 1–7) and remove the item from here.

The numbers are the B numbers of [`docs/history/design-notes.ja.md`](../history/design-notes.ja.md) (B-95 = round 8, B-94 = round 7). Code comments point to them, so keep them. Questions that already have an answer are not listed; their record is in that file (part C).

## B-95: round 8 (after version 12)

### B-95-1 The order of fetching the sound pack

- Question: spec 3 (45-A) says 「グランドピアノ」, 「スタジオキット」, then the hurry order of Y2. The 21 sounds added later have no place in that order.
- Now: first the sounds of the Y2 hurry order that are in the pack (「グランドピアノ」, 「スタジオキット」, 「フィンガーベース」, 「ストリングス」, 「エレクトリックピアノ」, 「オルガン」, which is now the pipe organ; the synth sounds are made by computing and are not fetched), then the order of the shelf (the order of the groups, then the order in each group). Reason: the order seen from the top of the shelf and the order of fetching are the same.
- From: the developers' provisional decision ([chapter 7](7-readings.md), A-24 item 4). Not in the list sent back at the end of round 8. Code: `crates/meloet-core/src/sounds/fetch.rs`.

### B-95-2 How to count the places in 「3 か所」 (export clipping)

- Question: the 41-A text 「0 dB をこえる所が 3 か所あります（…）」 in spec 3 does not say how places are counted.
- Now: a run of frames over 0 dB is one place; when 100 ms not over 0 dB follows, the next run is a new place. Reason: counting every wave peak inside one note gives too large a number; this is close to what a listener hears as one place. The dialog row is the 41-A text; the spec 6 scene "Export would clip" is read as the status line when clipping is found after export (chapter 7, A-24 item 8).
- From: the developers' provisional decision (A-24 item 8). Code: `OverPlaces` in `crates/meloet-core/src/export/job.rs`.

### B-95-3 「プラグインの窓」 twice in a history row

- Question: for a plugin window step, the (what) is 「プラグインの窓」 (spec 3, 94-6) and the right column is the spec 6 text 「5 Surge XT ・ プラグインの窓」. The same words appear twice in one row. Should the right column be only 「5 Surge XT」?
- Now: both as the spec says (the spec is right until it changes).
- From: A-24 item 11.

### B-95-4 Where the arrange starts track rows while the tempo lane is open

- Question: the table in spec 3 says the arrange becomes track rows from 136 (the 3 ruler rows, 72, + one track, 64). When the tempo and time signature lane (48) is open, it pushes the tracks down inside the arrange, so at 136 the inside of the track was only 21 px.
- Now: while the tempo lane is open, track rows start from 136 + 48 = 184; under that, the overview (`layout_with` in `crates/meloet-core/src/layout.rs`).
- From: the developers' provisional form (made when fixing 94-92).

### B-95-5 The text while moving a fade-out

- Question: the spec 6 scene "Moving a fade" has an example only for the fade at the start (「前のフェード」).
- Now: for the fade at the end, the word 「後」 from the panel of mock F7-AudioEdit: 「歌 テイク 1 の後のフェード …」.
- From: the developers' provisional form.

### B-95-6 Enter on an empty 「手で決める…」 box

- Question: the spec does not say what happens.
- Now: an empty box stops using the number set by hand (back to the number the PC reports + the measured number). Text that cannot be read closes the box and sends nothing. A number outside 0–500 is set to the nearest end without a word (the number field rule in spec 3 says the status line tells; this place does not follow it yet).
- From: the developers' provisional form.

### B-95-7 The two numbers in 「まだ 12 / 31」

- Question: spec 3 (the place where the sound pack can be fetched later) does not say which numbers these are.
- Now: "the sounds there / all the sounds" (the same reading as 「92 / 240 MB」 in the progress). The 5 sounds made by computing (they always play) count as there; 31 is the number of instruments on the shelf.
- From: the developers' provisional form.

### B-95-8 A selection with keyboard clips in it

- Question: 40-A in spec 3 is for "two or more audio clips". It does not say what happens when keyboard clips are also selected.
- Now: if even one keyboard clip is in the selection, the 「まとめて変える」 panel is not shown, and the panel of the current selection stays.
- From: the developers' provisional decision. Not in the list sent back at the end of round 8. Code: `crates/meloet-core/src/play/audio.rs`.

### B-95-9 The pitch field when two or more clips are chosen

- Question: 40-A does not give the form of the pitch field when the clips differ.
- Now: when both the semitones and the cents differ, only 「—」 (the mock's form); pressing twice opens a box to type the semitones. When only one of them differs: the form 「— 半音 20 セント」.
- From: the developers' provisional form (not in the spec).

### B-95-10 A 32-bit wav that goes over 0 dB

- Question: 41-A does not cover a file that goes over 0 dB but does not clip (32-bit float does not clip inside the file).
- Now: 「いちばん大きい所 +0.4 dB ・ 音割れはありません」.
- From: the developers' provisional form (not in the spec).

### B-95-11 The sound pack server answers 404, 5xx, or 403 again and again

- Question: the protocol has four kinds of failure: `Offline`, `CutOff`, `DiskFull`, `Corrupt`. None of them is for a server that answers with an error.
- Now: the PC is online, so it uses the `CutOff` text (「もう一度」 fetches again). Other write errors (for example, no permission) are the same.
- From: the developers' provisional form.

### B-95-12 The hints of flows 1–4 and asking about the sound pack

- Question: spec 3 (43-B) says that when the hints of flows 1–4 are there, asking about the sound pack comes after them. It does not say what to do with hints that stay for a long time.
- Now: while the output chooser (flow 2) is open, it waits, then asks in the input meter band. The hints in the arrange (`SoundCameOut`, `EmptyArrange`) stay until a clip is made, so it does **not** wait for them (otherwise a new user is not asked until the first take; mock F8-PackAsk also asks with an empty arrange). While a hint that stays up is shown (no keyboard, the keyboard was pulled out), the asking moves to the status line too (like the 44-B form: the text with 「取りにいく」 and 「あとで」). Reason: nothing is fetched until the user answers, so with no place to ask, the user stays with the sine wave.
- From: the developers' provisional form. Code: `ui/src/components/pack/pack.ts`.

### B-95-13 Small points of the sound pack screens

- Question: these are not in the spec or the mocks.
- Now:
  - The second line of the band when fetching failed: when offline, the mock's text; for the other three failures, the spec 6 text.
  - In the slim input meter (when a lower band is open), the two lines become one.
  - In the split form of a vocal track, the hint band comes first, and the mic meter is not shown.
  - In the smallest window, the small text is the short text of mock R12-Fit; if it still does not fit, its end is cut with 「…」 (the bar and the buttons are not hidden; the bar stays 160).
  - During export, recording, and count-in, the 44-B form is not shown.
  - While fetching, 「取りにいく」 in the advanced settings and in the shelf notice is disabled.
- From: the developers' provisional form.

### B-95-14 The size text of the pack

- Question: the spec uses 「240 MB」, but the real pack is 238,909,440 bytes (239 MB as it is).
- Now: the size of the whole pack (「約 240 MB」, 「ぜんぶあります ・ 240 MB」) is rounded to 10 MB. The progress (「92 / 239 MB」) shows the number as it is. While fetching, the shelf notice of missing things (85) is not shown (it is not in mock F8-PackGet, and its meaning is different).
- From: the developers' provisional form. Code: `packMb` in `ui/src/components/pack/pack.ts`.

### B-95, other points

Sent back at the end of round 8 with the items above:

- The status line after 「やめる」 while fetching the sound pack: it says nothing now.
- The name of the folder for the sound pack: the mock says 「音の素材」; the code uses `sounds`.
- The count of 「お気に入り」 on the shelf while fetching: sounds not fetched yet are not counted, so it is 0.
- The value text of pan and send in the mixer: the spec says every frame (the version 12 answer to 92), but it is updated 15 times a second, with the knobs, because of the cost of drawing again on the CPU path.

## B-94: round 7 (no answer in version 12)

### B-94-19 The text 「測った数 ・ 12.4 ms」

- Question: the text for the measured latency in the sound settings is not in the spec.
- Now: 「測った数 ・ 12.4 ms」 (`builder:2026-11-21`).
- From: the developers' provisional text.

### B-94-33 Where 「SF2 は 1 つ棚へ …」 goes when the box is split

- Question: when SF2 and audio files are dragged in together and there are two or more audio files, the box is split left and right (30-C). Spec 3 says the box in the middle shows 「SF2 は 1 つ棚へ ・ 音のファイル 3 つはアレンジへ」, but there is no middle box in the split form.
- Now: a third row, 480 wide, under the split boxes.
- From: the developers' provisional form.

### B-94-81 The export status line before the time left is known

- Question: spec 6 "Exporting" has 「残り 約 40 秒」, but in the first few blocks the time left is not known yet.
- Now: 「残り 約 …」 is taken out: 「{file} ・ 弾けます（書き出しの音は聞こえません）」 (`builder:2026-11-21`). The time left is 「N 秒」 under one minute (at least 1), and 「N 分」 (rounded) otherwise.
- From: the developers' provisional text.

### B-94-83 The range button in a song with no loop

- Question: the export dialog has a range button for the loop. Its form in a song with no loop is not given (the other parts of 83 were answered by 41-A).
- Now: the button text is 「ループ」 with no numbers, in the disabled form.
- From: the developers' provisional form.

### B-94-86 The status line when the user chose real-time export

- Question: spec 6 "Exporting in real time" is for when a plugin cannot export fast. There is no text for when the user turned on 「実時間で書き出す」.
- Now: 「実時間で書き出しています（3:02）。」 (`builder:2026-11-21`). When a plugin is the reason, the spec 6 text.
- From: the developers' provisional text.

## B-96: round 10 (found when the readings moved into chapter 7)

### B-96-1 Where the spec 6 text "Export would clip" is shown

- Question: spec 6 puts 「いちばん大きい所が 0 dB をこえています。このままだと音が割れます。マスターを下げてから書き出すのがおすすめです。」 in the export dialog, with the buttons 「このまま書き出す / やめる」. The check row of the dialog uses the 41-A texts of spec 3 instead.
- Now: the dialog shows the 41-A texts; the spec 6 text is used on the status line when clipping is found only after the export (chapter 7, A-24). The spec body and the screen do not agree.
- From: the developers' reading (A-24 item 8).

### B-96-2 The third sentence of the empty arrange hint

- Question: the third sentence of the F-Empty hint still says 「トラックを増やすには左下の「＋ トラックを追加」」, but spec 3 (93-1) replaced that button with 「＋ 鍵盤のトラック」 and 「＋ 歌のトラック」.
- Now: the old sentence is shown (only while a track can be added). `ui/src/components/SoundHint.svelte`.
- From: chapter 7, A-13.

### B-96-3 The name of key 47 in the built-in drum kits

- Question: the built-in kits call key 47 「ミッドタム」, but the table of spec 3 for SF2 kits (GM) calls 47 「ミッドタム 2」 and 48 「ミッドタム」.
- Now: the built-in kits use 「ミッドタム」 for 47 (`crates/meloet-model/src/drums.rs`).
- From: chapter 7, A-11.

## B-97: round 11 (the AI link)

### B-97-1 The history row of a batch

- Question: the AI tool `batch` makes several changes as one undo step. What should its row in the undo history band say?
- Now: the name of the last operation inside it. A mix of 15 operations shows one row such as the volume change of track 3, which does not tell what is inside.
- Proposal (helper): a new row wording such as 「まとめて変える（N 個）」. It needs a new step name (`StepKind`, `StepName`, `ja.ts`; a protocol change).
- From: architecture 24-16 A.

### B-97-2 Step names of mute sets, and the word 「セット」

- Question: saving, recalling and deleting a mute set are one step each. What should their rows say? And is 「セット」 the right word, or 「組み合わせ」?
- Now: all three show the step name 「ミュート」 with the set's name as the (what), place 「ミキサー」.
- Proposal (helper): 「ミュートのセットを保存する」, 「ミュートのセットを呼び出す」, 「ミュートのセットを消す」 (and 「ミュートのセットの名前を変える」 if renaming is built).
- From: architecture 24-16 F. Code: `crates/meloet-core/src/play/history.rs` (a comment marks the open question).

### B-97-3 Where on the screen a mute set is chosen

- Question: mute sets can only be used by the AI now. Should the screen have them, and where? Should saving, renaming and deleting be possible from the screen too?
- Now: nothing on the screen.
- Proposal (helper): the menu at the head of the mixer band, the 「表示」 menu, or the context menu of a track head.
- From: architecture 24-16 F.

### B-97-5 Renaming a mute set

- Question: is a tool (and later a screen way) to rename a mute set wanted?
- Now: not built (it was not in the list asked for). A set can be saved again under a new name and the old one deleted.
- From: architecture 24-16 F.

### B-97-6 The history row of a plugin state set by the AI

- Question: `set_plugin_state`, `set_plugin_state_text`, `load_preset` and the Surge XT tools put a whole plugin state as one step. What should the row say?
- Now: 「プラグインの窓」 (`StepKind::PluginWindow`), as a change made in the plugin's window, so no new wording was made.
- From: architecture 24-16 G.

### B-97-7 The history row of several plugin parameters set at once

- Question: `set_params` sets several parameters of one plugin as one step. What should the row say?
- Now: the row of the first parameter (`SetPluginParam`). The model has no step name for "several parameters".
- From: architecture 24-16 H.

(B-97-4, 「最近ひらいた曲」 in the file menu, was answered by the round 12 request: built as A5, spec 3 "Rules added in round 12".)

## B-98: round 12 (the owner's request of 2026-11-23)

The readings behind these are in [chapter 7](7-readings.md), A-29; how each is built is architecture chapter 25. Nothing below was tried by the owner yet; "heard" means listened to by a person, which no helper did.

### B-98-1 F2: mp3, ship or not

- Question: mp3 export was built with `rusty_mp3` 1.2.2 but failed the request's acceptance condition 2 ("at 192 kbps the difference is 20 dB or more below the source, for a 1 kHz tone and pink noise"): pink noise came back 14.4 dB (44100 Hz) and 13.4 dB (48000 Hz) below. LAME 3.101 gives 13.7 / 12.8 dB on the same signal, and a rough estimate says no encoder can reach 20 dB on independent pink noise at 192 kbps, so the rule itself may be too strict. Conditions 1 (length) and 3 (odd signals, cancel) passed. Choices: keep mp3 hidden; change the rule (for example a tone and a real song only, or 256 kbps) and ship it; or ship it as it is.
- Also to decide if it ships: `rusty_mp3` reads 11 environment variables in a normal build (`MP3_RESERVOIR`, `MP3_RESV_GAIN`, `MP3_RESV_LOOKAHEAD`, `MP3_STEREO`, `MP3_PICK`, `MP3_SHAPE`, `MP3_PSY_DOMAIN`, `MP3_SHORT_SHAPE`, `MP3_THR_CAP`, `MP3_SMR_DB`, `RFF_MP3_XRPOW`), so a person's environment could change the files; its own notes name a bug when the stereo mode switches nearly every frame, and its reservoir is off at 320 kbps.
- Now: mp3 is not offered. The whole work (with tests and two scenes) is on the branch `fea/round12-f2-mp3`. Listening files: `target/r12-f2-ear/` (`test-song-192k.mp3`, `test-song-320k.mp3`, `test-song-24bit.wav`).
- From: architecture 22-11, 25-25; chapter 7 A-23 (C-11).

### B-98-2 D2: the AppImage before any distribution

- Question: the AppImage holds about 170 Ubuntu libraries under the LGPL and libfuse in its runtime; publishing it needs a source offer for them from the same place. It also holds `libwebkit2gtk-4.1.so.0` with bytes changed by linuxdeploy-plugin-gtk (`/usr` → `././`), a change to an LGPL library that has to be stated. How should these be done, and should the AppImage be published at all?
- Now: built as a trial only (`target/appimage/`, `scripts/build-appimage.sh`); not published; the READMEs say nothing about it.
- From: architecture 23-5, 25-35; `docs/development/appimage.md`.

### B-98-3 P2: the label of 「ほかのトラックも見る」 at 1280 px

- Question: at 1280 × 800 (100 % and 150 %) and 1920 × 1080 at 150 % the label does not fit next to the clip text, so it is hidden and the switch is only the first row of the ▾'s list. Is that right, or should the row show the label and give up the clip text or the key text?
- Now: the label hides first; pictures at 1280 × 800 are as before round 12 but for the ▾.
- From: chapter 7 A-29 (P2, "How the round was run").

### B-98-4 R1: the button 「さっきのを残す」 at 1280 px

- Question: at 1280 px wide the button is hidden (the feature is reached by Shift R and 「再生 ▸ さっきの演奏を残す」). Is that right?
- Now: it shows only when everything else of the top band shows in full (1920 × 1080).
- From: chapter 7 A-29 (R1).

### B-98-5 R2: 「キーで弾く」 in small windows, and what was not checked

- Question: in the input meter the switch and 「オクターブ C3」 take only left-over room; after E2 at 1280 × 800 at 100 % the arpeggio button takes it and 「キーで弾く」 is not shown; at 150 % neither. Is that right?
- Not checked on the real app: that WebKitGTK hands the key requests to Rust in the order they were made (a release before its press would leave a note sounding until the key is pressed again or the switch goes off), how the input method's keys come (`isComposing`, `Process`), that `KeyboardEvent.timeStamp` is on the clock assumed, and that blur and `visibilitychange` come when another window takes the keys.
- Now: Mod K, the menu row and the status line always work.
- From: architecture 25-17; chapter 7 A-29 (R2).

### B-98-6 E2: arpeggio — stop, small windows, the mixer strip

- Question 1: should stop also end a held chord (「指を離しても続ける」)? Now it goes on after stop, at the tempo (one line would change it).
- Question 2: at 1280 × 800 at 150 % the button 「アルペジオ ▾」 does not fit in the input meter, and there is no menu row or key, so the arpeggio cannot be turned on there. Add a way?
- Question 3: on the mixer strip 「アルペジオ」 sits at the right end of the heading only while on (the track name is cut to about one character on a 108 px strip). The other way is an empty row on every strip, which moves everything below down by about 32 px. Which?
- Listening files: `target/r12-e2-ear/`.
- From: architecture 25-26; chapter 7 A-29 (E2).

### B-98-7 M2: the thresholds and the number in 「音が大きすぎます」

- Question: are 4 s, 3 dB of reduction for 2 s, and 3 readings over 0 dBFS sensitive enough on real songs? The limiter sentence says the mean of the reduction over the time it held 3 dB or more (so 「ずっと 6 dB」 means what it says); the protocol comment says the mean over the whole 4 s. Which?
- Now: the mean over the time held; the button's number is that amount rounded up.
- From: architecture 25-24; chapter 7 A-29 (M2).

### B-98-8 M3: mono and the meters, the bar's scale

- Question 1: with 「モノで聞く」 on, every meter of the master (strip, top band, bars, loudness, M2's watch) hears the mono sound, so 「曲ぜんたい」 of a play with mono on part of the time is partly mono. Should the loudness hear the song as it is instead (one line)?
- Question 2: the bar of 「いまの大きさ」 is on the meters' scale (−14 LUFS at 37 %); the request's picture had another scale (−13.2 LUFS at 70 %) given nowhere. Which?
- Also: 「いまの大きさ」 and 「曲ぜんたい」 come from two meters (one could not keep the last play's number and still show what is heard now).
- From: architecture 25-29; chapter 7 A-29 (M3).

### B-98-9 E1: the width cap of lit notes

- Question: notes whose lit box would be wider than 15 % of the window's area over the note area's height (about 530 px at 1920 × 1080) are not lit, so long notes at a high zoom do not light. The request's own idea was "32 notes at most". Is the cap all right? It was chosen by reasoning and should be checked with `tools/measure/webkit-damage.sh`.
- From: architecture 25-19.

### B-98-10 E3: 「のび」 on sampled drum kits

- Question: on the studio and brush kits 「のび」 can only make hits shorter (below 50 %); at 50 % and above nothing changes, because a recording cannot ring longer than it was recorded. Is "only shorter" right?
- Listening files: `target/r12-e3-ear/` (26 files: grand piano, synth lead, studio kit, electro kit, each knob at its ends).
- From: architecture 25-27.

### B-98-11 R3: the two new click sounds

- Question: 「カチ」 and 「カウベル」 were designed by numbers and not heard. Are they right? Should choosing a sound play it at once (now it is heard on the next click)?
- From: architecture 25-8.

### B-98-12 A1: where ⟳ shows

- Question: the request says the mark shows "when the mouse is on it". It shows only while the pointer is on the handle (or the handle is held), not anywhere on the clip. Right?
- From: chapter 7 A-29 (A1).

### B-98-13 A2: 「この前に空きを入れる…」 and the empty section's tip

- Question 1: 「この前に空きを入れる…」 needs a usable section, like the other two rows, so it cannot be used before the last marker when nothing comes after it, though only the marker's own place matters for it. Should it need only the marker on a bar head?
- Question 2: the reason shown for that case, 「このマーカーより後ろにクリップがないので、区間がありません」, was made by the developers.
- From: chapter 7 A-29 (A2).

### B-98-14 A7: muting the whole source track for a clip bounce

- Question: 「音のクリップにする」 on some clips mutes the whole keys track (nothing smaller than a track can be muted), so clips outside the range go silent. Is that wanted, or should the track stay unmuted for 「音のクリップにする」?
- From: chapter 7 A-29 (A7).

### B-98-15 A8: recording into a comped clip; reverse, normalize and tune with a comp

- Question 1: a recording over a clip with a comp keeps the comp (the person still hears it) and adds the new takes to choose from. The other choice: the comp goes and the new take sounds whole. Which?
- Question 2: 「逆にする」, 「ノーマライズ」 and 「音程を寄せる」 are disabled on a clip with a comp (a tune set before stays and is heard). Turning 「音程を寄せる」 on for comps is one line now. Wanted?
- Also: the step name 「いいとこ取りを外す」 is provisional.
- From: architecture 25-33; chapter 7 A-29 (A8).

### B-98-16 M5: where the fade ends, and fades again

- Question 3: folded, the master's row stays as a low row 「▸ マスター 音量」; the master's automation mark sits at the right end of its value row (there was no disabled mark before). Right?
- From: architecture 25-32; chapter 7 A-29 (M5).

### B-98-17 A5: the names in 「最近ひらいた曲」

- Question: a row shows the song file's name without `.meloet`, not the name inside the file (that would mean reading every file). Normally they are the same. Right?
- From: chapter 7 A-29 (A5).

### B-98-18 D1: when 「新しい版」 shows

- Question: 「止まっているとき」 is read as "the song is not moving", so the notice waits while playing too, not only while counting in and recording. Right?
- From: chapter 7 A-29 (D1).

### B-98-19 M7: quality, and the separate stretch preset

- Question: tuned clips use a denser stretch preset than other made clips, so a stretched or pitched clip sounds slightly different once it is also tuned. Known limits: octave or fifth mistakes of the detector, whistling or very high voices, breathy voices left alone in patches, chords and backing tracks not detected (the tip is the only warning), a few dB of level difference at some note edges in the listening set. Is the result good enough by ear, and is the preset all right?
- Listening files: `target/r12-m7-ear/` (speech and singing; dry, 50 %, 100 %).
- From: architecture 25-34.

### B-98-20 P1: 「色」 on drum tracks and the menu bar

- Question: on drum rows 「色」 is 「使えない」 and borrows the scale lock's tip (「ドラムには音階がないので、スケール固定は使えません。」), whose second half names スケール固定. A tip of its own needs new wording. And the menu bar has no 「色をつける」 row (the request names only the toolbar and the right panel). Add either?
- From: chapter 7 A-29 (P1).

### B-98-21 P4: the patterns and plugin drum tracks

- Question 1: the twelve patterns (which drum where, velocities) were written by the developers and not heard. Are they right?
- Question 2: a plugin drum track (avldrums and so on) is not a drum track for patterns: 「ここに置く」 makes a studio kit track and ▶ plays the studio kit, because meloet does not know a plugin's key map. Right?
- From: architecture 25-20; chapter 7 A-29 (P4).

### B-98-22 P3: out-of-range values said nowhere

- Question: a number outside 10–300 in 「割合」 or 1–32 in 「ばらつき」 is put at the nearest end without a word, and an unreadable one only turns the box red. Should the status line say it, as for the other boxes? (The shapes `ValueField::Percent` and `Amount` exist since protocol 32; the screen does not use them yet.)
- From: architecture 25-10.

### B-98-23 M1: the factory presets and the mix of reverb and delay

- Question 1: the 18 factory values (and M6's six) were chosen by the developers and not heard, nor was switching presets while playing. Are they right?
- Question 2: a factory preset of the reverb or the delay leaves the mix as it is (100 % on a return, 30 % on a track or the master); a user preset sets the mix it kept. Should user presets also leave the mix alone?
- From: architecture 25-22.

### B-98-24 M6: the saturation's level, the de-esser's depth, the preset names

- Question: the saturation bends around −18 dBFS (louder material gets quieter as the drive goes up; one constant to change); at the de-esser's frequency itself the cut is half (the full cut about an octave and a half above); the six preset names were made by the developers. Right by ear? The de-esser was tried only on a made-up "voice", not a real one.
- Listening files: `target/r12-m6-ear/` (made by `MELOET_EAR_DIR=<folder> cargo test -p meloet-sound --test effects_ear -- --ignored --nocapture`).
- From: architecture 25-23.

### B-98-25 G1: 33 row names of the key list

- Question: 33 rows of the key list were named by the developers (chapter 7 A-29, G1). Are the names right?
- From: chapter 7 A-29 (G1).

### B-98-26 E7: analysis while the bars are hidden

- Question (for the lead): when the screen hides the bars for lack of room, the analysis still runs (core does not know the screen's width; telling it needs a new shape). The same holds for M3's loudness while the box is scrolled out of view. Worth a shape?
- From: chapter 7 A-29 (E7, M3).

### B-98-27 A3: 「トラックは 64 本まで…」 on the other ways of adding a track

- Question (for the lead): the notice is said by 「トラックを複製」, 「音のクリップにする」 and 「パターンを置く」, but the shelf's new track and 「＋ 歌のトラック」 still say nothing at 64 (the protocol comment says "and every other way").
- From: chapter 7 A-29 (A3).

### B-98-28 Wording made by the developers in round 12

- Question: every text with the source `builder:2026-11-23` or `builder:2026-11-24` in `ui/src/text/ja.ts` waits for the owner's approval. By feature (the texts are listed in chapter 7, A-29): G1 (33 key list rows), A3 (`msg.tooManyTracks`), R3 (`metronome.tapNone`), P3 (four status lines), A9 (three status lines), F1 (two check lines), E9 (`exported.lengthSeconds`), R2 (`msg.typedKeysIme`), P2 (`pr.ghostTracks`, `pr.ghostDrums`), P4 (`shelf.placeOnNewTrack`, `shelf.playPattern`), A1 (`arrange.repeatCount`), M1 (four texts), M6 (six preset names), F2 (`export.removePicture`, `export.noPicture`, two picture messages), E2 (`input.arpMenu`), E3 (`status.soundKnobSelected`, `history.soundKnob`), A2 (five texts), A7 (eight status lines), M5 (three texts), A8 (two disabled-row reasons), M7 (`msg.tuneDragging`).
- Now: shown as written.
- From: chapter 7 A-29.

### B-98-29 A9: no message when a clip's file cannot be read or written

- Question: a 「ノーマライズ」 of a file that cannot be read, or a 「逆にする」 whose new file cannot be written (disk full), does nothing and says nothing. The shape `clipWorkFailed {count}` exists since protocol 32 but has no wording. What should it say?
- From: architecture 25-12.

### B-98-30 D4: where the language choice lives

- Question: the spec says only "in the settings". It was put at 「表示 ▸ ことば / Language ▸」, next to the theme. Would the sound settings dialog be better?
- Now: in the 「表示」 menu.
- From: chapter 7 (D4).

### B-98-31 D4: in English, the spectrum and the 「キーで弾く」 switch give way more often

- Question: at 1280×800 in English the song name in the transport does not fold to 「▾」, so the spectrum (E7), which must not push other things out, is not shown in most scenes; and the input meter shows the modulation, so the "PC keys" switch (R2) has no room in five scenes (also while on). In Japanese both are shown. Keep the order of who gives way?
- Now: the rules of E7 and R2 as they are.
- From: the D4 helper's check of all scenes in both languages (2026-11-24).

### B-98-32 D4: the English wording was translated by the developers

- Question: 1535 texts, 99 step names and 107 terms are in English (`ui/src/text/en.ts`), following `glossary.md`. The owner has not read them. Words to look at first: いいとこ取り "Comping", 音程を寄せる "Pull to scale", さっきのを残す "Keep last", はじめての曲 "My first song", 「止」 "■", the short forms "Instr." and "Sel.". The texts shortened to fit are listed in chapter 7 (D4).
- Now: shown as written.
- From: chapter 7 (D4).

### B-98-33 E1: the redraw rule with 10 000 notes, and the measuring tool

- Question (for the lead and the owner): `tools/measure/webkit-damage.sh` stops the page at every changed rectangle, so the page runs at 15 to 25 paints a second and the arrange play line moves 2 px or more between paints; cut in three by the track meters it becomes 7 rectangles and the frame reaches 11. A feature that changes something every frame is measured worse than it is. Should the tool stop only at paints? And should the play line be kept from being cut by the track meter canvas (it can reach 11 in the real app too, when zoomed in or when frames drop)?
- Now: E1 stays on by default. On a quiet PC after the fix, with a tool that stops only at paints, on and off are the same (1 whole-window paint of about 410 / 460, the first after gdb attaches) and busyMs p95 differs by 0 to 1 ms; with the tool as it is, on gives 8 to 16 of about 110 and off 1 to 5 of about 124. The numbers are in `docs/measurements/2026-11-24-round12.md` ("E1 after the fix", "E1 on a quiet PC, after the fix").
- From: the E1 fix (2026-11-24).

### B-98-34 Measured at the edge in round 12: P2, M3, the export's speed

- Question: (1) P2 「ほかのトラックも見せる」: scrolling costs +2.3 ms at p95 (pass mark 2 ms; zoom +2.0). (2) M3: with the mixer open and every strip sounding, whole-window paints went from 1 of 90 to 8 of 91, 7 of them with the rectangle of M3's numbers. (3) The export takes 0.7 to 1.0 s longer (16 vocal tracks 2.97 → 3.7 s), read as F1's loudness check. Accept, or fix in a later round?
- Now: left as measured.
- From: `docs/measurements/2026-11-24-round12.md`.

### B-98-35 D4: seen in the real app

- Question 1: the file window's own GTK words (cancel, open, home …) follow the PC's language, not meloet's setting; with a Japanese PC and English chosen, the title is "Open" and the buttons are 「キャンセル」「開く」. Leave it?
- Question 2: at 1280×800 the mixer's instrument button reads "Instrument · …" in English and shows nothing of the sound's name (Japanese shows 「音源 ・ サイ…」). Shorten the English word?
- Now: as described. Pictures: `target/r12fin-d4/`.
- From: `docs/measurements/2026-11-24-round12.md` ("D4 in the real app").

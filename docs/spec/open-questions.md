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

### B-97-4 「最近ひらいた曲」 in the file menu

- Question: meloet now remembers the 20 songs opened or saved last (`recent-songs.json`; the AI reads it with `list_songs`). Should the file menu show them, and where and with what wording?
- Now: not shown on the screen.
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

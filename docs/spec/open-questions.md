# Open questions

This is the list of things for the owner to decide. Each item says what is asked, what the screen does now (the developers' provisional form), and where it comes from. When an item is decided, put the answer into the body of the spec (chapters 1–7) and remove the item from here.

The numbers are the B numbers of [`docs/history/design-notes.ja.md`](../history/design-notes.ja.md) (B-95 = round 8, B-94 = round 7). Code comments point to them, so keep them. Questions that already have an answer are not listed; their record is in that file (part C).

## B-96: round 10 (found when the readings moved into chapter 7)

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

## B-98: round 12 (the owner's request of 2026-11-23)

The readings behind these are in [chapter 7](7-readings.md), A-29; how each is built is architecture chapter 25. The owner answered every item on 2026-11-24 (the listening questions after hearing the files); what was decided is in the spec body. Below are only the parts not decided yet or not yet built.

### B-98-2 D2: the AppImage before any distribution

- Question: the AppImage holds about 170 Ubuntu libraries under the LGPL and libfuse in its runtime; publishing it needs a source offer for them from the same place. It also holds `libwebkit2gtk-4.1.so.0` with bytes changed by linuxdeploy-plugin-gtk (`/usr` → `././`), a change to an LGPL library that has to be stated. How should these be done, and should the AppImage be published at all?
- Now: built as a trial only (`target/appimage/`, `scripts/build-appimage.sh`); not published; the READMEs say nothing about it.
- From: architecture 23-5, 25-35; `docs/development/appimage.md`.

### B-98-6 E2: arpeggio — stop, small windows, the mixer strip

- Decided: question 1 (stop does not end a held chord; spec 3, E2; user:2026-11-24).
- Question 2: at 1280 × 800 at 150 % the button 「アルペジオ ▾」 does not fit in the input meter, and there is no menu row or key, so the arpeggio cannot be turned on there. Add a way?
- Question 3: on the mixer strip 「アルペジオ」 sits at the right end of the heading only while on (the track name is cut to about one character on a 108 px strip). The other way is an empty row on every strip, which moves everything below down by about 32 px. Which?
- From: architecture 25-26; chapter 7 A-29 (E2).

### B-98-22 P3: out-of-range values said nowhere

- Question: a number outside 10–300 in 「割合」 or 1–32 in 「ばらつき」 is put at the nearest end without a word, and an unreadable one only turns the box red. Should the status line say it, as for the other boxes? (The shapes `ValueField::Percent` and `Amount` exist since protocol 32; the screen does not use them yet.)
- From: architecture 25-10.

### B-98-27 A3: 「トラックは 64 本まで…」 on the other ways of adding a track

- Question (for the lead): the notice is said by 「トラックを複製」, 「音のクリップにする」 and 「パターンを置く」, but the shelf's new track and 「＋ 歌のトラック」 still say nothing at 64 (the protocol comment says "and every other way").
- From: chapter 7 A-29 (A3).

### B-98-29 A9: no message when a clip's file cannot be read or written

- Question: a 「ノーマライズ」 of a file that cannot be read, or a 「逆にする」 whose new file cannot be written (disk full), does nothing and says nothing. The shape `clipWorkFailed {count}` exists since protocol 32 but has no wording. What should it say?
- From: architecture 25-12.

### B-98-33 E1: the redraw rule with 10 000 notes, and the measuring tool

- Question (for the lead and the owner): `tools/measure/webkit-damage.sh` stops the page at every changed rectangle, so the page runs at 15 to 25 paints a second and the arrange play line moves 2 px or more between paints; cut in three by the track meters it becomes 7 rectangles and the frame reaches 11. A feature that changes something every frame is measured worse than it is. Should the tool stop only at paints? And should the play line be kept from being cut by the track meter canvas (it can reach 11 in the real app too, when zoomed in or when frames drop)?
- Now: E1 stays on by default. On a quiet PC after the fix, with a tool that stops only at paints, on and off are the same (1 whole-window paint of about 410 / 460, the first after gdb attaches) and busyMs p95 differs by 0 to 1 ms; with the tool as it is, on gives 8 to 16 of about 110 and off 1 to 5 of about 124. The numbers are in `docs/measurements/2026-11-24-round12.md` ("E1 after the fix", "E1 on a quiet PC, after the fix").
- From: the E1 fix (2026-11-24).

### B-98-35 D4: seen in the real app

- Decided: question 1 (the file window's own GTK words follow the PC's language; chapter 7 A-29, D4; user:2026-11-24).
- Question 2: at 1280×800 the mixer's instrument button reads "Instrument · …" in English and shows nothing of the sound's name (Japanese shows 「音源 ・ サイ…」). Shorten the English word?
- Now: as described. Pictures: `target/r12fin-d4/`.
- From: `docs/measurements/2026-11-24-round12.md` ("D4 in the real app").

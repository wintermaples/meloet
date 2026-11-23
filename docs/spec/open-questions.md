# Open questions

This is the list of things for the owner to decide. Each item says what is asked, what the screen does now (the developers' provisional form), and where it comes from. When an item is decided, put the answer into the body of the spec (chapters 1–7) and remove the item from here.

The numbers are the B numbers of [`docs/history/design-notes.ja.md`](../history/design-notes.ja.md) (B-95 = round 8, B-94 = round 7). Code comments point to them, so keep them. Questions that already have an answer are not listed; their record is in that file (part C).

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
- Question 2: at 1280 × 800 at 125 % (150 % is gone, user:2026-11-24; seen in the fake scenes arpOn, arpSettings and loop) the button 「アルペジオ ▾」 does not fit in the input meter, and there is no menu row or key, so the arpeggio cannot be turned on there. Add a way?
- Question 3: on the mixer strip 「アルペジオ」 sits at the right end of the heading only while on (the track name is cut to about one character on a 108 px strip). The other way is an empty row on every strip, which moves everything below down by about 32 px. Which?
- From: architecture 25-26; chapter 7 A-29 (E2).


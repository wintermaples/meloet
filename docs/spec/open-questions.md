# Open questions

This is the list of things for the owner to decide. Each item says what is asked, what the screen does now (the developers' provisional form), and where it comes from. When an item is decided, put the answer into the body of the spec (chapters 1–7) and remove the item from here.

The numbers are the B numbers of [`docs/history/design-notes.ja.md`](../history/design-notes.ja.md) (B-95 = round 8, B-94 = round 7). Code comments point to them, so keep them. Questions that already have an answer are not listed; their record is in that file (part C).

## B-97: round 11 (the AI link)

### B-97-3 Where on the screen a mute set is chosen

- Question: mute sets can only be used by the AI now. Should the screen have them, and where? Should saving, renaming and deleting be possible from the screen too?
- Answered (the owner, 2026-11-24): in the menu at the head of the mixer band, with saving, recalling, renaming and deleting. The shapes are in (protocol 34: `SongView::mute_sets`, `Command::SaveMuteSet` / `RecallMuteSet` / `DeleteMuteSet` / `RenameMuteSet`, architecture 25-37); the screen part is being built. Remove this item when it is in.
- From: architecture 24-16 F.

## B-98: round 12 (the owner's request of 2026-11-23)

The readings behind these are in [chapter 7](7-readings.md), A-29; how each is built is architecture chapter 25. The owner answered every item on 2026-11-24 (the listening questions after hearing the files); what was decided is in the spec body. Below are only the parts not decided yet or not yet built.

### B-98-6 E2: arpeggio — stop, small windows, the mixer strip

- Decided: question 1 (stop does not end a held chord; spec 3, E2; user:2026-11-24).
- Question 2: at 1280 × 800 at 125 % (150 % is gone, user:2026-11-24; seen in the fake scenes arpOn, arpSettings and loop) the button 「アルペジオ ▾」 does not fit in the input meter, and there is no menu row or key, so the arpeggio cannot be turned on there. Add a way?
- Question 3: on the mixer strip 「アルペジオ」 sits at the right end of the heading only while on (the track name is cut to about one character on a 108 px strip). The other way is an empty row on every strip, which moves everything below down by about 32 px. Which?
- From: architecture 25-26; chapter 7 A-29 (E2).

### B-98-36 M5: a fade-out added again after the song grew

- Question: when clips are added after an old fade-out and the fade-out is added again, the old fade (a line going down to −∞ in the middle of the song) lies before the new start, so it stays, and the new fade goes from −∞ to −∞: silence. Should "the lane's last line going down to −∞" be removed wherever it lies, so the new fade replaces the old one?
- Now: points before the new start are never touched (chapter 7 A-29, M5).
- From: the helper "fade" (2026-11-24).

### B-98-38 Rules for pages that can no longer be reached

- Question (for the lead): with the smallest window at 1024×640 times the scale, the rules, wording and scenes for pages narrower than 1024 or lower than 640 cannot be reached: the input meter's split form at 853 (spec 3; `InputMeter.svelte`, `meterFit.ts`), the piano roll's 448 row of the height table, `pack.askDetailShort` (mock R12-Fit), the scenes `vocalFit` and `exportFit` and the eight 853×448 pictures of `shoot_ui.py`, the narrow transport forms of mock F-PR150. Some may still be used at 1024 (not measured). Remove them in a later job, after checking each at 1024×640.
- Now: left in; spec 1 and 3 say "cannot be reached now".
- From: the helper "scale" (2026-11-24).

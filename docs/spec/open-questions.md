# Open questions

This is the list of things for the owner to decide. Each item says what is asked, what the screen does now (the developers' provisional form), and where it comes from. When an item is decided, put the answer into the body of the spec (chapters 1–7) and remove the item from here.

The numbers are the B numbers of [`docs/history/design-notes.ja.md`](../history/design-notes.ja.md) (B-95 = round 8, B-94 = round 7). Code comments point to them, so keep them. Questions that already have an answer are not listed; their record is in that file (part C).

## B-98: round 12 (the owner's request of 2026-11-23)

The readings behind these are in [chapter 7](7-readings.md), A-29; how each is built is architecture chapter 25. The owner answered every item on 2026-11-24 (the listening questions after hearing the files); what was decided is in the spec body. Below are only the parts not decided yet or not yet built.

### B-98-38 Rules for pages that can no longer be reached

- Question (for the lead): with the smallest window at 1024×640 times the scale, the rules, wording and scenes for pages narrower than 1024 or lower than 640 cannot be reached: the input meter's split form at 853 (spec 3; `InputMeter.svelte`, `meterFit.ts`), the piano roll's 448 row of the height table, `pack.askDetailShort` (mock R12-Fit), the scenes `vocalFit` and `exportFit` and the eight 853×448 pictures of `shoot_ui.py`, the narrow transport forms of mock F-PR150. Some may still be used at 1024 (not measured). Remove them in a later job, after checking each at 1024×640.
- Now: left in; spec 1 and 3 say "cannot be reached now".
- From: the helper "scale" (2026-11-24).

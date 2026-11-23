# Open questions

This is the list of things for the owner to decide. Each item says what is asked, what the screen does now (the developers' provisional form), and where it comes from. When an item is decided, put the answer into the body of the spec (chapters 1–7) and remove the item from here.

The numbers are the B numbers of [`docs/history/design-notes.ja.md`](../history/design-notes.ja.md) (B-95 = round 8, B-94 = round 7). Code comments point to them, so keep them. Questions that already have an answer are not listed; their record is in that file (part C).

## B-98: round 12 (the owner's request of 2026-11-23)

The readings behind these are in [chapter 7](7-readings.md), A-29; how each is built is architecture chapter 25. The owner answered every item on 2026-11-24 (the listening questions after hearing the files); what was decided is in the spec body. Below are only the parts not decided yet or not yet built.

### B-98-2 D2: the AppImage before any distribution

- Question: the AppImage holds about 170 Ubuntu libraries under the LGPL and libfuse in its runtime; publishing it needs a source offer for them from the same place. It also holds `libwebkit2gtk-4.1.so.0` with bytes changed by linuxdeploy-plugin-gtk (`/usr` → `././`), a change to an LGPL library that has to be stated. How should these be done, and should the AppImage be published at all?
- Now: built as a trial only (`target/appimage/`, `scripts/build-appimage.sh`); not published; the READMEs say nothing about it.
- From: architecture 23-5, 25-35; `docs/development/appimage.md`.

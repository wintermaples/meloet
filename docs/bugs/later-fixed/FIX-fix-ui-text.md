# FIX-fix-ui-text — fixed in the second pass (2026-11-25)

- ui/src/bridge/fakeParams.ts:192 — the fake's plugin `range` for a clamped typed value is joined with 「〜」 in both languages; the core now picks 「〜」 / "–" by its language (UI-TEXT-004), so the fake's English scenes still show 「〜」.
- crates/meloet-core/src/play/bounce.rs:71 — `bounce_file_name` keeps 「（音）」 in English, so the work file of "Piano (audio)" is named after 「Piano（音）」; architecture 25-36 says file names of recordings follow the track name.
- crates/meloet-core/src/play/bounce.rs:93 — no core test watches that `bounce_made` passes `Play::names` (the model test checks the name in both languages).

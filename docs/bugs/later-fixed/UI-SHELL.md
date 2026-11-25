# UI-SHELL — fixed in the second pass (2026-11-25)

- ui/src/components/SettingsDialog.svelte:171-177 — On 「くわしい設定」 (a second page with 「◂」), Esc closes the whole dialog (core `Back` takes `self.dialog`, crates/meloet-core/src/state.rs:1702-1710); spec 4's Esc row says "on the second page, to the first page" (written for About; unclear whether it covers the settings).
- ui/src/keys/attach.ts:538-540 — On the advanced settings page the bright button is 「閉じる」 (`settingsCancel`, SettingsDialog.svelte:385-386) but Enter sends `settingsApply`, so a draft output picked on page 1 is applied by Enter and dropped by the button.
- ui/src/components/history/historyUi.svelte.ts:29 — ↓ (older) with nothing chosen and every step undone (current 0) chooses row 1, which is a newer row than "all undone".
- ui/src/components/StatusLine.svelte:420-421 — `lengthMatched` writes the length with the song-start time signature (`app.timeline.meterNum/Den`), not the one at the notes, in songs with time signature changes.
- ui/src/components/StatusLine.svelte:892-902 — While stopped, the held-keys effect re-runs and scans 128 keys on every telemetry message, also when nothing is held.

## not true when looked at again (2026-11-25)

- ui/src/components/SoundHint.svelte:25-28 — Sentences are joined with a fixed `" "`, not `SENTENCE_JOIN` (empty in Japanese); check against mock F-Connect3.

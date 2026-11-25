# PROTO — fixed in the second pass (2026-11-25)

- crates/meloet-protocol/src/timeline.rs:3 — the module comment says a tempo is beats of the meter's lower number a minute; CLAUDE.md and the rest of the code say quarter notes a minute whatever the meter. Misleading comment in the contract.
- crates/meloet-protocol/src/lib.rs:754 — `Command::Audition` says the screen sends `AuditionOff`; the screen sends `audition` with empty `keys` instead (`ui/src/components/pianoroll/audition.ts:78`), so `Command::AuditionOff` has no sender.
- crates/meloet-protocol/src/lib.rs:1527 — `Command::TypedKey` says the screen turns the key time into Rust's clock with `HelloReply::mono_us`; it uses `clockOffsetUs` (`ui/src/keys/typed.svelte.ts:69`), and `HelloReply.monoUs` is read by nothing.
- crates/meloet-protocol/src/lib.rs:3824 — `StatusMessage::AudioImported.skipped` (and `count`) are not shown: when some chosen files do not read, the status line names only the first file brought in, and the person is not told files were left out. Spec 6 has no sentence for it (needs wording from the owner).

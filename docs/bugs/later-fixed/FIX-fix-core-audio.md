# FIX-fix-core-audio — fixed in the second pass (2026-11-25)

- crates/meloet-core/src/state.rs `status` (with play.rs `record`) — `StatusMessage::NoInput`, set when R is pressed with an armed vocal track that has no input, is a plain notice: `moving_status` (counting in, recording) comes before it, so it is never seen while the keys are recorded, and the next command (Space) puts it away. `VocalsNotRecorded` now has its own place before `moving_status`.
- crates/meloet-core/src/made.rs:201 — the key of audio made for a clip (stretched, at another pitch: `stretch/<key>.wav`) holds the source's uid, not its contents; after 「さがす…」 takes a file with other contents, a stretched clip can still play audio made from the old file (CORE-AUDIO-007 lets go only of the decoded copies and the peaks).

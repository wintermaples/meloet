# CORE-PLAY-A — fixed in the second pass (2026-11-25)

- crates/meloet-core/src/play.rs:1414-1417 — when a step (an undo of 「トラックを足す」 or 「トラックを複製」, say) takes the selected track away, the selection goes to the first track, while `delete_track` (play/tracks.rs:24-27) goes to the track below; the two rules differ.
- crates/meloet-core/src/play.rs:2183-2186 — `can_record` stays true while a vocal recording waits for the disk writer (`state` is `Stopped`, `rec` is still `Some`); for that short time R and Space do nothing although the buttons look usable.
- crates/meloet-core/src/play/recent.rs:400-413 — keys played while the shelf auditions a sound (「ためし弾き」) still go into the phrase of the keys target track, so 「さっきのを残す」 can keep a phrase that was heard with another instrument.
- crates/meloet-core/src/play/timeline.rs:437-443 — `go_to_marker` selects the marker also while counting in or recording, where `locate` refuses to move; the marker is chosen but the line stays.

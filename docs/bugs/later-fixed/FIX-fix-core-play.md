# FIX-fix-core-play — fixed in the second pass (2026-11-25)

- crates/meloet-core/src/play/clips.rs:56 (`create_clip`) and play/shelf.rs:299 (a new track from the shelf) set the selected track without `selected_track_changed`: only keys tracks, so no ● is missed, but `last_keys` stays and `create_clip` does not call `recent_follow_target`. No visible fault found.

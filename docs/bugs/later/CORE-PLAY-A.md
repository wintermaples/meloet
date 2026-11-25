# CORE-PLAY-A — worth a look later (not bugs)

- crates/meloet-core/src/play.rs:986-1000 — L with no range is "a range step + on"; undo takes the range away and turns the loop off, but redo brings the range back with the loop still off. Maybe meant (on/off is the session's), worth a decision.
- crates/meloet-core/src/play/progression.rs:230-238 — `add_notes_at` runs `notes::write_notes` once per distinct start until it fits; fine for the six progressions, quadratic for a long list from the AI link.

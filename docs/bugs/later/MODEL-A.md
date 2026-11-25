# MODEL-A — worth a look later (not bugs)

- crates/meloet-model/src/record.rs:136-140 — a key of an ended pass that arrives late keeps its tick and so lies at the end of the pass going on, one loop length after it was played (tested as meant in `closed_passes_are_given_once_and_late_keys_go_on`; musically doubtful).
- crates/meloet-model/src/quantize.rs:146-174 — 「もとの位置へ戻す」 can make a step with `count: 0` that only drops the origin (all notes already back after hand moves, or every note with an origin index deleted).
- crates/meloet-model/src/arrange.rs:565-591 — joining keeps only the control points inside each clip, so a pedal or bend carried to a clip's start (or reset at its end) by `track_events` sounds different after the join (as written in architecture 24-6).

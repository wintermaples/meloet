# MODEL-A — worth a look later (not bugs)

- crates/meloet-model/src/quantize.rs:81-88 — with 「始まりだけ」 a note that has an origin index takes the origin note's length, so a length changed by hand after the first quantize goes back to the recorded length on 「かけなおす」; architecture 16 says "keeping its length" and "starts from the recorded position" without saying which length is meant.
- crates/meloet-model/src/timeline.rs:85-90 — a tempo point keeps `ramp: true` after the point after it is deleted (harmless while it is last); adding a point after it later makes it ramp into the new point without the person asking.
- crates/meloet-model/src/record.rs:136-140 — a key of an ended pass that arrives late keeps its tick and so lies at the end of the pass going on, one loop length after it was played (tested as meant in `closed_passes_are_given_once_and_late_keys_go_on`; musically doubtful).
- crates/meloet-model/src/quantize.rs:146-174 — 「もとの位置へ戻す」 can make a step with `count: 0` that only drops the origin (all notes already back after hand moves, or every note with an origin index deleted).
- crates/meloet-model/src/arrange.rs:565-591 — joining keeps only the control points inside each clip, so a pedal or bend carried to a clip's start (or reset at its end) by `track_events` sounds different after the join (as written in architecture 24-6).

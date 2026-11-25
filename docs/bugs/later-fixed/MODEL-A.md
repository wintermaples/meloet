# MODEL-A — fixed in the second pass (2026-11-25)

- crates/meloet-model/src/quantize.rs:81-88 — with 「始まりだけ」 a note that has an origin index takes the origin note's length, so a length changed by hand after the first quantize goes back to the recorded length on 「かけなおす」; architecture 16 says "keeping its length" and "starts from the recorded position" without saying which length is meant.
- crates/meloet-model/src/timeline.rs:85-90 — a tempo point keeps `ramp: true` after the point after it is deleted (harmless while it is last); adding a point after it later makes it ramp into the new point without the person asking.

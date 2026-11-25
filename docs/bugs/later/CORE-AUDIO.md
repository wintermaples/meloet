# Later: CORE-AUDIO

- crates/meloet-core/src/audiofiles.rs:429-433 — two table entries with one uid (the same bytes brought in under two names) open only the first name; when that file is missing the other is not tried, and `watch` reports the uid by the first name only.
- crates/meloet-core/src/readahead.rs:325-340 — a second splice that comes inside the 10 ms of the first starts from the first's new audio, so the frame before it (still part old) steps to the new audio without the overlap.
- crates/meloet-core/src/vocalrec.rs:1000-1013 (service/input.rs:413) — `DiskWriter::mark` makes the core thread wait up to 300 ms (`MARK_WAIT`) when no block after `from` has come yet (R while playing with a large output delay and a fast tempo).

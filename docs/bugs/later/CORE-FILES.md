# Later: CORE-FILES

- crates/meloet-core/src/recent.rs:30 — a song on a disk that is not mounted at start leaves the list for good at the next write; architecture 25-7 speaks only of a failed open.
- crates/meloet-core/src/sounds/fetch.rs:94 — `version_dir` with no preferred version takes the largest folder name; versions are content hashes, so with two old versions kept it is not the newest one.
- crates/meloet-core/src/maker.rs:528 — `make_now` writes `<key>.export-<pid>.wav`, which is not a `.part`: an export killed in the middle leaves it until `trim_cache` takes it.

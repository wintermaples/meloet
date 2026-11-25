# Later: CORE-FILES

- crates/meloet-core/src/songfile.rs:1981 — the temporary names `.{name}.new` and `.{name}.prev.new` add up to 10 bytes to the song file's name, and `folder_name` has no length cap: a song name of about 245 bytes (about 80 Japanese characters) saves once, then every later save fails with ENAMETOOLONG.
- crates/meloet-core/src/songfile.rs:1985 — a save with no change since the last one copies the current file over `.prev`, so the real previous version is lost (a design question: should `.prev` change when the bytes are the same?).
- crates/meloet-core/src/songfile.rs:107 — a field of a return or the master of another shape (e.g. a return's `pan` of 12.5) drops the whole mixer, master plugin inserts and their state with it, without a word; meant by the comment, but the master's `pan` has a lenient reader (`pan_or_middle`) and a return's does not.
- crates/meloet-core/src/settings.rs:403 — one field of the wrong type (e.g. `"scale": "x"`) resets every setting (output, favorites, plugin places), and the next save writes the defaults over the file; meant by the tests, worth a look.
- crates/meloet-core/src/settings.rs:549, recent.rs:66, presets.rs:216, updates.rs:235, sounds/fetch.rs:869 — rename without a folder sync after it (the song file and the recovery file sync the folder).
- crates/meloet-core/src/recent.rs:30 — a song on a disk that is not mounted at start leaves the list for good at the next write; architecture 25-7 speaks only of a failed open.
- crates/meloet-core/src/sounds/fetch.rs:664-679 — a server that answers 416, or a 206 without `Content-Range`, to a request with no `Range` makes `part` loop with no pause until stopped (`Try::Again` has no count); GitHub does not do this.
- crates/meloet-core/src/sounds/fetch.rs:94 — `version_dir` with no preferred version takes the largest folder name; versions are content hashes, so with two old versions kept it is not the newest one.
- crates/meloet-core/src/maker.rs:528 — `make_now` writes `<key>.export-<pid>.wav`, which is not a `.part`: an export killed in the middle leaves it until `trim_cache` takes it.

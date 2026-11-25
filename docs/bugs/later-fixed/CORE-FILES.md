# CORE-FILES — fixed in the second pass (2026-11-25)

- crates/meloet-core/src/songfile.rs:1981 — the temporary names `.{name}.new` and `.{name}.prev.new` add up to 10 bytes to the song file's name, and `folder_name` has no length cap: a song name of about 245 bytes (about 80 Japanese characters) saves once, then every later save fails with ENAMETOOLONG.
- crates/meloet-core/src/songfile.rs:1985 — a save with no change since the last one copies the current file over `.prev`, so the real previous version is lost (a design question: should `.prev` change when the bytes are the same?).
- crates/meloet-core/src/songfile.rs:107 — a field of a return or the master of another shape (e.g. a return's `pan` of 12.5) drops the whole mixer, master plugin inserts and their state with it, without a word; meant by the comment, but the master's `pan` has a lenient reader (`pan_or_middle`) and a return's does not.
- crates/meloet-core/src/settings.rs:403 — one field of the wrong type (e.g. `"scale": "x"`) resets every setting (output, favorites, plugin places), and the next save writes the defaults over the file; meant by the tests, worth a look.
- crates/meloet-core/src/settings.rs:549, recent.rs:66, presets.rs:216, updates.rs:235, sounds/fetch.rs:869 — rename without a folder sync after it (the song file and the recovery file sync the folder).
- crates/meloet-core/src/sounds/fetch.rs:664-679 — a server that answers 416, or a 206 without `Content-Range`, to a request with no `Range` makes `part` loop with no pause until stopped (`Try::Again` has no count); GitHub does not do this.

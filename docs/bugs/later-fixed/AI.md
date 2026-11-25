# AI — fixed in the second pass (2026-11-25)

- crates/meloet-ai/src/lib.rs:286 — an `accept` error is `continue`d at once; an error that repeats (EMFILE when the process is out of file handles) turns the link's thread into a busy loop. A short sleep after an error would do.
- crates/meloet-ai/src/tools/render.rs:133-149, tools/plugstate.rs:311 — `scratch.clean` (which may `remove_dir_all` folders of `render_audio`) and `scratch.dir` run on the link's one runtime thread, not on the blocking pool; a big folder stalls every connection for that time.
- crates/meloet-ai/src/surge/sources.rs:1 — the module comment says Surge XT 1.3.x has 41 sources; `SOURCES` holds 40 (ids 1–40).
- crates/meloet-ai/src/tools/midi.rs:458 — the error says the name must end in `.mid`, but `.midi` is taken too (`is_midi_name`).

## not true when looked at again (2026-11-25)

- crates/meloet-core/src/play/timeline.rs:405-420 — an AI `rename_marker` of the marker the person has just made with K (still "fresh") amends that step, the undo count does not change, and the core answers `Unchanged`, so the tool says "No change" though the name changed.

# EDGE — fixed in the second pass (2026-11-25)

- crates/meloet-app/src/single.rs:30-32,222-242 — a later meloet waits only `TALK` (2 s) for the answer, but the first one starts its listener thread only after `Core::start` (main.rs:518-533); a second start during the first's own start may get `NoAnswer` and exit(1), and the first then drops the handed song because writing `ok` fails (single.rs:286).
- crates/meloet-app/src/pcwindow.rs:136-142 — the song file filter takes only `*.meloet` in small letters, while the SF2, audio and picture filters take both cases (`patterns`).

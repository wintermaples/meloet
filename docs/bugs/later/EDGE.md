# EDGE: worth a look later (not reports)

- crates/meloet-devices/src/facts.rs:503-513 — a Route whose `devices` list matches two nodes gives two endpoints with the same id `<device.name>:<route name>`; not seen on real cards, but nothing keeps ids unique.
- crates/meloet-midi/src/input.rs:298-300 — after ENOSPC (events dropped) only the plug state is counted again; a dropped note-off leaves a held key until all-notes-off, and a keyboard unplugged and plugged again with the same client number inside the lost events stays "active" without a subscription.
- crates/meloet-app/src/pcwindow.rs:98-101,182,187 — `SHOWN` keeps one native dialog; a second pick while one is shown would drop the first and its answer would never come. Not reachable now (the window is modal and the core guards each pick), but nothing in the shell guards it.
- crates/meloet-mcp/src/main.rs:87-89 — a line from meloet longer than `MAX_LINE` (16 MiB) is skipped silently; the request it answered stays pending until meloet hangs up, so the AI app waits.

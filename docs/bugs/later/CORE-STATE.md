# CORE-STATE: worth a look later (not reports)

- crates/meloet-core/src/hub.rs:105-109 — a page that loads again (「ことば」, a web process that came back) subscribes new sinks; the old ones stay in the hub until a send to them fails, which depends on what Tauri's Channel does for a page that is gone (meloet-app).
- crates/meloet-core/src/state/updates.rs:66-76 — `settle_updates` reads the status line before the rest of `handle` sets its notices, so `said` can be true for a line that was not shown; the song moving then puts it away for the run (rare).
- crates/meloet-core/src/state.rs:1297-1302 — files let go of over the window while recording or counting in are passed over without a word on the status line (the status line showed the drag line just before).
- crates/meloet-core/src/state/songs.rs:377-383 — a name in the save window that is empty after taking off `.meloet` and spaces does nothing and says nothing.

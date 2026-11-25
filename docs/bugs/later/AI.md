# AI — worth a look later (not bugs)

- crates/meloet-ai/src/tools.rs:750, tools/notes.rs:514, tools/midi.rs:392, tools/render.rs:269 — a track given as `"id:7"` is turned into its number on the AI thread and the ask carries the number; if the person adds, moves or deletes a track before the core runs the ask (outside a batch), the number names another track. Asks that take track ids (`SetInstrument`, `SetPluginOutput` …) do not have this window.
- crates/meloet-ai/src/tools/automation.rs:324-373 — `move_automation` / `delete_automation` name points by index with no guard like the take id of notes; after the lane changed (the person, or another call), an old index moves or deletes another point.

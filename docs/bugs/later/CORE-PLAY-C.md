# CORE-PLAY-C: worth a look later (not reports)

- crates/meloet-core/src/play/plugs.rs:657-690 — a sample rate change during a visit to a plugin's window gives the plugin a new ticket, so `sync_windows` drops the visit, while the new instance is loaded with the old one's live state: what was changed in the window before the reload is in the plugin but never becomes a step (song and plugin differ until a save asks the plugin; the recovery file and undo do not have it).

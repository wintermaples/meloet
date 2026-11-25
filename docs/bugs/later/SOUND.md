# SOUND — worth a look later (not bugs)

- crates/meloet-sound/src/phrases.rs:229-251 — `PhrasePlayer` does not pass `ask` or `latency` on and cuts each block at note frames; on the sample seat with a plugin (`service.rs:3691`) every piece is its own synchronous round trip to the plugin host, never asked ahead.

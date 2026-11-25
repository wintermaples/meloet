# fix-core-audio — seen while fixing, not fixed

- crates/meloet-core/src/vocalrec.rs `Side::start` — when no input is open as the recording starts (a race with the input going away), the error is the same `VocalsNotStarted`, so the status line says the work place cannot be written; the input's own notice (`InputLost`) comes with it.

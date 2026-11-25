# FIX-fix-core-play — worth a look later (found while fixing, not fixed)

- crates/meloet-core/src/play/autosync.rs:187-208 — with `all` (another song) `before` is always empty, so nothing gives back a parameter that an old song's lane moved: a plugin instrument that stays on its seat across another song (`Want::Kept` in `play/plugs.rs` `seat_instrument`, the same plugin in the same saved state) may keep the value where the old lane left it. Read from the code, not run.

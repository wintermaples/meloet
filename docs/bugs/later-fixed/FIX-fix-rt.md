# FIX-fix-rt — fixed in the second pass (2026-11-25)

- crates/meloet-plughost/src/formats/vst3.rs:1085-1095 — `Vst3Processor::reset` does nothing when the gate is taken (buses being switched) and nothing does it later, so a reset asked in that moment is lost; LV2 keeps its reset in `quiet` until the next block that runs.

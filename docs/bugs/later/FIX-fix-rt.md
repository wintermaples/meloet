# FIX-fix-rt — worth a look later (seen while fixing, not fixed)

- crates/meloet-plughost/src/formats/vst3.rs:1085-1095 — `Vst3Processor::reset` does nothing when the gate is taken (buses being switched) and nothing does it later, so a reset asked in that moment is lost; LV2 keeps its reset in `quiet` until the next block that runs.
- crates/meloet-plughost/src/formats/lv2.rs:1523-1528 — `midi` drops an event when the atom buffer is full (the result of `push_midi_event` is not looked at); with many `patch:Set` and window messages on the same port, a note-off late in the block could be lost in theory (512 events of MIDI alone fit the 16 KiB buffer).

# PLUG — worth a look later (not bugs)

- crates/meloet-plughost/src/formats/vst3.rs:987-997, 1167-1176 — values the person moves in a VST3 window (`performEdit`) come back to the controller with `setParamNormalized` after their block; during a drag in the plugin's own window an echo of an older value may move its knob back for a moment, depending on the plugin. Not seen.
- crates/meloet-plughost/src/rt.rs:135-143, 156-164 — one block takes at most 512 changes from the main thread (`to_plugin_events` + `COMMANDS`); an LV2 window that writes more ports at once (a preset loaded by the window itself) loses the rest: the DSP never gets them while `Shared::values` shows them.
- crates/meloet-plughost/src/control.rs:171 — `Load` refuses a `max_frames` larger than the mapping's but takes a smaller one, while the engine's blocks go up to the mapping's size; VST3 buffers sized by the activation would then overflow. Core always sends the mapping's size today.

# UI-MIX — worth a look later (not bugs)

- ui/src/components/mixer/control.svelte.ts:147 — mixer and params drags handle `pointercancel` but not `lostpointercapture` or a window blur; if the `pointerup` is ever lost (focus taken during a drag), `press` stays set, hovering keeps moving the control and the next press is ignored. Not reached in a test; the other bands do the same.
- ui/src/components/params/ParamRow.svelte:111 — a press on a built-in parameter bar jumps the value to the pressed point, while mixer drags move only by the distance from the press; worth checking against spec 2 "Parameter row".

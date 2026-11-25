# UI-ARRANGE — worth a look later (not bugs)

- ui/src/components/automation/AutoLane.svelte:216-225 — the rec border grows by writing `style.width` every frame while writing (layout per frame; architecture 7 rule (e) moves lengths with `transform`); not measured with `tools/measure/webkit-damage.sh`.
- ui/src/components/Arrange.svelte:520-523 — `lanePicture` depends on `liveVersion`, so every key that arrives while recording rebuilds the whole big face (all clips), not only the committed part.

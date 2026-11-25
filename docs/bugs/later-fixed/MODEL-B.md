# MODEL-B — fixed in the second pass (2026-11-25)

- crates/meloet-core/src/play/bounce.rs:93 — turning a soloed keys track into audio: the keys track is muted and stays soloed, the new vocal track is not soloed, so nothing is heard until solo is cleared (mute wins over solo). The spec says nothing about solo here.
- crates/meloet-model/src/automation.rs:380, 568 — the `flat` value (the hand's place) goes into points with `clamp`, which keeps NaN; today the plugin values it comes from seem to be filtered for non-finite values upstream, but the model's "values are never NaN" rests on that.
- crates/meloet-model/src/audio/comp.rs:148 — while a clip has a comp, `set_comp` accepts a one-piece comp naming the shown take: it sounds as no comp, but `played_frames` stays `None`, so 「ノーマライズ」 and 「逆にする」 stay unavailable for that clip.

<script lang="ts">
  // Starts the WebGL engine and feeds it; shows nothing itself. The engine puts a canvas into
  // every box marked with glSlot (see slot.ts), wherever in the page that box is.
  import { onMount } from "svelte";
  import { clockOffsetUs, measureEnabled, measureReport } from "../bridge";
  import { current, onEvent, onTelemetry } from "../state/app.svelte";
  import { Engine } from "./engine";
  import { KeyToScreen } from "./measure";
  import { FrameProfile } from "./profile";

  let engine = $state.raw<Engine | null>(null);

  onMount(() => {
    const made = new Engine();
    engine = made;
    if (current.telemetry) made.onTelemetry(current.telemetry);
    let measure: KeyToScreen | null = null;
    let profile: FrameProfile | null = null;
    let gone = false;

    const stopTelemetry = onTelemetry((t) => made.onTelemetry(t));
    const stopEvents = onEvent((e) => {
      if (measure && e.type === "note" && e.on) measure.noteOn(e.note, e.tUs);
      made.onEvent(e);
    });
    measureEnabled()
      .then((on) => {
        if (!on || gone) return;
        measure = new KeyToScreen({ clockOffsetUs, report: measureReport, now: () => performance.now() });
        made.onFrame = (ts) => measure?.drawn(ts);
        measure.start();
        profile = new FrameProfile(measureReport);
        made.profile = profile;
        profile.start();
      })
      .catch(() => {});

    return () => {
      gone = true;
      stopTelemetry();
      stopEvents();
      measure?.stop();
      profile?.stop();
      made.destroy();
      engine = null;
    };
  });

  $effect(() => {
    const state = current.state;
    if (!engine || !state) return;
    engine.setScale(state.workspace.scale);
    engine.setTracks(state.song.tracks.map((t) => t.number));
  });
</script>

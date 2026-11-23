<script lang="ts">
  // Starts the WebGL engine and feeds it; shows nothing itself. The engine puts a canvas into
  // every box marked with glSlot (see slot.ts), wherever in the page that box is.
  import { onMount } from "svelte";
  import { clockOffsetUs, measureEnabled, measureReport, uiScript } from "../bridge";
  import { metrics } from "../components/metrics.svelte";
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
    let stopScript: (() => void) | null = null;
    let gone = false;

    const stopTelemetry = onTelemetry((t) => made.onTelemetry(t));
    const stopEvents = onEvent((e) => {
      if (measure && e.type === "note" && e.on) measure.noteOn(e.note, e.tUs);
      made.onEvent(e);
    });
    // Only a build with the development hooks measures (MELOET_DEV_HOOKS=1, vite.config.ts).
    if (__DEV_HOOKS__) {
      measureEnabled()
        .then((on) => {
          if (!on || gone) return;
          measure = new KeyToScreen({ clockOffsetUs, report: measureReport, now: () => performance.now() });
          made.onFrame = (ts) => measure?.drawn(ts);
          measure.start();
          profile = new FrameProfile(measureReport);
          made.profile = profile;
          profile.start();
          return uiScript();
        })
        .then(async (script) => {
          if (!script || gone) return;
          const dev = await import("../dev/start");
          if (!gone) stopScript = dev.startUiScript(script, measureReport);
        })
        .catch(() => {});
    }

    return () => {
      gone = true;
      stopTelemetry();
      stopEvents();
      measure?.stop();
      profile?.stop();
      stopScript?.();
      made.destroy();
      engine = null;
    };
  });

  $effect(() => {
    const state = current.state;
    if (!engine || !state) return;
    engine.setScale(state.workspace.scale);
    engine.setTheme(metrics.theme);
    engine.setTracks(state.song.tracks.map((t) => t.number));
  });
</script>

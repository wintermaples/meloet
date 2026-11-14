<script lang="ts">
  // The one WebGL canvas over the whole window. It draws into the boxes marked with glSlot.
  import { onMount } from "svelte";
  import { clockOffsetUs, measureEnabled, measureReport } from "../bridge";
  import { current, onEvent, onTelemetry } from "../state/app.svelte";
  import { Engine } from "./engine";
  import { KeyToScreen } from "./measure";

  let canvas: HTMLCanvasElement;
  let engine = $state.raw<Engine | null>(null);

  onMount(() => {
    let made: Engine;
    try {
      made = new Engine(canvas);
    } catch (e) {
      // No WebGL2: the rest of the screen still works; the slots stay empty.
      console.error(e);
      return;
    }
    engine = made;
    if (current.telemetry) made.onTelemetry(current.telemetry);
    let measure: KeyToScreen | null = null;
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
      })
      .catch(() => {});

    return () => {
      gone = true;
      stopTelemetry();
      stopEvents();
      measure?.stop();
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

<canvas bind:this={canvas} aria-hidden="true"></canvas>

<style>
  canvas {
    position: fixed;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }
</style>

<!-- Writes the current position of what lanes move (autoState.svelte.ts). Draws nothing.
     When stopped, once at the play line when the state or the points change. When moving, every frame of the WebGL manager while a lane exists. -->
<script lang="ts">
  import { untrack } from "svelte";
  import { everyGlFrame } from "../../gl/slot";
  import { current } from "../../state/app.svelte";
  import { anyLane, playTick, pointsVersion, refreshPositions, wroteVersion } from "./autoState.svelte";

  const state = $derived(current.state);
  const moving = $derived(state !== null && state.transport.state !== "stopped" && anyLane(state));

  $effect(() => {
    const s = state;
    void pointsVersion();
    if (!s) return;
    untrack(() => refreshPositions(s, playTick()));
  });

  $effect(() => {
    if (!moving) return;
    return everyGlFrame(() => {
      const s = current.state;
      if (!s) return;
      layoutNow();
      refreshPositions(s, playTick(), true);
    });
  });

  // WebKit collects the repaint rectangles asked for during one layout, and when they get complex, makes them "one rectangle around all"
  // (docs/architecture.md 7, "The Region collected during layout"). If what lanes moved (knobs, value text) falls into the same group as other
  // changes in the same frame (the numbers in the top band), the bounding rectangle reaches from the top band to the mixer. So layout is
  // finished before writing positions and after what was written is on screen, to keep the groups apart.
  function layoutNow(): void {
    void document.documentElement.scrollTop;
  }
  $effect(() => {
    void wroteVersion();
    if (moving) layoutNow();
  });
</script>

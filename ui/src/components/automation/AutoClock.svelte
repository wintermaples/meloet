<!-- 帯が動かしている物のいまの位置を書く係（autoState.svelte.ts）。何も描かない。
     止まっているときは、状態か点が変わったときに再生の線の所で 1 回。動いているときは、帯があるあいだ WebGL の係のコマごと。 -->
<script lang="ts">
  import { untrack } from "svelte";
  import { everyGlFrame } from "../../gl/slot";
  import { current } from "../../state/app.svelte";
  import { anyLane, playTick, pointsVersion, refreshPositions } from "./autoState.svelte";

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
      if (s) refreshPositions(s, playTick());
    });
  });
</script>

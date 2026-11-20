<!-- 帯が動かしている物のいまの位置を書く係（autoState.svelte.ts）。何も描かない。
     止まっているときは、状態か点が変わったときに再生の線の所で 1 回。動いているときは、帯があるあいだ WebGL の係のコマごと。 -->
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

  // WebKit は、1 回のレイアウトの間に頼まれた描きなおしの四角をひとまとめにためて、こみ入ると「ぜんぶを囲む四角」にする
  // （docs/architecture.md 7 章「レイアウトでためる Region」）。帯が動かした物（つまみ、値の字）が、同じコマのほかの変化（上の帯の数字）と
  // 同じまとまりに入ると、囲む四角は上の帯からミキサーまでになる。だから、位置を書く前と、書いた物が画面にのったあとに、そこまでの
  // レイアウトをすませて、まとまりを分ける。
  function layoutNow(): void {
    void document.documentElement.scrollTop;
  }
  $effect(() => {
    void wroteVersion();
    if (moving) layoutNow();
  });
</script>

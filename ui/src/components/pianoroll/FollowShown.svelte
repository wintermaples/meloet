<!-- 見せているテイクを追う（shown.svelte.ts の followShownTake）。何も描かない。
     テイクの中身は Rust とつながってから取りにいけるので、帯の部品は、つながってからこれを置く。 -->
<script lang="ts">
  import { followShownTake } from "./shown.svelte";

  followShownTake();
</script>

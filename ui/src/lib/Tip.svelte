<!-- 用語の説明の置き場。App に 1 つだけ置く。 -->
<script lang="ts">
  import { TERM_SEPARATOR } from "../text/ja";
  import { tipView } from "./tip.svelte";

  let width = $state(0);
  let height = $state(0);
  let innerWidth = $state(0);
  let innerHeight = $state(0);

  const view = $derived(tipView.value);
  const left = $derived(view ? Math.max(0, Math.min(view.left, innerWidth - width)) : 0);
  // 下に入りきらなければ、上に出す。
  const above = $derived(view !== null && view.bottom + height > innerHeight && view.top - height >= 0);
</script>

<svelte:window bind:innerWidth bind:innerHeight />

{#if view}
  <div
    class="tip"
    class:above
    role="tooltip"
    bind:offsetWidth={width}
    bind:offsetHeight={height}
    style:left="{left}px"
    style:top={above ? null : `${view.bottom}px`}
    style:bottom={above ? `${innerHeight - view.top}px` : null}
  >
    {#if view.term !== null}<strong>{view.term}</strong>{TERM_SEPARATOR}{/if}{view.body}
  </div>
{/if}

<style>
  /* すきまの space.2 は、モック F-Connect4・F-ParamFew の見本の位置から。 */
  .tip {
    position: fixed;
    z-index: 1000;
    box-sizing: border-box;
    width: var(--part-tip-width);
    padding: var(--part-tip-pad);
    margin-top: var(--space-2);
    background: var(--color-tip-bg);
    color: var(--color-tip-fg);
    border-radius: var(--radius-1);
    font-family: var(--font-sans);
    font-size: var(--text-s-size);
    line-height: var(--text-s-line-height);
    font-weight: var(--text-s-weight-400);
    white-space: normal;
    pointer-events: none;
  }
  .tip.above {
    margin-top: 0;
    margin-bottom: var(--space-2);
  }
  strong {
    font-weight: var(--text-s-weight-600);
  }
</style>

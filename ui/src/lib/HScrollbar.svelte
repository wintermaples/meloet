<!-- 横の棒（仕様書 2 章「スクロールの棒」を横にした物、3 章「横の拡大 ・ 縮小」）。トラックの行やノートの場所のいちばん下に重ねて描く。
     場所はとらない。入りきるとき（visible >= total）は出さない。数は CSS px。 -->
<script lang="ts">
  interface Props {
    /** 中身ぜんぶの幅。 */
    total: number;
    /** 見えている幅。 */
    visible: number;
    /** 見えている所の左はし（0 〜 total − visible）。 */
    offset: number;
    /** 人が動かした。新しい offset。 */
    onscroll: (offset: number) => void;
  }
  let { total, visible, offset, onscroll }: Props = $props();
</script>

{#if visible < total}
  <div class="hbar" data-testid="hscrollbar" data-offset={offset} data-total={total} data-visible={visible} aria-hidden="true" onwheel={() => onscroll(offset)}></div>
{/if}

<style>
  .hbar {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
  }
</style>

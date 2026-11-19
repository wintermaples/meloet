<!-- 窓がひらいている印（仕様書 2 章: 10×10 の窓の形の線画、gray.10。色は足さない）。線は line.hair のまま太らせない。 -->
<svg class="mark" viewBox="0 0 10 10" aria-hidden="true" data-test="window-mark">
  <rect x="0.5" y="0.5" width="9" height="9" />
  <path d="M0.5 3h9" />
</svg>

<style>
  .mark {
    flex-shrink: 0;
    width: var(--part-windowmark-size);
    height: var(--part-windowmark-size);
    fill: none;
    stroke: var(--color-gray-10);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }
  .mark * {
    vector-effect: non-scaling-stroke;
  }
</style>

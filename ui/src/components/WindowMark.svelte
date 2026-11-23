<!-- Window-open mark (spec 2: a 10×10 line drawing of a window, gray.10, no added colors). The line stays line.hair and is not made thicker. -->
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

<!-- The "write automation" mark at the right of a parameter row (spec 2 "Parameter row", spec 3 Y16, 20×20).
     Pressing it adds, under the track, a band that writes the movement (opens it if folded). Rows with a band are accent
     (writing automation). Inserts of returns A and B and of the master make no band, so they are disabled
     (docs/history/design-notes.ja.md A-19, item 7). -->
<script lang="ts">
  import { t } from "../../text";

  let { on, usable, add }: { on: boolean; usable: boolean; add: () => void } = $props();
</script>

<button
  class="automation"
  class:on
  class:usable
  aria-disabled={!usable || undefined}
  aria-pressed={on}
  aria-label={t("params.automation")}
  data-test="param-automation"
  onpointerdown={(e) => e.stopPropagation()}
  onclick={() => usable && add()}><svg viewBox="0 0 10 10" aria-hidden="true"><path d="M0 8L3 3l3 4 4-6" /></svg></button
>

<style>
  /* Disabled: border gray.5, text gray.7, cannot be pressed. Usable: border gray.6, text gray.8. A row that writes is accent. */
  .automation {
    box-sizing: border-box;
    width: var(--part-param-automation);
    height: var(--part-param-automation);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: var(--line-hair) solid var(--color-gray-5);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-gray-7);
    cursor: default;
  }
  .automation.usable {
    border-color: var(--color-gray-6);
    color: var(--color-gray-8);
    cursor: pointer;
  }
  .automation.usable:hover {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .automation.on,
  .automation.on:hover {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  .automation svg {
    width: var(--part-button-icon);
    height: var(--part-button-icon);
    fill: none;
    stroke: currentColor;
    stroke-width: 1;
  }
</style>

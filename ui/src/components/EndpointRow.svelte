<!-- Output and input row (spec 2 "Output and input row"). 32 high with a test sound in flow 2, 24 high in the sound settings. -->
<script lang="ts">
  import { tipWhenCut } from "../lib/tip.svelte";
  import { t } from "../text";
  import type { EndpointLook } from "./endpoint";

  let {
    look,
    selected,
    disabled,
    size,
    still = false,
    onpick,
    ontest,
  }: {
    look: EndpointLook;
    selected: boolean;
    disabled: boolean;
    size: "connect" | "settings";
    /** Shown only, cannot be pressed (does not look disabled). */
    still?: boolean;
    onpick?: () => void;
    /** Only when given, a test sound button at the right end. */
    ontest?: () => void;
  } = $props();

  const connect = $derived(size === "connect");
  let row = $state<HTMLElement>();

  // Scroll the row chosen with ↑ ↓ into view inside the scrolling list.
  $effect(() => {
    if (connect && selected) row?.scrollIntoView?.({ block: "nearest" });
  });
</script>

<div
  class="row"
  class:connect
  class:highlighted={connect && selected}
  class:disabled
  data-test="endpoint-row"
  data-selected={selected}
  data-disabled={disabled}
  bind:this={row}
>
  <button type="button" class="pick" disabled={disabled || still} onclick={() => onpick?.()} aria-pressed={selected}>
    <span class="choice" class:selected><span class="dot"></span></span>
    <span class="ep-name" class:bold={connect} class:dim={look.link === "nothing"}>{look.name}</span>
    {#if look.sideIsDevice}
      <span class="friendly" use:tipWhenCut>{look.side}</span>
    {:else}
      <span class="friendly">{look.side ?? ""}</span>
    {/if}
    {#if look.link !== null}
      <span class="link" class:yes={look.link !== "nothing"}>
        <span class="link-mark"></span>
        {look.link === "plugged"
          ? t("endpoint.plugged")
          : look.link === "connected"
            ? t("endpoint.connected")
            : t("endpoint.nothingPlugged")}
      </span>
    {/if}
  </button>
  {#if ontest && !disabled}
    <button type="button" class="test" aria-label={t("endpoint.testTone")} data-test="test-tone" onclick={ontest}>
      <svg viewBox="0 0 10 10" aria-hidden="true"><path d="M1 0l8 5-8 5z" fill="currentColor" /></svg>
    </button>
  {/if}
</div>

<style>
  .row {
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    height: var(--part-endpoint-height-settings);
    white-space: nowrap;
  }
  .row.connect {
    height: var(--part-endpoint-height-connect);
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  .row.highlighted {
    background: var(--color-gray-4);
    border-left: var(--line-mark) solid var(--color-accent-ink);
    padding-left: calc(var(--space-3) - var(--line-mark));
  }
  .pick {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    height: 100%;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .pick:disabled {
    cursor: default;
  }
  .choice {
    box-sizing: border-box;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--part-choice-size);
    height: var(--part-choice-size);
    border: var(--line-hair) solid var(--color-gray-8);
  }
  .choice.selected {
    border-color: var(--color-accent-ink);
  }
  .dot {
    width: var(--part-choice-dot);
    height: var(--part-choice-dot);
  }
  .choice.selected .dot {
    background: var(--color-accent-ink);
  }
  .ep-name {
    flex-shrink: 0;
    width: var(--part-endpoint-name);
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-gray-10);
  }
  .ep-name.bold {
    font-weight: var(--text-s-weight-600);
  }
  .ep-name.dim {
    color: var(--color-gray-8);
  }
  .friendly {
    flex-shrink: 0;
    width: var(--part-endpoint-friendly);
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-gray-8);
  }
  .link {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    color: var(--color-gray-8);
  }
  .link.yes {
    color: var(--color-gray-10);
  }
  .link-mark {
    box-sizing: border-box;
    flex-shrink: 0;
    width: var(--part-endpoint-link);
    height: var(--part-endpoint-link);
    border: var(--line-hair) solid var(--color-gray-7);
  }
  .link.yes .link-mark {
    border: 0;
    background: var(--color-gray-10);
  }
  .test {
    box-sizing: border-box;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: var(--part-toggle-size);
    height: var(--part-toggle-size);
    padding: 0;
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-gray-8);
    cursor: pointer;
  }
  .test:hover {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .test svg {
    width: var(--part-button-icon);
    height: var(--part-button-icon);
  }
  .pick:focus-visible,
  .test:focus-visible {
    outline: var(--line-hair) solid var(--color-gray-10);
    outline-offset: var(--line-hair);
  }

  /* Disabled (spec 2): border gray.5, text gray.7, cannot be pressed */
  .row.disabled .ep-name,
  .row.disabled .friendly,
  .row.disabled .link {
    color: var(--color-gray-7);
  }
  .row.disabled .choice {
    border-color: var(--color-gray-5);
  }
  .row.disabled .choice .dot {
    background: none;
  }
  .row.disabled .link-mark {
    border-color: var(--color-gray-7);
  }
  .row.disabled .link.yes .link-mark {
    background: var(--color-gray-7);
  }
</style>

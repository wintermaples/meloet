<!-- Sound settings ▸ advanced settings ▸ plugin locations (spec 2 "Plugin location row", spec 3 Y13, mock R9-Scan).
     Folds by the heading (▾ ▸ and a count). Default locations are gray.8 with 「ふつうの場所」, added locations gray.10 with 「外す」.
     The protocol does not send CLAP locations (docs/history/design-notes.ja.md A-16). -->
<script lang="ts">
  import type { PluginsView } from "../../bridge";
  import { send } from "../../state/app.svelte";
  import { t } from "../../text";
  import Button from "../Button.svelte";
  import { placesReveal } from "./scan";

  let { plugins }: { plugins: PluginsView } = $props();
  const open = $derived(plugins.placesOpen);

  /** When opened from 「プラグインの場所…」 in the stopped-plugins dialog, scroll it into view. */
  function reveal(node: HTMLElement): void {
    if (!placesReveal.pending) return;
    placesReveal.pending = false;
    requestAnimationFrame(() => node.scrollIntoView?.({ block: "nearest" }));
  }
</script>

<div class="places" data-test="plugin-places" use:reveal>
  <button type="button" class="hd m-focus-in" onclick={() => send({ type: "togglePluginPlaces" })} data-test="plugin-places-head">
    <span class={open ? "m-hi" : "m-lbl"}>{open ? "▾" : "▸"}</span>
    <span class="m-hi name">{t("plugin.places")}</span>
    <span class="m-num m-lbl">{plugins.places.length}</span>
  </button>
  {#if open}
    {#each plugins.places as p (p.path)}
      <div class="row" class:added={p.added} data-test="plugin-place">
        <span class="m-num path">{p.shownPath}</span>
        {#if p.added}
          <Button test="plugin-place-remove" onclick={() => send({ type: "removePluginPlace", path: p.path })}>{t("button.remove")}</Button>
        {:else}
          <span class="m-lbl">{t("plugin.usualPlace")}</span>
        {/if}
      </div>
    {/each}
    <div class="foot">
      <Button test="plugin-place-add" onclick={() => send({ type: "addPluginPlace" })}>{t("button.addPlace")}</Button>
      <span class="m-spacer"></span>
      <Button test="plugin-rescan" onclick={() => send({ type: "rescanPlugins" })}>{t("button.rescan")}</Button>
    </div>
  {/if}
</div>

<style>
  .places {
    display: flex;
    flex-direction: column;
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-6);
    white-space: nowrap;
  }
  .hd {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--size-row);
    padding: 0 var(--space-3);
    border: 0;
    background: var(--color-gray-4);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .hd:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .name {
    flex: 1;
    min-width: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--size-row);
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-line-row);
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-gray-8);
  }
  .row.added .path {
    color: var(--color-gray-10);
  }
  .foot {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
  }
</style>

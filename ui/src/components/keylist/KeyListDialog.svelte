<!-- The key list (round 12 G1): the keys of spec 4 by group, a filter box on top, and the link to the user guide.
     Built like 「meloet について」 (spec 2 "Dialog"). Only the screen holds it (keyList.svelte.ts). -->
<script lang="ts">
  import { movableDialog } from "../../lib/dialogMove";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { keyListGroups, type KeyGroup } from "../../keys/keyList";
  import { platformOf, type Platform } from "../../keys/keys";
  import { send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import Button from "../Button.svelte";
  import { keyNames } from "../menu/menus";
  import { metrics } from "../metrics.svelte";
  import { closeKeyList, keyList, toggleKeyGroup } from "./keyList.svelte";

  let { platform = platformOf(navigator) }: { platform?: Platform } = $props();

  const searching = $derived(keyList.query.trim() !== "");
  const groups = $derived(keyListGroups(keyNames(platform), keyList.query));
  // While searching, every group with a match is open.
  const open = (g: KeyGroup): boolean => searching || !keyList.folded.includes(g);
</script>

{#if keyList.open}
  <div class="place">
    <div class="lead"></div>
    <div class="dialog" role="dialog" aria-label={t("keyList.title")} data-test="keylist-dialog" style:color-scheme={metrics.colorScheme} use:movableDialog>
      <div class="head" data-dialog-head>
        <span class="title">{t("keyList.title")}</span>
        <span class="spacer"></span>
        <button type="button" class="close" onclick={closeKeyList} data-test="keylist-close-head">
          {t("button.close")}<span class="mono">{t("key.esc")}</span>
        </button>
      </div>
      <div class="search-bar">
        <input class="search" type="text" placeholder={t("keyList.search")} bind:value={keyList.query} data-test="keylist-search" />
      </div>
      <ScrollArea data-test="keylist-body">
        {#each groups as g (g.group)}
          {@const opened = open(g.group)}
          <button type="button" class="hd m-focus-in" onclick={() => toggleKeyGroup(g.group)} data-test="keylist-group" data-group={g.group}>
            <span class={opened ? "strong" : "label"}>{opened ? "▾" : "▸"}</span>
            <span class="strong grow">{g.label}</span>
          </button>
          {#if opened}
            {#each g.rows as r, i (i)}
              <div class="row" data-test="keylist-row">
                <span class="grow">{r.label}</span>
                <span class="mono label key">{r.keys}</span>
              </div>
            {/each}
          {/if}
        {/each}
      </ScrollArea>
      <div class="foot">
        <button type="button" class="link" onclick={() => void send({ type: "openUserGuide" })} data-test="keylist-guide">{t("keyList.openGuide")}</button>
        <span class="spacer"></span>
        <Button primary test="keylist-close" onclick={closeKeyList}>{t("button.close")}</Button>
      </div>
    </div>
    <div class="lead"></div>
  </div>
{/if}

<style>
  .place {
    position: absolute;
    inset: 0;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: var(--part-dialog-margin) 0;
  }
  .lead {
    flex: 1 1 0;
  }
  .dialog {
    flex-shrink: 0;
    pointer-events: auto;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    width: var(--part-dialog-width);
    max-height: 100%;
    background: var(--color-gray-3);
    border: var(--line-hair) solid var(--color-gray-8);
    border-radius: var(--radius-1);
    color: var(--color-gray-9);
    font-family: var(--font-sans);
    font-size: var(--text-s-size);
    line-height: var(--text-s-line-height);
    font-weight: var(--text-s-weight-400);
    white-space: nowrap;
  }
  .head {
    flex-shrink: 0;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    height: var(--part-dialog-head);
    padding: 0 var(--space-4);
    border-bottom: var(--line-hair) solid var(--color-gray-6);
    cursor: move;
  }
  .head:hover {
    background: var(--color-gray-4);
  }
  .title {
    color: var(--color-gray-10);
    font-weight: var(--text-s-weight-600);
  }
  .close,
  .link {
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    cursor: pointer;
  }
  .close {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--color-gray-8);
  }
  .close:hover {
    color: var(--color-gray-10);
  }
  .link {
    color: var(--color-accent);
  }
  .link:hover {
    text-decoration: underline;
  }
  .spacer,
  .grow {
    flex: 1;
    min-width: 0;
  }
  .grow {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .label {
    color: var(--color-gray-8);
  }
  .strong {
    color: var(--color-gray-10);
  }
  .mono {
    font-family: var(--font-mono);
    font-variant-numeric: tabular-nums;
  }
  .search-bar {
    flex-shrink: 0;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    height: var(--mock-about-search);
    padding: 0 var(--space-4);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  .search {
    flex: 1;
    min-width: 0;
    height: var(--part-input-height);
    padding: 0 var(--part-input-pad-x);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    color: var(--color-gray-9);
    font: inherit;
    outline: none;
  }
  .search::placeholder {
    color: var(--color-gray-7);
  }
  .search:focus {
    border-color: var(--color-gray-8);
  }
  .hd,
  .row {
    width: 100%;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--size-row);
    white-space: nowrap;
  }
  .hd {
    padding: 0 var(--space-3);
    border: 0;
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    background: var(--color-gray-4);
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .hd:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .row {
    padding: 0 var(--space-4) 0 var(--mock-about-indent);
    border-bottom: var(--line-hair) solid var(--color-line-row);
  }
  .key {
    flex-shrink: 0;
  }
  .foot {
    flex-shrink: 0;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--part-dialog-foot);
    padding: 0 var(--space-4);
    border-top: var(--line-hair) solid var(--color-gray-6);
  }
</style>

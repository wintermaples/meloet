<!-- Right end: the open panel (shelf or params; only the one in front) and the vertical headings of folded ones.
     Mock F-Empty (vertical headings), F-WindowOpen (in the params vertical heading, the name and count of the chosen plugin). Contents are Shelf.svelte and params/ParamsPanel.svelte. -->
<script lang="ts">
  import type { Band } from "../bridge";
  import { current, send } from "../state/app.svelte";
  import { t } from "../text/ja";
  import { metrics } from "./metrics.svelte";
  import ParamsPanel from "./params/ParamsPanel.svelte";
  import { allRows, rowCount } from "./params/paramRows";
  import { pluginAt } from "./params/selected";
  import Shelf from "./Shelf.svelte";
  import { shelfCounts } from "./shelfList";
  import { shelfToggle } from "./tracks.svelte";

  const layout = $derived(current.state!.workspace.layout);
  const front = $derived(layout.rightFront);

  const RIGHT: Band[] = ["shelf", "params"];
  const tabs = $derived(RIGHT.filter((b) => b !== front));
  const toggle = (band: Band) => send(band === "shelf" ? shelfToggle(current.state!) : { type: "toggleBand", band });
  const counts = $derived(shelfCounts(current.state!.shelf, current.state!.library));
  const selected = $derived(current.state!.params.selected);
  const paramsCount = $derived.by(() => {
    if (!selected) return null;
    if (selected.kind === "builtin") {
      return t("params.tabCount", { name: t(`effect.${selected.effect}`), count: rowCount(allRows(selected.effect, selected.values)) });
    }
    if (selected.kind === "sound" || selected.kind === "clip") return null;
    const plugin = pluginAt(current.state!.song, selected.slot);
    return plugin ? t("params.tabCount", { name: plugin.name, count: selected.count }) : null;
  });

  // When the window is too low for every vertical heading, the shelf counts go first.
  let nav = $state<HTMLElement>();
  let countsFit = $state(true);
  let heightWithCounts = 0;

  function fitCounts() {
    if (!nav) return;
    if (countsFit) {
      heightWithCounts = nav.scrollHeight;
      if (heightWithCounts > nav.clientHeight) countsFit = false;
    } else if (heightWithCounts <= nav.clientHeight) {
      countsFit = true;
    }
  }

  $effect(() => {
    void metrics.scale;
    countsFit = true;
  });

  $effect(() => {
    void tabs;
    if (!nav) return;
    const observer = new ResizeObserver(fitCounts);
    observer.observe(nav);
    for (const tab of nav.children) observer.observe(tab);
    return () => observer.disconnect();
  });
</script>

{#snippet title(band: Band)}
  <span class="m-name">{band === "shelf" ? t("band.shelf") : t("band.params")}</span>
{/snippet}

{#if front === "shelf"}
  <Shelf />
{:else if front}
  <ParamsPanel onfold={() => toggle(front)} />
{/if}

<nav class="tabs" bind:this={nav}>
  {#each tabs as band, i (band)}
    {#if i > 0}<div class="rule"></div>{/if}
    <button class="m-plain tab m-focus-in" onclick={() => toggle(band)} data-band={band}>
      <span class="m-lbl">▸</span>
      <span class="vt">{@render title(band)}</span>
      {#if band === "shelf" && countsFit}
        <span class="vt m-lbl">{t("band.shelfCount", { instruments: counts.instruments, effects: counts.effects })}</span>
      {:else if band === "params" && paramsCount && countsFit}
        <span class="vt m-lbl" data-test="params-tab-count">{paramsCount}</span>
      {/if}
      <span class="m-num m-lbl">{band === "shelf" ? t("key.shelf") : t("key.params")}</span>
    </button>
  {/each}
</nav>

<style>
  .tabs {
    width: var(--size-tab-v);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-gray-4);
    border-left: var(--line-hair) solid var(--color-gray-6);
    overflow: hidden;
  }
  .tab {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) 0;
  }
  .tab:hover .m-name,
  .tab:hover .m-lbl {
    color: var(--color-gray-10);
  }
  /* Pressed (spec 2: bg gray.2, border gray.8, drawn on the inside). */
  .tab:active {
    background: var(--color-gray-2);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .rule {
    height: var(--line-hair);
    flex-shrink: 0;
    background: var(--color-gray-6);
  }
  .vt {
    writing-mode: vertical-rl;
    white-space: nowrap;
  }
</style>

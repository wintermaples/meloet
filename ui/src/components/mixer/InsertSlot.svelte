<!-- Insert slot (spec 2 "Insert slot", "Insert slot: added states", "On mark of an insert slot": height 24, empty is 「空き ＋」, chosen = accent border).
     A filled slot has an on mark on the left (12×12; on is an accent dot, bypassed is empty with the name in gray.7). Pressing it sends ToggleBypass. Stopped and empty slots have no mark.
     The name place is narrower by the mark; names that do not fit end with 「…」. Hover shows the full name (with 「（とばしている）」 for a bypassed slot).
     A plugin whose window is open has a window mark at the right end. A stopped one has a red border and 「止」; a missing one has its name in gray.8.
     Pressing a filled slot chooses that item (SelectInsert; a plugin also opens its window). Right-click opens the menu without choosing (insertMenu; rows point to this slot by InsertRef,
     because choosing opens the plugin window). Delete removes the chosen slot.
     Pressing an empty slot opens the shelf as 「（ストリップ）にエフェクトを入れる」. Also usable while recording and during count-in (A-14).
     Strip owns the reorder drag (lift: the vertical offset of the slot being moved, line: the line where it goes in). -->
<script lang="ts">
  import type { InsertRef, InsertView } from "../../bridge";
  import { tipOf } from "../../lib/tip.svelte";
  import { send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { openContextMenu } from "../menu/contextMenu.svelte";
  import { insertName } from "../sounds";
  import { openShelf } from "../tracks.svelte";
  import WindowMark from "../WindowMark.svelte";
  import { insertMenu } from "./insertMenu";

  interface Props {
    insert: InsertRef;
    view: InsertView | null;
    selected: boolean;
    windowOpen?: boolean;
    lift?: number | null;
    line?: "above" | "below" | null;
    ondown?: (e: PointerEvent) => void;
  }
  let { insert, view, selected, windowOpen = false, lift = null, line = null, ondown }: Props = $props();

  const pluginState = $derived(view?.effect.kind === "plugin" ? view.effect.state : null);
  const marked = $derived(view !== null && pluginState !== "stopped");
  const name = $derived(view ? insertName(view) : "");

  let nameEl = $state<HTMLElement>();
  const tipText = () => {
    if (!view) return null;
    if (view.bypassed) return `${name}${t("mixer.bypassedTip")}`;
    return nameEl && nameEl.scrollWidth > nameEl.clientWidth ? name : null;
  };

  // Pressing the chosen plugin's slot again brings its window to the front (spec 3 (params panel)). Built-in ones do nothing.
  function press(): void {
    if (view === null) send(openShelf({ kind: "insertEffect", strip: insert.strip, slot: insert.slot }));
    else if (!selected || view.effect.kind === "plugin") send({ type: "selectInsert", insert });
  }

  function context(e: MouseEvent): void {
    e.preventDefault();
    if (view === null) return;
    openContextMenu(e.clientX, e.clientY, insertMenu(insert, view, send));
  }

  function key(e: KeyboardEvent): void {
    if (e.key !== "Delete" || e.shiftKey || e.ctrlKey || e.metaKey || e.altKey || view === null) return;
    e.preventDefault();
    // Do not pass it on to other deletes (chosen clips or notes).
    e.stopPropagation();
    if (selected) send({ type: "removeEffect", insert });
  }
</script>

<div
  class="slot"
  class:filled={view !== null}
  class:selected
  class:stopped={pluginState === "stopped"}
  class:missing={pluginState === "missing"}
  class:bypassed={view?.bypassed === true}
  class:lifted={lift !== null}
  style:transform={lift !== null ? `translateY(${lift}px)` : null}
  role="presentation"
  data-test="mixer-insert"
  data-slot={insert.slot}
  onpointerdown={(e) => view !== null && ondown?.(e)}
  onclick={press}
  onkeydown={key}
  oncontextmenu={context}
>
  {#if marked}
    <button
      class="m-plain mark"
      class:on={!view!.bypassed}
      role="switch"
      aria-checked={!view!.bypassed}
      aria-label={t("params.bypass")}
      data-test="mixer-bypass"
      onpointerdown={(e) => e.stopPropagation()}
      onclick={(e) => {
        e.stopPropagation();
        send({ type: "toggleBypass", insert });
      }}><span></span></button
    >
  {/if}
  <button class="m-plain press" use:tipOf={tipText}>
    {#if view !== null}
      <span class="name" bind:this={nameEl}>{name}</span>
      {#if pluginState === "stopped"}<span>{t("mixer.stoppedMark")}</span>{/if}
      {#if windowOpen}<WindowMark />{/if}
    {:else}
      <span class="name">{t("mixer.emptySlot")}</span><span>{t("mixer.emptySlotAdd")}</span>
    {/if}
  </button>
  {#if line}<div class="line {line}" data-test="mixer-insert-line"></div>{/if}
</div>

<style>
  .slot {
    position: relative;
    height: var(--part-insert-height);
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: 0 0 0 var(--space-1);
    border: var(--line-hair) solid var(--color-gray-5);
    background: var(--color-gray-1);
    color: var(--color-gray-8);
    white-space: nowrap;
  }
  .slot.filled {
    border-color: var(--color-gray-7);
    color: var(--color-gray-10);
  }
  .slot:hover {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .slot:active {
    background: var(--color-gray-2);
    border-color: var(--color-gray-8);
  }
  .slot.selected,
  .slot.selected:hover {
    border-color: var(--color-accent);
  }
  .slot.missing {
    color: var(--color-gray-8);
  }
  .slot.bypassed .name {
    color: var(--color-gray-7);
  }
  .slot.stopped,
  .slot.stopped:hover {
    border-color: var(--color-rec);
    color: var(--color-rec);
  }
  /* During a reorder drag the slot is lifted with an accent line.hair (spec 2 "Insert reorder mark"). */
  .slot.lifted {
    z-index: 2;
    outline: var(--line-hair) solid var(--color-accent);
    outline-offset: calc(-1 * var(--line-hair));
  }
  .press {
    flex: 1;
    min-width: 0;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-1);
    padding: 0 var(--space-1) 0 0;
    color: inherit;
    overflow: hidden;
  }
  .slot:not(.filled) .press {
    padding-left: 0;
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    text-align: left;
  }
  /* The on mark (the 12×12 choice mark of spec 2). On is an accent dot, bypassed is empty. */
  .mark {
    flex-shrink: 0;
    box-sizing: border-box;
    width: var(--part-choice-size);
    height: var(--part-choice-size);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border: var(--line-hair) solid var(--color-gray-8);
  }
  .mark span {
    width: var(--part-choice-dot);
    height: var(--part-choice-dot);
  }
  .mark.on {
    border-color: var(--color-accent);
  }
  .mark.on span {
    background: var(--color-accent);
  }
  /* Where it goes in: an accent line.mark horizontal line (as in track reordering). Laid over the gap between slots. */
  .line {
    position: absolute;
    left: calc(-1 * var(--line-hair));
    right: calc(-1 * var(--line-hair));
    height: var(--line-mark);
    background: var(--color-accent);
    pointer-events: none;
    z-index: 3;
  }
  .line.above {
    top: calc(-1 * (var(--space-1) / 4 + var(--line-mark) / 2 + var(--line-hair)));
  }
  .line.below {
    bottom: calc(-1 * (var(--space-1) / 4 + var(--line-mark) / 2 + var(--line-hair)));
  }
</style>

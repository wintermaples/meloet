<!-- The piano roll band (mock A-PianoRoll, F-PR720, F-PR150, F-WidePR, F-PRPlay, F-PRRec).
     From the top: the toolbar; on the left 「ノートの一覧」 (a vertical heading when folded); in the middle the ruler, the keyboard column, the note area, the velocity lane and the CC tab row;
     on the right 「選んだノート」 (a vertical heading when folded). While dragging, the 「その場の読み値」 at the top left of the cursor. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { line, space } from "../../design/tokens";
  import { current } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import ChoiceMenu from "../ChoiceMenu.svelte";
  import { textWidth } from "../fit";
  import { menuOpened } from "../../lib/menu";
  import { term } from "../../lib/tip.svelte";
  import { termParts } from "../format";
  import { metrics } from "../metrics.svelte";
  import * as G from "./geom";
  import NoteInspector from "./NoteInspector.svelte";
  import CcListPanel from "./CcListPanel.svelte";
  import NoteListPanel from "./NoteListPanel.svelte";
  import Roll from "./Roll.svelte";
  import { followShownTake, shown } from "./shown.svelte";
  import Toolbar from "./Toolbar.svelte";
  import ChordRow from "./ChordRow.svelte";
  import { ccLanes, ccList, dragReading, prPanels, prView, velocityLane, type LaneTag } from "./view.svelte";

  let { height }: { height: number } = $props();

  followShownTake();

  const pr = $derived(current.state!.pianoRoll);
  const noteTotal = $derived(shown.clip?.noteCount ?? 0);
  const selectedCount = $derived(G.indexesOf(pr.selectedNotes).length);
  const cc = $derived(G.ccCounts(shown.take));

  // Left and right panels: when first shown, if the note area is less than 4 bars (640), they start folded (the band table in spec 3).
  let bandW = $state(0);
  $effect(() => {
    if (bandW <= 0 || (prPanels.list !== null && prPanels.inspector !== null)) return;
    const s = metrics.scale / 100;
    const keys = metrics.px("size.col.keys");
    const rest = bandW - keys - metrics.px("size.panel.list") - metrics.px("size.panel.insp");
    const open = rest >= 640 * s;
    prPanels.list ??= open;
    prPanels.inspector ??= open;
  });

  // CC tabs: if there is not enough width, only names and counts, and 「そのほかの CC」 is not shown (F-PR150).
  // When the velocity lane is folded, 「▸ ベロシティ」 at the left end (the band table in spec 3).
  let ccW = $state(0);
  const ccFull = $derived.by(() => {
    const pad = space["space.3"][0] * 2 + line["line.hair"];
    const gap = space["space.2"][0];
    const tag = (name: string, detail: string) => pad + textWidth("▸") + gap + textWidth(name) + gap + textWidth(detail);
    const need =
      (velocityLane.open ? 0 : pad + textWidth("▸") + gap + textWidth(t("pr.velocity"))) +
      tag(t("pr.sustain"), t("pr.sustainDetail", { count: cc.sustain })) +
      tag(t("pr.pitchBend"), t("pr.points", { count: cc.pitchBend })) +
      tag(t("pr.modulation"), t("pr.modulationDetail", { count: cc.modulation })) +
      tag(t("pr.otherCc"), "");
    return (need * metrics.scale) / 100 <= ccW;
  });

  // Pressing a CC tab opens or folds that lane (the opening order is remembered, so the oldest are folded first).
  const isOpen = (tag: LaneTag) => ccLanes.open.includes(tag);
  function toggleLane(tag: LaneTag): void {
    ccLanes.open = isOpen(tag) ? ccLanes.open.filter((x) => x !== tag) : [...ccLanes.open, tag];
  }
  const mark = (tag: LaneTag) => (isOpen(tag) ? "▾" : "▸");

  // 「そのほかの CC」: the list of CC numbers in the take other than 1 and 64 (the owner's decision. spec 2 "List", opens just above the pressed tab).
  const otherUsable = $derived(cc.others.length > 0);
  let otherMenu = $state(false);
  let otherButton = $state<HTMLButtonElement>();
  let otherPopup = $state<HTMLElement>();
  let otherAnchor = $state({ left: 0, bottom: 0 });
  const otherRows = $derived(cc.others.map((number) => ({ value: number, label: t("pr.ccNumber", { number: String(number) }) })));
  function pressOther(): void {
    if (isOpen("other")) toggleLane("other");
    else if (otherUsable) otherMenu = !otherMenu;
  }
  function pickOther(number: number): void {
    ccLanes.other = number;
    if (!isOpen("other")) toggleLane("other");
    closeOther();
  }
  function closeOther(): void {
    const hadFocus = otherPopup?.contains(document.activeElement) ?? false;
    otherMenu = false;
    if (hadFocus) otherButton?.focus();
  }
  function placeOther(): void {
    if (!otherButton) return;
    const r = otherButton.getBoundingClientRect();
    otherAnchor = { left: r.left, bottom: window.innerHeight - r.top };
  }
  function toBody(node: HTMLElement): { destroy(): void } {
    document.body.append(node);
    return { destroy: () => node.remove() };
  }
  $effect(() => {
    if (!otherMenu) return;
    placeOther();
    window.addEventListener("resize", placeOther);
    const forget = menuOpened(closeOther);
    const onDown = (ev: PointerEvent) => {
      const target = ev.target as Node | null;
      if (target && (otherPopup?.contains(target) || otherButton?.contains(target))) return;
      closeOther();
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => {
      forget();
      window.removeEventListener("resize", placeOther);
      document.removeEventListener("pointerdown", onDown, true);
    };
  });

  // The reading in place (spec 2): shown 6 away to the top left of the cursor. At the band's edges it is moved inward.
  let band = $state<HTMLElement>();
  let readingAt = $state<{ x: number; y: number } | null>(null);
  let readingW = $state(0);
  let readingH = $state(0);
  function onreading(at: { x: number; y: number } | null): void {
    if (!at || !band) {
      readingAt = null;
      return;
    }
    const r = band.getBoundingClientRect();
    readingAt = { x: at.x - r.left, y: at.y - r.top };
  }
  const readingBox = $derived.by(() => {
    if (!readingAt || !dragReading.text) return null;
    const off = metrics.px("part.reading.offset");
    return { left: Math.max(0, readingAt.x - off - readingW), top: Math.max(0, readingAt.y - off - readingH) };
  });

  // Delete and Mod A go to the place pressed last (piano roll or arrange).
  onMount(() => {
    const onDown = (ev: PointerEvent) => {
      const target = ev.target as Element | null;
      if (!target?.closest) return;
      if (band?.contains(target)) prView.focus = "pianoRoll";
      else if (target.closest("section.arrange, section.overview")) prView.focus = "arrange";
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => document.removeEventListener("pointerdown", onDown, true);
  });
</script>

{#snippet noteWord(text: string)}{#each termParts(text, "ノート") as piece, j (j)}{#if piece.term}<span use:term={"ノート"}
        >{piece.text}</span
      >{:else}{piece.text}{/if}{/each}{/snippet}

<section class="band" style:height="{metrics.scaled(height)}px" data-band="pianoRoll" bind:this={band} bind:clientWidth={bandW}>
  <Toolbar />
  <ChordRow />
  <div class="body">
    {#if prPanels.list !== false}
      <div class="panel list">
        <button class="m-plain hd m-focus-in" onclick={() => (prPanels.list = false)} data-test="pr-list-fold">
          <span class="m-hi">▾</span>
          <span class="m-hi grow">{@render noteWord(t("pr.noteList"))}</span>
          <span class="m-num m-lbl">{noteTotal}</span>
        </button>
        <div class="panel-body"><NoteListPanel /></div>
        <button
          class="m-plain hd cc-list m-focus-in"
          class:open={ccList.open}
          aria-expanded={ccList.open}
          data-test="pr-cc-list"
          onclick={() => (ccList.open = !ccList.open)}
        >
          <span class={ccList.open ? "m-hi" : "m-lbl"}>{ccList.open ? "▾" : "▸"}</span>
          <span class="m-hi grow">{t("pr.ccList")}</span>
          <span class="m-num m-lbl">{cc.total}</span>
        </button>
        {#if ccList.open}<div class="panel-body"><CcListPanel /></div>{/if}
      </div>
    {:else}
      <button class="m-plain vtab left m-focus-in" onclick={() => (prPanels.list = true)} data-test="pr-list-open">
        <span class="m-lbl">▸</span>
        <span class="vt m-name">{@render noteWord(t("pr.noteList"))}</span>
        <span class="vt m-num m-lbl">{noteTotal}</span>
      </button>
    {/if}
    <div class="center">
      <Roll {onreading} />
      <div class="cc-row" bind:clientWidth={ccW}>
        {#if !velocityLane.open}
          <button class="m-plain cc m-focus-in" aria-expanded="false" onclick={() => (velocityLane.open = true)} data-test="pr-velocity-open">
            <span class="m-lbl">▸</span><span class="m-hi" use:term={"ベロシティ"}>{t("pr.velocity")}</span>
          </button>
        {/if}
        <button class="m-plain cc m-focus-in" aria-expanded={isOpen("sustain")} onclick={() => toggleLane("sustain")} data-test="pr-cc-sustain">
          <span class="m-lbl">{mark("sustain")}</span><span class="m-hi" use:term={"サステイン"}>{t("pr.sustain")}</span>
          <span class="m-lbl">{#if ccFull}{t("pr.sustainDetail", { count: cc.sustain })}{:else}<span class="m-num">{cc.sustain}</span>{/if}</span>
        </button>
        <button class="m-plain cc m-focus-in" aria-expanded={isOpen("pitchBend")} onclick={() => toggleLane("pitchBend")} data-test="pr-cc-pitchBend">
          <span class="m-lbl">{mark("pitchBend")}</span><span class="m-hi" use:term={"ピッチベンド"}>{t("pr.pitchBend")}</span>
          <span class="m-lbl">{#if ccFull}{t("pr.points", { count: cc.pitchBend })}{:else}<span class="m-num">{cc.pitchBend}</span>{/if}</span>
        </button>
        <button class="m-plain cc m-focus-in" aria-expanded={isOpen("modulation")} onclick={() => toggleLane("modulation")} data-test="pr-cc-modulation">
          <span class="m-lbl">{mark("modulation")}</span><span class="m-hi" use:term={"モジュレーション"}>{t("pr.modulation")}</span>
          <span class="m-lbl">{#if ccFull}{t("pr.modulationDetail", { count: cc.modulation })}{:else}<span class="m-num">{cc.modulation}</span>{/if}</span>
        </button>
        {#if ccFull}
          <button
            class="m-plain cc m-focus-in"
            class:unusable={!otherUsable && !isOpen("other")}
            aria-disabled={!otherUsable && !isOpen("other")}
            aria-haspopup="menu"
            aria-expanded={isOpen("other") || otherMenu}
            bind:this={otherButton}
            onclick={pressOther}
            data-test="pr-cc-other"
          >
            <span class="m-lbl">{mark("other")}</span><span class="m-hi"
              >{#each termParts(t("pr.otherCc"), "CC") as piece, j (j)}{#if piece.term}<span use:term={"CC"}>{piece.text}</span
                  >{:else}{piece.text}{/if}{/each}</span
            >
            {#if isOpen("other") && ccLanes.other !== null}<span class="m-lbl">{t("pr.ccNumber", { number: String(ccLanes.other) })}</span>{/if}
          </button>
        {/if}
      </div>
    </div>
    {#if prPanels.inspector !== false}
      <div class="panel inspector">
        <button class="m-plain hd m-focus-in" onclick={() => (prPanels.inspector = false)} data-test="pr-inspector-fold">
          <span class="m-hi">▾</span>
          <span class="m-hi grow">{@render noteWord(t("pr.selectedNotes"))}</span>
          <span class="m-num" class:accent={selectedCount > 0} class:m-lbl={selectedCount === 0}>{selectedCount}</span>
        </button>
        <div class="panel-body"><NoteInspector /></div>
      </div>
    {:else}
      <button class="m-plain vtab right m-focus-in" onclick={() => (prPanels.inspector = true)} data-test="pr-inspector-open">
        <span class="m-lbl">▸</span>
        <span class="vt m-name">{@render noteWord(t("pr.selectedNotes"))}</span>
        <span class="vt m-num m-lbl">{selectedCount}</span>
      </button>
    {/if}
  </div>
  {#if otherMenu}
    <div class="popup" style:left="{otherAnchor.left}px" style:bottom="{otherAnchor.bottom}px" use:toBody bind:this={otherPopup}>
      <ChoiceMenu heading={null} label={t("pr.otherCc")} rows={otherRows} selected={ccLanes.other ?? undefined} onpick={pickOther} test="pr-cc-other-menu" />
    </div>
  {/if}
  {#if readingBox}
    <div
      class="m-num reading"
      style:left="{readingBox.left}px"
      style:top="{readingBox.top}px"
      bind:clientWidth={readingW}
      bind:clientHeight={readingH}
      data-test="pr-reading"
    >
      {dragReading.text}
    </div>
  {/if}
</section>

<style>
  .band {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    position: relative;
    background: var(--color-gray-1);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .panel {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: var(--color-gray-3);
    overflow: hidden;
  }
  .list {
    width: var(--size-panel-list);
    border-right: var(--line-hair) solid var(--color-gray-6);
  }
  .inspector {
    width: var(--size-panel-insp);
    border-left: var(--line-hair) solid var(--color-gray-6);
  }
  .hd {
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    white-space: nowrap;
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  .panel-body {
    flex: 1;
    min-height: 0;
  }
  /* The 「CC の一覧」 heading is below the note list. When open, the list is below it (sharing the height with the note list). */
  .cc-list {
    border-bottom: 0;
    border-top: var(--line-hair) solid var(--color-line-thin);
  }
  .cc-list.open {
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  .accent {
    color: var(--color-accent);
  }
  .vtab {
    width: var(--size-tab-v);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) 0;
    background: var(--color-gray-4);
    overflow: hidden;
  }
  .vtab.left {
    border-right: var(--line-hair) solid var(--color-gray-6);
  }
  .vtab.right {
    border-left: var(--line-hair) solid var(--color-gray-6);
  }
  .vtab:hover .m-name,
  .vtab:hover .m-lbl {
    color: var(--color-gray-10);
  }
  /* Pressed (spec 2: bg gray.2, border gray.8. The border is drawn inside). */
  .hd:not(.cc-list):active,
  .vtab:active,
  .cc:not(.unusable):active {
    background: var(--color-gray-2);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .vt {
    writing-mode: vertical-rl;
    white-space: nowrap;
  }
  .center {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .cc-row {
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    background: var(--color-gray-4);
    border-top: var(--line-hair) solid var(--color-gray-6);
    white-space: nowrap;
    overflow: hidden;
  }
  .cc {
    height: 100%;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    border-right: var(--line-hair) solid var(--color-gray-6);
  }
  .cc.unusable,
  .cc.unusable span {
    color: var(--color-gray-7);
    cursor: default;
  }
  .popup {
    position: fixed;
    z-index: 40;
  }
  .reading {
    position: absolute;
    z-index: 10;
    height: var(--part-reading-height);
    padding: 0 var(--part-reading-pad-x);
    display: flex;
    align-items: center;
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-8);
    color: var(--color-gray-10);
    white-space: nowrap;
    pointer-events: none;
  }
</style>

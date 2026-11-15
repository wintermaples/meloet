<!-- ピアノロールの帯（モック A-PianoRoll・F-PR720・F-PR150・F-WidePR・F-PRPlay・F-PRRec）。
     上から道具の列、左に「ノートの一覧」（たたむと縦の見出し）、まん中に目盛と鍵盤の列とノートの場所とベロシティの帯と CC の札の行、
     右に「選んだノート」（たたむと縦の見出し）。ドラッグ中は、カーソルの左上に「その場の読み値」。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import { line, space } from "../../design/tokens";
  import { current } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import ChoiceMenu from "../ChoiceMenu.svelte";
  import { textWidth } from "../fit";
  import { menuOpened } from "../../lib/menu";
  import { metrics } from "../metrics.svelte";
  import * as G from "./geom";
  import NoteInspector from "./NoteInspector.svelte";
  import NoteListPanel from "./NoteListPanel.svelte";
  import Roll from "./Roll.svelte";
  import { followShownTake, shown } from "./shown.svelte";
  import Toolbar from "./Toolbar.svelte";
  import { ccLanes, dragReading, prPanels, prView, type LaneTag } from "./view.svelte";

  let { height }: { height: number } = $props();

  followShownTake();

  const pr = $derived(current.state!.pianoRoll);
  const noteTotal = $derived(shown.clip?.noteCount ?? 0);
  const selectedCount = $derived(G.indexesOf(pr.selectedNotes).length);
  const cc = $derived(G.ccCounts(shown.take));

  // 左右の欄: はじめて出したとき、ノートの場所が 4 小節ぶん（640）を下まわるなら、たたんで始める（仕様書 3 章の帯の表）。
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

  // CC の札: 横が足りなければ、名前と数だけにして「そのほかの CC」を出さない（F-PR150）。
  let ccW = $state(0);
  const ccFull = $derived.by(() => {
    const pad = space["space.3"][0] * 2 + line["line.hair"];
    const gap = space["space.2"][0];
    const tag = (name: string, detail: string) => pad + textWidth("▸") + gap + textWidth(name) + gap + textWidth(detail);
    const need =
      tag(t("pr.sustain"), t("pr.sustainDetail", { count: cc.sustain })) +
      tag(t("pr.pitchBend"), t("pr.points", { count: cc.pitchBend })) +
      tag(t("pr.modulation"), t("pr.modulationDetail", { count: cc.modulation })) +
      tag(t("pr.otherCc"), "");
    return (need * metrics.scale) / 100 <= ccW;
  });

  // CC の札を押すと、その帯をひらく・たたむ（ひらいた順を覚える。古いほうからたたむため）。
  const isOpen = (tag: LaneTag) => ccLanes.open.includes(tag);
  function toggleLane(tag: LaneTag): void {
    ccLanes.open = isOpen(tag) ? ccLanes.open.filter((x) => x !== tag) : [...ccLanes.open, tag];
  }
  const mark = (tag: LaneTag) => (isOpen(tag) ? "▾" : "▸");

  // 「そのほかの CC」: テイクにある 1・64 のほかの CC の番号の一覧（ヤチヨさんの決め 2026-11-15。仕様書 2 章「一覧」、押した札のすぐ上にひらく）。
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

  // その場の読み値（仕様書 2 章）: カーソルの左上 6 にはなして出す。帯のはしでは内がわへずらす。
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

  // Delete や Mod A を、最後に押した所（ピアノロールかアレンジ）に向ける。
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

<section class="band" style:height="{metrics.scaled(height)}px" data-band="pianoRoll" bind:this={band} bind:clientWidth={bandW}>
  <Toolbar />
  <div class="body">
    {#if prPanels.list !== false}
      <div class="panel list">
        <button class="m-plain hd" onclick={() => (prPanels.list = false)} data-test="pr-list-fold">
          <span class="m-hi">▾</span>
          <span class="m-hi grow">{t("pr.noteList")}</span>
          <span class="m-num m-lbl">{noteTotal}</span>
        </button>
        <div class="panel-body"><NoteListPanel /></div>
        <button class="m-plain hd cc-list" aria-disabled="true" tabindex="-1">
          <span>▸</span>
          <span class="grow">{t("pr.ccList")}</span>
          <span class="m-num">{cc.total}</span>
        </button>
      </div>
    {:else}
      <button class="m-plain vtab left" onclick={() => (prPanels.list = true)} data-test="pr-list-open">
        <span class="m-lbl">▸</span>
        <span class="vt m-name">{t("pr.noteList")}</span>
        <span class="vt m-num m-lbl">{noteTotal}</span>
      </button>
    {/if}
    <div class="center">
      <Roll {onreading} />
      <div class="cc-row" bind:clientWidth={ccW}>
        <button class="m-plain cc" aria-expanded={isOpen("sustain")} onclick={() => toggleLane("sustain")} data-test="pr-cc-sustain">
          <span class="m-lbl">{mark("sustain")}</span><span class="m-hi">{t("pr.sustain")}</span>
          <span class="m-lbl">{#if ccFull}{t("pr.sustainDetail", { count: cc.sustain })}{:else}<span class="m-num">{cc.sustain}</span>{/if}</span>
        </button>
        <button class="m-plain cc" aria-expanded={isOpen("pitchBend")} onclick={() => toggleLane("pitchBend")} data-test="pr-cc-pitchBend">
          <span class="m-lbl">{mark("pitchBend")}</span><span class="m-hi">{t("pr.pitchBend")}</span>
          <span class="m-lbl">{#if ccFull}{t("pr.points", { count: cc.pitchBend })}{:else}<span class="m-num">{cc.pitchBend}</span>{/if}</span>
        </button>
        <button class="m-plain cc" aria-expanded={isOpen("modulation")} onclick={() => toggleLane("modulation")} data-test="pr-cc-modulation">
          <span class="m-lbl">{mark("modulation")}</span><span class="m-hi">{t("pr.modulation")}</span>
          <span class="m-lbl">{#if ccFull}{t("pr.modulationDetail", { count: cc.modulation })}{:else}<span class="m-num">{cc.modulation}</span>{/if}</span>
        </button>
        {#if ccFull}
          <button
            class="m-plain cc"
            class:unusable={!otherUsable && !isOpen("other")}
            aria-disabled={!otherUsable && !isOpen("other")}
            aria-haspopup="menu"
            aria-expanded={isOpen("other") || otherMenu}
            bind:this={otherButton}
            onclick={pressOther}
            data-test="pr-cc-other"
          >
            <span class="m-lbl">{mark("other")}</span><span class="m-hi">{t("pr.otherCc")}</span>
            {#if isOpen("other") && ccLanes.other !== null}<span class="m-lbl">{t("pr.ccNumber", { number: String(ccLanes.other) })}</span>{/if}
          </button>
        {/if}
      </div>
    </div>
    {#if prPanels.inspector !== false}
      <div class="panel inspector">
        <button class="m-plain hd" onclick={() => (prPanels.inspector = false)} data-test="pr-inspector-fold">
          <span class="m-hi">▾</span>
          <span class="m-hi grow">{t("pr.selectedNotes")}</span>
          <span class="m-num" class:accent={selectedCount > 0} class:m-lbl={selectedCount === 0}>{selectedCount}</span>
        </button>
        <div class="panel-body"><NoteInspector /></div>
      </div>
    {:else}
      <button class="m-plain vtab right" onclick={() => (prPanels.inspector = true)} data-test="pr-inspector-open">
        <span class="m-lbl">▸</span>
        <span class="vt m-name">{t("pr.selectedNotes")}</span>
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
    border-bottom: var(--line-hair) solid var(--color-gray-5);
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
  .cc-list {
    border-bottom: 0;
    border-top: var(--line-hair) solid var(--color-gray-5);
    color: var(--color-gray-7);
    cursor: default;
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

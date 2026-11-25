<!-- About meloet (spec 3; choice 10 is 10-A made into 2 pages; mock R7-About, R7-AboutLicense; spec 2 "Dialog",
     "Row to the second page of a dialog", "Scroll bar"). Only the screen holds it (about.svelte.ts). -->
<script lang="ts">
  import { movableDialog } from "../../lib/dialogMove";
  import ScrollArea from "../../lib/ScrollArea.svelte";
  import { current } from "../../state/app.svelte";
  import { hasText, STATUS_JOIN, t } from "../../text";
  import Button from "../Button.svelte";
  import { metrics } from "../metrics.svelte";
  import { about, closeAbout, showFirstStage, showLicenses, toggleGroup, togglePart, toggleSection } from "./about.svelte";
  import {
    allParts,
    fontsMatching,
    fullText,
    licenseList,
    licenseText,
    loadTexts,
    partGroups,
    partKey,
    soundLicenseText,
    soundsMatching,
    type TextsFile,
  } from "./licenses";

  let { list = licenseList, load = loadTexts }: { list?: typeof licenseList; load?: () => Promise<TextsFile | null> } = $props();

  const searching = $derived(about.query.trim() !== "");
  /** Location of the sound pack (spec 3, the place to fetch the sound pack later: shown inside the 「音の素材」 heading). */
  const soundsPlace = $derived(current.state?.sounds.place ?? null);
  /** A newer version that 「新しい版をたしかめる」 found (round 12, D1). */
  const newVersion = $derived(current.state?.updates.newVersion ?? null);
  const sounds = $derived(soundsMatching(list.sounds, about.query));
  const fonts = $derived(fontsMatching(list.fonts, about.query));
  const parts = $derived(allParts(list));
  const groups = $derived(partGroups(parts, about.query));
  const groupTotals = $derived(new Map(partGroups(parts, "").map((g) => [g.group, g.parts.length])));
  // While searching, a heading counts 「合う数 / 全部」 (spec 3).
  const countText = (shown: number, total: number): string => (searching ? t("shelf.matchCount", { shown, total }) : String(shown));

  let fullTexts = $state<TextsFile | null>(null);
  $effect(() => {
    if (about.stage === 2 && !fullTexts) void load().then((f) => (fullTexts = f));
  });
  const textOf = (key: string): string => (fullTexts ? (fullText(fullTexts, key) ?? "") : "");

  const changesOf = (id: string): string => {
    const key = `about.changes.${id}`;
    return hasText(key) ? t(key) : "";
  };
  // While searching, open the headings that have matches.
  const groupOpen = (g: string): boolean => searching || about.groups.includes(g);
</script>

{#snippet textBox(key: string)}
  <div class="text-box" data-test="about-text">
    <ScrollArea>
      <div class="text">{textOf(key)}</div>
    </ScrollArea>
  </div>
{/snippet}

{#snippet itemRow(key: string, name: string, right: string)}
  {@const opened = about.part === key}
  <button type="button" class="row m-focus-in" class:opened onclick={() => togglePart(key)} data-test="about-part" data-key={key}>
    <span class="mark strong">{opened ? "▾" : ""}</span>
    <span class="strong grow">{name}</span>
    <span class="mono label">{right}</span>
  </button>
  {#if opened}{@render textBox(key)}{/if}
{/snippet}

{#snippet sectionHead(name: string, count: string, open: boolean, onclick: () => void, test: string)}
  <button type="button" class="hd m-focus-in" {onclick} data-test={test}>
    <span class={open ? "strong" : "label"}>{open ? "▾" : "▸"}</span>
    <span class="strong grow">{name}</span>
    <span class="mono label">{count}</span>
  </button>
{/snippet}

{#if about.open}
  <div class="place">
    <div class="lead"></div>
    <div class="dialog" role="dialog" aria-label={t("about.title")} data-test="about-dialog" style:color-scheme={metrics.colorScheme} use:movableDialog>
      <div class="head" data-dialog-head>
        {#if about.stage === 1}
          <span class="title">{t("about.title")}</span>
        {:else}
          <span class="title">
            <button type="button" class="back" onclick={showFirstStage} data-test="about-back">{t("about.backTo", { name: t("about.title") })}</button
            >{STATUS_JOIN}{t("about.openSource")}
          </span>
        {/if}
        <span class="spacer"></span>
        {#if about.stage === 1}
          <button type="button" class="close" onclick={closeAbout} data-test="about-close-head">
            {t("about.close")}<span class="mono">{t("key.esc")}</span>
          </button>
        {:else}
          <button type="button" class="close" onclick={showFirstStage} data-test="about-back-head">
            {t("about.back")}<span class="mono">{t("key.esc")}</span>
          </button>
        {/if}
      </div>

      {#if about.stage === 1}
        <ScrollArea data-test="about-body">
          <div class="name-box">
            <span class="name">{t("about.name")}</span>
            <span class="label">{t("about.tagline")}</span>
          </div>
          <div class="facts">
            <div class="fact">
              <span class="label fact-label">{t("about.version")}</span>
              <span class="mono strong" data-test="about-version">{list.meloet.version}</span>
            </div>
            {#if newVersion}
              <div class="fact">
                <span class="fact-label"></span>
                <span class="strong" data-test="about-new-version">{t("update.newVersion", { version: newVersion })}</span>
              </div>
            {/if}
            <div class="fact">
              <span class="label fact-label">{t("about.license")}</span>
              <span class="strong">{licenseText(list.meloet.license)}</span>
            </div>
          </div>
          <button type="button" class="next" onclick={showLicenses} data-test="about-licenses">
            <span class="strong grow">{t("about.openSource")}</span>
            <span class="label" data-test="about-summary">{t("about.openSourceSummary", { count: parts.length })}</span>
            <span class="label">▸</span>
          </button>
        </ScrollArea>
      {:else}
        <ScrollArea data-test="about-body">
          <label class="search-bar">
            <span class="label">{t("about.search")}</span>
            <input class="search" type="text" placeholder={t("about.searchLabel")} bind:value={about.query} data-test="about-search" />
          </label>

          {#if sounds.length}
            {@render sectionHead(t("about.sounds"), countText(sounds.length, list.sounds.length), searching || about.sections.sounds, () => toggleSection("sounds"), "about-sounds")}
            {#if searching || about.sections.sounds}
              {#if soundsPlace}
                <div class="fields" data-test="about-sounds-place"><div class="field"><span class="mono label">{soundsPlace}</span></div></div>
              {/if}
              {#each sounds as s (s.id)}
                <div class="fields" data-test="about-sound">
                  <div class="field"><span class="label field-label">{t("about.soundName")}</span><span class="strong">{s.title}</span></div>
                  <div class="field"><span class="label field-label">{t("about.soundCreator")}</span><span class="strong">{s.creator}</span></div>
                  <div class="field">
                    <span class="label field-label">{t("about.soundLicense")}</span><span class="strong">{soundLicenseText(s.license)}</span>
                  </div>
                  <div class="field"><span class="label field-label">{t("about.soundSource")}</span><span class="strong mono">{s.source}</span></div>
                  <div class="field">
                    <span class="label field-label">{t("about.soundChanges")}</span><span class="strong">{changesOf(s.id)}</span>
                  </div>
                </div>
              {/each}
            {/if}
          {/if}

          {#if fonts.length}
            {@render sectionHead(t("about.fonts"), countText(fonts.length, list.fonts.length), searching || about.sections.fonts, () => toggleSection("fonts"), "about-fonts")}
            {#if searching || about.sections.fonts}
              {#each fonts as f (f.package)}
                {@render itemRow(partKey({ name: f.package, version: f.version }), f.name, f.license)}
              {/each}
            {/if}
          {/if}

          {#if groups.length}
            <div class="parts-label"><span class="label">{t("about.parts")}</span></div>
            {#each groups as g (g.group)}
              {@const open = groupOpen(g.group)}
              {@render sectionHead(g.label, countText(g.parts.length, groupTotals.get(g.group) ?? g.parts.length), open, () => toggleGroup(g.group), "about-group")}
              {#if open}
                {#each g.parts as p (partKey(p))}
                  {@render itemRow(partKey(p), p.name, p.version)}
                {/each}
              {/if}
            {/each}
          {/if}
        </ScrollArea>
      {/if}

      <div class="foot">
        <Button primary test="about-close" onclick={closeAbout}>{t("about.close")}</Button>
      </div>
    </div>
    <div class="lead"></div>
  </div>
{/if}

<style>
  /* The top is the mock position. If the window is low, move it up to a gap of 24 above and below; if it still does not fit, the content scrolls. */
  .place {
    position: absolute;
    inset: 0;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: var(--part-dialog-margin) 0;
  }
  /* Centred vertically in the window (spec 3 82). */
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
  /* Hovering or moving the heading row: bg gray.4 (spec 2 "Dialog heading row (moving)"). */
  .head:hover {
    background: var(--color-gray-4);
  }
  .title {
    color: var(--color-gray-10);
    font-weight: var(--text-s-weight-600);
  }
  .back,
  .close {
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    cursor: pointer;
  }
  .back {
    color: var(--color-gray-10);
  }
  .close {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    color: var(--color-gray-8);
  }
  .back:hover,
  .close:hover {
    color: var(--color-gray-10);
  }
  .spacer,
  .grow {
    flex: 1;
    min-width: 0;
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

  /* First page */
  .name-box {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--mock-about-name-pad-top) var(--space-4) var(--space-4);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  .name {
    color: var(--color-gray-10);
    font-size: var(--text-l-size);
    line-height: var(--text-l-line-height);
    font-weight: var(--text-l-weight-600);
  }
  .facts {
    display: flex;
    flex-direction: column;
    padding: var(--space-2) var(--space-4);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  .fact {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--size-row);
  }
  .fact-label {
    flex-shrink: 0;
    width: var(--mock-about-label);
  }
  /* Row to the second page of a dialog (spec 2: height 32, 16 left and right, a line.thin line below). */
  .next {
    width: 100%;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--part-dialognext-height);
    padding: 0 var(--space-4);
    border: 0;
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  /* The last row sits on the foot's top line; that line is its line below. */
  .next:last-child {
    border-bottom: 0;
  }
  .next:hover {
    background: var(--color-gray-4);
  }

  /* Second page */
  .search-bar {
    display: flex;
    align-items: center;
    gap: var(--space-2);
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
    display: flex;
    align-items: center;
    gap: var(--space-2);
    height: var(--size-row);
    border: 0;
    color: inherit;
    font: inherit;
    text-align: left;
    white-space: nowrap;
    cursor: pointer;
  }
  .hd {
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  .row {
    padding: 0 var(--space-3) 0 var(--mock-about-indent);
    background: none;
    border-bottom: var(--line-hair) solid var(--color-line-row);
  }
  .row.opened {
    background: var(--color-gray-4);
  }
  .hd:hover,
  .row:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .grow {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .fields {
    display: flex;
    flex-direction: column;
    padding: var(--mock-about-fields-pad-top) 0 var(--space-2);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  .field {
    display: flex;
    gap: var(--space-2);
    padding: var(--mock-about-field-pad-y) var(--space-4) var(--mock-about-field-pad-y) var(--mock-about-indent);
    white-space: normal;
    overflow-wrap: anywhere;
  }
  .field-label {
    flex-shrink: 0;
    width: var(--mock-about-field);
  }
  .parts-label {
    display: flex;
    align-items: center;
    height: var(--size-row);
    padding: 0 var(--space-4);
    background: var(--color-gray-1);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  /* Full text box: only the inside of this box scrolls. */
  .text-box {
    display: flex;
    flex-direction: column;
    height: var(--mock-about-text);
    margin: 0 var(--space-4) var(--space-2) var(--mock-about-indent);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-line-thin);
  }
  .text {
    padding: var(--space-2);
    color: var(--color-gray-8);
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    -webkit-user-select: text;
    user-select: text;
  }
  .foot {
    flex-shrink: 0;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: var(--space-2);
    height: var(--part-dialog-foot);
    padding: 0 var(--space-4);
    border-top: var(--line-hair) solid var(--color-gray-6);
  }
</style>

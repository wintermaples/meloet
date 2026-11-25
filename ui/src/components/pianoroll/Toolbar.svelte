<!-- The piano roll toolbar (the top row of the band in mock A-PianoRoll, F-PRPlay, F-PRRec).
     「▾ ピアノロール」 (pressing folds it; B-74), the track name, the take switch tag, the tools (select, draw, erase, with keys at the right end), horizontal − ＋ (the same shape as R3-Arrange),
     grid (a list), triplet, scale lock and the scale ▾ (B-65). On a drum track, scale lock and ▾ are 「使えない」 (spec 3, spec 6). If there is not enough width, the tool keys and then the track name are hidden (F-PR150, B-73).
     Right of the track name, 「ほかのトラックも見る ▾」 (round 12 P2; 「使えない」 on a drum track); its label is hidden first, leaving the ▾.
     When keys go to the piano roll, an accent line on the top edge (B-72). -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import type { Command, ScaleMode } from "../../bridge";
  import { songBusy } from "../../keys/attach";
  import { menuOpened } from "../../lib/menu";
  import { term, tip } from "../../lib/tip.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t, type TextKey } from "../../text";
  import ChoiceMenu from "../ChoiceMenu.svelte";
  import { toolbarWithGhost, type ToolbarParts } from "../fit";
  import { gridText, scaleName, termParts, textParts } from "../format";
  import { metrics } from "../metrics.svelte";
  import { rollShowsDrums } from "./drums";
  import * as G from "./geom";
  import { ghostCandidates, ghostHidden, shownTrackId, toggleGhostTrack } from "./ghosts";
  import ScaleMenu from "./ScaleMenu.svelte";
  import { chooseChord, chordPick } from "./chordPick.svelte";
  import { CHORD_SHAPES, pickName, shapeName, shapeOnC, type ChordShape } from "./chords";
  import { shown } from "./shown.svelte";
  import { ghostView, prView, zoomBy, type Tool } from "./view.svelte";

  const app = $derived(current.state!);
  const pr = $derived(app.pianoRoll);
  const clip = $derived(shown.clip);
  const track = $derived(shown.track);
  const rec = $derived(
    app.transport.state === "recording" && app.transport.recording && clip && app.transport.recording.clip === clip.id
      ? app.transport.recording
      : null,
  );
  const drums = $derived(rollShowsDrums(app, clip, track));

  // The name of the track the piano roll shows (user:2026-11-24). Where there is no clip (spec 3): the track of 「弾くと鳴る先」.
  const info = $derived.by((): string | null => {
    if (clip) return track?.name ?? null;
    if (app.transport.recording !== null) return null;
    return app.song.tracks.find((x) => x.id === app.song.keysTarget)?.name ?? null;
  });

  // If there is not enough width, the tool keys and then the track name are not shown. Its width can be measured with scrollWidth even when not shown.
  // The key width is measured with a hidden copy (ruler).
  let row = $state<HTMLElement>();
  let ruler = $state<HTMLElement>();
  let ghostRuler = $state<HTMLElement>();
  let infoNode = $state<HTMLElement>();
  let parts = $state<ToolbarParts & { ghost: boolean }>({ keys: true, info: true, ghost: true });
  const infoFits = $derived(parts.info);

  function measure(): void {
    if (!row || !ruler) return;
    const style = getComputedStyle(row);
    const gap = parseFloat(style.columnGap) || 0;
    const pad = (parseFloat(style.paddingLeft) || 0) + (parseFloat(style.paddingRight) || 0);
    const children = [...row.children].filter((el) => !el.classList.contains("ruler") && !el.classList.contains("focus-mark")) as HTMLElement[];
    const fixed = children.filter((el) => el !== infoNode);
    const widths = fixed.reduce((sum, el) => sum + (el.classList.contains("m-spacer") ? 0 : el.offsetWidth), 0);
    const keys = [...ruler.children].reduce((sum, el) => sum + (el as HTMLElement).offsetWidth + metrics.px("space.1"), 0);
    const ghost = ghostRuler ? ghostRuler.offsetWidth + gap : 0;
    const base = pad + widths + gap * Math.max(0, fixed.length - 1) - (parts.keys ? keys : 0) - (parts.ghost ? ghost : 0);
    parts = toolbarWithGhost(row.clientWidth, { base, keys, info: infoNode ? infoNode.scrollWidth + gap : 0 }, ghost);
  }
  onMount(() => {
    const observer = new ResizeObserver(() => measure());
    if (row) observer.observe(row);
    if (ruler) observer.observe(ruler);
    return () => observer.disconnect();
  });
  $effect(() => {
    void [info, takeTag, gridName, scaleText, drawChord, metrics.scale];
    untrack(measure);
  });

  const takeTag = $derived.by(() => {
    if (!clip) return null;
    if (rec) return { take: rec.take, takes: Math.max(clip.takes, rec.take), recording: true };
    return clip.takes >= 2 ? { take: clip.take, takes: clip.takes, recording: false } : null;
  });
  /** The same rule as 「次のテイク」「前のテイク」 in the menu: not while recording or counting in. */
  const takeBusy = $derived(songBusy(app));
  /** stepTake acts on the arrange's chosen clip, so the shown clip is chosen first. */
  function stepTake(delta: 1 | -1): void {
    if (!clip) return;
    if (app.arrange.selectedClip !== clip.id) send({ type: "selectClip", clip: clip.id });
    send({ type: "stepTake", delta });
  }

  const TOOLS: { tool: Tool; key: TextKey; hotkey: TextKey }[] = [
    { tool: "select", key: "pr.toolSelect", hotkey: "key.toolSelect" },
    { tool: "draw", key: "pr.toolDraw", hotkey: "key.toolDraw" },
    { tool: "erase", key: "pr.toolErase", hotkey: "key.toolErase" },
  ];

  const gridName = $derived(gridText(pr.grid.division, false));
  const scaleText = $derived(drums ? t("inspector.scaleNone") : scaleName(pr.scale.root, pr.scale.mode));

  function setGrid(division: number): void {
    send({ type: "setGrid", division, triplet: pr.grid.triplet });
    closeMenu();
  }
  // Choosing with the scale ▾ turns scale lock on (B-65).
  function pickScale(root: number, mode: ScaleMode): void {
    send({ type: "setMusicalScale", root, mode, lock: true, highlight: pr.scale.highlight } satisfies Command);
    closeMenu();
  }
  const toggleTriplet = () => send({ type: "setGrid", division: pr.grid.division, triplet: !pr.grid.triplet });
  const toggleLock = () => send({ type: "setMusicalScale", root: pr.scale.root, mode: pr.scale.mode, lock: !pr.scale.lock, highlight: pr.scale.highlight } satisfies Command);
  const zoom = (steps: number) => zoomBy(steps, metrics.scale);

  // The chord of the draw tool: its name on the draw button, and the list 「和音の形」 (not on drums).
  const drawChord = $derived(drums ? null : pickName(chordPick.pick));
  const chordRows = $derived(CHORD_SHAPES.map((shape) => ({ value: shape, label: shapeName(shape), key: shapeOnC(shape) })));
  function pickShape(shape: ChordShape): void {
    chooseChord({ kind: "shape", shape });
    closeMenu();
  }

  // 「ほかのトラックも見る」 (round 12 P2; drawn by Roll through ghosts.ts) and its ▾: the tracks to show, chosen one by one (the list stays open).
  // When the label does not fit, only the ▾ stays and the switch is the first row of its list.
  const GHOST_SWITCH = -1;
  const ghostChoices = $derived(ghostCandidates(app.song.tracks, shownTrackId(clip, track, app.song.keysTarget)));
  const ghostRows = $derived.by(() => {
    const off = ghostHidden(ghostView, app.song.serial);
    const rows = ghostChoices.map((x) => ({
      value: x.id,
      label: t("track.label", { number: x.number, name: x.name }),
      checked: !off.includes(x.id),
    }));
    return parts.ghost ? rows : [{ value: GHOST_SWITCH, label: t("pr.ghost"), checked: ghostView.on }, ...rows];
  });
  const ghostMenuUsable = $derived(!drums && (!parts.ghost || ghostChoices.length > 0));
  const toggleGhosts = () => (ghostView.on = !ghostView.on);
  function pickGhost(id: number): void {
    if (id === GHOST_SWITCH) {
      toggleGhosts();
      return;
    }
    ghostView.hidden = toggleGhostTrack(ghostHidden(ghostView, app.song.serial), id);
    ghostView.serial = app.song.serial;
  }

  // The grid list and the scale list (spec 2 "List"; opens just below or above what was pressed; Esc or pressing outside closes it).
  type Menu = "grid" | "scale" | "chord" | "ghost";
  let menu = $state<Menu | null>(null);
  const menuOpen = $derived(menu === "grid");
  let gridButton = $state<HTMLButtonElement>();
  let scaleButton = $state<HTMLButtonElement>();
  let chordButton = $state<HTMLButtonElement>();
  let ghostButton = $state<HTMLButtonElement>();
  const menuButton = $derived(menu === "scale" ? scaleButton : menu === "chord" ? chordButton : menu === "ghost" ? ghostButton : gridButton);
  let popup = $state<HTMLElement>();
  let anchor = $state<{ left: number; top: number | null; bottom: number | null; maxHeight: number | null }>({
    left: 0,
    top: 0,
    bottom: null,
    maxHeight: null,
  });
  const gridRows = $derived(G.GRID_DIVISIONS.map((d) => ({ value: d as number, label: gridText(d, false) })));

  /** The grid list opens just below (if it sticks out, the list moves inside the window). The scale list is long, so like the right panel, it fits just below or above, whichever has more room. */
  function place(): void {
    const button = menuButton;
    if (!button) return;
    const r = button.getBoundingClientRect();
    if (menu !== "scale") {
      anchor = { left: r.left, top: r.bottom, bottom: null, maxHeight: null };
      return;
    }
    const below = window.innerHeight - r.bottom;
    const above = r.top;
    // It is at the right end of the toolbar, so if it sticks out of the window on the right, it is moved inside the window.
    const left = Math.max(0, Math.min(r.left, window.innerWidth - metrics.px("part.list.width")));
    anchor =
      below >= above
        ? { left, top: r.bottom, bottom: null, maxHeight: below }
        : { left, top: null, bottom: window.innerHeight - r.top, maxHeight: above };
  }
  function toBody(node: HTMLElement): { destroy(): void } {
    document.body.append(node);
    return { destroy: () => node.remove() };
  }
  function closeMenu(): void {
    const hadFocus = popup?.contains(document.activeElement) ?? false;
    const button = menuButton;
    menu = null;
    if (hadFocus) button?.focus();
  }
  const toggleMenu = (which: Menu) => (menu = menu === which ? null : which);
  // When it switches to drums, the scale list closes (▾ becomes 「使えない」). So does the list of tracks when it has nothing to offer.
  $effect(() => {
    const open = untrack(() => menu);
    if (drums && (open === "scale" || open === "chord")) menu = null;
    if (!ghostMenuUsable && open === "ghost") menu = null;
  });
  $effect(() => {
    if (!menu) return;
    const button = menuButton;
    place();
    window.addEventListener("resize", place);
    const forget = menuOpened(closeMenu);
    const onDown = (ev: PointerEvent) => {
      const target = ev.target as Node | null;
      if (target && (popup?.contains(target) || button?.contains(target))) return;
      closeMenu();
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => {
      forget();
      window.removeEventListener("resize", place);
      document.removeEventListener("pointerdown", onDown, true);
    };
  });
</script>

{#snippet termed(text: string, word: string)}{#each termParts(text, word) as piece, j (j)}{#if piece.term}<span use:term={word}
        >{piece.text}</span
      >{:else}{piece.text}{/if}{/each}{/snippet}

<div class="toolbar" bind:this={row}>
  {#if prView.focus === "pianoRoll"}<div class="focus-mark" data-test="pr-focus-mark"></div>{/if}
  <button class="m-plain head m-focus-in" onclick={() => send({ type: "toggleBand", band: "pianoRoll" })} data-test="pr-fold">
    <span class="m-hi">▾</span>
    <span class="m-name" use:term={"ピアノロール"}>{t("band.pianoRoll")}</span>
  </button>
  {#if info}
    <span class="m-lbl info" class:cut={!infoFits} bind:this={infoNode} data-test="pr-info">{info}</span>
  {/if}
  {#if takeTag}
    <div class="take" data-test="pr-take">
      <button class="m-plain step" aria-label={t("arrange.takePrev")} disabled={takeBusy} onclick={() => stepTake(-1)}
        >◂</button
      >
      <span class="m-hi">
        {#if takeTag.recording}<span class="rec">{t("pr.recordingDot")}</span>{/if}
        {#each textParts("arrange.takeSwitch", { take: takeTag.take, takes: takeTag.takes }) as part, i (i)}
          {#if part.hole}<span class="m-num">{part.text}</span>{:else}{@render termed(part.text, "テイク")}{/if}
        {/each}
      </span>
      <button class="m-plain step" aria-label={t("arrange.takeNext")} disabled={takeBusy} onclick={() => stepTake(1)}
        >▸</button
      >
      <span class="m-num m-lbl">{t("arrange.takeKey")}</span>
    </div>
  {/if}
  <span class="m-spacer"></span>
  {#if parts.ghost}
    {#if drums}
      <button class="m-btn" aria-disabled="true" use:tip={t("pr.ghostDrums")} data-test="pr-ghost">{t("pr.ghost")}</button>
    {:else}
      <button class="m-btn" class:on={ghostView.on} aria-pressed={ghostView.on} onclick={toggleGhosts} data-test="pr-ghost">{t("pr.ghost")}</button>
    {/if}
  {/if}
  <button
    class="m-btn ghost-menu"
    class:stub={parts.ghost}
    class:on={ghostView.on && !drums}
    class:open={menu === "ghost"}
    aria-disabled={ghostMenuUsable ? undefined : "true"}
    aria-haspopup="menu"
    aria-expanded={menu === "ghost"}
    aria-label={t("pr.ghostTracks")}
    bind:this={ghostButton}
    onclick={() => ghostMenuUsable && toggleMenu("ghost")}
    data-test="pr-ghost-menu">▾</button
  >
  <div class="tools">
    {#each TOOLS as tool (tool.tool)}
      <button class="m-btn" class:on={prView.tool === tool.tool} aria-pressed={prView.tool === tool.tool} onclick={() => (prView.tool = tool.tool)} data-test="pr-tool-{tool.tool}"
        >{t(tool.key)}{#if tool.tool === "draw" && drawChord}<span class="m-num chord" data-test="pr-tool-chord">{drawChord}</span>{/if}{#if parts.keys}<span
            class="m-num m-lbl key"
            data-test="pr-tool-key">{t(tool.hotkey)}</span
          >{/if}</button
      >
      {#if tool.tool === "draw"}
        {#if drums}
          <button class="m-btn chord-stub" aria-disabled="true" aria-label={t("pr.chordShapes")} data-test="pr-chord-menu">▾</button>
        {:else}
          <button
            class="m-btn chord-stub"
            class:on={prView.tool === "draw" && drawChord !== null}
            class:open={menu === "chord"}
            aria-haspopup="menu"
            aria-expanded={menu === "chord"}
            aria-label={t("pr.chordShapes")}
            bind:this={chordButton}
            use:term={"和音のスタンプ"}
            onclick={() => toggleMenu("chord")}
            data-test="pr-chord-menu">▾</button
          >
        {/if}
      {/if}
    {/each}
  </div>
  <div class="zoom">
    <button class="m-btn square" aria-label={t("zoom.out")} onclick={() => zoom(-1)} data-test="pr-zoom-out">{t("zoom.outMark")}</button>
    <button class="m-btn square" aria-label={t("zoom.in")} onclick={() => zoom(1)} data-test="pr-zoom-in">{t("zoom.inMark")}</button>
  </div>
  <button
    class="m-btn"
    class:open={menuOpen}
    aria-haspopup="menu"
    aria-expanded={menuOpen}
    bind:this={gridButton}
    use:term={"グリッド"}
    onclick={() => toggleMenu("grid")}
    data-test="pr-grid"
  >
    {t("pr.grid")} <span class="m-num value">{gridName}</span>
  </button>
  <button class="m-btn" class:on={pr.grid.triplet} aria-pressed={pr.grid.triplet} use:term={"三連符"} onclick={toggleTriplet} data-test="pr-triplet"
    >{t("grid.triplet")}</button
  >
  {#if drums}
    <button class="m-btn" aria-disabled="true" use:tip={t("msg.drumsNoScaleLock")} data-test="pr-scale-lock">
      {t("pr.scaleLock")} <span class="m-num">{scaleText}</span>
    </button>
    <button class="m-btn scale-stub" aria-disabled="true" aria-label={t("inspector.musicalScale")} data-test="pr-scale-menu">▾</button>
  {:else}
    <button class="m-btn" class:on={pr.scale.lock} aria-pressed={pr.scale.lock} use:term={"スケール固定"} onclick={toggleLock} data-test="pr-scale-lock">
      {t("pr.scaleLock")} <span class="m-num">{scaleText}</span>
    </button>
    <button
      class="m-btn scale-stub"
      class:on={pr.scale.lock}
      class:open={menu === "scale"}
      aria-haspopup="menu"
      aria-expanded={menu === "scale"}
      aria-label={t("inspector.musicalScale")}
      bind:this={scaleButton}
      onclick={() => toggleMenu("scale")}
      data-test="pr-scale-menu">▾</button
    >
  {/if}
  <div class="ruler" aria-hidden="true" bind:this={ruler}>
    {#each TOOLS as tool (tool.tool)}<span class="m-num">{t(tool.hotkey)}</span>{/each}
  </div>
  <div class="ruler" aria-hidden="true"><span class="m-btn" bind:this={ghostRuler}>{t("pr.ghost")}</span></div>
</div>

{#if menu}
  <div
    class="popup"
    style:left="{anchor.left}px"
    style:top={anchor.top === null ? null : `${anchor.top}px`}
    style:bottom={anchor.bottom === null ? null : `${anchor.bottom}px`}
    style:max-height={anchor.maxHeight === null ? null : `${anchor.maxHeight}px`}
    use:toBody
    bind:this={popup}
  >
    {#if menu === "scale"}
      <ScaleMenu scale={pr.scale} onpick={pickScale} />
    {:else if menu === "ghost"}
      <ChoiceMenu heading={t("pr.ghostTracks")} rows={ghostRows} onpick={pickGhost} test="pr-ghost-tracks" />
    {:else if menu === "chord"}
      <ChoiceMenu
        heading={t("pr.chordShapes")}
        rows={chordRows}
        selected={chordPick.pick.kind === "shape" ? chordPick.pick.shape : undefined}
        onpick={pickShape}
        test="pr-chord-shapes"
      >
        {#snippet note()}{@render termed(t("pr.chordRootC"), "根音")}{/snippet}
      </ChoiceMenu>
    {:else}
      <ChoiceMenu heading={t("pr.grid")} rows={gridRows} selected={pr.grid.division} onpick={setGrid} test="pr-grid-menu" />
    {/if}
  </div>
{/if}

<style>
  .toolbar {
    position: relative;
    height: var(--size-toolbar);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    border-bottom: var(--line-hair) solid var(--color-gray-6);
    white-space: nowrap;
    overflow: hidden;
  }
  /* The mark of the band that keys go to (B-72): a line.mark accent on the top edge. Drawn on top without changing the height. */
  .focus-mark {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    height: var(--line-mark);
    background: var(--color-accent-ink);
    pointer-events: none;
  }
  /* 「▾ ピアノロール」: pressing folds it. States are those of the button in spec 2 (the border is drawn inside). */
  .head {
    flex-shrink: 0;
    align-self: stretch;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    /* The pressable area starts at the band's left edge. The gap to the right neighbor stays space.3 like the others. */
    margin: 0 calc(-1 * var(--space-2)) 0 calc(-1 * var(--space-3));
    padding: 0 var(--space-2) 0 var(--space-3);
  }
  .head:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .head:active {
    background: var(--color-gray-2);
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .info {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
  }
  .info.cut {
    visibility: hidden;
  }
  .take {
    flex-shrink: 0;
    height: var(--part-button-height);
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: 0 var(--space-1);
    background: var(--color-gray-4);
    border: var(--line-hair) solid var(--color-gray-8);
  }
  .step {
    width: var(--part-toggle-size);
    height: var(--part-toggle-size);
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-gray-8);
    text-align: center;
  }
  .step:disabled {
    color: var(--color-gray-7);
    cursor: default;
  }
  .rec {
    color: var(--color-rec);
    margin-right: var(--space-1);
  }
  .tools,
  .zoom {
    flex-shrink: 0;
    display: flex;
  }
  .zoom {
    gap: var(--space-2);
  }
  .square {
    width: var(--part-button-height);
    padding: 0;
  }
  .m-btn.on,
  .m-btn.on:hover {
    border-color: var(--color-accent-ink);
    background: var(--color-accent);
    color: var(--color-accent-fg);
  }
  .m-btn.open {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .value {
    color: var(--color-gray-10);
  }
  /* Keys are gray.8; on an accent fill everything takes the fill's text color. */
  .m-btn .key {
    color: var(--color-gray-8);
  }
  .m-btn.on .m-num,
  .m-btn.on .key {
    color: var(--color-accent-fg);
  }
  /* The ▾ of the draw tool: the same form as the ▾ of the scale lock, right after 「書く」 with one line between. */
  .chord-stub {
    width: calc(var(--part-metronomemenu-width) - var(--line-hair));
    padding: 0;
    border-left: 0;
  }
  /* The scale ▾ (B-65): the same shape as the metronome ▾. 20 wide, right after the scale lock button with no gap. One line between. */
  .scale-stub {
    width: calc(var(--part-metronomemenu-width) - var(--line-hair));
    padding: 0;
    border-left: 0;
    margin-left: calc(-1 * var(--space-3));
  }
  /* The ▾ of 「ほかのトラックも見る」: the same form as the scale ▾ while the label is there; a ▾ of its own when the label does not fit. */
  .ghost-menu {
    width: var(--part-metronomemenu-width);
    padding: 0;
  }
  .ghost-menu.stub {
    width: calc(var(--part-metronomemenu-width) - var(--line-hair));
    border-left: 0;
    margin-left: calc(-1 * var(--space-3));
  }
  .ruler {
    position: absolute;
    left: 0;
    top: 0;
    display: flex;
    visibility: hidden;
    pointer-events: none;
  }
  /* Stacking: above dialogs (30), below term tips (1000). */
  .popup {
    position: fixed;
    z-index: 40;
    display: flex;
    flex-direction: column;
  }
</style>

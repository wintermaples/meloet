<!-- ピアノロールの道具の列（モック A-PianoRoll・F-PRPlay・F-PRRec の帯のいちばん上の行）。
     「▾ ピアノロール」（押すとたたむ。B-74）、クリップの文、テイクの切りかえの札、道具（選ぶ ・ 書く ・ 消す。右はしにキー）、横の − ＋（R3-Arrange と同じ形）、
     グリッド（一覧）、三連符、スケール固定と音階の ▾（B-65）。ドラムのトラックでは、スケール固定と ▾ は「使えない」（仕様書 3 章、6 章）。横が足りなければ、道具のキー、クリップの文の順に引っこめる（F-PR150、B-73）。
     キーがピアノロールに向いているときは、上辺に accent の線（B-72）。 -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { liveNotes, type Command, type ScaleMode } from "../../bridge";
  import { menuOpened } from "../../lib/menu";
  import { term, tip } from "../../lib/tip.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t, type TextKey } from "../../text/ja";
  import ChoiceMenu from "../ChoiceMenu.svelte";
  import { toolbarParts, type ToolbarParts } from "../fit";
  import { gridText, scaleName, termParts, textParts } from "../format";
  import { metrics } from "../metrics.svelte";
  import { rollShowsDrums } from "./drums";
  import * as G from "./geom";
  import ScaleMenu from "./ScaleMenu.svelte";
  import { chooseChord, chordPick } from "./chordPick.svelte";
  import { CHORD_SHAPES, pickName, shapeName, shapeOnC, type ChordShape } from "./chords";
  import { shown } from "./shown.svelte";
  import { prView, zoomBy, type Tool } from "./view.svelte";

  const app = $derived(current.state!);
  const pr = $derived(app.pianoRoll);
  const meter = $derived({ num: app.timeline.meterNum, den: app.timeline.meterDen });
  const clip = $derived(shown.clip);
  const track = $derived(shown.track);
  const rec = $derived(
    app.transport.state === "recording" && app.transport.recording && clip && app.transport.recording.clip === clip.id
      ? app.transport.recording
      : null,
  );
  const selectedCount = $derived(G.indexesOf(pr.selectedNotes).length);
  const drums = $derived(rollShowsDrums(app, clip, track));

  // 録音中のノートの数は、録っている物の数（F-PRRec「録音中 ・ ノート 22」）。
  let liveCount = $state(0);
  $effect(() => {
    const r = rec;
    if (!r) return;
    let book: ReturnType<typeof liveNotes>;
    try {
      book = liveNotes();
    } catch {
      return;
    }
    const count = () => (liveCount = book.notesOf(r.id, current.telemetry?.position?.lap ?? r.lap).length);
    untrack(count);
    return book.listen(count);
  });

  const info = $derived.by((): { key: TextKey; values: Record<string, string | number> } | null => {
    if (!clip || !track) {
      // クリップがない所（仕様書 3 章）: 弾くと鳴る先のトラック。
      const target = app.song.tracks.find((x) => x.id === app.song.keysTarget);
      return !clip && target && app.transport.recording === null ? { key: "pr.noClip", values: { number: target.number, track: target.name } } : null;
    }
    const base = { number: track.number, track: track.name, clip: clip.name, count: clip.noteCount };
    if (rec) return { key: "pr.clipRecording", values: { ...base, count: liveCount } };
    const bars = G.barsText(clip.start, clip.end, meter);
    if (selectedCount > 0) return { key: "pr.clipSelected", values: { ...base, bars, selected: selectedCount } };
    return { key: "pr.clip", values: { ...base, bars } };
  });
  const MONO = new Set(["number", "bars", "count"]);

  // 横が足りなければ、道具のキー、クリップの文の順に出さない。クリップの文の幅は、出していなくても scrollWidth で測れる。
  // キーの幅は、見えない写し（ruler）で測る。
  let row = $state<HTMLElement>();
  let ruler = $state<HTMLElement>();
  let infoNode = $state<HTMLElement>();
  let parts = $state<ToolbarParts>({ keys: true, info: true });
  const infoFits = $derived(parts.info);

  function measure(): void {
    if (!row || !ruler) return;
    const style = getComputedStyle(row);
    const gap = parseFloat(style.columnGap) || 0;
    const pad = (parseFloat(style.paddingLeft) || 0) + (parseFloat(style.paddingRight) || 0);
    const children = [...row.children].filter((el) => el !== ruler && !el.classList.contains("focus-mark")) as HTMLElement[];
    const fixed = children.filter((el) => el !== infoNode);
    const widths = fixed.reduce((sum, el) => sum + (el.classList.contains("m-spacer") ? 0 : el.offsetWidth), 0);
    const keys = [...ruler.children].reduce((sum, el) => sum + (el as HTMLElement).offsetWidth + metrics.px("space.1"), 0);
    const base = pad + widths + gap * Math.max(0, fixed.length - 1) - (parts.keys ? keys : 0);
    parts = toolbarParts(row.clientWidth, { base, keys, info: infoNode ? infoNode.scrollWidth + gap : 0 });
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
  // 音階の ▾ でえらんだら、スケール固定をオンにする（B-65）。
  function pickScale(root: number, mode: ScaleMode): void {
    send({ type: "setMusicalScale", root, mode, lock: true } satisfies Command);
    closeMenu();
  }
  const toggleTriplet = () => send({ type: "setGrid", division: pr.grid.division, triplet: !pr.grid.triplet });
  const toggleLock = () => send({ type: "setMusicalScale", root: pr.scale.root, mode: pr.scale.mode, lock: !pr.scale.lock } satisfies Command);
  const zoom = (steps: number) => zoomBy(steps, metrics.scale);

  // The chord of the draw tool: its name on the draw button, and the list 「和音の形」 (not on drums).
  const drawChord = $derived(drums ? null : pickName(chordPick.pick));
  const chordRows = $derived(CHORD_SHAPES.map((shape) => ({ value: shape, label: shapeName(shape), key: shapeOnC(shape) })));
  function pickShape(shape: ChordShape): void {
    chooseChord({ kind: "shape", shape });
    closeMenu();
  }

  // グリッドの一覧と音階の一覧（仕様書 2 章「一覧」。押した物のすぐ下か上にひらく。Esc か外を押すと、とじる）。
  type Menu = "grid" | "scale" | "chord";
  let menu = $state<Menu | null>(null);
  const menuOpen = $derived(menu === "grid");
  let gridButton = $state<HTMLButtonElement>();
  let scaleButton = $state<HTMLButtonElement>();
  let chordButton = $state<HTMLButtonElement>();
  const menuButton = $derived(menu === "scale" ? scaleButton : menu === "chord" ? chordButton : gridButton);
  let popup = $state<HTMLElement>();
  let anchor = $state<{ left: number; top: number | null; bottom: number | null; maxHeight: number | null }>({
    left: 0,
    top: 0,
    bottom: null,
    maxHeight: null,
  });
  const gridRows = $derived(G.GRID_DIVISIONS.map((d) => ({ value: d as number, label: gridText(d, false) })));

  /** グリッドの一覧はすぐ下（はみ出せば一覧が窓の中へずれる）。音階の一覧は長いので、右の欄と同じく、すぐ下か上の広いほうに入る高さで。 */
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
    // 道具の列の右はしにあるので、窓の右からはみ出すときは窓の中へ寄せる。
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
  // ドラムに切りかわったら、音階の一覧はとじる（▾ が「使えない」になる）。
  $effect(() => {
    const open = untrack(() => menu);
    if (drums && (open === "scale" || open === "chord")) menu = null;
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
    <span class="m-lbl info" class:cut={!infoFits} bind:this={infoNode} data-test="pr-info">
      {#each textParts(info.key, info.values) as part, i (i)}
        {#if part.hole === "selected"}<span class="m-num selected">{part.text}</span
          >{:else if part.hole && MONO.has(part.hole)}<span class="m-num">{part.text}</span>{:else if part.hole}{part.text}{:else}{@render termed(
            part.text,
            "ノート",
          )}{/if}
      {/each}
    </span>
  {/if}
  {#if takeTag}
    <div class="take" data-test="pr-take">
      <button class="m-plain step" aria-label={t("arrange.takePrev")} disabled={takeTag.recording} onclick={() => send({ type: "stepTake", delta: -1 })}
        >◂</button
      >
      <span class="m-hi">
        {#if takeTag.recording}<span class="rec">{t("pr.recordingDot")}</span>{/if}
        {#each textParts("arrange.takeSwitch", { take: takeTag.take, takes: takeTag.takes }) as part, i (i)}
          {#if part.hole}<span class="m-num">{part.text}</span>{:else}{@render termed(part.text, "テイク")}{/if}
        {/each}
      </span>
      <button class="m-plain step" aria-label={t("arrange.takeNext")} disabled={takeTag.recording} onclick={() => send({ type: "stepTake", delta: 1 })}
        >▸</button
      >
      <span class="m-num m-lbl">{t("arrange.takeKey")}</span>
    </div>
  {/if}
  <span class="m-spacer"></span>
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
            class:on={drawChord !== null}
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
  /* キーが向く帯の印（B-72）: 上辺に accent の line.mark。高さは変えず、重ねて描く。 */
  .focus-mark {
    position: absolute;
    left: 0;
    right: 0;
    top: 0;
    height: var(--line-mark);
    background: var(--color-accent);
    pointer-events: none;
  }
  /* 「▾ ピアノロール」: 押すとたたむ。状態は仕様書 2 章のボタン（枠は内がわに描く）。 */
  .head {
    flex-shrink: 0;
    align-self: stretch;
    display: flex;
    align-items: center;
    gap: var(--space-3);
    /* 押せる所は帯の左はしから。右どなりとの間は、ほかの物と同じ space.3 のまま。 */
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
  .selected {
    color: var(--color-accent);
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
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  .m-btn.open {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .value {
    color: var(--color-gray-10);
  }
  .m-btn.on .m-num {
    color: var(--color-accent);
  }
  /* キーは、オンでも gray.8 のまま（再生まわりと同じ）。 */
  .m-btn .key {
    color: var(--color-gray-8);
  }
  /* The ▾ of the draw tool: the same form as the ▾ of the scale lock, right after 「書く」 with one line between. */
  .chord-stub {
    width: calc(var(--part-metronomemenu-width) - var(--line-hair));
    padding: 0;
    border-left: 0;
  }
  /* 音階の ▾（B-65）: メトロノームの ▾ と同じ形。幅 20 で、スケール固定のボタンの右にすきまなく。間の線は 1 本。 */
  .scale-stub {
    width: calc(var(--part-metronomemenu-width) - var(--line-hair));
    padding: 0;
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
  /* 重なり: ダイアログ（30）より上、用語の説明（1000）より下。 */
  .popup {
    position: fixed;
    z-index: 40;
    display: flex;
    flex-direction: column;
  }
</style>

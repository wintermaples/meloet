<!-- ピアノロールの道具の列（モック A-PianoRoll・F-PRPlay・F-PRRec の帯のいちばん上の行）。
     「▾ ピアノロール」、クリップの文、テイクの切りかえの札、道具（選ぶ ・ 書く ・ 消す）、横の − ＋（R3-Arrange と同じ形）、
     グリッド（一覧）、三連符、スケール固定。横が足りなければクリップの文を引っこめる（F-PR150）。 -->
<script lang="ts">
  import { untrack } from "svelte";
  import { liveNotes, type Command } from "../../bridge";
  import { menuOpened } from "../../lib/menu";
  import { term } from "../../lib/tip.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t, type TextKey } from "../../text/ja";
  import ChoiceMenu from "../ChoiceMenu.svelte";
  import { textParts } from "../format";
  import { metrics } from "../metrics.svelte";
  import * as G from "./geom";
  import { shown } from "./shown.svelte";
  import { prView, zoomBy, type Tool } from "./view.svelte";

  const app = $derived(current.state!);
  const pr = $derived(app.pianoRoll);
  const meter = $derived({ num: app.song.meterNum, den: app.song.meterDen });
  const clip = $derived(shown.clip);
  const track = $derived(shown.track);
  const rec = $derived(
    app.transport.state === "recording" && app.transport.recording && clip && app.transport.recording.clip === clip.id
      ? app.transport.recording
      : null,
  );
  const selectedCount = $derived(G.indexesOf(pr.selectedNotes).length);

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
    if (!clip || !track) return null;
    const base = { number: track.number, track: track.name, clip: clip.name, count: clip.noteCount };
    if (rec) return { key: "pr.clipRecording", values: { ...base, count: liveCount } };
    const bars = G.barsText(clip.start, clip.end, meter);
    if (selectedCount > 0) return { key: "pr.clipSelected", values: { ...base, bars, selected: selectedCount } };
    return { key: "pr.clip", values: { ...base, bars } };
  });
  const MONO = new Set(["number", "bars", "count"]);

  // 横が足りなければ、クリップの文を出さない（中身の幅は、出していなくても scrollWidth で測れる）。
  let infoNode = $state<HTMLElement>();
  let infoFits = $state(true);
  $effect(() => {
    const node = infoNode;
    if (!node) return;
    const check = () => (infoFits = node.scrollWidth <= node.clientWidth + 1);
    const observer = new ResizeObserver(check);
    observer.observe(node);
    check();
    return () => observer.disconnect();
  });
  $effect(() => {
    void info;
    if (infoNode) infoFits = infoNode.scrollWidth <= infoNode.clientWidth + 1;
  });

  const takeTag = $derived.by(() => {
    if (!clip) return null;
    if (rec) return { take: rec.take, takes: Math.max(clip.takes, rec.take), recording: true };
    return clip.takes >= 2 ? { take: clip.take, takes: clip.takes, recording: false } : null;
  });

  const TOOLS: { tool: Tool; key: TextKey }[] = [
    { tool: "select", key: "pr.toolSelect" },
    { tool: "draw", key: "pr.toolDraw" },
    { tool: "erase", key: "pr.toolErase" },
  ];

  const gridText = $derived(G.gridFraction(pr.grid.division) ?? t("pr.gridBar"));
  const scaleText = $derived(t(pr.scale.mode === "major" ? "pr.scaleMajor" : "pr.scaleMinor", { root: G.NOTE_NAMES[pr.scale.root] ?? "C" }));

  function setGrid(division: number): void {
    send({ type: "setGrid", division, triplet: pr.grid.triplet });
    closeMenu();
  }
  const toggleTriplet = () => send({ type: "setGrid", division: pr.grid.division, triplet: !pr.grid.triplet });
  const toggleLock = () => send({ type: "setMusicalScale", root: pr.scale.root, mode: pr.scale.mode, lock: !pr.scale.lock } satisfies Command);
  const zoom = (steps: number) => zoomBy(steps, metrics.scale);

  // グリッドの一覧（仕様書 2 章「一覧」。押した物のすぐ下にひらく。Esc か外を押すと、とじる）。
  let menuOpen = $state(false);
  let gridButton = $state<HTMLButtonElement>();
  let popup = $state<HTMLElement>();
  let anchor = $state({ left: 0, top: 0 });
  const gridRows = $derived(G.GRID_DIVISIONS.map((d) => ({ value: d as number, label: G.gridFraction(d) ?? t("pr.gridBar") })));

  function place(): void {
    if (!gridButton) return;
    const r = gridButton.getBoundingClientRect();
    anchor = { left: r.left, top: r.bottom };
  }
  function toBody(node: HTMLElement): { destroy(): void } {
    document.body.append(node);
    return { destroy: () => node.remove() };
  }
  function closeMenu(): void {
    const hadFocus = popup?.contains(document.activeElement) ?? false;
    menuOpen = false;
    if (hadFocus) gridButton?.focus();
  }
  $effect(() => {
    if (!menuOpen) return;
    place();
    window.addEventListener("resize", place);
    const forget = menuOpened(closeMenu);
    const onDown = (ev: PointerEvent) => {
      const target = ev.target as Node | null;
      if (target && (popup?.contains(target) || gridButton?.contains(target))) return;
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

<div class="toolbar">
  <span class="m-hi">▾</span>
  <span class="m-name">{t("band.pianoRoll")}</span>
  {#if info}
    <span class="m-lbl info" class:cut={!infoFits} bind:this={infoNode} data-test="pr-info">
      {#each textParts(info.key, info.values) as part, i (i)}
        {#if part.hole === "selected"}<span class="m-num selected">{part.text}</span
          >{:else if part.hole && MONO.has(part.hole)}<span class="m-num">{part.text}</span>{:else}{part.text}{/if}
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
          {#if part.hole}<span class="m-num">{part.text}</span>{:else}{part.text}{/if}
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
        >{t(tool.key)}</button
      >
    {/each}
  </div>
  <div class="zoom">
    <button class="m-btn square" aria-label={t("pr.zoomOut")} onclick={() => zoom(-1)} data-test="pr-zoom-out">{t("pr.zoomOutMark")}</button>
    <button class="m-btn square" aria-label={t("pr.zoomIn")} onclick={() => zoom(1)} data-test="pr-zoom-in">{t("pr.zoomInMark")}</button>
  </div>
  <button
    class="m-btn"
    class:open={menuOpen}
    aria-haspopup="menu"
    aria-expanded={menuOpen}
    bind:this={gridButton}
    use:term={"グリッド"}
    onclick={() => (menuOpen = !menuOpen)}
    data-test="pr-grid"
  >
    {t("pr.grid")} <span class="m-num value">{gridText}</span>
  </button>
  <button class="m-btn" class:on={pr.grid.triplet} aria-pressed={pr.grid.triplet} use:term={"三連符"} onclick={toggleTriplet} data-test="pr-triplet"
    >{t("pr.triplet")}</button
  >
  <button class="m-btn" class:on={pr.scale.lock} aria-pressed={pr.scale.lock} use:term={"スケール固定"} onclick={toggleLock} data-test="pr-scale-lock">
    {t("pr.scaleLock")} <span class="m-num">{scaleText}</span>
  </button>
</div>

{#if menuOpen}
  <div class="popup" style:left="{anchor.left}px" style:top="{anchor.top}px" use:toBody bind:this={popup}>
    <ChoiceMenu heading={t("pr.grid")} rows={gridRows} selected={pr.grid.division} onpick={setGrid} test="pr-grid-menu" />
  </div>
{/if}

<style>
  .toolbar {
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
    width: var(--mock-pianoroll-zoom-button);
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
  /* 重なり: ダイアログ（30）より上、用語の説明（1000）より下。 */
  .popup {
    position: fixed;
    z-index: 40;
  }
</style>

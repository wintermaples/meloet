<!-- The column of track heads (mock R7-Arrange6, R6-Tracks, F7-VocalTrack) and, at the bottom, 「＋ 鍵盤のトラック」 「＋ 歌のトラック」. Placed inside the arrange head box (the trackMeters slot).
     A press selects the track (B-70). Pressing the name twice, F2, or 「名前を変える」 in the context menu turns the name into an input box in place (spec 2).
     Dragging a head up or down reorders (spec 2 "Track reorder mark"): only the screen moves first, and one MoveTrack is sent on release.
     If it moved less than space.1, it was a plain press (select). Esc cancels first in the key order (cancelDrag in pianoroll/view.svelte.ts).
     Pressing the instrument name chooses that instrument (params panel; 19-A). Replacing is 「音源を入れかえる…」 in the context menu and the button in the panel.
     While auditioning a replacement, the auditioned instrument name is accent (mock F4-ShelfReplace).
     Vocal tracks (spec 2 "Vocal track head"): the second line is 「入口 1 ・ モノ ▾」 and 「モニター」. The ● is set on its own, even when not selected.
     While the ● is on, the level bars show the incoming sound (mics in the meters).
     「帯」 to the left of M on the first line (spec 2 "「帯」 button in the head", spec 3 (94-9, 36-B), mock F8-FirstLane): a press opens the list to add a band.
     While open, it is accent. The name area gets that much (24) narrower, and a name that does not fit ends in 「…」. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import type { TrackView } from "../bridge";
  import { glSlot } from "../gl/slot";
  import { term, tip } from "../lib/tip.svelte";
  import { current, send } from "../state/app.svelte";
  import { findTerm, t } from "../text/ja";
  import { dbNumber } from "./format";
  import { openContextMenu } from "./menu/contextMenu.svelte";
  import { metrics } from "./metrics.svelte";
  import { onDragCancel } from "./pianoroll/view.svelte";
  import { trackContextMenu, vocalInputRows } from "./trackMenu";
  import { soundRefName, soundViewName, standInTip } from "./sounds";
  import { moveSlotAt, openShelf, trackEdit } from "./tracks.svelte";
  import { arrangeRows, showsAddRow } from "./automation/autoGeom";
  import LaneAddRow from "./automation/LaneAddRow.svelte";
  import LaneHead from "./automation/LaneHead.svelte";
  import { bandHeight } from "./comp/comp";
  import CompHead from "./comp/CompHead.svelte";
  import { openCompOf } from "./comp/compState.svelte";
  import { laneAddOpenFor, openLaneAdd } from "./automation/laneAdd.svelte";
  import { windowOf } from "./params/selected";
  import WindowMark from "./WindowMark.svelte";
  import ValueField from "./params/ValueField.svelte";
  import { gainCommand } from "./mixer/control.svelte";
  import { faderText, parseDb } from "./mixer/scale";

  const song = $derived(current.state!.song);
  const shelf = $derived(current.state!.shelf);
  const transport = $derived(current.state!.transport);
  const busy = $derived(transport.state === "recording" || transport.state === "countIn");

  /** The track whose dB is being typed (a double click on the dB opens the same field as the mixer fader's). */
  let typingGain = $state<number | null>(null);
  function submitGain(track: TrackView, text: string): void {
    const db = parseDb(text);
    if (db === undefined) return;
    typingGain = null;
    send(gainCommand({ kind: "track", id: track.id }, db, true));
  }

  const shiftKey = (letter: string) => `${t("key.shift")} ${letter}`;

  /** The id of the track being auditioned for a replacement, and the auditioned instrument. */
  const auditioned = $derived(
    shelf.mode.kind === "replace" && shelf.selectedSound !== null
      ? { track: shelf.mode.track, name: soundRefName(shelf.selectedSound, current.state!.library) }
      : null,
  );

  // ---- Name input box ----

  let field = $state<HTMLInputElement>();
  let draft = $state("");
  $effect(() => {
    const id = trackEdit.renaming;
    if (id === null) return;
    const track = song.tracks.find((x) => x.id === id);
    if (!track) {
      trackEdit.renaming = null;
      return;
    }
    draft = track.name;
    void tick().then(() => {
      field?.focus();
      field?.select();
    });
  });
  // If recording starts while a name is being typed, cancel (core does not take it).
  $effect(() => {
    if (busy) trackEdit.renaming = null;
  });

  function commitName(track: TrackView): void {
    trackEdit.renaming = null;
    const name = draft.trim();
    // If emptied, keep the old name (spec 3). The same name is not an undo step.
    if (name !== "" && name !== track.name) send({ type: "renameTrack", track: track.id, name });
  }
  function fieldKey(e: KeyboardEvent, track: TrackView): void {
    if (e.isComposing) return;
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      commitName(track);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      trackEdit.renaming = null;
    }
  }

  // ---- Press and reorder ----

  let press: { id: number; from: number; y0: number; pointer: number; el: HTMLElement } | null = null;
  let drag = $state<{ id: number; from: number; dy: number } | null>(null);
  const trackH = $derived(metrics.px("size.track"));
  /** Track rows and the automation bands under them (the same order as the time side; automation/autoGeom.ts). */
  /** The band 「いいとこ取り」 that is open (round 12, A8), as on the time side. */
  const comp = $derived(openCompOf(current.state!));
  const compRows = $derived(comp ? { track: comp.track.id, height: bandHeight({ headH: metrics.px("size.row"), rowH: metrics.px("size.lane") }, comp.takes.length) } : null);
  const rows = $derived(
    arrangeRows(
      song.tracks,
      current.state!.automation.folded,
      {
        trackH,
        laneH: metrics.px("size.lane"),
        rowH: metrics.px("size.row"),
        tall: { track: current.state!.arrange.tallTrack, h: metrics.px("part.track.vocal") },
      },
      compRows,
    ),
  );
  const bounds = $derived([...rows.tops, rows.height]);
  const dropAt = (d: { from: number; dy: number }) => moveSlotAt(d.from, bounds, rows.tops[d.from]! + rows.heights[d.from]! / 2 + d.dy);
  const slot = $derived(drag ? dropAt(drag) : null);

  function headDown(e: PointerEvent, track: TrackView, at: number): void {
    if (e.button !== 0 || (e.target as Element | null)?.closest("button, input")) return;
    const el = e.currentTarget as HTMLElement;
    press = { id: track.id, from: at, y0: e.clientY, pointer: e.pointerId, el };
    el.setPointerCapture?.(e.pointerId);
  }
  function headMove(e: PointerEvent): void {
    if (!press || e.pointerId !== press.pointer) return;
    const dy = e.clientY - press.y0;
    if (!drag) {
      if (busy || Math.abs(dy) < metrics.px("space.1")) return;
      drag = { id: press.id, from: press.from, dy };
    } else drag = { ...drag, dy };
  }
  function headUp(e: PointerEvent): void {
    if (!press || e.pointerId !== press.pointer) return;
    const p = press;
    const d = drag;
    endPress();
    if (d) {
      const to = dropAt(d).to;
      if (to !== d.from) send({ type: "moveTrack", track: d.id, to });
    } else if (song.playTarget !== p.id) send({ type: "selectTrack", track: p.id, fromArm: false });
  }
  function endPress(): void {
    if (press?.el.hasPointerCapture?.(press.pointer)) press.el.releasePointerCapture(press.pointer);
    press = null;
    drag = null;
  }
  onMount(() =>
    onDragCancel(() => {
      if (!drag) return false;
      endPress();
      return true;
    }),
  );

  function headContext(e: MouseEvent, track: TrackView): void {
    e.preventDefault();
    if (drag) return;
    const select = song.playTarget !== track.id && !busy;
    if (select) send({ type: "selectTrack", track: track.id, fromArm: false });
    const armed = track.kind.kind === "keys" && song.keysTarget === track.id;
    openContextMenu(e.clientX, e.clientY, trackContextMenu(current.state!, track, armed || select, send, shiftKey));
  }

  function replaceInstrument(track: TrackView): void {
    if (!busy) send(openShelf({ kind: "replace", track: track.id }));
  }
  function instrumentContext(e: MouseEvent, track: TrackView): void {
    e.preventDefault();
    e.stopPropagation();
    openContextMenu(e.clientX, e.clientY, [{ kind: "row", label: "track.replaceSound", usable: !busy, run: () => replaceInstrument(track) }]);
  }
  function addTrack(): void {
    if (!busy) send(openShelf({ kind: "newTrack" }));
  }
  function addVocalTrack(): void {
    if (!busy) send({ type: "addVocalTrack" });
  }

  /** The ● of a keyboard track is where playing sounds (SongView.keysTarget); the ● of a vocal track is the one set on it. */
  const armOn = (track: TrackView) => (track.kind.kind === "vocal" ? track.kind.armed : song.keysTarget === track.id);
  function arm(track: TrackView): void {
    if (track.kind.kind === "vocal") send({ type: "toggleVocalArm", track: track.id });
    else send({ type: "selectTrack", track: track.id, fromArm: true });
  }

  /** The input list (mock F7-VocalTrack): 「入口 1 ・ モノ」 / 「入口 2 ・ モノ」 / 「入口 1 ・ 2 ・ ステレオ」 / 「音の設定…」. Opens right under the button. */
  function openInputs(e: MouseEvent, track: TrackView): void {
    e.stopPropagation();
    const kind = track.kind;
    if (kind.kind !== "vocal") return;
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    openContextMenu(r.left, r.bottom, vocalInputRows(track.id, kind.input, busy, send));
  }
</script>

<div class="tracks">
  {#each song.tracks as track, at (track.id)}
    {@const lifted = drag?.id === track.id}
    <div
      class="track-head"
      class:selected={song.playTarget === track.id}
      class:lifted
      style:transform={lifted ? `translateY(${drag!.dy}px)` : null}
      style:height={rows.heights[at] !== trackH ? `${rows.heights[at]}px` : null}
      role="presentation"
      data-test="track-head"
      onpointerdown={(e) => headDown(e, track, at)}
      onpointermove={headMove}
      onpointerup={headUp}
      onpointercancel={endPress}
      oncontextmenu={(e) => headContext(e, track)}
    >
      <div class="track-color" style:background="var(--color-track-{((track.color - 1) % 5) + 1})"></div>
      <div class="track-body">
        <div class="line">
          <span class="m-num m-lbl track-number">{track.number}</span>
          {#if trackEdit.renaming === track.id}
            <input
              class="name-field"
              type="text"
              aria-label={t("menu.renameTrack")}
              data-test="track-name-field"
              bind:this={field}
              bind:value={draft}
              onkeydown={(e) => fieldKey(e, track)}
              onblur={() => {
                // A press outside the box commits (the owner's decision). After Esc, renaming is null.
                if (trackEdit.renaming === track.id) commitName(track);
              }}
            />
          {:else}
            <span
              class="m-name track-name"
              role="presentation"
              use:term={"トラック"}
              ondblclick={() => !busy && (trackEdit.renaming = track.id)}>{track.name}</span
            >
          {/if}
          <button
            class="tog"
            class:live={!busy}
            class:on={laneAddOpenFor() === track.id}
            aria-haspopup="menu"
            aria-disabled={busy || undefined}
            aria-label={t("lane.add")}
            data-test="track-lanes"
            onclick={(e) => !busy && openLaneAdd(e.currentTarget, track, busy, true)}>{t("arrange.laneShort")}</button
          >
          <!-- Usable while recording and during count-in too (A-14). -->
          <button
            class="tog live"
            class:on={track.muted}
            aria-pressed={track.muted}
            aria-label={t("arrange.mute")}
            data-test="track-mute"
            use:term={"ミュート"}
            onclick={() => send({ type: "toggleMute", track: track.id })}>{t("arrange.muteShort")}</button
          >
          <button
            class="tog live"
            class:on={track.soloed}
            aria-pressed={track.soloed}
            aria-label={t("arrange.solo")}
            use:term={"ソロ"}
            onclick={() => send({ type: "toggleSolo", track: track.id })}>{t("arrange.soloShort")}</button
          >
          <button
            class="tog live arm"
            class:on={armOn(track)}
            aria-pressed={armOn(track)}
            aria-label={t("arrange.arm")}
            data-test="track-arm"
            use:term={"録音待機"}
            onclick={() => arm(track)}>{t("arrange.armShort")}</button
          >
        </div>
        <div class="line gap2">
          {#if track.kind.kind === "vocal"}
            {@const vocal = track.kind}
            <button
              class="m-plain m-lbl instrument"
              class:no-input={!vocal.inputReady}
              aria-haspopup="menu"
              data-test="track-input"
              onclick={(e) => openInputs(e, track)}
              >{vocal.inputReady ? t("vocal.inputChoose", { input: t(`vocal.input.${vocal.input}`) }) : t("vocal.noInput")}</button
            >
            <button
              class="monitor"
              class:on={vocal.monitor}
              aria-pressed={vocal.monitor}
              data-test="track-monitor"
              use:term={"モニター"}
              onclick={() => send({ type: "toggleMonitor", track: track.id })}>{t("vocal.monitor")}</button
            >
          {:else if auditioned?.track === track.id}
            <button class="m-plain instrument auditioned" data-test="track-instrument" onclick={() => replaceInstrument(track)}
              >{t("arrange.instrumentAuditioned", { instrument: auditioned.name })}</button
            >
          {:else if track.standIn}
            <!-- Playing with a sine wave instead (spec 3 (85): gray.8; hover shows why). -->
            <button class="m-plain m-lbl instrument" data-test="track-instrument" use:tip={standInTip(track.standIn, track.sound)} onclick={() => replaceInstrument(track)}
              oncontextmenu={(e) => instrumentContext(e, track)}
              >{t("track.standIn")}</button
            >
          {:else if track.sound.kind === "plugin"}
            <!-- A plugin instrument: a press chooses it and opens its window (SelectInstrument). Replacing is 「音源を入れかえる…」 in the context menu (the owner's decision). -->
            <button
              class="m-plain m-lbl instrument plugin"
              data-test="track-instrument"
              onclick={() => send({ type: "selectInstrument", track: track.id })}
              oncontextmenu={(e) => instrumentContext(e, track)}
              ><span class="cut">{track.sound.name}</span>{#if windowOf(current.state!.windows, { kind: "instrument", track: track.id })}<WindowMark
                />{/if}</button
            >
          {:else}
            {@const name = soundViewName(track.sound)}
            <!-- Only presets with a term tip (the sine wave) show it on hover. -->
            {#if findTerm(name)}
              <button
                class="m-plain m-lbl instrument"
                data-test="track-instrument"
                use:term={name}
                onclick={() => send({ type: "selectInstrument", track: track.id })}
                oncontextmenu={(e) => instrumentContext(e, track)}>{name}</button
              >
            {:else}
              <button
                class="m-plain m-lbl instrument"
                data-test="track-instrument"
                onclick={() => send({ type: "selectInstrument", track: track.id })}
                oncontextmenu={(e) => instrumentContext(e, track)}>{name}</button
              >
            {/if}
          {/if}
          {#if track.kind.kind === "keys"}
            {#if typingGain === track.id}
              <ValueField
                initial={faderText(track.gainDb)}
                label={t("mixer.db")}
                unit={t("mixer.db")}
                onsubmit={(text) => submitGain(track, text)}
                oncancel={() => (typingGain = null)}
              />
            {:else}
              <span class="m-num" role="presentation" data-test="track-gain" use:term={"dB"} ondblclick={() => (typingGain = track.id)}
                >{track.gainDb === null ? t("common.dbSilent") : t("common.db", { value: dbNumber(track.gainDb) })}</span
              >
            {/if}
          {/if}
        </div>
        <div
          class="meter"
          use:glSlot={{
            kind: "meter",
            source: track.kind.kind === "vocal" && track.kind.armed ? { mic: track.id } : { track: track.number },
            gap: metrics.px("line.hair"),
          }}
        ></div>
      </div>
    </div>
    {#if comp && comp.track.id === track.id}<CompHead band={comp} />{/if}
    {#each rows.lanes.filter((l) => l.track.id === track.id) as l (JSON.stringify(l.lane.target))}
      <LaneHead {track} lane={l.lane} />
    {/each}
    {#if showsAddRow(track)}<LaneAddRow {track} />{/if}
  {/each}
  {#if slot && drag && slot.to !== drag.from}
    <div class="insert" style:top="{bounds[slot.slot]}px" data-test="track-insert"></div>
  {/if}
</div>
<div class="adds">
  <button class="m-plain add keys" class:usable={!busy} aria-disabled={busy || undefined} data-test="add-track" onclick={addTrack}>
    {t("arrange.addKeysTrack")}
  </button>
  <button class="m-plain add" class:usable={!busy} aria-disabled={busy || undefined} data-test="add-vocal-track" onclick={addVocalTrack}>
    {t("arrange.addVocalTrack")}
  </button>
</div>

<style>
  .tracks {
    position: relative;
  }
  .track-head {
    position: relative;
    height: var(--size-track);
    display: flex;
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    background: var(--color-gray-3);
  }
  /* The selected track (B-70): the same background as the selected row of a list. */
  .track-head.selected {
    background: var(--color-gray-4);
  }
  /* The lifted head (spec 2 "Track reorder mark"): an accent line.hair border. Drawn above the other heads. */
  .track-head.lifted {
    z-index: 2;
    outline: var(--line-hair) solid var(--color-accent);
    outline-offset: calc(-1 * var(--line-hair));
  }
  /* Where it goes: a horizontal accent line.mark, over the border between heads. */
  .insert {
    position: absolute;
    left: 0;
    right: 0;
    height: var(--line-mark);
    margin-top: calc(-1 * var(--line-mark) / 2);
    background: var(--color-accent);
    pointer-events: none;
    z-index: 3;
  }
  .track-color {
    width: var(--line-track);
    flex-shrink: 0;
  }
  .track-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-2) 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  .line.gap2 {
    gap: var(--space-2);
  }
  .track-number {
    width: var(--mock-track-number);
    flex-shrink: 0;
  }
  .track-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* The track name input box (spec 2: background gray.0, border gray.10, weight 600). Height 20 from mock R6-Tracks (the same as M, S, ●). */
  .name-field {
    flex: 1;
    min-width: 0;
    height: var(--part-toggle-size);
    padding: 0 var(--space-1);
    background: var(--color-gray-0);
    border: var(--line-hair) solid var(--color-gray-10);
    border-radius: var(--radius-1);
    color: var(--color-gray-10);
    font: inherit;
    font-weight: var(--text-s-weight-600);
    outline: none;
  }
  .instrument {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .instrument:hover {
    color: var(--color-gray-10);
  }
  .instrument.plugin {
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }
  .cut {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .instrument.auditioned,
  .instrument.auditioned:hover {
    color: var(--color-accent);
  }
  .tog {
    width: var(--part-toggle-size);
    height: var(--part-toggle-size);
    flex-shrink: 0;
    padding: 0;
    border: var(--line-hair) solid var(--color-gray-5);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-gray-7);
    font: inherit;
    text-align: center;
    cursor: default;
  }
  /* The off border is gray.5, as for disabled ones, so that ● on (rec) has 3:1 against it (spec 1 “Contrast”). */
  .tog.live {
    color: var(--color-gray-8);
    cursor: pointer;
  }
  .tog.live:hover {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  /* On (spec 2: border and text accent; only ● uses rec). */
  .tog.live.on {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  .tog.arm.on {
    border-color: var(--color-rec);
    color: var(--color-rec);
  }
  /* Pressed (spec 2: bg gray.2, border gray.8). */
  .tog.live:active {
    background: var(--color-gray-2);
    border-color: var(--color-gray-8);
  }
  .meter {
    height: calc(var(--part-meter-horizontal) * 2 + var(--line-hair));
  }
  /* 「入口なし ▾」 of a vocal track (spec 3: gray.8). */
  .instrument.no-input {
    color: var(--color-gray-8);
  }
  /* 「モニター」 (mock F7-VocalTrack: height 20; on uses the chosen form = border and text accent). */
  .monitor {
    box-sizing: border-box;
    flex-shrink: 0;
    height: var(--part-toggle-size);
    padding: 0 var(--space-1);
    border: var(--line-hair) solid var(--color-gray-6);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-gray-8);
    font: inherit;
    cursor: pointer;
  }
  .monitor:hover {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  .monitor:active {
    background: var(--color-gray-2);
    border-color: var(--color-gray-8);
  }
  .monitor.on {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  /* 「＋ 鍵盤のトラック」 「＋ 歌のトラック」 (mock F7-VocalTrack: text gray.9, the left box 120 wide). Disabled while recording. */
  .adds {
    display: flex;
  }
  .add.keys {
    width: var(--mock-arrange-add-keys);
  }
  .add {
    flex-shrink: 0;
    white-space: nowrap;
    height: var(--mock-arrange-add-track);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-2);
    color: var(--color-gray-7);
    cursor: default;
  }
  .add.usable {
    color: var(--color-gray-9);
    cursor: pointer;
  }
  .add.usable:hover {
    color: var(--color-gray-10);
  }
</style>

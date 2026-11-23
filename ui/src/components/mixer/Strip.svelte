<!-- One mixer strip (mock F4-Mixer). From the top: the heading (number or A / B, and the name), the instrument / return / output slot, 4 inserts, sends, pan,
     the scale marks, fader and meter, the fader value, M / S / ●. The meter is a slot in the mixerMeters region (the MixerStrips box).
     Insert reorder (spec 3 (86-2)): hold a filled slot and move it up or down. An accent line where it goes in; release sends one MoveInsert; Esc cancels. Only inside the same strip. -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { InsertView, StripRef } from "../../bridge";
  import { INSERT_SLOTS } from "../../bridge/types";
  import { glSlot, type MeterSource } from "../../gl/slot";
  import { term, tip } from "../../lib/tip.svelte";
  import { t } from "../../text";
  import { metrics } from "../metrics.svelte";
  import ValueField from "../params/ValueField.svelte";
  import AutoMark from "../params/AutoMark.svelte";
  import { MASTER_HOME } from "../automation/laneValue";
  import { onDragCancel } from "../pianoroll/view.svelte";
  import { moveSlot } from "../tracks.svelte";
  import WindowMark from "../WindowMark.svelte";
  import { send } from "../../state/app.svelte";
  import Fader from "./Fader.svelte";
  import InsertSlot from "./InsertSlot.svelte";
  import PanKnob from "./PanKnob.svelte";
  import { faderControl } from "./control.svelte";
  import { FADER_MIN, faderText } from "./scale";
  import SendBar from "./SendBar.svelte";
  import { askIf, askLive, coverIf, whenSeen } from "../automation/coverIf";
  import type { StripAuto } from "../automation/laneValue";

  interface Toggle {
    on: boolean;
    usable: boolean;
    run: () => void;
  }

  interface Props {
    strip: StripRef;
    /** The heading's left part (track number, A / B for returns, empty for the master) and the name. */
    mark: string;
    name: string;
    /** The color of the heading's top edge (a CSS variable). */
    color: string;
    /** dim: 「サイン波で代わり」 (gray.8, tip on hover). stopped: the instrument of a stopped plugin (red border and 「止」). */
    /** window: the plugin window is open (window mark at the right end). context: right-click. */
    frame: {
      text: string;
      accent?: boolean;
      run?: (e: MouseEvent) => void;
      usable?: boolean;
      dim?: boolean;
      tip?: string | null;
      stopped?: boolean;
      window?: boolean;
      context?: (e: MouseEvent) => void;
    };
    slots: readonly (InsertView | null)[];
    selectedSlot: number | null;
    /** The insert whose window is open (slot index). */
    windowSlots?: readonly number[];
    /** Tracks only. homes is the amount Home goes back to (the amount when the track was added). */
    sends: { track: number; dbs: readonly [number | null, number | null]; homes: readonly [number | null, number | null] } | null;
    pan: number;
    gainDb: number | null;
    meter: MeterSource;
    mute: Toggle;
    solo: Toggle;
    arm: Toggle;
    auto?: StripAuto | null;
    /** Tracks only (E2): the keyboards' arpeggio. Shown in the heading only while on; a press turns it off. */
    arp?: Toggle | null;
    /** The newest value while a lane moves the volume (only for the fader value text; changes every frame). undefined when not moving. */
    live?: { gainDb?: number | null };
  }
  let { strip, mark, name, color, frame, slots, selectedSlot, windowSlots = [], sends, pan, gainDb, meter, mute, solo, arm, auto = null, live = {}, arp = null }: Props =
    $props();
  const moving = $derived(auto?.gain === "moving");
  // What lanes move (docs/architecture.md 7 "The Region collected during layout").
  // - The fader and value box (.gain-box) go under the mixerMeters canvas while a lane moves anything in this strip (coverIf).
  //   They are about as tall as the meters, so the canvas does not grow. The frame that draws that canvas shows this strip's place.
  // - The pan row (.pan-box) and the two send rows (.sends-box) do not go under the canvas (if they did, the area redrawn in one frame would go over
  //   the 25 % rule). Only the rect of the row that moved is asked for (askIf). When both move, one is shown at a time (autoState).
  // The master's fader stands under the number `MASTER_HOME` where positions are kept by track (autoState).
  const trackId = $derived(strip.kind === "track" ? strip.id : strip.kind === "master" ? MASTER_HOME : null);
  const sendsMove = $derived(auto?.sends[0] === "moving" || auto?.sends[1] === "moving");
  const panMoves = $derived(auto?.pan === "moving");
  const movingTrack = $derived(moving || panMoves || sendsMove ? trackId : null);

  const kind = $derived(strip.kind);
  const fader = faderControl(
    () => strip,
    () => gainDb,
  );
  const meterGap = $derived(metrics.px("space.1") / 2);
  /** Whether the value box is visible (strips scrolled out of view get no per-frame text; whenSeen of coverIf). */
  let valueSeen = $state(typeof IntersectionObserver === "undefined");
  /** Value text: while a lane moves it, every frame without waiting for the knob (30 times a second) (spec 2 "Handle moved by automation"). While moved by hand, the hand value. */
  const gainShown = $derived(valueSeen && live.gainDb !== undefined && fader.shown === gainDb ? live.gainDb : fader.shown);

  // ---- Insert reorder ----

  let group = $state<HTMLElement>();
  let press: { slot: number; y0: number; pointer: number; el: HTMLElement; pitch: number } | null = null;
  let drag = $state<{ slot: number; dy: number; pitch: number } | null>(null);
  /** A click after a drag does not count as pressing the slot. */
  let dragged = false;
  const target = $derived(drag ? moveSlot(drag.slot, INSERT_SLOTS, (drag.slot + 0.5) * drag.pitch + drag.dy, drag.pitch) : null);

  function slotDown(e: PointerEvent, slot: number): void {
    if (e.button !== 0 || drag) return;
    const boxes = [...(group?.querySelectorAll<HTMLElement>('[data-test="mixer-insert"]') ?? [])].map((el) => el.getBoundingClientRect());
    const pitch = boxes.length > 1 ? boxes[1]!.top - boxes[0]!.top : boxes[0]?.height ?? 1;
    // Grab only once it starts moving (grabbing earlier keeps the slot's click from reaching the slot).
    press = { slot, y0: e.clientY, pointer: e.pointerId, el: e.currentTarget as HTMLElement, pitch };
    dragged = false;
  }
  function slotMove(e: PointerEvent): void {
    if (!press || e.pointerId !== press.pointer) return;
    const dy = e.clientY - press.y0;
    if (!drag) {
      if (Math.abs(dy) < metrics.px("space.1")) return;
      press.el.setPointerCapture?.(press.pointer);
      drag = { slot: press.slot, dy, pitch: press.pitch };
    } else drag = { ...drag, dy };
  }
  function slotUp(e: PointerEvent): void {
    if (!press || e.pointerId !== press.pointer) return;
    const d = drag;
    const to = target?.to;
    endPress();
    if (!d) return;
    dragged = true;
    if (to !== undefined && to !== d.slot) send({ type: "moveInsert", insert: { strip, slot: d.slot }, to });
  }
  function endPress(): void {
    if (press?.el.hasPointerCapture?.(press.pointer)) press.el.releasePointerCapture(press.pointer);
    press = null;
    drag = null;
  }
  function swallowClick(e: MouseEvent): void {
    if (!dragged) return;
    dragged = false;
    e.stopPropagation();
    e.preventDefault();
  }
  onMount(() =>
    onDragCancel(() => {
      if (!drag) return false;
      endPress();
      return true;
    }),
  );
  const lineOf = (slot: number): "above" | "below" | null => {
    if (!target || !drag || target.to === drag.slot) return null;
    if (target.slot === slot) return "above";
    return target.slot === INSERT_SLOTS && slot === INSERT_SLOTS - 1 ? "below" : null;
  };
  const toggles = $derived([
    { key: "mute", tog: mute, label: "arrange.mute", short: "arrange.muteShort", word: "ミュート" },
    { key: "solo", tog: solo, label: "arrange.solo", short: "arrange.soloShort", word: "ソロ" },
    { key: "arm", tog: arm, label: "arrange.arm", short: "arrange.armShort", word: "録音待機" },
  ] as const);
</script>

<div class="strip {kind}" data-test="mixer-strip" data-strip={kind === "track" ? `track-${(strip as { id: number }).id}` : kind === "return" ? `return-${(strip as { bus: string }).bus}` : "master"}>
  <div class="head" style:border-top-color={color}>
    <span class="m-num m-lbl">{mark}</span>
    <span class="m-name name">{name}</span>
    {#if arp?.on}
      <button
        class="arp"
        class:live={arp.usable}
        aria-pressed="true"
        aria-disabled={!arp.usable || undefined}
        data-test="mixer-arp"
        onclick={() => arp.usable && arp.run()}>{t("input.arp")}</button
      >
    {/if}
  </div>
  {#if frame.run}
    <button
      class="m-plain frame"
      class:usable={frame.usable}
      class:accent={frame.accent}
      class:dim={frame.dim}
      class:stopped={frame.stopped}
      aria-disabled={!frame.usable || undefined}
      data-test="mixer-frame"
      use:tip={frame.tip}
      onclick={(e) => frame.usable && frame.run?.(e)}
      oncontextmenu={frame.context}
      ><span class="cut">{frame.text}</span>{#if frame.stopped}<span class="stop">{t("mixer.stoppedMark")}</span>{/if}{#if frame.window}<WindowMark
        />{/if}</button
    >
  {:else}
    <div class="frame" data-test="mixer-frame"><span class="cut">{frame.text}</span></div>
  {/if}
  <div class="group" bind:this={group} onclickcapture={swallowClick} onpointermove={slotMove} onpointerup={slotUp} onpointercancel={endPress} role="presentation">
    <span class="m-lbl" use:term={"インサート"}>{t("mixer.inserts")}</span>
    {#each slots as view, slot (slot)}
      <InsertSlot
        insert={{ strip, slot }}
        {view}
        selected={selectedSlot === slot}
        windowOpen={windowSlots.includes(slot)}
        lift={drag?.slot === slot ? drag.dy : null}
        line={lineOf(slot)}
        ondown={(e) => slotDown(e, slot)}
      />
    {/each}
  </div>
  <div class="group">
    <span class="m-lbl" use:term={"センド"}>{t("mixer.sends")}</span>
    <div class="sends-box" use:askIf={sendsMove && trackId !== null ? { track: trackId, part: "sends" } : null}>
      {#if sends}
        <SendBar track={sends.track} bus="a" db={sends.dbs[0]} home={sends.homes[0]} auto={auto?.sends[0] === "moving"} />
        <SendBar track={sends.track} bus="b" db={sends.dbs[1]} home={sends.homes[1]} auto={auto?.sends[1] === "moving"} />
      {:else}
        <div class="empty-row"></div>
        <div class="empty-row"></div>
      {/if}
    </div>
  </div>
  <div class="pan-box" use:askIf={panMoves && trackId !== null ? { track: trackId, part: "pan" } : null}>
    <PanKnob {strip} {pan} auto={panMoves} />
  </div>
  <div class="gain-box" use:coverIf={movingTrack}>
    <div class="level" style:min-height="{metrics.scaled(FADER_MIN)}px">
      <Fader control={fader} auto={moving} />
      <div class="meter" use:glSlot={{ kind: "meter", vertical: true, source: meter, gap: meterGap }}></div>
    </div>
    <div class="value" use:askLive={moving && valueSeen ? trackId : null} use:whenSeen={(seen) => (valueSeen = seen)}>
      {#if fader.typing}
        <ValueField initial={faderText(fader.shown)} label={t("mixer.db")} onsubmit={(text) => fader.submit(text)} oncancel={fader.stopTyping} />
      {:else}
        <span class="m-num m-hi db live"><span class="in" role="presentation" data-test="mixer-gain" ondblclick={fader.startTyping}>{faderText(gainShown)}</span></span>
      {/if}
      <span class="m-lbl" use:term={"dB"}>{t("mixer.db")}</span>
      {#if auto?.mark}<span class="mark" data-test="mixer-master-automation"><AutoMark {...auto.mark} /></span>{/if}
    </div>
  </div>
  {#if auto?.row}
    <div class="auto-row" data-test="mixer-auto">
      {#if auto.gain === "moving"}
        <span class="m-lbl">{t("mixer.automation")}</span>
      {:else if auto.gain === "resting"}
        <span class="m-lbl">{t("lane.restingBare")}</span>
        <button
          class="m-btn restore"
          aria-disabled={!auto.restore || undefined}
          data-test="mixer-auto-restore"
          onclick={() => auto.restore?.()}>{t("lane.restore")}</button
        >
      {/if}
    </div>
  {/if}
  <div class="toggles">
    {#each toggles as b (b.key)}
      <button
        class="tog {b.key}"
        class:live={b.tog.usable}
        class:on={b.tog.usable && b.tog.on}
        aria-pressed={b.tog.usable ? b.tog.on : undefined}
        aria-disabled={!b.tog.usable || undefined}
        tabindex={b.tog.usable ? undefined : -1}
        aria-label={t(b.label)}
        data-test="mixer-{b.key}"
        use:term={b.word}
        onclick={() => b.tog.usable && b.tog.run()}>{t(b.short)}</button
      >
    {/each}
  </div>
</div>

<style>
  .strip {
    flex-shrink: 0;
    width: var(--size-strip);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: 0 var(--space-1) var(--space-2);
    background: var(--color-gray-3);
    border-right: var(--line-hair) solid var(--color-line-thin);
    /* Strips scrolled out of view are not drawn or laid out (in a 64-track song, the boxes of 40 hidden strips are not walked on every redraw).
       The width is fixed and the height follows the neighbors, so the size does not change. For Tab and find, the browser builds the inside. */
    content-visibility: auto;
  }
  .strip.return {
    background: var(--color-gray-2);
  }
  .strip.master {
    width: var(--part-strip-master);
    background: var(--color-gray-4);
    border-right-color: var(--color-gray-6);
  }
  .head {
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0 calc(-1 * var(--space-1));
    padding: 0 var(--space-1);
    border-top: var(--line-mark) solid;
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* 「アルペジオ」 while on (the place of MIDI effects is above 「音源」, spec 3): at the heading's right end, so nothing below moves.
     The look of M / S on; while counting in or recording it cannot be pressed. */
  .arp {
    flex-shrink: 0;
    margin-left: auto;
    height: var(--part-toggle-size);
    padding: 0 var(--space-1);
    border: var(--line-hair) solid var(--color-accent);
    border-radius: var(--radius-1);
    background: var(--color-gray-1);
    color: var(--color-accent);
    font: inherit;
    cursor: default;
  }
  .arp.live {
    cursor: pointer;
  }
  .arp.live:active {
    background: var(--color-gray-2);
  }
  .frame {
    height: var(--size-row);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    padding: 0 var(--space-1);
    border: var(--line-hair) solid var(--color-gray-6);
    background: var(--color-gray-1);
    color: var(--color-gray-9);
    white-space: nowrap;
    overflow: hidden;
  }
  button.frame {
    cursor: default;
    color: var(--color-gray-7);
    border-color: var(--color-gray-5);
  }
  button.frame.usable {
    cursor: pointer;
    color: var(--color-gray-9);
    border-color: var(--color-gray-6);
  }
  button.frame.usable:hover {
    border-color: var(--color-gray-8);
    color: var(--color-gray-10);
  }
  button.frame.accent,
  button.frame.accent:hover {
    color: var(--color-accent);
  }
  button.frame.usable.dim {
    color: var(--color-gray-8);
  }
  button.frame.usable {
    gap: var(--space-1);
    justify-content: space-between;
  }
  /* A stopped plugin (the same red border and 「止」 as spec 2 "Insert slot"). */
  button.frame.usable.stopped,
  button.frame.usable.stopped:hover {
    border-color: var(--color-rec);
    color: var(--color-rec);
    justify-content: space-between;
    gap: var(--space-1);
  }
  .cut {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .group {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: calc(var(--space-1) / 2);
  }
  .empty-row {
    height: var(--size-row);
  }
  /* The box of what lanes move. coverIf / askIf put an empty box filling it inside. That box reaches out left and right to the full strip width:
     joined with the boxes of the next strips, WebKit's collected rects become one (docs/architecture.md 7 "The Region collected during layout"). */
  .sends-box,
  .pan-box,
  .gain-box,
  .value {
    position: relative;
    --ask-left: calc(-1 * var(--space-1));
    --ask-right: calc(-1 * var(--space-1) - var(--line-hair));
  }
  /* The inner order stays as it was (.group, strip). */
  .sends-box {
    display: flex;
    flex-direction: column;
    gap: calc(var(--space-1) / 2);
  }
  .pan-box {
    flex-shrink: 0;
  }
  .gain-box {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    /* The handle at the very top (+6 dB) sticks out above the fader by half its height. */
    --ask-top: calc(var(--part-fader-knob-h) / -2 - var(--line-hair));
  }
  .level {
    flex: 1;
    display: flex;
    gap: var(--space-1);
  }
  .meter {
    flex-shrink: 0;
    width: calc(var(--part-meter-vertical) * 2 + var(--space-1) / 2);
  }
  .value {
    flex-shrink: 0;
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }
  /* The master's mark that writes its volume as automation: at the right end of the value's row, without making the row taller. */
  .mark {
    align-self: center;
    display: flex;
    margin: calc((var(--text-m-line-height) - var(--part-param-automation)) / 2) 0;
    margin-left: var(--space-1);
  }
  .db {
    flex: 1;
    min-width: 0;
    font-size: var(--text-m-size);
    line-height: var(--text-m-line-height);
  }
  /* Text that may change every frame goes in a box out of the flow: when the text changes, the strip's vertical layout is not run again
     (docs/architecture.md 7 "WebKit that paints on the CPU"). The outer box keeps the text's height and baseline with a zero-width character. */
  .live {
    position: relative;
  }
  .live::before {
    content: "\200b";
  }
  .live > .in {
    position: absolute;
    inset: 0;
    overflow: hidden;
    white-space: nowrap;
  }
  /* Below the fader value (mock F6-MixerAuto / F6-MixerRest: height 20, 「オートメーション」 in gray.8; when resting, 「休み」 and 「もどす」). */
  .auto-row {
    height: var(--part-toggle-size);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .restore {
    height: var(--part-toggle-size);
  }
  .toggles {
    flex-shrink: 0;
    display: flex;
    gap: var(--space-1);
  }
  /* M / S / ● (spec 2 "Toggle M, S, ●", the same look as the track head). Border gray.5; disabled ones have text gray.7. */
  .tog {
    flex: 1;
    min-width: 0;
    height: var(--part-toggle-size);
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
  .tog.live.on {
    border-color: var(--color-accent);
    color: var(--color-accent);
  }
  .tog.arm.on {
    border-color: var(--color-rec);
    color: var(--color-rec);
  }
  .tog.live:active {
    background: var(--color-gray-2);
    border-color: var(--color-gray-8);
  }
</style>

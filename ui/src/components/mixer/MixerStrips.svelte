<!-- The content of the mixer band (mock F4-Mixer): the row of tracks, returns A / B, the master strip and 「仕上げのメーター」 (FinishMeters, round 12 M3). Scrolls sideways; when the band is low, only the strips
     scroll up and down (spec 2: at first the faders and meters at the bottom are shown, and scrolling up shows the inserts). Keeps the position from the bottom when the height changes.
     Meters are one surface in this box's WebGL region mixerMeters (docs/architecture.md 19-5). With more than 10 tracks, at 30 Hz, the left and right halves are drawn one frame each (gl/engine.ts, architecture 7).
     Pressing a plugin instrument slot does 「えらぶ ＋ 窓」 like an insert (SelectInstrument). Replacing is 「音源を入れかえる…」 in the context menu (user:2026-11-19).
     A master with no output shows 「出口なし ▸」 (gray.8; pressing it opens the sound settings; spec 3 (86-8)). -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { InsertRef, PluginSlotRef, StripRef, TrackView } from "../../bridge";
  import { glRegion } from "../../gl/slot";
  import HScrollbar from "../../lib/HScrollbar.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text";
  import { soundRefName, soundViewName, standInTip } from "../sounds";
  import { openContextMenu } from "../menu/contextMenu.svelte";
  import { selectedInsert, windowOf } from "../params/selected";
  import { openShelf } from "../tracks.svelte";
  import { vocalInputRows } from "../trackMenu";
  import Strip from "./Strip.svelte";
  import FinishMeters from "./FinishMeters.svelte";
  import { mixerLivePosition, shownPosition } from "../automation/autoState.svelte";
  import { hasLaneRef, laneMoves, laneOf, masterHome, masterOwner, panAt, type AutoLook, type LaneOwner, type StripAuto } from "../automation/laneValue";
  import { fracToDb } from "./scale";
  import type { AutoTarget } from "../../bridge";
  import { returnEffectName, stripName } from "./stripName";
  import VScrollbar from "./VScrollbar.svelte";

  const app = $derived(current.state!);
  const song = $derived(app.song);
  const busy = $derived(app.transport.state === "recording" || app.transport.state === "countIn");
  const selected = $derived(selectedInsert(app.params));
  const shelf = $derived(app.shelf);
  const outputName = $derived(app.outputs.list.find((o) => o.id === app.outputs.chosen)?.name ?? null);
  /** No output: none chosen, or the chosen output is gone (docs/history/design-notes.ja.md A-17). */
  const noOutput = $derived(app.outputs.chosen === null || app.outputs.chosenState === "missing" || outputName === null);
  const masterFrame = $derived(
    noOutput
      ? { text: t("mixer.noOutput"), usable: true, dim: true, tip: t("mixer.noOutputTip"), run: () => send({ type: "openSettings", advanced: false }) }
      : { text: t("mixer.output", { name: outputName! }) },
  );

  /** Index of the insert whose window is open. */
  function windowSlots(strip: StripRef): number[] {
    return app.windows.list.flatMap((w) => (w.slot?.kind === "insert" && sameStrip(w.slot.insert.strip, strip) ? [w.slot.insert.slot] : []));
  }

  // ---- Automation (docs/architecture.md 21-8: the screen works out the value itself from the lane points and the play line) ----

  /** If any track has a volume lane, every strip shows the row under the value. */
  const autoRow = $derived(song.master.lanes.length > 0 || song.tracks.some((tr) => laneOf(tr, { kind: "volume" }) !== undefined));
  function lookOf(track: LaneOwner, target: AutoTarget): AutoLook {
    const lane = laneOf(track, target);
    if (!lane) return null;
    if (laneMoves(lane, app.automation.held, track.id)) return "moving";
    return lane.resting || hasLaneRef(app.automation.held, track.id, target) ? "resting" : null;
  }
  function autoOf(track: TrackView): StripAuto {
    const volume = laneOf(track, { kind: "volume" });
    return {
      gain: lookOf(track, { kind: "volume" }),
      pan: lookOf(track, { kind: "pan" }),
      sends: [lookOf(track, { kind: "send", bus: "a" }), lookOf(track, { kind: "send", bus: "b" })],
      row: autoRow,
      restore: volume?.resting && !busy ? () => send({ type: "restoreLane", home: { kind: "track", track: track.id }, target: { kind: "volume" } }) : null,
    };
  }
  /** Returns have no lanes (A-19, item 7). Only the row under the value is shown, to keep the heights equal. */
  const noAuto = $derived<StripAuto>({ gain: null, pan: null, sends: [null, null], row: autoRow, restore: null });
  /** The master has a lane of its volume only (round 12 M5): the fader follows it as a track's does, and the mark by the value makes it. */
  const master = $derived(masterOwner(song.master));
  const masterAuto = $derived.by((): StripAuto => {
    const volume = laneOf(master, { kind: "volume" });
    return {
      ...noAuto,
      gain: lookOf(master, { kind: "volume" }),
      restore: volume?.resting && !busy ? () => send({ type: "restoreLane", home: masterHome, target: { kind: "volume" } }) : null,
      mark: { on: volume !== undefined, usable: !busy, add: () => send({ type: "addLane", home: masterHome, target: { kind: "volume" } }) },
    };
  });
  /** What a lane moves shows the value on the line; others show the value set by hand. */
  function shownGain(track: LaneOwner, look: AutoLook): number | null {
    const pos = look === "moving" ? shownPosition(track.id, { kind: "volume" }) : null;
    return pos === null ? track.gainDb : fracToDb(pos);
  }
  function shownPan(track: TrackView, look: AutoLook): number {
    const pos = look === "moving" ? shownPosition(track.id, { kind: "pan" }) : null;
    return pos === null ? track.pan : panAt(pos);
  }
  function shownSends(track: TrackView, auto: StripAuto): [number | null, number | null] {
    return BUSES.map((bus, i) => {
      const pos = auto.sends[i] === "moving" ? shownPosition(track.id, { kind: "send", bus }) : null;
      return pos === null ? track.sends[i]! : fracToDb(pos);
    }) as [number | null, number | null];
  }
  /** The newest value on the lane line for the fader value text (every frame). undefined when not moving. */
  function liveValues(track: LaneOwner, auto: StripAuto): { gainDb?: number | null } {
    const pos = auto.gain === "moving" ? mixerLivePosition(track.id, { kind: "volume" }) : null;
    return { gainDb: pos === null ? undefined : fracToDb(pos) };
  }
  function shownMuted(track: TrackView): boolean {
    const lane = laneOf(track, { kind: "mute" });
    const pos = laneMoves(lane, app.automation.held, track.id) ? shownPosition(track.id, { kind: "mute" }) : null;
    return pos === null ? track.muted : pos >= 0.5;
  }
  const sameStrip = (a: StripRef, b: StripRef) => JSON.stringify(a) === JSON.stringify(b);
  const slotOf = (strip: StripRef, sel: InsertRef | null) => (sel && sameStrip(sel.strip, strip) ? sel.slot : null);
  const off = { on: false, usable: false, run: () => {} };
  const BUSES = ["a", "b"] as const;

  /**
   * During audition for a replace, the auditioned instrument's name is in accent (as in the track head).
   * While a sine plays in its place, 「サイン波で代わり」 (gray.8; spec 3 (85)); a stopped plugin has a red border and 「止」.
   */
  function instrumentFrame(track: TrackView) {
    // The instrument slot of a vocal track shows the input name (spec 3, the track head of a vocal track). Pressing it opens the same input list as the track head.
    const kind = track.kind;
    if (kind.kind === "vocal") {
      const inputs = (e: MouseEvent) => {
        const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
        openContextMenu(r.left, r.bottom, vocalInputRows(track.id, kind.input, busy, send));
      };
      return {
        text: kind.inputReady ? t(`vocal.input.${kind.input}`) : t("vocal.noInput"),
        dim: !kind.inputReady,
        usable: true,
        run: inputs,
        context: (e: MouseEvent) => e.preventDefault(),
      };
    }
    const trial = shelf.mode.kind === "replace" && shelf.mode.track === track.id ? shelf.selectedSound : null;
    const name = trial ? soundRefName(trial, app.library) : track.standIn ? t("track.standIn") : soundViewName(track.sound);
    const replace = () => send(openShelf({ kind: "replace", track: track.id }));
    if (trial === null && track.sound.kind === "plugin" && !track.standIn) {
      const slot: PluginSlotRef = { kind: "instrument", track: track.id };
      return {
        text: t("mixer.instrument", { name }),
        stopped: track.sound.state === "stopped",
        usable: true,
        window: windowOf(app.windows, slot) !== null,
        run: () => send({ type: "selectInstrument", track: track.id }),
        context: (e: MouseEvent) => {
          e.preventDefault();
          openContextMenu(e.clientX, e.clientY, [{ kind: "row", label: "track.replaceSound", usable: !busy, run: replace }]);
        },
      };
    }
    return {
      text: t("mixer.instrument", { name }),
      accent: trial !== null,
      dim: trial === null && track.standIn !== null,
      tip: trial === null && track.standIn ? standInTip(track.standIn, track.sound) : null,
      stopped: trial === null && !track.standIn && track.sound.kind === "plugin" && track.sound.state === "stopped",
      usable: !busy,
      run: replace,
      context: (e: MouseEvent) => {
        e.preventDefault();
        openContextMenu(e.clientX, e.clientY, [{ kind: "row", label: "track.replaceSound", usable: !busy, run: replace }]);
      },
    };
  }

  // ---- Scrolling ----

  let view = $state<HTMLDivElement>();
  let scroll = $state({ left: 0, top: 0, width: 0, height: 0, viewW: 0, viewH: 0 });
  /** Position from the bottom. 0 (the very bottom) at first. */
  let fromBottom = 0;

  function measure(): void {
    if (!view) return;
    scroll = { left: view.scrollLeft, top: view.scrollTop, width: view.scrollWidth, height: view.scrollHeight, viewW: view.clientWidth, viewH: view.clientHeight };
  }
  /** The scrollTop that keepBottom set. Its scroll event comes later (the content height may have changed already), so it is not taken as a move by a person. */
  let placed: number | null = null;
  function onScroll(): void {
    if (!view) return;
    if (placed === null || view.scrollTop !== placed) {
      placed = null;
      fromBottom = Math.max(0, view.scrollHeight - view.clientHeight - view.scrollTop);
    }
    measure();
  }
  /** When the size changes, keep the position from the bottom. */
  function keepBottom(): void {
    if (!view) return;
    view.scrollTop = Math.max(0, view.scrollHeight - view.clientHeight - fromBottom);
    placed = view.scrollTop;
    measure();
  }
  onMount(() => {
    const observer = new ResizeObserver(keepBottom);
    if (view) {
      observer.observe(view);
      if (view.firstElementChild) observer.observe(view.firstElementChild);
    }
    keepBottom();
    return () => observer.disconnect();
  });
</script>

<div class="mixer">
  <div class="view" bind:this={view} onscroll={onScroll} use:glRegion={"mixerMeters"} data-test="mixer-view">
    <div class="row">
      {#each song.tracks as track (track.id)}
        {@const strip: StripRef = { kind: "track", id: track.id }}
        {@const auto = autoOf(track)}
        <Strip
          {strip}
          mark={String(track.number)}
          name={track.name}
          color="var(--color-track-{((track.color - 1) % 5) + 1})"
          frame={instrumentFrame(track)}
          slots={track.slots}
          selectedSlot={slotOf(strip, selected)}
          windowSlots={windowSlots(strip)}
          sends={{ track: track.id, dbs: shownSends(track, auto), homes: track.sendDefaults }}
          pan={shownPan(track, auto.pan)}
          gainDb={shownGain(track, auto.gain)}
          meter={{ track: track.number }}
          mute={{ on: shownMuted(track), usable: true, run: () => send({ type: "toggleMute", track: track.id }) }}
          {auto}
          live={liveValues(track, auto)}
          solo={{ on: track.soloed, usable: true, run: () => send({ type: "toggleSolo", track: track.id }) }}
          arm={track.kind.kind === "vocal"
            ? { on: track.kind.armed, usable: true, run: () => send({ type: "toggleVocalArm", track: track.id }) }
            : { on: song.keysTarget === track.id, usable: true, run: () => send({ type: "selectTrack", track: track.id, fromArm: true }) }}
          arp={{ on: track.arp.on, usable: !busy, run: () => send({ type: "setArp", track: track.id, arp: { ...track.arp, on: false } }) }}
        />
      {/each}
      {#each BUSES as bus, i (bus)}
        {@const ret = song.returns[i]!}
        {@const strip: StripRef = { kind: "return", bus }}
        {@const top = ret.slots.find((e) => e !== null)}
        <Strip
          {strip}
          mark={t(bus === "a" ? "mixer.sendA" : "mixer.sendB")}
          name={top ? returnEffectName(top, app.library) : ""}
          color="var(--color-gray-8)"
          frame={{ text: t("mixer.returnOf", { bus: t(bus === "a" ? "mixer.sendA" : "mixer.sendB") }) }}
          slots={ret.slots}
          selectedSlot={slotOf(strip, selected)}
          windowSlots={windowSlots(strip)}
          sends={null}
          pan={ret.pan}
          gainDb={ret.gainDb}
          meter={{ ret: bus }}
          mute={{ on: ret.muted, usable: true, run: () => send({ type: "toggleReturnMute", bus }) }}
          solo={off}
          arm={off}
          auto={noAuto}
        />
      {/each}
      <Strip
        strip={{ kind: "master" }}
        mark=""
        name={stripName(song, { kind: "master" }, app.library)}
        color="var(--color-gray-10)"
        frame={masterFrame}
        slots={song.master.slots}
        selectedSlot={slotOf({ kind: "master" }, selected)}
        windowSlots={windowSlots({ kind: "master" })}
        sends={null}
        pan={song.master.pan}
        gainDb={shownGain(master, masterAuto.gain)}
        meter="master"
        mute={off}
        solo={off}
        arm={off}
        auto={masterAuto}
        live={liveValues(master, masterAuto)}
      />
      <FinishMeters />
    </div>
  </div>
  <VScrollbar total={scroll.height} visible={scroll.viewH} offset={scroll.top} onscroll={(top) => view && ((view.scrollTop = top), onScroll())} />
  <HScrollbar total={scroll.width} visible={scroll.viewW} offset={scroll.left} onscroll={(left) => view && ((view.scrollLeft = left), onScroll())} />
</div>

<style>
  .mixer {
    position: relative;
    height: 100%;
    background: var(--color-gray-1);
  }
  .view {
    position: relative;
    height: 100%;
    overflow: auto;
    scrollbar-width: none;
  }
  .view::-webkit-scrollbar {
    display: none;
  }
  .row {
    display: flex;
    width: max-content;
    min-height: 100%;
  }
</style>

<!-- ミキサーの帯の中身（モック F4-Mixer）: トラックの並び、戻り A ・ B、マスターのストリップ。横にスクロールし、帯が低いときはストリップだけ
     縦にスクロールする（仕様書 2 章: はじめは下のフェーダーとメーターを見せ、上へスクロールするとインサートが出る）。高さが変わっても、下からの位置を保つ。
     メーターは、この箱の WebGL の区画 mixerMeters に 1 枚（docs/architecture.md 19-5）。トラックが 10 本をこえると 30 Hz で、左半分と右半分を 1 コマずつ出す（gl/engine.ts、7 章）。
     プラグインの音源の枠を押すと、インサートと同じに「えらぶ ＋ 窓」（SelectInstrument）。入れかえは右クリックの「音源を入れかえる…」（user:2026-11-19）。
     出口がないマスターは「出口なし ▸」（gray.8、押すと音の設定。仕様書 3 章 86-8）。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { InsertRef, PluginSlotRef, StripRef, TrackView } from "../../bridge";
  import { glRegion } from "../../gl/slot";
  import HScrollbar from "../../lib/HScrollbar.svelte";
  import { current, send } from "../../state/app.svelte";
  import { t } from "../../text/ja";
  import { soundRefName, soundViewName, standInTip } from "../sounds";
  import { openContextMenu } from "../menu/contextMenu.svelte";
  import { selectedInsert, windowOf } from "../params/selected";
  import { openShelf } from "../tracks.svelte";
  import Strip from "./Strip.svelte";
  import { livePosition } from "../automation/autoState.svelte";
  import { hasLaneRef, laneMoves, laneOf, panAt, type AutoLook, type StripAuto } from "../automation/laneValue";
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
  /** 出口がない: えらんでいないか、えらんだ出口がなくなった（design-notes A-17）。 */
  const noOutput = $derived(app.outputs.chosen === null || app.outputs.chosenState === "missing" || outputName === null);
  const masterFrame = $derived(
    noOutput
      ? { text: t("mixer.noOutput"), usable: true, dim: true, tip: t("mixer.noOutputTip"), run: () => send({ type: "openSettings", advanced: false }) }
      : { text: t("mixer.output", { name: outputName! }) },
  );

  /** 窓がひらいているインサートの番号。 */
  function windowSlots(strip: StripRef): number[] {
    return app.windows.list.flatMap((w) => (w.slot?.kind === "insert" && sameStrip(w.slot.insert.strip, strip) ? [w.slot.insert.slot] : []));
  }

  // ---- オートメーション（docs/architecture.md 21-8: 帯の点と再生の線の所から、画面が自分で値を出す） ----

  /** どこかのトラックに音量の帯があれば、どのストリップにも値の下の行を出す。 */
  const autoRow = $derived(song.tracks.some((tr) => laneOf(tr, { kind: "volume" }) !== undefined));
  function lookOf(track: TrackView, target: AutoTarget): AutoLook {
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
      restore: volume?.resting && !busy ? () => send({ type: "restoreLane", track: track.id, target: { kind: "volume" } }) : null,
    };
  }
  /** 戻りとマスターには帯がない（A-19 の 7）。値の下の行だけ、高さをそろえるために出す。 */
  const noAuto = $derived<StripAuto>({ gain: null, pan: null, sends: [null, null], row: autoRow, restore: null });
  /** 帯が動かしている物は線の上の値、ほかは手で置いた値。 */
  function shownGain(track: TrackView, look: AutoLook): number | null {
    const pos = look === "moving" ? livePosition(track.id, { kind: "volume" }) : null;
    return pos === null ? track.gainDb : fracToDb(pos);
  }
  function shownPan(track: TrackView, look: AutoLook): number {
    const pos = look === "moving" ? livePosition(track.id, { kind: "pan" }) : null;
    return pos === null ? track.pan : panAt(pos);
  }
  function shownSends(track: TrackView, auto: StripAuto): [number | null, number | null] {
    return BUSES.map((bus, i) => {
      const pos = auto.sends[i] === "moving" ? livePosition(track.id, { kind: "send", bus }) : null;
      return pos === null ? track.sends[i]! : fracToDb(pos);
    }) as [number | null, number | null];
  }
  function shownMuted(track: TrackView): boolean {
    const lane = laneOf(track, { kind: "mute" });
    const pos = laneMoves(lane, app.automation.held, track.id) ? livePosition(track.id, { kind: "mute" }) : null;
    return pos === null ? track.muted : pos >= 0.5;
  }
  const sameStrip = (a: StripRef, b: StripRef) => JSON.stringify(a) === JSON.stringify(b);
  const slotOf = (strip: StripRef, sel: InsertRef | null) => (sel && sameStrip(sel.strip, strip) ? sel.slot : null);
  const off = { on: false, usable: false, run: () => {} };
  const BUSES = ["a", "b"] as const;

  /**
   * 入れかえのためし弾き中は、ためしの音源の名前を accent で（トラックの見出しと同じ）。
   * サイン波で代わりに鳴らしている間は「サイン波で代わり」（gray.8。仕様書 3 章 85）、止まったプラグインは赤い枠と「止」。
   */
  function instrumentFrame(track: TrackView) {
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

  // ---- スクロール ----

  let view = $state<HTMLDivElement>();
  let scroll = $state({ left: 0, top: 0, width: 0, height: 0, viewW: 0, viewH: 0 });
  /** 下からの位置。はじめは 0（いちばん下）。 */
  let fromBottom = 0;

  function measure(): void {
    if (!view) return;
    scroll = { left: view.scrollLeft, top: view.scrollTop, width: view.scrollWidth, height: view.scrollHeight, viewW: view.clientWidth, viewH: view.clientHeight };
  }
  /** keepBottom が置いた scrollTop。その scroll の知らせは、あとから来るので（中身の高さがもう変わっているかもしれない）、人が動かした物と見ない。 */
  let placed: number | null = null;
  function onScroll(): void {
    if (!view) return;
    if (placed === null || view.scrollTop !== placed) {
      placed = null;
      fromBottom = Math.max(0, view.scrollHeight - view.clientHeight - view.scrollTop);
    }
    measure();
  }
  /** 大きさが変わったら、下からの位置を保つ。 */
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
          solo={{ on: track.soloed, usable: true, run: () => send({ type: "toggleSolo", track: track.id }) }}
          arm={{ on: song.playTarget === track.id, usable: true, run: () => send({ type: "selectTrack", track: track.id, fromArm: true }) }}
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
        gainDb={song.master.gainDb}
        meter="master"
        mute={off}
        solo={off}
        arm={off}
        auto={noAuto}
      />
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

<!-- ミキサーの 1 本（モック F4-Mixer）。上から、見出し（番号か A ・ B と名前）、音源 ・ 戻り ・ 出口の枠、インサート 4 つ、センド、パン、
     目盛とフェーダーとメーター、フェーダーの値、M ・ S ・ ●。メーターは mixerMeters の区画（MixerStrips の箱）の中の slot。
     インサートの並べかえ（仕様書 3 章 86-2）: 入っている枠を押したまま上下に動かす。入る所に accent の線、離して MoveInsert を 1 つ、Esc でやめる。同じストリップの中だけ。 -->
<script lang="ts">
  import { onMount } from "svelte";
  import type { InsertView, StripRef } from "../../bridge";
  import { INSERT_SLOTS } from "../../bridge/types";
  import { glSlot, type MeterSource } from "../../gl/slot";
  import { term, tip } from "../../lib/tip.svelte";
  import { t } from "../../text/ja";
  import { metrics } from "../metrics.svelte";
  import ValueField from "../params/ValueField.svelte";
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
  import { coverIf } from "../automation/coverIf";
  import type { StripAuto } from "../automation/laneValue";

  interface Toggle {
    on: boolean;
    usable: boolean;
    run: () => void;
  }

  interface Props {
    strip: StripRef;
    /** 見出しの左（トラックの番号、戻りの A ・ B。マスターは空）と名前。 */
    mark: string;
    name: string;
    /** 見出しの上辺の色（CSS の変数）。 */
    color: string;
    /** dim: 「サイン波で代わり」（gray.8、乗ると tip）。stopped: 止まったプラグインの音源（赤い枠と「止」）。 */
    /** window: プラグインの窓がひらいている（右はしに窓の印）。context: 右クリック。 */
    frame: {
      text: string;
      accent?: boolean;
      run?: () => void;
      usable?: boolean;
      dim?: boolean;
      tip?: string | null;
      stopped?: boolean;
      window?: boolean;
      context?: (e: MouseEvent) => void;
    };
    slots: readonly (InsertView | null)[];
    selectedSlot: number | null;
    /** 窓がひらいているインサート（枠の番号）。 */
    windowSlots?: readonly number[];
    /** トラックだけ。homes は Home で戻す量（トラックを足したときの量）。 */
    sends: { track: number; dbs: readonly [number | null, number | null]; homes: readonly [number | null, number | null] } | null;
    pan: number;
    gainDb: number | null;
    meter: MeterSource;
    mute: Toggle;
    solo: Toggle;
    arm: Toggle;
    auto?: StripAuto | null;
  }
  let { strip, mark, name, color, frame, slots, selectedSlot, windowSlots = [], sends, pan, gainDb, meter, mute, solo, arm, auto = null }: Props = $props();
  const moving = $derived(auto?.gain === "moving");

  const kind = $derived(strip.kind);
  const fader = faderControl(
    () => strip,
    () => gainDb,
  );
  const meterGap = $derived(metrics.px("space.1") / 2);

  // ---- インサートの並べかえ ----

  let group = $state<HTMLElement>();
  let press: { slot: number; y0: number; pointer: number; el: HTMLElement; pitch: number } | null = null;
  let drag = $state<{ slot: number; dy: number; pitch: number } | null>(null);
  /** ドラッグのあとの click は、枠を押したことにしない。 */
  let dragged = false;
  const target = $derived(drag ? moveSlot(drag.slot, INSERT_SLOTS, (drag.slot + 0.5) * drag.pitch + drag.dy, drag.pitch) : null);

  function slotDown(e: PointerEvent, slot: number): void {
    if (e.button !== 0 || drag) return;
    const boxes = [...(group?.querySelectorAll<HTMLElement>('[data-test="mixer-insert"]') ?? [])].map((el) => el.getBoundingClientRect());
    const pitch = boxes.length > 1 ? boxes[1]!.top - boxes[0]!.top : boxes[0]?.height ?? 1;
    // つかむのは動きだしてから（その前につかむと、枠を押した click が枠に届かない）。
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
      onclick={() => frame.usable && frame.run?.()}
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
    {#if sends}
      <SendBar track={sends.track} bus="a" db={sends.dbs[0]} home={sends.homes[0]} auto={auto?.sends[0] === "moving"} />
      <SendBar track={sends.track} bus="b" db={sends.dbs[1]} home={sends.homes[1]} auto={auto?.sends[1] === "moving"} />
    {:else}
      <div class="empty-row"></div>
      <div class="empty-row"></div>
    {/if}
  </div>
  <PanKnob {strip} {pan} auto={auto?.pan === "moving"} />
  <!-- 動いているフェーダーと値は、毎コマ変わるので mixerMeters の canvas の下に入れる（docs/architecture.md 7 章）。 -->
  <div class="level" style:min-height="{metrics.scaled(FADER_MIN)}px" use:coverIf={moving}>
    <Fader control={fader} auto={moving} />
    <div class="meter" use:glSlot={{ kind: "meter", vertical: true, source: meter, gap: meterGap }}></div>
  </div>
  <div class="value" use:coverIf={moving}>
    {#if fader.typing}
      <ValueField initial={faderText(fader.shown)} label={t("mixer.db")} onsubmit={(text) => fader.submit(text)} oncancel={fader.stopTyping} />
    {:else}
      <span class="m-num m-hi db" data-test="mixer-gain">{faderText(fader.shown)}</span>
    {/if}
    <span class="m-lbl" use:term={"dB"}>{t("mixer.db")}</span>
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
    border-right: var(--line-hair) solid var(--color-gray-5);
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
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
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
  /* 止まったプラグイン（仕様書 2 章「インサートの枠」と同じ赤い枠と「止」）。 */
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
  .db {
    font-size: var(--text-m-size);
    line-height: var(--text-m-line-height);
  }
  /* フェーダーの値の下（モック F6-MixerAuto ・ F6-MixerRest: 高さ 20、gray.8 の「オートメーション」、休みは「休み」と「もどす」）。 */
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
  /* M ・ S ・ ●（仕様書 2 章「切りかえ」、トラックの見出しと同じ見た目）。使えない物は枠 gray.5 ・ 文字 gray.7。 */
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
  .tog.live {
    border-color: var(--color-gray-6);
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

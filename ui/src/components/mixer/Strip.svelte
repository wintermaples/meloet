<!-- ミキサーの 1 本（モック F4-Mixer）。上から、見出し（番号か A ・ B と名前）、音源 ・ 戻り ・ 出口の枠、インサート 4 つ、センド、パン、
     目盛とフェーダーとメーター、フェーダーの値、M ・ S ・ ●。メーターは mixerMeters の区画（MixerStrips の箱）の中の slot。 -->
<script lang="ts">
  import type { EffectKind, StripRef } from "../../bridge";
  import { glSlot, type MeterSource } from "../../gl/slot";
  import { term } from "../../lib/tip.svelte";
  import { t } from "../../text/ja";
  import { metrics } from "../metrics.svelte";
  import Fader from "./Fader.svelte";
  import InsertSlot from "./InsertSlot.svelte";
  import PanKnob from "./PanKnob.svelte";
  import { faderControl } from "./control.svelte";
  import { FADER_MIN, faderText } from "./scale";
  import SendBar from "./SendBar.svelte";

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
    frame: { text: string; accent?: boolean; run?: () => void; usable?: boolean };
    inserts: readonly (EffectKind | null)[];
    selectedSlot: number | null;
    /** トラックだけ。 */
    sends: { track: number; dbs: readonly [number | null, number | null] } | null;
    pan: number;
    gainDb: number | null;
    meter: MeterSource;
    mute: Toggle;
    solo: Toggle;
    arm: Toggle;
  }
  let { strip, mark, name, color, frame, inserts, selectedSlot, sends, pan, gainDb, meter, mute, solo, arm }: Props = $props();

  const kind = $derived(strip.kind);
  const fader = faderControl(
    () => strip,
    () => gainDb,
  );
  const meterGap = $derived(metrics.px("space.1") / 2);
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
      aria-disabled={!frame.usable || undefined}
      data-test="mixer-frame"
      onclick={() => frame.usable && frame.run?.()}><span class="cut">{frame.text}</span></button
    >
  {:else}
    <div class="frame" data-test="mixer-frame"><span class="cut">{frame.text}</span></div>
  {/if}
  <div class="group">
    <span class="m-lbl" use:term={"インサート"}>{t("mixer.inserts")}</span>
    {#each inserts as effect, slot (slot)}
      <InsertSlot insert={{ strip, slot }} {effect} selected={selectedSlot === slot} />
    {/each}
  </div>
  <div class="group">
    <span class="m-lbl" use:term={"センド"}>{t("mixer.sends")}</span>
    {#if sends}
      <SendBar track={sends.track} bus="a" db={sends.dbs[0]} />
      <SendBar track={sends.track} bus="b" db={sends.dbs[1]} />
    {:else}
      <div class="empty-row"></div>
      <div class="empty-row"></div>
    {/if}
  </div>
  <PanKnob {strip} {pan} />
  <div class="level" style:min-height="{metrics.scaled(FADER_MIN)}px">
    <Fader control={fader} />
    <div class="meter" use:glSlot={{ kind: "meter", vertical: true, source: meter, gap: meterGap }}></div>
  </div>
  <div class="value">
    <span class="m-num m-hi db" data-test="mixer-gain">{faderText(fader.shown)}</span>
    <span class="m-lbl" use:term={"dB"}>{t("mixer.db")}</span>
  </div>
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

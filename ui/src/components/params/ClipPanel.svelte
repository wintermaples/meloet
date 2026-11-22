<!-- 選んだクリップ（仕様書 2 章「選んだクリップ」、3 章 93-4 ・ Y22、モック F7-AudioEdit ・ F7-AudioMissing）。パラメータの欄の中身。
     上に名前と「選んだクリップ」、ファイルの行（「6 歌 ・ 歌 テイク 1.wav ・ 48000 Hz ・ 24 ビット」。サンプルレートがちがえば「44100 Hz（曲に合わせて鳴らします）」）。
     グループ「クリップ」（位置 ・ 長さ ・ 音量 ・ フェード）。値を 2 回押すと打ちこむ枠（Enter で SetClipField を 1 つ。範囲の外は Rust がだまって中に入れる。B-94-29）。
     グループ「のび縮み ・ 音の高さ」（3 章「のび縮み」「音の高さ」、モック F7-Stretch）: オン ・ オフは 12×12 の四角の印（パラメータの行と同じ）を押して
     SetClipStretch。この音のテンポは 2 回押して打ちこむ。「—」のときは「さがす」（FindSourceTempo）。音の高さは半音とセントを別々に、
     上下のドラッグ（離したときに 1 手）か 2 回押して打ちこむ。音を作っている間は、欄の上に進みの棒（audioWork の rendering）。
     音のファイルが見つからないときは、グループのかわりに見つからない物の知らせ（「さがす…」「曲から外す」）。 -->
<script lang="ts">
  import { CLIP_CENTS_MAX, CLIP_SEMITONES_MAX, type ClipField, type ClipView, type SelectedClip, type StretchChange, type ValueField as ValueFieldName } from "../../bridge";
  import { current, send } from "../../state/app.svelte";
  import { shownWork } from "../../state/audioWork.svelte";
  import { askWhole } from "../automation/coverIf";
  import { STATUS_JOIN, t } from "../../text/ja";
  import { mapOf } from "../../time/bars";
  import Button from "../Button.svelte";
  import { dbNumber, tempoText, textParts } from "../format";
  import { parseDb } from "../mixer/scale";
  import { stripName } from "../mixer/stripName";
  import { signed } from "../pianoroll/geom";
  import { positionText } from "../time";
  import { centsText, clipLengthLabel, fadeLengthText, parseClipLength, parseClipPosition, parseSourceBpm, parseWhole, pitchReading } from "./clipFields";
  import StretchNumber from "./StretchNumber.svelte";
  import ValueField from "./ValueField.svelte";

  let { sel }: { sel: SelectedClip } = $props();

  const app = $derived(current.state!);
  const busy = $derived(app.transport.state === "recording" || app.transport.state === "countIn");
  const clip = $derived(app.song.tracks.find((tr) => tr.id === sel.track)?.clips.find((c) => c.id === sel.clip) ?? null);
  const map = $derived(mapOf(app.timeline));
  const meterAtStart = $derived(clip ? map.meterAt(clip.start) : { num: app.timeline.meterNum, den: app.timeline.meterDen });

  const fileLine = $derived.by(() => {
    const audio = clip?.audio;
    if (!audio) return "";
    const parts = [stripName(app.song, { kind: "track", id: sel.track }, app.library), audio.file];
    parts.push(t(audio.resampled ? "clipParams.rateResampled" : "clipParams.rate", { rate: audio.sampleRate }));
    if (audio.bits !== null) parts.push(t("clipParams.bits", { bits: audio.bits }));
    return parts.join(STATUS_JOIN);
  });

  let folded = $state<string[]>([]);
  const toggle = (group: string) => (folded = folded.includes(group) ? folded.filter((g) => g !== group) : [...folded, group]);

  /** 打ちこんでいる欄（フェードは前と後で別の欄、音の高さは半音とセントで別の欄）。 */
  type TypedField = ClipField | "sourceBpm" | "semitones" | "cents";
  let typing = $state<TypedField | null>(null);

  function texts(c: ClipView): Record<ClipField, string> {
    const a = c.audio!;
    return {
      start: positionText(c.start, map),
      length: clipLengthLabel(c.end - c.start, meterAtStart),
      gainDb: t("common.db", { value: dbNumber(a.gainDb) }),
      fadeIn: fadeLengthText(a.fadeIn, meterAtStart),
      fadeOut: fadeLengthText(a.fadeOut, meterAtStart),
    };
  }

  const REJECT_FIELD: Partial<Record<ClipField, ValueFieldName>> = { start: "position", length: "length", fadeIn: "length", fadeOut: "length" };

  function submit(field: ClipField, text: string): void {
    typing = null;
    const c = clip;
    if (!c?.audio || busy) return;
    const value =
      field === "start"
        ? parseClipPosition(text, map)
        : field === "length"
          ? parseClipLength(text, meterAtStart)
          : field === "gainDb"
            ? (parseDb(text) ?? null)
            : parseClipLength(text, meterAtStart, true);
    if (value === null) {
      const reject = REJECT_FIELD[field];
      if (reject) send({ type: "rejectValue", field: reject, kept: texts(c)[field], range: null });
      return;
    }
    const now = field === "start" ? c.start : field === "length" ? c.end - c.start : c.audio[field];
    if (value !== now) send({ type: "setClipField", clip: c.id, field, value });
  }

  const startTyping = (field: TypedField) => {
    if (!busy) typing = field;
  };

  const stretch = $derived(clip?.audio?.stretch ?? null);
  const onOff = (on: boolean) => t(on ? "param.on" : "param.off");
  const bpmText = (bpm: number | null) => (bpm === null ? t("params.unknownValue") : tempoText(bpm));

  function setStretch(change: StretchChange): void {
    if (clip?.audio && !busy) send({ type: "setClipStretch", clip: clip.id, change });
  }

  /** 音の高さをドラッグしている間: 状態行の文とその場の読み値（仕様書 6 章「音の高さを動かしている」）。 */
  const pitchDragging = (semitones: number, cents: number) => ({
    status: t("msg.pitchDragging", { semitones: signed(semitones), cents: signed(cents) }),
    reading: pitchReading(semitones, cents),
  });

  function submitStretch(field: "sourceBpm" | "semitones" | "cents", text: string): void {
    typing = null;
    const s = stretch;
    if (!s || !clip?.audio || busy) return;
    if (field === "sourceBpm") {
      const bpm = parseSourceBpm(text);
      if (bpm === null) send({ type: "rejectValue", field: "tempo", kept: bpmText(s.sourceBpm), range: null });
      else if (bpm !== s.sourceBpm) setStretch({ kind: "sourceBpm", bpm });
      return;
    }
    // 読めない半音 ・ セントは何もしない（範囲の文の欄がない）。範囲の外は Rust がだまって中に入れる。
    const v = parseWhole(text);
    if (v === null) return;
    if (field === "semitones" && v !== s.semitones) setStretch({ kind: "semitones", semitones: v });
    if (field === "cents" && v !== s.cents) setStretch({ kind: "cents", cents: v });
  }

  /** このクリップの音を作っている進み（約束 24 の audioWork。clip がこのクリップの物。% はクリップの下 ・ 状態行と同じコマで変わる）。 */
  const rendering = $derived(shownWork.current.rendering.get(sel.clip) ?? null);

  /** % が変わったら、進みの行まるごとの四角を先に 1 つ頼む（棒と字と % の四角が、上下の端のちがう 3 つにならない）。 */
  function askRow(node: HTMLElement, percent: number) {
    let now = percent;
    return {
      update(next: number) {
        if (next !== now) askWhole(node);
        now = next;
      },
    };
  }
</script>

{#snippet value(field: ClipField, shown: string, label: string)}
  {#if typing === field}
    <ValueField initial={shown} {label} onsubmit={(text) => submit(field, text)} oncancel={() => (typing = null)} />
  {:else}
    <span class="m-num m-hi value" role="presentation" data-test="clip-value-{field}" ondblclick={() => startTyping(field)}>{shown}</span>
  {/if}
{/snippet}

<!-- オン ・ オフの行: 12×12 の四角の印（オンは枠と中の 6×6 が accent。params/ParamRow.svelte と同じ）と、値の字。 -->
{#snippet switchRow(kind: "follow" | "keepFormant", label: string, on: boolean)}
  <div class="row" data-test="clip-{kind}">
    <span class="m-lbl label">{label}</span>
    <button
      class="m-plain switch"
      class:on
      role="switch"
      aria-checked={on}
      aria-label={label}
      disabled={busy}
      data-test="clip-switch-{kind}"
      onclick={() => setStretch({ kind, on: !on })}><span></span></button
    >
    <span class="m-num m-hi value">{onOff(on)}</span>
  </div>
{/snippet}

{#if clip?.audio}
  {@const shown = texts(clip)}
  <div class="who" data-test="clip-params">
    <div class="line">
      <span class="m-name name" data-test="params-name">{clip.name}</span>
      <span class="m-lbl">{t("clipParams.selected")}</span>
    </div>
    <span class="m-lbl file" data-test="clip-file">{fileLine}</span>
  </div>
  {#if rendering && !sel.missing}
    <div class="rendering" data-test="clip-rendering" use:askRow={rendering.percent}>
      <span class="m-lbl">{t("clipParams.rendering")}</span>
      <div class="bar"><div class="fill" style:transform="scaleX({rendering.percent / 100})"></div></div>
      <!-- % の字は流れの外の箱: 字の幅が変わっても、行と欄はレイアウトしなおされない。幅は「99 %」の字で決める。 -->
      <span class="m-num m-hi percent" data-size={t("clipParams.renderingPercent", { percent: 99 })}
        ><span class="in">{t("clipParams.renderingPercent", { percent: rendering.percent })}</span></span
      >
    </div>
  {/if}
  {#if sel.missing}
    <div class="missing" data-test="clip-missing">
      <span>{t("params.audioFileMissing", { file: sel.missing.file, path: sel.missing.wasAt ?? "" })}</span>
      <div class="buttons">
        <Button test="clip-find" onclick={() => send({ type: "findAudio", audio: sel.missing!.audio })}>{t("button.findFile")}</Button>
        <Button test="clip-remove" disabled={busy} onclick={() => send({ type: "removeMissing", missing: { kind: "audio", audio: sel.missing!.audio } })}
          >{t("button.removeFromSong")}</Button
        >
      </div>
    </div>
  {:else}
    <div class="list">
      <button class="m-plain group m-focus-in" onclick={() => toggle("clip")} data-group="clip">
        <span class={folded.includes("clip") ? "m-lbl" : "m-hi"}>{folded.includes("clip") ? "▸" : "▾"}</span>
        <span class="m-hi group-name">{t("clipParams.groupClip")}</span>
        <span class="m-num m-lbl">4</span>
      </button>
      {#if !folded.includes("clip")}
        <div class="row"><span class="m-lbl label">{t("clipParams.position")}</span>{@render value("start", shown.start, t("clipParams.position"))}</div>
        <div class="row"><span class="m-lbl label">{t("clipParams.length")}</span>{@render value("length", shown.length, t("clipParams.length"))}</div>
        <div class="row"><span class="m-lbl label">{t("clipParams.gain")}</span>{@render value("gainDb", shown.gainDb, t("clipParams.gain"))}</div>
        <div class="row">
          <span class="m-lbl label">{t("clipParams.fade")}</span>
          {#if typing === "fadeIn" || typing === "fadeOut"}
            {@render value(typing, shown[typing], t("clipParams.fade"))}
          {:else}
            <!-- 「前 0.0480 ・ 後 0.0000」。前の数を 2 回押すとフェードイン、後の数でフェードアウトを打ちこむ。 -->
            <span class="m-num m-hi value" data-test="clip-value-fade"
              >{#each textParts("clipParams.fadeValue", { fadeIn: shown.fadeIn, fadeOut: shown.fadeOut }) as part, i (i)}{#if part.hole === "fadeIn" || part.hole === "fadeOut"}{@const edge = part.hole}<span
                    role="presentation"
                    data-test="clip-value-{edge}"
                    ondblclick={() => startTyping(edge)}>{part.text}</span
                  >{:else}{part.text}{/if}{/each}</span
            >
          {/if}
        </div>
      {/if}
      <button class="m-plain group m-focus-in" onclick={() => toggle("stretch")} data-group="stretch">
        <span class={folded.includes("stretch") ? "m-lbl" : "m-hi"}>{folded.includes("stretch") ? "▸" : "▾"}</span>
        <span class="m-hi group-name">{t("clipParams.groupStretch")}</span>
        <span class="m-num m-lbl">4</span>
      </button>
      {#if stretch && !folded.includes("stretch")}
        {@render switchRow("follow", t("clipParams.follow"), stretch.follow)}
        <div class="row" data-test="clip-source-tempo">
          <span class="m-lbl label">{t("clipParams.sourceTempo")}</span>
          {#if typing === "sourceBpm"}
            <ValueField
              initial={stretch.sourceBpm === null ? "" : tempoText(stretch.sourceBpm)}
              label={t("clipParams.sourceTempo")}
              onsubmit={(text) => submitStretch("sourceBpm", text)}
              oncancel={() => (typing = null)}
            />
          {:else}
            {#if stretch.sourceBpm === null}
              <Button test="clip-find-tempo" disabled={busy} onclick={() => send({ type: "findSourceTempo", clip: clip.id })}>{t("clipParams.findTempo")}</Button>
            {/if}
            <span class="m-num m-hi value" role="presentation" data-test="clip-value-sourceBpm" ondblclick={() => startTyping("sourceBpm")}
              >{bpmText(stretch.sourceBpm)}</span
            >
          {/if}
        </div>
        <div class="row" data-test="clip-pitch">
          <span class="m-lbl label">{t("clipParams.pitch")}</span>
          {#if typing === "semitones" || typing === "cents"}
            {@const field = typing}
            <ValueField
              initial={field === "semitones" ? signed(stretch.semitones) : centsText(stretch.cents)}
              label={t("clipParams.pitch")}
              onsubmit={(text) => submitStretch(field, text)}
              oncancel={() => (typing = null)}
            />
          {:else}
            <!-- 「+2 半音 0 セント」。半音とセントの数を、それぞれ上下にドラッグするか 2 回押して打ちこむ。 -->
            <span class="m-num m-hi value"
              >{#each textParts("clipParams.pitchValue", { semitones: "", cents: "" }) as part, i (i)}{#if part.hole === "semitones"}<StretchNumber
                    value={stretch.semitones}
                    min={-CLIP_SEMITONES_MAX}
                    max={CLIP_SEMITONES_MAX}
                    text={signed}
                    label={t("clipParams.pitch")}
                    test="clip-value-semitones"
                    disabled={busy}
                    reading={(v) => pitchDragging(v, stretch.cents)}
                    commit={(semitones) => setStretch({ kind: "semitones", semitones })}
                    ontype={() => startTyping("semitones")}
                  />{:else if part.hole === "cents"}<StretchNumber
                    value={stretch.cents}
                    min={-CLIP_CENTS_MAX}
                    max={CLIP_CENTS_MAX}
                    text={centsText}
                    label={t("clipParams.pitch")}
                    test="clip-value-cents"
                    disabled={busy}
                    reading={(v) => pitchDragging(stretch.semitones, v)}
                    commit={(cents) => setStretch({ kind: "cents", cents })}
                    ontype={() => startTyping("cents")}
                  />{:else}{part.text}{/if}{/each}</span
            >
          {/if}
        </div>
        {@render switchRow("keepFormant", t("clipParams.keepFormant"), stretch.keepFormant)}
      {/if}
    </div>
  {/if}
{/if}

<style>
  /* 名前とファイルの行（モック F7-AudioEdit: 上下 8 ・ 左右 12、すきま 4、下に gray.5 の線。ファイルの行は折り返す）。 */
  .who {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    white-space: nowrap;
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .file {
    white-space: normal;
  }
  /* 見つからない物の知らせ（仕様書 2 章: 左に gray.8 の line.mark、内 8 ・ 12）。 */
  .missing {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-left: var(--line-mark) solid var(--color-gray-8);
    border-bottom: var(--line-hair) solid var(--color-gray-4);
    white-space: normal;
  }
  .buttons {
    display: flex;
    gap: var(--space-2);
  }
  .list {
    min-height: 0;
    overflow-y: auto;
  }
  .group {
    width: 100%;
    height: var(--part-paramgroup-height);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    background: var(--color-gray-4);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    white-space: nowrap;
  }
  .group:hover {
    box-shadow: inset 0 0 0 var(--line-hair) var(--color-gray-8);
  }
  .group-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .row {
    height: var(--part-param-height);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-gray-4);
    white-space: nowrap;
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .value {
    flex-shrink: 0;
    text-align: right;
  }
  /* 12×12 の四角の印（params/ParamRow.svelte と同じ形）。 */
  .switch {
    flex-shrink: 0;
    box-sizing: border-box;
    width: var(--part-param-switch);
    height: var(--part-param-switch);
    display: flex;
    align-items: center;
    justify-content: center;
    border: var(--line-hair) solid var(--color-gray-8);
  }
  .switch span {
    width: var(--part-param-step);
    height: var(--part-param-step);
  }
  .switch.on {
    border-color: var(--color-accent);
  }
  .switch.on span {
    background: var(--color-accent);
  }
  /* 音を作っている進み（モック F7-Stretch: 高さ 24、左右 12、すきま 8、棒は地 gray.4 に accent。棒の太さは F7-Latency の棒と同じ）。 */
  .rendering {
    flex-shrink: 0;
    height: var(--part-param-height);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-gray-5);
    white-space: nowrap;
  }
  .bar {
    flex: 1;
    min-width: 0;
    height: var(--mock-latency-progress);
    background: var(--color-gray-4);
  }
  .fill {
    height: 100%;
    background: var(--color-accent);
    transform-origin: left;
  }
  .percent {
    position: relative;
    flex-shrink: 0;
  }
  .percent::before {
    content: attr(data-size);
    visibility: hidden;
  }
  .percent > .in {
    position: absolute;
    top: 0;
    right: 0;
    white-space: nowrap;
  }
</style>

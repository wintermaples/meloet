<!-- The chosen clip (spec 2 "Selected clip", spec 3 93-4 and Y22; mocks F7-AudioEdit, F7-AudioMissing). The content of the params panel.
     At the top, the name and 「選んだクリップ」, and the file line (「6 歌 ・ 歌 テイク 1.wav ・ 48000 Hz ・ 24 ビット」; with a different
     sample rate, 「44100 Hz（曲に合わせて鳴らします）」). The group 「クリップ」 (position, length, gain, fade). Pressing a value twice gives
     the typing box (Enter sends one SetClipField; Rust quietly brings values out of range inside; B-94-29).
     The group 「のび縮み ・ 音の高さ」 (spec 3 "Stretch", "Pitch"; mock F7-Stretch): on/off is a 12×12 square mark (the same as the
     parameter row) that sends SetClipStretch. The source tempo is typed after pressing twice; when it is 「—」, 「さがす」
     (FindSourceTempo). Pitch has semitones and cents apart, dragged up and down (one step on release) or typed after pressing
     twice. 「音程を寄せる」 (round 12, M7): the strength in percent, dragged or typed like the pitch, and under it the scale
     the clip is brought toward (its own, or at strength 0 the song's) with 「いまのスケールにする」. Both rows are shown
     disabled when `tuneUsable` is false.
     While the sound is being made, a progress bar above the panel (`rendering` of audioWork).
     When the audio file is not found, the notice of missing things (「さがす…」「曲から外す」) instead of the groups. -->
<script lang="ts">
  import { tick, untrack } from "svelte";
  import {
    CLIP_CENTS_MAX,
    CLIP_SEMITONES_MAX,
    TUNE_STRENGTH_MAX,
    type ClipField,
    type ClipView,
    type SelectedClip,
    type StretchChange,
    type ValueField as ValueFieldName,
  } from "../../bridge";
  import { current, send } from "../../state/app.svelte";
  import { shownWork } from "../../state/audioWork.svelte";
  import { askWhole } from "../automation/coverIf";
  import { clipEdit, renameCommand, startClipRename, stopClipRename } from "../clipName.svelte";
  import { tip } from "../../lib/tip.svelte";
  import { STATUS_JOIN, t } from "../../text";
  import { mapOf } from "../../time/bars";
  import Button from "../Button.svelte";
  import { dbNumber, scaleName, tempoText, textParts } from "../format";
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

  // ---- The name, typed in place (mock P10-ClipName (b)) ----
  const naming = $derived(clip !== null && clipEdit.place === "params" && clipEdit.renaming === clip.id);
  let nameDraft = $state("");
  let nameField = $state<HTMLInputElement>();
  $effect(() => {
    if (!naming) return;
    if (busy) {
      stopClipRename();
      return;
    }
    nameDraft = untrack(() => clip!.name);
    void tick().then(() => {
      nameField?.focus();
      nameField?.select();
    });
  });
  function decideName(): void {
    if (!clip) return;
    stopClipRename();
    const command = renameCommand(clip.id, clip.name, nameDraft);
    if (command) send(command);
  }
  function nameKey(e: KeyboardEvent): void {
    if (e.isComposing || !clip) return;
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      decideName();
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      stopClipRename();
    }
  }

  let folded = $state<string[]>([]);
  const toggle = (group: string) => (folded = folded.includes(group) ? folded.filter((g) => g !== group) : [...folded, group]);

  /** The field being typed (fade in and fade out are separate fields; so are semitones and cents). */
  type TypedField = ClipField | "sourceBpm" | "semitones" | "cents" | "tune";
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

  /** While dragging the pitch: the status line text and the reading in place (spec 6 "Moving the pitch"). */
  const pitchDragging = (semitones: number, cents: number) => ({
    status: t("msg.pitchDragging", { semitones: signed(semitones), cents: signed(cents) }),
    reading: pitchReading(semitones, cents),
  });

  // ---- 「音程を寄せる」 (round 12, M7) ----
  const tuneStrength = $derived(stretch?.tune?.strength ?? 0);
  /** The scale the clip keeps; at strength 0 it keeps none, and the song's is where it would go. */
  const tuneScale = $derived(stretch?.tune ?? app.pianoRoll.scale);
  const tuneOff = $derived(busy || !sel.tuneUsable);
  const sameScale = $derived(tuneScale.root === app.pianoRoll.scale.root && tuneScale.mode === app.pianoRoll.scale.mode);
  const percentText = (v: number) => t("param.value.percent", { value: v });
  const tuneDragging = (v: number) => ({ status: t("msg.tuneDragging", { percent: v }), reading: percentText(v) });
  function setTune(percent: number): void {
    if (!tuneOff && percent !== tuneStrength) setStretch({ kind: "tuneStrength", percent });
  }
  function submitTune(text: string): void {
    typing = null;
    // 「60」 and 「60 %」. What cannot be read does nothing; Rust quietly brings values out of range inside.
    const v = parseWhole(text.replace(/[%％]\s*$/, ""));
    if (v !== null) setTune(Math.min(TUNE_STRENGTH_MAX, Math.max(0, v)));
  }

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
    // Semitones or cents that cannot be read do nothing (there is no range text for them). Rust quietly brings values out of range inside.
    const v = parseWhole(text);
    if (v === null) return;
    if (field === "semitones" && v !== s.semitones) setStretch({ kind: "semitones", semitones: v });
    if (field === "cents" && v !== s.cents) setStretch({ kind: "cents", cents: v });
  }

  /** The render progress of this clip (`audioWork` of protocol 24, the job whose clip is this one; the % changes in the same frame as under the clip and in the status line). */
  const rendering = $derived(shownWork.current.rendering.get(sel.clip) ?? null);

  /** When the % changes, first ask for one rectangle over the whole progress row (so the bar, the text and the % are not three rectangles with different tops and bottoms). */
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

<!-- An on/off row: the 12×12 square mark (when on, the border and the 6×6 inside are accent, as in params/ParamRow.svelte) and the value text. -->
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
      {#if naming}
        <input
          class="name-field"
          type="text"
          spellcheck="false"
          autocomplete="off"
          aria-label={t("menu.renameClip")}
          data-test="params-name-field"
          bind:this={nameField}
          bind:value={nameDraft}
          onkeydown={nameKey}
          onblur={() => naming && decideName()}
        />
      {:else}
        <button class="m-plain m-name name" class:live={!busy} disabled={busy} data-test="params-name" onclick={() => clip && startClipRename(clip.id, "params")}
          >{clip.name}</button
        >
        <span class="m-lbl">{t("clipParams.selected")}</span>
      {/if}
    </div>
    <span class="m-lbl file" data-test="clip-file">{fileLine}</span>
  </div>
  {#if rendering && !sel.missing}
    <div class="rendering" data-test="clip-rendering" use:askRow={rendering.percent}>
      <span class="m-lbl">{t("clipParams.rendering")}</span>
      <div class="bar"><div class="fill" style:transform="scaleX({rendering.percent / 100})"></div></div>
      <!-- The % text is in a box out of the flow, so a change in its width does not lay out the row and the panel again. Its width comes from the text 「99 %」. -->
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
            <!-- 「前 0.0480 ・ 後 0.0000」. Pressing the first number twice types the fade-in, the second the fade-out. -->
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
        <span class="m-num m-lbl">5</span>
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
            <!-- 「+2 半音 0 セント」. The semitones and cents numbers are each dragged up and down, or typed after pressing twice. -->
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
        <div class="row" class:unusable={!sel.tuneUsable} data-test="clip-tune">
          <span class="m-lbl label" use:tip={t("clipParams.tuneTip")}>{t("clipParams.tune")}</span>
          {#if typing === "tune"}
            <ValueField initial={String(tuneStrength)} label={t("clipParams.tune")} onsubmit={submitTune} oncancel={() => (typing = null)} />
          {:else}
            <span class="m-num m-hi value"
              ><StretchNumber
                value={tuneStrength}
                min={0}
                max={TUNE_STRENGTH_MAX}
                text={percentText}
                label={t("clipParams.tune")}
                test="clip-value-tune"
                disabled={tuneOff}
                step="line.mark"
                reading={tuneDragging}
                commit={setTune}
                ontype={() => !tuneOff && startTyping("tune")}
              /></span
            >
          {/if}
        </div>
        <div class="row" class:unusable={!sel.tuneUsable} data-test="clip-tune-scale">
          <span class="m-lbl label" data-test="clip-tune-scale-name">{scaleName(tuneScale.root, tuneScale.mode)}</span>
          <Button test="clip-tune-to-scale" disabled={tuneOff || tuneStrength === 0 || sameScale} onclick={() => setStretch({ kind: "tuneScale" })}
            >{t("clipParams.tuneToScale")}</Button
          >
        </div>
      {/if}
    </div>
  {/if}
{/if}

<style>
  /* The name and file lines (mock F7-AudioEdit: 8 above and below, 12 left and right, gap 4, a line.thin line below; the file line wraps). */
  .who {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
    white-space: nowrap;
  }
  /* The name and its box sit one space.1 ＋ line.hair further left, so the text stays in line with the file line (P10-ClipName (b)). */
  .name,
  .name-field {
    box-sizing: border-box;
    height: var(--part-input-height);
    margin-left: calc(-1 * (var(--space-1) + var(--line-hair)));
    padding: 0 var(--space-1);
    border: var(--line-hair) solid transparent;
    border-radius: var(--radius-1);
  }
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: default;
  }
  .name.live:hover {
    border-color: var(--color-gray-8);
    cursor: text;
  }
  .name-field {
    flex: 1;
    min-width: 0;
    background: var(--color-gray-0);
    border-color: var(--color-gray-10);
    color: var(--color-gray-10);
    font: inherit;
    font-weight: var(--text-s-weight-600);
    outline: none;
  }
  .file {
    white-space: normal;
  }
  /* The notice of missing things (spec 2: a gray.8 line.mark on the left, padding 8 and 12). */
  .missing {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-3);
    border-left: var(--line-mark) solid var(--color-gray-8);
    border-bottom: var(--line-hair) solid var(--color-line-row);
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
    border-bottom: var(--line-hair) solid var(--color-line-thin);
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
    border-bottom: var(--line-hair) solid var(--color-line-row);
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
  /* Disabled (spec 2): text gray.7, cannot be pressed. */
  .row.unusable .label,
  .row.unusable .value {
    color: var(--color-gray-7);
  }
  .row.unusable .value :global(.drag) {
    cursor: default;
  }
  /* The 12×12 square mark (the same form as params/ParamRow.svelte). */
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
  /* Render progress (mock F7-Stretch: height 24, 12 left and right, gap 8, the bar accent on gray.4; as thick as the bar in F7-Latency). */
  .rendering {
    flex-shrink: 0;
    height: var(--part-param-height);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
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

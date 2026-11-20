<!-- 選んだクリップ（仕様書 2 章「選んだクリップ」、3 章 93-4 ・ Y22、モック F7-AudioEdit ・ F7-AudioMissing）。パラメータの欄の中身。
     上に名前と「選んだクリップ」、ファイルの行（「6 歌 ・ 歌 テイク 1.wav ・ 48000 Hz ・ 24 ビット」。サンプルレートがちがえば「44100 Hz（曲に合わせて鳴らします）」）。
     グループ「クリップ」（位置 ・ 長さ ・ 音量 ・ フェード）。値を 2 回押すと打ちこむ枠（Enter で SetClipField を 1 つ。範囲の外は Rust がだまって中に入れる。B-94-29）。
     グループ「のび縮み ・ 音の高さ」は、のび縮みを作るまで「使えない」（文字 gray.7）。値は曲が持っている物をそのまま見せる。
     音のファイルが見つからないときは、グループのかわりに見つからない物の知らせ（「さがす…」「曲から外す」）。 -->
<script lang="ts">
  import type { ClipField, ClipView, SelectedClip, ValueField as ValueFieldName } from "../../bridge";
  import { current, send } from "../../state/app.svelte";
  import { STATUS_JOIN, t } from "../../text/ja";
  import { mapOf } from "../../time/bars";
  import Button from "../Button.svelte";
  import { dbNumber, tempoText, textParts } from "../format";
  import { parseDb } from "../mixer/scale";
  import { stripName } from "../mixer/stripName";
  import { signed } from "../pianoroll/geom";
  import { positionText } from "../time";
  import { clipLengthLabel, fadeLengthText, parseClipLength, parseClipPosition } from "./clipFields";
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

  /** 打ちこんでいる欄（フェードは前と後で別の欄）。 */
  let typing = $state<ClipField | null>(null);

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

  const startTyping = (field: ClipField) => {
    if (!busy) typing = field;
  };

  const stretch = $derived(clip?.audio?.stretch ?? null);
  const onOff = (on: boolean) => t(on ? "param.on" : "param.off");
  const cents = (v: number) => (v < 0 ? `−${-v}` : String(v));
</script>

{#snippet value(field: ClipField, shown: string, label: string)}
  {#if typing === field}
    <ValueField initial={shown} {label} onsubmit={(text) => submit(field, text)} oncancel={() => (typing = null)} />
  {:else}
    <span class="m-num m-hi value" role="presentation" data-test="clip-value-{field}" ondblclick={() => startTyping(field)}>{shown}</span>
  {/if}
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
        <div class="row unusable" aria-disabled="true" data-test="clip-stretch">
          <span class="label">{t("clipParams.follow")}</span><span class="m-num value">{onOff(stretch.follow)}</span>
        </div>
        <div class="row unusable" aria-disabled="true">
          <span class="label">{t("clipParams.sourceTempo")}</span><span class="m-num value"
            >{stretch.sourceBpm === null ? t("params.unknownValue") : tempoText(stretch.sourceBpm)}</span
          >
        </div>
        <div class="row unusable" aria-disabled="true">
          <span class="label">{t("clipParams.pitch")}</span><span class="m-num value"
            >{t("clipParams.pitchValue", { semitones: signed(stretch.semitones), cents: cents(stretch.cents) })}</span
          >
        </div>
        <div class="row unusable" aria-disabled="true">
          <span class="label">{t("clipParams.keepFormant")}</span><span class="m-num value">{onOff(stretch.keepFormant)}</span>
        </div>
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
  /* まだ中身のない行は「使えない」（文字 gray.7）。 */
  .row.unusable {
    color: var(--color-gray-7);
  }
</style>

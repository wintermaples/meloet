<!-- The params panel when two or more audio clips are chosen (spec 3 40-A; mocks F8-MultiClip, Opt8-MultiClip; ParamsView.clips of protocol 27).
     At the top, 「3 クリップを選んでいます」 and the track names (「6 歌 ・ 7 歌 2」). The group 「まとめて変える」 has gain, follow the song
     tempo, pitch, and keep the voice character. A field whose values differ shows 「—」. A typed value (press twice for the typing
     box, Enter) and a pressed on/off go to all chosen clips (SetClipGains, SetClipsStretch; each one step). Pitch has semitones
     and cents apart (the same form as the panel for one clip). At the bottom, 「位置 ・ 長さ ・ フェード ・ この音のテンポは、1 つだけ選ぶと出ます。」. -->
<script lang="ts">
  import { CLIP_CENTS_MAX, CLIP_SEMITONES_MAX, type SelectedClips, type StretchChange } from "../../bridge";
  import { current, send } from "../../state/app.svelte";
  import { STATUS_JOIN, t } from "../../text";
  import { dbNumber, textParts } from "../format";
  import { parseDb } from "../mixer/scale";
  import { stripName } from "../mixer/stripName";
  import { signed } from "../pianoroll/geom";
  import { centsText, parseWhole, pitchReading } from "./clipFields";
  import StretchNumber from "./StretchNumber.svelte";
  import ValueField from "./ValueField.svelte";

  let { sel }: { sel: SelectedClips } = $props();

  const app = $derived(current.state!);
  const busy = $derived(app.transport.state === "recording" || app.transport.state === "countIn");
  const tracksLine = $derived(sel.tracks.map((id) => stripName(app.song, { kind: "track", id }, app.library)).join(STATUS_JOIN));
  const dash = t("params.unknownValue");

  let folded = $state(false);
  type Typed = "gainDb" | "semitones" | "cents";
  let typing = $state<Typed | null>(null);

  const gainText = (db: number | null) => (db === null ? dash : t("common.db", { value: dbNumber(db) }));
  const onOff = (on: boolean | null) => (on === null ? dash : t(on ? "param.on" : "param.off"));

  const startTyping = (field: Typed) => {
    if (!busy) typing = field;
  };

  function setStretch(change: StretchChange): void {
    if (!busy) send({ type: "setClipsStretch", clips: sel.clips, change });
  }

  function submit(field: Typed, text: string): void {
    typing = null;
    if (busy) return;
    if (field === "gainDb") {
      // 「−∞」 and text that cannot be read do nothing (the same as the panel for one clip). Rust quietly brings values out of range inside.
      const db = parseDb(text);
      if (typeof db === "number" && db !== sel.gainDb) send({ type: "setClipGains", clips: sel.clips, db });
      return;
    }
    const v = parseWhole(text);
    if (v === null) return;
    if (field === "semitones" && v !== sel.semitones) setStretch({ kind: "semitones", semitones: v });
    if (field === "cents" && v !== sel.cents) setStretch({ kind: "cents", cents: v });
  }

  /** While dragging the pitch: the status line text and the reading in place (spec 6 "Moving the pitch"). When the other one differs, it reads as 0. */
  const pitchDragging = (semitones: number, cents: number) => ({
    status: t("msg.pitchDragging", { semitones: signed(semitones), cents: signed(cents) }),
    reading: pitchReading(semitones, cents),
  });
</script>

<!-- An on/off row. When they differ (null), the mark is empty and 「—」. Pressing turns it on (off if all are on). -->
{#snippet switchRow(kind: "follow" | "keepFormant", label: string, on: boolean | null)}
  <div class="row" data-test="clips-{kind}">
    <span class="m-lbl label">{label}</span>
    <button
      class="m-plain switch"
      class:on={on === true}
      role="switch"
      aria-checked={on === null ? "mixed" : on}
      aria-label={label}
      disabled={busy}
      data-test="clips-switch-{kind}"
      onclick={() => setStretch({ kind, on: on !== true })}><span></span></button
    >
    <span class="m-num m-hi value" data-test="clips-value-{kind}">{onOff(on)}</span>
  </div>
{/snippet}

{#snippet pitchPart(field: "semitones" | "cents")}
  {@const v = field === "semitones" ? sel.semitones : sel.cents}
  {#if v === null}
    <span role="presentation" data-test="clips-value-{field}" ondblclick={() => startTyping(field)}>{dash}</span>
  {:else if field === "semitones"}
    <StretchNumber
      value={v}
      min={-CLIP_SEMITONES_MAX}
      max={CLIP_SEMITONES_MAX}
      text={signed}
      label={t("clipParams.pitch")}
      test="clips-value-semitones"
      disabled={busy}
      reading={(s) => pitchDragging(s, sel.cents ?? 0)}
      commit={(semitones) => setStretch({ kind: "semitones", semitones })}
      ontype={() => startTyping("semitones")}
    />
  {:else}
    <StretchNumber
      value={v}
      min={-CLIP_CENTS_MAX}
      max={CLIP_CENTS_MAX}
      text={centsText}
      label={t("clipParams.pitch")}
      test="clips-value-cents"
      disabled={busy}
      reading={(c) => pitchDragging(sel.semitones ?? 0, c)}
      commit={(cents) => setStretch({ kind: "cents", cents })}
      ontype={() => startTyping("cents")}
    />
  {/if}
{/snippet}

<div class="who" data-test="clips-params">
  <span class="m-name name" data-test="params-name">{t("clips.selected", { count: sel.clips.length })}</span>
  <span class="m-lbl tracks" data-test="clips-tracks">{tracksLine}</span>
</div>
<div class="list">
  <button class="m-plain group m-focus-in" onclick={() => (folded = !folded)} data-group="clips">
    <span class={folded ? "m-lbl" : "m-hi"}>{folded ? "▸" : "▾"}</span>
    <span class="m-hi group-name">{t("clips.groupAll")}</span>
    <span class="m-num m-lbl">4</span>
  </button>
  {#if !folded}
    <div class="row" data-test="clips-gain">
      <span class="m-lbl label">{t("clipParams.gain")}</span>
      {#if typing === "gainDb"}
        <ValueField
          initial={sel.gainDb === null ? "" : dbNumber(sel.gainDb)}
          label={t("clipParams.gain")}
          onsubmit={(text) => submit("gainDb", text)}
          oncancel={() => (typing = null)}
        />
      {:else}
        <span class="m-num m-hi value" role="presentation" data-test="clips-value-gainDb" ondblclick={() => startTyping("gainDb")}>{gainText(sel.gainDb)}</span>
      {/if}
    </div>
    {@render switchRow("follow", t("clipParams.follow"), sel.follow)}
    <div class="row" data-test="clips-pitch">
      <span class="m-lbl label">{t("clipParams.pitch")}</span>
      {#if typing === "semitones" || typing === "cents"}
        {@const field = typing}
        {@const now = field === "semitones" ? sel.semitones : sel.cents}
        <ValueField
          initial={now === null ? "" : field === "semitones" ? signed(now) : centsText(now)}
          label={t("clipParams.pitch")}
          onsubmit={(text) => submit(field, text)}
          oncancel={() => (typing = null)}
        />
      {:else if sel.semitones === null && sel.cents === null}
        <!-- Both semitones and cents differ: only 「—」 (mock F8-MultiClip). Pressing twice types the semitones. -->
        <span class="m-num m-hi value" role="presentation" data-test="clips-value-pitch" ondblclick={() => startTyping("semitones")}>{dash}</span>
      {:else}
        <span class="m-num m-hi value" data-test="clips-value-pitch"
          >{#each textParts("clipParams.pitchValue", { semitones: "", cents: "" }) as part, i (i)}{#if part.hole === "semitones" || part.hole === "cents"}{@render pitchPart(
                part.hole,
              )}{:else}{part.text}{/if}{/each}</span
        >
      {/if}
    </div>
    {@render switchRow("keepFormant", t("clipParams.keepFormant"), sel.keepFormant)}
  {/if}
  <div class="only-one m-lbl" data-test="clips-only-one">{t("clips.onlyOne")}</div>
</div>

<style>
  /* 「3 クリップを選んでいます」 and the track names (mock F8-MultiClip: 8 above and below, 12 left and right, gap 4, a line.thin line below). */
  .who {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-3);
    border-bottom: var(--line-hair) solid var(--color-line-thin);
    white-space: nowrap;
  }
  .name,
  .tracks {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
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
  /* The 12×12 square mark (the same form as ClipPanel and ParamRow). */
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
    border-color: var(--color-accent-ink);
  }
  .switch.on span {
    background: var(--color-accent-ink);
  }
  .only-one {
    padding: var(--space-2) var(--space-3);
    white-space: normal;
  }
</style>

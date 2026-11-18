<!-- 状態行（モック F-Connect1〜3・F-BufferFixed・F-Zoom のいちばん下の行）。 -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { clockOffsetUs, keyIsDown, type Band, type Command, type NoteSelection, type StatusMessage, type TrackRef, type ValueField } from "../bridge";
  import type { Scale } from "../design/tokens";
  import { everyGlFrame } from "../gl/slot";
  import { platformOf } from "../keys/keys";
  import { menuOpened } from "../lib/menu";
  import { term } from "../lib/tip.svelte";
  import { current, onEvent, send } from "../state/app.svelte";
  import { STATUS_JOIN, t, type TextKey } from "../text/ja";
  import { gridText, msText, noteLengthText, pitchClassName, scaleName, tempoText, termParts, textParts, type TextPart } from "./format";
  import { stripName } from "./mixer/stripName";
  import { paramName } from "./params/paramRows";
  import { isDrumKit, keyLabel, keysInstrument, keysLabel, rollShowsDrums } from "./pianoroll/drums";
  import { indexesOf } from "./pianoroll/geom";
  import { shown as shownNotes } from "./pianoroll/shown.svelte";
  import { sameKeys, soundingKeys } from "./pianoroll/sounding";
  import { dragReading } from "./pianoroll/view.svelte";
  import { statusFit, type StatusRightShown } from "./statusFit";
  import { shownTick } from "./time";
  import ZoomMenu from "./ZoomMenu.svelte";

  const app = $derived(current.state!);
  /** ピアノロールがドラムの行を見せている（選んだノートと鳴っている音を楽器の名前で）。 */
  const rollDrums = $derived(rollShowsDrums(app, shownNotes.clip, shownNotes.track));
  /** 鍵を弾くと鳴る先がドラム（押している音を楽器の名前で）。 */
  const keysDrums = $derived(isDrumKit(keysInstrument(app)));

  interface Shown {
    parts: TextPart[];
    /** 値を gray.10 で出す穴。 */
    strong: string[];
    button: { label: string; command: Command } | null;
    /** 文の中で用語の説明をそえる言葉。 */
    term?: string;
  }

  const RECOVERY_TERM = "なおすためのファイル";
  const platform = platformOf(navigator);
  const modKey = (letter: string) => `${platform === "mac" ? t("key.cmd") : t("key.ctrl")} ${letter}`;

  const BAND_NAMES: Record<Band, TextKey> = {
    pianoRoll: "band.pianoRoll",
    mixer: "band.mixer",
    history: "band.history",
    shelf: "band.shelf",
    params: "band.params",
  };

  /** 数字の欄に打った値の、欄の名前と範囲。 */
  // 範囲がクリップで変わる欄（位置・長さ）は、範囲の文が知らせについてくる（`range`）。
  const VALUE_FIELDS: Record<ValueField, [TextKey, TextKey | null]> = {
    tempo: ["value.tempo", "value.tempoRange"],
    meterNum: ["value.meterNum", "value.meterNumRange"],
    meterDen: ["value.meterDen", "value.meterDenRange"],
    velocity: ["value.velocity", "value.velocityRange"],
    position: ["note.position", null],
    key: ["note.key", null],
    length: ["note.length", null],
    playPosition: ["note.position", null],
    bar: ["value.bar", null],
    beat: ["value.beat", null],
    tick: ["value.tick", null],
    controlValue: ["ccList.value", null],
  };

  const track = (r: TrackRef) => t("track.label", { number: r.number, name: r.name });

  /** カウントイン中の「あと {beats} 拍」。計器がカウントインを言っていれば、その残りの拍。 */
  function beatsLeft(fallback: number): number {
    const p = current.telemetry?.position;
    return p?.state === "countIn" ? p.countInBeatsLeft : fallback;
  }

  /** 文の部分どうしを「 ・ 」でつなぐ（仕様書 6 章「止まっているときの状態行の左」）。 */
  function joined(...pieces: TextPart[][]): TextPart[] {
    return pieces.filter((p) => p.length > 0).flatMap((p, i) => (i === 0 ? p : [{ text: STATUS_JOIN, hole: null }, ...p]));
  }

  /** 「選択 3 ノート ・ D4 E4 F4 ・ クオンタイズ済み（もとの位置へ戻せます）」（モック A-PianoRoll）。音名は Rust が送る最初の 4 つで、残りがあれば「…」。 */
  function notesParts(n: NoteSelection): TextPart[] {
    const values = { count: n.count, keys: n.keys.map((k) => keyLabel(k, rollDrums)).join(" ") };
    return joined(
      textParts(n.count > n.keys.length ? "status.notesSelectedMore" : "status.notesSelected", values),
      n.quantized ? textParts("status.notesQuantized", {}) : [],
    );
  }

  function shown(m: StatusMessage | null): Shown | null {
    const plain = (key: TextKey, values: Record<string, string | number> = {}): Shown => ({
      parts: textParts(key, values),
      strong: [],
      button: null,
    });
    const settings = (advanced: boolean, label: string) => ({ label, command: { type: "openSettings", advanced } as Command });
    switch (m?.id) {
      case "searchingKeyboard":
        return plain("status.searchingKeyboard");
      case "keyboardFound":
        return plain("msg.keyboardFound", { keyboard: m.keyboard });
      case "keyboardLost":
        return plain("msg.keyboardLost", { keyboard: m.keyboard });
      case "keyboardInputStopped":
        return plain("msg.keyboardInputStopped");
      case "audioSystemStopped":
        return plain("msg.audioSystemStopped");
      case "recordingStoppedAudioSystem":
        return plain("msg.recordingStoppedAudioSystem", { take: m.take });
      case "recordingStoppedOutputMissing":
        return {
          ...plain("msg.recordingStoppedOutputMissing", { output: m.output, take: m.take }),
          button: settings(false, t("button.openAudioSettings")),
        };
      case "outputNotPlugged":
        return { ...plain("msg.outputNotPlugged", { output: m.output }), button: settings(false, t("button.openAudioSettings")) };
      case "outputMissing":
        return { ...plain("msg.outputLost", { output: m.output }), button: settings(false, t("button.openAudioSettings")) };
      case "bufferFixed":
        return {
          parts: textParts("msg.bufferFixedStatus", {
            actual: m.actual,
            requested: m.requested,
            latencyMs: msText(m.latencyMs),
            latencyMsAtRequested: msText(m.latencyMsAtRequested),
          }),
          strong: ["actual", "latencyMs"],
          button: settings(true, t("button.details")),
        };
      case "settingsOpen":
        return plain("status.settingsOpen");
      case "scaleDoesNotFit":
        return plain("msg.scaleKept", { requested: m.requested, kept: m.kept });
      case "scaleWindowGrown":
        return plain("msg.scaleWindowGrown", { scale: m.scale });
      case "scaleLoweredForScreen":
        return plain("msg.scaleLowered", { from: m.from, to: m.to });
      case "bandFoldedWindowLow":
        return plain("msg.bandFoldedWindowLow", { band: t(BAND_NAMES[m.band]) });
      case "bandFoldedToOpen":
        return plain("msg.bandFoldedToOpen", { opened: t(BAND_NAMES[m.opened]), folded: t(BAND_NAMES[m.folded]) });
      case "bandFoldedThird":
        return plain("msg.bandFoldedThird", { opened: t(BAND_NAMES[m.opened]), folded: t(BAND_NAMES[m.folded]) });
      case "bandFoldedScale":
        return plain("msg.bandFoldedScale", { scale: m.scale, band: t(BAND_NAMES[m.band]) });
      case "valueRejected": {
        if (m.field === "playPosition" || m.field === "position") return plain("msg.playPositionRejected", { kept: m.kept });
        const [field] = VALUE_FIELDS[m.field];
        if (m.field === "key") return plain("msg.keyRejected", { field: t(field), range: m.range ?? "", kept: m.kept });
        return plain("msg.valueRejected", { field: t(field), kept: m.kept });
      }
      case "valueClamped": {
        if (m.field === "bar") return plain("msg.valueClampedBar", { value: m.value });
        const [field, range] = VALUE_FIELDS[m.field];
        return plain("msg.valueClamped", { field: t(field), range: m.range ?? (range ? t(range) : ""), value: m.value });
      }
      case "velocityMatched":
        return plain("msg.velocityMatched", { count: m.count, value: m.value });
      case "lengthMatched":
        return plain("msg.lengthMatched", { count: m.count, value: noteLengthText(m.value, { num: app.song.meterNum, den: app.song.meterDen }) });
      case "clipsNotMoved":
        return plain("msg.clipsNotMoved");
      case "clipsNotPasted":
        return plain("msg.clipsNotPasted");
      case "clipsNotDuplicated":
        return plain("msg.clipsNotDuplicated");
      case "notesNotWritten":
        return plain("msg.notesNotWritten");
      case "notesNotResized":
        return plain("msg.notesNotResized");
      case "outputChosen":
        return m.track
          ? plain("status.outputChosen", { output: m.output, track: track(m.track) })
          : plain("status.outputChosenOnly", { output: m.output });
      case "newSong": {
        const values = { tempo: tempoText(m.tempoBpm), signature: `${m.meterNum}/${m.meterDen}` };
        return m.track ? plain("status.newSong", { ...values, track: track(m.track) }) : plain("status.newSongOnly", values);
      }
      case "countingIn":
        return plain("msg.countingInRest", { beats: beatsLeft(m.beats) });
      case "recording":
        return plain("msg.recording", { track: track(m.track), take: m.take });
      case "loopRecording":
        return plain("msg.loopRecording", { track: track(m.track), from: m.fromBar, to: m.toBar, lap: m.lap });
      case "takeSwitched":
        return plain("msg.takeSwitched", { track: track(m.track), clip: m.clip, take: m.take, takes: m.takes });
      case "noArmedTrack":
        return plain("msg.noArmedTrack");
      case "armMoved":
        return plain("status.armMoved", { recordKey: t("key.record"), track: track(m.track) });
      case "trackAdded":
        return plain("msg.trackAdded", { track: track(m.track) });
      case "trackDeleted":
        return plain("msg.trackDeleted", { track: track(m.track), undoKey: modKey(t("key.undo")) });
      case "instrumentReplaced":
        return plain("msg.instrumentReplaced", {
          track: track(m.track),
          instrument: t(`instrument.${m.instrument}`),
          undoKey: modKey(t("key.undo")),
        });
      case "auditionNewTrack":
        return plain("status.auditionNewTrack", { instrument: t(`instrument.${m.instrument}`), key: t("key.enter") });
      case "auditionReplace":
        return plain("status.auditionReplace", {
          instrument: t(`instrument.${m.instrument}`),
          track: track(m.track),
          current: t(`instrument.${m.current}`),
          key: t("key.enter"),
        });
      case "samplePlaying":
        return plain("msg.samplePlaying", { name: t(`instrument.${m.instrument}`) });
      case "instrumentLoading":
        return plain("msg.auditionLoading", { name: t(`instrument.${m.instrument}`) });
      case "undoPending":
        return plain("msg.undoPending", { count: m.count });
      case "recovered":
        return { ...plain("status.recovered"), term: RECOVERY_TERM };
      case "recoveryDiscarded":
        return plain("msg.recoveryDiscarded", { undoKey: modKey(t("key.undo")) });
      case "recoveryUnreadable":
        return {
          ...plain("status.recoveryUnreadable", {
            opened: t(m.savedSong ? "status.recoveryOpenedSaved" : "status.recoveryOpenedNew"),
          }),
          term: RECOVERY_TERM,
        };
      case "quantized": {
        // 「もとの位置へ戻す」は、いま選んでいるノート（クオンタイズしたノートが選ばれている。なければ全部）。
        const take = app.pianoRoll.take;
        return {
          ...plain("msg.quantized", { grid: gridText(m.division, m.triplet), strength: m.strength, count: m.count }),
          button:
            take === null
              ? null
              : { label: t("button.restorePosition"), command: { type: "unquantize", take, notes: indexesOf(app.pianoRoll.selectedNotes) } },
        };
      }
      case "outOfScale":
        return plain("msg.outOfScale", { key: pitchClassName(m.key), scale: scaleName(m.root, m.mode) });
      case "paramSelected": {
        const loop = m.loopBars;
        const selected = textParts("status.paramSelected", { strip: stripName(app.song, m.insert.strip), param: paramName(m.effect, m.param) });
        return { parts: joined(loop ? textParts("status.loop", { from: loop.from, to: loop.to }) : [], selected), strong: [], button: null };
      }
      case "summary": {
        const loop = m.loopBars;
        if (m.notes) {
          const loopParts = loop ? textParts("status.loop", { from: loop.from, to: loop.to }) : [];
          return { parts: joined(loopParts, notesParts(m.notes)), strong: [], button: null };
        }
        if (loop && m.nothingSelected) return plain("status.loopNoSelection", { from: loop.from, to: loop.to });
        if (loop) return plain("status.loop", { from: loop.from, to: loop.to });
        if (m.nothingSelected) return plain("status.noSelection");
        return null;
      }
      default:
        return null;
    }
  }

  // ---- ピアノロールをひらいて再生しているとき（モック F-PRPlay）: 「1 ピアノ ・ Aメロ ・ テイク 2 / 3 ・ 鳴っている音 C3 E3 G3 C4」 ----

  const pianoRollOpen = $derived(app.workspace.layout.lower.some((b) => b.band === "pianoRoll"));
  const playingInPianoRoll = $derived(app.transport.state === "playing" && pianoRollOpen && shownNotes.clip !== null && shownNotes.track !== null);

  /** 鳴っている鍵。動いている間はコマごとに計算し、変わったときだけ入れかえる（文を書きかえるのは、変わったときだけ）。 */
  let sounding = $state.raw<number[]>([]);
  let offsetUs: number | null = null;
  $effect(() => {
    if (!playingInPianoRoll) {
      sounding = [];
      return;
    }
    if (offsetUs === null) {
      clockOffsetUs().then(
        (v) => (offsetUs = v),
        () => {},
      );
    }
    return everyGlFrame(() => {
      const now = offsetUs === null ? null : performance.now() * 1000 + offsetUs;
      const tick = shownTick(app.transport, current.telemetry, now);
      const keys = soundingKeys(shownNotes.take, shownNotes.clip, tick);
      if (!sameKeys(keys, sounding)) sounding = keys;
    });
  });

  function playingParts(): TextPart[] | null {
    const clip = shownNotes.clip;
    const tr = shownNotes.track;
    if (!clip || !tr) return null;
    return joined(
      textParts("status.pianoRollClip", { track: track({ number: tr.number, name: tr.name }), clip: clip.name }),
      clip.takes > 1 ? textParts("arrange.takeSwitch", { take: clip.take, takes: clip.takes }) : [],
      sounding.length > 0 ? textParts("status.sounding", { keys: keysLabel(sounding, rollDrums) }) : [],
    );
  }

  /** 止まっているときの左の文の終わりに足す「4 パッド ・ 5 ベース はミュート」（モック Main）。 */
  function mutedParts(): TextPart[] {
    const muted = app.song.tracks.filter((x) => x.muted).map(track);
    if (muted.length === 0) return [];
    return textParts(muted.length === 1 ? "status.muted" : "status.mutedMany", { tracks: muted.join(STATUS_JOIN) });
  }

  // ---- ピアノロールをひらいて録音しているとき（仕様書 6 章「録音中 ・ ピアノロール」）: 「1 ピアノ へ録音 ・ テイク 3 ・ 押している音 C3 E3 G3 C4」 ----

  const recordingInPianoRoll = $derived(pianoRollOpen && app.status.message?.id === "recording");

  /** 押している鍵（入力計と同じ物: 計器で受け、鍵の on/off は計器を待たずに当てる）。録音中にピアノロールをひらいている間だけ数える。 */
  let held = $state.raw<number[]>([]);
  const setHeld = (keys: number[]) => {
    if (!sameKeys(keys, held)) held = keys;
  };
  $effect(() => {
    const tel = current.telemetry;
    if (!recordingInPianoRoll || !tel) return;
    const keys: number[] = [];
    for (let k = 0; k < 128; k++) if (keyIsDown(tel, k)) keys.push(k);
    untrack(() => setHeld(keys));
  });
  onMount(() =>
    onEvent((e) => {
      if (e.type !== "note" || !recordingInPianoRoll) return;
      setHeld(e.on && e.velocity > 0 ? (held.includes(e.note) ? held : [...held, e.note].sort((a, b) => a - b)) : held.filter((k) => k !== e.note));
    }),
  );

  function recordingParts(m: Extract<StatusMessage, { id: "recording" }>): TextPart[] {
    return joined(
      textParts("msg.recordingPianoRoll", { track: track(m.track), take: m.take }),
      held.length > 0 ? textParts("status.held", { keys: keysLabel(held, keysDrums) }) : [],
    );
  }

  // ---- クリップがない所でピアノロールをひらいて止まっているとき（モック F-PREmpty）: 「1 ピアノ ・ クリップなし」 ----

  const noClipTrack = $derived.by((): TrackRef | null => {
    if (!pianoRollOpen || app.transport.state !== "stopped" || app.arrange.pianoRollClip !== null) return null;
    const tr = app.song.tracks.find((x) => x.id === app.song.playTarget);
    return tr ? { number: tr.number, name: tr.name } : null;
  });

  const message = $derived.by((): Shown | null => {
    // ドラッグしている間の読み値（仕様書 2 章「その場の読み値」: 同じ値を状態行にも出す）。
    if (dragReading.text !== null) return { parts: [{ text: dragReading.text, hole: "reading" }], strong: [], button: null };
    const m = app.status.message;
    if (playingInPianoRoll && (m === null || m.id === "summary")) {
      const parts = playingParts();
      if (parts) return { parts, strong: [], button: null };
    }
    if (recordingInPianoRoll && m?.id === "recording") return { parts: recordingParts(m), strong: [], button: null };
    let base: Shown | null;
    if (noClipTrack && (m === null || (m.id === "summary" && m.notes === null))) {
      const loop = m?.id === "summary" ? m.loopBars : null;
      const parts = joined(
        loop ? textParts("status.loop", { from: loop.from, to: loop.to }) : [],
        textParts("status.noClip", { track: track(noClipTrack) }),
      );
      base = { parts, strong: [], button: null };
    } else base = shown(m);
    if (app.transport.state === "stopped" && (m === null || m.id === "summary")) {
      const muted = mutedParts();
      if (muted.length > 0) return { parts: joined(base?.parts ?? [], muted), strong: base?.strong ?? [], button: base?.button ?? null };
    }
    return base;
  });

  /** いちばん左の札（ヤチヨさんの決め 2026-11-15）。録音とカウントインは rec。止まっていてためし弾き中は accent（モック F4-ShelfAudition）。 */
  const badge = $derived.by((): { text: string; rec: boolean; accent?: boolean } => {
    switch (app.transport.state) {
      case "playing":
        return { text: t("status.playing"), rec: false };
      case "recording":
        return { text: t("status.recording"), rec: true };
      case "countIn":
        return { text: t("status.countingIn"), rec: true };
      default:
        if (app.shelf.selected !== null) return { text: t("status.auditioning"), rec: false, accent: true };
        return { text: t("status.stopped"), rec: false };
    }
  });

  const outputName = $derived(app.outputs.list.find((o) => o.id === app.outputs.chosen)?.name ?? null);
  const keyboardName = $derived(
    app.keyboards.inputStopped ? null : (app.keyboards.list.find((k) => k.id === app.keyboards.active)?.name ?? null),
  );
  const actual = $derived(app.audio.actualFrames);
  const requested = $derived(app.audio.requestedFrames);
  const scale = $derived(app.workspace.scale);
  const unfit = $derived(app.workspace.unfitScales);

  // 拡大率の一覧。画面だけが持つ「ひらいている」。率を決めるのは core（setScale を送るだけ）。
  let open = $state(false);
  let zoomButton = $state<HTMLButtonElement>();
  let popup = $state<HTMLElement>();

  // 一覧はダイアログより手前に出すので、.app の重なりの外（body）に置き、状態行の右上にそろえる。
  let anchor = $state({ right: 0, bottom: 0 });
  function place(): void {
    if (!row) return;
    const r = row.getBoundingClientRect();
    anchor = { right: window.innerWidth - r.right, bottom: window.innerHeight - r.top };
  }
  function toBody(node: HTMLElement): { destroy(): void } {
    document.body.append(node);
    return { destroy: () => node.remove() };
  }

  function closeZoom(): void {
    const hadFocus = popup?.contains(document.activeElement) ?? false;
    open = false;
    if (hadFocus) zoomButton?.focus();
  }

  function pick(s: Scale): void {
    send({ type: "setScale", percent: s });
    closeZoom();
  }

  $effect(() => {
    if (!open) return;
    place();
    window.addEventListener("resize", place);
    const close = () => closeZoom();
    const forget = menuOpened(close);
    const onDown = (ev: PointerEvent) => {
      const target = ev.target as Node | null;
      if (target && (popup?.contains(target) || zoomButton?.contains(target))) return;
      close();
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => {
      forget();
      window.removeEventListener("resize", place);
      document.removeEventListener("pointerdown", onDown, true);
    };
  });

  // 右がわの入れかた: 見えない写し（ruler）で、ちぢめない幅を測る。
  let row = $state<HTMLElement>();
  let ruler = $state<HTMLElement>();
  let fit = $state<StatusRightShown>({ inputs: true, output: true, buffer: true });

  function measure(): void {
    if (!row || !ruler) return;
    const style = getComputedStyle(row);
    const gap = parseFloat(style.columnGap) || 0;
    if (open) place();
    const available = row.clientWidth - (parseFloat(style.paddingLeft) || 0) - (parseFloat(style.paddingRight) || 0);
    const width = (part: string) => ruler!.querySelector<HTMLElement>(`[data-part="${part}"]`)?.offsetWidth;
    fit = statusFit({
      available,
      gap,
      left: width("left") ?? 0,
      right: { inputs: width("inputs"), output: width("output"), buffer: width("buffer") },
      scale: width("scale") ?? 0,
    });
  }

  onMount(() => {
    const observer = new ResizeObserver(() => measure());
    if (row) observer.observe(row);
    if (ruler) observer.observe(ruler);
    return () => observer.disconnect();
  });
  $effect(() => {
    void [badge, message, outputName, keyboardName, actual, requested, scale];
    measure();
  });
</script>

{#snippet left()}
  <span class:rec={badge.rec} class:accent={badge.accent} data-test="status-badge">{badge.text}</span>
  {#if message}
    <span class="m-lbl message">
      {#each message.parts as part, i (i)}
        {#if part.hole !== null}
          <span class="m-num" class:m-hi={message.strong.includes(part.hole)}>{part.text}</span>
        {:else if message.term}
          {#each termParts(part.text, message.term) as piece, j (j)}{#if piece.term}<span use:term={message.term}
                >{piece.text}</span
              >{:else}{piece.text}{/if}{/each}
        {:else}{part.text}{/if}
      {/each}
    </span>
    {#if message.button}
      {@const command = message.button.command}
      <button class="m-plain m-hi link" onclick={() => send(command)}>{message.button.label}</button>
    {/if}
  {/if}
{/snippet}

{#snippet output()}{outputName ? t("status.output", { output: outputName }) : t("status.outputNone")}{/snippet}
{#snippet inputs()}{keyboardName ? t("status.inputs", { inputs: keyboardName }) : t("status.inputsNone")}{/snippet}
{#snippet buffer()}
  {#if actual !== requested}
    {#each textParts("status.bufferMismatch", { actual: actual!, requested }) as part, i (i)}
      {#if part.hole !== null}<span class="m-num" class:m-hi={part.hole === "actual"}>{part.text}</span>{:else}{part.text}{/if}
    {/each}
  {:else}
    {#each textParts("status.buffer", { actual: actual! }) as part, i (i)}
      {#if part.hole !== null}<span class="m-num">{part.text}</span>{:else}{part.text}{/if}
    {/each}
  {/if}
{/snippet}
{#snippet zoom()}<span class="m-num rate">{t("status.scale", { scale })}</span>▾{/snippet}

<div class="status-line">
  {#if open}
    <div class="popup" style:right="{anchor.right}px" style:bottom="{anchor.bottom}px" use:toBody bind:this={popup}>
      <ZoomMenu {scale} {unfit} {platform} onpick={pick} />
    </div>
  {/if}
  <footer class="status" bind:this={row}>
    {@render left()}
    <span class="m-spacer"></span>
    {#if fit.output}<span class="m-lbl" data-test="status-output">{@render output()}</span>{/if}
    {#if fit.inputs}<span class="m-lbl" data-test="status-inputs">{@render inputs()}</span>{/if}
    {#if actual !== null && fit.buffer}
      <span class="m-lbl" data-test="status-buffer" use:term={"バッファ"}>{@render buffer()}</span>
    {/if}
    <button
      class="m-plain zoom"
      class:open
      aria-label={t("zoom.heading")}
      aria-haspopup="menu"
      aria-expanded={open}
      data-test="status-scale"
      bind:this={zoomButton}
      onclick={() => (open = !open)}>{@render zoom()}</button
    >
  </footer>
  <div class="ruler" aria-hidden="true" bind:this={ruler}>
    <span class="group" data-part="left">{@render left()}</span>
    <span class="m-lbl" data-part="output">{@render output()}</span>
    <span class="m-lbl" data-part="inputs">{@render inputs()}</span>
    {#if actual !== null}<span class="m-lbl" data-part="buffer">{@render buffer()}</span>{/if}
    <span class="zoom wide" data-part="scale">{@render zoom()}</span>
  </div>
</div>

<style>
  .status-line {
    position: relative;
    flex-shrink: 0;
  }
  /* 重なり: ダイアログ（30）より上、用語の説明（1000）より下。 */
  .popup {
    position: fixed;
    z-index: 40;
    margin-right: var(--space-3);
  }
  .status {
    height: var(--size-status);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: 0 var(--space-3);
    background: var(--color-gray-0);
    border-top: var(--line-hair) solid var(--color-gray-5);
    white-space: nowrap;
    overflow: hidden;
  }
  .message {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .rec {
    color: var(--color-rec);
  }
  .accent {
    color: var(--color-accent);
  }
  .link {
    flex-shrink: 0;
    text-decoration: underline;
  }
  .status > :not(.message):not(.m-spacer) {
    flex-shrink: 0;
  }

  /* 拡大率: ふつう（地も枠も内がわもなし。モック Main）/ 乗った（文字 gray.10）/ ひらいている（地 gray.4、枠 gray.8。モック F-Zoom）。 */
  .zoom {
    height: 100%;
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    white-space: nowrap;
  }
  .rate {
    color: var(--color-gray-10);
  }
  button.zoom:hover {
    color: var(--color-gray-10);
  }
  button.zoom.open {
    padding: 0 var(--part-button-pad-x);
    background: var(--color-gray-4);
    border: var(--line-hair) solid var(--color-gray-8);
    color: var(--color-gray-10);
  }

  /* 写しでは、ひらいたときの幅（内がわと枠つき）で測る。 */
  .ruler .wide {
    padding: 0 var(--part-button-pad-x);
    border: var(--line-hair) solid transparent;
  }
  .ruler {
    position: absolute;
    left: 0;
    top: 0;
    display: flex;
    gap: var(--space-4);
    width: max-content;
    height: var(--size-status);
    visibility: hidden;
    pointer-events: none;
    white-space: nowrap;
  }
  .ruler > *,
  .group {
    display: inline-flex;
    align-items: center;
    flex-shrink: 0;
  }
  .group {
    gap: var(--space-4);
  }
</style>

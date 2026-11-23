<!-- The status line (the bottom row of mock F-Connect1 to 3, F-BufferFixed, F-Zoom, F-WindowOpen).
     The right side is output, inputs, 「窓 N ▾」 (not shown when there are no plugin windows; pressing it opens a list above), buffer, scale. -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import {
    clockOffsetUs,
    keyIsDown,
    paramTable,
    type Band,
    type Command,
    type EffectName,
    type InsertRef,
    type ParamEnd,
    type ParamRef,
    type PluginWindowView,
    type NoteSelection,
    type PluginSlotRef,
    type SongDialogKind,
    type StatusMessage,
    type TrackRef,
    type ValueField,
    type VocalRef,
  } from "../bridge";
  import type { Scale } from "../design/tokens";
  import { everyGlFrame } from "../gl/slot";
  import { platformOf } from "../keys/keys";
  import { menuOpened } from "../lib/menu";
  import { term } from "../lib/tip.svelte";
  import { current, onEvent, send } from "../state/app.svelte";
  import { shownWork } from "../state/audioWork.svelte";
  import { exportShown } from "../state/exportShown.svelte";
  import { LIST_COMMA, STATUS_JOIN, t, type TextKey } from "../text/ja";
  import { about } from "./about/about.svelte";
  import { runShown } from "./export/exportText";
  import { gridText, msText, noteLengthText, pitchClassName, scaleName, tempoText, termParts, textParts, type TextPart } from "./format";
  import { laneNameText, stepNameText, stepWhatText } from "./history/stepText";
  import { laneAddOpenFor } from "./automation/laneAdd.svelte";
  import { stripName } from "./mixer/stripName";
  import { paramName, valueText } from "./params/paramRows";
  import { paramUi } from "./params/paramUi.svelte";
  import { EFFECT_PARAMS } from "../bridge/types";
  import WindowMark from "./WindowMark.svelte";
  import WindowsMenu from "./WindowsMenu.svelte";
  import { keyCountsText, keysDrums, keysLabel, rollShowsDrums } from "./pianoroll/drums";
  import { soundNameText } from "./sounds";
  import { askInStatus, packInStatus, packMb, percentOf } from "./pack/pack";
  import { indexesOf } from "./pianoroll/geom";
  import { shown as shownNotes } from "./pianoroll/shown.svelte";
  import { sameKeys, soundingKeys } from "./pianoroll/sounding";
  import { dragReading } from "./pianoroll/view.svelte";
  import { chordHover } from "./pianoroll/chordPick.svelte";
  import { shelfStatus, shelfUi } from "./tracks.svelte";
  import { statusFit, type StatusRightShown } from "./statusFit";
  import { mapOf } from "../time/bars";
  import { positionText, shownTick } from "./time";
  import ZoomMenu from "./ZoomMenu.svelte";
  import { overdubClipName } from "./clipEdits";

  const app = $derived(current.state!);
  /** The piano roll shows drum rows (selected notes and sounding notes as drum piece names). */
  const rollDrums = $derived(rollShowsDrums(app, shownNotes.clip, shownNotes.track));
  /** The play target of the keys is drums (held notes as drum piece names). */
  const heldDrums = $derived(keysDrums(app));

  interface Shown {
    parts: TextPart[];
    /** Slots whose values are shown in gray.10. */
    strong: string[];
    button: { label: string; command: Command } | null;
    /** Words in the text that get a term tip. */
    term?: string;
  }

  const RECOVERY_TERM = "なおすためのファイル";
  /** Name slots (strip, plugin, effect, parameter). Shown in the same font as the text (the mock's status line). Other slots (numbers, values, keys) are monospace. */
  const NAME_HOLES = new Set(["name", "strip", "track", "effect", "plugin", "param", "target", "order"]);
  const platform = platformOf(navigator);
  const modKey = (letter: string) => `${platform === "mac" ? t("key.cmd") : t("key.ctrl")} ${letter}`;

  const SONG_DIALOG_NAMES: Record<SongDialogKind, TextKey> = {
    unsaved: "songDialog.unsaved",
    cannotSave: "songDialog.cannotSave",
    broken: "songDialog.broken",
  };

  const BAND_NAMES: Record<Band, TextKey> = {
    pianoRoll: "band.pianoRoll",
    mixer: "band.mixer",
    history: "band.history",
    shelf: "band.shelf",
    params: "band.params",
  };

  /** The field name and range for a value typed in a number field. */
  // For fields whose range depends on the clip (position, length), the range text comes with the message (`range`).
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

  /** The name of where the plugin is (its track for an instrument, its strip for an insert). */
  const slotName = (s: PluginSlotRef): string =>
    stripName(app.song, s.kind === "instrument" ? { kind: "track", id: s.track } : s.insert.strip, app.library);

  const effectName = (e: EffectName) => (e.kind === "builtin" ? t(`effect.${e.effect}`) : e.name);
  const insertStrip = (i: InsertRef) => stripName(app.song, i.strip, app.library);
  /** 「2 ベース の コンプレッサー 」 (with a space at the end). */
  const insertTarget = (i: InsertRef, e: EffectName) => t("status.insertTarget", { strip: insertStrip(i), effect: effectName(e) });
  /** The 「（名前）」 of the window texts in spec 6: the place and the plugin name, with a space at the end. Only the name for a plugin being tried in the shelf (docs/history/design-notes.ja.md A-17). */
  const windowName = (name: string, slot: PluginSlotRef | null) => `${slot ? t("status.windowPlace", { strip: slotName(slot), name }) : name} `;
  /** A name given by the plugin gets a space before the following 「は」 or 「の」 (「Surge XT の」). */
  const spaced = (name: string) => `${name} `;
  const MINUS = "−";
  const plainNumber = (n: number) => String(n).replace("-", MINUS);
  const mb = (bytes: number) => Math.round(bytes / 1_000_000);

  /** The parameter name for the typed-value text (built-in ones from the table, plugins from the shown table). */
  function typedParamName(p: ParamRef): string {
    if (p.kind === "builtin") return paramName(p.effect, p.param);
    try {
      const book = paramTable();
      const row = book.table()?.table === p.table ? book.row(p.param) : undefined;
      return row ? spaced(row.name) : "";
    } catch {
      return "";
    }
  }

  /** The value before typing. Built-in effects send a raw number ("-24"), so it is written with the same unit and digits as the row value. For plugins, the text as received. */
  function keptText(p: ParamRef, kept: string): string {
    if (p.kind !== "builtin") return kept;
    const spec = EFFECT_PARAMS[p.effect][p.param];
    const n = Number(kept.replace("−", "-"));
    return spec && kept.trim() !== "" && Number.isFinite(n) ? valueText(p.effect, spec, n) : kept;
  }

  /** The 3 parts of 「レシオは 1〜20 なので、20.0 : 1 にしました。」. */
  function clampedParts(to: ParamEnd): { param: string; range: string; value: string } {
    if (to.kind === "plugin") return { param: spaced(to.name), range: to.range, value: to.value };
    const spec = EFFECT_PARAMS[to.effect][to.param];
    if (!spec) return { param: "", range: "", value: "" };
    return { param: paramName(to.effect, to.param), range: `${plainNumber(spec.min)}〜${plainNumber(spec.max)}`, value: valueText(to.effect, spec, to.value) };
  }

  /** Where a plugin window belongs (「1 ピアノ ・ 音源」「A リバーブ ・ インサート 1」「棚 ・ ためし弾き」). */
  function windowPlace(w: PluginWindowView): string {
    const slot = w.slot;
    if (!slot) return t("windows.shelfAudition");
    if (slot.kind === "instrument") return t("params.whereInstrument", { strip: slotName(slot) });
    return t("params.where", { strip: slotName(slot), slot: slot.insert.slot + 1 });
  }

  /** 「あと {beats} 拍」 during the count-in. The clicks left from the telemetry if it reports a count-in. */
  function beatsLeft(fallback: number): number {
    const p = current.telemetry?.position;
    return p?.state === "countIn" ? p.countInBeatsLeft : fallback;
  }

  /** 「1 つは SF2 ではないので、足していません。」 (when some of the dropped files were not added). */
  const skippedParts = (skipped: number): TextPart[] => (skipped > 0 ? textParts("sf2.skipped", { count: skipped }) : []);

  /** 「ピアノ ・ 鍵盤 と ギター」「ピアノ ・ 鍵盤、ギター と ベース」「ピアノ ・ 鍵盤 ほか 4 つ」 (spec 6 "Shelf open (headings)"). */
  function groupsText(names: readonly string[]): string {
    if (names.length >= 5) return t("status.shelfGroupsMore", { first: names[0]!, count: names.length - 1 });
    if (names.length === 1) return names[0]!;
    return t("status.shelfGroupsLast", { first: names.slice(0, -1).join(LIST_COMMA), last: names.at(-1)! });
  }

  /** Joins parts of the text with 「 ・ 」 (spec 6 "Left of the status line while stopped"). */
  function joined(...pieces: TextPart[][]): TextPart[] {
    return pieces.filter((p) => p.length > 0).flatMap((p, i) => (i === 0 ? p : [{ text: STATUS_JOIN, hole: null }, ...p]));
  }

  /** The pass going on overdubs a clip: that clip's name. */
  const overdubName = $derived.by(() => {
    const r = app.transport.recording;
    return r ? overdubClipName(app.song.tracks, r) : null;
  });

  /** Recording with 「重ねる」 (mock P10-Overdub); the loop part only in a loop. */
  function overdubParts(who: TrackRef, clip: string, loop: { from: number; to: number; lap: number } | null): TextPart[] {
    return joined(
      textParts("msg.overdubRecording", { track: track(who), clip }),
      loop ? textParts("msg.overdubLap", loop) : [],
      textParts("msg.overdubKept", { clip }),
    );
  }

  /**
   * 「選択 3 ノート ・ D4 E4 F4 ・ クオンタイズ済み（もとの位置へ戻せます）」 (mock A-PianoRoll). Repeated names are grouped as 「スネア ×2」 (spec 3 85).
   * With 5 or more names, 「…」 after 4.
   */
  function notesParts(n: NoteSelection): TextPart[] {
    const values = { count: n.count, keys: keyCountsText(n.names.slice(0, 4), rollDrums) };
    return joined(
      textParts(n.names.length > 4 ? "status.notesSelectedMore" : "status.notesSelected", values),
      n.quantized ? textParts("status.notesQuantized", {}) : [],
    );
  }

  /** When lanes were removed too, 「（そのパラメータの帯 2 本も消えました）」 goes before the first 「。」 of the text (spec 3 92-19, spec 6). */
  function withLanes(shown: Shown, lanesRemoved: number): Shown {
    if (lanesRemoved <= 0) return shown;
    const gone = textParts("msg.lanesAlsoGone", { count: lanesRemoved });
    const at = shown.parts.findIndex((p) => p.hole === null && p.text.includes("。"));
    if (at < 0) return { ...shown, parts: [...shown.parts, ...gone] };
    const part = shown.parts[at]!;
    const cut = part.text.indexOf("。");
    const parts = [
      ...shown.parts.slice(0, at),
      { text: part.text.slice(0, cut), hole: null },
      ...gone,
      { text: part.text.slice(cut), hole: null },
      ...shown.parts.slice(at + 1),
    ].filter((p) => p.text !== "");
    return { ...shown, parts };
  }

  /**
   * The text while recording vocals (spec 6 "Recording vocals", "Recording 2 or more vocal tracks", "Recording keyboard and vocals"). The leading 「● 録音中 ・ 」 is said by the tag on the left, so it is left out of the text.
   * 「モニター オン ・ 」 is left out when monitoring is off. For keyboard plus 2 or more vocals, the vocals are listed as 「6 歌（入口 1）、7 歌 2」 (a form not in the spec).
   */
  function recordingVocalsParts(keys: TrackRef | null, vocals: VocalRef[], monitor: boolean): TextPart[] {
    const short = (v: VocalRef) => t(`vocal.inputShort.${v.input}`);
    const last = vocals.at(-1);
    if (!last) return keys ? textParts("msg.recording", { track: track(keys), take: app.transport.recording?.take ?? 1 }) : [];
    const lead = vocals.slice(0, -1).map((v) => `${track(v.track)}（${short(v)}）`);
    const vocal = [...lead, track(last.track)].join(LIST_COMMA);
    const values = { stopKey: t("key.playStop"), input: short(last) };
    const parts = keys
      ? textParts("msg.recordingKeysAndVocal", { ...values, keys: track(keys), vocal })
      : vocals.length > 1
        ? textParts("msg.recordingVocals", { stopKey: values.stopKey, vocals: vocals.map((v) => `${track(v.track)}（${short(v)}）`).join(LIST_COMMA) })
        : textParts("msg.recordingVocal", { ...values, track: vocal });
    const badge = `${t("status.recording")}${STATUS_JOIN}`;
    const monitorOn = `${t("meter.monitorOn")}${STATUS_JOIN}`;
    return parts
      .map((p, i) => {
        if (p.hole !== null) return p;
        let text = i === 0 && p.text.startsWith(badge) ? p.text.slice(badge.length) : p.text;
        if (!monitor) text = text.replace(monitorOn, "");
        return { ...p, text };
      })
      .filter((p) => p.text !== "");
  }

  function shown(m: StatusMessage | null): Shown | null {
    const plain = (key: TextKey, values: Record<string, string | number> = {}): Shown => ({
      parts: textParts(key, values),
      strong: [],
      button: null,
    });
    const settings = (advanced: boolean, label: string) => ({ label, command: { type: "openSettings", advanced } as Command });
    const place = (bar: number, beat: number) => (beat === 1 ? t("timeline.placeBar", { bar }) : t("timeline.placeBeat", { bar, beat }));
    // The point at the start of the song is not called a 「変更」 (spec 3 92-2).
    const tempoSelected = (bar: number, beat: number, bpm: number) =>
      bar === 1 && beat === 1
        ? plain("msg.headTempoSelected", { tempo: tempoText(bpm) })
        : plain("msg.tempoPointSelected", { place: place(bar, beat), tempo: tempoText(bpm) });
    const placeOrHead = (bar: number, beat: number) => (bar === 1 && beat === 1 ? t("timeline.placeHead") : place(bar, beat));
    switch (m?.id) {
      case "undone":
      case "redone": {
        const name = stepNameText(m.name);
        const what = stepWhatText(m.name, m.what, app);
        const redoKey = modKey(`${t("key.shift")} ${t("key.undo")}`);
        if (m.id === "undone") return what === null ? plain("msg.undoneBare", { name, redoKey }) : plain("msg.undone", { name, what, redoKey });
        return what === null ? plain("msg.redoneBare", { name }) : plain("msg.redone", { name, what });
      }
      case "historyJumped":
        return plain(m.forward ? "msg.historyJumpedForward" : "msg.historyJumped", { index: m.index, name: stepNameText(m.name), steps: m.steps });
      case "nothingToUndo":
        return plain("msg.nothingToUndo");
      case "nothingToRedo":
        return plain("msg.nothingToRedo");
      case "automationWriting":
        return plain("msg.automationWriting", { track: track(m.track), target: laneNameText(m.target) });
      case "automationRested":
        return plain("msg.automationRested", { track: track(m.track), target: laneNameText(m.target) });
      case "automationRestored":
        return plain("msg.automationRestored", { track: track(m.track), target: laneNameText(m.target) });
      case "automationMoving": {
        const names = m.tracks.map(track);
        const tracks = names.length < 2 ? (names[0] ?? "") : t("status.shelfGroupsLast", { first: names.slice(0, -1).join(LIST_COMMA), last: names.at(-1)! });
        return plain("msg.automationMoving", { tracks });
      }
      case "tempoRampSelected": {
        if (m.toTempoBpm === m.tempoBpm) return tempoSelected(m.bar, m.beat, m.tempoBpm);
        const ramp = {
          tempo: tempoText(m.tempoBpm),
          toPlace: place(m.toBar, m.toBeat),
          toTempo: tempoText(m.toTempoBpm),
          direction: t(m.toTempoBpm > m.tempoBpm ? "timeline.rampFaster" : "timeline.rampSlower"),
        };
        if (m.bar === 1 && m.beat === 1) return plain("msg.headTempoRampSelected", ramp);
        return plain("msg.tempoRampSelected", { ...ramp, place: place(m.bar, m.beat) });
      }
      case "tempoPointSelected":
        return tempoSelected(m.bar, m.beat, m.tempoBpm);
      case "meterChangeSelected": {
        const signature = `${m.meterNum}/${m.meterDen}`;
        return m.bar === 1 ? plain("msg.headMeterSelected", { signature }) : plain("msg.meterChangeSelected", { place: place(m.bar, 1), signature });
      }
      case "markerAdded":
        return plain("msg.markerAdded", { name: m.name });
      case "tempoPointSet":
        return plain("msg.tempoPointSet", { place: placeOrHead(m.bar, m.beat), tempo: tempoText(m.tempoBpm) });
      case "meterChangeSet":
        return plain("msg.meterChangeSet", { place: placeOrHead(m.bar, 1), signature: `${m.meterNum}/${m.meterDen}` });
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
      case "scanStoppedOpen":
        return plain("status.dialogOpen", { dialog: t("plugin.stoppedTitle") });
      case "pluginsFound":
        return plain("plugin.newFound", { count: m.count });
      case "scanFinished":
        return m.stopped === 0
          ? plain("msg.scanDoneNone", { added: m.added })
          : {
              ...plain("msg.scanDone", { added: m.added, stopped: m.stopped }),
              button: { label: t("button.showStopped", { count: m.stopped }), command: { type: "openScanStopped" } },
            };
      case "scanRetryAdded":
        return plain("plugin.retryOk", { name: m.name });
      case "scanRetryStopped":
        return plain("plugin.retryStopped", { name: m.name });
      case "pluginStopped":
        return plain("msg.pluginStopped", { name: spaced(m.name), track: slotName(m.slot) });
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
        return plain("msg.lengthMatched", { count: m.count, value: noteLengthText(m.value, { num: app.timeline.meterNum, den: app.timeline.meterDen }) });
      case "velocityScaled":
        return plain("msg.velocityScaled", { count: m.count, percent: m.percent });
      case "velocityRamped":
        return plain("msg.velocityRamped", { count: m.count, from: m.from, to: m.to });
      case "velocityRandomized":
        return plain("msg.velocityRandomized", { count: m.count, amount: m.amount });
      case "notesLegato":
        return plain("msg.notesLegato", { count: m.count });
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
        if (overdubName !== null) return { parts: overdubParts(m.track, overdubName, null), strong: [], button: null };
        return plain("msg.recording", { track: track(m.track), take: m.take });
      case "loopRecording":
        if (overdubName !== null) return { parts: overdubParts(m.track, overdubName, { from: m.fromBar, to: m.toBar, lap: m.lap }), strong: [], button: null };
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
        return withLanes(
          plain("msg.instrumentReplaced", {
            track: track(m.track),
            instrument: t(`instrument.${m.instrument}`),
            undoKey: modKey(t("key.undo")),
          }),
          m.lanesRemoved,
        );
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
      case "auditionNewTrackSound":
        return plain("status.auditionNewTrack", { instrument: soundNameText(m.sound), key: t("key.enter") });
      case "auditionReplaceSound":
        return plain("status.auditionReplace", {
          instrument: soundNameText(m.sound),
          track: track(m.track),
          current: soundNameText(m.current),
          key: t("key.enter"),
        });
      case "samplePlayingSound":
        return plain("msg.samplePlaying", { name: soundNameText(m.sound) });
      case "instrumentReplacedSound":
        return withLanes(
          plain("msg.instrumentReplaced", { track: track(m.track), instrument: soundNameText(m.sound), undoKey: modKey(t("key.undo")) }),
          m.lanesRemoved,
        );
      case "soundLoading":
        return plain("msg.auditionLoading", { name: soundNameText(m.sound) });
      case "pluginLoading":
        return { parts: joined(textParts("plugin.loading", { name: m.name }), textParts("status.cancelKey", { key: t("key.esc") })), strong: [], button: null };
      case "sf2Dragging":
        return plain("sf2.dragging");
      case "notSf2Dragging":
        return plain("sf2.notSf2Dragging");
      // 「音色 128 ・ ドラムのキット 9」 does not end with 「。」, so it is joined to the not-added text with 「 ・ 」 (docs/history/design-notes.ja.md A-19).
      case "sf2Added":
        return { parts: joined(textParts("sf2.added", { file: m.file, count: m.presets, kits: m.kits }), skippedParts(m.skipped)), strong: [], button: null };
      case "sf2AddedMany":
        return {
          parts: [...textParts("sf2.addedMany", { count: m.added, file: m.first, others: m.added - 1 }), ...skippedParts(m.skipped)],
          strong: [],
          button: null,
        };
      case "pluginMissing":
        return plain("plugin.missingSong", { name: m.name, count: m.tracks });
      case "pluginUnreadable":
        return plain("plugin.unreadableSong", { name: m.name, count: m.tracks });
      case "instrumentPluginRemoved":
        return plain("plugin.instrumentRemoved", { track: track(m.track), undoKey: modKey(t("key.undo")) });
      case "sf2Unreadable":
        return plain("sf2.unreadable", { file: m.file });
      case "sf2Missing":
        return plain("sf2.missing", { file: m.file, count: m.tracks });
      case "soundsMissing":
        return plain("msg.soundsMissingSong", { count: m.tracks });
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
        // 「もとの位置へ戻す」 acts on the selected notes (the quantized notes are selected; all of them if none).
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
        // A return has the effect in its name, so 「A リバーブの「…」」; tracks and the master are 「2 ベース ・ コンプレッサーの「…」」 (spec 3 86-10).
        const selected =
          m.insert.strip.kind === "return"
            ? textParts("status.paramSelected", { strip: insertStrip(m.insert), param: paramName(m.effect, m.param) })
            : textParts("status.paramSelectedInsert", { strip: insertStrip(m.insert), effect: t(`effect.${m.effect}`), param: paramName(m.effect, m.param) });
        return { parts: joined(loop ? textParts("status.loop", { from: loop.from, to: loop.to }) : [], selected), strong: [], button: null };
      }
      case "pluginParamSelected": {
        const loop = m.loopBars;
        const s = m.slot;
        const selected =
          s.kind === "insert" && s.insert.strip.kind === "return"
            ? textParts("status.paramSelected", { strip: slotName(s), param: m.param })
            : textParts("status.paramSelectedInsert", { strip: slotName(s), effect: spaced(m.plugin), param: m.param });
        return { parts: joined(loop ? textParts("status.loop", { from: loop.from, to: loop.to }) : [], selected), strong: [], button: null };
      }
      case "paramClamped":
        return plain("params.valueClamped", clampedParts(m.to));
      case "paramRejected":
        return plain("msg.valueRejected", { field: typedParamName(m.param), kept: keptText(m.param, m.kept) });
      case "effectInserted":
        return plain("msg.effectInserted", { strip: insertStrip(m.insert), slot: m.insert.slot + 1, effect: effectName(m.effect) });
      case "effectRemoved":
        return withLanes(
          plain("msg.effectRemoved", { strip: insertStrip(m.insert), slot: m.insert.slot + 1, effect: effectName(m.effect), undoKey: modKey(t("key.undo")) }),
          m.lanesRemoved,
        );
      case "insertsReordered":
        return plain("msg.insertsReordered", { strip: stripName(app.song, m.strip, app.library), order: m.order.map(effectName).join(" → ") });
      case "insertReplaced":
        return withLanes(
          plain("msg.insertReplaced", { strip: insertStrip(m.insert), slot: m.insert.slot + 1, effect: effectName(m.effect), undoKey: modKey(t("key.undo")) }),
          m.lanesRemoved,
        );
      case "effectBypassed":
        return plain(m.bypassed ? "msg.effectBypassed" : "msg.effectRestored", { target: insertTarget(m.insert, m.effect) });
      case "pluginWindowOpening":
        return plain("msg.windowOpening", { name: windowName(m.name, m.slot) });
      case "pluginWindowLarge":
        return plain("msg.windowTooLarge", { name: windowName(m.name, m.slot), width: m.width, height: m.height });
      case "pluginWindowOpen":
        return plain("msg.windowOpen", { name: m.name });
      case "pluginWindowsOpen":
        return plain("msg.windowsOpen", { count: m.count });
      case "pluginWindowFailed":
        return plain("msg.windowFailed", { name: m.name });
      case "pluginWindowNotYet": {
        const loop = m.loopBars;
        return {
          parts: joined(loop ? textParts("status.loop", { from: loop.from, to: loop.to }) : [], textParts("status.windowNotYet", { name: m.name })),
          strong: [],
          button: null,
        };
      }
      case "mixerPassesBy":
        return {
          parts: joined(
            ...m.bypassed.map((b) => textParts("status.insertBypassedNow", { strip: insertStrip(b.insert), effect: effectName(b.effect) })),
            ...m.stopped.map((b) => textParts("status.pluginStoppedNow", { strip: insertStrip(b.insert), name: effectName(b.effect) })),
          ),
          strong: [],
          button: null,
        };
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
      case "noParams":
        return plain("msg.noSoundParams", { name: soundNameText(m.sound) });
      case "vocalTrackAdded":
        return plain("msg.vocalTrackAdded", { track: track(m.track), input: t(`vocal.inputShort.${m.input}`) });
      case "noInput":
        return { ...plain("msg.noAudioInput"), button: settings(true, t("button.audioSettingsEllipsis")) };
      case "monitorFirstOn":
        return plain("msg.monitorFirstOn", { ms: m.latencyMs === null ? t("meter.playTargetNone") : String(Math.round(m.latencyMs)) });
      case "micTooLoud":
        return plain("msg.micTooLoud");
      case "vocalHasNoPianoRoll":
        return plain("msg.vocalNoPianoRoll");
      case "recordingVocals":
        return { parts: recordingVocalsParts(m.keys, m.vocals, m.monitor), strong: [], button: null };
      case "latencyMeasureOpen":
        return plain("status.songDialogOpen", { dialog: t("latency.title") });
      case "exportDialogOpen": {
        // The check progress, and whether some part goes over 0 dB (mock F8-ExportCheck, F8-ExportClip).
        const check = exportShown.current?.dialog?.check;
        const counting = check?.state === "counting" ? textParts("status.exportChecking", { percent: check.percent }) : [];
        const over = (check?.state === "counting" || check?.state === "done") && check.clips ? textParts("status.exportOver", {}) : [];
        return { parts: joined(textParts("status.songDialogOpen", { dialog: t("export.title") }), counting, over), strong: [], button: null };
      }
      // The spec has no text for cutting or adding several, or for dragging in just one, so these use
      // the text with the first name (cut, added) and the text with only the count (dragging in).
      case "audioMissing":
        return plain("msg.audioFilesMissing", { count: m.count });
      case "clipSplit":
        return plain("msg.clipSplit", { clip: m.name, position: positionText(m.tick, mapOf(app.timeline)) });
      case "audioImported":
        return plain("msg.audioImported", { file: m.file, track: track(m.track), position: positionText(m.tick, mapOf(app.timeline)) });
      case "audioUnreadable":
        return plain("msg.audioUnreadable", { file: m.file });
      case "audioDragging":
        return m.audio > 1 ? plain("msg.audioDragging", { count: m.audio }) : plain("msg.audioDraggingOne");
      case "sourceTempoFound":
        return {
          parts: [...textParts("msg.sourceTempoFound", { bpm: tempoText(m.bpm) }), ...textParts(m.same ? "msg.sourceTempoSame" : "msg.sourceTempoSet", {})],
          strong: [],
          button: null,
        };
      case "sourceTempoNotFound":
        return plain("msg.sourceTempoNotFound");
      // MB is 1000 × 1000 bytes.
      case "inputLost":
        return plain("msg.inputLost", { input: t(`vocal.inputShort.${m.input}`), device: m.device });
      case "recordingGaps":
        return plain("msg.recordingGaps", { count: m.count });
      case "audioFound":
        return plain("msg.audioFound", { file: m.file, clips: m.clips });
      case "audioFoundMismatch":
        return plain("msg.audioFoundMismatch");
      case "audioBroken":
        return plain("msg.audioBroken", { file: m.file });
      case "audioTooManyChannels":
        return plain("msg.audioTooManyChannels", { file: m.file });
      case "savingAudio":
        return plain("msg.savingAudio", { done: m.done, total: m.total, doneMb: mb(m.doneBytes), totalMb: mb(m.totalBytes) });
      case "diskTooSlow":
        return plain("msg.diskTooSlow");
      case "packAsking":
        return plain("pack.askingStatus");
      case "soundReady":
        return plain("pack.soundReady", { instrument: t(`instrument.${m.instrument}`) });
      case "packFetched":
        return plain("pack.fetched", { count: m.count });
      case "packFailed": {
        const f = m.failure;
        const again = { label: t(f.kind === "cutOff" ? "button.resumeFetch" : "button.again"), command: { type: "fetchPack" } as Command };
        if (f.kind === "offline") return { ...plain("pack.offline"), button: again };
        if (f.kind === "cutOff") return { ...plain("pack.cutOff", { done: mb(m.doneBytes), total: mb(m.totalBytes) }), button: again };
        if (f.kind === "diskFull") return { ...plain("pack.diskFull", { mb: mb(Math.max(0, f.neededBytes - f.freeBytes)) }), button: again };
        return { ...plain("pack.corrupt"), button: again };
      }
      case "newPack":
        return { ...plain("pack.newPack", { mb: mb(m.bytes) }), button: { label: t("button.fetchPack"), command: { type: "fetchPack" } } };
      default:
        return null;
    }
  }

  // ---- Playing with the piano roll open (mock F-PRPlay): 「1 ピアノ ・ Aメロ ・ テイク 2 / 3 ・ 鳴っている音 C3 E3 G3 C4」 ----

  const pianoRollOpen = $derived(app.workspace.layout.lower.some((b) => b.band === "pianoRoll"));
  const playingInPianoRoll = $derived(app.transport.state === "playing" && pianoRollOpen && shownNotes.clip !== null && shownNotes.track !== null);

  /** The sounding keys. Computed every frame while playing and replaced only when they change (the text is rewritten only on change). */
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

  /** 「4 パッド ・ 5 ベース はミュート」, added to the end of the left text when stopped (mock Main). */
  function mutedParts(): TextPart[] {
    const muted = app.song.tracks.filter((x) => x.muted).map(track);
    if (muted.length === 0) return [];
    return textParts(muted.length === 1 ? "status.muted" : "status.mutedMany", { tracks: muted.join(STATUS_JOIN) });
  }

  /**
   * Replaces the usual summary while the shelf is open (spec 6 "Shelf open (headings)", mock F5-ShelfSearch).
   * The headings text is said when the shelf opens and when a heading opens or folds, and stays until the core message changes. The search text is always shown while searching.
   */
  const shelfParts = $derived.by((): TextPart[] | null => {
    const s = shelfStatus(app);
    if (!s) return null;
    if (s.kind === "search") return textParts("status.shelfSearch", { query: s.query, count: s.count });
    if (!shelfAnnounced) return null;
    if (s.groups.length === 0) return textParts(s.kind === "effects" ? "shelf.effectCount" : "status.shelfCount", { count: s.count });
    return textParts(s.kind === "effects" ? "status.shelfEffectsOpen" : "status.shelfOpen", { count: s.count, groups: groupsText(s.groups) });
  });
  const shelfFront = $derived(app.workspace.layout.rightFront === "shelf");
  /** The core message at the time the headings text was said. Once the message changes, it is not said any more. */
  let shelfMark = $state.raw<{ message: StatusMessage | null } | null>(null);
  $effect(() => {
    void shelfUi.open;
    void shelfUi.view;
    const front = shelfFront;
    untrack(() => (shelfMark = front ? { message: app.status.message } : null));
  });
  const shelfAnnounced = $derived(shelfMark !== null && shelfMark.message === app.status.message);

  // ---- Mic too loud (spec 6 "Mic too loud (how long)"): 4 seconds after it last reached 0 dB, or until another message comes ----

  const MIC_LOUD_MS = 4000;
  let micLoudAt = 0;
  let micLoudTimer: ReturnType<typeof setTimeout> | null = null;
  /** 4 seconds have passed. Go back to the usual summary even if the core message is still "too loud". */
  let micLoudOver = $state(false);
  function micReached(now: number): void {
    micLoudAt = now;
    if (micLoudOver) micLoudOver = false;
    if (micLoudTimer === null) micLoudTimer = setTimeout(micLoudCheck, MIC_LOUD_MS);
  }
  function micLoudCheck(): void {
    const left = micLoudAt + MIC_LOUD_MS - performance.now();
    micLoudTimer = left > 0 ? setTimeout(micLoudCheck, left) : null;
    if (left <= 0) micLoudOver = true;
  }
  const micLoud = $derived(app.status.message?.id === "micTooLoud");
  $effect(() => {
    if (micLoud) untrack(() => micReached(performance.now()));
  });
  $effect(() => {
    if (!micLoud) return;
    const tel = current.telemetry;
    if (tel?.mics.some((m) => m.clipped)) untrack(() => micReached(performance.now()));
  });
  onMount(() => () => {
    if (micLoudTimer !== null) clearTimeout(micLoudTimer);
  });
  /** The core message. A "too loud" older than 4 seconds counts as none. */
  const statusMessage = $derived(micLoud && micLoudOver ? null : app.status.message);

  // ---- Recording with the piano roll open (spec 6 "Recording, piano roll"): 「1 ピアノ へ録音 ・ テイク 3 ・ 押している音 C3 E3 G3 C4」 ----

  const recordingInPianoRoll = $derived(pianoRollOpen && app.status.message?.id === "recording");
  /** Also when stopped, 「・ 押している音 キック スネア」 at the end while keys are held (spec 3 85). */
  const stoppedSummary = $derived(
    app.transport.state === "stopped" && (statusMessage === null || statusMessage.id === "summary"),
  );
  const countingHeld = $derived(recordingInPianoRoll || stoppedSummary);

  /** The held keys (same as the input meter: from the telemetry, with key on/off applied without waiting for it). Counted only while recording with the piano roll open, and while stopped. */
  let held = $state.raw<number[]>([]);
  const setHeld = (keys: number[]) => {
    if (!sameKeys(keys, held)) held = keys;
  };
  $effect(() => {
    const tel = current.telemetry;
    if (!countingHeld) {
      untrack(() => setHeld([]));
      return;
    }
    if (!tel) return;
    const keys: number[] = [];
    for (let k = 0; k < 128; k++) if (keyIsDown(tel, k)) keys.push(k);
    untrack(() => setHeld(keys));
  });
  onMount(() =>
    onEvent((e) => {
      if (e.type !== "note" || !countingHeld) return;
      setHeld(e.on && e.velocity > 0 ? (held.includes(e.note) ? held : [...held, e.note].sort((a, b) => a - b)) : held.filter((k) => k !== e.note));
    }),
  );

  function recordingParts(m: Extract<StatusMessage, { id: "recording" }>): TextPart[] {
    return joined(
      overdubName !== null ? overdubParts(m.track, overdubName, null) : textParts("msg.recordingPianoRoll", { track: track(m.track), take: m.take }),
      held.length > 0 ? textParts("status.held", { keys: keysLabel(held, heldDrums) }) : [],
    );
  }

  // ---- Stopped with the piano roll open where there is no clip (mock F-PREmpty): 「1 ピアノ ・ クリップなし」 ----

  const noClipTrack = $derived.by((): TrackRef | null => {
    if (!pianoRollOpen || app.transport.state !== "stopped" || app.arrange.pianoRollClip !== null) return null;
    const tr = app.song.tracks.find((x) => x.id === app.song.keysTarget);
    return tr ? { number: tr.number, name: tr.name } : null;
  });

  /** The clip name and % of the earliest started "making the sound" (the % changes in the same frame as under the clip and in the panel). */
  const rendering = $derived.by((): { clip: string; percent: number } | null => {
    const job = shownWork.current.firstRendering;
    if (!job) return null;
    const clip = app.song.tracks.flatMap((x) => x.clips).find((c) => c.id === job.clip);
    return clip ? { clip: clip.name, percent: job.percent } : null;
  });

  /**
   * The export state (spec 2 "Export progress"). While exporting, the progress replaces the tag and text (before other messages).
   * Done, stopped and failed come after screen-only dialogs and drags, and before core messages. exportShown changes the % up to 2 times a second.
   */
  const run = $derived.by(() => {
    const r = exportShown.current?.run ?? null;
    return r ? runShown(r) : null;
  });
  const exporting = $derived(run?.kind === "running" ? run : null);
  /** When recording while exporting, 「● 録音中」 comes first. The export progress is short at the left end: 「書き出し 46 %」 (spec 3 94-84). */
  const exportAside = $derived(exporting !== null && (app.transport.state === "recording" || app.transport.state === "countIn"));
  /**
   * If a hint of flows 1 to 4 appears while the sound pack is being fetched, the progress moves to the left of the status line (spec 3 44-A; the form is 44-B of Opt8-PackShow).
   * Not shown while exporting, or while recording or in the count-in (the tag comes first).
   */
  const packShown = $derived.by(() => {
    if (exporting || app.transport.state === "recording" || app.transport.state === "countIn") return null;
    const f = packInStatus(app);
    return f ? percentOf(f.doneBytes, f.totalBytes) : null;
  });
  /** While a lasting hint is shown (no keyboard, for example), the question is also asked at the left of the status line (fetch / later). */
  const packAsk = $derived(!exporting && app.transport.state !== "recording" && app.transport.state !== "countIn" && askInStatus(app));

  const message = $derived.by((): Shown | null => {
    // The reading while dragging (spec 2 "Reading in place": the same value is also shown in the status line).
    if (dragReading.text !== null) return { parts: [{ text: dragReading.text, hole: "reading" }], strong: [], button: null };
    // The chord the draw tool would write where the cursor is (mock P10-ChordStamp).
    if (chordHover.text !== null) return { parts: [{ text: chordHover.text, hole: null }], strong: [], button: null };
    // 「meloet について」 is a screen-only dialog, so it is said here instead of a core message (spec 3 85).
    if (about.open) return { parts: textParts("status.dialogOpen", { dialog: t("about.title") }), strong: [], button: null };
    // Song dialogs are named by the dialog name (spec 3 89-6; the song name is not shown).
    const songDialog = app.workspace.songDialog;
    if (songDialog) return { parts: textParts("status.songDialogOpen", { dialog: t(SONG_DIALOG_NAMES[songDialog.kind]) }), strong: [], button: null };
    // While dragging a panel row (the value while dragging does not come back from core; mock F-ParamFew).
    if (paramUi.moving) return { parts: textParts("status.paramMoving", paramUi.moving), strong: [], button: null };
    if (run?.kind === "message")
      return { parts: run.parts, strong: [], button: run.folder ? { label: t("button.openFolder"), command: { type: "showExported" } } : null };
    const m = statusMessage;
    const usual = m === null || m.id === "summary";
    // While the 「窓」 list is open, it replaces the usual summary (user:2026-11-19, mock F-WindowOpen).
    if (windowsOpen && usual) {
      const loop = m?.loopBars ?? null;
      const open = windows.length === 1 ? textParts("msg.windowOpen", { name: windows[0]!.name }) : textParts("msg.windowsOpen", { count: windows.length });
      const parts = joined(loop ? textParts("status.loop", { from: loop.from, to: loop.to }) : [], open);
      return { parts, strong: [], button: null };
    }
    // While the heading's 「帯」 list is open (mock F8-FirstLane).
    const laneAdd = laneAddOpenFor();
    const laneTrack = laneAdd === null ? undefined : app.song.tracks.find((x) => x.id === laneAdd);
    if (laneTrack && usual) return { parts: textParts("status.laneAddOpen", { track: track(laneTrack) }), strong: [], button: null };
    // While searching parameters (the screen does the search; mock F-ParamSearch).
    if (searching && usual) return { parts: textParts("status.paramSearching", { query: paramUi.query.trim(), count: paramUi.matches }), strong: [], button: null };
    // While making the sound (spec 6 "Making the sound", mock F7-Stretch). The progress is audioWork, so the screen makes the text. It replaces the usual summary and comes after other messages.
    if (rendering && usual) return { parts: textParts("msg.audioRendering", rendering), strong: [], button: null };
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
    } else if ((m === null || m.id === "summary") && shelfParts) base = { parts: shelfParts, strong: [], button: null };
    else if (app.params.clips && (m === null || (m.id === "summary" && m.notes === null))) {
      // 2 or more audio clips are selected (mock F8-MultiClip).
      const loop = m?.id === "summary" ? m.loopBars : null;
      const count = app.params.clips.clips.length;
      base = { parts: joined(loop ? textParts("status.loop", { from: loop.from, to: loop.to }) : [], textParts("status.clipsSelected", { count })), strong: [], button: null };
    } else base = shown(m);
    if (stoppedSummary) {
      const muted = mutedParts();
      const keys = held.length > 0 ? textParts("status.held", { keys: keysLabel(held, heldDrums) }) : [];
      if (muted.length > 0 || keys.length > 0)
        return { parts: joined(base?.parts ?? [], muted, keys), strong: base?.strong ?? [], button: base?.button ?? null };
    }
    return base;
  });

  /** The tag at the far left (the owner's decision). rec for recording and the count-in. accent when stopped and trying a sound (mock F4-ShelfAudition). */
  const badge = $derived.by((): { text: string; rec: boolean; accent?: boolean } => {
    switch (app.transport.state) {
      case "playing":
        return { text: t("status.playing"), rec: false };
      case "recording":
        return { text: t("status.recording"), rec: true };
      case "countIn":
        return { text: t("status.countingIn"), rec: true };
      default:
        if (app.shelf.selectedSound !== null && app.shelf.loadingSound === null) return { text: t("status.auditioning"), rec: false, accent: true };
        return { text: t("status.stopped"), rec: false };
    }
  });

  // ---- 「窓 N ▾」 and the list above it (only the screen holds "open") ----

  const windows = $derived(app.windows.list);
  let windowsOpen = $state(false);
  let windowsButton = $state<HTMLButtonElement>();
  let windowsPopup = $state<HTMLElement>();
  let windowsAnchor = $state({ right: 0, bottom: 0 });
  const searching = $derived(paramUi.query.trim() !== "" && app.workspace.layout.rightFront === "params" && app.params.selected !== null);
  $effect(() => {
    if (windows.length === 0) windowsOpen = false;
  });
  function placeWindows(): void {
    if (!windowsButton) return;
    const r = windowsButton.getBoundingClientRect();
    const top = row?.getBoundingClientRect().top ?? r.top;
    windowsAnchor = { right: window.innerWidth - r.right, bottom: window.innerHeight - top };
  }
  function closeWindows(): void {
    const hadFocus = windowsPopup?.contains(document.activeElement) ?? false;
    windowsOpen = false;
    if (hadFocus) windowsButton?.focus();
  }
  function pickWindow(command: Command): void {
    send(command);
    closeWindows();
  }
  $effect(() => {
    if (!windowsOpen) return;
    placeWindows();
    window.addEventListener("resize", placeWindows);
    const close = () => closeWindows();
    const forget = menuOpened(close);
    const onDown = (ev: PointerEvent) => {
      const target = ev.target as Node | null;
      if (target && (windowsPopup?.contains(target) || windowsButton?.contains(target))) return;
      close();
    };
    document.addEventListener("pointerdown", onDown, true);
    return () => {
      forget();
      window.removeEventListener("resize", placeWindows);
      document.removeEventListener("pointerdown", onDown, true);
    };
  });

  const outputName = $derived(app.outputs.list.find((o) => o.id === app.outputs.chosen)?.name ?? null);
  const keyboardName = $derived(
    app.keyboards.inputStopped ? null : (app.keyboards.list.find((k) => k.id === app.keyboards.active)?.name ?? null),
  );
  const actual = $derived(app.audio.actualFrames);
  const requested = $derived(app.audio.requestedFrames);
  const scale = $derived(app.workspace.scale);
  const unfit = $derived(app.workspace.unfitScales);

  // The scale list. Only the screen holds "open"; core decides the scale (the screen only sends setScale).
  let open = $state(false);
  let zoomButton = $state<HTMLButtonElement>();
  let popup = $state<HTMLElement>();

  // The list appears in front of dialogs, so it is placed outside the .app stacking (in body), aligned to the top right of the status line.
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

  // Fitting the right side: measure the uncut widths with a hidden copy (ruler).
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
      // 「窓」 is never removed either, like the scale.
      scale: (width("scale") ?? 0) + (windows.length > 0 ? (width("windows") ?? 0) + gap : 0),
    });
  }

  onMount(() => {
    const observer = new ResizeObserver(() => measure());
    if (row) observer.observe(row);
    if (ruler) observer.observe(ruler);
    return () => observer.disconnect();
  });
  $effect(() => {
    void [badge, message, exporting, packShown, packAsk, outputName, keyboardName, actual, requested, scale, windows.length];
    measure();
  });
</script>

{#snippet left()}
  {#if exporting && !exportAside}
    <span class="exporting" data-test="export-run">
      <span class="accent">{t("export.running")}</span>
      <span class="bar"><span class="fill" style:transform="scaleX({exporting.percent / 100})"></span></span>
      <span class="percent m-num m-hi"><span class="size">{t("export.runningPercent", { percent: 99 })}</span><span class="in" data-test="export-percent"
          >{t("export.runningPercent", { percent: exporting.percent })}</span
        ></span>
      <span class="m-lbl message" data-test="export-rest">
        {#each exporting.rest as part, i (i)}{#if part.hole === "left" || part.hole === "length"}<span class="m-num">{part.text}</span
            >{:else}{part.text}{/if}{/each}
      </span>
      <button class="m-btn stop" data-test="export-cancel" onclick={() => send({ type: "cancelExport" })}>{t("button.cancel")}</button>
    </span>
  {:else if packShown !== null}
    <span class="exporting" data-test="pack-status">
      <span class="accent">{t("pack.fetching")}</span>
      <span class="bar pack-bar"><span class="fill" style:transform="scaleX({packShown / 100})"></span></span>
      <span class="percent m-num m-hi"><span class="size">{t("pack.fetchingPercent", { percent: 99 })}</span><span class="in" data-test="pack-percent"
          >{t("pack.fetchingPercent", { percent: packShown })}</span
        ></span>
      <button class="m-btn stop" data-test="pack-status-stop" onclick={() => send({ type: "stopPackFetch" })}>{t("button.cancel")}</button>
    </span>
  {:else if packAsk}
    <span class="exporting" data-test="pack-status-ask">
      <span class="accent">{t("pack.ask", { mb: packMb(app.sounds.packBytes) })}</span>
      <button class="m-btn stop" data-test="pack-status-fetch" onclick={() => send({ type: "fetchPack" })}>{t("button.fetchPack")}</button>
      <button class="m-btn stop" data-test="pack-status-later" onclick={() => send({ type: "packLater" })}>{t("button.packLater")}</button>
    </span>
  {:else}
    <span class:rec={badge.rec} class:accent={badge.accent} data-test="status-badge">{badge.text}</span>
  {/if}
  {#if message && (!exporting || exportAside)}
    <span class="m-lbl message">
      {#each message.parts as part, i (i)}
        {#if part.hole !== null}
          <span class:m-num={!NAME_HOLES.has(part.hole)} class:m-hi={message.strong.includes(part.hole)}>{part.text}</span>
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
  {#if exporting && exportAside}
    <span class="m-lbl export-aside" data-test="export-aside"
      >{#each textParts("status.exportShort", { percent: exporting.percent }) as part, i (i)}{#if part.hole !== null}<span class="m-num">{part.text}</span
        >{:else}{part.text}{/if}{/each}</span
    >
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
{#snippet windowsLabel()}<WindowMark /><span class="m-num">{t("windows.button", { count: windows.length })}</span>▾{/snippet}
{#snippet zoom()}<span class="m-num rate">{t("status.scale", { scale })}</span>▾{/snippet}

<div class="status-line">
  {#if open}
    <div class="popup" style:right="{anchor.right}px" style:bottom="{anchor.bottom}px" use:toBody bind:this={popup}>
      <ZoomMenu {scale} {unfit} {platform} onpick={pick} />
    </div>
  {/if}
  {#if windowsOpen}
    <div class="popup windows" style:right="{windowsAnchor.right}px" style:bottom="{windowsAnchor.bottom}px" use:toBody bind:this={windowsPopup}>
      <WindowsMenu list={windows} placeOf={windowPlace} onpick={pickWindow} />
    </div>
  {/if}
  <footer class="status" bind:this={row}>
    {@render left()}
    <span class="m-spacer"></span>
    {#if fit.output}<span class="m-lbl" data-test="status-output">{@render output()}</span>{/if}
    {#if fit.inputs}<span class="m-lbl" data-test="status-inputs">{@render inputs()}</span>{/if}
    {#if windows.length > 0}
      <button
        class="m-plain zoom"
        class:open={windowsOpen}
        aria-label={t("windows.heading")}
        aria-haspopup="menu"
        aria-expanded={windowsOpen}
        data-test="status-windows"
        bind:this={windowsButton}
        onclick={() => (windowsOpen = !windowsOpen)}>{@render windowsLabel()}</button
      >
    {/if}
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
    <span class="zoom wide" data-part="windows">{@render windowsLabel()}</span>
    <span class="zoom wide" data-part="scale">{@render zoom()}</span>
  </div>
</div>

<style>
  .status-line {
    position: relative;
    flex-shrink: 0;
  }
  /* Stacking: above dialogs (30), below term tips (1000). */
  .popup {
    position: fixed;
    z-index: 40;
    margin-right: var(--space-3);
  }
  .popup.windows {
    margin-right: 0;
  }
  .status {
    height: var(--size-status);
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: var(--space-4);
    padding: 0 var(--space-3);
    background: var(--color-gray-0);
    border-top: var(--line-hair) solid var(--color-line-thin);
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
  .status > :not(.message):not(.m-spacer):not(.exporting) {
    flex-shrink: 0;
  }

  /* Export progress (spec 2, mock F7-Exporting): 「書き出しています」 (accent), a 160 bar (background gray.4, accent), %, text, 「やめる」. */
  .exporting {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    min-width: 0;
    overflow: hidden;
  }
  .exporting > :not(.message) {
    flex-shrink: 0;
  }
  .bar {
    width: var(--part-exporting-bar);
    height: var(--mock-exporting-bar);
    background: var(--color-gray-4);
  }
  .bar.pack-bar {
    width: var(--part-packband-bar);
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--color-accent);
    transform-origin: left;
  }
  /* The % text is out of flow (the row is not laid out again when the text width changes; same as 22-22). The width is set by a hidden 「99 %」. */
  .percent {
    position: relative;
    display: inline-block;
  }
  .percent .size {
    visibility: hidden;
  }
  .percent .in {
    position: absolute;
    right: 0;
    top: 0;
  }
  .stop {
    height: var(--mock-exporting-stop);
  }

  /* Scale: normal (no background, border or padding; mock Main) / hover (text gray.10) / open (background gray.4, border gray.8; mock F-Zoom). */
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

  /* In the copy, measure with the open width (with padding and border). */
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

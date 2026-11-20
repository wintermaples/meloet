<!-- 状態行（モック F-Connect1〜3・F-BufferFixed・F-Zoom ・ F-WindowOpen のいちばん下の行）。
     右がわは 出口、入口、「窓 N ▾」（プラグインの窓が 1 つもなければ出さない。押すと上に一覧）、バッファ、拡大率。 -->
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
  } from "../bridge";
  import type { Scale } from "../design/tokens";
  import { everyGlFrame } from "../gl/slot";
  import { platformOf } from "../keys/keys";
  import { menuOpened } from "../lib/menu";
  import { term } from "../lib/tip.svelte";
  import { current, onEvent, send } from "../state/app.svelte";
  import { LIST_COMMA, STATUS_JOIN, t, type TextKey } from "../text/ja";
  import { about } from "./about/about.svelte";
  import { gridText, msText, noteLengthText, pitchClassName, scaleName, tempoText, termParts, textParts, type TextPart } from "./format";
  import { laneNameText, stepNameText, stepWhatText } from "./history/stepText";
  import { stripName } from "./mixer/stripName";
  import { paramName, valueText } from "./params/paramRows";
  import { paramUi } from "./params/paramUi.svelte";
  import { EFFECT_PARAMS } from "../bridge/types";
  import WindowMark from "./WindowMark.svelte";
  import WindowsMenu from "./WindowsMenu.svelte";
  import { keyCountsText, keysDrums, keysLabel, rollShowsDrums } from "./pianoroll/drums";
  import { soundNameText } from "./sounds";
  import { indexesOf } from "./pianoroll/geom";
  import { shown as shownNotes } from "./pianoroll/shown.svelte";
  import { sameKeys, soundingKeys } from "./pianoroll/sounding";
  import { dragReading } from "./pianoroll/view.svelte";
  import { shelfStatus, shelfUi } from "./tracks.svelte";
  import { statusFit, type StatusRightShown } from "./statusFit";
  import { shownTick } from "./time";
  import ZoomMenu from "./ZoomMenu.svelte";

  const app = $derived(current.state!);
  /** ピアノロールがドラムの行を見せている（選んだノートと鳴っている音を楽器の名前で）。 */
  const rollDrums = $derived(rollShowsDrums(app, shownNotes.clip, shownNotes.track));
  /** 鍵を弾くと鳴る先がドラム（押している音を楽器の名前で）。 */
  const heldDrums = $derived(keysDrums(app));

  interface Shown {
    parts: TextPart[];
    /** 値を gray.10 で出す穴。 */
    strong: string[];
    button: { label: string; command: Command } | null;
    /** 文の中で用語の説明をそえる言葉。 */
    term?: string;
  }

  const RECOVERY_TERM = "なおすためのファイル";
  /** 名前（ストリップ、プラグイン、エフェクト、パラメータ）の穴。文と同じ字で出す（モックの状態行）。ほかの穴（数、値、キー）は等幅。 */
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

  /** プラグインのいる所の名前（音源ならそのトラック、インサートならそのストリップ）。 */
  const slotName = (s: PluginSlotRef): string =>
    stripName(app.song, s.kind === "instrument" ? { kind: "track", id: s.track } : s.insert.strip, app.library);

  const effectName = (e: EffectName) => (e.kind === "builtin" ? t(`effect.${e.effect}`) : e.name);
  const insertStrip = (i: InsertRef) => stripName(app.song, i.strip, app.library);
  /** 「2 ベース の コンプレッサー 」（うしろに空白）。 */
  const insertTarget = (i: InsertRef, e: EffectName) => t("status.insertTarget", { strip: insertStrip(i), effect: effectName(e) });
  /** 6 章の窓の文の「（名前）」: 居場所とプラグインの名前、うしろに空白。棚でためし弾き中の子は名前だけ（design-notes A-17）。 */
  const windowName = (name: string, slot: PluginSlotRef | null) => `${slot ? t("status.windowPlace", { strip: slotName(slot), name }) : name} `;
  /** プラグインが言ってきた名前は、うしろの「は」「の」との間に空白を置く（「Surge XT の」）。 */
  const spaced = (name: string) => `${name} `;
  const MINUS = "−";
  const plainNumber = (n: number) => String(n).replace("-", MINUS);

  /** 打ちこんだ値の文に出すパラメータの名前（最初から入っている物は表から、プラグインは見せている表から）。 */
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

  /** 打ちこむ前の値。最初から入っているエフェクトは整えていない数（"-24"）で来るので、行の値と同じ単位とけたで。プラグインは来た文字のまま。 */
  function keptText(p: ParamRef, kept: string): string {
    if (p.kind !== "builtin") return kept;
    const spec = EFFECT_PARAMS[p.effect][p.param];
    const n = Number(kept.replace("−", "-"));
    return spec && kept.trim() !== "" && Number.isFinite(n) ? valueText(p.effect, spec, n) : kept;
  }

  /** 「レシオは 1〜20 なので、20.0 : 1 にしました。」の 3 つ。 */
  function clampedParts(to: ParamEnd): { param: string; range: string; value: string } {
    if (to.kind === "plugin") return { param: spaced(to.name), range: to.range, value: to.value };
    const spec = EFFECT_PARAMS[to.effect][to.param];
    if (!spec) return { param: "", range: "", value: "" };
    return { param: paramName(to.effect, to.param), range: `${plainNumber(spec.min)}〜${plainNumber(spec.max)}`, value: valueText(to.effect, spec, to.value) };
  }

  /** プラグインの窓の居場所（「1 ピアノ ・ 音源」「A リバーブ ・ インサート 1」「棚 ・ ためし弾き」）。 */
  function windowPlace(w: PluginWindowView): string {
    const slot = w.slot;
    if (!slot) return t("windows.shelfAudition");
    if (slot.kind === "instrument") return t("params.whereInstrument", { strip: slotName(slot) });
    return t("params.where", { strip: slotName(slot), slot: slot.insert.slot + 1 });
  }

  /** カウントイン中の「あと {beats} 拍」。計器がカウントインを言っていれば、その残りの拍。 */
  function beatsLeft(fallback: number): number {
    const p = current.telemetry?.position;
    return p?.state === "countIn" ? p.countInBeatsLeft : fallback;
  }

  /** 「1 つは SF2 ではないので、足していません。」（持ってきたファイルのうち、足さなかった物があるとき）。 */
  const skippedParts = (skipped: number): TextPart[] => (skipped > 0 ? textParts("sf2.skipped", { count: skipped }) : []);

  /** 「ピアノ ・ 鍵盤 と ギター」「ピアノ ・ 鍵盤、ギター と ベース」「ピアノ ・ 鍵盤 ほか 4 つ」（仕様書 6 章「棚をひらいている（見出し）」）。 */
  function groupsText(names: readonly string[]): string {
    if (names.length >= 5) return t("status.shelfGroupsMore", { first: names[0]!, count: names.length - 1 });
    if (names.length === 1) return names[0]!;
    return t("status.shelfGroupsLast", { first: names.slice(0, -1).join(LIST_COMMA), last: names.at(-1)! });
  }

  /** 文の部分どうしを「 ・ 」でつなぐ（仕様書 6 章「止まっているときの状態行の左」）。 */
  function joined(...pieces: TextPart[][]): TextPart[] {
    return pieces.filter((p) => p.length > 0).flatMap((p, i) => (i === 0 ? p : [{ text: STATUS_JOIN, hole: null }, ...p]));
  }

  /**
   * 「選択 3 ノート ・ D4 E4 F4 ・ クオンタイズ済み（もとの位置へ戻せます）」（モック A-PianoRoll）。同じ名前はまとめて「スネア ×2」（仕様書 3 章 85）。
   * 名前が 5 つ以上なら、4 つのあとに「…」。
   */
  function notesParts(n: NoteSelection): TextPart[] {
    const values = { count: n.count, keys: keyCountsText(n.names.slice(0, 4), rollDrums) };
    return joined(
      textParts(n.names.length > 4 ? "status.notesSelectedMore" : "status.notesSelected", values),
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
    const place = (bar: number, beat: number) => (beat === 1 ? t("timeline.placeBar", { bar }) : t("timeline.placeBeat", { bar, beat }));
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
      case "tempoRampSelected":
        if (m.toTempoBpm === m.tempoBpm) return plain("msg.tempoPointSelected", { place: place(m.bar, m.beat), tempo: tempoText(m.tempoBpm) });
        return plain("msg.tempoRampSelected", {
          place: place(m.bar, m.beat),
          tempo: tempoText(m.tempoBpm),
          toPlace: place(m.toBar, m.toBeat),
          toTempo: tempoText(m.toTempoBpm),
          direction: t(m.toTempoBpm > m.tempoBpm ? "timeline.rampFaster" : "timeline.rampSlower"),
        });
      case "tempoPointSelected":
        return plain("msg.tempoPointSelected", { place: place(m.bar, m.beat), tempo: tempoText(m.tempoBpm) });
      case "meterChangeSelected":
        return plain("msg.meterChangeSelected", { place: place(m.bar, 1), signature: `${m.meterNum}/${m.meterDen}` });
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
        return plain("msg.instrumentReplaced", { track: track(m.track), instrument: soundNameText(m.sound), undoKey: modKey(t("key.undo")) });
      case "soundLoading":
        return plain("msg.auditionLoading", { name: soundNameText(m.sound) });
      case "pluginLoading":
        return { parts: joined(textParts("plugin.loading", { name: m.name }), textParts("status.cancelKey", { key: t("key.esc") })), strong: [], button: null };
      case "sf2Dragging":
        return plain("sf2.dragging");
      case "notSf2Dragging":
        return plain("sf2.notSf2Dragging");
      // 「音色 128 ・ ドラムのキット 9」は「。」で終わらないので、足さなかった物の文とは「 ・ 」でつなぐ（design-notes A-19）。
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
        // 戻りは名前にエフェクトが入っているので「A リバーブの「…」」、トラックとマスターは「2 ベース ・ コンプレッサーの「…」」（仕様書 3 章 86-10）。
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
        return plain("msg.effectRemoved", { strip: insertStrip(m.insert), slot: m.insert.slot + 1, effect: effectName(m.effect), undoKey: modKey(t("key.undo")) });
      case "insertsReordered":
        return plain("msg.insertsReordered", { strip: stripName(app.song, m.strip, app.library), order: m.order.map(effectName).join(" → ") });
      case "insertReplaced":
        return plain("msg.insertReplaced", { strip: insertStrip(m.insert), slot: m.insert.slot + 1, effect: effectName(m.effect), undoKey: modKey(t("key.undo")) });
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

  /**
   * 棚がひらいているときの、いつものまとめの代わり（仕様書 6 章「棚をひらいている（見出し）」、モック F5-ShelfSearch）。
   * 見出しの文は、棚をひらいた時と、見出しをひらく ・ たたむ時に言い、core の知らせが変わるまで出す。さがしている間の文は、いつも。
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
  /** 見出しの文を言った時の core の知らせ。知らせが変わったら、もう言わない。 */
  let shelfMark = $state.raw<{ message: StatusMessage | null } | null>(null);
  $effect(() => {
    void shelfUi.open;
    void shelfUi.view;
    const front = shelfFront;
    untrack(() => (shelfMark = front ? { message: app.status.message } : null));
  });
  const shelfAnnounced = $derived(shelfMark !== null && shelfMark.message === app.status.message);

  // ---- ピアノロールをひらいて録音しているとき（仕様書 6 章「録音中 ・ ピアノロール」）: 「1 ピアノ へ録音 ・ テイク 3 ・ 押している音 C3 E3 G3 C4」 ----

  const recordingInPianoRoll = $derived(pianoRollOpen && app.status.message?.id === "recording");
  /** 止まっているときも、鍵盤を押している間は終わりに「・ 押している音 キック スネア」（仕様書 3 章 85）。 */
  const stoppedSummary = $derived(
    app.transport.state === "stopped" && (app.status.message === null || app.status.message.id === "summary"),
  );
  const countingHeld = $derived(recordingInPianoRoll || stoppedSummary);

  /** 押している鍵（入力計と同じ物: 計器で受け、鍵の on/off は計器を待たずに当てる）。録音中にピアノロールをひらいている間と、止まっている間だけ数える。 */
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
      textParts("msg.recordingPianoRoll", { track: track(m.track), take: m.take }),
      held.length > 0 ? textParts("status.held", { keys: keysLabel(held, heldDrums) }) : [],
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
    // 「meloet について」は画面だけが持つダイアログなので、core の知らせの代わりにここで言う（仕様書 3 章 85）。
    if (about.open) return { parts: textParts("status.dialogOpen", { dialog: t("about.title") }), strong: [], button: null };
    // 曲のダイアログは、ダイアログの名前で（仕様書 3 章 89-6。曲の名前は出さない）。
    const songDialog = app.workspace.songDialog;
    if (songDialog) return { parts: textParts("status.songDialogOpen", { dialog: t(SONG_DIALOG_NAMES[songDialog.kind]) }), strong: [], button: null };
    // 欄の行をドラッグしている間（ドラッグ中の値は core に戻らない。モック F-ParamFew）。
    if (paramUi.moving) return { parts: textParts("status.paramMoving", paramUi.moving), strong: [], button: null };
    const m = app.status.message;
    const usual = m === null || m.id === "summary";
    // 「窓」の一覧をひらいている間は、いつものまとめに代えて（user:2026-11-19、モック F-WindowOpen）。
    if (windowsOpen && usual) {
      const loop = m?.loopBars ?? null;
      const open = windows.length === 1 ? textParts("msg.windowOpen", { name: windows[0]!.name }) : textParts("msg.windowsOpen", { count: windows.length });
      const parts = joined(loop ? textParts("status.loop", { from: loop.from, to: loop.to }) : [], open);
      return { parts, strong: [], button: null };
    }
    // パラメータをさがしている間（さがすのは画面。モック F-ParamSearch）。
    if (searching && usual) return { parts: textParts("status.paramSearching", { query: paramUi.query.trim(), count: paramUi.matches }), strong: [], button: null };
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
    else base = shown(m);
    if (stoppedSummary) {
      const muted = mutedParts();
      const keys = held.length > 0 ? textParts("status.held", { keys: keysLabel(held, heldDrums) }) : [];
      if (muted.length > 0 || keys.length > 0)
        return { parts: joined(base?.parts ?? [], muted, keys), strong: base?.strong ?? [], button: base?.button ?? null };
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
        if (app.shelf.selectedSound !== null && app.shelf.loadingSound === null) return { text: t("status.auditioning"), rec: false, accent: true };
        return { text: t("status.stopped"), rec: false };
    }
  });

  // ---- 「窓 N ▾」と、その上の一覧（画面だけが持つ「ひらいている」） ----

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
      // 「窓」も拡大率と同じく消さない。
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
    void [badge, message, outputName, keyboardName, actual, requested, scale, windows.length];
    measure();
  });
</script>

{#snippet left()}
  <span class:rec={badge.rec} class:accent={badge.accent} data-test="status-badge">{badge.text}</span>
  {#if message}
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
  /* 重なり: ダイアログ（30）より上、用語の説明（1000）より下。 */
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

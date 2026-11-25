// The text of the undo history rows and of the undo and redo status line (spec 3 (Y19), spec 6). Rust sends the kind of step name and
// the values that fill 「（何を）」, and the text is made here. Pure functions that do not know the DOM.

import { EFFECT_PARAMS } from "../../bridge/types";
import type { EffectName, HistoryRow, LaneName, LibraryView, SongView, StepName, StepPlace, StepWhat, StripName, TrackRef } from "../../bridge";
import { fill, stepNameText as rawStepName, t, textLanguage } from "../../text";
import { gridText, tempoText } from "../format";
import { noteName } from "../meterFormat";
import { faderText, panText } from "../mixer/scale";
import { stripName } from "../mixer/stripName";
import { paramName, valueNumberText, valueText } from "../params/paramRows";
import { controlName } from "../pianoroll/ccFields";
import { soundNameText } from "../sounds";

/** The return name (「A リバーブ」) is made from the current song. */
export interface StepContext {
  song: SongView;
  library: LibraryView;
}

/** A name from a plugin gets a space before the following 「の」 (「Surge XT の」). */
const spaced = (name: string) => (textLanguage() === "en" ? name : `${name} `);
const trackLabel = (r: TrackRef) => t("track.label", { number: r.number, name: r.name });
const effectText = (e: EffectName) => (e.kind === "builtin" ? t(`effect.${e.effect}`) : e.name);
const busText = (bus: "a" | "b") => t(bus === "a" ? "mixer.sendA" : "mixer.sendB");
const placeText = (bar: number, beat: number) =>
  bar === 1 && beat === 1 ? t("timeline.placeHead") : beat === 1 ? t("timeline.placeBar", { bar }) : t("timeline.placeBeat", { bar, beat });

function stripText(strip: StripName, ctx: StepContext): string {
  if (strip.kind === "track") return trackLabel(strip.track);
  if (strip.kind === "master") return t("mixer.master");
  return stripName(ctx.song, { kind: "return", bus: strip.bus }, ctx.library);
}

/** The name of what the band moves (「音量」「コンプレッサーの「しきい値」」). */
export function laneNameText(name: LaneName): string {
  switch (name.kind) {
    case "volume":
      return t("lane.volume");
    case "pan":
      return t("mixer.pan");
    case "send":
      return t("lane.send", { bus: busText(name.bus) });
    case "mute":
      return t("lane.mute");
    case "builtin":
      return t("lane.param", { effect: t(`effect.${name.effect}`), param: paramName(name.effect, name.param) });
    case "plugin":
      return name.param === null
        ? t("lane.pluginParam", { plugin: spaced(name.plugin) })
        : t("lane.param", { effect: spaced(name.plugin), param: name.param });
    case "knob":
      return t(`knob.${name.knob}`);
  }
}

/** The step name (「ノートを動かす」「コンプレッサーの「しきい値」を変える」). */
export function stepNameText(name: StepName): string {
  const text = rawStepName(name.kind);
  switch (name.kind) {
    case "setParam":
      return fill(text, { effect: t(`effect.${name.effect}`), param: paramName(name.effect, name.param) });
    case "setPluginParam":
      return name.param === null
        ? t("history.pluginParam", { plugin: spaced(name.plugin) })
        : fill(text, { effect: spaced(name.plugin), param: name.param });
    case "pluginWindow":
      return fill(text, { name: name.plugin });
    case "batch":
      return fill(text, { count: name.count });
    default:
      return text;
  }
}

/** Where a value changed, was added or was removed (「5 小節目 96 → 120」「5 小節目 120」「5 小節目」). */
function atPlace(place: string, from: string | null, to: string | null): string {
  if (from !== null && to !== null) return t("history.changeAt", { place, from, to });
  const value = to ?? from;
  return value === null ? place : t("history.valueAt", { place, value });
}

/** 「（何を）」 (「3 ノート」「4 パッド ・ −10.5 → −12.0 dB」). Null for a step with nothing. */
export function stepWhatText(name: StepName, what: StepWhat, ctx: StepContext): string | null {
  switch (what.kind) {
    case "none":
      return null;
    case "notes":
      return t("history.notes", { count: what.count });
    case "points":
      return t("history.points", { count: what.count });
    case "clips":
      return t("history.clips", { count: what.count });
    case "take":
      return t("history.take", { number: what.number });
    case "takeSwitch":
      return t("history.takeSwitch", { from: what.from, to: what.to });
    case "track":
      return trackLabel(what.track);
    case "strip":
      return name.kind === "mute" ? t("history.muteOn", { strip: stripText(what.strip, ctx) }) : stripText(what.strip, ctx);
    case "place":
      return placeText(what.bar, what.beat);
    case "name":
      return what.name;
    case "renamed":
      return t("history.change", { from: what.from, to: what.to });
    case "tempo": {
      const text = (bpm: number | null) => (bpm === null ? null : tempoText(bpm));
      return atPlace(placeText(what.bar, what.beat), text(what.fromBpm), text(what.toBpm));
    }
    case "meter": {
      const text = (m: { num: number; den: number } | null) => (m === null ? null : `${m.num}/${m.den}`);
      return atPlace(placeText(what.bar, 1), text(what.from), text(what.to));
    }
    case "moved":
      return t("history.change", { from: placeText(what.bar, what.beat), to: placeText(what.toBar, what.toBeat) });
    case "bars":
      return t("history.bars", { from: what.span.from, to: what.span.to });
    case "gain":
      return t("history.gain", { strip: stripText(what.strip, ctx), from: faderText(what.fromDb), to: faderText(what.toDb) });
    case "pan":
      return t("history.pan", { strip: stripText(what.strip, ctx), from: panText(what.from), to: panText(what.to) });
    case "send":
      return t("history.send", { strip: trackLabel(what.track), bus: busText(what.bus), from: faderText(what.fromDb), to: faderText(what.toDb) });
    case "param": {
      // 「−24.0 → −18.0 dB」: the unit only on the right (spec 3 (92-3)).
      const spec = name.kind === "setParam" ? EFFECT_PARAMS[name.effect][name.param] : undefined;
      if (!spec || name.kind !== "setParam") return t("history.change", { from: String(what.from), to: String(what.to) });
      return t("history.change", { from: valueNumberText(name.effect, spec, what.from), to: valueText(name.effect, spec, what.to) });
    }
    case "effect":
      return effectText(what.effect);
    case "replaced":
      return t("history.change", { from: effectText(what.from), to: effectText(what.to) });
    case "sound":
      return soundNameText(what.sound);
    case "lane":
      return what.points === null ? laneNameText(what.target) : t("history.lanePoints", { target: laneNameText(what.target), count: what.points });
    case "missing":
      return t("history.missing", { tracks: what.tracks, inserts: what.inserts });
    case "note":
      return noteName(what.key);
    case "quantized":
      return t("history.quantized", { grid: gridText(what.division, what.triplet), count: what.count });
    case "velocity":
      return what.from === null || what.to === null
        ? t("history.notes", { count: what.count })
        : t("history.velocity", { count: what.count, from: what.from, to: what.to });
    case "controlPoints":
      return what.control === null
        ? t("history.points", { count: what.count })
        : t("history.controlPoints", { control: controlName(what.control), count: what.count });
    case "color":
      return t("history.color", { track: trackLabel(what.track), color: what.color });
    case "soundReplaced":
      return t("history.soundReplaced", { track: trackLabel(what.track), from: soundNameText(what.from), to: soundNameText(what.to) });
    case "vocalInput":
      return t("history.soundReplaced", { track: trackLabel(what.track), from: t(`vocal.input.${what.from}`), to: t(`vocal.input.${what.to}`) });
    // 「歌 テイク 1 ・ −3.0 → 0.0 dB」: the same form as a mixer gain step (provisional; spec 3 (92-3) has no clip gain).
    case "clipGain":
      return t("history.gain", { strip: what.name, from: faderText(what.fromDb), to: faderText(what.toDb) });
    case "soundKnob":
      return t("history.soundKnob", {
        track: trackLabel(what.track),
        knob: t(`knob.${what.knob}`),
        from: String(Math.round(what.from)),
        to: String(Math.round(what.to)),
      });
    case "section":
      return t("history.section", { name: what.name, bars: what.bars });
  }
}

/** The 「どこで」 on the right of the row (「ピアノロール」「4 パッド」「5 Surge XT ・ プラグインの窓」). Null for a step that names no place. */
export function stepPlaceText(place: StepPlace | null, ctx: StepContext): string | null {
  switch (place?.kind) {
    case undefined:
      return null;
    case "arrange":
      return t("band.arrange");
    case "pianoRoll":
      return t("band.pianoRoll");
    case "mixer":
      return t("band.mixer");
    case "track":
      return trackLabel(place.track);
    case "pluginWindow":
      return t("history.placePluginWindow", { strip: stripText(place.strip, ctx) });
    case "master":
      return t("mixer.master");
  }
}

/**
 * The 「（何を）」 of a row (spec 3 (94-6)): a step whose right field shows the track name does not write the track name (replacing the instrument is 「グランドピアノ → オルガン」).
 * A window step is 「プラグインの窓」. Steps whose right field is the arrange or mixer (color, gain, pan, send, mute, vocal input) keep the name (the same as the status line).
 */
export function rowWhatText(row: HistoryRow, ctx: StepContext): string | null {
  if (row.name.kind === "pluginWindow") return t("history.pluginWindowWhat");
  if (row.place?.kind === "track" && row.what.kind === "soundReplaced")
    return t("history.change", { from: soundNameText(row.what.from), to: soundNameText(row.what.to) });
  if (row.place?.kind === "track" && row.what.kind === "track" && row.what.track.number === row.place.track.number) return null;
  if (row.place?.kind === "track" && row.what.kind === "soundKnob")
    return t("history.changeAt", {
      place: t(`knob.${row.what.knob}`),
      from: String(Math.round(row.what.from)),
      to: t("param.value.percent", { value: String(Math.round(row.what.to)) }),
    });
  return stepWhatText(row.name, row.what, ctx);
}

/** The row text (「「ノートを動かす」・ 3 ノート」). */
export function historyRowText(row: HistoryRow, ctx: StepContext): string {
  const name = stepNameText(row.name);
  const what = rowWhatText(row, ctx);
  return what === null ? t("history.rowBare", { name }) : t("history.row", { name, what });
}

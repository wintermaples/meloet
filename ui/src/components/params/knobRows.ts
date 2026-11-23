// The four rows of a sound that comes with meloet (round 12 E3): 「明るさ」「立ち上がり」「のび」「ゆれ」, each 0 to 100 %.
// A knob's place in SOUND_KNOBS is its row and its `param` (a lane moves it as `{ kind: "instrument", param }`).

import {
  SOUND_KNOB_DEFAULTS,
  SOUND_KNOBS,
  type EffectParam,
  type InstrumentKind,
  type SoundKnob,
} from "../../bridge/types";
import { t } from "../../text";

/** Which knob of which track's sound a row moves; `instrument` is the sound the row was shown for. */
export interface KnobRef {
  track: number;
  instrument: InstrumentKind;
  knob: SoundKnob;
}

/** A knob as a parameter of the panel: a bar from 0 to 100 %. */
export function knobSpec(knob: SoundKnob): EffectParam {
  const at = SOUND_KNOBS.indexOf(knob);
  return {
    id: knob,
    group: null,
    shape: "continuous",
    steps: [],
    unit: "percent",
    min: 0,
    max: 100,
    default: SOUND_KNOB_DEFAULTS[at] ?? 0,
  };
}

/** 「35 %」. */
export function knobText(value: number): string {
  return t("param.value.percent", { value: String(Math.round(value)) });
}

/** A typed value (「35」「35 %」「35%」), or null when it is not a number. Rust puts it into 0 to 100. */
export function typedKnob(text: string): number | null {
  const s = text.replace(/[%％\s]/g, "").replace("−", "-");
  const v = s === "" ? NaN : Number(s);
  return Number.isFinite(v) ? v : null;
}

/** The row of a lane's knob, or null for a parameter that is not one. */
export function knobAt(param: number): SoundKnob | null {
  return SOUND_KNOBS[param] ?? null;
}

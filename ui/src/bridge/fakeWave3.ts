// The fake's part of protocol 33 (round 12, waves 3 and 4). Until each feature is built, the fake takes its intents
// and changes nothing, as Rust does (`// round12 stub: <ID>`).
import { INSTRUMENT_GROUPS, SOUND_KNOB_DEFAULTS, SOUND_KNOBS } from "./types";
import type { Arp, Command, InstrumentKind, SelectedParams, SoundKnobsView } from "./types";

/** E2: the arpeggio a track starts with (off). */
export const FAKE_ARP_OFF: Arp = { on: false, shape: "up", rate: "sixteenth", triplet: false, octaves: 1, hold: false };

/** E3: which knobs a sound that comes with meloet uses: a drum kit only 「明るさ」 and 「のび」. */
export function fakeKnobsUsable(instrument: InstrumentKind): [boolean, boolean, boolean, boolean] {
  const drums = INSTRUMENT_GROUPS.find((g) => g.group === "drums")?.instruments.includes(instrument) ?? false;
  return drums ? [true, false, true, false] : [true, true, true, true];
}

/** E3: the knobs of a sound as they start. */
export function fakeKnobs(instrument: InstrumentKind): SoundKnobsView {
  return { values: [...SOUND_KNOB_DEFAULTS] as SoundKnobsView["values"], usable: fakeKnobsUsable(instrument), row: null };
}

/** E3 `setSoundKnob` on the selected sound (the fake keeps the knobs of the selected sound only, as the values of the
 * selected insert): a drag changes nothing here, a let go or a typed value is put into 0 to 100. */
export function fakeSetSoundKnob(selected: SelectedParams | null, c: Extract<Command, { type: "setSoundKnob" }>): void {
  if (selected?.kind !== "sound" || selected.track !== c.track || !selected.knobs) return;
  if (selected.sound.kind !== "builtin" || selected.sound.instrument !== c.instrument) return;
  const at = SOUND_KNOBS.indexOf(c.knob);
  if (!c.commit || !selected.knobs.usable[at] || Number.isNaN(c.value)) return;
  selected.knobs.values[at] = Math.max(0, Math.min(100, c.value));
}

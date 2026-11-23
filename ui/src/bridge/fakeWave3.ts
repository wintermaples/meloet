// The fake's part of protocol 33 (round 12, waves 3 and 4). Until each feature is built, the fake takes its intents
// and changes nothing, as Rust does (`// round12 stub: <ID>`).
import type { FakeState } from "./fake";
import { INSTRUMENT_GROUPS, SOUND_KNOB_DEFAULTS, SOUND_KNOBS } from "./types";
import type { Arp, Change, Command, InstrumentKind, SelectedParams, SoundKnobsView } from "./types";

/** E2: the arpeggio a track starts with (off). */
export const FAKE_ARP_OFF: Arp = { on: false, shape: "up", rate: "sixteenth", triplet: false, octaves: 1, hold: false };

/** E7 「表示 ▸ 音の動きを見せる」: a setting, not a step. `null`: not this command. */
export function reduceSpectrum(state: FakeState, command: Command): Change[] | null {
  if (command.type !== "setSpectrum") return null;
  if (state.workspace.spectrum === command.on) return [];
  return [{ section: "workspace", value: { ...state.workspace, spectrum: command.on } }];
}

/** dB of each band under the master's peak: a song with a full low end and highs that fall away. */
const SONG_SHAPE = [-10, -6, -4, -3, -5, -4, -6, -8, -7, -9, -10, -12, -11, -13, -15, -14, -17, -19, -18, -21, -24, -27, -31, -36];

/** E7: the telemetry's 24 bands (−72..0 dB as 0..255) of a scene whose master sounds at `master` (1.0 = 0 dBFS); none when it is silent. */
export function fakeSpectrum(master: readonly [number, number] | undefined): number[] | undefined {
  const peak = Math.max(master?.[0] ?? 0, master?.[1] ?? 0);
  if (!(peak > 0)) return undefined;
  const db = 20 * Math.log10(peak);
  return SONG_SHAPE.map((d) => Math.round(Math.min(Math.max((db + d + 72) / 72, 0), 1) * 255));
}

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

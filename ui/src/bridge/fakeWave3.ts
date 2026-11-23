// The fake's part of protocol 33 (round 12, waves 3 and 4). Until each feature is built, the fake takes its intents
// and changes nothing, as Rust does (`// round12 stub: <ID>`).
import type { FakeState } from "./fake";
import type { Arp, Change, Command } from "./types";

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

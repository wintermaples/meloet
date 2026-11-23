// The fake's part of protocol 33 (round 12, waves 3 and 4). Until each feature is built, the fake takes its intents
// and changes nothing, as Rust does (`// round12 stub: <ID>`).
import type { FakeState } from "./fake";
import { ARP_OCTAVES_MAX, INSTRUMENT_GROUPS, LANGUAGES, SOUND_KNOB_DEFAULTS, SOUND_KNOBS } from "./types";
import type { Arp, Change, Command, HistoryRow, InstrumentKind, Language, SelectedParams, SoundKnobsView, StatusMessage } from "./types";

/** E2: the arpeggio a track starts with (off). */
export const FAKE_ARP_OFF: Arp = { on: false, shape: "up", rate: "sixteenth", triplet: false, octaves: 1, hold: false };

/**
 * E2 「アルペジオを変える」: one step with its history row, as Rust takes it. Nothing for an unknown track, a vocal track, the
 * same setting, or while counting in or recording. `null`: not this command.
 */
export function reduceArp(state: FakeState, command: Command): { changes: Change[]; row: HistoryRow | null } | null {
  if (command.type !== "setArp") return null;
  const nothing = { changes: [], row: null };
  const busy = state.transport.state === "recording" || state.transport.state === "countIn";
  const index = state.song.tracks.findIndex((t) => t.id === command.track);
  const track = state.song.tracks[index];
  if (busy || !track || track.kind.kind === "vocal") return nothing;
  const arp: Arp = { ...command.arp, octaves: Math.min(Math.max(Math.round(command.arp.octaves), 1), ARP_OCTAVES_MAX) };
  if (JSON.stringify(arp) === JSON.stringify(track.arp)) return nothing;
  const song = { ...state.song, tracks: [...state.song.tracks] };
  song.tracks[index] = { ...track, arp };
  Object.assign(song, { undoCount: song.undoCount + 1, redoCount: 0, file: "changed" });
  const ref = { number: track.number, name: track.name };
  const row: HistoryRow = { index: song.undoCount, step: song.undoCount, name: { kind: "setArp" }, what: { kind: "track", track: ref }, place: { kind: "track", track: ref } };
  return { changes: [{ section: "song", value: song }], row };
}

/** E7 「表示 ▸ 音の動きを見せる」: a setting, not a step. `null`: not this command. */
export function reduceSpectrum(state: FakeState, command: Command): Change[] | null {
  if (command.type !== "setSpectrum") return null;
  if (state.workspace.spectrum === command.on) return [];
  return [{ section: "workspace", value: { ...state.workspace, spectrum: command.on } }];
}

/**
 * D4 「ことば / Language」: a setting, not a step; refused while counting in or recording. The fake keeps the choice in the
 * address (`?lang=`), so the page that loads itself again starts in it. `null`: not this command.
 */
export function reduceLanguage(state: FakeState, command: Command): Change[] | null {
  if (command.type !== "setLanguage") return null;
  const busy = state.transport.state === "recording" || state.transport.state === "countIn";
  if (busy || state.workspace.language === command.language) return [];
  if (typeof location !== "undefined" && typeof history !== "undefined") {
    const url = new URL(location.href);
    url.searchParams.set("lang", command.language);
    history.replaceState(history.state, "", url);
  }
  return [{ section: "workspace", value: { ...state.workspace, language: command.language } }];
}

/** `?lang=en`: the fake starts in that language. */
export function languageFromUrl(): Language | null {
  if (typeof location === "undefined") return null;
  const asked = new URLSearchParams(location.search).get("lang");
  return LANGUAGES.find((l) => l === asked) ?? null;
}

/**
 * M3 「モノで聞く」: only for listening, not a step, not a setting. While it is on the status line says 「モノで聞いています」 (Rust says it
 * when it is turned on, and in place of the summary while it stays on). `null`: not this command.
 */
export function reduceMonoListen(state: FakeState, command: Command): Change[] | null {
  if (command.type !== "setMonoListen") return null;
  if (state.workspace.monoListen === command.on) return [];
  const said = state.status.message?.id === "monoListening";
  const message: StatusMessage | null = command.on
    ? { id: "monoListening" }
    : said
      ? { id: "summary", loopBars: null, nothingSelected: true, notes: null }
      : state.status.message;
  return [
    { section: "workspace", value: { ...state.workspace, monoListen: command.on } },
    { section: "status", value: { message } },
  ];
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

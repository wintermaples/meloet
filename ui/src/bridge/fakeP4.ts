// The fake's drum patterns of the shelf (round 12, P4): the rows, ▶ and 「ここに置く」, as Rust does them. The rows are a
// copy of tools/patterns/drums.json (a test compares them); the notes the fake writes are a plain 8 beat for every
// pattern, enough for pictures. The fake has no undo and no audio: ▶ only shows the row playing.

import type { FakeState, FakeTake, FakeTrack } from "./fake";
import type { Change, ClipView, Command, HistoryRow, PatternView, StatusMessage } from "./types";
import { TempoMap } from "../time/tempoMap";

/** The rows of 「パターン」, in their order. */
export const FAKE_PATTERNS: readonly PatternView[] = [
  ...["eightBeat", "eightBeatOpen", "sixteenBeat", "fourOnTheFloor", "shuffle", "halfTime", "rideEightBeat", "ballad"].map(
    (id): PatternView => ({ id, group: "basic", bars: 1 }),
  ),
  ...["snareRoll", "tomFill", "crashStop", "smallFill"].map((id): PatternView => ({ id, group: "fill", bars: 1 })),
];

const TPQ = 1920;
const BAR = 4 * TPQ;

/** The fake's notes of every pattern, from its head: hi-hat on the eighths, kick on 1 and 3, snare on 2 and 4. */
function fakeHits(): FakeTake["notes"] {
  const hats = Array.from({ length: 8 }, (_, i) => ({ start: (i * TPQ) / 2, length: 240, key: 42, velocity: i % 2 ? 72 : 100 }));
  const kick = [0, 2].map((b) => ({ start: b * TPQ, length: 240, key: 36, velocity: 112 }));
  const snare = [1, 3].map((b) => ({ start: b * TPQ, length: 240, key: 38, velocity: 110 }));
  return [...hats, ...kick, ...snare];
}

/** Whether the track is a drum track: a kit of meloet or of an SF2 file (a plugin never is). */
export function fakeIsDrums(track: Pick<FakeTrack, "sound"> | undefined): boolean {
  const s = track?.sound;
  if (!s) return false;
  if (s.kind === "builtin") return s.instrument === "studioKit" || s.instrument === "electroKit" || s.instrument === "brushKit";
  return s.kind === "sf2" && s.family === "drumKit";
}

/**
 * `shelfSelectPattern`, `playPattern`, `placePattern`, and `stopSample` while a pattern plays. `newKit`: puts a new studio
 * kit track at `place` in the song and gives its id (as `shelfConfirm`). `null`: not one of these.
 */
export function reduceP4(
  state: FakeState,
  command: Command,
  takes: Map<number, FakeTake>,
  newKit: (tracks: FakeTrack[], place: number) => FakeTrack,
  nowUs: number,
): { changes: Change[]; row: HistoryRow | null } | null {
  const open = state.workspace.layout.rightOpen.includes("shelf") && state.shelf.mode.kind === "newTrack";
  const busy = state.transport.state === "recording" || state.transport.state === "countIn";
  const shelf = structuredClone(state.shelf);
  const nothing = { changes: [], row: null };
  const known = (id: string | null) => id !== null && FAKE_PATTERNS.some((p) => p.id === id);
  switch (command.type) {
    case "shelfSelectPattern":
      if (!open) return nothing;
      shelf.selectedPattern = known(command.id) ? command.id : null;
      return { changes: [{ section: "shelf", value: shelf }], row: null };
    case "playPattern": {
      if (!open || !known(command.id) || !state.transport.canPlay) return nothing;
      const map = new TempoMap(state.timeline);
      const head = map.meterAt(state.transport.position).barTick;
      const lengthUs = Math.round((map.secondsAt(head + BAR) - map.secondsAt(head)) * 1e6);
      shelf.sample = null;
      shelf.soundSample = null;
      shelf.patternSample = { id: command.id, playing: { startedUs: nowUs, lengthUs } };
      return { changes: [{ section: "shelf", value: shelf }], row: null };
    }
    case "stopSample":
      if (shelf.patternSample === null) return null;
      shelf.patternSample = null;
      return { changes: [{ section: "shelf", value: shelf }], row: null };
    case "placePattern":
      break;
    default:
      return null;
  }
  if (!open || busy || !known(command.id)) return nothing;
  const song = structuredClone(state.song);
  const arrange = structuredClone(state.arrange);
  const pianoRoll = structuredClone(state.pianoRoll);
  const transport = structuredClone(state.transport);
  const map = new TempoMap(state.timeline);
  const start = map.meterAt(transport.position).barTick;
  const end = start + BAR;
  const said = (message: StatusMessage) => ({ changes: [{ section: "status", value: { message } } as Change], row: null });
  const at = map.meterAt(start);
  if (at.num !== 4 || at.den !== 4) return said({ id: "patternNeedsFourFour" });
  let track = song.tracks.find((t) => t.id === song.playTarget);
  if (!fakeIsDrums(track)) {
    if (song.tracks.length >= 64) return said({ id: "tooManyTracks" });
    const sel = track ? song.tracks.indexOf(track) : -1;
    track = newKit(song.tracks, sel < 0 ? song.tracks.length : sel + 1);
    song.playTarget = track.id;
  }
  const kit = track!;
  if (kit.clips.some((c) => c.start < end && start < c.end)) return said({ id: "notesNotWritten" });
  let next = Math.max(0, ...takes.keys(), ...song.tracks.flatMap((t) => t.clips.flatMap((c) => [c.id, c.takeId]))) + 1;
  const take: FakeTake = { id: next++, length: BAR, notes: fakeHits() };
  takes.set(take.id, take);
  const clip: ClipView = { id: next++, name: kit.name, start, end, take: 1, takes: 1, takeId: take.id, takeOffset: 0, noteCount: take.notes.length, audio: null, comp: [] };
  kit.clips = [...kit.clips, clip].sort((a, b) => a.start - b.start);
  arrange.selectedClips = [clip.id];
  arrange.selectedClip = clip.id;
  arrange.pianoRollClip = clip.id;
  pianoRoll.take = clip.takeId;
  pianoRoll.selectedNotes = [];
  pianoRoll.selectedControls = [];
  transport.position = end;
  Object.assign(song, { undoCount: song.undoCount + 1, redoCount: 0, file: "changed" });
  const row: HistoryRow = {
    index: song.undoCount,
    step: song.undoCount,
    name: { kind: "placePattern" },
    what: { kind: "track", track: { number: song.tracks.indexOf(kit) + 1, name: kit.name } },
    place: { kind: "arrange" },
  };
  return {
    changes: [
      { section: "song", value: song },
      { section: "arrange", value: arrange },
      { section: "pianoRoll", value: pianoRoll },
      { section: "transport", value: transport },
    ],
    row,
  };
}

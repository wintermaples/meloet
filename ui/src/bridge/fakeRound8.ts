// The fake's side of protocol 27 (round 8, docs/architecture.md 23-7): the sound pack and its fetch, several vocal
// clips selected, the soloed tracks of the export dialog. The scenes are in fake.ts.

import { INSTRUMENT_GROUPS } from "./generated/instruments";
import type { FakeState } from "./fake";
import type {
  Change,
  ClipView,
  Command,
  ExportSolo,
  InstrumentKind,
  PackFailure,
  SelectedClips,
  SoundsView,
  StatusMessage,
  StretchChange,
} from "./types";

type AppState = FakeState;

/** The shelf's instruments, in its order. */
const SHELF_ORDER: InstrumentKind[] = INSTRUMENT_GROUPS.flatMap((g) => [...g.instruments]);
/** Made by computing: they sound without the pack (`meloet_core::instruments::sampled`). */
const COMPUTED: InstrumentKind[] = ["synthBass", "electroKit", "warmPad", "synthLead", "sine"];
/** Design-notes B-95-1: the urgent ones of Y2 the pack has, then the shelf's order. */
const URGENT: InstrumentKind[] = ["grandPiano", "studioKit", "fingerBass", "strings", "electricPiano", "organ"];
export const FETCH_ORDER: InstrumentKind[] = [
  ...URGENT,
  ...SHELF_ORDER.filter((i) => !URGENT.includes(i) && !COMPUTED.includes(i)),
];

export const FAKE_PACK_BYTES = 240_000_000;
/** The common file (credits and licenses) is small; the instruments share the rest evenly. */
const COMMON_BYTES = 200_000;
const INSTRUMENT_BYTES = Math.floor((FAKE_PACK_BYTES - COMMON_BYTES) / FETCH_ORDER.length);
/** What the fake's line brings in a second. */
const FAKE_BYTES_PER_S = 2_000_000;
export const FAKE_PACK_PLACE = "~/.local/share/meloet/sounds";

/** The pack all there (what a state starts with). */
export function fakeSounds(): SoundsView {
  return {
    ask: false,
    fetch: { state: "idle" },
    have: SHELF_ORDER.length,
    total: SHELF_ORDER.length,
    packBytes: FAKE_PACK_BYTES,
    newPackBytes: null,
    place: FAKE_PACK_PLACE,
  };
}

/** The bytes on the disk once the common file and these instruments are there. */
const bytesFor = (have: readonly InstrumentKind[]) => COMMON_BYTES + have.filter((i) => FETCH_ORDER.includes(i)).length * INSTRUMENT_BYTES;

/**
 * The pack as it is on a PC that has `have` of it (the computed instruments always sound): the shelf shows the rest
 * as 「まだ」, and keys tracks on those sound as the sine wave.
 */
export function withPack(s: AppState, have: readonly InstrumentKind[]): void {
  const sounding = SHELF_ORDER.filter((i) => COMPUTED.includes(i) || have.includes(i));
  s.shelf.available = sounding;
  s.shelf.notYet = SHELF_ORDER.filter((i) => !sounding.includes(i));
  s.sounds = { ...fakeSounds(), have: sounding.length };
  for (const t of s.song.tracks) {
    if (t.kind.kind === "keys" && t.sound.kind === "builtin") t.standIn = sounding.includes(t.instrument) ? null : "packMissing";
  }
}

/** The first start: nothing of the pack is here, and the band asks (mock F8-PackAsk). */
export function packAsking(s: AppState): void {
  withPack(s, []);
  s.sounds.ask = true;
  s.status.message = { id: "packAsking" };
}

/** Fetching `instrument`, with these on the disk already (mock F8-PackGet). */
export function packFetching(s: AppState, have: readonly InstrumentKind[], instrument: InstrumentKind, doneBytes: number, remainingS: number | null): void {
  withPack(s, have);
  s.sounds.fetch = { state: "fetching", instrument, doneBytes, totalBytes: FAKE_PACK_BYTES, remainingS };
}

/** The fetch stopped for `failure` with `doneBytes` on the disk (mock F8-PackFail and spec 6). */
export function packFailed(s: AppState, have: readonly InstrumentKind[], failure: PackFailure, doneBytes: number): void {
  withPack(s, have);
  s.sounds.fetch = { state: "failed", failure, doneBytes, totalBytes: FAKE_PACK_BYTES };
  s.status.message = { id: "packFailed", failure, doneBytes, totalBytes: FAKE_PACK_BYTES };
}

const sounding = (s: AppState) => new Set(s.shelf.available);
const nextToFetch = (s: AppState) => FETCH_ORDER.find((i) => !sounding(s).has(i)) ?? null;
const remainingOf = (done: number, total: number) => Math.ceil((total - done) / FAKE_BYTES_PER_S);

/**
 * Protocol 27's commands of the pack, enough for pictures and tests: 「取りにいく」 starts a fetch that `advancePack`
 * moves, 「やめる」 and 「あとで」 stop it. `null`: not one of these commands.
 */
export function reducePack(state: AppState, command: Command): Change[] | null {
  const sounds = structuredClone(state.sounds);
  let message: StatusMessage | null | undefined = undefined;
  switch (command.type) {
    case "fetchPack": {
      const next = nextToFetch(state);
      if (sounds.fetch.state === "fetching" || (next === null && sounds.newPackBytes === null)) return [];
      const doneBytes = bytesFor(state.shelf.available);
      sounds.ask = false;
      sounds.fetch = { state: "fetching", instrument: next, doneBytes, totalBytes: FAKE_PACK_BYTES, remainingS: null };
      if (next === null) {
        // Only a new pack: the fake takes it at once.
        sounds.fetch = { state: "idle" };
        sounds.newPackBytes = null;
      }
      if (state.status.message?.id === "packAsking" || state.status.message?.id === "packFailed" || state.status.message?.id === "newPack") message = null;
      break;
    }
    case "packLater":
      if (!sounds.ask && sounds.fetch.state !== "failed") return [];
      sounds.ask = false;
      sounds.fetch = { state: "idle" };
      if (state.status.message?.id === "packAsking" || state.status.message?.id === "packFailed") message = null;
      break;
    case "stopPackFetch":
      if (sounds.fetch.state !== "fetching") return [];
      sounds.fetch = { state: "idle" };
      break;
    default:
      return null;
  }
  const changes: Change[] = [{ section: "sounds", value: sounds }];
  if (message !== undefined) changes.push({ section: "status", value: { message } });
  return changes;
}

/**
 * `seconds` of the fake's line: the bytes go up; an instrument whose file is all there sounds (its row leaves 「まだ」,
 * its tracks leave the sine wave) and the status line says so; the last one says 「ぜんぶ取りました」.
 */
export function advancePack(state: AppState, seconds: number): Change[] {
  const fetch = state.sounds.fetch;
  if (fetch.state !== "fetching") return [];
  const sounds = structuredClone(state.sounds);
  const doneBytes = Math.min(FAKE_PACK_BYTES, fetch.doneBytes + Math.round(seconds * FAKE_BYTES_PER_S));
  // A file is there each time the bytes pass another file's length; at the end, all of them.
  const queue = FETCH_ORDER.filter((i) => !sounding(state).has(i));
  const files = doneBytes >= FAKE_PACK_BYTES ? queue.length : Math.floor(doneBytes / INSTRUMENT_BYTES) - Math.floor(fetch.doneBytes / INSTRUMENT_BYTES);
  const came = queue.slice(0, files);
  const have = SHELF_ORDER.filter((i) => sounding(state).has(i) || came.includes(i));
  const changes: Change[] = [];
  if (came.length > 0) {
    const shelf = structuredClone(state.shelf);
    shelf.available = have;
    shelf.notYet = SHELF_ORDER.filter((i) => !have.includes(i));
    changes.push({ section: "shelf", value: shelf });
    const song = structuredClone(state.song);
    for (const t of song.tracks) if (t.standIn === "packMissing" && have.includes(t.instrument)) t.standIn = null;
    if (JSON.stringify(song) !== JSON.stringify(state.song)) changes.push({ section: "song", value: song });
  }
  sounds.have = have.length;
  const next = FETCH_ORDER.find((i) => !have.includes(i)) ?? null;
  let message: StatusMessage | null = null;
  if (next === null) {
    sounds.fetch = { state: "idle" };
    message = { id: "packFetched", count: have.length };
  } else {
    sounds.fetch = { ...fetch, instrument: next, doneBytes, remainingS: remainingOf(doneBytes, fetch.totalBytes) };
    if (came.length > 0) message = { id: "soundReady", instrument: came[came.length - 1]! };
  }
  changes.push({ section: "sounds", value: sounds });
  if (message) changes.push({ section: "status", value: { message } });
  return changes;
}

// ---- several vocal clips (39-B, 40-A) ----

const vocalClips = (state: AppState, ids: readonly number[]) =>
  state.song.tracks.flatMap((t) => t.clips.filter((c) => c.audio !== null && ids.includes(c.id)).map((c) => ({ track: t.id, clip: c })));

/** What one value of the group 「まとめて変える」 shows: the one value, or `null` (「—」). */
function same<T>(values: T[]): T | null {
  return values.every((v) => v === values[0]) ? values[0]! : null;
}

/** The panel for the selected clips: two or more, every one a vocal clip; `null` otherwise. */
export function fakeClipsPanel(state: AppState): SelectedClips | null {
  const ids = state.arrange.selectedClips;
  const found = vocalClips(state, ids);
  if (ids.length < 2 || found.length !== ids.length) return null;
  const order = (id: number) => ids.indexOf(id);
  found.sort((a, b) => order(a.clip.id) - order(b.clip.id));
  const audio = found.map((f) => f.clip.audio!);
  return {
    clips: found.map((f) => f.clip.id),
    tracks: state.song.tracks.map((t) => t.id).filter((id) => found.some((f) => f.track === id)),
    gainDb: same(audio.map((a) => a.gainDb)),
    follow: same(audio.map((a) => a.stretch.follow)),
    semitones: same(audio.map((a) => a.stretch.semitones)),
    cents: same(audio.map((a) => a.stretch.cents)),
    keepFormant: same(audio.map((a) => a.stretch.keepFormant)),
  };
}

const clampGain = (db: number) => Math.min(12, Math.max(-60, Math.round(db * 100) / 100));

/**
 * Protocol 27's commands for several vocal clips, each one step as Rust makes it: a drag of the volume line moves
 * every clip by the same dB, a typed value goes into every clip. `null`: not one of these commands.
 */
export function reduceClips27(state: AppState, command: Command): Change[] | null {
  let edit: ((clip: ClipView) => void) | null = null;
  let ids: number[] = [];
  switch (command.type) {
    case "shiftClipGains":
      if (Number.isNaN(command.byDb) || command.byDb === 0) return [];
      ids = command.clips;
      edit = (c) => (c.audio!.gainDb = clampGain(c.audio!.gainDb + command.byDb));
      break;
    case "setClipGains":
      if (Number.isNaN(command.db)) return [];
      ids = command.clips;
      edit = (c) => (c.audio!.gainDb = clampGain(command.db));
      break;
    case "setClipsStretch": {
      const change: StretchChange = command.change;
      const whole = (v: number, most: number) => Math.min(most, Math.max(-most, Math.round(v)));
      ids = command.clips;
      edit = (c) => {
        const s = c.audio!.stretch;
        if (change.kind === "follow") s.follow = change.on;
        else if (change.kind === "keepFormant") s.keepFormant = change.on;
        else if (change.kind === "semitones" && !Number.isNaN(change.semitones)) s.semitones = whole(change.semitones, 12);
        else if (change.kind === "cents" && !Number.isNaN(change.cents)) s.cents = whole(change.cents, 50);
      };
      break;
    }
    default:
      return null;
  }
  const song = structuredClone(state.song);
  for (const t of song.tracks) for (const c of t.clips) if (c.audio && ids.includes(c.id)) edit(c);
  if (JSON.stringify(song) === JSON.stringify(state.song)) return [];
  Object.assign(song, { undoCount: song.undoCount + 1, redoCount: 0, file: "changed" });
  return [{ section: "song", value: song }];
}

/** `ExportDialogView::solo` of the song. */
export function fakeSolo(state: Pick<AppState, "song">): ExportSolo | null {
  const soloed = state.song.tracks.filter((t) => t.soloed);
  const first = soloed[0];
  return first ? { first: { number: first.number, name: first.name }, others: soloed.length - 1 } : null;
}

// The fake's side of protocols 28 and 29 (round 10): clip names, cutting a clip at a place, joining clips, an empty
// clip made in the arrange, a marker at a place, overdub, the chord stamp, 「ピアノロールでひらく」 of a track's header
// and the switch 「AI とつなぐ」. Each changes the fake's song as Rust does
// and gives the row of the history band; the fake has no undo. The scenes are in fake.ts.

import type { FakeBands, FakeState, FakeTake } from "./fake";
import type { Change, ClipView, Command, HistoryRow, StatusMessage, StepPlace } from "./types";
import { TempoMap } from "../time/tempoMap";

type AppState = FakeState;
type Note = FakeTake["notes"][number];

const NEW_CLIP_BARS = 4;
const ARRANGE: StepPlace = { kind: "arrange" };
const PIANO_ROLL: StepPlace = { kind: "pianoRoll" };

/** `null`: not one of these commands. `row`: the step made, at `row.index`. */
export function reduceRound10(
  state: AppState,
  bands: FakeBands,
  command: Command,
  takes: Map<number, FakeTake>,
): { changes: Change[]; row: HistoryRow | null } | null {
  const song = structuredClone(state.song);
  const arrange = structuredClone(state.arrange);
  const pianoRoll = structuredClone(state.pianoRoll);
  const timeline = structuredClone(state.timeline);
  const transport = structuredClone(state.transport);
  const workspace = structuredClone(state.workspace);
  const aiLink = structuredClone(state.aiLink);
  const map = new TempoMap(state.timeline);
  const busy = state.transport.state === "recording" || state.transport.state === "countIn";
  const nothing = { changes: [], row: null };
  let message: StatusMessage | null = null;
  let row: Omit<HistoryRow, "index" | "step"> | null = null;

  const clips = () => song.tracks.flatMap((t) => t.clips.map((clip) => ({ track: t, clip })));
  const found = (id: number) => clips().find((f) => f.clip.id === id) ?? null;
  // Clip and take ids come from one counter, as in the song model.
  let next = Math.max(0, ...takes.keys(), ...clips().flatMap((f) => [f.clip.id, f.clip.takeId])) + 1;
  const newTake = (length: number, notes: Note[]): FakeTake => {
    const take = { id: next++, length, notes: notes.sort((a, b) => a.start - b.start || a.key - b.key) };
    takes.set(take.id, take);
    return take;
  };
  const notesOf = (clip: ClipView) => takes.get(clip.takeId)?.notes ?? [];
  const show = (clip: ClipView, notes: number[] = []) => {
    arrange.selectedClips = [clip.id];
    arrange.selectedClip = clip.id;
    arrange.pianoRollClip = clip.id;
    pianoRoll.take = clip.takeId;
    pianoRoll.selectedNotes = notes.map((i) => ({ start: i, end: i + 1 }));
    pianoRoll.selectedControls = [];
  };
  const barHead = (tick: number) => map.meterAt(tick).barTick;
  const barAfter = (tick: number) => map.tickOfBar(map.meterAt(Math.max(0, tick - 1)).bar + 1);

  switch (command.type) {
    case "renameClip": {
      const f = found(command.clip);
      const name = command.name.trim();
      if (!f || busy || name === "" || name === f.clip.name) return nothing;
      row = { name: { kind: "renameClip" }, what: { kind: "renamed", from: f.clip.name, to: name }, place: ARRANGE };
      f.clip.name = name;
      break;
    }
    case "splitClipAt": {
      const f = found(command.clip);
      const at = command.tick;
      if (!f || busy || !(f.clip.start < at && at < f.clip.end)) return nothing;
      const c = f.clip;
      const left: ClipView = { ...c, end: at };
      if (!c.audio) {
        // The left half gets a take with a new id when a note sounds over the cut: there it ends at the cut.
        const cut = at - (c.start + c.takeOffset);
        const old = notesOf(c);
        if (old.some((n) => n.start < cut && n.start + n.length > cut)) {
          const ended = old.map((n) => (n.start < cut && n.start + n.length > cut ? { ...n, length: cut - n.start } : { ...n }));
          left.takeId = newTake(takes.get(c.takeId)!.length, ended).id;
        }
      }
      const right: ClipView = { ...c, id: next++, start: at, takeOffset: c.takeOffset - (at - c.start) };
      f.track.clips.splice(f.track.clips.indexOf(c), 1, left, right);
      arrange.selectedClips = [right.id];
      arrange.selectedClip = right.id;
      if (arrange.pianoRollClip === c.id) pianoRoll.take = left.takeId;
      message = { id: "clipSplit", name: c.name, tick: at, count: 1 };
      row = { name: { kind: "splitClips" }, what: { kind: "clips", count: 1 }, place: ARRANGE };
      break;
    }
    case "joinClips": {
      const picked = [...new Set(command.clips)].map(found);
      const track = picked[0]?.track;
      if (busy || picked.length < 2 || picked.some((f) => !f || f.track !== track || f.clip.audio)) return nothing;
      const joined = picked.map((f) => f!.clip).sort((a, b) => a.start - b.start);
      const first = joined[0]!;
      const start = first.start;
      const end = Math.max(...joined.map((c) => c.end));
      // Another clip of the track between them would be under the joined one.
      if (track!.clips.some((c) => !joined.includes(c) && c.start < end && c.end > start)) return nothing;
      const notes: Note[] = [];
      for (const c of joined) {
        const zero = c.start + c.takeOffset;
        for (const n of notesOf(c)) {
          const s = zero + n.start;
          if (s < c.start || s >= c.end) continue;
          notes.push({ start: s - start, length: Math.min(n.length, c.end - s), key: n.key, velocity: n.velocity });
        }
      }
      const take = newTake(end - start, notes);
      const clip: ClipView = { ...first, end, take: 1, takes: 1, takeId: take.id, takeOffset: 0, noteCount: take.notes.length };
      const gone = new Set(joined.map((c) => c.id));
      track!.clips = track!.clips.filter((c) => !gone.has(c.id) || c.id === first.id).map((c) => (c.id === first.id ? clip : c));
      show(clip);
      row = { name: { kind: "joinClips" }, what: { kind: "clips", count: joined.length }, place: ARRANGE };
      break;
    }
    case "createClip": {
      const track = song.tracks.find((t) => t.id === command.track);
      const tick = command.tick;
      if (!track || busy || track.kind.kind === "vocal" || track.clips.some((c) => c.start <= tick && tick < c.end)) return nothing;
      const head = barHead(tick);
      const start = Math.max(head, ...track.clips.filter((c) => c.end <= tick).map((c) => c.end));
      const end = Math.min(map.tickOfBar(map.meterAt(tick).bar + NEW_CLIP_BARS), ...track.clips.filter((c) => c.start > tick).map((c) => c.start));
      const take = newTake(end - start, []);
      const clip: ClipView = { id: next++, name: track.name, start, end, take: 1, takes: 1, takeId: take.id, takeOffset: 0, noteCount: 0, audio: null };
      track.clips = [...track.clips, clip].sort((a, b) => a.start - b.start);
      song.playTarget = track.id;
      show(clip);
      if (!bands.bottom.includes("pianoRoll")) {
        bands.toggle("pianoRoll");
        workspace.layout = bands.layout();
      }
      row = { name: { kind: "writeClip" }, what: { kind: "name", name: clip.name }, place: ARRANGE };
      break;
    }
    case "addMarker": {
      if (busy) return nothing;
      const id = Math.max(0, ...timeline.markers.map((m) => m.id)) + 1;
      const tick = nearestBeat(map, command.tick ?? transport.position);
      timeline.markers = [...timeline.markers, { id, tick, name: command.name }].sort((a, b) => a.tick - b.tick || a.id - b.id);
      timeline.selected = { kind: "marker", id };
      timeline.renaming = id;
      message = { id: "markerAdded", name: command.name };
      row = { name: { kind: "addMarker" }, what: { kind: "name", name: command.name }, place: ARRANGE };
      break;
    }
    case "setOverdub":
      transport.overdub = command.on;
      break;
    case "setAiLink":
      // The fake has no socket: on is 「待っています」 (the scene aiLinkConnected shows the third state).
      aiLink.on = command.on;
      aiLink.port = command.on ? "waiting" : "closed";
      break;
    case "openPianoRoll": {
      const track = song.tracks.find((t) => t.id === command.track);
      if (!track || track.kind.kind === "vocal") return nothing;
      const at = transport.position;
      const clip = track.clips.find((c) => c.start <= at && at < c.end) ?? null;
      if (!busy) {
        song.playTarget = track.id;
        song.keysTarget = track.id;
      }
      if (clip) show(clip);
      else if (!busy) {
        arrange.selectedClips = [];
        arrange.selectedClip = null;
        arrange.pianoRollClip = null;
        Object.assign(pianoRoll, { take: null, selectedNotes: [], selectedControls: [] });
      }
      if (!bands.bottom.includes("pianoRoll")) {
        bands.toggle("pianoRoll");
        workspace.layout = bands.layout();
      }
      break;
    }
    case "addNotes": {
      const keys = [...new Set(command.keys)].filter((k) => k <= 127);
      if (busy || keys.length === 0) return nothing;
      let length = Math.max(1, command.length);
      const velocity = Math.min(127, Math.max(1, command.velocity));
      const at = command.start;
      let f: { track: AppState["song"]["tracks"][number]; clip: ClipView } | null;
      if (command.take !== null) {
        // Only into the take the piano roll shows, as Rust.
        const shown = state.pianoRoll.take === command.take ? arrange.pianoRollClip : null;
        f = shown === null ? null : found(shown);
        // The fake does not grow a clip to the left.
        if (!f || at < f.clip.start) return nothing;
      } else {
        if (state.pianoRoll.take !== null) return nothing;
        const track = song.tracks.find((t) => t.id === song.playTarget);
        if (!track || track.kind.kind === "vocal") return nothing;
        const there = track.clips.find((c) => c.start <= at && at < c.end) ?? null;
        if (there) f = { track, clip: there };
        else {
          const start = Math.max(barHead(at), ...track.clips.filter((c) => c.end <= at).map((c) => c.end));
          const end = Math.min(map.tickOfBar(map.meterAt(at).bar + NEW_CLIP_BARS), ...track.clips.filter((c) => c.start > at).map((c) => c.start));
          const take = newTake(end - start, []);
          const clip: ClipView = { id: next++, name: track.name, start, end, take: 1, takes: 1, takeId: take.id, takeOffset: 0, noteCount: 0, audio: null };
          track.clips = [...track.clips, clip].sort((a, b) => a.start - b.start);
          f = { track, clip };
          length = Math.min(length, end - at);
        }
      }
      const c = f.clip;
      const later = f.track.clips.filter((x) => x.start >= c.end).reduce((m, x) => Math.min(m, x.start), Infinity);
      const end = Math.max(c.end, barAfter(at + length));
      if (end > later) return { changes: [{ section: "status", value: { message: { id: "notesNotWritten" } } }], row: null };
      const zero = c.start + c.takeOffset;
      const added = keys.map((key) => ({ start: at - zero, length, key, velocity }));
      const take = newTake(Math.max(takes.get(c.takeId)?.length ?? 0, end - zero), [...notesOf(c).map((n) => ({ ...n })), ...added]);
      const index = take.notes
        .map((n, i) => (added.some((a) => a.start === n.start && a.key === n.key && a.length === n.length) ? i : -1))
        .filter((i) => i >= 0);
      const clip: ClipView = { ...c, end, takeId: take.id, noteCount: take.notes.length };
      f.track.clips.splice(f.track.clips.indexOf(c), 1, clip);
      // Into the shown take the clip selection stays; on a track the clip is selected and shown.
      if (command.take === null) show(clip, index);
      else {
        pianoRoll.take = clip.takeId;
        pianoRoll.selectedNotes = index.map((i) => ({ start: i, end: i + 1 }));
      }
      row = {
        name: { kind: "writeNote" },
        what: keys.length === 1 ? { kind: "note", key: keys[0]! } : { kind: "notes", count: keys.length },
        place: PIANO_ROLL,
      };
      break;
    }
    // P6 (round 12) 「並べる」: only into the take the piano roll shows (the fake makes no clip for it). The clip grows to the
    // right by bars; chords from the one that would reach the next clip are left out, as Rust.
    case "addNotesAt": {
      const shown = command.take !== null && state.pianoRoll.take === command.take ? arrange.pianoRollClip : null;
      const f = shown === null ? null : found(shown);
      if (busy || !f) return nothing;
      const c = f.clip;
      const later = f.track.clips.filter((x) => x.start >= c.end).reduce((m, x) => Math.min(m, x.start), Infinity);
      const given = command.notes.filter((n) => n.key <= 127 && n.start >= c.start);
      const starts = [...new Set(given.map((n) => n.start))].sort((a, b) => a - b);
      const fits = starts.filter((s) => given.filter((n) => n.start <= s).every((n) => barAfter(n.start + Math.max(1, n.length)) <= later));
      const last = fits[fits.length - 1] ?? -1;
      const kept = given.filter((n) => n.start <= last);
      if (kept.length === 0) return nothing;
      const end = Math.max(c.end, ...kept.map((n) => barAfter(n.start + Math.max(1, n.length))));
      const zero = c.start + c.takeOffset;
      const added = kept.map((n) => ({ start: n.start - zero, length: Math.max(1, n.length), key: n.key, velocity: Math.min(127, Math.max(1, n.velocity)) }));
      const take = newTake(Math.max(takes.get(c.takeId)?.length ?? 0, end - zero), [...notesOf(c).map((n) => ({ ...n })), ...added]);
      const index = take.notes
        .map((n, i) => (added.some((a) => a.start === n.start && a.key === n.key && a.length === n.length) ? i : -1))
        .filter((i) => i >= 0);
      const clip: ClipView = { ...c, end, takeId: take.id, noteCount: take.notes.length };
      f.track.clips.splice(f.track.clips.indexOf(c), 1, clip);
      pianoRoll.take = clip.takeId;
      pianoRoll.selectedNotes = index.map((i) => ({ start: i, end: i + 1 }));
      row = { name: { kind: "writeNote" }, what: { kind: "notes", count: added.length }, place: PIANO_ROLL };
      break;
    }
    default:
      return null;
  }

  const changes: Change[] = [];
  let made: HistoryRow | null = null;
  if (row) {
    Object.assign(song, { undoCount: song.undoCount + 1, redoCount: 0, file: "changed" });
    made = { index: song.undoCount, step: song.undoCount, ...row };
  }
  const changed = <T>(a: T, b: T) => JSON.stringify(a) !== JSON.stringify(b);
  if (changed(song, state.song)) changes.push({ section: "song", value: song });
  if (changed(arrange, state.arrange)) changes.push({ section: "arrange", value: arrange });
  if (changed(pianoRoll, state.pianoRoll)) changes.push({ section: "pianoRoll", value: pianoRoll });
  if (changed(timeline, state.timeline)) changes.push({ section: "timeline", value: timeline });
  if (changed(transport, state.transport)) changes.push({ section: "transport", value: transport });
  if (changed(workspace, state.workspace)) changes.push({ section: "workspace", value: workspace });
  if (changed(aiLink, state.aiLink)) changes.push({ section: "aiLink", value: aiLink });
  if (message) changes.push({ section: "status", value: { message } });
  return { changes, row: made };
}

/** The head of the beat nearest `tick`; half way, the later one. */
function nearestBeat(map: TempoMap, tick: number): number {
  const inBeat = map.barBeatTick(tick).tick;
  if (inBeat === 0) return tick;
  const before = tick - inBeat;
  const after = map.nextBeat(tick).tick;
  return tick - before < after - tick ? before : after;
}

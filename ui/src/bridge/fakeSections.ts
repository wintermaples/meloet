// The fake's side of round 12 A2: a section copied right after itself, a section taken out, empty bars put in.
// As Rust (`crates/meloet-model/src/sections.rs`), all three are one splice: the song before `keep`, `gap` empty
// ticks, then the song from `resume` on. The fake moves the clips (cut at the two places), the markers, the tempo
// points, the meter changes and the loop. It does not move lane points, and it puts in none of the points Rust adds
// at a seam to keep the tempo on both sides; it has no undo.

import type { FakeState, FakeTake } from "./fake";
import { INSERT_SPACE_BARS_MAX } from "./types";
import type { Change, ClipView, Command, HistoryRow, MarkerView, SectionUse, SongView, StepName, TimelineView } from "./types";
import { TempoMap } from "../time/tempoMap";

type Song = Pick<SongView, "tracks">;

/** The song's end for a section: the latest clip end, up to the head of a bar. */
function songEnd(song: Song, map: TempoMap): number {
  const end = Math.max(0, ...song.tracks.flatMap((t) => t.clips.map((c) => c.end)));
  const at = map.meterAt(end);
  return at.barTick === end ? end : map.tickOfBar(at.bar + 1);
}

/** The section of the marker at `from`: to the next marker that is later, or to the song's end. */
export function fakeSection(song: Song, timeline: TimelineView, from: number): { use: SectionUse; to: number; bars: number } {
  const map = new TempoMap(timeline);
  const later = timeline.markers.map((m) => m.tick).filter((tick) => tick > from);
  const to = later.length > 0 ? Math.min(...later) : songEnd(song, map);
  if (to <= from) return { use: "empty", to, bars: 0 };
  const [a, b] = [map.meterAt(from), map.meterAt(to)];
  const onBars = a.barTick === from && b.barTick === to;
  return { use: onBars ? "usable" : "notOnBars", to, bars: b.bar - a.bar };
}

/** The markers with `section` as the song has it now. */
export function withSections(song: Song, timeline: TimelineView): MarkerView[] {
  return timeline.markers.map((m) => ({ ...m, section: fakeSection(song, timeline, m.tick).use }));
}

interface Splice {
  keep: number;
  gap: number;
  resume: number;
}

/** `null`: not one of these commands. `row`: the step made, at `row.index`. */
export function reduceSections(state: FakeState, command: Command, takes: Map<number, FakeTake>): { changes: Change[]; row: HistoryRow | null } | null {
  if (command.type !== "duplicateSection" && command.type !== "deleteSection" && command.type !== "insertSpace") return null;
  const nothing = { changes: [], row: null };
  const busy = state.transport.state === "recording" || state.transport.state === "countIn";
  const marker = state.timeline.markers.find((m) => m.id === command.marker);
  if (busy || !marker) return nothing;
  const from = marker.tick;
  const section = fakeSection(state.song, state.timeline, from);
  if (section.use !== "usable") return nothing;
  const map = new TempoMap(state.timeline);
  const fromBar = map.meterAt(from).bar;
  let cut: Splice;
  let bars: { keep: number; gap: number; resume: number };
  let name: StepName;
  let stepBars = section.bars;
  if (command.type === "insertSpace") {
    if (!Number.isInteger(command.bars) || command.bars < 1 || command.bars > INSERT_SPACE_BARS_MAX) return nothing;
    cut = { keep: from, gap: map.tickOfBar(fromBar + 1) - from, resume: from };
    cut.gap *= command.bars;
    bars = { keep: fromBar, gap: command.bars, resume: fromBar };
    name = { kind: "insertSpace" };
    stepBars = command.bars;
  } else if (command.type === "deleteSection") {
    cut = { keep: from, gap: 0, resume: section.to };
    bars = { keep: fromBar, gap: 0, resume: fromBar + section.bars };
    name = { kind: "deleteSection" };
  } else {
    cut = { keep: section.to, gap: 0, resume: from };
    bars = { keep: fromBar + section.bars, gap: 0, resume: fromBar };
    name = { kind: "duplicateSection" };
  }
  const moved = (tick: number) => tick - cut.resume + cut.keep + cut.gap;

  const song = structuredClone(state.song);
  const timeline = structuredClone(state.timeline);
  const arrange = structuredClone(state.arrange);
  const pianoRoll = structuredClone(state.pianoRoll);
  // Clip, take and marker ids come from one counter, as in the song model.
  let next = Math.max(0, ...takes.keys(), ...timeline.markers.map((m) => m.id), ...song.tracks.flatMap((t) => t.clips.flatMap((c) => [c.id, c.takeId]))) + 1;

  for (const track of song.tracks) {
    let clips = track.clips;
    for (const at of new Set([cut.keep, cut.resume])) {
      clips = clips.flatMap((c): ClipView[] =>
        c.start < at && at < c.end ? [{ ...c, end: at }, { ...c, id: next++, start: at, takeOffset: c.takeOffset - (at - c.start) }] : [c],
      );
    }
    track.clips = [
      ...clips.filter((c) => c.start < cut.keep),
      ...clips.filter((c) => c.start >= cut.resume).map((c) => ({ ...c, id: c.start < cut.keep ? next++ : c.id, start: moved(c.start), end: moved(c.end) })),
    ];
  }
  timeline.markers = [
    ...timeline.markers.filter((m) => m.tick < cut.keep),
    ...timeline.markers.filter((m) => m.tick >= cut.resume).map((m) => ({ ...m, id: m.tick < cut.keep ? next++ : m.id, tick: moved(m.tick) })),
  ].sort((a, b) => a.tick - b.tick || a.id - b.id);
  timeline.tempoPoints = [
    ...timeline.tempoPoints.filter((p) => p.tick < cut.keep),
    ...timeline.tempoPoints.filter((p) => p.tick >= cut.resume && p.tick > 0).map((p) => ({ ...p, tick: moved(p.tick) })),
  ];
  // Empty bars are bars of the meter at the cut: a change on the cut stays, the later ones move.
  timeline.meterChanges =
    bars.gap > 0
      ? timeline.meterChanges.map((c) => (c.bar > bars.keep ? { ...c, bar: c.bar + bars.gap } : c))
      : [
          ...timeline.meterChanges.filter((c) => c.bar < bars.keep),
          ...timeline.meterChanges.filter((c) => c.bar >= bars.resume && c.bar > 0).map((c) => ({ ...c, bar: c.bar - bars.resume + bars.keep })),
        ];
  const loop = song.loopRange;
  if (loop) {
    let { start, end } = loop;
    if (cut.resume > cut.keep) {
      const at = (tick: number) => (tick <= cut.keep ? tick : tick < cut.resume ? cut.keep : tick - (cut.resume - cut.keep));
      [start, end] = [at(start), at(end)];
    } else {
      const by = cut.gap + (cut.keep - cut.resume);
      if (start >= cut.keep) start += by;
      if (end > cut.keep) end += by;
    }
    song.loopRange = end > start ? { start, end } : null;
    if (!song.loopRange) song.loopOn = false;
  }
  timeline.markers = withSections(song, timeline);

  // Nothing stays selected; the play line stays.
  timeline.selected = null;
  timeline.renaming = null;
  arrange.selectedClips = [];
  arrange.selectedClip = null;
  if (arrange.pianoRollClip !== null && !song.tracks.some((t) => t.clips.some((c) => c.id === arrange.pianoRollClip))) arrange.pianoRollClip = null;
  pianoRoll.selectedNotes = [];
  pianoRoll.selectedControls = [];

  Object.assign(song, { undoCount: song.undoCount + 1, redoCount: 0, file: "changed" });
  const row: HistoryRow = { index: song.undoCount, step: song.undoCount, name, what: { kind: "section", name: marker.name, bars: stepBars }, place: { kind: "arrange" } };
  const changes: Change[] = [
    { section: "song", value: song },
    { section: "timeline", value: timeline },
    { section: "arrange", value: arrange },
    { section: "pianoRoll", value: pianoRoll },
  ];
  return { changes, row };
}

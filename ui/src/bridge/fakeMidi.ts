// The fake's copy of 「入れるトラック」 of a MIDI export (protocol 36; spec 3 "MIDI export", spec 7 A-31): what a choice
// takes of the song, and why it cannot be written. Rust's own rules are in meloet-core.

import type { ExportDialogView, ExportMidiProblem, ExportMidiTrack, SongView, TrackView } from "./types";

/**
 * The numbers and ranges of 「1-3, 5」 as [low, high] pairs (spec 7 A-31 "How numbers are read"): full-width digits are
 * digits, 「、」 「，」 are commas and 「－」 「ー」 「‐」 「〜」 「~」 hyphens; spaces around them do not count, but a space
 * between two digits cannot be read; a range written backwards (「5-3」) is read as 3 to 5; empty parts (「1,,2,」) are
 * passed by. null: the text cannot be read.
 */
export function readTrackNumbers(text: string): [number, number][] | null {
  const plain = text
    .replace(/[０-９]/g, (d) => String.fromCharCode(d.charCodeAt(0) - 0xfee0))
    .replace(/[、，]/g, ",")
    .replace(/[－ー‐〜~]/g, "-");
  const ranges: [number, number][] = [];
  for (const part of plain.split(",")) {
    if (part.trim() === "") continue;
    const ends = part.split("-").map((p) => p.trim());
    if (ends.length > 2 || ends.some((p) => !/^\d+$/.test(p))) return null;
    const a = Number(ends[0]);
    const b = Number(ends[ends.length - 1]);
    if (a > 0xffff_ffff || b > 0xffff_ffff) return null;
    ranges.push(a <= b ? [a, b] : [b, a]);
  }
  return ranges;
}

export type FakeMidiTaken = Pick<ExportDialogView, "midiSelectedUsable" | "midiTaken" | "midiVocal" | "midiProblem">;

const isKeys = (t: TrackView) => t.kind.kind === "keys";
const named = (t: TrackView): ExportMidiTrack => ({ number: t.number, name: t.name });

/** What `choice` takes of `song`, as `ExportDialogView` says it. */
export function fakeMidiTaken(song: Pick<SongView, "tracks" | "playTarget">, choice: ExportDialogView["midiTracks"], numbers: string): FakeMidiTaken {
  const selected = song.tracks.find((t) => t.id === song.playTarget);
  const midiSelectedUsable = selected !== undefined && isKeys(selected);
  let taken: TrackView[] = [];
  let vocal: TrackView[] = [];
  let problem: ExportMidiProblem | null = null;
  if (choice === "all") taken = song.tracks.filter(isKeys);
  else if (choice === "selected") taken = midiSelectedUsable ? [selected] : [];
  else {
    const ranges = readTrackNumbers(numbers);
    if (ranges === null) problem = { kind: "unreadable" };
    else {
      const inside = song.tracks.filter((t) => ranges.some(([low, high]) => t.number >= low && t.number <= high));
      taken = inside.filter(isKeys);
      vocal = inside.filter((t) => !isKeys(t));
      // Tracks are numbered 1 to their count: the smallest number of each range that is no track's.
      const count = song.tracks.length;
      const missing = ranges.flatMap(([low, high]) => (low === 0 ? [0] : high > count ? [Math.max(low, count + 1)] : []));
      if (missing.length > 0) problem = { kind: "noSuchTrack", number: Math.min(...missing) };
    }
  }
  if (problem === null && taken.length === 0) problem = { kind: "noTrack" };
  return { midiSelectedUsable, midiTaken: taken.map(named), midiVocal: vocal.map(named), midiProblem: problem };
}

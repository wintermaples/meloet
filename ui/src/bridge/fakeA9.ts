// The fake's 「ノーマライズ」 and 「逆にする」 (round 12, A9): one step each, as Rust makes it once its audio thread has
// read or written the files. The fake answers at once, from the peaks of its made-up files, over the frames of the
// shown take.

import { PEAK_LEVELS } from "./generated/audio";
import { type FakeAudio, fakePeakCount, fakePeakValues } from "./fakeAudio";
import type { FakeState } from "./fake";
import type { Change, ClipView, Command, StatusMessage } from "./types";

/** The loudest sample of the frames `from..to` of the file, of full scale, from its finest peaks. */
function loudest(a: FakeAudio, from: number, to: number): number {
  const size = PEAK_LEVELS[0]!;
  const first = Math.floor(from / size);
  const count = Math.min(fakePeakCount(a, 0), Math.ceil(to / size)) - first;
  if (count <= 0) return 0;
  let most = 0;
  for (const v of fakePeakValues(a, 0, first, count)) most = Math.max(most, Math.abs(v));
  return most / 32767;
}

/** `meloet_model::normalize_gain`: the volume and whether it stopped at an end of −60..+12. */
function normalizeGain(peak: number): [number, boolean] {
  const db = Math.round((-1 - 20 * Math.log10(peak)) * 100) / 100;
  return [Math.min(12, Math.max(-60, db)), db < -60 || db > 12];
}

/**
 * `normalizeClips` and `reverseClips`. `files`: what the `peaks` invoke knows, where a backwards file is put.
 * `null`: not one of these commands.
 */
export function reduceA9(state: FakeState, command: Command, files: Map<number, FakeAudio>): Change[] | null {
  if (command.type !== "normalizeClips" && command.type !== "reverseClips") return null;
  if (state.transport.state === "countIn" || state.transport.state === "recording") return [];
  const song = structuredClone(state.song);
  let changed = 0;
  let silent = 0;
  let limited = 0;
  let followOff = 0;
  const ids = [
    ...song.tracks.flatMap((t) => t.clips.flatMap((c) => [c.id, c.takeId, c.audio?.audio ?? 0])),
    ...files.keys(),
  ];
  let next = Math.max(0, ...ids) + 1;
  for (const track of song.tracks) {
    for (const clip of track.clips) {
      const audio = clip.audio;
      const file = audio ? files.get(audio.audio) : undefined;
      if (!audio || !file || audio.state === "missing" || !command.clips.includes(clip.id)) continue;
      const [from, to] = [audio.firstFrame, Math.min(audio.firstFrame + audio.frames, file.frames)];
      if (command.type === "normalizeClips") {
        const peak = loudest(file, from, to);
        if (peak === 0) {
          silent += 1;
          continue;
        }
        const [db, put] = normalizeGain(peak);
        limited += put ? 1 : 0;
        if (db !== audio.gainDb) changed += 1;
        audio.gainDb = db;
        continue;
      }
      if (to <= from) continue;
      const made: FakeAudio = {
        id: next,
        file: `${track.name} ${next}.wav`,
        sampleRate: file.sampleRate,
        bits: file.bits,
        channels: file.channels,
        frames: to - from,
        backwardsOf: { audio: file, end: to },
      };
      files.set(made.id, made);
      if (audio.stretch.follow && audio.sourceTempo && audio.sourceTempo.tempoPoints.length > 0) followOff += 1;
      reverseClip(clip, made, next + 1);
      next += 2;
      changed += 1;
    }
  }
  const message: StatusMessage | null =
    silent > 0
      ? { id: "normalizeSilent", count: silent }
      : limited > 0
        ? { id: "normalizeLimited", count: limited }
        : followOff > 0
          ? { id: "reverseFollowOff", count: followOff }
          : null;
  const changes: Change[] = [];
  if (changed > 0) {
    Object.assign(song, { undoCount: song.undoCount + 1, redoCount: 0, file: "changed" });
    changes.push({ section: "song", value: song });
  }
  if (message) changes.push({ section: "status", value: { message } });
  return changes;
}

/** The clip shows a new take of `made` from its start; a tempo that changed along it is not followed any more. */
function reverseClip(clip: ClipView, made: FakeAudio, takeId: number): void {
  const audio = clip.audio!;
  const steady = audio.sourceTempo === null || audio.sourceTempo.tempoPoints.length === 0;
  Object.assign(clip, { takeId, takeOffset: 0 });
  Object.assign(audio, { audio: made.id, file: made.file, firstFrame: 0, frames: made.frames });
  if (!steady) {
    audio.stretch = { ...audio.stretch, follow: false, sourceBpm: null, ratio: null };
    audio.sourceTempo = null;
  }
}

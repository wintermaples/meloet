// The fake's 「いいとこ取り」 (round 12, A8): `setComp` and `clearComp` as one step each, with Rust's rules for a
// list of pieces (`meloet_model::comp_is_valid`), and what the other commands do to a clip that has a comp.

import type { FakeAudio } from "./fakeAudio";
import type { FakeState } from "./fake";
import type { AudioClipView, AudioTakeView, Change, ClipView, Command, CompPiece } from "./types";

/** `comp_is_valid`: the first at 0, in the order of `from`, inside the clip, of takes the clip has, no two in a row of one take. */
export function fakeCompValid(pieces: readonly CompPiece[], length: number, takes: readonly number[]): boolean {
  return (
    (pieces.length === 0 || pieces[0]!.from === 0) &&
    pieces.every((p) => Number.isInteger(p.from) && p.from >= 0 && p.from < length && takes.includes(p.take)) &&
    pieces.every((p, i) => i === 0 || (pieces[i - 1]!.from < p.from && pieces[i - 1]!.take !== p.take))
  );
}

/**
 * `comp_window`: the pieces of a clip whose start moved by `shift` ticks and that is `length` long now. What sounded at
 * a place of the song still sounds there; a piece outside the clip goes, the first reaches back to the start.
 */
export function fakeCompWindow(comp: readonly CompPiece[], shift: number, length: number): CompPiece[] {
  const out: CompPiece[] = [];
  for (const [i, piece] of comp.entries()) {
    const from = i === 0 ? 0 : Math.max(0, piece.from - shift);
    if (from >= length) break;
    if (out.at(-1)?.from === from) out.pop();
    if (out.at(-1)?.take !== piece.take) out.push({ from, take: piece.take });
  }
  return out;
}

/** The takes of a clip of the fake: take `i + 1` plays `files[i]` from its first frame, with the clip's own tempo. */
export function fakeTakes(firstId: number, files: readonly FakeAudio[], like: AudioClipView): AudioTakeView[] {
  return files.map((a, i) => ({
    takeId: firstId + i,
    number: i + 1,
    audio: a.id,
    firstFrame: 0,
    frames: a.frames,
    takeOffset: 0,
    sourceTempo: like.sourceTempo,
    state: a.missing ? "missing" : "ready",
  }));
}

/** The clip shows take `number` (from 1) of its `audio.takes`. */
export function fakeShowTake(clip: ClipView, number: number, files: ReadonlyMap<number, FakeAudio>): void {
  const audio = clip.audio!;
  const take = audio.takes[number - 1]!;
  const file = files.get(take.audio);
  Object.assign(clip, { take: number, takeId: take.takeId, takeOffset: take.takeOffset });
  Object.assign(audio, { audio: take.audio, file: file?.file ?? audio.file, firstFrame: take.firstFrame, frames: take.frames, sourceTempo: take.sourceTempo, state: take.state });
}

/**
 * `setComp`, `clearComp`, and on a vocal clip that has its takes listed: `stepTake` (the comp goes in the same step)
 * and `deleteTake` (nothing while there is a comp). `null`: not one of these.
 */
export function reduceComp(state: FakeState, command: Command, files: ReadonlyMap<number, FakeAudio>): Change[] | null {
  if (command.type !== "setComp" && command.type !== "clearComp" && command.type !== "stepTake" && command.type !== "deleteTake") return null;
  const id = command.type === "stepTake" ? state.arrange.selectedClip : command.clip;
  const found = state.song.tracks.flatMap((t) => t.clips).find((c) => c.id === id);
  const listed = (found?.audio?.takes.length ?? 0) >= 2;
  if (command.type === "stepTake" && !listed) return null;
  if (command.type === "deleteTake" && !(found && found.comp.length > 0)) return null;
  const busy = state.transport.state === "countIn" || state.transport.state === "recording";
  if (busy || !found || command.type === "deleteTake") return [];

  const song = structuredClone(state.song);
  const clip = song.tracks.flatMap((t) => t.clips).find((c) => c.id === id)!;
  switch (command.type) {
    case "setComp": {
      const takes = clip.audio?.takes.map((t) => t.takeId) ?? [];
      const plays: CompPiece[] = clip.comp.length > 0 ? clip.comp : [{ from: 0, take: clip.takeId }];
      const fits =
        listed &&
        command.span.start === clip.start &&
        command.span.end === clip.end &&
        command.pieces.length > 0 &&
        fakeCompValid(command.pieces, clip.end - clip.start, takes) &&
        JSON.stringify(command.pieces) !== JSON.stringify(plays);
      if (!fits) return [];
      clip.comp = command.pieces.map((p) => ({ from: p.from, take: p.take }));
      break;
    }
    case "clearComp":
      if (clip.comp.length === 0) return [];
      clip.comp = [];
      break;
    case "stepTake": {
      if (command.delta === 0) return [];
      const count = clip.audio!.takes.length;
      fakeShowTake(clip, ((((clip.take - 1 + command.delta) % count) + count) % count) + 1, files);
      clip.comp = [];
      break;
    }
  }
  Object.assign(song, { undoCount: song.undoCount + 1, redoCount: 0, file: "changed" });
  const changes: Change[] = [{ section: "song", value: song }];
  // 「音程を寄せる」 is for a clip that plays one take.
  const shown = state.params.selected;
  if (shown?.kind === "clip" && shown.clip === clip.id && shown.missing === null && shown.tuneUsable !== (clip.comp.length === 0)) {
    changes.push({ section: "params", value: { ...state.params, selected: { ...shown, tuneUsable: clip.comp.length === 0 } } });
  }
  if (command.type === "stepTake") {
    const track = song.tracks.find((t) => t.clips.includes(clip))!;
    const message = { id: "takeSwitched" as const, track: { number: track.number, name: track.name }, clip: clip.name, take: clip.take, takes: clip.takes };
    changes.push({ section: "status", value: { message } });
  }
  return changes;
}

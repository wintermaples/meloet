// The fake's mute sets (round 12, the owner's answers of 2026-11-24; B-97-3, B-97-5): `saveMuteSet`, `recallMuteSet`,
// `deleteMuteSet` and `renameMuteSet` by Rust's rules (`crates/meloet-model/src/mute_sets.rs`), one step each, and
// `SongView.muteSets` made from the sets the fake keeps on its song (`muteSetsKept`). The fake has no undo, does not rest
// mute lanes on a recall, and keeps solo on the track views (so no solo of a deleted track is left behind).

import type { FakeState } from "./fake";
import { MUTE_SET_NAME_MAX, MUTE_SETS_MAX } from "./types";
import type { Change, Command, HistoryRow, MuteSetView, SongView, StepName, StepWhat } from "./types";

/** A set as the fake keeps it next to the contract's fields (Rust: `meloet_model::mute_sets::MuteSet`). Ids in order. */
export interface FakeMuteSet {
  name: string;
  /** Every track of the song when it was saved. */
  tracks: number[];
  muted: number[];
  /** Return A, return B. */
  mutedReturns: [boolean, boolean];
  soloed: number[];
}

type Song = Pick<SongView, "tracks" | "returns"> & { muteSetsKept?: FakeMuteSet[] };

const busy = (s: FakeState) => s.transport.state === "recording" || s.transport.state === "countIn";

/** `name` as a set keeps it, or null when it is no name. */
function nameOf(name: string): string | null {
  const kept = name.trim();
  const count = [...kept].length;
  return count === 0 || count > MUTE_SET_NAME_MAX || /\p{Cc}/u.test(kept) ? null : kept;
}

const ids = (song: Song, pick: (t: SongView["tracks"][number]) => boolean) =>
  song.tracks
    .filter(pick)
    .map((t) => t.id)
    .sort((a, b) => a - b);

/** A set named `name` that holds `muted`, `soloed` and `mutedReturns` (as the song is now when left out) of every track of `song`. */
export function fakeMuteSet(song: Song, name: string, held: Partial<Omit<FakeMuteSet, "name" | "tracks">> = {}): FakeMuteSet {
  return {
    name,
    tracks: ids(song, () => true),
    muted: held.muted ?? ids(song, (t) => t.muted),
    mutedReturns: held.mutedReturns ?? [song.returns[0].muted, song.returns[1].muted],
    soloed: held.soloed ?? ids(song, (t) => t.soloed),
  };
}

/** A recall would change nothing: the tracks of the set that the song has are muted and soloed as it says, and the returns. */
export function fakeIsNow(set: FakeMuteSet, song: Song): boolean {
  const tracks = song.tracks.filter((t) => set.tracks.includes(t.id));
  return (
    tracks.every((t) => t.muted === set.muted.includes(t.id) && t.soloed === set.soloed.includes(t.id)) &&
    song.returns.every((r, i) => r.muted === set.mutedReturns[i])
  );
}

/** `song` with `muteSets` as the sets it keeps say now. */
export function withMuteSets<T extends Song & { muteSets: MuteSetView[] }>(song: T): T {
  const muteSets = (song.muteSetsKept ?? []).map((set) => ({ name: set.name, asNow: fakeIsNow(set, song) }));
  return JSON.stringify(muteSets) === JSON.stringify(song.muteSets) ? song : { ...song, muteSets };
}

const MUTE_SET_COMMANDS: readonly Command["type"][] = ["saveMuteSet", "recallMuteSet", "deleteMuteSet", "renameMuteSet"];

/** `null`: not one of these commands. `row`: the step made, at `row.index`. */
export function reduceMuteSets(state: FakeState, command: Command): { changes: Change[]; row: HistoryRow | null } | null {
  if (!MUTE_SET_COMMANDS.includes(command.type)) return null;
  const nothing = { changes: [], row: null };
  if (busy(state)) return nothing;
  const song = structuredClone(state.song);
  const sets = song.muteSetsKept ?? [];
  const at = (name: string) => sets.findIndex((s) => s.name === name.trim());
  let name: StepName;
  let what: StepWhat;
  switch (command.type) {
    case "saveMuteSet": {
      const kept = nameOf(command.name);
      if (kept === null) return nothing;
      const set = fakeMuteSet(song, kept);
      const i = at(kept);
      if (i >= 0) {
        if (!command.overwrite || JSON.stringify(sets[i]) === JSON.stringify(set)) return nothing;
        sets[i] = set;
      } else {
        if (sets.length >= MUTE_SETS_MAX) return nothing;
        sets.push(set);
      }
      name = { kind: "saveMuteSet" };
      what = { kind: "name", name: kept };
      break;
    }
    case "recallMuteSet": {
      const set = sets[at(command.name)];
      if (!set) return nothing;
      let muted = false;
      let soloed = false;
      for (const t of song.tracks.filter((t) => set.tracks.includes(t.id))) {
        muted ||= t.muted !== set.muted.includes(t.id);
        soloed ||= t.soloed !== set.soloed.includes(t.id);
        t.muted = set.muted.includes(t.id);
        t.soloed = set.soloed.includes(t.id);
      }
      for (const [i, r] of song.returns.entries()) {
        muted ||= r.muted !== set.mutedReturns[i];
        r.muted = set.mutedReturns[i]!;
      }
      // The solo is no step.
      if (!muted) return soloed ? { changes: [{ section: "song", value: withMuteSets(song) }], row: null } : nothing;
      name = { kind: "recallMuteSet" };
      what = { kind: "name", name: set.name };
      break;
    }
    case "deleteMuteSet": {
      const i = at(command.name);
      if (i < 0) return nothing;
      const [gone] = sets.splice(i, 1);
      name = { kind: "deleteMuteSet" };
      what = { kind: "name", name: gone!.name };
      break;
    }
    case "renameMuteSet": {
      const i = at(command.name);
      const to = nameOf(command.to);
      // The name it has, or the name of another set: nothing.
      if (i < 0 || to === null || at(to) >= 0) return nothing;
      what = { kind: "renamed", from: sets[i]!.name, to };
      sets[i] = { ...sets[i]!, name: to };
      name = { kind: "renameMuteSet" };
      break;
    }
    default:
      return nothing;
  }
  song.muteSetsKept = sets;
  Object.assign(song, { undoCount: song.undoCount + 1, redoCount: 0, file: "changed" });
  const row: HistoryRow = { index: song.undoCount, step: song.undoCount, name, what, place: { kind: "mixer" } };
  return { changes: [{ section: "song", value: withMuteSets(song) }], row };
}

// The clip the piano roll shows, and the content of its take (notes). All parts of the piano roll (note area, velocity lane,
// note list, selected notes, keyboard column) read this. The take is taken by id from the kept takes with `takeNotes` (docs/architecture.md 3-1, item 8).
//
// Usage: the piano roll band part calls `followShownTake()` once at the start (it is inside a part, so $effect can be used).

import { untrack } from "svelte";
import { takeNotes, type ClipView, type TakeNotes, type TrackView } from "../../bridge";
import { current } from "../../state/app.svelte";

// $state.raw: a take's content is large typed arrays, so it is not made a deep proxy.
let clip = $state.raw<ClipView | null>(null);
let track = $state.raw<TrackView | null>(null);
let take = $state.raw<TakeNotes | null>(null);
let origin = $state.raw<TakeNotes | null>(null);
let takeSerial = 0;

export const shown = {
  /** The clip of `arrange.pianoRollClip`. Null if none (a song without clips). */
  get clip(): ClipView | null {
    return clip;
  },
  /** The clip's track. */
  get track(): TrackView | null {
    return track;
  },
  /** The content of the shown take. While fetching, the previous one stays (so it does not flicker). id is `take.id`. */
  get take(): TakeNotes | null {
    return take;
  },
  /** The content of the original take of a quantized take (to draw the border of the position before aligning). Null if none. */
  get origin(): TakeNotes | null {
    return origin;
  },
};

function findClip(id: number | null): { clip: ClipView; track: TrackView } | null {
  const s = current.state;
  if (!s || id === null) return null;
  for (const track of s.song.tracks) {
    const clip = track.clips.find((c) => c.id === id);
    if (clip) return { clip, track };
  }
  return null;
}

/** Call once at the start of the part. Keeps `shown` in step each time the state changes. */
export function followShownTake(): void {
  let asked = 0;
  $effect(() => {
    const found = findClip(current.state?.arrange.pianoRollClip ?? null);
    clip = found?.clip ?? null;
    track = found?.track ?? null;
    const id = found?.clip.takeId ?? null;
    const serial = current.state?.song.serial ?? 0;
    const ticket = ++asked;
    if (id === null) {
      take = null;
      origin = null;
      return;
    }
    // Cannot fetch before connecting (the bridge throws). Once connected, this $effect runs again.
    // The take shown now is only read (setting it to null here does not run this $effect again).
    const now = untrack(() => take);
    if ((now?.id === id && takeSerial === serial) || !current.connected) return;
    // "The previous one stays" is to avoid flicker, but notes of another song are not kept.
    if (now && takeSerial !== serial) {
      take = null;
      origin = null;
    }
    void takeNotes(id).then(async (got) => {
      if (ticket !== asked) return;
      const from = got.origin !== 0 ? await takeNotes(got.origin) : null;
      if (ticket !== asked) return;
      takeSerial = serial;
      take = got;
      origin = from;
    }, () => {
      // Refused (an id core no longer has): show nothing rather than the take of the clip shown before.
      if (ticket !== asked) return;
      take = null;
      origin = null;
    });
  });
}

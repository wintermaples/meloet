// E2: which track 「アルペジオ」 acts on and whether it can be pressed, for the input meter's button and the 「再生」 menu's row.

import type { AppState, Command, TrackView } from "../bridge";

/** The track the keyboards play, when it can have an arpeggio (not a vocal track). */
export function arpTarget(state: AppState): TrackView | null {
  const track = state.song.tracks.find((x) => x.id === state.song.keysTarget);
  return track && track.kind.kind !== "vocal" ? track : null;
}

/** Nothing can be changed while counting in or recording. */
export function arpBusy(state: AppState): boolean {
  return state.transport.state === "countIn" || state.transport.state === "recording";
}

/** The intent that turns the arpeggio of `arpTarget` on or off; null when it cannot be pressed. */
export function arpToggle(state: AppState): Command | null {
  const track = arpTarget(state);
  if (track === null || arpBusy(state)) return null;
  return { type: "setArp", track: track.id, arp: { ...track.arp, on: !track.arp.on } };
}

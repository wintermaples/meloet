// The fake's 「音のクリップにする」 and 「このトラックを音にする」 (round 12, A7). Rust renders the track alone in the background
// (「音にしています… 40 %」) and makes one step when the file is ready; the fake answers at once with a made-up file as
// long as the range and a second of tail.

import type { FakeState, FakeTrack } from "./fake";
import { type FakeAudio, fakeAudioClip, fakeStretch, fakeVocalClip } from "./fakeAudio";
import { FAKE_ARP_OFF } from "./fakeWave3";
import type { Change, Command, StatusMessage } from "./types";
import { TempoMap } from "../time/tempoMap";

/** A song has at most this many tracks (`MAX_TRACKS` in meloet-core/src/play/seats.rs). */
const MAX_TRACKS = 64;
const RATE = 48000;
/** What the fake's tail rings on for. */
const TAIL_S = 1;

const said = (message: StatusMessage): Change[] => [{ section: "status", value: { message } }];

type Asked = { track: FakeTrack; start: number; end: number } | "nothing" | null;

/** The track and the range of the command; "nothing" for a track with no clip; null when it does nothing. */
function range(state: FakeState, command: Extract<Command, { type: "bounceToAudio" | "bounceTrack" }>): Asked {
  const keys = state.song.tracks.filter((t) => t.kind.kind === "keys");
  if (command.type === "bounceTrack") {
    const track = keys.find((t) => t.id === command.track);
    if (!track) return null;
    if (track.clips.length === 0) return "nothing";
    return { track, start: track.clips[0]!.start, end: track.clips.at(-1)!.end };
  }
  const holding = keys.filter((t) => t.clips.some((c) => c.audio === null && command.clips.includes(c.id)));
  if (holding.length !== 1) return null;
  const track = holding[0]!;
  const clips = track.clips.filter((c) => command.clips.includes(c.id));
  return { track, start: Math.min(...clips.map((c) => c.start)), end: Math.max(...clips.map((c) => c.end)) };
}

/**
 * `bounceToAudio`, `bounceTrack` and `cancelBounce`. `files`: what the `peaks` invoke knows, where the made file is put.
 * `copyLane`: the points of a lane under a new id. `null`: not one of these commands.
 */
export function reduceA7(
  state: FakeState,
  command: Command,
  files: Map<number, FakeAudio>,
  copyLane: (from: number, to: number) => void,
): Change[] | null {
  if (command.type === "cancelBounce") {
    if (!state.audioWork.bounce) return [];
    return [{ section: "audioWork", value: { ...state.audioWork, bounce: null } }, ...said({ id: "bounceCancelled" })];
  }
  if (command.type !== "bounceToAudio" && command.type !== "bounceTrack") return null;
  if (state.transport.state === "countIn" || state.transport.state === "recording") return [];
  const asked = range(state, command);
  if (!asked) return [];
  if (asked === "nothing") return said({ id: "nothingToBounce" });
  if (state.audioWork.bounce || state.export.dialog || state.export.run?.state === "running") return said({ id: "bounceBusy" });
  if (state.song.tracks.length >= MAX_TRACKS) return said({ id: "tooManyTracks" });

  const song = structuredClone(state.song);
  const at = song.tracks.findIndex((t) => t.id === asked.track.id);
  const from = song.tracks[at]!;
  const ids = [
    ...song.tracks.flatMap((t) => [t.id, ...t.clips.flatMap((c) => [c.id, c.takeId, c.audio?.audio ?? 0]), ...t.lanes.map((l) => l.laneId)]),
    ...files.keys(),
  ];
  let next = Math.max(0, ...ids) + 1;
  const names = new Set(song.tracks.map((t) => t.name));
  let name = `${from.name}（音）`;
  for (let n = 2; names.has(name); n++) name = `${from.name}（音） ${n}`;
  const map = new TempoMap(state.timeline);
  const seconds = map.secondsAt(asked.end) - map.secondsAt(asked.start) + TAIL_S;
  const made: FakeAudio = { id: next++, file: "", sampleRate: RATE, bits: 24, channels: 2, frames: Math.round(seconds * RATE) };
  made.file = `${name} ${made.id}.wav`;
  files.set(made.id, made);
  const end = Math.ceil(map.tickAt(map.secondsAt(asked.start) + made.frames / RATE) - 1e-6);
  const clip = fakeVocalClip(next++, name, asked.start, end, fakeAudioClip(made, fakeStretch(null, map.tempoAt(asked.start), { follow: false })));
  next += 1;
  const lanes = from.lanes
    .filter((l) => l.target.kind === "volume" || l.target.kind === "pan" || l.target.kind === "send")
    .map((l) => {
      const laneId = next++;
      copyLane(l.laneId, laneId);
      return { ...l, laneId };
    });
  const track: FakeTrack = {
    ...structuredClone(from),
    id: next++,
    name,
    color: ((at + 1) % 5) + 1,
    kind: { kind: "vocal", input: "mono1", inputReady: true, armed: false, monitor: false },
    sound: { kind: "builtin", instrument: "sine" },
    standIn: null,
    drumKeys: null,
    keyNames: null,
    slots: [null, null, null, null],
    muted: false,
    soloed: false,
    lanes,
    clips: [clip],
    arp: FAKE_ARP_OFF,
    instrument: "sine",
    inserts: [null, null, null, null],
  };
  from.muted = true;
  song.tracks.splice(at + 1, 0, track);
  song.tracks.forEach((t, i) => (t.number = i + 1));
  Object.assign(song, { undoCount: song.undoCount + 1, redoCount: 0, file: "changed" });
  return [{ section: "song", value: song }, ...said({ id: "bounced", track: { number: track.number, name } })];
}

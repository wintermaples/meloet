// The fake's audio files and their peaks (protocol 24): what the `peaks` invoke answers for the scenes of
// F7-AudioEdit, F7-AudioDrop, F7-AudioMissing and R11-Audio. The waves are made up: bursts like sung
// syllables, the same for the same file every time.

import { PEAK_LEVELS, PEAKS_MAX_COUNT } from "./generated/audio";
import { encodePeaks } from "./peaks";
import type { AudioClipView, AudioTakeView, ClipView, SourceTempoView, StretchView } from "./types";

export interface FakeAudio {
  id: number;
  file: string;
  sampleRate: number;
  bits: number | null;
  channels: number;
  frames: number;
  /** A recording going on or a file being read: only this many frames have peaks yet. */
  growingTo?: number;
  /** The file is not found: `peaks` rejects. */
  missing?: boolean;
  /** Made by 「逆にする」: the frames of `audio` before `end`, backwards. */
  backwardsOf?: { audio: FakeAudio; end: number };
}

/** A number in 0..1 from two whole numbers, the same every time. */
function noise(a: number, b: number): number {
  let h = Math.imul(a ^ 0x9e3779b9, 0x85ebca6b) ^ Math.imul(b + 0x7f4a7c15, 0xc2b2ae35);
  h ^= h >>> 15;
  h = Math.imul(h, 0x2c1b3c6d);
  h ^= h >>> 12;
  return (h >>> 0) / 0xffffffff;
}

/** How loud the made-up voice is at `seconds`: syllables of a third of a second, phrases of a few seconds, breaths between. */
function loudness(audio: number, seconds: number): number {
  const syllable = Math.floor(seconds * 3);
  const within = seconds * 3 - syllable;
  const strength = 0.25 + 0.75 * noise(audio, syllable);
  const phrase = Math.sin((seconds / 3.2 + noise(audio, -1)) * Math.PI) ** 2;
  const attack = Math.min(1, within * 8) * Math.exp(-within * 2.2);
  return Math.min(1, strength * (0.15 + 0.85 * phrase) * attack);
}

/** The finest peak `index` of `channel`: smallest and largest sample as i16. */
function finestPeak(a: FakeAudio, channel: number, index: number): [number, number] {
  if (a.backwardsOf) {
    const { audio, end } = a.backwardsOf;
    return finestPeak(audio, channel, Math.max(0, Math.floor((end - 1 - index * PEAK_LEVELS[0]!) / PEAK_LEVELS[0]!)));
  }
  const seconds = (index * PEAK_LEVELS[0]!) / a.sampleRate;
  const loud = loudness(a.id, seconds) * (channel === 0 ? 1 : 0.85);
  const up = loud * (0.7 + 0.3 * noise(a.id * 31 + channel, index));
  const down = loud * (0.7 + 0.3 * noise(a.id * 37 + channel, index));
  return [-Math.round(down * 32767), Math.round(up * 32767)];
}

/** How many peaks of `level` the frames of the file that are there have. */
export function fakePeakCount(a: FakeAudio, level: number): number {
  return Math.ceil((a.growingTo ?? a.frames) / PEAK_LEVELS[level]!);
}

/** The values of the peaks `first .. first + count` of `level`: every coarser peak is the widest of its finest ones. */
export function fakePeakValues(a: FakeAudio, level: number, first: number, count: number): Int16Array {
  const end = Math.min(first + count, fakePeakCount(a, level));
  const values = new Int16Array(Math.max(0, end - first) * a.channels * 2);
  const fan = PEAK_LEVELS[level]! / PEAK_LEVELS[0]!;
  const finest = fakePeakCount(a, 0);
  for (let index = first; index < end; index++) {
    for (let channel = 0; channel < a.channels; channel++) {
      let low = 32767;
      let high = -32768;
      for (let i = index * fan; i < Math.min((index + 1) * fan, finest); i++) {
        const [down, up] = finestPeak(a, channel, i);
        low = Math.min(low, down);
        high = Math.max(high, up);
      }
      const at = ((index - first) * a.channels + channel) * 2;
      values[at] = low;
      values[at + 1] = high;
    }
  }
  return values;
}

/** What `peaks(audio, level, first, count)` answers for the file. */
export function fakePeaks(a: FakeAudio, level: number, first: number, count: number): ArrayBuffer {
  if (a.missing) throw new Error(`fake: no peaks of audio ${a.id}`);
  return encodePeaks({
    audio: a.id,
    level,
    channels: a.channels,
    growing: a.growingTo !== undefined,
    first,
    total: fakePeakCount(a, level),
    values: fakePeakValues(a, level, first, Math.min(count, PEAKS_MAX_COUNT)),
  });
}

/** The tempo of the fake's songs, which its recorded audio was sung at. */
export const FAKE_SONG_BPM = 96;

/** One tempo all along: of recorded audio where the song has no tempo points, and of a tempo typed or found. */
export const steadyTempo = (tempoBpm: number): SourceTempoView => ({ tempoBpm, headRamp: false, tempoPoints: [] });

/**
 * `stretch` and `sourceTempo` of a clip whose audio has the tempo `source` (null: 「—」), in a song that is at
 * `songBpm` at the clip's start. As Rust sends them: the ratio and the tempo points only while the clip follows.
 */
export function fakeStretch(
  source: SourceTempoView | null,
  songBpm: number,
  more: Partial<Pick<StretchView, "follow" | "semitones" | "cents" | "keepFormant">> = {},
): Pick<AudioClipView, "stretch" | "sourceTempo"> {
  const shape = { follow: source !== null, semitones: 0, cents: 0, keepFormant: true, ...more };
  const follows = shape.follow && source !== null;
  return {
    stretch: { ...shape, sourceBpm: source?.tempoBpm ?? null, ratio: follows ? songBpm / source.tempoBpm : null, tune: null },
    sourceTempo: follows ? source : null,
  };
}

/** `ClipView.audio` of a clip that plays `a` from its first frame, as recorded (it follows the tempo it was sung at, 96 a minute). */
export function fakeAudioClip(a: FakeAudio, more: Partial<AudioClipView> = {}): AudioClipView {
  return {
    audio: a.id,
    file: a.file,
    sampleRate: a.sampleRate,
    bits: a.bits,
    channels: a.channels,
    resampled: a.sampleRate !== 48000,
    firstFrame: 0,
    frames: a.frames,
    fadeIn: 0,
    fadeOut: 0,
    gainDb: 0,
    state: a.missing ? "missing" : a.growingTo !== undefined ? "loading" : "ready",
    ...fakeStretch(steadyTempo(FAKE_SONG_BPM), FAKE_SONG_BPM),
    takes: [],
    ...more,
  };
}

/**
 * A vocal clip. Its take id is the clip's id + 1, as recorded clips have it in the fake. With no takes given, `audio.takes`
 * lists the shown one, as Rust always does.
 */
export function fakeVocalClip(id: number, name: string, start: number, end: number, audio: AudioClipView, takeOffset = 0): ClipView {
  const shown: AudioTakeView = {
    takeId: id + 1,
    number: 1,
    audio: audio.audio,
    firstFrame: audio.firstFrame,
    frames: audio.frames,
    takeOffset,
    sourceTempo: audio.sourceTempo,
    state: audio.state,
  };
  const listed = audio.takes.length > 0 ? audio : { ...audio, takes: [shown] };
  return { id, name, start, end, take: 1, takes: 1, takeId: id + 1, takeOffset, noteCount: 0, audio: listed, comp: [] };
}

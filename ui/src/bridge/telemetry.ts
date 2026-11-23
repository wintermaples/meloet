// Telemetry binary, version 5, little-endian. The layout is fixed in
// crates/meloet-protocol/src/telemetry.rs (table: docs/architecture.md 3-3).

import type { TransportState } from "./types";

export const TELEMETRY_KIND = 1;
export const TELEMETRY_VERSION = 5;
export const TELEMETRY_HEADER_LEN = 124;
/** The 16-bit value of `lufsShort`, `lufsIntegrated` and `truePeakDb` when there is none. */
export const TELEMETRY_NO_VALUE = -32768;
/** E7: the bars of `spectrum`, low to high. */
export const SPECTRUM_BANDS = 24;
/** One mic meter: the track's id, the peak left and right, the flags and 3 spare bytes. */
export const TELEMETRY_MIC_LEN = 16;
const HEADER_LEN = TELEMETRY_HEADER_LEN;
const STATES: readonly TransportState[] = ["stopped", "countIn", "playing", "recording"];

/**
 * What the listener hears at `sentUs`. Between messages:
 * `tick + (rustNowUs - sentUs) * ticksPerUs` (see `positionAt`).
 */
export interface TransportReading {
  /** Song position in ticks, with the fraction. Negative during a count-in from near 0. */
  tick: number;
  /** 0 while stopped. */
  ticksPerUs: number;
  state: TransportState;
  /** Beats still to come before recording starts, the one sounding included; 0 outside a count-in. */
  countInBeatsLeft: number;
  /** Pass of the loop while recording, 0 for the first. */
  lap: number;
}

/** The input of one vocal track: what comes in, before the track's fader (spec 3, 31-C). */
export interface MicMeter {
  /** `TrackView.id`. */
  track: number;
  /** Left, right, as `masterPeak`. A mono input has the same value in both. */
  peak: [number, number];
  /** The input reached 0 dB since the last message. */
  clipped: boolean;
  /** The track records two channels (「入口 1 ・ 2 ・ ステレオ」). */
  stereo: boolean;
}

export interface Telemetry {
  seq: number;
  /** When Rust sent it: CLOCK_MONOTONIC, whole microseconds. */
  sentUs: number;
  /** Held keys as four 32-bit words; bit n of word w is note 32 * w + n. */
  keys: Uint32Array;
  /** 0 = none yet. */
  lastVelocity: number;
  sustain: number;
  modulation: number;
  running: boolean;
  masterClipped: boolean;
  /** -8192..8191 */
  pitchBend: number;
  loadPermille: number;
  /** Left, right. Largest value since the last message; 1.0 = 0 dBFS. */
  masterPeak: [number, number];
  /** null before the audio has played anything. */
  position: TransportReading | null;
  /** Return A, then return B, left and right of each: [aL, aR, bL, bR], as `masterPeak`. */
  returnPeaks: Float32Array;
  /** Left, right per track: [l1, r1, l2, r2, ...]. */
  trackPeaks: Float32Array;
  /** The vocal tracks whose input is metered now, in the order of the tracks. Empty while the input is not open. */
  mics: MicMeter[];
  /** M2: how far the master's limiter holds the sound down, dB in 0.1 steps (25.5 at most); 0 when it does not. */
  masterReductionDb: number;
  /** E7: the master's sound from low to high, 40 Hz to 16 kHz in 24 bands, each −72..0 dB as 0..255. All 0 when silent. */
  spectrum: Uint8Array;
  /** M3 「いまの大きさ」: LUFS of the last 3 s, in 0.01 steps; null when there is none. */
  lufsShort: number | null;
  /** M3 「曲ぜんたい」: LUFS since the play started; null when there is none. */
  lufsIntegrated: number | null;
  /** M3 「トゥルーピーク」: dBTP, the highest since the play started; null when there is none. */
  truePeakDb: number | null;
}

export function decodeTelemetry(data: ArrayBuffer | ArrayBufferView): Telemetry {
  const view = ArrayBuffer.isView(data)
    ? new DataView(data.buffer, data.byteOffset, data.byteLength)
    : new DataView(data);
  if (view.byteLength < HEADER_LEN) {
    throw new Error(`telemetry is ${view.byteLength} bytes, shorter than the header`);
  }
  if (view.getUint8(0) !== TELEMETRY_KIND) {
    throw new Error(`telemetry kind ${view.getUint8(0)}, expected ${TELEMETRY_KIND}`);
  }
  if (view.getUint8(1) !== TELEMETRY_VERSION) {
    throw new Error(`telemetry version ${view.getUint8(1)}, expected ${TELEMETRY_VERSION}`);
  }
  const tracks = view.getUint16(2, true);
  const micCount = view.getUint16(88, true);
  const expected = HEADER_LEN + 8 * tracks + TELEMETRY_MIC_LEN * micCount;
  if (view.byteLength !== expected) {
    throw new Error(`telemetry is ${view.byteLength} bytes, expected ${expected}`);
  }

  const keys = new Uint32Array(4);
  for (let w = 0; w < 4; w++) keys[w] = view.getUint32(16 + 4 * w, true);
  const returnPeaks = new Float32Array(4);
  for (let i = 0; i < 4; i++) returnPeaks[i] = view.getFloat32(72 + 4 * i, true);
  const trackPeaks = new Float32Array(2 * tracks);
  for (let i = 0; i < trackPeaks.length; i++) {
    trackPeaks[i] = view.getFloat32(HEADER_LEN + 4 * i, true);
  }
  const mics: MicMeter[] = [];
  for (let i = 0, p = HEADER_LEN + 8 * tracks; i < micCount; i++, p += TELEMETRY_MIC_LEN) {
    const micFlags = view.getUint8(p + 12);
    mics.push({
      track: view.getUint32(p, true),
      peak: [view.getFloat32(p + 4, true), view.getFloat32(p + 8, true)],
      clipped: (micFlags & 1) !== 0,
      stereo: (micFlags & 2) !== 0,
    });
  }
  const hundredths = (at: number): number | null => {
    const v = view.getInt16(at, true);
    return v === TELEMETRY_NO_VALUE ? null : v / 100;
  };
  const spectrum = new Uint8Array(SPECTRUM_BANDS);
  for (let i = 0; i < SPECTRUM_BANDS; i++) spectrum[i] = view.getUint8(100 + i);
  const flags = view.getUint8(35);
  const state = STATES[view.getUint8(64)];
  if (state === undefined) throw new Error(`telemetry transport state ${view.getUint8(64)}`);

  return {
    seq: view.getUint32(4, true),
    // Nanoseconds do not fit a double after 104 days of uptime; whole microseconds do.
    sentUs: Number(view.getBigUint64(8, true) / 1000n),
    keys,
    lastVelocity: view.getUint8(32),
    sustain: view.getUint8(33),
    modulation: view.getUint8(34),
    running: (flags & 1) !== 0,
    masterClipped: (flags & 2) !== 0,
    pitchBend: view.getInt16(36, true),
    loadPermille: view.getUint16(38, true),
    masterPeak: [view.getFloat32(40, true), view.getFloat32(44, true)],
    position:
      (flags & 4) === 0
        ? null
        : {
            tick: view.getFloat64(48, true),
            ticksPerUs: view.getFloat64(56, true),
            state,
            countInBeatsLeft: view.getUint16(66, true),
            lap: view.getUint32(68, true),
          },
    returnPeaks,
    trackPeaks,
    mics,
    masterReductionDb: view.getUint8(98) / 10,
    spectrum,
    lufsShort: hundredths(92),
    lufsIntegrated: hundredths(94),
    truePeakDb: hundredths(96),
  };
}

export function keyIsDown(telemetry: Telemetry, note: number): boolean {
  return ((telemetry.keys[note >>> 5]! >>> (note & 31)) & 1) === 1;
}

/** The song position heard at `rustNowUs` (Rust's clock, see `clockOffsetUs`). */
export function positionAt(telemetry: Telemetry, rustNowUs: number): number | null {
  const p = telemetry.position;
  return p === null ? null : p.tick + (rustNowUs - telemetry.sentUs) * p.ticksPerUs;
}

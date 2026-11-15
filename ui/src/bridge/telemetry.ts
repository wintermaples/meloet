// Telemetry binary, version 2, little-endian. The layout is fixed in
// crates/meloet-protocol/src/telemetry.rs (table: docs/architecture.md 3-3).

import type { TransportState } from "./types";

export const TELEMETRY_KIND = 1;
export const TELEMETRY_VERSION = 2;
export const TELEMETRY_HEADER_LEN = 72;
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
  /** Left, right per track: [l1, r1, l2, r2, ...]. */
  trackPeaks: Float32Array;
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
  const expected = HEADER_LEN + 8 * tracks;
  if (view.byteLength !== expected) {
    throw new Error(`telemetry is ${view.byteLength} bytes, expected ${expected}`);
  }

  const keys = new Uint32Array(4);
  for (let w = 0; w < 4; w++) keys[w] = view.getUint32(16 + 4 * w, true);
  const trackPeaks = new Float32Array(2 * tracks);
  for (let i = 0; i < trackPeaks.length; i++) {
    trackPeaks[i] = view.getFloat32(HEADER_LEN + 4 * i, true);
  }
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
    trackPeaks,
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

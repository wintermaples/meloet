// The fade shape of audio clips (spec 3 (27-A): 「線は曲がった形 1 つ（大きさがへこまない形）」).
// A copy of Rust's `crates/meloet-model/src/audio/fade.rs`, so the shape the engine applies to the sound matches the screen's line and waveform.
// Same answers are checked with the table `fade.table.json` that a Rust test writes (tests/arrange.fade.test.ts).

/** Fade-in level at x along its length (0 = start, 1 = end). Fade-out is `fadeGain(1 - x)`. */
export function fadeGain(x: number): number {
  if (x >= 1) return 1;
  if (x > 0) return Math.sin((x * Math.PI) / 2);
  return 0;
}

/**
 * The level both fades apply at `at` from the clip start. All numbers in the same unit (ticks or px).
 * 0 outside the clip. The clip gain is not included.
 */
export function clipFadeGain(at: number, length: number, fadeIn: number, fadeOut: number): number {
  if (!(at >= 0 && at < length)) return 0;
  const goingIn = fadeIn > 0 ? fadeGain(at / fadeIn) : 1;
  const goingOut = fadeOut > 0 ? fadeGain((length - at) / fadeOut) : 1;
  return goingIn * goingOut;
}

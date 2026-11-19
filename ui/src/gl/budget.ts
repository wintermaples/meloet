// Every place on the screen that has a WebGL canvas, and how many WebGL contexts the page may make.
//
// WebKitGTK 2.52 keeps 16 WebGL contexts per page. Making the 17th loses the oldest one for good.
// A context counts until it is garbage collected, even after loseContext() and removing its canvas,
// so canvases are made once and used again (pool.ts), one per region rather than one per slot.

/** WebKitGTK's own limit per page. */
export const WEBKIT_CONTEXT_LIMIT = 16;

/**
 * How many contexts meloet may make in a page's life. The rest of the limit is left for mistakes.
 * The regions below need at most 10 at once: a window tall enough to keep the arrange in track rows
 * with the piano roll and the mixer open, and the sound settings on top (tests/gl.surface.test.ts).
 */
export const CONTEXT_BUDGET = 12;

/**
 * A region is a box marked with `use:glRegion`; every slot inside it is drawn into the region's one
 * canvas. `opaque`: its slots paint every pixel of the canvas, so nothing behind needs to show.
 */
export const REGIONS = {
  /** Input meter: the keyboard and the bar beside the velocity (text lies between them). Changes with each key. */
  inputMeter: { opaque: false },
  /** Transport: the master meter. Up to 60 Hz while there is sound. */
  masterMeter: { opaque: false },
  /** Arrange: the meters of the track heads, all tracks in one canvas. Up to 60 Hz. */
  trackMeters: { opaque: false },
  /** Arrange: bar lines, clips and the finished bars of the clip being recorded. Changes only with the song, the size or the scroll. */
  arrangeLanes: { opaque: false },
  /** The arrange folded to its overview. Takes the place of trackMeters and arrangeLanes. */
  overview: { opaque: false },
  /** Arrange: the growing end of the clip being recorded (at most two bars). 60 Hz while recording. */
  recordingClip: { opaque: false },
  /** Piano roll: rows, grid lines, notes, the frames of where notes were, and the velocity bars. Changes with the take, the selection, the scroll and the zoom. */
  pianoRoll: { opaque: false },
  /** Piano roll: the keys column. Lights with each key held or sounding. */
  pianoRollKeys: { opaque: true },
  /** Piano roll: what moves with the mouse or every frame (the notes being dragged, the held notes of a recording). As small as what it shows. */
  pianoRollLive: { opaque: false },
  /** Mixer: the meters of every strip (tracks, returns, master) in one canvas, also when the strips scroll. Up to 60 Hz. */
  mixerMeters: { opaque: false },
  /** Sound settings: the level beside the test tone and the keyboard that lights (rows of text lie between them). */
  settings: { opaque: false },
} as const satisfies Record<string, { opaque: boolean }>;

export type RegionName = keyof typeof REGIONS;

/**
 * A frame that shows the canvas of the key shows the canvas of the value too, changed or not.
 * Where WebKit paints the page on the CPU, it repaints one rectangle around everything that changed
 * when the changes fill a quarter of that rectangle. The mixer's meters are wide and low on the
 * screen; with the master meter (top right) always among the changes, the rectangle around them is
 * big enough for each to be painted by itself (docs/architecture.md, chapter 7, "CPU で描く WebKit").
 */
export const SHOWN_WITH: Partial<Record<RegionName, RegionName>> = { mixerMeters: "masterMeter" };

/**
 * How often a region may be shown, in Hz. The mixer's meters of more than 10 tracks are drawn at 30 Hz (spec 3, the 8th
 * edition): they look the same, and the page paints half as often. The keys and the play line stay at 60 Hz.
 */
export function regionHz(name: RegionName, tracks: number): number {
  return name === "mixerMeters" && tracks > MIXER_FULL_RATE_TRACKS ? 30 : 60;
}
export const MIXER_FULL_RATE_TRACKS = 10;

/** A mistake in how the surfaces are used: stops development and tests; only logged in the app. */
export function fault(message: string): void {
  if (import.meta.env.DEV) throw new Error(message);
  console.error(message);
}

// A Transport with no Rust behind it: for tests, and for looking at the screen in a plain
// browser (`npm run dev`, `vite preview`). `?scene=<name>&scale=<100|125|150>` picks a fixed
// scene (see SCENES). It answers the few commands below; the band heights follow the same
// numbers as crates/meloet-core/src/layout.rs, but meloet-core stays the rule book.

import type { Transport } from "./session";
import { CONTROL_CC, CONTROL_PITCH_BEND, NO_FROM, TAKE_HEADER_LEN, TAKE_KIND, TAKE_NOTE_LEN, TAKE_VERSION } from "./take";
import { TELEMETRY_HEADER_LEN, TELEMETRY_KIND, TELEMETRY_VERSION, type TransportReading } from "./telemetry";
import {
  PROTOCOL_VERSION,
  type AppState,
  type Band,
  type Change,
  type ClipView,
  type Command,
  type EndpointView,
  type IndexRange,
  type LayoutView,
  type LiveNote,
  type LiveNotesSnapshot,
  type PianoRollView,
  type ServerMessage,
  type SongDialogKind,
  type SongView,
  type ArrangeView,
  type StatusMessage,
  type TrackRef,
} from "./types";

// ---- the band rules (a copy of the numbers in meloet-core/src/layout.rs) ----

const MENU = 24;
const TRANSPORT = 48;
const BAND_ROW = 24;
const STATUS = 24;
const METER_FULL = 52;
const METER_SLIM = 24;
const OVERVIEW = 40;
const ARRANGE_ROWS = 136;
const SMALLEST: Partial<Record<Band, number>> = { pianoRoll: 268, mixer: 516, history: 248 };
const LARGEST: Partial<Record<Band, number>> = { pianoRoll: 736, mixer: 660, history: 248 };
const BAND_TOOLS = 32;
const MAX_BOTTOM_OPEN = 2;
const MIN_WINDOW = { width: 853, height: 448 };
const SCALES = [100, 125, 150];
const BOTTOM_ORDER: Band[] = ["pianoRoll", "mixer", "history"];
const isBottom = (b: Band) => BOTTOM_ORDER.includes(b);

/** The smallest window in CSS px at this zoom. */
export function minWindow(scale: number): { width: number; height: number } {
  return { width: Math.ceil((MIN_WINDOW.width * scale) / 100), height: Math.ceil((MIN_WINDOW.height * scale) / 100) };
}

/** The zooms whose smallest window does not fit this screen space. 100 is never one of them. */
export function unfitScales(screen: { width: number; height: number }): number[] {
  return SCALES.slice(1).filter((s) => minWindow(s).width > screen.width || minWindow(s).height > screen.height);
}

export class FakeBands {
  /** Oldest first. */
  bottom: Band[] = [];
  /** Oldest first; the last one is in front. */
  side: Band[] = [];

  /** Window height in px at 100 %. */
  constructor(public height: number) {}

  private meterHeight(): number {
    return this.bottom.length === 0 ? METER_FULL : METER_SLIM;
  }

  /** What is left with the arrange view at 40 and every open band at its smallest. */
  private spare(): number {
    const fixed = MENU + TRANSPORT + this.meterHeight() + BAND_ROW + STATUS;
    return this.height - fixed - OVERVIEW - this.bottom.reduce((sum, b) => sum + (SMALLEST[b] ?? 0), 0);
  }

  /** Folds the older band when two do not fit, and returns it. The newest band is never folded. */
  private fit(): Band | null {
    return this.bottom.length > 1 && this.spare() < 0 ? this.bottom.shift()! : null;
  }

  /** Returns the band that was folded because the window got too low for both. */
  setHeight(height: number): Band | null {
    this.height = height;
    return this.fit();
  }

  /**
   * Returns the band folded to open this one, and why ("third": two at most; "tooLow": they do not
   * fit together). When two are folded, the later fold is returned.
   */
  toggle(band: Band): { band: Band; why: "third" | "tooLow" } | null {
    if (isBottom(band)) {
      const at = this.bottom.indexOf(band);
      if (at >= 0) this.bottom.splice(at, 1);
      else {
        this.bottom.push(band);
        const third = this.bottom.length > MAX_BOTTOM_OPEN ? this.bottom.shift()! : null;
        const low = this.fit();
        if (low) return { band: low, why: "tooLow" };
        return third ? { band: third, why: "third" } : null;
      }
    } else if (this.side.at(-1) === band) {
      this.side.pop();
    } else {
      this.side = [...this.side.filter((b) => b !== band), band];
    }
    return null;
  }

  goArrange(): void {
    this.bottom = [];
  }

  back(): void {
    if (this.side.pop() === undefined) this.bottom.pop();
  }

  layout(): LayoutView {
    let spare = this.spare();
    const heights = new Map<Band, number>();
    // Newest first: it grows first.
    for (const band of [...this.bottom].reverse()) {
      const smallest = SMALLEST[band] ?? 0;
      const grow = Math.max(0, Math.min(spare, (LARGEST[band] ?? smallest) - smallest));
      spare -= grow;
      heights.set(band, smallest + grow);
    }
    // A band that is open alone and still does not fit gets what is left.
    if (this.bottom.length === 1 && spare < 0) {
      const only = this.bottom[0]!;
      heights.set(only, Math.max(BAND_TOOLS, heights.get(only)! + spare));
    }
    const arrangeHeight = OVERVIEW + Math.max(0, spare);
    return {
      meter: this.meterHeight() === METER_FULL ? "full" : "slim",
      arrangeHeight,
      arrangeOverview: arrangeHeight < ARRANGE_ROWS,
      lower: BOTTOM_ORDER.filter((b) => heights.has(b)).map((band) => ({ band, height: heights.get(band)! })),
      rightOpen: [...this.side],
      rightFront: this.side.at(-1) ?? null,
      showBackHint: this.bottom.length > 0,
    };
  }
}

// ---- the scenes ----

const RYZEN = "Ryzen HD Audio Controller";
const GB206 = "GB206 High Definition Audio Controller";
const RADEON = "Radeon High Definition Audio Controller";

const HEADPHONES: EndpointView = {
  id: "out-headphones",
  name: "アナログヘッドフォン",
  friendly: "headphones",
  availability: "yes",
  link: "jack",
  nodeName: "alsa_output.pci-0000_0c_00.6.analog-stereo",
  device: RYZEN,
  unusable: null,
};
const USB_AUDIO: EndpointView = {
  id: "out-usb",
  name: "USB オーディオ",
  friendly: null,
  availability: "unknown",
  link: "plug",
  nodeName: "alsa_output.usb-0d8c_USB_Audio-00.analog-stereo",
  device: "USB Audio Device",
  unusable: null,
};
const LINE_OUT: EndpointView = {
  id: "out-line",
  name: "ライン出力",
  friendly: "lineOut",
  availability: "no",
  link: "jack",
  nodeName: HEADPHONES.nodeName,
  device: RYZEN,
  unusable: "switchedByPc",
};
const HDMI: EndpointView = {
  id: "out-hdmi2",
  name: "HDMI / DisplayPort 2",
  friendly: "hdmi",
  availability: "no",
  link: "plug",
  nodeName: null,
  device: RADEON,
  unusable: "stoppedByPc",
};
const OUTPUTS = [HEADPHONES, LINE_OUT, HDMI];
const hdmi = (id: string, name: string, device: string): EndpointView => ({ ...HDMI, id, name, device });
/**
 * The outputs of a PC with two cards of HDMI / DisplayPort, in the order meloet-core gives:
 * the usable ones, then the 8 that cannot be chosen.
 */
const PC_OUTPUTS = [
  HEADPHONES,
  USB_AUDIO,
  LINE_OUT,
  hdmi("out-hdmi-a1", "HDMI / DisplayPort", GB206),
  hdmi("out-hdmi-a2", "HDMI / DisplayPort 2", GB206),
  hdmi("out-hdmi-a3", "HDMI / DisplayPort 3", GB206),
  hdmi("out-hdmi-a4", "HDMI / DisplayPort 4", GB206),
  hdmi("out-hdmi-b1", "HDMI / DisplayPort", RADEON),
  hdmi("out-hdmi-b2", "HDMI / DisplayPort 2", RADEON),
  hdmi("out-hdmi-b3", "HDMI / DisplayPort 3", RADEON),
];

/** The voice inputs of the same PC: the rear mic, then the 2 that cannot be chosen. */
const MIC: EndpointView = {
  id: "in-rear-mic",
  name: "リアマイクロフォン",
  friendly: "rearMic",
  availability: "yes",
  link: "jack",
  nodeName: "alsa_input.pci-0000_0c_00.6.analog-stereo",
  device: RYZEN,
  unusable: null,
};
const PC_INPUTS = [
  MIC,
  { ...MIC, id: "in-front-mic", name: "フロントマイクロフォン", friendly: null, availability: "no", unusable: "switchedByPc" },
  { ...MIC, id: "in-line", name: "ライン入力", friendly: null, availability: "no", unusable: "stoppedByPc" },
] satisfies EndpointView[];

const keyboard = (name: string, n = 1) => ({ id: `kb-${n}`, name, portName: name, address: `${23 + n}:0` });

const PIANO: TrackRef = { number: 1, name: "ピアノ" };
const NEW_SONG: StatusMessage = { id: "newSong", tempoBpm: 96, meterNum: 4, meterDen: 4, track: PIANO };

/** One clip selected (and shown in the piano roll), or none; nothing to paste. */
export function arrangeOf(clip: number | null): ArrangeView {
  return { selectedClips: clip === null ? [] : [clip], selectedClip: clip, pianoRollClip: clip, clipboardClips: 0 };
}

export function fakeState(): AppState {
  const bands = new FakeBands(800);
  return {
    song: {
      serial: 1,
      name: "はじめての曲",
      tempoBpm: 96,
      meterNum: 4,
      meterDen: 4,
      tempoChanges: 0,
      undoCount: 0,
      redoCount: 0,
      noteCount: 0,
      tracks: [{ number: 1, name: "ピアノ", color: 1, instrument: "placeholder", gainDb: 0, armed: true, muted: false, soloed: false, clips: [] }],
      playTarget: 1,
      loopRange: null,
      loopOn: false,
      file: "neverSaved",
    },
    transport: {
      state: "stopped",
      position: 0,
      metronome: false,
      countInBars: 1,
      countInBeats: 4,
      canPlay: false,
      canRecord: false,
      recording: null,
    },
    arrange: arrangeOf(null),
    pianoRoll: {
      take: null,
      selectedNotes: [],
      selectedControls: [],
      grid: { division: 16, triplet: false },
      scale: { root: 0, mode: "major", lock: false },
      quantize: { division: 16, triplet: false, strength: 100, swing: 0, align: "start" },
      clipboardNotes: 0,
    },
    workspace: {
      scale: 100,
      viewportWidth: 1280,
      viewportHeight: 800,
      unfitScales: [],
      layout: bands.layout(),
      settings: null,
      chooser: null,
      hint: null,
      songDialog: null,
      recovery: null,
    },
    keyboards: { list: [], active: null, lost: null, inputStopped: false, velocityFixed: { on: false, value: 100 } },
    outputs: { list: OUTPUTS.map((o) => ({ ...o })), chosen: null, chosenState: null },
    inputs: { list: [], chosen: null },
    audio: {
      running: false,
      system: "PipeWire",
      sampleRate: null,
      requestedFrames: 256,
      actualFrames: null,
      forcedQuantum: null,
      latencyMs: null,
      latencyMsAtRequested: null,
      outputDevice: null,
      midiInput: null,
    },
    status: { message: NEW_SONG },
  };
}

/** What the meters show in a scene. Sent again and again, so the WebGL side has something to draw. */
export interface FakeMeters {
  keys?: number[];
  velocity?: number;
  sustain?: number;
  modulation?: number;
  pitchBend?: number;
  loadPermille?: number;
  /** 1.0 = 0 dBFS */
  master?: [number, number];
  tracks?: [number, number][];
  /** Where the listener is when the scene starts. It moves by `ticksPerUs`; scenes that stand for a mock use 0. */
  position?: TransportReading;
}

interface Scene {
  state: AppState;
  bands: FakeBands;
  meters: FakeMeters;
  /** Keep every scrolling box (`data-gl-scroll`) scrolled to its end. */
  scrollToEnd?: boolean;
  live: LiveNotesSnapshot;
}

interface SceneDef {
  setup(s: AppState): void;
  bands?: Band[];
  meters: FakeMeters;
  scrollToEnd?: boolean;
  /** The zoom the scene starts at, when the URL gives none. */
  scale?: number;
  /** What the screen can give to the inside of the window, in CSS px. */
  screen?: { width: number; height: number };
  /** The status line after the bands are open (a notice about what just happened). */
  after?(s: AppState): void;
  /** The notes of the recording going on. */
  live?: LiveNotesSnapshot;
}

const dbfs = (db: number) => 10 ** (db / 20);

function withKeyboard(s: AppState, name = "Keystation 49 MK3"): void {
  const kb = keyboard(name);
  s.keyboards = { ...s.keyboards, list: [kb], active: kb.id, lost: null, inputStopped: false };
  s.audio.midiInput = kb.portName;
}

function withOutput(s: AppState): void {
  s.outputs.chosen = HEADPHONES.id;
  s.outputs.chosenState = "ready";
  s.transport.canPlay = true;
  s.transport.canRecord = s.song.tracks.some((t) => t.armed);
  Object.assign(s.audio, {
    running: true,
    sampleRate: 48000,
    actualFrames: 256,
    latencyMs: 13.5,
    latencyMsAtRequested: 13.5,
    outputDevice: HEADPHONES.nodeName,
  });
}

/** The second one was plugged in last; the first one was played last. */
function twoKeyboards(s: AppState): void {
  ready(s);
  const second = keyboard("microKEY2-37", 2);
  s.keyboards.list.push(second);
}

function ready(s: AppState): void {
  withKeyboard(s);
  withOutput(s);
  s.status.message = NEW_SONG;
}

function bufferFixed(s: AppState): void {
  ready(s);
  Object.assign(s.audio, { forcedQuantum: 1024, actualFrames: 1024, latencyMs: 53, latencyMsAtRequested: 13.5 });
  s.status.message = { id: "bufferFixed", actual: 1024, requested: 256, latencyMs: 53, latencyMsAtRequested: 13.5 };
}

function settings(s: AppState): void {
  ready(s);
  s.outputs.list = PC_OUTPUTS.map((o) => ({ ...o }));
  s.inputs.list = PC_INPUTS.map((i) => ({ ...i }));
  s.workspace.settings = { draftOutput: HEADPHONES.id, advancedOpen: false };
  s.status.message = { id: "settingsOpen" };
}

function settingsAdvFixed(s: AppState): void {
  bufferFixed(s);
  s.outputs.list = PC_OUTPUTS.map((o) => ({ ...o }));
  s.workspace.settings = { draftOutput: HEADPHONES.id, advancedOpen: true };
}

const QUIET: FakeMeters = { velocity: 83, loadPermille: 90 };

/** The arrange of the mock F-Takes, 3 steps in, never saved. */
function changedSong(s: AppState): void {
  ready(s);
  s.song.undoCount = 3;
  s.song.file = "changed";
  s.song.tracks[0]!.clips = [fakeClip(1, "イントロ", 1, 5, 1, [2]), fakeClip(3, "Aメロ", 5, 9, 2, [5, 6, 7])];
  s.transport.position = 4 * BAR;
  s.status.message = { id: "summary", loopBars: null, nothingSelected: true, notes: null };
}

function songDialog(kind: SongDialogKind, title = "はじめての曲", previous = false) {
  return (s: AppState): void => {
    changedSong(s);
    s.workspace.songDialog = { id: 1, kind, title, previous };
  };
}

// ---- takes (scenes recording, takes) ----

/** Ticks in a bar of 4/4. */
const BAR = 1920;

interface FakeTake {
  id: number;
  length: number;
  /** The take a quantized take was made from. */
  origin?: number;
  /** `from`: the index of the note of the origin take it came from. */
  notes: { start: number; length: number; key: number; velocity: number; from?: number }[];
  /** In order of tick. `number` 0 for pitch bend. */
  controls?: { tick: number; kind: typeof CONTROL_CC | typeof CONTROL_PITCH_BEND; number: number; value: number }[];
}

/** Bytes of one control in the take binary (docs/architecture.md 3-4). */
const FAKE_CONTROL_LEN = 8;

/** A made-up phrase of `count` notes over `bars` bars; the same seed gives the same notes. */
function phrase(id: number, bars: number, count: number, seed: number): FakeTake {
  const length = bars * BAR;
  const step = Math.floor(length / count);
  const notes = Array.from({ length: count }, (_, i) => {
    const r = (seed * 7919 + i * 104729) % 97;
    return { start: i * step, length: Math.max(60, step - 40 - (r % 3) * 60), key: 55 + ((r * 5 + i * 3) % 24), velocity: 60 + (r % 50) };
  });
  return { id, length, notes };
}

/** Clip and take ids come from one counter, as in the song model. */
const FAKE_TAKES = new Map(
  [phrase(2, 4, 23, 1), phrase(4, 2, 12, 2), phrase(5, 4, 19, 3), phrase(6, 4, 23, 4), phrase(7, 4, 17, 5)].map((t) => [t.id, t]),
);

function fakeClip(id: number, name: string, fromBar: number, toBar: number, take: number, takeIds: number[]): ClipView {
  const takeId = takeIds[take - 1]!;
  return {
    id,
    name,
    start: (fromBar - 1) * BAR,
    end: (toBar - 1) * BAR,
    take,
    takes: takeIds.length,
    takeId,
    takeOffset: 0,
    noteCount: FAKE_TAKES.get(takeId)!.notes.length,
  };
}

/** Writes the take binary (docs/architecture.md 3-4). */
export function encodeTake(t: FakeTake): ArrayBuffer {
  const controls = t.controls ?? [];
  const buf = new ArrayBuffer(TAKE_HEADER_LEN + TAKE_NOTE_LEN * t.notes.length + FAKE_CONTROL_LEN * controls.length);
  const v = new DataView(buf);
  v.setUint8(0, TAKE_KIND);
  v.setUint8(1, TAKE_VERSION);
  v.setUint32(4, t.id, true);
  v.setUint32(8, t.length, true);
  v.setUint32(12, t.notes.length, true);
  v.setUint32(16, controls.length, true);
  v.setUint32(20, t.origin ?? 0, true);
  t.notes.forEach((n, i) => {
    const p = TAKE_HEADER_LEN + TAKE_NOTE_LEN * i;
    v.setUint32(p, n.start, true);
    v.setUint32(p + 4, n.length, true);
    v.setUint8(p + 8, n.key);
    v.setUint8(p + 9, n.velocity);
    v.setUint32(p + 12, n.from ?? NO_FROM, true);
  });
  controls.forEach((c, i) => {
    const p = TAKE_HEADER_LEN + TAKE_NOTE_LEN * t.notes.length + FAKE_CONTROL_LEN * i;
    v.setUint32(p, c.tick, true);
    v.setUint8(p + 4, c.kind);
    v.setUint8(p + 5, c.number);
    v.setInt16(p + 6, c.value, true);
  });
  return buf;
}

const NO_LIVE: LiveNotesSnapshot = { seq: 0, recording: null, notes: [] };

/** 11 月 15 日 22:41 in the PC's own time, as the mocks F-Recover1 and Opt3-Recover show it. */
const RECOVERY_WRITTEN = new Date(2026, 10, 15, 22, 41).getTime();

/** Recording from bar 5; the listener is at 7.3.240, with C4 E4 G4 held since 7.3.000. */
const RECORDING_LIVE: LiveNotesSnapshot = {
  seq: 14,
  recording: 1,
  notes: [
    ...Array.from({ length: 11 }, (_, i): LiveNote => {
      const r = (i * 37) % 11;
      return { lap: 0, index: i, key: 57 + ((r * 7) % 19), velocity: 70 + r * 3, start: 4 * BAR + i * 440, length: 200 + r * 20 };
    }),
    ...[60, 64, 67].map((key, i): LiveNote => ({ lap: 0, index: 11 + i, key, velocity: 87, start: 6 * BAR + 960, length: null })),
  ],
};

// ---- the piano roll (mocks A-PianoRoll, F-PR720, F-PR150, F-WidePR, F-PRPlay, F-PRRec) ----

const NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
/** "3.1.360" (bar . beat . tick, 4/4) to ticks from the song's start. */
const at = (text: string) => {
  const [bar, beat, tick] = text.split(".").map(Number) as [number, number, number];
  return (bar - 1) * BAR + (beat - 1) * 480 + tick;
};
/** "1.240" (beats . ticks) to ticks. */
const span = (text: string) => {
  const [beats, ticks] = text.split(".").map(Number) as [number, number];
  return beats * 480 + ticks;
};
/** "D#4" to the MIDI number (C4 = 60). */
const key = (text: string) => NOTE_NAMES.indexOf(text.slice(0, -1)) + 12 * (Number(text.slice(-1)) + 1);

/** The note list of the mocks (F-WidePR, left), as it is written there: position, key, length, Vel. */
const MOCK_NOTES: [string, string, string, number][] = [
  ["1.1.000", "C3", "4.000", 70],
  ["1.1.000", "G3", "4.000", 64],
  ["1.1.000", "E4", "1.000", 87],
  ["1.2.000", "G4", "1.000", 92],
  ["1.3.000", "C5", "1.240", 101],
  ["1.4.240", "G4", "0.240", 74],
  ["2.1.000", "F3", "4.000", 66],
  ["2.1.000", "A3", "4.000", 62],
  ["2.1.000", "A4", "1.000", 84],
  ["2.2.000", "G4", "0.240", 70],
  ["2.2.240", "F4", "0.240", 72],
  ["2.3.000", "E4", "2.000", 90],
  ["3.1.000", "G3", "4.000", 71],
  ["3.1.000", "B3", "4.000", 60],
  ["3.1.000", "D4", "0.360", 80],
  ["3.1.360", "E4", "0.360", 83],
  ["3.2.240", "F4", "0.240", 77],
  ["3.3.000", "G4", "2.000", 96],
  ["4.1.000", "C3", "4.000", 73],
  ["4.1.000", "E3", "4.000", 61],
  ["4.1.000", "G3", "4.000", 63],
  ["4.1.000", "C4", "3.000", 88],
  ["4.4.000", "E4", "0.360", 58],
];
/** D4 E4 F4 of bar 3: quantized, and selected. */
const QUANTIZED: IndexRange = { start: 14, end: 17 };
/** Where D4 E4 F4 were played, before they were quantized (read off the gray.7 frames of A-PianoRoll). */
const PLAYED_AT: Record<number, number> = { 14: at("3.1.036"), 15: at("3.1.312"), 16: at("3.2.276") };

const PLAYED_TAKE = 11;
const QUANTIZED_TAKE = 12;
FAKE_TAKES.set(PLAYED_TAKE, {
  id: PLAYED_TAKE,
  length: 4 * BAR,
  notes: MOCK_NOTES.map(([start, k, length, velocity], i) => ({
    start: PLAYED_AT[i] ?? at(start),
    length: span(length),
    key: key(k),
    velocity,
  })),
});
/**
 * The CC tags of A-PianoRoll: sustain pressed 5 times, 6 pitch bend points (shaped like R3-Lanes), no modulation;
 * and CC 7 for "そのほかの CC".
 */
const MOCK_CONTROLS: NonNullable<FakeTake["controls"]> = [
  ...[
    ["1.1.000", "1.4.240"],
    ["2.1.000", "2.4.240"],
    ["3.1.000", "3.2.240"],
    ["3.3.000", "3.4.240"],
    ["4.1.000", "4.4.240"],
  ].flatMap(([down, up]) => [
    { tick: at(down!), kind: CONTROL_CC, number: 64, value: 127 } as const,
    { tick: at(up!), kind: CONTROL_CC, number: 64, value: 0 } as const,
  ]),
  ...(
    [
      ["2.1.000", 0],
      ["2.1.240", 1536],
      ["2.2.000", 4096],
      ["2.3.000", 3072],
      ["2.4.000", 1024],
      ["2.4.240", 0],
    ] as const
  ).map(([tick, value]) => ({ tick: at(tick), kind: CONTROL_PITCH_BEND, number: 0, value }) as const),
  ...(
    [
      ["1.1.000", 100],
      ["3.1.000", 90],
      ["4.3.000", 110],
    ] as const
  ).map(([tick, value]) => ({ tick: at(tick), kind: CONTROL_CC, number: 7, value }) as const),
].sort((a, b) => a.tick - b.tick);

FAKE_TAKES.set(QUANTIZED_TAKE, {
  id: QUANTIZED_TAKE,
  length: 4 * BAR,
  origin: PLAYED_TAKE,
  controls: MOCK_CONTROLS,
  notes: MOCK_NOTES.map(([start, k, length, velocity], i) => ({
    start: at(start),
    length: span(length),
    key: key(k),
    velocity,
    from: i,
  })),
});
for (const t of [phrase(14, 4, 21, 6), phrase(15, 4, 19, 7)]) FAKE_TAKES.set(t.id, t);

/** The other tracks of F-WidePR (MIDI clips only: the fake cannot show the sung clips). */
const WIDE_TRACKS: { name: string; gainDb: number; clips: [string, number, number][] }[] = [
  { name: "ベース", gainDb: -7.5, clips: [["ベース A", 5, 9], ["ベース A", 9, 13], ["ベース サビ", 13, 17]] },
  { name: "ドラム", gainDb: -6.0, clips: [["フィル", 3, 5], ["8ビート", 5, 13], ["サビ ビート", 13, 17]] },
  { name: "パッド", gainDb: -12.0, clips: [["コード", 1, 5], ["コード サビ", 13, 17]] },
  { name: "ボーカル", gainDb: -3.2, clips: [] },
];

function pianoRollView(take: number, scaleLock: boolean): PianoRollView {
  return {
    take,
    selectedNotes: [{ ...QUANTIZED }],
    selectedControls: [],
    grid: { division: 16, triplet: false },
    scale: { root: 0, mode: "major", lock: scaleLock },
    quantize: { division: 16, triplet: false, strength: 75, swing: 0, align: "start" },
    clipboardNotes: 0,
  };
}

/** A-PianoRoll: the clip "イントロ" (bars 1–4, one take) in the piano roll, D4 E4 F4 selected, stopped at 3.1.000. */
function pianoRollSong(s: AppState): void {
  ready(s);
  s.song.undoCount = 14;
  s.song.file = "changed";
  s.song.tracks[0]!.clips = [fakeClip(10, "イントロ", 1, 5, 1, [QUANTIZED_TAKE])];
  s.song.noteCount = MOCK_NOTES.length;
  s.arrange = arrangeOf(10);
  s.pianoRoll = pianoRollView(QUANTIZED_TAKE, true);
  s.transport.position = at("3.1.000");
  s.status.message = {
    id: "summary",
    loopBars: null,
    nothingSelected: false,
    notes: { count: 3, keys: [key("D4"), key("E4"), key("F4")], quantized: true },
  };
}

function pianoRollWide(s: AppState): void {
  pianoRollSong(s);
  s.song.tracks[0]!.gainDb = -4.8;
  let id = 20;
  WIDE_TRACKS.forEach((t, i) => {
    const clips = t.clips.map(([name, from, to]) => {
      const take = phrase(++id, to - from, 4 * (to - from), id);
      FAKE_TAKES.set(take.id, take);
      return fakeClip(++id, name, from, to, 1, [take.id]);
    });
    s.song.tracks.push({ ...s.song.tracks[0]!, number: i + 2, name: t.name, color: i + 2, gainDb: t.gainDb, armed: false, clips });
  });
}

/** F-PRPlay and F-PRRec: the clip "Aメロ" (bars 1–4) with 3 takes, the second shown. */
function aMelody(s: AppState, takes: number[]): void {
  ready(s);
  s.song.undoCount = 14;
  s.song.file = "saved";
  s.song.tracks[0]!.clips = [fakeClip(13, "Aメロ", 1, 5, 2, takes)];
  s.song.noteCount = MOCK_NOTES.length;
  s.arrange = arrangeOf(13);
  s.pianoRoll = pianoRollView(QUANTIZED_TAKE, false);
  s.status.message = null;
}

/** F-PRRec: recording take 3 from bar 1; at 4.2.000, C3 E3 G3 held since 4.1.000 and C4 since 4.1.120. */
const PIANO_ROLL_LIVE: LiveNotesSnapshot = {
  seq: 22,
  recording: 1,
  notes: [
    ...MOCK_NOTES.slice(0, 18).map(([start, k, length, velocity], index): LiveNote => ({
      lap: 0,
      index,
      key: key(k),
      velocity,
      start: at(start),
      length: span(length),
    })),
    ...(
      [
        ["4.1.000", "C3"],
        ["4.1.000", "E3"],
        ["4.1.000", "G3"],
        ["4.1.120", "C4"],
      ] as const
    ).map(([start, k], i): LiveNote => ({ lap: 0, index: 18 + i, key: key(k), velocity: 88, start: at(start), length: null })),
  ],
};

const PIANO_ROLL_PLAYING: FakeMeters = {
  velocity: 83,
  loadPermille: 160,
  master: [dbfs(-7.1), dbfs(-7.6)],
  tracks: [[dbfs(-7.1), dbfs(-7.6)]],
  position: { tick: at("4.1.240"), ticksPerUs: 0, state: "playing", countInBeatsLeft: 0, lap: 0 },
};

/**
 * R3-Arrange: two tracks. The piano has "イントロ" (bars 1–4) and "Aメロ" with its first bar trimmed off (bars 6–8);
 * the bass has "ベース A" (bars 5–8) and a copy of it trimmed to 2.5 bars (bars 14–16.2), selected. The trimmed notes are not drawn.
 */
function arrangeTrimmed(s: AppState): void {
  ready(s);
  const bass = phrase(40, 4, 11, 6);
  FAKE_TAKES.set(bass.id, bass);
  s.song.undoCount = 4;
  s.song.tracks[0]!.clips = [
    fakeClip(1, "イントロ", 1, 5, 1, [2]),
    { ...fakeClip(3, "Aメロ", 5, 9, 1, [5]), start: 5 * BAR, takeOffset: -BAR },
  ];
  s.song.tracks.push({
    ...s.song.tracks[0]!,
    number: 2,
    name: "ベース",
    color: 2,
    gainDb: -7.5,
    armed: false,
    clips: [fakeClip(41, "ベース A", 5, 9, 1, [40]), { ...fakeClip(42, "ベース A", 14, 18, 1, [40]), end: 13 * BAR + BAR * 2.5 }],
  });
  s.arrange = arrangeOf(42);
  s.status.message = null;
}

export const SCENES: Record<string, SceneDef> = {
  connect1: { setup: (s) => (s.status.message = { id: "searchingKeyboard" }), meters: { loadPermille: 90 } },
  connect2: {
    setup(s) {
      withKeyboard(s);
      s.outputs.list = PC_OUTPUTS.map((o) => ({ ...o }));
      s.workspace.chooser = { highlighted: HEADPHONES.id };
      s.status.message = { id: "keyboardFound", keyboard: "Keystation 49 MK3" };
    },
    meters: { keys: [60, 64, 67], velocity: 87, sustain: 127, loadPermille: 90 },
  },
  connect3: {
    setup(s) {
      withKeyboard(s);
      withOutput(s);
      s.workspace.hint = "soundCameOut";
      s.status.message = { id: "outputChosen", output: HEADPHONES.name, track: PIANO };
    },
    meters: {
      keys: [60, 64, 67],
      velocity: 87,
      sustain: 127,
      modulation: 12,
      loadPermille: 60,
      master: [dbfs(-8.4), dbfs(-9.1)],
      tracks: [[dbfs(-8.4), dbfs(-9.1)]],
    },
  },
  empty: { setup: (s) => (ready(s), (s.workspace.hint = "emptyArrange")), meters: QUIET },
  bufferFixed: { setup: bufferFixed, meters: QUIET },
  settings: { setup: settings, meters: QUIET },
  settingsAdvFixed: { setup: settingsAdvFixed, meters: QUIET },
  settingsEnd: { setup: settings, meters: QUIET, scrollToEnd: true },
  settingsAdvFixedEnd: { setup: settingsAdvFixed, meters: QUIET, scrollToEnd: true },
  twoKeyboards: { setup: twoKeyboards, meters: QUIET },
  longName: { setup: (s) => (ready(s), withKeyboard(s, "Arturia KeyLab Essential 61 mk3 MIDI 1")), meters: QUIET },
  pianoRoll: { setup: pianoRollSong, bands: ["pianoRoll"], meters: QUIET },
  // The same at a window of 1280x720: open it in a window of that size.
  pianoRoll720: { setup: pianoRollSong, bands: ["pianoRoll"], meters: QUIET },
  pianoRoll150: { setup: pianoRollSong, bands: ["pianoRoll"], meters: QUIET, scale: 150 },
  // Open it in a window of 1920x1080.
  pianoRollWide: { setup: pianoRollWide, bands: ["pianoRoll"], meters: QUIET },
  // A-PianoRoll with the pitch bend point at 2.2.000 selected, for the CC lanes (R3-Lanes). The lanes are opened by clicking their tags.
  pianoRollCc: {
    setup(s) {
      pianoRollSong(s);
      const point = MOCK_CONTROLS.findIndex((c) => c.kind === CONTROL_PITCH_BEND && c.tick === at("2.2.000"));
      s.pianoRoll.selectedControls = toRanges([point]);
    },
    bands: ["pianoRoll"],
    meters: QUIET,
  },
  pianoRollPlay: {
    setup(s) {
      aMelody(s, [14, QUANTIZED_TAKE, 15]);
      s.transport.state = "playing";
    },
    bands: ["pianoRoll"],
    meters: PIANO_ROLL_PLAYING,
  },
  // Takes 1 and 2 are in the clip; take 3 is being recorded.
  pianoRollRec: {
    setup(s) {
      aMelody(s, [14, QUANTIZED_TAKE]);
      s.transport.state = "recording";
      s.transport.recording = { id: 1, track: 1, from: 0, take: 3, clip: 13, lap: 0 };
      s.status.message = { id: "recording", track: PIANO, take: 3 };
    },
    bands: ["pianoRoll"],
    meters: {
      ...PIANO_ROLL_PLAYING,
      keys: [key("C3"), key("E3"), key("G3"), key("C4")],
      velocity: 88,
      sustain: 127,
      position: { tick: at("4.2.000"), ticksPerUs: 0, state: "recording", countInBeatsLeft: 0, lap: 0 },
    },
    live: PIANO_ROLL_LIVE,
  },
  // The mock F-PREmpty: a new song with no clip, the piano roll open (spec 3: only the grid, and the hint at the top left).
  pianoRollEmpty: {
    setup: ready,
    bands: ["pianoRoll"],
    after: (s) => (s.status.message = { id: "summary", loopBars: null, nothingSelected: true, notes: null }),
    meters: QUIET,
  },
  mixer: { setup: ready, bands: ["mixer"], meters: QUIET },
  history: { setup: ready, bands: ["history"], meters: QUIET },
  shelf: { setup: ready, bands: ["shelf"], meters: QUIET },
  params: { setup: ready, bands: ["params"], meters: QUIET },
  // Not from a mock: 16 tracks, every meter showing a level, to check that the WebGL contexts do not grow with the tracks.
  tracks16: {
    setup(s) {
      ready(s);
      s.song.tracks = Array.from({ length: 16 }, (_, i) => ({ ...s.song.tracks[0]!, number: i + 1, color: (i % 5) + 1 }));
    },
    meters: {
      keys: [60, 64, 67],
      velocity: 87,
      loadPermille: 120,
      master: [dbfs(-6.2), dbfs(-7.0)],
      tracks: Array.from({ length: 16 }, (_, i): [number, number] => [dbfs(-3 - 2 * i), dbfs(-4 - 2 * i)]),
    },
  },
  // The mock F-ArrangeRec without the loop: recording to the piano from bar 5, over a clip with one take.
  recording: {
    setup(s) {
      ready(s);
      s.song.undoCount = 12;
      s.song.loopRange = { start: 4 * BAR, end: 8 * BAR };
      s.song.tracks[0]!.clips = [fakeClip(1, "イントロ", 1, 5, 1, [2]), fakeClip(3, "ピアノ", 5, 7, 1, [4])];
      s.transport.state = "recording";
      s.transport.position = 4 * BAR;
      s.transport.metronome = true;
      s.transport.recording = { id: 1, track: 1, from: 4 * BAR, take: 2, clip: 3, lap: 0 };
      s.status.message = { id: "recording", track: PIANO, take: 2 };
    },
    meters: {
      keys: [60, 64, 67],
      velocity: 87,
      sustain: 127,
      modulation: 12,
      loadPermille: 140,
      master: [dbfs(-6.2), dbfs(-7.0)],
      tracks: [[dbfs(-6.2), dbfs(-7.0)]],
      position: { tick: 6 * BAR + 960 + 240, ticksPerUs: 0, state: "recording", countInBeatsLeft: 0, lap: 0 },
    },
    live: RECORDING_LIVE,
  },
  // The mock F-Connect4: a new song, the third beat of a one-bar count-in.
  countIn: {
    setup(s) {
      ready(s);
      s.transport.state = "countIn";
      s.transport.metronome = true;
      s.transport.recording = { id: 1, track: 1, from: 0, take: 1, clip: null, lap: 0 };
      s.status.message = { id: "countingIn", beats: 4 };
    },
    meters: { ...QUIET, position: { tick: -720, ticksPerUs: 0, state: "countIn", countInBeatsLeft: 2, lap: 0 } },
  },
  // The mock F-Takes: the clip "Aメロ" has 3 takes, shows the second, and is selected; the piano roll shows it.
  takes: {
    setup(s) {
      ready(s);
      s.song.undoCount = 3;
      s.song.tracks[0]!.clips = [fakeClip(1, "イントロ", 1, 5, 1, [2]), fakeClip(3, "Aメロ", 5, 9, 2, [5, 6, 7])];
      s.song.noteCount = FAKE_TAKES.get(6)!.notes.length;
      s.arrange = arrangeOf(3);
      s.transport.position = 4 * BAR;
      s.status.message = null;
    },
    bands: ["pianoRoll"],
    meters: { ...QUIET, position: { tick: 4 * BAR, ticksPerUs: 0, state: "stopped", countInBeatsLeft: 0, lap: 0 } },
  },
  arrangeTrimmed: { setup: arrangeTrimmed, meters: QUIET },
  // The same with the piano roll open on the trimmed clip: the notes trimmed off are not shown there either.
  arrangeTrimmedRoll: { setup: arrangeTrimmed, bands: ["pianoRoll"], meters: QUIET },
  // The mock "拡大率をえらぶ所": a 1200x700 screen, 150 % does not fit, 125 % is on.
  zoom: { setup: ready, scale: 125, screen: { width: 1200, height: 700 }, meters: QUIET },
  // The mixer was open, then the piano roll was opened in a window too low for both.
  bandFolded: {
    setup: ready,
    bands: ["mixer", "pianoRoll"],
    after: (s) => (s.status.message = { id: "bandFoldedToOpen", opened: "pianoRoll", folded: "mixer" }),
    meters: QUIET,
  },
  // The arrange of the mock F-Takes with no band open: the clip "Aメロ" (take 2 of 3) is selected.
  takesArrange: {
    setup(s) {
      ready(s);
      s.song.tracks[0]!.clips = [fakeClip(1, "イントロ", 1, 5, 1, [2]), fakeClip(3, "Aメロ", 5, 9, 2, [5, 6, 7])];
      s.arrange = arrangeOf(3);
      s.transport.position = 4 * BAR;
      s.status.message = null;
    },
    meters: QUIET,
  },
  // Not from a mock: the song dialogs (spec 2 "ダイアログ", spec 6).
  unsaved: { setup: songDialog("unsaved"), meters: QUIET },
  cannotSave: { setup: songDialog("cannotSave"), meters: QUIET },
  broken: { setup: songDialog("broken", "はじめての曲.meloet", true), meters: QUIET },
  brokenNoPrevious: { setup: songDialog("broken", "はじめての曲.meloet"), meters: QUIET },
  // The mock F-Recover1: a new song never saved, brought back from the recovery file written as the PC ended.
  // Bringing it back is the first step ("1 / 1").
  recover1: {
    setup(s) {
      ready(s);
      s.song.undoCount = 1;
      s.song.file = "changed";
      s.song.tracks[0]!.clips = [fakeClip(1, "ピアノ", 1, 5, 1, [2]), fakeClip(3, "ピアノ", 5, 7, 1, [4])];
      s.workspace.recovery = { kind: "recovered", writtenMs: RECOVERY_WRITTEN, cause: "pcEnding", song: null, takes: 2 };
      s.status.message = { id: "recovered" };
    },
    meters: QUIET,
  },
  // The mock F-Recover2: "戻さない" was pressed; the song is as last saved, and the history band is open.
  recover2: {
    setup(s) {
      changedSong(s);
      s.song.undoCount = 2;
      s.song.file = "saved";
      s.status.message = { id: "recoveryDiscarded" };
    },
    bands: ["history"],
    meters: QUIET,
  },
  // The mock F-Recover3: the recovery file did not read; the song last saved is open.
  recover3: {
    setup(s) {
      changedSong(s);
      s.song.undoCount = 0;
      s.song.file = "saved";
      s.workspace.recovery = { kind: "unreadable", song: "はじめての曲" };
      s.status.message = { id: "recoveryUnreadable", savedSong: true };
    },
    meters: QUIET,
  },
  // The song name of the mock Main: saved, 14 steps ("保存済み ・ 取り消し 14").
  saved: {
    setup(s) {
      changedSong(s);
      s.song.undoCount = 14;
      s.song.file = "saved";
    },
    meters: QUIET,
  },
  // 2 of 14 steps taken back: the history band label "12 / 14" and the status line "2 手ぶん取り消してあります…".
  undone: {
    setup(s) {
      changedSong(s);
      s.song.undoCount = 12;
      s.song.redoCount = 2;
      s.status.message = { id: "undoPending", count: 2 };
    },
    meters: QUIET,
  },
  // The loop of the mock Main: bars 5–9, on, stopped at bar 5.
  loop: {
    setup(s) {
      ready(s);
      s.song.tracks[0]!.clips = [fakeClip(1, "イントロ", 1, 5, 1, [2]), fakeClip(3, "Aメロ", 5, 9, 2, [5, 6, 7])];
      s.song.loopRange = { start: 4 * BAR, end: 8 * BAR };
      s.song.loopOn = true;
      s.transport.position = 4 * BAR;
      s.status.message = null;
    },
    meters: QUIET,
  },
  // The mock Main: the loop scene with the other tracks of F-WidePR, "4 パッド" muted ("… ・ 4 パッド はミュート").
  muted: {
    setup(s) {
      ready(s);
      s.song.tracks[0]!.clips = [fakeClip(1, "イントロ", 1, 5, 1, [2]), fakeClip(3, "Aメロ", 5, 9, 2, [5, 6, 7])];
      let id = 200;
      WIDE_TRACKS.forEach((t, i) => {
        const clips = t.clips.map(([name, from, to]) => {
          const take = phrase(++id, to - from, 4 * (to - from), id);
          FAKE_TAKES.set(take.id, take);
          return fakeClip(++id, name, from, to, 1, [take.id]);
        });
        s.song.tracks.push({ ...s.song.tracks[0]!, number: i + 2, name: t.name, color: i + 2, gainDb: t.gainDb, armed: false, clips });
      });
      s.song.tracks[3]!.muted = true;
      s.song.undoCount = 14;
      s.song.file = "saved";
      s.song.loopRange = { start: 4 * BAR, end: 8 * BAR };
      s.song.loopOn = true;
      s.transport.position = 4 * BAR;
      s.status.message = { id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: true, notes: null };
    },
    meters: QUIET,
  },
  // Not from a mock (B-70, B-71): three tracks; the bass is selected (it is where the keys play) and muted, the drums
  // soloed. "Aメロ" of the piano and "ベース A" are selected together, "ベース A" clicked last. One clip is ready to paste.
  arrangeSelect: { setup: arrangeSelect, meters: QUIET },
  // Not from a mock (B-66): ベロシティ固定 is on at 100.
  velocityFixed: {
    setup(s) {
      ready(s);
      s.keyboards.velocityFixed = { on: true, value: 100 };
    },
    meters: { ...QUIET, velocity: 100 },
  },
  // Not from a mock (B-68, B-67): the piano roll after a velocity out of range was typed, and after "そろえる".
  valueClamped: {
    setup: pianoRollSong,
    bands: ["pianoRoll"],
    after: (s) => (s.status.message = { id: "valueClamped", field: "velocity", value: "127", range: null }),
    meters: QUIET,
  },
  meterClamped: {
    setup: ready,
    after: (s) => (s.status.message = { id: "valueClamped", field: "meterDen", value: "4", range: null }),
    meters: QUIET,
  },
  velocityMatched: {
    setup: pianoRollSong,
    bands: ["pianoRoll"],
    after: (s) => (s.status.message = { id: "velocityMatched", count: 3, value: 100 }),
    meters: QUIET,
  },
  lengthMatched: {
    setup: pianoRollSong,
    bands: ["pianoRoll"],
    after: (s) => (s.status.message = { id: "lengthMatched", count: 3, value: 240 }),
    meters: QUIET,
  },
  // Not from a mock (B-68): a part of a position put in its range.
  positionClamped: {
    setup: ready,
    after: (s) => (s.status.message = { id: "valueClamped", field: "beat", value: "3. 4. 000", range: "1〜4" }),
    meters: QUIET,
  },
  barClamped: {
    setup: ready,
    after: (s) => (s.status.message = { id: "valueClamped", field: "bar", value: "1. 1. 000", range: null }),
    meters: QUIET,
  },
  // Not from a mock (B-71): clips that would overlap other clips were not put there.
  clipsNotMoved: {
    setup: arrangeSelect,
    after: (s) => (s.status.message = { id: "clipsNotMoved" }),
    meters: QUIET,
  },
  clipsNotPasted: {
    setup: arrangeSelect,
    after: (s) => (s.status.message = { id: "clipsNotPasted" }),
    meters: QUIET,
  },
  clipsNotDuplicated: {
    setup: arrangeSelect,
    after: (s) => (s.status.message = { id: "clipsNotDuplicated" }),
    meters: QUIET,
  },
};

function arrangeSelect(s: AppState): void {
  ready(s);
  s.song.undoCount = 5;
  s.song.file = "changed";
  const track = s.song.tracks[0]!;
  track.clips = [fakeClip(1, "イントロ", 1, 5, 1, [2]), fakeClip(3, "Aメロ", 5, 9, 2, [5, 6, 7])];
  const bass = phrase(40, 4, 11, 6);
  FAKE_TAKES.set(bass.id, bass);
  s.song.tracks.push(
    { ...track, number: 2, name: "ベース", color: 2, gainDb: -7.5, armed: false, muted: true, clips: [fakeClip(41, "ベース A", 5, 9, 1, [40])] },
    { ...track, number: 3, name: "ドラム", color: 3, gainDb: -6, armed: false, soloed: true, clips: [fakeClip(43, "8ビート", 1, 9, 1, [7])] },
  );
  s.song.playTarget = 2;
  s.arrange = { selectedClips: [3, 41], selectedClip: 41, pianoRollClip: 41, clipboardClips: 1 };
  s.transport.position = 4 * BAR;
  s.status.message = { id: "summary", loopBars: null, nothingSelected: false, notes: null };
}

// Not from a mock: the scene recording, with the listener moving at 96.00 (the clip being recorded grows).
SCENES.recordingMoving = {
  ...SCENES.recording!,
  meters: {
    ...SCENES.recording!.meters,
    position: { tick: 6 * BAR, ticksPerUs: 768e-6, state: "recording", countInBeatsLeft: 0, lap: 0 },
  },
};

// Not from a mock: one clip of 10 000 notes over 64 bars, the first 200 selected, for measuring the piano roll.
const TEN_THOUSAND = 90;
FAKE_TAKES.set(TEN_THOUSAND, phrase(TEN_THOUSAND, 64, 10_000, 9));
SCENES.pianoRoll10k = {
  setup(s) {
    pianoRollSong(s);
    s.song.tracks[0]!.clips = [fakeClip(91, "イントロ", 1, 65, 1, [TEN_THOUSAND])];
    s.song.noteCount = 10_000;
    s.arrange = arrangeOf(91);
    s.pianoRoll = { ...pianoRollView(TEN_THOUSAND, false), selectedNotes: [{ start: 0, end: 200 }] };
    s.status.message = null;
  },
  bands: ["pianoRoll"],
  meters: QUIET,
};
// The same while playing from bar 1 (the play line moves, the keys light, the roll pages).
SCENES.pianoRoll10kPlay = {
  ...SCENES.pianoRoll10k,
  setup(s) {
    SCENES.pianoRoll10k!.setup(s);
    s.transport.state = "playing";
    s.transport.position = 0;
  },
  meters: { ...QUIET, position: { tick: 0, ticksPerUs: 768e-6, state: "playing", countInBeatsLeft: 0, lap: 0 } },
};

export function fakeScene(name: string, scale?: number, viewport = { width: 1280, height: 800 }): Scene | null {
  const def = SCENES[name];
  if (!def) return null;
  const state = fakeState();
  const unfit = def.screen ? unfitScales(def.screen) : [];
  const asked = scale ?? def.scale ?? 100;
  const s = SCALES.includes(asked) && !unfit.includes(asked) ? asked : 100;
  const bands = new FakeBands((viewport.height * 100) / s);
  for (const b of def.bands ?? []) bands.toggle(b);
  def.setup(state);
  state.workspace.scale = s;
  state.workspace.viewportWidth = viewport.width;
  state.workspace.viewportHeight = viewport.height;
  state.workspace.unfitScales = unfit;
  state.workspace.layout = bands.layout();
  def.after?.(state);
  return { state, bands, meters: def.meters, scrollToEnd: def.scrollToEnd, live: def.live ?? NO_LIVE };
}

function sceneFromUrl(): Scene | null {
  if (typeof location === "undefined") return null;
  const q = new URLSearchParams(location.search);
  const name = q.get("scene");
  if (!name) return null;
  const vp = typeof window === "undefined" ? undefined : { width: window.innerWidth, height: window.innerHeight };
  const scale = q.get("scale");
  return fakeScene(name, scale === null ? undefined : Number(scale), vp);
}

// ---- telemetry ----

const STATE_BYTES = { stopped: 0, countIn: 1, playing: 2, recording: 3 } as const;

/** Writes the telemetry binary (docs/architecture.md 3-3). `position` is what is heard at `sentUs`. */
export function encodeTelemetry(
  m: FakeMeters,
  seq: number,
  sentUs: number,
  trackCount: number,
  position: TransportReading | null = m.position ?? null,
): ArrayBuffer {
  const H = TELEMETRY_HEADER_LEN;
  const buf = new ArrayBuffer(H + 8 * trackCount);
  const v = new DataView(buf);
  v.setUint8(0, TELEMETRY_KIND);
  v.setUint8(1, TELEMETRY_VERSION);
  v.setUint16(2, trackCount, true);
  v.setUint32(4, seq, true);
  v.setBigUint64(8, BigInt(Math.round(sentUs)) * 1000n, true);
  const words = [0, 0, 0, 0];
  for (const n of m.keys ?? []) words[n >>> 5]! |= 1 << (n & 31);
  words.forEach((w, i) => v.setUint32(16 + 4 * i, w >>> 0, true));
  v.setUint8(32, m.velocity ?? 0);
  v.setUint8(33, m.sustain ?? 0);
  v.setUint8(34, m.modulation ?? 0);
  v.setUint8(35, (m.master && (m.master[0] > 0 || m.master[1] > 0) ? 1 : 0) | (position ? 4 : 0));
  v.setInt16(36, m.pitchBend ?? 0, true);
  v.setUint16(38, m.loadPermille ?? 0, true);
  v.setFloat32(40, m.master?.[0] ?? 0, true);
  v.setFloat32(44, m.master?.[1] ?? 0, true);
  if (position) {
    v.setFloat64(48, position.tick, true);
    v.setFloat64(56, position.ticksPerUs, true);
    v.setUint8(64, STATE_BYTES[position.state]);
    v.setUint16(66, position.countInBeatsLeft, true);
    v.setUint32(68, position.lap, true);
  }
  for (let i = 0; i < trackCount; i++) {
    const [l, r] = m.tracks?.[i] ?? [0, 0];
    v.setFloat32(H + 8 * i, l, true);
    v.setFloat32(H + 4 + 8 * i, r, true);
  }
  return buf;
}

// ---- commands ----

/** The status messages that tell what just happened. The next thing the user does takes them away. */
const NOTICES: StatusMessage["id"][] = [
  "bandFoldedWindowLow",
  "bandFoldedToOpen",
  "bandFoldedThird",
  "bandFoldedScale",
  "valueRejected",
  "quantized",
  "scaleDoesNotFit",
  "scaleWindowGrown",
  "scaleLoweredForScreen",
  "bufferFixed",
  "valueClamped",
  "velocityMatched",
  "lengthMatched",
  "clipsNotMoved",
  "clipsNotPasted",
  "clipsNotDuplicated",
];
const isNotice = (m: StatusMessage | null) => m !== null && NOTICES.includes(m.id);

/** Sorted indexes to ranges `[start, end)`, the touching ones joined. */
export function toRanges(indexes: Iterable<number>): IndexRange[] {
  const out: IndexRange[] = [];
  for (const i of [...new Set(indexes)].sort((a, b) => a - b)) {
    const last = out.at(-1);
    if (last && last.end === i) last.end = i + 1;
    else out.push({ start: i, end: i + 1 });
  }
  return out;
}

const fromRanges = (ranges: IndexRange[]) => ranges.flatMap((r) => Array.from({ length: r.end - r.start }, (_, k) => r.start + k));

/** The selection commands (of notes and of control points), the grid, the scale and the quantize settings. The fake does not change notes or points. */
function reducePianoRoll(pr: PianoRollView, command: Command): void {
  const count = pr.take === null ? 0 : (FAKE_TAKES.get(pr.take)?.notes.length ?? 0);
  const points = pr.take === null ? 0 : (FAKE_TAKES.get(pr.take)?.controls?.length ?? 0);
  switch (command.type) {
    case "selectNotes": {
      if (command.take !== pr.take) break;
      const picked = command.notes.filter((i) => i < count);
      const had = new Set(fromRanges(pr.selectedNotes));
      if (command.mode === "replace") had.clear();
      for (const i of picked) {
        if (command.mode === "toggle" && had.has(i)) had.delete(i);
        else had.add(i);
      }
      pr.selectedNotes = toRanges(had);
      break;
    }
    case "selectControls": {
      if (command.take !== pr.take) break;
      const had = new Set(command.mode === "replace" ? [] : fromRanges(pr.selectedControls));
      for (const i of command.points.filter((p) => p < points)) {
        if (command.mode === "toggle" && had.has(i)) had.delete(i);
        else had.add(i);
      }
      pr.selectedControls = toRanges(had);
      break;
    }
    case "selectAllNotes":
      pr.selectedNotes = count > 0 ? [{ start: 0, end: count }] : [];
      break;
    case "clearNoteSelection":
      pr.selectedNotes = [];
      break;
    case "setGrid":
      pr.grid = { division: command.division, triplet: command.triplet };
      break;
    case "setMusicalScale":
      pr.scale = { root: command.root, mode: command.mode, lock: command.lock };
      break;
    case "setQuantize": {
      const { division, triplet, strength, swing, align } = command;
      pr.quantize = { division, triplet, strength, swing, align };
      break;
    }
    default:
      break;
  }
}

/** The status line's summary for the selection of the piano roll (NOTE_SELECTION_KEYS in meloet-protocol is 4). */
function noteSelection(pr: PianoRollView): Extract<StatusMessage, { id: "summary" }>["notes"] {
  const take = pr.take === null ? undefined : FAKE_TAKES.get(pr.take);
  const picked = fromRanges(pr.selectedNotes);
  if (!take || picked.length === 0) return null;
  return { count: picked.length, keys: picked.slice(0, 4).map((i) => take.notes[i]!.key), quantized: (take.origin ?? 0) !== 0 };
}

/** What `copyClips` and `cutClips` keep: each clip with its row and start counted from the first row and the earliest start. */
export interface FakeClipboard {
  clips: { row: number; at: number; clip: ClipView }[];
}

const CLIP_COMMANDS: Command["type"][] = [
  "selectClip",
  "selectClips",
  "selectAllClips",
  "selectTrack",
  "toggleMute",
  "toggleSolo",
  "moveClips",
  "duplicateClips",
  "trimClip",
  "deleteTake",
  "deleteSelected",
  "copyClips",
  "cutClips",
  "pasteClips",
];

/**
 * The clip and track commands of docs/architecture.md 17 and protocol 8, enough for pictures and tests: no history,
 * no undo count. Returns the new song and arrange, or null when nothing changes (as Rust does on an overlap).
 */
export function reduceClips(
  state: AppState,
  command: Command,
  clipboard: FakeClipboard = { clips: [] },
): { song: SongView; arrange: ArrangeView; notice?: StatusMessage } | null {
  if (!CLIP_COMMANDS.includes(command.type)) return null;
  const song = structuredClone(state.song);
  const arrange = structuredClone(state.arrange);
  const rowOf = (id: number) => song.tracks.findIndex((t) => t.clips.some((c) => c.id === id));
  const clipOf = (id: number) => song.tracks.flatMap((t) => t.clips).find((c) => c.id === id);
  const inOrder = (ids: Iterable<number>) => {
    const set = new Set(ids);
    return song.tracks.flatMap((t) => t.clips.filter((c) => set.has(c.id)).map((c) => c.id));
  };
  const select = (ids: number[], last: number | null) => {
    arrange.selectedClips = inOrder(ids);
    arrange.selectedClip = last !== null && arrange.selectedClips.includes(last) ? last : arrange.selectedClips.length === 1 ? arrange.selectedClips[0]! : null;
    if (arrange.selectedClip !== null) arrange.pianoRollClip = arrange.selectedClip;
  };
  const done = () => ({ song, arrange });
  const track = (n: number) => song.tracks.find((t) => t.number === n);
  switch (command.type) {
    case "selectClip":
      if (command.clip === null) select([], null);
      else if (clipOf(command.clip)) {
        select([command.clip], command.clip);
        song.playTarget = song.tracks[rowOf(command.clip)]!.number;
      }
      return done();
    case "selectClips": {
      const picked = command.clips.filter((id) => clipOf(id));
      const had = new Set(command.mode === "replace" ? [] : arrange.selectedClips);
      for (const id of picked) {
        if (command.mode === "toggle" && had.has(id)) had.delete(id);
        else had.add(id);
      }
      // Shift + click (one clip added) makes it the clip clicked last; a box does not.
      const clicked = command.mode !== "replace" && picked.length === 1 ? picked[0]! : null;
      select([...had], clicked ?? arrange.selectedClip);
      if (clicked !== null && had.has(clicked)) song.playTarget = song.tracks[rowOf(clicked)]!.number;
      return done();
    }
    case "selectAllClips":
      select(
        song.tracks.flatMap((t) => t.clips.map((c) => c.id)),
        arrange.selectedClip,
      );
      return done();
    case "selectTrack":
      if (track(command.track)) song.playTarget = command.track;
      return done();
    case "toggleMute":
    case "toggleSolo": {
      const t = track(command.track);
      if (!t) return null;
      if (command.type === "toggleMute") t.muted = !t.muted;
      else t.soloed = !t.soloed;
      return done();
    }
    case "copyClips":
    case "cutClips": {
      const ids = inOrder(command.clips);
      if (ids.length === 0) return null;
      const rows = ids.map(rowOf);
      const top = Math.min(...rows);
      const first = Math.min(...ids.map((id) => clipOf(id)!.start));
      clipboard.clips = ids.map((id, i) => ({ row: rows[i]! - top, at: clipOf(id)!.start - first, clip: { ...clipOf(id)! } }));
      arrange.clipboardClips = ids.length;
      if (command.type === "copyClips") return done();
      if (busy(state)) return null;
      for (const t of song.tracks) t.clips = t.clips.filter((c) => !ids.includes(c.id));
      select([], null);
      return done();
    }
    default:
      break;
  }
  if (busy(state)) return null;
  const nextId = () => Math.max(0, ...FAKE_TAKES.keys(), ...song.tracks.flatMap((t) => t.clips.map((c) => c.id))) + 1;
  const byStart = (a: ClipView, b: ClipView) => a.start - b.start;
  /**
   * Puts `items` (copies when `fresh`) where they say. When one is off the tracks nothing changes; when one would
   * overlap another clip nothing changes either, and the status line says `overlap` (as core does).
   */
  const place = (
    items: { row: number; start: number; clip: ClipView }[],
    fresh: boolean,
    taken: number[],
    overlap: StatusMessage,
  ): ReturnType<typeof reduceClips> => {
    if (!items.every(({ row, start }) => row >= 0 && row < song.tracks.length && start >= 0)) return null;
    for (const t of song.tracks) t.clips = t.clips.filter((c) => !taken.includes(c.id));
    const overlaps = items.some(({ row, start, clip }) =>
      song.tracks[row]!.clips.some((c) => c.start < start + clip.end - clip.start && start < c.end),
    );
    if (overlaps) return { song: state.song, arrange: state.arrange, notice: overlap };
    const placed: number[] = [];
    let last: number | null = null;
    for (const { row, start, clip } of items) {
      const id = fresh ? nextId() : clip.id;
      song.tracks[row]!.clips.push({ ...clip, id, start, end: start + clip.end - clip.start });
      song.tracks[row]!.clips.sort(byStart);
      placed.push(id);
      if (clip.id === arrange.selectedClip || last === null) last = id;
    }
    select(placed, last);
    return done();
  };
  switch (command.type) {
    case "trimClip": {
      const from = rowOf(command.clip);
      if (from < 0) return null;
      const clips = song.tracks[from]!.clips;
      const at = clips.findIndex((c) => c.id === command.clip);
      const clip = clips[at]!;
      const before = clips.filter((c) => c.end <= clip.start).reduce((m, c) => Math.max(m, c.end), 0);
      const after = clips.filter((c) => c.start >= clip.end).reduce((m, c) => Math.min(m, c.start), Infinity);
      const start = command.edge === "start" ? Math.min(Math.max(clip.start + command.ticks, before), clip.end - 1) : clip.start;
      const end = command.edge === "end" ? Math.max(Math.min(clip.end + command.ticks, after), clip.start + 1) : clip.end;
      if (start === clip.start && end === clip.end) return null;
      clips[at] = { ...clip, start, end, takeOffset: clip.takeOffset - (start - clip.start) };
      return done();
    }
    case "deleteTake": {
      // The fake keeps no other take ids: the number moves, the id stays.
      const clip = clipOf(command.clip);
      if (!clip || clip.takes < 2) return null;
      clip.takes -= 1;
      clip.take = Math.max(1, clip.take - 1);
      return done();
    }
    case "deleteSelected": {
      const ids = arrange.selectedClips;
      if (ids.length === 0) return null;
      for (const t of song.tracks) t.clips = t.clips.filter((c) => !ids.includes(c.id));
      select([], null);
      return done();
    }
    case "moveClips":
    case "duplicateClips": {
      const ids = inOrder(command.clips);
      if (ids.length === 0) return null;
      const clips = ids.map((id) => clipOf(id)!);
      const copy = command.type === "duplicateClips" || command.copy;
      const rows = command.type === "duplicateClips" ? 0 : command.tracks;
      const earliest = Math.min(...clips.map((c) => c.start));
      // The group stops at the song's start.
      const ticks = command.type === "duplicateClips" ? Math.max(...clips.map((c) => c.end)) - earliest : Math.max(command.ticks, -earliest);
      if (!copy && ticks === 0 && rows === 0) return null;
      const items = clips.map((clip) => ({ row: rowOf(clip.id) + rows, start: clip.start + ticks, clip }));
      return place(items, copy, copy ? [] : ids, { id: copy ? "clipsNotDuplicated" : "clipsNotMoved" });
    }
    case "pasteClips": {
      if (clipboard.clips.length === 0) return null;
      const perBar = (1920 / song.meterDen) * song.meterNum;
      const to = command.to ?? { track: song.playTarget ?? 0, tick: Math.floor(state.transport.position / perBar) * perBar };
      const top = song.tracks.findIndex((t) => t.number === to.track);
      if (top < 0) return null;
      const items = clipboard.clips.map(({ row, at, clip }) => ({ row: top + row, start: to.tick + at, clip }));
      return place(items, true, [], { id: "clipsNotPasted" });
    }
    default:
      return null;
  }
}

const busy = (state: AppState) => state.transport.state === "recording" || state.transport.state === "countIn";

/** `rest` is the status line when no notice is up. */
function reduce(state: AppState, bands: FakeBands, rest: StatusMessage | null, command: Command, clipboard: FakeClipboard): Change[] {
  const ws = structuredClone(state.workspace);
  const outputs = structuredClone(state.outputs);
  const keyboards = structuredClone(state.keyboards);
  const transport = structuredClone(state.transport);
  const pianoRoll = structuredClone(state.pianoRoll);
  reducePianoRoll(pianoRoll, command);
  const choosable = outputs.list.filter((o) => o.unusable === null).map((o) => o.id);
  let notice = isNotice(state.status.message) && command.type === "setViewport" ? state.status.message : null;
  const fitWindow = (scale?: number) => {
    const folded = bands.setHeight((ws.viewportHeight * 100) / ws.scale);
    if (folded) notice = scale ? { id: "bandFoldedScale", scale, band: folded } : { id: "bandFoldedWindowLow", band: folded };
  };
  switch (command.type) {
    case "setViewport":
      ws.viewportWidth = command.width;
      ws.viewportHeight = command.height;
      fitWindow();
      break;
    case "setScale": {
      const percent = command.percent;
      if (!SCALES.includes(percent) || percent === ws.scale) break;
      if (ws.unfitScales.includes(percent)) {
        notice = { id: "scaleDoesNotFit", requested: percent, kept: ws.scale };
        break;
      }
      ws.scale = percent;
      const min = minWindow(percent);
      if (ws.viewportWidth < min.width || ws.viewportHeight < min.height) {
        ws.viewportWidth = Math.max(ws.viewportWidth, min.width);
        ws.viewportHeight = Math.max(ws.viewportHeight, min.height);
        notice = { id: "scaleWindowGrown", scale: percent };
      }
      fitWindow(percent);
      break;
    }
    case "toggleBand": {
      const fold = bands.toggle(command.band);
      if (fold?.why === "third") notice = { id: "bandFoldedThird", opened: command.band, folded: fold.band };
      if (fold?.why === "tooLow") notice = { id: "bandFoldedToOpen", opened: command.band, folded: fold.band };
      break;
    }
    case "clampValue":
      notice = { id: "valueClamped", field: command.field, value: command.value, range: command.range };
      break;
    // The fake does not change notes; it only says what Rust says.
    case "matchVelocity":
      if (command.take === pianoRoll.take && command.notes.length > 0) notice = { id: "velocityMatched", count: command.notes.length, value: command.value };
      break;
    case "matchLength":
      if (command.take === pianoRoll.take && command.notes.length > 0) notice = { id: "lengthMatched", count: command.notes.length, value: Math.max(1, command.value) };
      break;
    case "setVelocityFixed":
      keyboards.velocityFixed = { on: command.on, value: Math.min(127, Math.max(1, command.value)) };
      break;
    case "locate":
      if (transport.state === "stopped") transport.position = command.tick;
      break;
    case "rejectValue":
      notice = { id: "valueRejected", field: command.field, kept: command.kept, range: command.range };
      break;
    // The fake does not move notes; it only says what Rust says.
    case "quantize": {
      const { division, triplet, strength } = pianoRoll.quantize;
      const all = command.take === pianoRoll.take ? (FAKE_TAKES.get(command.take)?.notes.length ?? 0) : 0;
      notice = { id: "quantized", division, triplet, strength, count: command.notes.length || all };
      break;
    }
    case "goArrange":
      bands.goArrange();
      break;
    case "back":
      if (ws.songDialog) ws.songDialog = null;
      else if (ws.settings) ws.settings = null;
      else bands.back();
      break;
    // The fake keeps no file: every answer just puts the dialog away.
    case "answerSongDialog":
      if (ws.songDialog?.id === command.dialog) ws.songDialog = null;
      break;
    case "openSettings":
      if (ws.settings) ws.settings.advancedOpen ||= command.advanced;
      else ws.settings = { draftOutput: outputs.chosen, advancedOpen: command.advanced };
      break;
    case "settingsSelectOutput":
      if (ws.settings && choosable.includes(command.output)) ws.settings.draftOutput = command.output;
      break;
    case "settingsToggleAdvanced":
      if (ws.settings) ws.settings.advancedOpen = !ws.settings.advancedOpen;
      break;
    case "settingsApply":
      if (ws.settings?.draftOutput) {
        outputs.chosen = ws.settings.draftOutput;
        outputs.chosenState = "ready";
      }
      ws.settings = null;
      break;
    case "settingsCancel":
      ws.settings = null;
      break;
    case "chooserMove":
      if (ws.chooser) {
        const at = choosable.indexOf(ws.chooser.highlighted ?? "");
        if (at >= 0) ws.chooser.highlighted = choosable[Math.max(0, Math.min(choosable.length - 1, at + command.delta))]!;
      }
      break;
    case "chooserSelect":
      if (ws.chooser && choosable.includes(command.output)) ws.chooser.highlighted = command.output;
      break;
    case "chooserConfirm":
      if (ws.chooser?.highlighted) {
        outputs.chosen = ws.chooser.highlighted;
        outputs.chosenState = "ready";
        ws.chooser = null;
      }
      break;
    case "dismissHint":
      if (ws.recovery?.kind === "unreadable") ws.recovery = null;
      else if (!ws.recovery) ws.hint = null;
      break;
    case "discardRecovered":
      if (ws.recovery?.kind === "recovered") ws.recovery = null;
      break;
    default:
      break;
  }
  ws.layout = bands.layout();
  const changes: Change[] = [];
  const clips = reduceClips(state, command, clipboard);
  if (clips) {
    if (JSON.stringify(clips.song) !== JSON.stringify(state.song)) changes.push({ section: "song", value: clips.song });
    if (JSON.stringify(clips.arrange) !== JSON.stringify(state.arrange)) changes.push({ section: "arrange", value: clips.arrange });
  }
  if (JSON.stringify(ws) !== JSON.stringify(state.workspace)) changes.push({ section: "workspace", value: ws });
  if (JSON.stringify(outputs) !== JSON.stringify(state.outputs)) changes.push({ section: "outputs", value: outputs });
  if (JSON.stringify(keyboards) !== JSON.stringify(state.keyboards)) changes.push({ section: "keyboards", value: keyboards });
  if (JSON.stringify(transport) !== JSON.stringify(state.transport)) changes.push({ section: "transport", value: transport });
  if (JSON.stringify(pianoRoll) !== JSON.stringify(state.pianoRoll)) changes.push({ section: "pianoRoll", value: pianoRoll });
  // Scenes set the rest of the status line by hand; only the notices and the selected notes move here.
  let message = notice ?? clips?.notice ?? (isNotice(state.status.message) ? rest : state.status.message);
  if (message?.id === "summary") {
    const notes = noteSelection(pianoRoll);
    message = { ...message, notes, nothingSelected: notes === null && (clips?.arrange ?? state.arrange).selectedClips.length === 0 };
  }
  if (JSON.stringify(message) !== JSON.stringify(state.status.message)) changes.push({ section: "status", value: { message } });
  return changes;
}

/** With no `initial`, the scene comes from the URL (`?scene=`), or else the plain fakeState(). */
export function createFakeTransport(initial?: AppState): Transport {
  const scene = initial ? null : sceneFromUrl();
  let state = initial ?? scene?.state ?? fakeState();
  const bands = scene?.bands ?? new FakeBands((state.workspace.viewportHeight * 100) / state.workspace.scale);
  if (!scene) for (const l of state.workspace.layout.lower) bands.bottom.push(l.band);
  if (!scene) bands.side = [...state.workspace.layout.rightOpen];
  const rest = isNotice(state.status.message) ? null : state.status.message;
  const clipboard: FakeClipboard = { clips: [] };
  let rev = 1;
  let listener: ((message: ServerMessage) => void) | null = null;
  const start = performance.now();
  const nowUs = () => Math.round((performance.now() - start) * 1000);

  return {
    async hello(protocolVersion) {
      return { protocolVersion: PROTOCOL_VERSION, ok: protocolVersion === PROTOCOL_VERSION, monoUs: nowUs() };
    },
    async subscribe(onMessage, onTelemetry) {
      listener = onMessage;
      // Only scenes send meters, so tests that use the plain fake leave no timer behind.
      if (scene) {
        let seq = 0;
        setInterval(() => {
          const sent = nowUs();
          const p = scene.meters.position;
          const position = p ? { ...p, tick: p.tick + sent * p.ticksPerUs } : null;
          onTelemetry(encodeTelemetry(scene.meters, ++seq, sent, state.song.tracks.length, position));
          if (scene.scrollToEnd) for (const el of document.querySelectorAll("[data-gl-scroll]")) el.scrollTop = el.scrollHeight;
        }, 1000 / 60);
      }
    },
    async snapshot() {
      return { rev, state };
    },
    async send(command) {
      const changes = reduce(state, bands, rest, command, clipboard);
      if (changes.length > 0) {
        rev += 1;
        state = { ...state };
        for (const c of changes) (state as Record<string, unknown>)[c.section] = c.value;
        listener?.({ kind: "patch", rev, changes });
      }
      return { rev };
    },
    async clock() {
      return nowUs();
    },
    async takeNotes(id) {
      const take = FAKE_TAKES.get(id);
      if (!take) throw new Error(`fake: no take ${id}`);
      return encodeTake(take);
    },
    async liveNotes() {
      return scene?.live ?? NO_LIVE;
    },
  };
}

/** The fake has nothing to measure, except with `?measure=1` in a browser: then the frame profile is kept on `window.__meloetMeasure`. */
export async function fakeMeasureEnabled(): Promise<boolean> {
  return typeof location !== "undefined" && new URLSearchParams(location.search).get("measure") === "1";
}

export async function fakeMeasureReport(name: string, json: string): Promise<void> {
  if (typeof window === "undefined") return;
  const w = window as unknown as { __meloetMeasure?: Record<string, string> };
  (w.__meloetMeasure ??= {})[name] = json;
}

/** With `?measure=1&uiScript=pr-scroll:10` in a browser, that script. */
export async function fakeUiScript(): Promise<string | null> {
  if (typeof location === "undefined" || !(await fakeMeasureEnabled())) return null;
  return new URLSearchParams(location.search).get("uiScript");
}

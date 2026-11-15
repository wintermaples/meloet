// A Transport with no Rust behind it: for tests, and for looking at the screen in a plain
// browser (`npm run dev`, `vite preview`). `?scene=<name>&scale=<100|125|150>` picks a fixed
// scene (see SCENES). It answers the few commands below; the band heights follow the same
// numbers as crates/meloet-core/src/layout.rs, but meloet-core stays the rule book.

import type { Transport } from "./session";
import { TAKE_KIND, TAKE_VERSION } from "./take";
import { TELEMETRY_HEADER_LEN, TELEMETRY_KIND, TELEMETRY_VERSION, type TransportReading } from "./telemetry";
import {
  PROTOCOL_VERSION,
  type AppState,
  type Band,
  type Change,
  type ClipView,
  type Command,
  type EndpointView,
  type LayoutView,
  type LiveNote,
  type LiveNotesSnapshot,
  type ServerMessage,
  type SongDialogKind,
  type StatusMessage,
  type TrackRef,
} from "./types";

// ---- the band rules (a copy of the numbers in meloet-core/src/layout.rs) ----

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
    const fixed = TRANSPORT + this.meterHeight() + BAND_ROW + STATUS;
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

export function fakeState(): AppState {
  const bands = new FakeBands(800);
  return {
    song: {
      name: "はじめての曲",
      tempoBpm: 96,
      meterNum: 4,
      meterDen: 4,
      tempoChanges: 0,
      undoCount: 0,
      redoCount: 0,
      noteCount: 0,
      tracks: [{ number: 1, name: "ピアノ", color: 1, instrument: "placeholder", gainDb: 0, armed: true, clips: [] }],
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
    arrange: { selectedClip: null, pianoRollClip: null },
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
    },
    keyboards: { list: [], active: null, lost: null, inputStopped: false },
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
  s.keyboards = { list: [kb], active: kb.id, lost: null, inputStopped: false };
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
  s.status.message = { id: "summary", loopBars: null, nothingSelected: true };
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
  notes: { start: number; length: number; key: number; velocity: number }[];
}

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

/** Writes the take binary (docs/architecture.md 3-4). Notes only: the fake takes have no controls. */
export function encodeTake(t: FakeTake): ArrayBuffer {
  const buf = new ArrayBuffer(20 + 12 * t.notes.length);
  const v = new DataView(buf);
  v.setUint8(0, TAKE_KIND);
  v.setUint8(1, TAKE_VERSION);
  v.setUint32(4, t.id, true);
  v.setUint32(8, t.length, true);
  v.setUint32(12, t.notes.length, true);
  v.setUint32(16, 0, true);
  t.notes.forEach((n, i) => {
    const p = 20 + 12 * i;
    v.setUint32(p, n.start, true);
    v.setUint32(p + 4, n.length, true);
    v.setUint8(p + 8, n.key);
    v.setUint8(p + 9, n.velocity);
  });
  return buf;
}

const NO_LIVE: LiveNotesSnapshot = { seq: 0, recording: null, notes: [] };

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
  pianoRoll: { setup: ready, bands: ["pianoRoll"], meters: QUIET },
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
      s.arrange = { selectedClip: 3, pianoRollClip: 3 };
      s.transport.position = 4 * BAR;
      s.status.message = null;
    },
    bands: ["pianoRoll"],
    meters: { ...QUIET, position: { tick: 4 * BAR, ticksPerUs: 0, state: "stopped", countInBeatsLeft: 0, lap: 0 } },
  },
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
      s.arrange = { selectedClip: 3, pianoRollClip: 3 };
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
};

// Not from a mock: the scene recording, with the listener moving at 96.00 (the clip being recorded grows).
SCENES.recordingMoving = {
  ...SCENES.recording!,
  meters: {
    ...SCENES.recording!.meters,
    position: { tick: 6 * BAR, ticksPerUs: 768e-6, state: "recording", countInBeatsLeft: 0, lap: 0 },
  },
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
  "scaleDoesNotFit",
  "scaleWindowGrown",
  "scaleLoweredForScreen",
  "bufferFixed",
];
const isNotice = (m: StatusMessage | null) => m !== null && NOTICES.includes(m.id);

/** `rest` is the status line when no notice is up. */
function reduce(state: AppState, bands: FakeBands, rest: StatusMessage | null, command: Command): Change[] {
  const ws = structuredClone(state.workspace);
  const outputs = structuredClone(state.outputs);
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
    case "rejectValue":
      notice = { id: "valueRejected", field: command.field, kept: command.kept };
      break;
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
      ws.hint = null;
      break;
    default:
      break;
  }
  ws.layout = bands.layout();
  const changes: Change[] = [];
  if (JSON.stringify(ws) !== JSON.stringify(state.workspace)) changes.push({ section: "workspace", value: ws });
  if (JSON.stringify(outputs) !== JSON.stringify(state.outputs)) changes.push({ section: "outputs", value: outputs });
  // Scenes set the rest of the status line by hand; only the notices move here.
  const message = notice ?? (isNotice(state.status.message) ? rest : state.status.message);
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
      const changes = reduce(state, bands, rest, command);
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

/** The fake has nothing to measure. */
export async function fakeMeasureEnabled(): Promise<boolean> {
  return false;
}

export async function fakeMeasureReport(_name: string, _json: string): Promise<void> {}

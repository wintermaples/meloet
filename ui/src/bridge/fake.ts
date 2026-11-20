// A Transport with no Rust behind it: for tests, and for looking at the screen in a plain
// browser (`npm run dev`, `vite preview`). `?scene=<name>&scale=<100|125|150>` picks a fixed
// scene (see SCENES). It answers the few commands below; the band heights follow the same
// numbers as crates/meloet-core/src/layout.rs, but meloet-core stays the rule book.

import type { Transport } from "./session";
import { CONTROL_CC, CONTROL_PITCH_BEND, NO_FROM, TAKE_HEADER_LEN, TAKE_KIND, TAKE_NOTE_LEN, TAKE_VERSION } from "./take";
import { TELEMETRY_HEADER_LEN, TELEMETRY_KIND, TELEMETRY_VERSION, type TransportReading } from "./telemetry";
import { FLUID, emptyLibrary, fullLibrary, lv2Key, row as pluginRow, vst3Key } from "./fakeLibrary";
import { FAKE_TABLES, FakeParamTables, SURGE_AUTOMATED, fakeParamCount } from "./fakeParams";
import {
  EFFECT_PARAMS,
  INSTRUMENT_GROUPS,
  PROTOCOL_VERSION,
  type AppState as WireState,
  type Band,
  type Change,
  type ClipView,
  type Command,
  type EffectKind,
  type EffectRef,
  type EndpointView,
  type IndexRange,
  type InsertRef,
  type InsertView,
  type InstrumentKind,
  type KeyCount,
  type LayoutView,
  type LibraryView,
  type LiveNote,
  type LiveNotesSnapshot,
  type MissingView,
  type PianoRollView,
  type PluginPlaceView,
  type PluginSlotRef,
  type PluginEffectCategory,
  type PluginState,
  type PluginView,
  type PluginWindowView,
  type PluginsView,
  type SelectedPlugin,
  type ServerMessage,
  type ShelfView as WireShelf,
  type SongDialogKind,
  type SongView as WireSong,
  type SoundName,
  type SoundRef,
  type SoundView,
  type ArrangeView,
  type StatusMessage,
  type EffectName,
  type NamedInsert,
  type ParamValues,
  type StoppedPluginView,
  type StripRef,
  type TrackRef,
  type TrackView as WireTrack,
} from "./types";

// The scenes and the commands below keep, next to the contract's fields, fields of their own that
// name only what comes with meloet (the instrument of a track, the effects in its slots, the row
// the shelf auditions…). `settleSong` and `settleShelf` bring the contract's fields in line with
// them. The screen reads only the contract's fields; these ride along on the state the fake holds.
type Inserts = [EffectKind | null, EffectKind | null, EffectKind | null, EffectKind | null];
interface OwnStrip {
  /** The effects that come with meloet, top first; `null` for an empty slot and for a plugin's. */
  inserts: Inserts;
}
export type FakeTrack = WireTrack & OwnStrip & { /** "sine" for an SF2 preset and for a plugin. */ instrument: InstrumentKind };
export type FakeSong = Omit<WireSong, "tracks" | "returns" | "master"> & {
  tracks: FakeTrack[];
  returns: [WireSong["returns"][0] & OwnStrip, WireSong["returns"][1] & OwnStrip];
  master: WireSong["master"] & OwnStrip;
};
export type FakeShelf = WireShelf & {
  favorites: InstrumentKind[];
  selected: InstrumentKind | null;
  current: InstrumentKind | null;
  loading: InstrumentKind | null;
  sample: { instrument: InstrumentKind; startedUs: number; lengthUs: number } | null;
  selectedEffect: EffectKind | null;
};
export type FakeState = Omit<WireState, "song" | "shelf"> & { song: FakeSong; shelf: FakeShelf };
type AppState = FakeState;
type SongView = FakeSong;
type ShelfView = FakeShelf;
type TrackView = FakeTrack;

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

const FAKE_HOME = "/home/yachiyo";
const place = (shownPath: string, added = false): PluginPlaceView => ({ path: shownPath.replace(/^~/, FAKE_HOME), shownPath, added });

/** The usual places of Linux for LV2 and VST3 (protocol 16): no scan running, nothing stopped. */
export function fakePlugins(): PluginsView {
  return {
    scan: { state: "idle" },
    stopped: [],
    places: ["~/.lv2", "/usr/lib/lv2", "/usr/local/lib/lv2", "~/.vst3", "/usr/lib/vst3", "/usr/local/lib/vst3"].map((p) => place(p)),
    stoppedOpen: false,
    placesOpen: false,
  };
}

// The rows of the mock F5-ScanStopped. Its third row is a CLAP plugin (~/.clap/TapeWobble.clap); CLAP is not read in round 5
// (docs/architecture.md 20-9), so that row is an LV2 bundle here. Tape Wobble is a whole bundle that could not be read (id null);
// the others are one plugin each of their bundles.
const stoppedPlugin = (
  name: string,
  format: StoppedPluginView["format"],
  reason: StoppedPluginView["reason"],
  shownPath: string,
  id: string | null,
): StoppedPluginView => ({
  name,
  format,
  reason,
  path: shownPath.replace(/^~/, FAKE_HOME),
  id,
  shownPath,
  retrying: false,
});
const STOPPED_PLUGINS: StoppedPluginView[] = [
  stoppedPlugin("Old Chorus", "vst3", "crashed", "~/.vst3/OldChorus.vst3", "4F4C4443484F52555330303030303031"),
  stoppedPlugin("Granular Fog", "lv2", "timedOut", "/usr/lib/lv2/granfog.lv2", "urn:granfog:fog"),
  stoppedPlugin("Tape Wobble", "lv2", "broken", "~/.lv2/TapeWobble.lv2", null),
  stoppedPlugin("Brick EQ 1.0", "vst3", "crashed", "/usr/lib/vst3/BrickEQ.vst3", "4252494B455131303030303030303031"),
];

/** One clip selected (and shown in the piano roll), or none; nothing to paste. */
export function arrangeOf(clip: number | null): ArrangeView {
  return { selectedClips: clip === null ? [] : [clip], selectedClip: clip, pianoRollClip: clip, clipboardClips: 0 };
}

/** Every instrument, in the order of the shelf: the fake has them all, so that the mocks can be drawn. */
const ALL_INSTRUMENTS: InstrumentKind[] = INSTRUMENT_GROUPS.flatMap((g) => [...g.instruments]);

/** The shelf as it is while closed, and when opened with "＋ トラックを追加" before a row is picked. */
function closedShelf(): ShelfView {
  return {
    mode: { kind: "newTrack" },
    available: [...ALL_INSTRUMENTS],
    favorites: ["grandPiano", "studioKit"],
    selected: null,
    current: null,
    loading: null,
    sample: null,
    selectedEffect: null,
    selectedSound: null,
    currentSound: null,
    loadingSound: null,
    soundSample: null,
    chosenEffect: null,
    currentEffect: null,
    unreadable: null,
    favoriteSounds: [builtinSound("grandPiano"), builtinSound("studioKit")],
    favoriteEffects: [builtinEffect("reverb"), builtinEffect("delay")],
    missing: [],
  };
}

type Slots = TrackView["slots"];
const noInserts = (): Inserts => [null, null, null, null];
const firstInsert = (effect: EffectKind): Inserts => [effect, null, null, null];

function builtinSound(instrument: InstrumentKind): Extract<SoundRef, { kind: "builtin" }> {
  return { kind: "builtin", instrument };
}
function builtinEffect(effect: EffectKind): EffectRef {
  return { kind: "builtin", effect };
}

/** The slots of a strip that holds only effects that come with meloet, none passed by. */
export function builtinSlots(inserts: Inserts): Slots {
  return inserts.map((effect): InsertView | null => (effect === null ? null : { effect: { kind: "builtin", effect }, bypassed: false })) as Slots;
}

/** The counts of `NoteSelection.names`: each key once, lowest first, five at most. */
export function keyCounts(keys: number[]): KeyCount[] {
  const counts = new Map<number, number>();
  for (const k of keys) counts.set(k, (counts.get(k) ?? 0) + 1);
  return [...counts].sort((a, b) => a[0] - b[0]).slice(0, 5).map(([key, count]) => ({ key, count }));
}

// The three functions below bring the contract's fields (`sound`, `slots`, `selectedSound`…) in line
// with the fake's own (`instrument`, `inserts`, the shelf's `selected`…), and leave alone what names
// an SF2 preset or a plugin.

function settleSlots(inserts: Inserts, slots: Slots): Slots {
  return inserts.map((effect, i): InsertView | null => {
    const had = slots[i] ?? null;
    if (effect === null) return had?.effect.kind === "plugin" ? had : null;
    return had?.effect.kind === "builtin" && had.effect.effect === effect ? had : { effect: { kind: "builtin", effect }, bypassed: false };
  }) as Slots;
}

function settleSong(song: SongView): SongView {
  const strip = <T extends { inserts: Inserts; slots: Slots }>(s: T): T => ({ ...s, slots: settleSlots(s.inserts, s.slots) });
  return {
    ...song,
    tracks: song.tracks.map((t) => {
      const own = t.sound.kind === "builtin" || t.instrument !== "sine";
      return strip({
        ...t,
        sound: own ? builtinSound(t.instrument) : t.sound,
        standIn: own && t.standIn !== "packMissing" ? null : t.standIn,
        drumKeys: own ? null : t.drumKeys,
      });
    }),
    returns: [strip(song.returns[0]), strip(song.returns[1])],
    master: strip(song.master),
  };
}

function settleShelf(shelf: ShelfView): ShelfView {
  const sound = (own: InstrumentKind | null, any: SoundRef | null): SoundRef | null =>
    own !== null ? builtinSound(own) : any?.kind === "builtin" ? null : any;
  return {
    ...shelf,
    selectedSound: sound(shelf.selected, shelf.selectedSound),
    currentSound: sound(shelf.current, shelf.currentSound),
    loadingSound: sound(shelf.loading, shelf.loadingSound),
    soundSample: shelf.sample
      ? { sound: builtinSound(shelf.sample.instrument), playing: { startedUs: shelf.sample.startedUs, lengthUs: shelf.sample.lengthUs } }
      : shelf.soundSample?.sound.kind === "builtin"
        ? null
        : shelf.soundSample,
    chosenEffect: shelf.selectedEffect !== null ? builtinEffect(shelf.selectedEffect) : shelf.chosenEffect?.kind === "builtin" ? null : shelf.chosenEffect,
    favoriteSounds: [...shelf.favorites.map(builtinSound), ...shelf.favoriteSounds.filter((s) => s.kind !== "builtin")],
  };
}

/** The values of an effect that is new, in the order of its table. */
export const effectDefaults = (effect: EffectKind): number[] => EFFECT_PARAMS[effect].map((p) => p.default);

/** The same in a slot of `strip`: a reverb or a delay on a track or on the master starts with its mix at 30 % (`Insert::new_on`). */
export const insertDefaults = (effect: EffectKind, strip: StripRef): number[] =>
  EFFECT_PARAMS[effect].map((p) => (p.id === "mix" && (effect === "reverb" || effect === "delay") && strip.kind !== "return" ? 30 : p.default));

/** What a new track sends to return A and B (spec 3, Y9). */
const newTrackSends = (instrument: InstrumentKind): TrackView["sends"] => {
  const group = INSTRUMENT_GROUPS.find((g) => g.instruments.includes(instrument))!.group;
  return [group === "bass" ? null : group === "drums" ? -24 : -15, null];
};

/** The returns and the master of a new song: a reverb on A, a delay on B, a limiter on the master (spec 3, Y9). */
function newSongMixer(): Pick<SongView, "returns" | "master"> {
  return {
    returns: [
      { gainDb: 0, pan: 0, muted: false, inserts: firstInsert("reverb"), slots: builtinSlots(firstInsert("reverb")) },
      { gainDb: 0, pan: 0, muted: false, inserts: firstInsert("delay"), slots: builtinSlots(firstInsert("delay")) },
    ],
    master: { gainDb: 0, pan: 0, inserts: firstInsert("limiter"), slots: builtinSlots(firstInsert("limiter")) },
  };
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
      tracks: [
        {
          id: 1,
          number: 1,
          name: "ピアノ",
          color: 1,
          instrument: "grandPiano",
          sound: builtinSound("grandPiano"),
          standIn: null,
          drumKeys: null,
          gainDb: 0,
          pan: 0,
          sends: newTrackSends("grandPiano"),
          sendDefaults: newTrackSends("grandPiano"),
          inserts: noInserts(),
          slots: builtinSlots(noInserts()),
          muted: false,
          soloed: false,
          clips: [],
        },
      ],
      ...newSongMixer(),
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
      fileDrop: null,
    },
    shelf: closedShelf(),
    library: emptyLibrary(),
    params: { selected: null },
    plugins: fakePlugins(),
    windows: { list: [] },
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
  /** Return A, then return B. */
  returns?: [[number, number], [number, number]];
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
  s.transport.canRecord = s.song.playTarget !== null;
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
const NO_NOTES_TAKE = 16;
FAKE_TAKES.set(NO_NOTES_TAKE, { id: NO_NOTES_TAKE, length: 4 * BAR, notes: [] });

/** The quantized take of A-PianoRoll with G4 written at 5.1.000, after the clip's end (B-82): the clip grew to bar 5. */
const GROWN_TAKE = 18;
FAKE_TAKES.set(GROWN_TAKE, {
  ...FAKE_TAKES.get(QUANTIZED_TAKE)!,
  id: GROWN_TAKE,
  length: 4 * BAR + BAR / 4,
  notes: [...FAKE_TAKES.get(QUANTIZED_TAKE)!.notes, { start: 4 * BAR, length: BAR / 4, key: key("G4"), velocity: 100 }],
});

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
    notes: { count: 3, keys: [key("D4"), key("E4"), key("F4")], quantized: true, names: keyCounts([key("D4"), key("E4"), key("F4")]) },
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
    s.song.tracks.push({ ...s.song.tracks[0]!, id: i + 2, number: i + 2, name: t.name, color: i + 2, gainDb: t.gainDb, clips });
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
    id: 2,
    number: 2,
    name: "ベース",
    color: 2,
    gainDb: -7.5,
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
  // Not from a mock: A-PianoRoll after every note was deleted (the clip stays, its take has no note).
  pianoRollNoNotes: {
    setup(s) {
      pianoRollSong(s);
      s.song.tracks[0]!.clips = [fakeClip(10, "イントロ", 1, 5, 1, [NO_NOTES_TAKE])];
      s.song.noteCount = 0;
      s.pianoRoll = { ...s.pianoRoll, take: NO_NOTES_TAKE, selectedNotes: [], selectedControls: [] };
      s.status.message = { id: "summary", loopBars: null, nothingSelected: true, notes: null };
    },
    bands: ["pianoRoll"],
    meters: QUIET,
  },
  // Not from a mock (B-82): A-PianoRoll after G4 was written at 5.1.000; the clip grew to bars 1–5, the note is selected.
  pianoRollGrown: {
    setup(s) {
      pianoRollSong(s);
      s.song.undoCount = 15;
      s.song.tracks[0]!.clips = [fakeClip(10, "イントロ", 1, 6, 1, [GROWN_TAKE])];
      s.song.noteCount = MOCK_NOTES.length + 1;
      const last = MOCK_NOTES.length;
      s.pianoRoll = { ...s.pianoRoll, take: GROWN_TAKE, selectedNotes: [{ start: last, end: last + 1 }] };
      s.transport.position = 4 * BAR;
      s.status.message = { id: "summary", loopBars: null, nothingSelected: false, notes: { count: 1, keys: [key("G4")], quantized: false, names: keyCounts([key("G4")]) } };
    },
    bands: ["pianoRoll"],
    meters: QUIET,
  },
  // Not from a mock (B-82): the write tool after the clip, where it cannot grow over the next clip.
  notesNotWritten: {
    setup: pianoRollSong,
    bands: ["pianoRoll"],
    after: (s) => (s.status.message = { id: "notesNotWritten" }),
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
      s.song.tracks = Array.from({ length: 16 }, (_, i) => ({ ...s.song.tracks[0]!, id: i + 1, number: i + 1, color: (i % 5) + 1 }));
    },
    meters: {
      keys: [60, 64, 67],
      velocity: 87,
      loadPermille: 120,
      master: [dbfs(-6.2), dbfs(-7.0)],
      tracks: Array.from({ length: 16 }, (_, i): [number, number] => [dbfs(-3 - 2 * i), dbfs(-4 - 2 * i)]),
    },
  },
  // Not from a mock: the same 16 tracks with the mixer open. Over 10 tracks the mixer's meters are shown as a left and a
  // right part in turn (docs/architecture.md, chapter 7).
  mixer16: {
    setup(s) {
      ready(s);
      s.song.tracks = Array.from({ length: 16 }, (_, i) => ({ ...s.song.tracks[0]!, id: i + 1, number: i + 1, color: (i % 5) + 1 }));
    },
    bands: ["mixer"],
    meters: {
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
  // The mock R9-Scan: the first start, the shelf open, 37 of 112 bundles looked at ("スキャン中 37 / 112").
  scanning: {
    setup(s) {
      ready(s);
      s.plugins.scan = { state: "running", done: 37, total: 112 };
    },
    bands: ["shelf"],
    meters: QUIET,
  },
  // R9-Scan's status line when that scan is over: 667 plugins in the shelf, 4 bundles stopped ("止まった 4 個を見る").
  scanFinished: {
    setup(s) {
      ready(s);
      s.plugins.scan = { state: "finished", added: 667, stopped: 4 };
      s.plugins.stopped = structuredClone(STOPPED_PLUGINS);
    },
    bands: ["shelf"],
    after: (s) => (s.status.message = { id: "scanFinished", added: 667, stopped: 4 }),
    meters: QUIET,
  },
  // R9-Scan's status line from the second start on: "新しいプラグインが 3 個見つかりました。…"
  pluginsFound: {
    setup(s) {
      ready(s);
      s.status.message = { id: "pluginsFound", count: 3 };
    },
    meters: QUIET,
  },
  // R9-Scan: 音の設定 ▸ くわしい設定 ▸ プラグインの場所, with the two places of the mock added by hand. The mock also
  // lists ~/.clap and /usr/lib/clap; CLAP is not read in round 5, so they are not here.
  pluginPlaces: {
    setup(s) {
      settings(s);
      s.workspace.settings = { draftOutput: HEADPHONES.id, advancedOpen: true };
      s.plugins.places.push(place("~/音楽/プラグイン", true), place("/opt/surge-xt/lib/vst3", true));
      s.plugins.placesOpen = true;
    },
    meters: QUIET,
    scrollToEnd: true,
  },
  // The mock F5-ScanStopped: the dialog 「スキャンで止まったプラグイン」 over the saved song.
  scanStopped: {
    setup(s) {
      changedSong(s);
      s.song.undoCount = 14;
      s.song.file = "saved";
      s.plugins.stopped = structuredClone(STOPPED_PLUGINS);
      s.plugins.stoppedOpen = true;
      s.status.message = { id: "scanStoppedOpen" };
    },
    meters: QUIET,
  },
  // The same with "もう一度ためす" pressed on the second row ("ためしています…").
  scanStoppedRetrying: {
    setup(s) {
      ready(s);
      s.plugins.stopped = structuredClone(STOPPED_PLUGINS);
      s.plugins.stopped[1]!.retrying = true;
      s.plugins.stoppedOpen = true;
      s.status.message = { id: "scanStoppedOpen" };
    },
    meters: QUIET,
  },
  // Spec 6 「もう一度ためして入った」: the row is gone and the status line says so.
  scanRetryAdded: {
    setup(s) {
      ready(s);
      s.plugins.stopped = structuredClone(STOPPED_PLUGINS).filter((p) => p.name !== "Granular Fog");
      s.plugins.stoppedOpen = true;
      s.status.message = { id: "scanRetryAdded", name: "Granular Fog" };
    },
    meters: QUIET,
  },
  // Spec 6 「もう一度ためしても止まった」.
  scanRetryStopped: {
    setup(s) {
      ready(s);
      s.plugins.stopped = structuredClone(STOPPED_PLUGINS);
      s.plugins.stoppedOpen = true;
      s.status.message = { id: "scanRetryStopped", name: "Granular Fog" };
    },
    meters: QUIET,
  },
  // Spec 6 「スキャンが終わった」 with nothing stopped: only the first half of the sentence (design-notes A-16).
  scanFinishedNone: {
    setup(s) {
      ready(s);
      s.plugins.scan = { state: "finished", added: 667, stopped: 0 };
    },
    bands: ["shelf"],
    after: (s) => (s.status.message = { id: "scanFinished", added: 667, stopped: 0 }),
    meters: QUIET,
  },
  // 「プラグインの場所」 folded (▸ and the count).
  pluginPlacesFolded: {
    setup(s) {
      settings(s);
      s.workspace.settings = { draftOutput: HEADPHONES.id, advancedOpen: true };
      s.plugins.places.push(place("~/音楽/プラグイン", true));
      s.plugins.placesOpen = false;
    },
    meters: QUIET,
    scrollToEnd: true,
  },
  // More stopped bundles than fit in a low window: only the rows scroll (spec 2 「ダイアログ」).
  scanStoppedMany: {
    setup(s) {
      ready(s);
      s.plugins.stopped = Array.from({ length: 5 }, () => structuredClone(STOPPED_PLUGINS)).flatMap((list, n) =>
        list.map((p) => ({ ...p, path: `${p.path}.${n}`, shownPath: `${p.shownPath}.${n}` })),
      );
      s.plugins.stoppedOpen = true;
      s.status.message = { id: "scanStoppedOpen" };
    },
    meters: QUIET,
  },
  // Spec 6 「プラグインが止まった」 in the status line: an instrument plugin of track 1.
  pluginStopped: {
    setup(s) {
      ready(s);
      s.status.message = { id: "pluginStopped", name: "Surge XT", slot: { kind: "instrument", track: s.song.tracks[0]!.id } };
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
        s.song.tracks.push({ ...s.song.tracks[0]!, id: i + 2, number: i + 2, name: t.name, color: i + 2, gainDb: t.gainDb, clips });
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
    { ...track, id: 2, number: 2, name: "ベース", color: 2, gainDb: -7.5, muted: true, clips: [fakeClip(41, "ベース A", 5, 9, 1, [40])] },
    { ...track, id: 3, number: 3, name: "ドラム", color: 3, gainDb: -6, soloed: true, clips: [fakeClip(43, "8ビート", 1, 9, 1, [7])] },
  );
  s.song.playTarget = 2;
  s.arrange = { selectedClips: [3, 41], selectedClip: 41, pianoRollClip: 41, clipboardClips: 1 };
  s.transport.position = 4 * BAR;
  s.status.message = { id: "summary", loopBars: null, nothingSelected: false, notes: null };
}

// Round 4. The song of the mock R7-Arrange6: six tracks, each with its instrument; "2 ベース" selected. The ids are
// not the numbers (11 to 16), as after tracks were added and moved: a command must carry the id.
const BAND_TRACKS: { name: string; instrument: InstrumentKind; gainDb: number; clips: [string, number, number][] }[] = [
  { name: "ピアノ", instrument: "grandPiano", gainDb: -4.8, clips: [["イントロ", 1, 5], ["サビ", 13, 17]] },
  { name: "ベース", instrument: "fingerBass", gainDb: -7.5, clips: [["ベース A", 5, 9], ["ベース A", 9, 13], ["ベース サビ", 13, 17]] },
  { name: "ドラム", instrument: "studioKit", gainDb: -6.0, clips: [["フィル", 3, 5], ["8ビート", 5, 13], ["サビ ビート", 13, 17]] },
  { name: "パッド", instrument: "strings", gainDb: -12.0, clips: [["コード", 1, 5], ["コード サビ", 13, 17]] },
  { name: "シンセ", instrument: "synthLead", gainDb: -9.0, clips: [["リード", 9, 13], ["リード サビ", 13, 17]] },
  { name: "ベース 2", instrument: "synthBass", gainDb: -10.5, clips: [["サビ うら", 13, 17]] },
];
const BAND_TRACK_ID = 10;

function bandSong(s: AppState, tracks = BAND_TRACKS): void {
  ready(s);
  let id = 300;
  s.song.tracks = tracks.map((t, i) => ({
    id: BAND_TRACK_ID + i + 1,
    number: i + 1,
    name: t.name,
    color: (i % 5) + 1,
    instrument: t.instrument,
    sound: builtinSound(t.instrument),
    standIn: null,
    drumKeys: null,
    gainDb: t.gainDb,
    pan: 0,
    sends: newTrackSends(t.instrument),
    sendDefaults: newTrackSends(t.instrument),
    inserts: noInserts(),
    slots: builtinSlots(noInserts()),
    muted: false,
    soloed: false,
    clips: t.clips.map(([name, from, to]) => {
      const take = phrase(++id, to - from, 4 * (to - from), id);
      FAKE_TAKES.set(take.id, take);
      return fakeClip(++id, name, from, to, 1, [take.id]);
    }),
  }));
  s.song.playTarget = BAND_TRACK_ID + 1;
  s.song.undoCount = 14;
  s.song.file = "saved";
  s.song.loopRange = { start: 4 * BAR, end: 8 * BAR };
  s.song.loopOn = true;
  s.transport.position = 4 * BAR;
  s.status.message = { id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: true, notes: null };
}

/** The four tracks of the mocks F4-Shelf*, with the shelf open. */
function shelfSong(s: AppState, shelf: Partial<ShelfView>): void {
  bandSong(s, BAND_TRACKS.slice(0, 4));
  s.shelf = { ...closedShelf(), ...shelf };
}

// R7-Arrange6: the ● of "2 ベース" was pressed, so it is the selected track now.
SCENES.tracks6 = {
  setup(s) {
    bandSong(s);
    s.song.playTarget = BAND_TRACK_ID + 2;
  },
  after: (s) => (s.status.message = { id: "armMoved", track: { number: 2, name: "ベース" } }),
  meters: QUIET,
};
// F4-ShelfSample without the sample: "＋ トラックを追加" opened the shelf, no row picked yet.
SCENES.shelfNewTrack = { setup: (s) => shelfSong(s, {}), bands: ["shelf"], meters: QUIET };
// R8-Shelf84: the 31 instruments of the shelf (spec 3, 84). The folded groups are the screen's own
// (tools/design/shoot_ui.py folds all but "ピアノ ・ 鍵盤" and "ギター" by clicking their headings).
SCENES.shelf84 = { setup: (s) => shelfSong(s, {}), bands: ["shelf"], meters: QUIET };
// F4-ShelfAudition: "シンセベース" is auditioned for a new track (it would be track 5).
SCENES.shelfAudition = {
  setup(s) {
    shelfSong(s, { selected: "synthBass" });
    s.transport.canRecord = false;
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "auditionNewTrack", instrument: "synthBass" }),
  meters: { ...QUIET, keys: [60, 64, 67], velocity: 87, sustain: 127, modulation: 12 },
};
// F4-ShelfReplace: "エレクトリックピアノ" is auditioned in place of the instrument of "1 ピアノ".
SCENES.shelfReplace = {
  setup(s) {
    shelfSong(s, { mode: { kind: "replace", track: BAND_TRACK_ID + 1 }, selected: "electricPiano", current: "grandPiano" });
    s.transport.canRecord = false;
  },
  bands: ["shelf"],
  after: (s) =>
    (s.status.message = {
      id: "auditionReplace",
      instrument: "electricPiano",
      track: { number: 1, name: "ピアノ" },
      current: "grandPiano",
    }),
  meters: { ...QUIET, keys: [60, 64, 67], velocity: 87, sustain: 127, modulation: 12 },
};

// F4-Mixer: five tracks, the two returns and the master, playing at 9.2.000. The reverb of return A is the selected
// insert, and its first row "響きの長さ" is chosen in the parameters panel.
const RETURN_A: StripRef = { kind: "return", bus: "a" };
const RETURN_B: StripRef = { kind: "return", bus: "b" };
const MIXER_STRIPS: Pick<TrackView, "gainDb" | "pan" | "sends" | "inserts">[] = [
  { gainDb: 0, pan: 0, sends: [-15, null], inserts: noInserts() },
  { gainDb: 0, pan: 0, sends: [null, null], inserts: firstInsert("compressor") },
  { gainDb: 0, pan: 0, sends: [-24, null], inserts: firstInsert("compressor") },
  { gainDb: -3, pan: -20, sends: [-15, null], inserts: firstInsert("eq4") },
  { gainDb: 0, pan: 20, sends: [-15, -18], inserts: noInserts() },
];
function mixerSong(s: AppState, selected: InsertRef, effect: EffectKind, row: number | null): void {
  bandSong(s, BAND_TRACKS.slice(0, 5));
  s.song.tracks = s.song.tracks.map((t, i) => ({ ...t, ...structuredClone(MIXER_STRIPS[i]!) }));
  s.params = { selected: { kind: "builtin", insert: selected, effect, values: effectDefaults(effect), row } };
}
const MIXER_METERS: FakeMeters = {
  velocity: 83,
  loadPermille: 120,
  master: [dbfs(-2.5), dbfs(-4)],
  returns: [
    [dbfs(-13), dbfs(-15)],
    [dbfs(-28), dbfs(-34)],
  ],
  tracks: [
    [dbfs(-8), dbfs(-9.5)],
    [dbfs(-11), dbfs(-13)],
    [dbfs(-5), dbfs(-6.5)],
    [dbfs(-17), dbfs(-20)],
    [dbfs(-10), dbfs(-12)],
  ],
  position: { tick: 8 * BAR + 480, ticksPerUs: 0, state: "playing", countInBeatsLeft: 0, lap: 0 },
};
SCENES.mixer = {
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", 0);
    s.transport.state = "playing";
  },
  bands: ["mixer", "params"],
  after: (s) =>
    (s.status.message = { id: "paramSelected", loopBars: { from: 5, to: 9 }, insert: { strip: RETURN_A, slot: 0 }, effect: "reverb", param: 0 }),
  meters: MIXER_METERS,
};
// R6-Delay: the delay of return B in the parameters panel, no row chosen.
SCENES.paramsDelay = {
  setup: (s) => mixerSong(s, { strip: RETURN_B, slot: 0 }, "delay", null),
  bands: ["mixer", "params"],
  meters: QUIET,
};
// Not from a mock: "空き ＋" of the second slot of "1 ピアノ" opened the shelf, which lists the effects; "コンプレッサー" is chosen.
SCENES.shelfEffects = {
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", null);
    s.shelf = { ...closedShelf(), mode: { kind: "insertEffect", strip: { kind: "track", id: BAND_TRACK_ID + 1 }, slot: 1 }, selectedEffect: "compressor" };
  },
  bands: ["mixer", "shelf"],
  meters: QUIET,
};

// Not from a mock: the scene recording, with the listener moving at 96.00 (the clip being recorded grows).
SCENES.recordingMoving = {
  ...SCENES.recording!,
  meters: {
    ...SCENES.recording!.meters,
    position: { tick: 6 * BAR, ticksPerUs: 768e-6, state: "recording", countInBeatsLeft: 0, lap: 0 },
  },
};

// R7-Drum: "3 ドラム" (スタジオキット) with its clip "8ビート" (bars 1–4, 53 notes) in the piano roll. An 8-beat for three
// bars and a half, a tom fill in the last two beats; the snares of 1.2 and 2.2 selected; キック and スネア held.
const DRUM_TAKE = 80;
const DRUM_CLIP = 81;
const GM = { kick: 36, snare: 38, lowTom: 45, midTom: 47, closedHiHat: 42, crash: 49, highTom: 50 } as const;
const DRUM_NOTES: { start: number; length: number; key: number; velocity: number }[] = (() => {
  const hit = (start: number, key: number, velocity: number) => ({ start, length: 120, key, velocity });
  const out = [hit(0, GM.crash, 110)];
  for (let eighth = 0; eighth < 30; eighth++) out.push(hit(eighth * 240, GM.closedHiHat, eighth % 2 === 0 ? 88 : 72));
  for (let bar = 0; bar < 4; bar++) {
    const b = bar * BAR;
    out.push(hit(b, GM.kick, 104), hit(b + 720, GM.kick, 104), hit(b + 480, GM.snare, 96));
    if (bar < 3) out.push(hit(b + 960, GM.kick, 104), hit(b + 1440, GM.snare, 96));
  }
  const fill = 3 * BAR + 1440;
  out.push(hit(fill, GM.highTom, 100), hit(fill + 120, GM.highTom, 84), hit(fill + 240, GM.midTom, 100), hit(fill + 360, GM.lowTom, 104));
  return out.sort((a, b) => a.start - b.start || a.key - b.key);
})();
FAKE_TAKES.set(DRUM_TAKE, { id: DRUM_TAKE, length: 4 * BAR, notes: DRUM_NOTES });
const DRUM_SELECTED = DRUM_NOTES.flatMap((n, i) => (n.key === GM.snare && (n.start === 480 || n.start === BAR + 480) ? [i] : []));
SCENES.drumRoll = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 5));
    const drum = s.song.tracks[2]!;
    drum.clips = [fakeClip(DRUM_CLIP, "8ビート", 1, 5, 1, [DRUM_TAKE]), ...drum.clips.slice(1)];
    s.song.playTarget = drum.id;
    s.song.loopOn = false;
    s.song.noteCount = DRUM_NOTES.length;
    s.arrange = arrangeOf(DRUM_CLIP);
    s.pianoRoll = {
      ...pianoRollView(DRUM_TAKE, false),
      selectedNotes: DRUM_SELECTED.map((i) => ({ start: i, end: i + 1 })),
      quantize: { division: 16, triplet: false, strength: 100, swing: 0, align: "start" },
    };
    s.transport.position = 0;
    s.status.message = {
      id: "summary",
      loopBars: null,
      nothingSelected: false,
      notes: { count: DRUM_SELECTED.length, keys: DRUM_SELECTED.map((i) => DRUM_NOTES[i]!.key), quantized: false, names: keyCounts(DRUM_SELECTED.map((i) => DRUM_NOTES[i]!.key)) },
    };
  },
  bands: ["pianoRoll"],
  meters: { ...QUIET, keys: [GM.kick, GM.snare], velocity: 104, modulation: 12 },
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

// ---- round 5, step 2: 「あとから入れたもの」 (protocol 17) ----

const SURGE = lv2Key("Surge XT");
const SURGE_SOUND: SoundRef = { kind: "plugin", key: SURGE };
const CLEAN_GUITAR: Extract<SoundRef, { kind: "sf2" }> = { kind: "sf2", file: FLUID, bank: 0, program: 27 };
const playingKeys: Partial<FakeMeters> = { keys: [60, 64, 67], velocity: 87, sustain: 127, modulation: 12 };

const pluginView = (name: string, state: PluginState = "ready"): PluginView => ({ key: lv2Key(name), name, format: "lv2", state });
const pluginSlot = (name: string, state: PluginState = "ready", bypassed = false): InsertView => ({ effect: { kind: "plugin", ...pluginView(name, state) }, bypassed });

/** The song of the mocks F5-Plugin*: the four tracks of the band and "5 Surge XT", a track of the plugin. */
function surgeSong(s: AppState, state: PluginState = "ready"): void {
  bandSong(s, BAND_TRACKS.slice(0, 5));
  const surge = s.song.tracks[4]!;
  Object.assign(surge, { name: "Surge XT", instrument: "sine", sound: { kind: "plugin", ...pluginView("Surge XT", state) } satisfies SoundView });
  s.song.loopOn = false;
  s.status.message = { id: "summary", loopBars: null, nothingSelected: true, notes: null };
}

function shelfOpen(s: AppState, shelf: Partial<ShelfView>, library: LibraryView): void {
  s.shelf = { ...closedShelf(), ...shelf };
  s.library = library;
  s.transport.canRecord = s.shelf.selected === null && (s.shelf.selectedSound ?? null) === null;
}

// F5-SF2Shelf: FluidR3_GM.sf2 was added (its heading is the open one); "Clean Guitar" is auditioned for a new track.
SCENES.sf2Shelf = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    shelfOpen(s, { selectedSound: CLEAN_GUITAR }, { ...emptyLibrary(), sf2: fullLibrary().sf2, openedSf2: FLUID });
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "auditionNewTrackSound", sound: { kind: "sf2", name: "Clean Guitar", file: FLUID } }),
  meters: { ...QUIET, ...playingKeys },
};
// Not from a mock (spec 6 「SF2 を足した」): right after the file was added, nothing auditioned yet.
SCENES.sf2Added = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    shelfOpen(s, {}, { ...emptyLibrary(), sf2: fullLibrary().sf2, openedSf2: FLUID });
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "sf2Added", file: FLUID, presets: 128, kits: 9, skipped: 0 }),
  meters: QUIET,
};
// Not from a mock (spec 6 「SF2 が読めない」): the file was not added.
SCENES.sf2Unreadable = {
  setup: (s) => bandSong(s, BAND_TRACKS.slice(0, 4)),
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "sf2Unreadable", file: FLUID }),
  meters: QUIET,
};
// F5-SF2Drop: an SF2 file is held over the window; nothing was added yet.
SCENES.sf2DropAddable = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    s.workspace.fileDrop = { name: FLUID, addable: true, files: 1, sf2: 1 };
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "sf2Dragging" }),
  meters: QUIET,
};
// The same with a file that is no SF2 (spec 2: 「足せない」, spec 6 「SF2 でないファイルを持ってきている」).
SCENES.sf2DropNotAddable = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    s.workspace.fileDrop = { name: "piano-demo.wav", addable: false, files: 1, sf2: 0 };
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "notSf2Dragging" }),
  meters: QUIET,
};
// Not from a mock (spec 3 89-10, spec 6 「ファイルをいくつも持ってきている」): three files, two of them SF2 files.
SCENES.sf2DropMany = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    s.workspace.fileDrop = { name: FLUID, addable: true, files: 3, sf2: 2 };
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "sf2Dragging" }),
  meters: QUIET,
};
// Not from a mock (spec 6 「SF2 をいくつも足した」): two SF2 files added, one file passed over.
SCENES.sf2AddedMany = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    shelfOpen(s, {}, { ...emptyLibrary(), sf2: fullLibrary().sf2, openedSf2: FLUID });
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "sf2AddedMany", added: 2, first: FLUID, skipped: 1 }),
  meters: QUIET,
};

/** F5-ShelfFx: the inserts of the five strips, with the plugins among them ("Brick EQ" has stopped). */
function surgeMixer(s: AppState): void {
  surgeSong(s);
  const [piano, bass, drums, pad, surge] = s.song.tracks as [TrackView, TrackView, TrackView, TrackView, TrackView];
  piano.inserts = ["eq4", "compressor", null, null];
  bass.inserts = ["compressor", null, null, null];
  bass.sends = [null, null];
  drums.inserts = ["compressor", null, null, null];
  drums.slots = [null, pluginSlot("Brick EQ", "stopped"), null, null];
  pad.inserts = [null, "eq4", null, null];
  pad.slots = [pluginSlot("MVerb"), null, null, null];
  Object.assign(pad, { pan: -20, gainDb: -3 });
  surge.slots = [pluginSlot("Calf Reverb"), null, null, null];
  Object.assign(surge, { pan: 20, gainDb: 0, sends: [-15, -18] });
  for (const t of [piano, bass, drums]) t.gainDb = 0;
}

// F5-ShelfFx: "空き ＋" of the second slot of "5 Surge XT" opened the shelf on the effects; "Dragonfly Hall Reverb" is chosen.
SCENES.shelfFx = {
  setup(s) {
    surgeMixer(s);
    const surge = s.song.tracks[4]!;
    shelfOpen(s, { mode: { kind: "insertEffect", strip: { kind: "track", id: surge.id }, slot: 1 }, chosenEffect: { kind: "plugin", key: lv2Key("Dragonfly Hall Reverb") } }, fullLibrary());
    s.transport.state = "playing";
    s.transport.position = at("9.2.000");
  },
  bands: ["mixer", "shelf"],
  meters: { ...QUIET, loadPermille: 240, master: [dbfs(-6.2), dbfs(-7)], position: { tick: at("9.2.000"), ticksPerUs: 0, state: "playing", countInBeatsLeft: 0, lap: 0 } },
};
// F5-ShelfSearch: the same before a row is chosen. What is typed into the search field is the screen's own
// (tools/design/shoot_ui.py types "Dragonfly").
SCENES.shelfSearch = {
  ...SCENES.shelfFx,
  setup(s) {
    SCENES.shelfFx!.setup(s);
    s.shelf.chosenEffect = null;
  },
};
// Opt5-Plugins (14-A): the shelf with FluidR3 and the 540 plugins, opened for a new track. Which of the two
// lists is shown, and which headings are unfolded, is the screen's own.
SCENES.shelfPlugins = {
  setup(s) {
    surgeSong(s);
    shelfOpen(s, {}, fullLibrary());
  },
  bands: ["shelf"],
  meters: QUIET,
};
// F5-PluginLoading: "Surge XT" was selected and is being read.
SCENES.pluginLoading = {
  setup(s) {
    surgeSong(s);
    shelfOpen(s, { selectedSound: SURGE_SOUND, loadingSound: SURGE_SOUND }, fullLibrary());
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "pluginLoading", name: "Surge XT" }),
  meters: QUIET,
};
// F5-PluginAudition: it is read, and the keys sound with it ("窓をひらく").
SCENES.pluginAudition = {
  setup(s) {
    surgeSong(s);
    shelfOpen(s, { selectedSound: SURGE_SOUND }, fullLibrary());
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "auditionNewTrackSound", sound: { kind: "plugin", name: "Surge XT" } }),
  meters: { ...QUIET, ...playingKeys },
};
// Not from a mock (spec 6 「プラグインを読みこめない」): it did not load; nothing is selected.
SCENES.pluginUnreadable = {
  setup(s) {
    surgeSong(s);
    shelfOpen(s, { unreadable: SURGE_SOUND }, fullLibrary());
  },
  bands: ["shelf"],
  meters: QUIET,
};
// F5-PluginTrack: "5 Surge XT" is the selected track; two of sixteen steps are undone. (What the history band lists is not made yet.)
SCENES.pluginTrack = {
  setup(s) {
    surgeSong(s);
    s.library = fullLibrary();
    Object.assign(s.song, { playTarget: s.song.tracks[4]!.id, undoCount: 14, redoCount: 2, file: "changed" });
    s.transport.position = at("9.1.000");
  },
  bands: ["history"],
  after: (s) => (s.status.message = { id: "undoPending", count: 2 }),
  meters: QUIET,
};
// R8-Missing: the sound pack is not found. The four tracks keep their sounds' names and sound as the sine wave.
SCENES.missingPack = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    for (const t of s.song.tracks) t.standIn = "packMissing";
    shelfOpen(s, { available: ["sine"] }, { ...emptyLibrary(), packMissing: true });
    s.song.loopOn = false;
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "soundsMissing", tracks: 4 }),
  meters: QUIET,
};

const MISSING_PLUGINS: MissingView[] = [
  { kind: "plugin", key: SURGE, name: "Surge XT", vendor: "Surge Synth Team", format: "lv2", role: "instrument", shownPath: "/usr/lib/lv2/Surge XT.lv2" },
  { kind: "plugin", key: lv2Key("MVerb"), name: "MVerb", vendor: "DISTRHO", format: "lv2", role: "effect", shownPath: "~/.lv2/MVerb.lv2" },
];

// Not from a mock (spec 3 「SF2 が見つからないとき」): a song from another PC. Two of its tracks use presets of a file
// this PC does not have.
SCENES.missingSf2 = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    const preset = (program: number, name: string): Partial<TrackView> => ({
      instrument: "sine",
      sound: { kind: "sf2", file: FLUID, bank: 0, program, name, family: "guitar" },
      standIn: "sf2Missing",
    });
    Object.assign(s.song.tracks[0]!, preset(27, "Clean Guitar"), { name: "ギター" });
    Object.assign(s.song.tracks[3]!, preset(24, "Nylon String Guitar"), { name: "ギター 2" });
    shelfOpen(s, { missing: [{ kind: "sf2", file: FLUID, tracks: 2 }] }, emptyLibrary());
    s.song.loopOn = false;
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "sf2Missing", file: FLUID, tracks: 2 }),
  meters: QUIET,
};
// Not from a mock (spec 6 「プラグインが見つからない」): the song uses a plugin instrument and a plugin effect this PC
// does not have. The track sounds as the sine wave; the slot is passed by.
SCENES.missingPlugin = {
  setup(s) {
    surgeMixer(s);
    Object.assign(s.song.tracks[4]!, { sound: { kind: "plugin", ...pluginView("Surge XT", "missing") } satisfies SoundView, standIn: "pluginMissing" });
    s.song.tracks[3]!.slots = [pluginSlot("MVerb", "missing"), null, null, null];
    s.song.tracks[2]!.slots = [null, null, null, null];
    s.song.tracks[4]!.slots = [null, null, null, null];
    shelfOpen(s, { missing: MISSING_PLUGINS.map((m) => ({ ...m })) }, emptyLibrary());
  },
  bands: ["mixer", "shelf"],
  meters: QUIET,
};
// R8-DrumPlay: the scene drumRoll while it plays, at 2.2.000: the closed hi-hat and the snare sound.
SCENES.drumPlay = {
  ...SCENES.drumRoll!,
  setup(s) {
    SCENES.drumRoll!.setup(s);
    s.pianoRoll.selectedNotes = [];
    s.transport.state = "playing";
    s.transport.position = 0;
    s.status.message = null;
  },
  meters: { ...QUIET, loadPermille: 140, master: [dbfs(-8), dbfs(-8.6)], position: { tick: BAR + 480, ticksPerUs: 0, state: "playing", countInBeatsLeft: 0, lap: 0 } },
};
/** A kit with a sound on every key from 35 to 81, as `meloet_model::drums::gm_kit_rows` orders its rows. */
const SF2_KIT_ROWS = [
  ...[49, 57, 55, 52, 51, 59, 53, 46, 44, 42, 50, 48, 47, 45, 43, 41, 39, 38, 40, 37, 36, 35],
  ...[54, 56, 58],
  ...Array.from({ length: 22 }, (_, i) => 60 + i),
];
// Not from a mock (spec 3 「SF2 のドラムのキット」): "3 ドラム" sounds with the standard kit of FluidR3; its rows are the
// keys the kit has, in the order of the spec.
SCENES.drumRollSf2 = {
  ...SCENES.drumRoll!,
  setup(s) {
    SCENES.drumRoll!.setup(s);
    s.library = { ...emptyLibrary(), sf2: fullLibrary().sf2 };
    Object.assign(s.song.tracks[2]!, {
      instrument: "sine",
      sound: { kind: "sf2", file: FLUID, bank: 128, program: 0, name: "Standard", family: "drumKit" } satisfies SoundView,
      drumKeys: SF2_KIT_ROWS,
    });
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
  state.song = settleSong(state.song);
  state.shelf = settleShelf(state.shelf);
  return { state, bands, meters: def.meters, scrollToEnd: def.scrollToEnd, live: def.live ?? NO_LIVE };
}

// ---- round 5, step 3: plugin windows, a plugin in the parameters panel, what is left of 86 (protocol 18) ----

const SURGE_VST3: PluginView = { key: vst3Key("Surge XT"), name: "Surge XT", format: "vst3", state: "ready" };
const trackStrip = (number: number): StripRef => ({ kind: "track", id: BAND_TRACK_ID + number });
const pluginSelected = (slot: PluginSlotRef, table: number, more: Partial<SelectedPlugin> = {}): AppState["params"] => ({
  selected: { kind: "plugin", slot, window: "opens", table, count: fakeParamCount(table), row: null, automated: [], ...more },
});
const playingAt = (text: string): FakeMeters["position"] => ({ tick: at(text), ticksPerUs: 0, state: "playing", countInBeatsLeft: 0, lap: 0 });

/**
 * The song of the mocks F-WindowOpen, F-ParamFew and F-ParamNoWindow, playing at 9.2.000: "Brick EQ" on "3 ドラム"
 * has stopped, "Dragonfly Reverb" sits on return A. The mocks' fifth track is a voice from the microphone,
 * which is not made yet: here it keeps the band's fifth instrument.
 */
function windowSong(s: AppState): void {
  bandSong(s, BAND_TRACKS.slice(0, 5));
  const [piano, bass, drums, pad, vocal] = s.song.tracks as [TrackView, TrackView, TrackView, TrackView, TrackView];
  Object.assign(piano, { inserts: ["eq4", "compressor", null, null], sends: [-12, null], pan: -12 });
  Object.assign(bass, { inserts: firstInsert("compressor"), sends: [null, null] });
  Object.assign(drums, { inserts: firstInsert("compressor"), slots: [null, pluginSlot("Brick EQ", "stopped"), null, null], sends: [-18, null] });
  Object.assign(pad, { instrument: "warmPad", sends: [-6, -15], pan: 24 });
  Object.assign(vocal, { name: "ボーカル", inserts: ["eq4", "compressor", null, null], sends: [-9, -18], gainDb: -3.2 });
  Object.assign(s.song.returns[0], { inserts: noInserts(), slots: [pluginSlot("Dragonfly Reverb"), null, null, null] });
  s.song.returns[1].gainDb = -2;
  // The shelf knows the plugins of the song: the return takes the kind of its plugin as its name ("A リバーブ", user:2026-11-19).
  const effect = (category: PluginEffectCategory) => ({ role: "effect", category }) as const;
  s.library = {
    ...emptyLibrary(),
    plugins: [
      pluginRow("Dragonfly Reverb", "Michael Willis", effect("reverb"), ["lv2"]),
      pluginRow("Mossy Verb", "Example Audio", effect("reverb"), ["lv2"]),
      pluginRow("Brick EQ", "Example Audio", effect("eq"), ["lv2"]),
    ],
  };
  s.transport.state = "playing";
  s.transport.position = at("9.2.000");
}
const WINDOW_METERS: FakeMeters = { ...QUIET, loadPermille: 210, master: [dbfs(-6.2), dbfs(-7)], position: playingAt("9.2.000") };
const DRAGONFLY_SLOT: PluginSlotRef = { kind: "insert", insert: { strip: RETURN_A, slot: 0 } };
const SURGE_SLOT: PluginSlotRef = { kind: "instrument", track: BAND_TRACK_ID + 1 };
const SURGE_WINDOW: PluginWindowView = { slot: SURGE_SLOT, name: "Surge XT", width: 1597, height: 995, largerThanScreen: true };

// F-WindowOpen: the windows of Surge XT ("1 ピアノ") and of Dragonfly Reverb (return A) are open. The list above
// 「窓 2」 is the screen's own (tools/design/shoot_ui.py clicks it open). The mock gives no size for the second window.
SCENES.windowOpen = {
  setup(s) {
    windowSong(s);
    Object.assign(s.song.tracks[0]!, { instrument: "sine", sound: { kind: "plugin", ...SURGE_VST3 } satisfies SoundView });
    s.windows = { list: [SURGE_WINDOW, { slot: DRAGONFLY_SLOT, name: "Dragonfly Reverb", width: 640, height: 480, largerThanScreen: false }] };
    s.params = pluginSelected(SURGE_SLOT, FAKE_TABLES.surge);
  },
  bands: ["mixer"],
  meters: WINDOW_METERS,
};
// F-ParamFew: Dragonfly Reverb (LV2, 18 parameters, no groups) in the panel; its window is not open. "…の Decay を
// 動かしています" of the mock's status line is the screen's own, while a row is dragged.
SCENES.paramFew = {
  setup(s) {
    windowSong(s);
    s.params = pluginSelected(DRAGONFLY_SLOT, FAKE_TABLES.dragonfly);
  },
  bands: ["mixer", "params"],
  meters: WINDOW_METERS,
};
// F-ParamNoWindow: Mossy Verb on "4 パッド" has a window meloet cannot open yet; a list of steps and switches among its 12 parameters.
SCENES.paramNoWindow = {
  setup(s) {
    windowSong(s);
    s.song.tracks[3]!.slots = [pluginSlot("Mossy Verb"), null, null, null];
    s.params = pluginSelected({ kind: "insert", insert: { strip: trackStrip(4), slot: 0 } }, FAKE_TABLES.mossy, { window: "notYet" });
  },
  bands: ["mixer", "params"],
  after: (s) => (s.status.message = { id: "pluginWindowNotYet", loopBars: { from: 5, to: 9 }, name: "Mossy Verb" }),
  meters: WINDOW_METERS,
};
// F-ParamMany: Surge XT (VST3, 2855 parameters in six groups) is the sound of "1 ピアノ"; its window is open and
// larger than the screen. Two of its parameters are written by automation lanes, which are not made yet.
SCENES.paramMany = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 5));
    Object.assign(s.song.tracks[0]!, { instrument: "sine", sound: { kind: "plugin", ...SURGE_VST3 } satisfies SoundView });
    s.windows = { list: [SURGE_WINDOW] };
    s.params = pluginSelected(SURGE_SLOT, FAKE_TABLES.surge, { automated: [...SURGE_AUTOMATED] });
  },
  bands: ["params"],
  after: (s) => (s.status.message = { id: "pluginWindowLarge", name: "Surge XT", slot: SURGE_SLOT, width: 1597, height: 995 }),
  meters: QUIET,
};
// F6-ParamMany: the same panel. The 2080 parameters that only take a keyboard's knobs are not listed (91-3); the
// line under the list says so.
SCENES.paramMany6 = { ...SCENES.paramMany };
// F-ParamSearch: the same while "cutoff" is in the search field, which is the screen's own, as is what the status
// line says then (tools/design/shoot_ui.py types it).
SCENES.paramSearch = {
  ...SCENES.paramMany,
  after: (s) => (s.status.message = { id: "summary", loopBars: null, nothingSelected: true, notes: null }),
};
// F5-MixerBypass: MVerb on "4 パッド" is passed by (13-A) and selected; "Brick EQ" on "3 ドラム" has stopped. When the
// mock's status line ("… をとばしています ・ … は止まっています…") is said is not decided.
SCENES.mixerBypass = {
  setup(s) {
    surgeMixer(s);
    s.song.tracks[3]!.slots = [pluginSlot("MVerb", "ready", true), null, null, null];
    s.params = pluginSelected({ kind: "insert", insert: { strip: trackStrip(4), slot: 0 } }, FAKE_TABLES.mverb);
    s.transport.state = "playing";
    s.transport.position = at("9.2.000");
  },
  bands: ["mixer", "params"],
  meters: { ...WINDOW_METERS, loadPermille: 240 },
};
// R9-Inserts: 「入れかえる…」 on the compressor of "2 ベース" opened the shelf on the effects; "EQ 4 バンド" is chosen.
SCENES.insertReplace = {
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", null);
    s.params = { selected: null };
    s.shelf = { ...closedShelf(), mode: { kind: "replaceEffect", strip: trackStrip(2), slot: 0 }, selectedEffect: "eq4", currentEffect: builtinEffect("compressor") };
  },
  bands: ["mixer", "shelf"],
  meters: QUIET,
};
// R8-MixerFx: "空き ＋" of the second slot of "2 ベース" opened the shelf on the effects; "リバーブ" is chosen.
SCENES.mixerFx = {
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", null);
    s.params = { selected: null };
    s.shelf = { ...closedShelf(), mode: { kind: "insertEffect", strip: trackStrip(2), slot: 1 }, selectedEffect: "reverb" };
  },
  bands: ["mixer", "shelf"],
  meters: QUIET,
};
// R9-Params (86-5): the parameters panel with nothing selected.
SCENES.paramsNone = {
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", null);
    s.params = { selected: null };
  },
  bands: ["mixer", "params"],
  meters: QUIET,
};
// R9-Params (86-8): no output is chosen, so the master's output slot says "出口なし ▸" and no meter moves.
SCENES.noOutput = {
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", null);
    s.params = { selected: null };
    s.outputs = { ...s.outputs, chosen: null, chosenState: null };
    Object.assign(s.audio, { running: false, outputDevice: null });
    Object.assign(s.transport, { canPlay: false, canRecord: false });
  },
  bands: ["mixer"],
  meters: { velocity: 83, loadPermille: 0 },
};
// R9-Params (86-6, 86-10): "40" was typed for the ratio of the compressor of "2 ベース", and put at 20 : 1.
SCENES.paramClamped = {
  setup(s) {
    mixerSong(s, { strip: trackStrip(2), slot: 0 }, "compressor", 1);
    if (s.params.selected?.kind === "builtin") s.params.selected.values[1] = 20;
  },
  bands: ["mixer", "params"],
  after: (s) => (s.status.message = { id: "paramClamped", to: { kind: "builtin", effect: "compressor", param: 1, value: 20 } }),
  meters: QUIET,
};
// Opt5-Undo (15-C): back from the window of Surge XT, where three knobs were turned: one more step, which undo takes
// back. (What the history band lists is not made yet.)
SCENES.windowUndo = {
  setup(s) {
    surgeSong(s);
    s.library = fullLibrary();
    Object.assign(s.song, { playTarget: s.song.tracks[4]!.id, undoCount: 15, redoCount: 0, file: "changed" });
  },
  bands: ["history"],
  meters: QUIET,
};

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
  (m.returns ?? []).flat().forEach((peak, i) => v.setFloat32(72 + 4 * i, peak, true));
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
  "notesNotWritten",
  "notesNotResized",
  "armMoved",
  "trackAdded",
  "trackDeleted",
  "instrumentReplaced",
  "paramClamped",
  "paramRejected",
  "effectInserted",
  "effectRemoved",
  "insertsReordered",
  "insertReplaced",
  "effectBypassed",
  "pluginWindowOpen",
  "pluginWindowsOpen",
  "mixerPassesBy",
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
  const keys = picked.map((i) => take.notes[i]!.key);
  return { count: picked.length, keys: keys.slice(0, 4), quantized: (take.origin ?? 0) !== 0, names: keyCounts(keys) };
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
  const track = (id: number) => song.tracks.find((t) => t.id === id);
  switch (command.type) {
    case "selectClip":
      if (command.clip === null) select([], null);
      else if (clipOf(command.clip)) {
        select([command.clip], command.clip);
        song.playTarget = song.tracks[rowOf(command.clip)]!.id;
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
      if (clicked !== null && had.has(clicked)) song.playTarget = song.tracks[rowOf(clicked)]!.id;
      return done();
    }
    case "selectAllClips":
      select(
        song.tracks.flatMap((t) => t.clips.map((c) => c.id)),
        arrange.selectedClip,
      );
      return done();
    case "selectTrack": {
      const t = track(command.track);
      if (!t || busy(state)) return done();
      const moved = command.fromArm && song.playTarget !== command.track;
      song.playTarget = command.track;
      return moved ? { ...done(), notice: { id: "armMoved", track: { number: t.number, name: t.name } } } : done();
    }
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
      const top = song.tracks.findIndex((t) => t.id === to.track);
      if (top < 0) return null;
      const items = clipboard.clips.map(({ row, at, clip }) => ({ row: top + row, start: to.tick + at, clip }));
      return place(items, true, [], { id: "clipsNotPasted" });
    }
    default:
      return null;
  }
}

const busy = (state: AppState) => state.transport.state === "recording" || state.transport.state === "countIn";

const SHELF_MESSAGES: StatusMessage["id"][] = [
  "auditionNewTrack",
  "auditionReplace",
  "samplePlaying",
  "auditionNewTrackSound",
  "auditionReplaceSound",
  "samplePlayingSound",
  "pluginLoading",
];
const isShelfMessage = (m: StatusMessage | null) => m !== null && SHELF_MESSAGES.includes(m.id);
/** The short names of new tracks, by group; the organs are "オルガン" and the mallets "マレット" (spec 3, トラックの名前（84）). */
const GROUP_TRACK_NAMES: Record<string, string> = {
  pianoKeys: "ピアノ",
  guitar: "ギター",
  bass: "ベース",
  drums: "ドラム",
  padStrings: "パッド",
  wind: "管楽器",
  synth: "シンセ",
};
const groupOf = (i: InstrumentKind) => INSTRUMENT_GROUPS.find((g) => g.instruments.includes(i))!.group;
const trackNameOf = (i: InstrumentKind) =>
  i.startsWith("organ") ? "オルガン" : i === "vibraphone" || i === "marimba" ? "マレット" : GROUP_TRACK_NAMES[groupOf(i)]!;
const SAMPLE_US = 4_000_000;

/** Whether the shelf has the sound: one that comes with meloet, a preset of a file that was added, or a plugin in one of its formats. */
function inLibrary(sound: SoundRef, library: LibraryView): boolean {
  if (sound.kind === "builtin") return true;
  if (sound.kind === "plugin") return library.plugins.some((r) => r.kind.role === "instrument" && r.formats.some((f) => f.key === sound.key));
  return library.sf2.some((f) => f.file === sound.file && f.presets.some((p) => p.bank === sound.bank && p.program === sound.program));
}

/** The name the status line gives a sound of the shelf. */
function soundName(sound: SoundRef, library: LibraryView): SoundName {
  if (sound.kind === "builtin") return sound;
  if (sound.kind === "plugin") return { kind: "plugin", name: library.plugins.find((r) => r.formats.some((f) => f.key === sound.key))?.name ?? sound.key };
  const preset = library.sf2.find((f) => f.file === sound.file)?.presets.find((p) => p.bank === sound.bank && p.program === sound.program);
  return { kind: "sf2", name: preset?.name ?? "", file: sound.file };
}

function soundNameOf(view: SoundView): SoundName {
  if (view.kind === "builtin") return view;
  return view.kind === "plugin" ? { kind: "plugin", name: view.name } : { kind: "sf2", name: view.name, file: view.file };
}

/** What the shelf says on the status line while it auditions or plays a sample. */
function shelfMessage(shelf: ShelfView, song: SongView, library: LibraryView): StatusMessage | null {
  const replaced = shelf.mode.kind === "replace" ? song.tracks.find((x) => x.id === (shelf.mode as { track: number }).track) : undefined;
  if (shelf.selected !== null) {
    if (shelf.mode.kind === "newTrack") return { id: "auditionNewTrack", instrument: shelf.selected };
    if (replaced) return { id: "auditionReplace", instrument: shelf.selected, track: { number: replaced.number, name: replaced.name }, current: replaced.instrument };
  }
  const picked = shelf.selectedSound;
  if (picked !== null && picked.kind !== "builtin") {
    const sound = soundName(picked, library);
    if (shelf.loadingSound !== null && sound.kind === "plugin") return { id: "pluginLoading", name: sound.name };
    if (shelf.mode.kind === "newTrack") return { id: "auditionNewTrackSound", sound };
    if (replaced) return { id: "auditionReplaceSound", sound, track: { number: replaced.number, name: replaced.name }, current: soundNameOf(replaced.sound) };
  }
  return shelf.sample ? { id: "samplePlaying", instrument: shelf.sample.instrument } : null;
}

/** 「曲から外す」: the tracks that use it get the sine wave, and the slots that hold it are emptied. */
function withoutMissing(song: SongView, gone: Extract<Command, { type: "removeMissing" }>["missing"]): SongView {
  const is = (p: PluginView) => gone.kind === "plugin" && p.key === gone.key;
  const strip = <T extends { slots: Slots }>(s: T): T => ({ ...s, slots: s.slots.map((x) => (x?.effect.kind === "plugin" && is(x.effect) ? null : x)) as Slots });
  return {
    ...song,
    tracks: song.tracks.map((t) => {
      const uses = t.sound.kind === "plugin" ? is(t.sound) : t.sound.kind === "sf2" && gone.kind === "sf2" && t.sound.file === gone.file;
      return strip(uses ? { ...t, instrument: "sine", sound: builtinSound("sine"), standIn: null, drumKeys: null } : t);
    }),
    returns: [strip(song.returns[0]), strip(song.returns[1])],
    master: strip(song.master),
  };
}

/** The commands for any sound or effect, as the ones for what comes with meloet when they name one of those. */
function ownCommand(command: Command): Command {
  switch (command.type) {
    case "shelfSelectSound":
      return command.sound === null || command.sound.kind === "builtin" ? { type: "shelfSelect", instrument: command.sound?.instrument ?? null } : command;
    case "playSampleOf":
      return command.sound.kind === "builtin" ? { type: "playSample", instrument: command.sound.instrument } : command;
    case "toggleFavoriteSound":
      return command.sound.kind === "builtin" ? { type: "toggleFavorite", instrument: command.sound.instrument } : command;
    case "shelfChooseEffect":
      return command.effect === null || command.effect.kind === "builtin" ? { type: "shelfSelectEffect", effect: command.effect?.effect ?? null } : command;
    default:
      return command;
  }
}

const sameRef = (a: SoundRef | EffectRef, b: SoundRef | EffectRef) => JSON.stringify(a) === JSON.stringify(b);

/**
 * Protocol 17: SF2 presets and plugins in the shelf, enough for pictures and tests. A plugin is read at once
 * (the scenes show the reading), `shelfConfirm` puts none of them into the song, and nothing sounds.
 * Changes `shelf`, `library` and `fileDrop` in place.
 */
function reduceAdded(command: Command, shelf: ShelfView, library: LibraryView, ws: AppState["workspace"]): void {
  const toggle = <T extends SoundRef | EffectRef>(list: T[], item: T): T[] =>
    list.some((x) => sameRef(x, item)) ? list.filter((x) => !sameRef(x, item)) : [...list, item];
  switch (command.type) {
    // What ends an audition, or starts one of a sound of meloet, puts away the SF2 preset or plugin that was auditioned.
    case "shelfSelect":
    case "shelfCancel":
    case "shelfConfirm":
    case "openShelf":
      shelf.selectedSound = shelf.loadingSound = shelf.unreadable = null;
      if (command.type !== "shelfSelect") shelf.chosenEffect = null;
      break;
    case "shelfSelectEffect":
      shelf.chosenEffect = null;
      break;
    case "shelfSelectSound":
      if (shelf.mode.kind === "insertEffect" || shelf.mode.kind === "replaceEffect") break;
      shelf.selected = shelf.loading = shelf.loadingSound = shelf.unreadable = null;
      shelf.selectedSound = command.sound !== null && inLibrary(command.sound, library) ? command.sound : null;
      break;
    case "shelfChooseEffect": {
      const effect = command.effect;
      if (shelf.mode.kind !== "insertEffect" && shelf.mode.kind !== "replaceEffect") break;
      const has = effect?.kind === "plugin" && library.plugins.some((r) => r.kind.role === "effect" && r.formats.some((f) => f.key === effect.key));
      shelf.selectedEffect = null;
      shelf.chosenEffect = has ? effect : null;
      break;
    }
    case "toggleFavoriteSound":
      if (inLibrary(command.sound, library)) shelf.favoriteSounds = toggle(shelf.favoriteSounds, command.sound);
      break;
    case "toggleFavoriteEffect": {
      const effect = command.effect;
      if (effect.kind === "builtin" || library.plugins.some((r) => r.formats.some((f) => f.key === effect.key))) {
        shelf.favoriteEffects = toggle(shelf.favoriteEffects, effect);
      }
      break;
    }
    case "usePluginFormat": {
      const row = library.plugins.find((r) => r.formats.some((f) => f.key === command.key));
      if (!row || row.key === command.key) break;
      // What named the row by the format it used names it by the new one: the star stays on the row.
      const moved = <T extends SoundRef | EffectRef>(x: T): T => (x.kind === "plugin" && x.key === row.key ? { ...x, key: command.key } : x);
      shelf.favoriteSounds = shelf.favoriteSounds.map(moved);
      shelf.favoriteEffects = shelf.favoriteEffects.map(moved);
      row.key = command.key;
      break;
    }
    case "cancelFileDrop":
      ws.fileDrop = null;
      break;
    case "removeMissing": {
      const gone = command.missing;
      shelf.missing = shelf.missing.filter((m) => !(m.kind === gone.kind && (m.kind === "sf2" ? gone.kind === "sf2" && m.file === gone.file : gone.kind === "plugin" && m.key === gone.key)));
      break;
    }
    default:
      break;
  }
}

/**
 * Protocol 13: the track commands and the shelf, enough for pictures and tests (no history, no undo count).
 * Returns what changed, or null when the command is none of these.
 */
export function reduceTracks(
  state: AppState,
  bands: FakeBands,
  command: Command,
  nowUs = 0,
): { song: SongView; shelf: ShelfView; canRecord: boolean; notice?: StatusMessage } | null {
  const song = structuredClone(state.song);
  const shelf = structuredClone(state.shelf);
  const at = (id: number) => song.tracks.findIndex((t) => t.id === id);
  const ref = (i: number): TrackRef => ({ number: i + 1, name: song.tracks[i]!.name });
  const renumber = () => song.tracks.forEach((t, i) => (t.number = i + 1));
  const audition = (selected: InstrumentKind | null) => {
    shelf.selected = selected;
    shelf.loading = null;
  };
  let notice: StatusMessage | undefined;
  const change = !busy(state);
  switch (command.type) {
    case "deleteTrack": {
      const i = at(command.track);
      if (i < 0 || !change) return null;
      notice = { id: "trackDeleted", track: ref(i) };
      song.tracks.splice(i, 1);
      renumber();
      if (song.playTarget === command.track) song.playTarget = (song.tracks[i] ?? song.tracks[i - 1])?.id ?? null;
      break;
    }
    case "renameTrack": {
      const i = at(command.track);
      const name = command.name.trim();
      if (i < 0 || !change || name === "") return null;
      song.tracks[i]!.name = name;
      break;
    }
    case "moveTrack": {
      const i = at(command.track);
      if (i < 0 || !change) return null;
      const [t] = song.tracks.splice(i, 1);
      song.tracks.splice(Math.min(command.to, song.tracks.length), 0, t!);
      renumber();
      break;
    }
    case "setTrackColor": {
      const i = at(command.track);
      if (i < 0 || !change || command.color < 1 || command.color > 5) return null;
      song.tracks[i]!.color = command.color;
      break;
    }
    case "openShelf": {
      if (!change) return null;
      const mode = command.mode;
      if (mode.kind === "replace" && at(mode.track) < 0) return null;
      const held = mode.kind === "replaceEffect" ? (stripOf(song, mode.strip)?.slots[mode.slot] ?? null) : null;
      if (mode.kind === "replaceEffect" && !held) return null;
      shelf.mode = mode;
      shelf.current = mode.kind === "replace" ? song.tracks[at(mode.track)]!.instrument : null;
      shelf.currentEffect = held ? (held.effect.kind === "builtin" ? builtinEffect(held.effect.effect) : { kind: "plugin", key: held.effect.key }) : null;
      audition(null);
      if (bands.side.at(-1) !== "shelf") bands.side = [...bands.side.filter((b) => b !== "shelf"), "shelf"];
      break;
    }
    case "shelfSelect":
      audition(command.instrument !== null && shelf.available.includes(command.instrument) ? command.instrument : null);
      break;
    case "shelfCancel":
      audition(null);
      break;
    case "shelfConfirm": {
      const picked = shelf.selected;
      if (picked === null || !change) return null;
      if (shelf.mode.kind === "newTrack") {
        const base = trackNameOf(picked);
        const names = new Set(song.tracks.map((t) => t.name));
        let name = base;
        for (let n = 2; names.has(name); n++) name = `${base} ${n}`;
        const sel = song.playTarget === null ? -1 : at(song.playTarget);
        const place = sel < 0 ? song.tracks.length : sel + 1;
        const id = Math.max(0, ...song.tracks.map((t) => t.id)) + 1;
        song.tracks.splice(place, 0, {
          id,
          number: place + 1,
          name,
          color: (place % 5) + 1,
          instrument: picked,
          sound: builtinSound(picked),
          standIn: null,
          drumKeys: null,
          gainDb: 0,
          pan: 0,
          sends: newTrackSends(picked),
          sendDefaults: newTrackSends(picked),
          inserts: noInserts(),
          slots: builtinSlots(noInserts()),
          muted: false,
          soloed: false,
          clips: [],
        });
        renumber();
        song.playTarget = id;
        notice = { id: "trackAdded", track: ref(place) };
      } else if (shelf.mode.kind === "replace") {
        const i = at(shelf.mode.track);
        if (i < 0) return null;
        if (song.tracks[i]!.instrument !== picked) {
          song.tracks[i]!.instrument = picked;
          notice = { id: "instrumentReplaced", track: ref(i), instrument: picked };
        }
        shelf.current = picked;
      }
      audition(null);
      break;
    }
    case "toggleFavorite": {
      const had = shelf.favorites.includes(command.instrument);
      const all = INSTRUMENT_GROUPS.flatMap((g) => [...g.instruments]);
      shelf.favorites = all.filter((i) => (i === command.instrument ? !had : shelf.favorites.includes(i)));
      break;
    }
    case "playSample":
      shelf.sample = { instrument: command.instrument, startedUs: nowUs, lengthUs: SAMPLE_US };
      break;
    case "stopSample":
      shelf.sample = null;
      break;
    default:
      return null;
  }
  const canRecord = shelf.selected === null && song.playTarget !== null && state.transport.canPlay;
  return { song, shelf, canRecord, notice };
}

/** The table of each fake plugin that has parameters, by its name. */
const PLUGIN_TABLES: Record<string, number> = {
  "Dragonfly Reverb": FAKE_TABLES.dragonfly,
  "Mossy Verb": FAKE_TABLES.mossy,
  MVerb: FAKE_TABLES.mverb,
  "Surge XT": FAKE_TABLES.surge,
};

/** The strip `strip` names in `song`: a track, a return or the master. */
function stripOf(song: SongView, strip: StripRef) {
  if (strip.kind === "master") return song.master;
  if (strip.kind === "return") return song.returns[strip.bus === "a" ? 0 : 1];
  const id = strip.id;
  return song.tracks.find((t) => t.id === id);
}

/** A level as Rust keeps it: `null` (−∞) below −60, at most +6, in steps of 0.01. */
const clampDb = (db: number | null) => (db === null || db < -60 ? null : Math.min(6, Math.round(db * 100) / 100));

/**
 * Protocol 14: the mixer, enough for pictures and tests (no history, no undo count, no previews: a command with
 * `commit: false` changes nothing here, as the state never shows a preview). Returns what changed, or null when
 * the command is none of these. The values of the effects are kept only for the selected insert. Taken while
 * recording and counting in too (design-notes A-14).
 */
export function reduceMixer(
  state: AppState,
  shelfNow: ShelfView,
  songNow: SongView,
  command: Command,
): {
  song: SongView;
  params: AppState["params"];
  shelf: ShelfView;
  windows: AppState["windows"];
  showParams: boolean;
  notice: StatusMessage | null;
} | null {
  const song = structuredClone(songNow);
  let notice: StatusMessage | null = null;
  const params = structuredClone(state.params);
  const shelf = structuredClone(shelfNow);
  const windows = structuredClone(state.windows);
  let showParams = false;
  const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
  /** The selected insert when it holds an effect that comes with meloet. */
  const own = () => (params.selected?.kind === "builtin" ? params.selected : null);
  const pluginAt = (slot: PluginSlotRef): PluginView | null => {
    if (slot.kind === "instrument") {
      const sound = song.tracks.find((t) => t.id === slot.track)?.sound;
      return sound?.kind === "plugin" ? sound : null;
    }
    const held = stripOf(song, slot.insert.strip)?.slots[slot.insert.slot]?.effect;
    return held?.kind === "plugin" ? held : null;
  };
  const openWindow = (slot: PluginSlotRef, name: string) => {
    if (windows.list.some((w) => same(w.slot, slot))) return;
    windows.list.push({ slot, name, width: 800, height: 600, largerThanScreen: false });
    notice = windows.list.length === 1 ? { id: "pluginWindowOpen", name } : { id: "pluginWindowsOpen", count: windows.list.length };
  };
  /** The effect in a slot, as the status line names it. */
  const nameAt = (insert: InsertRef): EffectName | null => {
    const held = stripOf(song, insert.strip)?.slots[insert.slot];
    if (!held) return null;
    return held.effect.kind === "builtin" ? { kind: "builtin", effect: held.effect.effect } : { kind: "plugin", name: held.effect.name };
  };
  // A plugin's window opens with it; one that has none the fake could open brings the panel to the front.
  const selectPlugin = (slot: PluginSlotRef, plugin: PluginView) => {
    if (params.selected?.kind !== "plugin" || !same(params.selected.slot, slot)) {
      const table = PLUGIN_TABLES[plugin.name] ?? null;
      const window = plugin.name === "Mossy Verb" ? "notYet" : "opens";
      params.selected = { kind: "plugin", slot, window, table, count: table === null ? 0 : fakeParamCount(table), row: null, automated: [] };
    }
    if (plugin.state !== "ready") return;
    if (params.selected.window === "opens") openWindow(slot, plugin.name);
    else showParams = true;
  };
  const select = (insert: InsertRef | null) => {
    const plugin = insert === null ? null : pluginAt({ kind: "insert", insert });
    if (insert !== null && plugin) return selectPlugin({ kind: "insert", insert }, plugin);
    const effect = insert === null ? null : (stripOf(song, insert.strip)?.inserts[insert.slot] ?? null);
    if (insert === null || effect === null) params.selected = null;
    else if (!same(own()?.insert, insert)) {
      params.selected = { kind: "builtin", insert, effect, values: insertDefaults(effect, insert.strip), row: null };
    }
    showParams = params.selected !== null;
  };
  switch (command.type) {
    case "setGain": {
      const strip = stripOf(song, command.strip);
      if (strip && command.commit) strip.gainDb = clampDb(command.db);
      break;
    }
    case "setPan": {
      const strip = stripOf(song, command.strip);
      if (strip && "pan" in strip && command.commit) strip.pan = Math.max(-100, Math.min(100, Math.round(command.pan)));
      break;
    }
    case "setSend": {
      const track = song.tracks.find((t) => t.id === command.track);
      if (track && command.commit) track.sends[command.bus === "a" ? 0 : 1] = clampDb(command.db);
      break;
    }
    case "setParam": {
      const sel = own();
      const spec = EFFECT_PARAMS[command.effect][command.param];
      const there = stripOf(song, command.insert.strip)?.inserts[command.insert.slot];
      if (!spec || there !== command.effect || !command.commit) break;
      if (sel && JSON.stringify(sel.insert) === JSON.stringify(command.insert)) {
        const v = Math.max(spec.min, Math.min(spec.max, command.value));
        sel.values[command.param] = spec.shape === "continuous" ? v : Math.round(v);
      }
      break;
    }
    case "cancelMixPreview":
      break;
    // A typed value of an effect that comes with meloet: the number in it, put at an end of the range (and said so); a
    // text with no number changes nothing and says so with the value as a bare number ("-24"), as core does. A plugin's is the
    // transport's (FakeParamTables).
    case "typeParam": {
      const p = command.param;
      if (p.kind !== "builtin") return null;
      const spec = EFFECT_PARAMS[p.effect][p.param];
      const sel = own();
      if (!spec || stripOf(song, p.insert.strip)?.inserts[p.insert.slot] !== p.effect) break;
      const now = sel && same(sel.insert, p.insert) ? (sel.values[p.param] ?? spec.default) : spec.default;
      const n = Number.parseFloat(command.text.trim().replace(/^[−ー]/, "-"));
      if (!Number.isFinite(n)) {
        notice = { id: "paramRejected", param: p, kept: String(now) };
        break;
      }
      const v = Math.max(spec.min, Math.min(spec.max, spec.shape === "continuous" ? n : Math.round(n)));
      if (v !== n) notice = { id: "paramClamped", to: { kind: "builtin", effect: p.effect, param: p.param, value: v } };
      if (sel && same(sel.insert, p.insert)) sel.values[p.param] = v;
      break;
    }
    case "toggleReturnMute": {
      const ret = song.returns[command.bus === "a" ? 0 : 1];
      ret.muted = !ret.muted;
      break;
    }
    case "removeEffect": {
      const strip = stripOf(song, command.insert.strip);
      if (!strip) break;
      const at: PluginSlotRef = { kind: "insert", insert: command.insert };
      const gone = nameAt(command.insert);
      if (gone) notice = { id: "effectRemoved", insert: command.insert, effect: gone };
      strip.inserts[command.insert.slot] = null;
      strip.slots[command.insert.slot] = null;
      if (same(own()?.insert, command.insert) || (params.selected?.kind === "plugin" && same(params.selected.slot, at))) params.selected = null;
      windows.list = windows.list.filter((w) => !same(w.slot, at));
      break;
    }
    case "selectInsert":
      select(command.insert);
      break;
    case "selectInstrument": {
      const slot: PluginSlotRef = { kind: "instrument", track: command.track };
      const plugin = pluginAt(slot);
      if (plugin) selectPlugin(slot, plugin);
      break;
    }
    case "selectParam": {
      const sel = params.selected;
      if (sel?.kind === "builtin") sel.row = command.param !== null && EFFECT_PARAMS[sel.effect][command.param] ? command.param : null;
      else if (sel) sel.row = command.param;
      break;
    }
    case "toggleBypass": {
      const held = stripOf(song, command.insert.strip)?.slots[command.insert.slot];
      if (!held || (held.effect.kind === "plugin" && held.effect.state === "stopped")) break;
      held.bypassed = !held.bypassed;
      notice = { id: "effectBypassed", insert: command.insert, effect: nameAt(command.insert)!, bypassed: held.bypassed };
      break;
    }
    case "moveInsert": {
      const strip = stripOf(song, command.insert.strip);
      const from = command.insert.slot;
      const to = Math.min(strip ? strip.slots.length - 1 : 0, command.to);
      if (!strip || !strip.slots[from] || to === from) break;
      for (const list of [strip.inserts, strip.slots] as unknown[][]) list.splice(to, 0, ...list.splice(from, 1));
      // Where a slot of this strip is afterwards: the selection and the windows stay with their effects.
      const moved = (slot: number) => (slot === from ? to : slot > from && slot <= to ? slot - 1 : slot < from && slot >= to ? slot + 1 : slot);
      const follow = (insert: InsertRef) => {
        if (same(insert.strip, command.insert.strip)) insert.slot = moved(insert.slot);
      };
      const sel = params.selected;
      if (sel?.kind === "builtin") follow(sel.insert);
      else if (sel?.slot.kind === "insert") follow(sel.slot.insert);
      for (const w of windows.list) if (w.slot?.kind === "insert") follow(w.slot.insert);
      const order = strip.slots.flatMap((_, i) => nameAt({ strip: command.insert.strip, slot: i }) ?? []);
      notice = { id: "insertsReordered", strip: command.insert.strip, order };
      break;
    }
    case "openPluginWindow": {
      const plugin = command.slot && pluginAt(command.slot);
      if (command.slot && plugin && plugin.state === "ready" && plugin.name !== "Mossy Verb") openWindow(command.slot, plugin.name);
      break;
    }
    case "closePluginWindow":
      windows.list = windows.list.filter((w) => !same(w.slot, command.slot));
      break;
    case "shelfSelectEffect":
      if (shelf.mode.kind === "insertEffect" || shelf.mode.kind === "replaceEffect") shelf.selectedEffect = command.effect;
      break;
    case "shelfConfirm": {
      if (shelf.mode.kind !== "insertEffect" && shelf.mode.kind !== "replaceEffect") return null;
      const { kind, strip, slot } = shelf.mode;
      const to = stripOf(song, strip);
      if (!to || shelf.selectedEffect === null || (to.slots[slot] !== null) !== (kind === "replaceEffect")) break;
      if (same(shelf.currentEffect, builtinEffect(shelf.selectedEffect))) break;
      const effect: EffectName = { kind: "builtin", effect: shelf.selectedEffect };
      notice = kind === "replaceEffect" ? { id: "insertReplaced", insert: { strip, slot }, effect } : { id: "effectInserted", insert: { strip, slot }, effect };
      to.inserts[slot] = shelf.selectedEffect;
      to.slots[slot] = null;
      windows.list = windows.list.filter((w) => !same(w.slot, { kind: "insert", insert: { strip, slot } }));
      shelf.mode = { kind: "newTrack" };
      shelf.selectedEffect = shelf.currentEffect = null;
      params.selected = null;
      select({ strip, slot });
      break;
    }
    default:
      return null;
  }
  return { song, params, shelf, windows, showParams, notice };
}

/** The slots the mixer passes by as it opens (MixerPassesBy): in the mixer's order, the bypassed ones and the stopped ones. */
function passedBy(song: SongView): StatusMessage | null {
  const strips: { strip: StripRef; slots: SongView["master"]["slots"] }[] = [
    ...song.tracks.map((t) => ({ strip: { kind: "track", id: t.id } as StripRef, slots: t.slots })),
    { strip: RETURN_A, slots: song.returns[0].slots },
    { strip: { kind: "return", bus: "b" }, slots: song.returns[1].slots },
    { strip: { kind: "master" }, slots: song.master.slots },
  ];
  const bypassed: NamedInsert[] = [];
  const stopped: NamedInsert[] = [];
  for (const { strip, slots } of strips)
    slots.forEach((held, slot) => {
      if (!held) return;
      const effect: EffectName = held.effect.kind === "builtin" ? { kind: "builtin", effect: held.effect.effect } : { kind: "plugin", name: held.effect.name };
      if (held.effect.kind === "plugin" && held.effect.state === "stopped") stopped.push({ insert: { strip, slot }, effect });
      else if (held.bypassed) bypassed.push({ insert: { strip, slot }, effect });
    });
  return bypassed.length + stopped.length > 0 ? { id: "mixerPassesBy", bypassed, stopped } : null;
}

/** `rest` is the status line when no notice is up. */
function reduce(state: AppState, bands: FakeBands, rest: StatusMessage | null, sent: Command, clipboard: FakeClipboard, nowUs = 0): Change[] {
  const command = ownCommand(sent);
  const library = structuredClone(state.library);
  const ws = structuredClone(state.workspace);
  const outputs = structuredClone(state.outputs);
  const keyboards = structuredClone(state.keyboards);
  const transport = structuredClone(state.transport);
  const pianoRoll = structuredClone(state.pianoRoll);
  const plugins = structuredClone(state.plugins);
  reducePianoRoll(pianoRoll, command);
  const tracks = reduceTracks(state, bands, command, nowUs);
  if (tracks) transport.canRecord = tracks.canRecord;
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
      const mixerWasOpen = bands.bottom.includes("mixer");
      const fold = bands.toggle(command.band);
      if (command.band === "mixer" && !mixerWasOpen && bands.bottom.includes("mixer")) notice = passedBy(state.song);
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
      if (plugins.stoppedOpen) plugins.stoppedOpen = false;
      else if (ws.songDialog) ws.songDialog = null;
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
    // The fake scans nothing: a retry stays "ためしています…", and "場所を足す…" and "もう一度スキャン" do nothing.
    case "openScanStopped":
      plugins.stoppedOpen = true;
      break;
    case "closeScanStopped":
      plugins.stoppedOpen = false;
      break;
    case "openPluginPlaces":
      plugins.stoppedOpen = false;
      plugins.placesOpen = true;
      ws.settings = { draftOutput: ws.settings?.draftOutput ?? outputs.chosen, advancedOpen: true };
      break;
    case "togglePluginPlaces":
      plugins.placesOpen = !plugins.placesOpen;
      break;
    case "removePluginPlace":
      plugins.places = plugins.places.filter((p) => !p.added || p.path !== command.path);
      break;
    case "retryStoppedPlugin":
      for (const p of plugins.stopped) if (p.path === command.path && p.id === command.id) p.retrying = true;
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
  const mixer = reduceMixer(state, tracks?.shelf ?? state.shelf, tracks?.song ?? clips?.song ?? state.song, command);
  if (mixer?.showParams && bands.side.at(-1) !== "params") {
    bands.side = [...bands.side.filter((b) => b !== "params"), "params"];
    ws.layout = bands.layout();
  }
  let song = mixer?.song ?? tracks?.song ?? clips?.song ?? state.song;
  if ((tracks || mixer) && JSON.stringify(song) !== JSON.stringify(state.song)) changes.push({ section: "song", value: song });
  if (command.type === "removeMissing") {
    song = withoutMissing(song, command.missing);
    if (JSON.stringify(song) !== JSON.stringify(state.song)) changes.push({ section: "song", value: song });
  }
  for (const c of changes) if (c.section === "song") song = c.value = settleSong(c.value as SongView);
  if (mixer && JSON.stringify(mixer.params) !== JSON.stringify(state.params)) changes.push({ section: "params", value: mixer.params });
  if (mixer && JSON.stringify(mixer.windows) !== JSON.stringify(state.windows)) changes.push({ section: "windows", value: mixer.windows });
  let shelf = structuredClone(mixer?.shelf ?? tracks?.shelf ?? state.shelf);
  reduceAdded(command, shelf, library, ws);
  // A closed shelf has no mode, audition or sample (protocol 13, ShelfView). Its stars and what is missing stay.
  if (!ws.layout.rightOpen.includes("shelf")) {
    const { available, favorites, favoriteSounds, favoriteEffects, missing } = shelf;
    shelf = { ...closedShelf(), available, favorites, favoriteSounds, favoriteEffects, missing };
  }
  shelf = settleShelf(shelf);
  if (JSON.stringify(shelf.selectedSound) !== JSON.stringify(state.shelf.selectedSound)) {
    transport.canRecord = shelf.selectedSound === null && song.playTarget !== null && transport.canPlay;
  }
  if (JSON.stringify(shelf) !== JSON.stringify(state.shelf)) changes.push({ section: "shelf", value: shelf });
  if (JSON.stringify(library) !== JSON.stringify(state.library)) changes.push({ section: "library", value: library });
  if (JSON.stringify(ws) !== JSON.stringify(state.workspace)) changes.push({ section: "workspace", value: ws });
  if (JSON.stringify(outputs) !== JSON.stringify(state.outputs)) changes.push({ section: "outputs", value: outputs });
  if (JSON.stringify(keyboards) !== JSON.stringify(state.keyboards)) changes.push({ section: "keyboards", value: keyboards });
  if (JSON.stringify(transport) !== JSON.stringify(state.transport)) changes.push({ section: "transport", value: transport });
  if (JSON.stringify(pianoRoll) !== JSON.stringify(state.pianoRoll)) changes.push({ section: "pianoRoll", value: pianoRoll });
  if (JSON.stringify(plugins) !== JSON.stringify(state.plugins)) changes.push({ section: "plugins", value: plugins });
  // Scenes set the rest of the status line by hand; only the notices and the selected notes move here.
  let message =
    notice ??
    mixer?.notice ??
    clips?.notice ??
    tracks?.notice ??
    shelfMessage(shelf, song, library) ??
    (isNotice(state.status.message) || isShelfMessage(state.status.message) ? rest : state.status.message);
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
  const rest = isNotice(state.status.message) || isShelfMessage(state.status.message) ? null : state.status.message;
  const clipboard: FakeClipboard = { clips: [] };
  const tables = new FakeParamTables();
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
      const changes = reduce(state, bands, rest, command, clipboard, nowUs());
      // The fake plugins' parameters: the value and its text come back on the messages Channel; a typed one may be put at an end.
      let values: ParamValues | null = null;
      if (command.type === "setPluginParam") values = tables.set(command.table, command.param, command.value);
      if (command.type === "typeParam" && command.param.kind === "plugin") {
        const p = command.param;
        const typed = tables.type(p.table, p.param, command.text);
        values = typed.values;
        const said: StatusMessage | null = typed.rejected !== null
          ? { id: "paramRejected", param: p, kept: typed.rejected }
          : typed.clamped
            ? { id: "paramClamped", to: { kind: "plugin", ...typed.clamped } }
            : null;
        if (said) {
          const at = changes.findIndex((c) => c.section === "status");
          if (at >= 0) changes.splice(at, 1);
          changes.push({ section: "status", value: { message: said } });
        }
      }
      if (changes.length > 0) {
        rev += 1;
        state = { ...state };
        for (const c of changes) (state as Record<string, unknown>)[c.section] = c.value;
        listener?.({ kind: "patch", rev, changes });
      }
      if (values) listener?.({ kind: "paramValues", ...values });
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
    async pluginParams(table) {
      return tables.table(table);
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

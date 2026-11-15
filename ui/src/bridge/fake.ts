// A Transport with no Rust behind it: for tests, and for looking at the screen in a plain
// browser (`npm run dev`, `vite preview`). `?scene=<name>&scale=<100|125|150>` picks a fixed
// scene (see SCENES). It answers the few commands below; the band heights follow the same
// numbers as crates/meloet-core/src/layout.rs, but meloet-core stays the rule book.

import type { Transport } from "./session";
import { TELEMETRY_KIND, TELEMETRY_VERSION } from "./telemetry";
import {
  PROTOCOL_VERSION,
  type AppState,
  type Band,
  type Change,
  type Command,
  type EndpointView,
  type LayoutView,
  type ServerMessage,
  type StatusMessage,
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
   * Returns the band that was folded because the opened one did not fit beside it. A band folded
   * only because it would be the third is not reported.
   */
  toggle(band: Band): Band | null {
    if (isBottom(band)) {
      const at = this.bottom.indexOf(band);
      if (at >= 0) this.bottom.splice(at, 1);
      else {
        this.bottom.push(band);
        if (this.bottom.length > MAX_BOTTOM_OPEN) this.bottom.shift();
        return this.fit();
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
      noteCount: 0,
      tracks: [{ number: 1, name: "ピアノ", color: 1, instrument: "placeholder", gainDb: 0 }],
      playTarget: 1,
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
    status: { message: { id: "newSong" } },
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
}

interface Scene {
  state: AppState;
  bands: FakeBands;
  meters: FakeMeters;
  /** Keep every scrolling box (`data-gl-scroll`) scrolled to its end. */
  scrollToEnd?: boolean;
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
  s.status.message = { id: "newSong" };
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
      s.status.message = { id: "outputChosen", output: HEADPHONES.name };
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
  empty: { setup: ready, meters: QUIET },
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
  // The mock "拡大率をえらぶ所": a 1200x700 screen, 150 % does not fit, 125 % is on.
  zoom: { setup: ready, scale: 125, screen: { width: 1200, height: 700 }, meters: QUIET },
  // The mixer was open, then the piano roll was opened in a window too low for both.
  bandFolded: {
    setup: ready,
    bands: ["mixer", "pianoRoll"],
    after: (s) => (s.status.message = { id: "bandFoldedToOpen", opened: "pianoRoll", folded: "mixer" }),
    meters: QUIET,
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
  return { state, bands, meters: def.meters, scrollToEnd: def.scrollToEnd };
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

/** Writes the telemetry binary (docs/architecture.md 3-3). */
export function encodeTelemetry(m: FakeMeters, seq: number, sentUs: number, trackCount: number): ArrayBuffer {
  const buf = new ArrayBuffer(48 + 8 * trackCount);
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
  v.setUint8(35, m.master && (m.master[0] > 0 || m.master[1] > 0) ? 1 : 0);
  v.setInt16(36, m.pitchBend ?? 0, true);
  v.setUint16(38, m.loadPermille ?? 0, true);
  v.setFloat32(40, m.master?.[0] ?? 0, true);
  v.setFloat32(44, m.master?.[1] ?? 0, true);
  for (let i = 0; i < trackCount; i++) {
    const [l, r] = m.tracks?.[i] ?? [0, 0];
    v.setFloat32(48 + 8 * i, l, true);
    v.setFloat32(52 + 8 * i, r, true);
  }
  return buf;
}

// ---- commands ----

/** The status messages that tell what just happened. The next thing the user does takes them away. */
const NOTICES: StatusMessage["id"][] = [
  "bandFoldedWindowLow",
  "bandFoldedToOpen",
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
  const fitWindow = () => {
    const folded = bands.setHeight((ws.viewportHeight * 100) / ws.scale);
    if (folded) notice = { id: "bandFoldedWindowLow", band: folded };
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
      fitWindow();
      break;
    }
    case "toggleBand": {
      const folded = bands.toggle(command.band);
      if (folded) notice = { id: "bandFoldedToOpen", opened: command.band, folded };
      break;
    }
    case "goArrange":
      bands.goArrange();
      break;
    case "back":
      if (ws.settings) ws.settings = null;
      else bands.back();
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
          onTelemetry(encodeTelemetry(scene.meters, ++seq, nowUs(), state.song.tracks.length));
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
  };
}

/** The fake has nothing to measure. */
export async function fakeMeasureEnabled(): Promise<boolean> {
  return false;
}

export async function fakeMeasureReport(_name: string, _json: string): Promise<void> {}

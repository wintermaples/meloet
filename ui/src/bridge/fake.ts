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
} from "./types";

// ---- the band rules (a copy of the numbers in meloet-core/src/layout.rs) ----

const TRANSPORT = 48;
const BAND_ROW = 24;
const STATUS = 24;
const METER_FULL = 52;
const METER_SLIM = 24;
const OVERVIEW = 40;
const SMALLEST: Partial<Record<Band, number>> = { pianoRoll: 268, mixer: 516, history: 248 };
const BAND_TOOLS = 32;
const TALL_WINDOW = 960;
const MAX_BOTTOM_OPEN = 2;
const BOTTOM_ORDER: Band[] = ["pianoRoll", "mixer", "history"];
const isBottom = (b: Band) => BOTTOM_ORDER.includes(b);

export class FakeBands {
  /** Oldest first. */
  bottom: Band[] = [];
  /** Oldest first; the last one is in front. */
  side: Band[] = [];

  /** Window height in px at 100 %. */
  constructor(public height: number) {}

  private meterHeight(): number {
    return this.height >= TALL_WINDOW || this.bottom.length === 0 ? METER_FULL : METER_SLIM;
  }

  private rest(): number {
    return this.height - TRANSPORT - this.meterHeight() - BAND_ROW - STATUS;
  }

  /** What is left with the arrange view at 40 and every open band at its smallest. */
  private spare(): number {
    return this.rest() - OVERVIEW - this.bottom.reduce((sum, b) => sum + (SMALLEST[b] ?? 0), 0);
  }

  /** The band that grows with the window; null is the arrange view. */
  private spareTaker(): Band | null {
    if (this.bottom.includes("pianoRoll")) return "pianoRoll";
    if (this.bottom.includes("mixer") && this.height < TALL_WINDOW) return "mixer";
    return null;
  }

  private fits(): boolean {
    return this.spare() >= 0;
  }

  fit(): void {
    while (this.bottom.length > 1 && !this.fits()) this.bottom.shift();
  }

  setHeight(height: number): void {
    this.height = height;
    this.fit();
  }

  toggle(band: Band): void {
    if (isBottom(band)) {
      const at = this.bottom.indexOf(band);
      if (at >= 0) this.bottom.splice(at, 1);
      else {
        this.bottom.push(band);
        if (this.bottom.length > MAX_BOTTOM_OPEN) this.bottom.shift();
        this.fit();
      }
    } else if (this.side.at(-1) === band) {
      this.side.pop();
    } else {
      this.side = [...this.side.filter((b) => b !== band), band];
    }
  }

  goArrange(): void {
    this.bottom = [];
  }

  back(): void {
    if (this.side.pop() === undefined) this.bottom.pop();
  }

  layout(): LayoutView {
    const spare = this.spare();
    const taker = this.spareTaker();
    const alone = this.bottom.length === 1;
    const arrangeHeight = OVERVIEW + (taker === null ? Math.max(0, spare) : 0);
    return {
      meter: this.meterHeight() === METER_FULL ? "full" : "slim",
      arrangeHeight,
      arrangeOverview: this.bottom.includes("pianoRoll") || arrangeHeight <= OVERVIEW,
      lower: BOTTOM_ORDER.filter((b) => this.bottom.includes(b)).map((band) => ({
        band,
        height: Math.max(BAND_TOOLS, (SMALLEST[band] ?? 0) + (taker === band || (alone && spare < 0) ? spare : 0)),
      })),
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
      layout: bands.layout(),
      settings: null,
      chooser: null,
      hint: null,
    },
    keyboards: { list: [], active: null, lost: null },
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

const dbfs = (db: number) => 10 ** (db / 20);

function withKeyboard(s: AppState, name = "Keystation 49 MK3"): void {
  const kb = keyboard(name);
  s.keyboards = { list: [kb], active: kb.id, lost: null };
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
  s.workspace.settings = { draftOutput: HEADPHONES.id, advancedOpen: false };
  s.status.message = { id: "settingsOpen" };
}

function settingsAdvFixed(s: AppState): void {
  bufferFixed(s);
  s.outputs.list = PC_OUTPUTS.map((o) => ({ ...o }));
  s.workspace.settings = { draftOutput: HEADPHONES.id, advancedOpen: true };
}

const QUIET: FakeMeters = { velocity: 83, loadPermille: 90 };

export const SCENES: Record<
  string,
  { setup(s: AppState): void; bands?: Band[]; meters: FakeMeters; scrollToEnd?: boolean }
> = {
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
};

export function fakeScene(name: string, scale = 100, viewport = { width: 1280, height: 800 }): Scene | null {
  const def = SCENES[name];
  if (!def) return null;
  const state = fakeState();
  const s = [100, 125, 150].includes(scale) ? scale : 100;
  const bands = new FakeBands((viewport.height * 100) / s);
  for (const b of def.bands ?? []) bands.toggle(b);
  def.setup(state);
  state.workspace.scale = s;
  state.workspace.viewportWidth = viewport.width;
  state.workspace.viewportHeight = viewport.height;
  state.workspace.layout = bands.layout();
  return { state, bands, meters: def.meters, scrollToEnd: def.scrollToEnd };
}

function sceneFromUrl(): Scene | null {
  if (typeof location === "undefined") return null;
  const q = new URLSearchParams(location.search);
  const name = q.get("scene");
  if (!name) return null;
  const vp = typeof window === "undefined" ? undefined : { width: window.innerWidth, height: window.innerHeight };
  return fakeScene(name, Number(q.get("scale") ?? 100), vp);
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

function reduce(state: AppState, bands: FakeBands, command: Command): Change[] {
  const ws = structuredClone(state.workspace);
  const outputs = structuredClone(state.outputs);
  const choosable = outputs.list.filter((o) => o.unusable === null).map((o) => o.id);
  switch (command.type) {
    case "setViewport":
      ws.viewportWidth = command.width;
      ws.viewportHeight = command.height;
      bands.setHeight((command.height * 100) / ws.scale);
      break;
    case "setScale":
      if ([100, 125, 150].includes(command.percent)) ws.scale = command.percent;
      bands.setHeight((ws.viewportHeight * 100) / ws.scale);
      break;
    case "toggleBand":
      bands.toggle(command.band);
      break;
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
  return changes;
}

/** With no `initial`, the scene comes from the URL (`?scene=`), or else the plain fakeState(). */
export function createFakeTransport(initial?: AppState): Transport {
  const scene = initial ? null : sceneFromUrl();
  let state = initial ?? scene?.state ?? fakeState();
  const bands = scene?.bands ?? new FakeBands((state.workspace.viewportHeight * 100) / state.workspace.scale);
  if (!scene) for (const l of state.workspace.layout.lower) bands.bottom.push(l.band);
  if (!scene) bands.side = [...state.workspace.layout.rightOpen];
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
      const changes = reduce(state, bands, command);
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

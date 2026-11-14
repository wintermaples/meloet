// A Transport with no Rust behind it: for tests, and for looking at the screen in a plain
// browser (`npm run dev`). It keeps a fixed state and answers the few commands below.
// It is not the rule book; the band rules live in meloet-core.

import type { Transport } from "./session";
import {
  PROTOCOL_VERSION,
  type AppState,
  type Command,
  type ServerMessage,
  type WorkspaceView,
} from "./types";

export function fakeState(): AppState {
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
      layout: {
        meter: "full",
        arrangeHeight: 800,
        arrangeOverview: false,
        lower: [],
        rightOpen: [],
        rightFront: null,
        showBackHint: false,
      },
      settings: null,
      chooser: null,
      hint: "emptyArrange",
    },
    keyboards: { list: [], active: null, lost: null },
    outputs: { list: [], chosen: null, chosenState: null },
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

function reduce(state: AppState, command: Command): WorkspaceView | null {
  const ws = structuredClone(state.workspace);
  switch (command.type) {
    case "setViewport":
      ws.viewportWidth = command.width;
      ws.viewportHeight = command.height;
      break;
    case "setScale":
      if ([100, 125, 150].includes(command.percent)) ws.scale = command.percent;
      break;
    case "toggleBand": {
      const { band } = command;
      if (band === "pianoRoll" || band === "mixer") {
        const at = ws.layout.lower.findIndex((b) => b.band === band);
        if (at >= 0) ws.layout.lower.splice(at, 1);
        else ws.layout.lower.push({ band, height: 240 });
      } else {
        const at = ws.layout.rightOpen.indexOf(band);
        if (at >= 0) ws.layout.rightOpen.splice(at, 1);
        else ws.layout.rightOpen.push(band);
        ws.layout.rightFront = ws.layout.rightOpen.at(-1) ?? null;
      }
      break;
    }
    case "goArrange":
      ws.layout.lower = [];
      break;
    case "openSettings":
      ws.settings ??= { draftOutput: null, advancedOpen: false };
      break;
    case "settingsApply":
    case "settingsCancel":
      ws.settings = null;
      break;
    case "dismissHint":
      ws.hint = null;
      break;
    default:
      break;
  }
  return JSON.stringify(ws) === JSON.stringify(state.workspace) ? null : ws;
}

export function createFakeTransport(initial: AppState = fakeState()): Transport {
  let state = initial;
  let rev = 1;
  let listener: ((message: ServerMessage) => void) | null = null;
  const start = performance.now();

  return {
    async hello(protocolVersion) {
      return {
        protocolVersion: PROTOCOL_VERSION,
        ok: protocolVersion === PROTOCOL_VERSION,
        monoUs: Math.round((performance.now() - start) * 1000),
      };
    },
    async subscribe(onMessage) {
      listener = onMessage;
    },
    async snapshot() {
      return { rev, state };
    },
    async send(command) {
      const workspace = reduce(state, command);
      if (workspace) {
        rev += 1;
        state = { ...state, workspace };
        listener?.({ kind: "patch", rev, changes: [{ section: "workspace", value: workspace }] });
      }
      return { rev };
    },
    async clock() {
      return Math.round((performance.now() - start) * 1000);
    },
  };
}

// The only door between the screen and Rust. Everything outside ui/src/bridge imports from here.

import { createFakeTransport, fakeMeasureEnabled, fakeMeasureReport, fakeUiScript } from "./fake";
import { LiveNotesBook } from "./live";
import { ParamTableBook } from "./paramTable";
import { connectWith, type Connection, type Handlers, type Transport } from "./session";
import { songTakeIds, TakeStore, type TakeNotes } from "./take";
import { tauriMeasureEnabled, tauriMeasureReport, tauriTransport, tauriUiScript } from "./tauri";
import type { AppState, Command } from "./types";

export type * from "./types";
export { DRUM_ROWS, INSTRUMENT_GROUPS, PROTOCOL_VERSION } from "./types";
export { keyIsDown, positionAt, type Telemetry, type TransportReading } from "./telemetry";
export { CONTROL_CC, CONTROL_PITCH_BEND, NO_FROM, type TakeNotes } from "./take";
export type { LiveNotesBook } from "./live";
export type { ParamTableBook, ParamTableListener } from "./paramTable";
export { ProtocolVersionError, type Connection, type Handlers } from "./session";

let connection: Connection | null = null;

function pickTransport(): Transport {
  return "__TAURI_INTERNALS__" in window ? tauriTransport : createFakeTransport();
}

const takes = new TakeStore((id) => connected().takeNotes(id));
let book: LiveNotesBook | null = null;
let params: ParamTableBook | null = null;

/** hello → subscribe → snapshot. Rejects when the protocol versions differ. */
export async function connect(handlers: Handlers): Promise<Connection> {
  const live = new LiveNotesBook(
    () => connected().liveNotes(),
    (error) => handlers.onError?.(error),
  );
  const table = new ParamTableBook(
    (id) => connected().pluginParams(id),
    (error) => handlers.onError?.(error),
  );
  params = table;
  let song: AppState["song"] | null = null;
  let serial: number | null = null;
  connection = await connectWith(pickTransport(), {
    ...handlers,
    onState(state, rev) {
      // The song section is replaced whole when it changes; that is when takes may stop being used.
      if (state.song !== song) {
        song = state.song;
        if (song.serial !== serial) {
          serial = song.serial;
          takes.clear();
        }
        takes.keepOnly(songTakeIds(song));
      }
      const shown = state.params.selected;
      table.show(shown?.kind === "plugin" ? shown.table : null);
      handlers.onState(state, rev);
    },
    onParamValues(values) {
      table.receive(values);
      handlers.onParamValues?.(values);
    },
    onLiveNotes(notes) {
      live.receive(notes);
      handlers.onLiveNotes?.(notes);
    },
  });
  book = live;
  live.start();
  return connection;
}

/** The notes of a take, fetched once and kept by id. */
export function takeNotes(id: number): Promise<TakeNotes> {
  return takes.get(id);
}

/** The notes of a take if they are here already. */
export function peekTakeNotes(id: number): TakeNotes | undefined {
  return takes.peek(id);
}

/** The notes of the recording going on. */
export function liveNotes(): LiveNotesBook {
  if (!book) throw new Error("bridge: connect() has not finished");
  return book;
}

/** The parameters of the plugin the parameters panel shows. */
export function paramTable(): ParamTableBook {
  if (!params) throw new Error("bridge: connect() has not started");
  return params;
}

function connected(): Connection {
  if (!connection) throw new Error("bridge: connect() has not finished");
  return connection;
}

/** Sends an intent. Resolves to the rev after it. */
export async function send(command: Command): Promise<number> {
  return connected().send(command);
}

/** `rustMonoUs ≈ performance.now() * 1000 + offset`. */
export async function clockOffsetUs(): Promise<number> {
  return connected().clockOffsetUs();
}

const inTauri = () => "__TAURI_INTERNALS__" in window;

/** Development only: whether to measure key → screen. Does not need connect(). */
export function measureEnabled(): Promise<boolean> {
  return inTauri() ? tauriMeasureEnabled() : fakeMeasureEnabled();
}

/** Development only: asks Rust to write a measurement file. */
export function measureReport(name: string, json: string): Promise<void> {
  return inTauri() ? tauriMeasureReport(name, json) : fakeMeasureReport(name, json);
}

/** Development only: the input script to play while measuring (ui/src/dev/script.ts), or null. */
export function uiScript(): Promise<string | null> {
  return inTauri() ? tauriUiScript() : fakeUiScript();
}

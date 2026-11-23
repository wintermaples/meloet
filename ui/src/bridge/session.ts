// The connection rules of docs/architecture.md 3-1. Knows nothing about Tauri: it talks to a
// Transport, so the same rules run against the real shell, the fake, and the tests.

import { decodeTelemetry, type Telemetry } from "./telemetry";
import {
  PROTOCOL_VERSION,
  type AppState,
  type Command,
  type Event,
  type HelloReply,
  type HistoryRow,
  type LiveNotes,
  type LiveNotesSnapshot,
  type ParamTable,
  type ParamValues,
  type Patch,
  type SendReply,
  type ServerMessage,
  type Snapshot,
} from "./types";

export interface Transport {
  hello(protocolVersion: number): Promise<HelloReply>;
  /** Resolves once the other side is sending. Messages may arrive before it resolves. */
  subscribe(
    onMessage: (message: ServerMessage) => void,
    onTelemetry: (bytes: ArrayBuffer) => void,
  ): Promise<void>;
  snapshot(): Promise<Snapshot>;
  send(command: Command): Promise<SendReply>;
  /** As send, but Rust does not answer when it is done: resolves once Rust has it, in the order sent. */
  post(command: Command): Promise<void>;
  /** Rust's CLOCK_MONOTONIC in microseconds. */
  clock(): Promise<number>;
  /** The take binary (docs/architecture.md 3-4). */
  takeNotes(id: number): Promise<ArrayBuffer>;
  /** The points of a lane (`LaneView.laneId`), binary. Rejects when `laneId` is no lane's. */
  lane(laneId: number): Promise<ArrayBuffer>;
  /**
   * The peaks binary (docs/architecture.md 22-17): the peaks `first .. first + count` of `level` of an audio
   * file that there are. Rejects when `audio` is no file's, and while its peaks are not made.
   */
  peaks(audio: number, level: number, first: number, count: number): Promise<ArrayBuffer>;
  /** The rows `from`..`to` of the history band that there are, oldest first; rows count from 1. */
  historyRows(from: number, to: number): Promise<HistoryRow[]>;
  liveNotes(): Promise<LiveNotesSnapshot>;
  /** The parameters of the plugin in the parameters panel; null when `table` names them no more. */
  pluginParams(table: number): Promise<ParamTable | null>;
}

export interface Handlers {
  /** The whole state after a snapshot or a patch. Always a new object; sections that did not change keep their identity. */
  onState(state: AppState, rev: number): void;
  /** Key on/off, pedal, bend: called the moment the message arrives. */
  onEvent?(event: Event): void;
  /** Notes of the recording going on, in Channel order. */
  onLiveNotes?(notes: LiveNotes): void;
  /** Values of the plugin in the parameters panel that changed, in Channel order. */
  onParamValues?(values: ParamValues): void;
  onTelemetry?(telemetry: Telemetry): void;
  /** A snapshot that failed after connect, or telemetry that could not be read. */
  onError?(error: unknown): void;
}

export interface Connection {
  /** Sends an intent. Resolves to the rev after it; the new state arrives through onState. */
  send(command: Command): Promise<number>;
  /** Sends an intent without waiting for Rust to be done with it (the keys of the PC keyboard). */
  post(command: Command): Promise<void>;
  /** `rustMonoUs ≈ performance.now() * 1000 + offset`. */
  clockOffsetUs(rounds?: number): Promise<number>;
  rev(): number;
  takeNotes(id: number): Promise<ArrayBuffer>;
  lane(laneId: number): Promise<ArrayBuffer>;
  peaks(audio: number, level: number, first: number, count: number): Promise<ArrayBuffer>;
  historyRows(from: number, to: number): Promise<HistoryRow[]>;
  liveNotes(): Promise<LiveNotesSnapshot>;
  pluginParams(table: number): Promise<ParamTable | null>;
}

export class ProtocolVersionError extends Error {
  constructor(
    readonly ours: number,
    readonly theirs: number,
  ) {
    super(`protocol version mismatch: screen ${ours}, Rust ${theirs}`);
    this.name = "ProtocolVersionError";
  }
}

export interface SessionOptions {
  /** Milliseconds, like performance.now. */
  now?: () => number;
}

export async function connectWith(
  transport: Transport,
  handlers: Handlers,
  options: SessionOptions = {},
): Promise<Connection> {
  const now = options.now ?? (() => performance.now());

  const reply = await transport.hello(PROTOCOL_VERSION);
  if (!reply.ok || reply.protocolVersion !== PROTOCOL_VERSION) {
    throw new ProtocolVersionError(PROTOCOL_VERSION, reply.protocolVersion);
  }

  let current: Snapshot | null = null;
  // Patches that arrived while a snapshot was on its way.
  let waiting: Patch[] = [];
  let fetching = false;

  function applyPatch(base: Snapshot, patch: Patch): Snapshot {
    const state = { ...base.state };
    for (const change of patch.changes) {
      (state as Record<string, unknown>)[change.section] = change.value;
    }
    return { rev: patch.rev, state };
  }

  // Takes a snapshot, then the waiting patches above it in order. A gap among those means
  // something was lost on the way, so it takes another snapshot.
  async function sync(): Promise<void> {
    fetching = true;
    try {
      for (;;) {
        let next = await transport.snapshot();
        if (current && next.rev < current.rev) next = current;
        const queued = waiting;
        waiting = [];
        let gap = false;
        for (const patch of queued) {
          if (patch.rev <= next.rev) continue;
          if (patch.rev !== next.rev + 1) {
            gap = true;
            break;
          }
          next = applyPatch(next, patch);
        }
        const changed = current !== next;
        current = next;
        if (changed) handlers.onState(next.state, next.rev);
        if (!gap) return;
      }
    } finally {
      fetching = false;
    }
  }

  function onPatch(patch: Patch): void {
    if (fetching || !current) {
      waiting.push(patch);
      return;
    }
    if (patch.rev <= current.rev) return;
    if (patch.rev === current.rev + 1) {
      current = applyPatch(current, patch);
      handlers.onState(current.state, current.rev);
      return;
    }
    sync().catch((error) => handlers.onError?.(error));
  }

  function onMessage(message: ServerMessage): void {
    if (message.kind === "event") handlers.onEvent?.(message);
    else if (message.kind === "liveNotes") handlers.onLiveNotes?.(message);
    else if (message.kind === "paramValues") handlers.onParamValues?.(message);
    else onPatch(message);
  }

  function onTelemetry(bytes: ArrayBuffer): void {
    if (!handlers.onTelemetry) return;
    let telemetry: Telemetry;
    try {
      telemetry = decodeTelemetry(bytes);
    } catch (error) {
      handlers.onError?.(error);
      return;
    }
    handlers.onTelemetry(telemetry);
  }

  // Listen first, then take the whole state: nothing sent in between is lost.
  await transport.subscribe(onMessage, onTelemetry);
  await sync();

  return {
    async send(command) {
      return (await transport.send(command)).rev;
    },
    post: (command) => transport.post(command),
    async clockOffsetUs(rounds = 5) {
      // The shortest round trip has the least room for the reply to sit anywhere, so its
      // midpoint is the best guess of when Rust read its clock.
      let best = Infinity;
      let offset = 0;
      for (let i = 0; i < rounds; i++) {
        const t0 = now();
        const rustUs = await transport.clock();
        const t1 = now();
        if (t1 - t0 < best) {
          best = t1 - t0;
          offset = rustUs - ((t0 + t1) / 2) * 1000;
        }
      }
      return offset;
    },
    rev: () => current?.rev ?? 0,
    takeNotes: (id) => transport.takeNotes(id),
    lane: (laneId) => transport.lane(laneId),
    peaks: (audio, level, first, count) => transport.peaks(audio, level, first, count),
    historyRows: (from, to) => transport.historyRows(from, to),
    liveNotes: () => transport.liveNotes(),
    pluginParams: (table) => transport.pluginParams(table),
  };
}

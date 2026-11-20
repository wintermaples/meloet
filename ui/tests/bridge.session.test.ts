import { describe, expect, it } from "vitest";
import { createFakeTransport, fakeScene, fakeState } from "../src/bridge/fake";
import { ProtocolVersionError, connectWith, type Transport } from "../src/bridge/session";
import {
  PROTOCOL_VERSION,
  type AppState,
  type Patch,
  type ServerMessage,
  type Snapshot,
} from "../src/bridge/types";

interface Deferred<T> {
  promise: Promise<T>;
  resolve(value: T): void;
  reject(error: unknown): void;
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const tick = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

function withStatus(state: AppState, keyboard: string): AppState {
  return { ...state, status: { message: { id: "keyboardFound", keyboard } } };
}

function statusPatch(rev: number, keyboard: string): ServerMessage {
  const patch: Patch = {
    rev,
    changes: [{ section: "status", value: { message: { id: "keyboardFound", keyboard } } }],
  };
  return { kind: "patch", ...patch };
}

/** A Transport the test drives by hand: every snapshot waits until the test answers it. */
function scripted(version = PROTOCOL_VERSION) {
  const calls: string[] = [];
  const snapshots: Deferred<Snapshot>[] = [];
  let push: (message: ServerMessage) => void = () => {
    throw new Error("nobody subscribed yet");
  };
  let pushTelemetry: (bytes: ArrayBuffer) => void = () => {};
  let clockReplies: number[] = [];

  const transport: Transport = {
    async hello(protocolVersion) {
      calls.push("hello");
      return { protocolVersion: version, ok: protocolVersion === version, monoUs: 0 };
    },
    async subscribe(onMessage, onTelemetry) {
      calls.push("subscribe");
      push = onMessage;
      pushTelemetry = onTelemetry;
    },
    snapshot() {
      calls.push("snapshot");
      const d = deferred<Snapshot>();
      snapshots.push(d);
      return d.promise;
    },
    async send() {
      calls.push("send");
      return { rev: 0 };
    },
    async clock() {
      return clockReplies.shift() ?? 0;
    },
    async takeNotes() {
      return new ArrayBuffer(0);
    },
    async peaks() {
      return new ArrayBuffer(0);
    },
    async lane() {
      return new ArrayBuffer(0);
    },
    async historyRows() {
      return [];
    },
    async pluginParams() {
      return null;
    },
    async liveNotes() {
      return { seq: 0, recording: null, notes: [] };
    },
  };
  return {
    transport,
    calls,
    snapshots,
    push: (message: ServerMessage) => push(message),
    pushTelemetry: (bytes: ArrayBuffer) => pushTelemetry(bytes),
    setClockReplies: (replies: number[]) => (clockReplies = replies),
  };
}

function recorder() {
  const states: { rev: number; state: AppState }[] = [];
  const errors: unknown[] = [];
  return {
    states,
    errors,
    handlers: {
      onState: (state: AppState, rev: number) => states.push({ rev, state }),
      onError: (error: unknown) => errors.push(error),
    },
  };
}

describe("bridge session", () => {
  it("(a) listens first, then takes the snapshot", async () => {
    const s = scripted();
    const r = recorder();
    const connecting = connectWith(s.transport, r.handlers);
    await tick();
    expect(s.calls).toEqual(["hello", "subscribe", "snapshot"]);
    expect(r.states).toEqual([]);

    s.snapshots[0]!.resolve({ rev: 4, state: fakeState() });
    const connection = await connecting;
    expect(r.states).toEqual([{ rev: 4, state: fakeState() }]);
    expect(connection.rev()).toBe(4);
  });

  it("(b) patches that arrive before the snapshot: old ones are dropped, newer ones applied in order", async () => {
    const s = scripted();
    const r = recorder();
    const connecting = connectWith(s.transport, r.handlers);
    await tick();

    s.push(statusPatch(4, "old-4"));
    s.push(statusPatch(5, "old-5"));
    s.push(statusPatch(6, "new-6"));
    s.push(statusPatch(7, "new-7"));
    expect(r.states).toEqual([]);

    const base = withStatus(fakeState(), "snapshot-5");
    s.snapshots[0]!.resolve({ rev: 5, state: base });
    const connection = await connecting;

    expect(s.calls.filter((c) => c === "snapshot")).toHaveLength(1);
    expect(r.states).toEqual([{ rev: 7, state: withStatus(base, "new-7") }]);
    expect(connection.rev()).toBe(7);
  });

  it("(b) a gap among the patches that arrived before the snapshot takes another snapshot", async () => {
    const s = scripted();
    const r = recorder();
    const connecting = connectWith(s.transport, r.handlers);
    await tick();

    s.push(statusPatch(8, "eight"));
    s.snapshots[0]!.resolve({ rev: 5, state: fakeState() });
    await tick();
    expect(s.snapshots).toHaveLength(2);

    s.snapshots[1]!.resolve({ rev: 8, state: withStatus(fakeState(), "eight") });
    const connection = await connecting;
    expect(connection.rev()).toBe(8);
    expect(r.states.at(-1)).toEqual({ rev: 8, state: withStatus(fakeState(), "eight") });
  });

  it("applies a patch whose rev is exactly one more, replacing only that section", async () => {
    const s = scripted();
    const r = recorder();
    const connecting = connectWith(s.transport, r.handlers);
    await tick();
    const base = fakeState();
    s.snapshots[0]!.resolve({ rev: 1, state: base });
    await connecting;

    s.push(statusPatch(2, "Keystation 49 MK3"));
    expect(r.states).toHaveLength(2);
    const after = r.states[1]!;
    expect(after.rev).toBe(2);
    expect(after.state.status).toEqual({
      message: { id: "keyboardFound", keyboard: "Keystation 49 MK3" },
    });
    expect(after.state).not.toBe(base);
    expect(after.state.song).toBe(base.song);
    expect(base.status.message?.id).toBe("newSong");

    s.push(statusPatch(2, "again"));
    s.push(statusPatch(1, "older"));
    expect(r.states).toHaveLength(2);
    expect(s.snapshots).toHaveLength(1);
  });

  it("(c) a jump in rev takes the whole snapshot again", async () => {
    const s = scripted();
    const r = recorder();
    const connecting = connectWith(s.transport, r.handlers);
    await tick();
    s.snapshots[0]!.resolve({ rev: 1, state: fakeState() });
    const connection = await connecting;

    s.push(statusPatch(3, "three"));
    expect(r.states).toHaveLength(1);
    expect(s.snapshots).toHaveLength(2);

    // While the new snapshot is on its way, patches follow the same rule as at connect.
    s.push(statusPatch(4, "four"));
    s.push(statusPatch(5, "five"));
    expect(r.states).toHaveLength(1);
    expect(s.snapshots).toHaveLength(2);

    const fresh = withStatus(fakeState(), "snapshot-4");
    s.snapshots[1]!.resolve({ rev: 4, state: fresh });
    await tick();

    expect(r.states).toHaveLength(2);
    expect(r.states[1]).toEqual({ rev: 5, state: withStatus(fresh, "five") });
    expect(connection.rev()).toBe(5);

    s.push(statusPatch(6, "six"));
    expect(r.states).toHaveLength(3);
    expect(s.snapshots).toHaveLength(2);
  });

  it("(c) a snapshot that fails after connect is reported, and the next patch tries again", async () => {
    const s = scripted();
    const r = recorder();
    const connecting = connectWith(s.transport, r.handlers);
    await tick();
    s.snapshots[0]!.resolve({ rev: 1, state: fakeState() });
    const connection = await connecting;

    s.push(statusPatch(3, "three"));
    s.snapshots[1]!.reject(new Error("boom"));
    await tick();
    expect(r.errors).toHaveLength(1);
    expect(connection.rev()).toBe(1);

    s.push(statusPatch(4, "four"));
    expect(s.snapshots).toHaveLength(3);
    s.snapshots[2]!.resolve({ rev: 4, state: withStatus(fakeState(), "four") });
    await tick();
    expect(connection.rev()).toBe(4);
  });

  it("(e) connect fails when the versions differ, before it subscribes", async () => {
    const s = scripted(PROTOCOL_VERSION + 1);
    const r = recorder();
    await expect(connectWith(s.transport, r.handlers)).rejects.toBeInstanceOf(ProtocolVersionError);
    expect(s.calls).toEqual(["hello"]);
    expect(r.states).toEqual([]);
  });

  it("hands events over at once, even while the snapshot is on its way", async () => {
    const s = scripted();
    const r = recorder();
    const events: unknown[] = [];
    const connecting = connectWith(s.transport, { ...r.handlers, onEvent: (e) => events.push(e) });
    await tick();

    s.push({ kind: "event", type: "note", on: true, note: 60, velocity: 100, tUs: 123 });
    expect(events).toHaveLength(1);
    expect(events[0]).toMatchObject({ type: "note", on: true, note: 60, velocity: 100, tUs: 123 });

    s.snapshots[0]!.resolve({ rev: 1, state: fakeState() });
    await connecting;
  });

  it("reports telemetry it cannot read instead of throwing into the transport", async () => {
    const s = scripted();
    const r = recorder();
    const seen: unknown[] = [];
    const connecting = connectWith(s.transport, {
      ...r.handlers,
      onTelemetry: (t) => seen.push(t),
    });
    await tick();
    s.snapshots[0]!.resolve({ rev: 1, state: fakeState() });
    await connecting;

    s.pushTelemetry(new ArrayBuffer(3));
    expect(seen).toEqual([]);
    expect(r.errors).toHaveLength(1);
  });

  it("takes the clock offset from the shortest round trip", async () => {
    const s = scripted();
    const r = recorder();
    // Each round reads now() twice: [0, 10], [20, 22], [30, 38] ms. The 2 ms round wins.
    const times = [0, 10, 20, 22, 30, 38];
    const connecting = connectWith(s.transport, r.handlers, { now: () => times.shift()! });
    await tick();
    s.snapshots[0]!.resolve({ rev: 1, state: fakeState() });
    const connection = await connecting;

    s.setClockReplies([5_000_000, 5_021_500, 5_034_000]);
    expect(await connection.clockOffsetUs(3)).toBe(5_021_500 - 21_000);
  });

  it("the fake transport connects and answers a command with a patch", async () => {
    const r = recorder();
    const connection = await connectWith(createFakeTransport(), r.handlers);
    expect(r.states).toHaveLength(1);
    expect(r.states[0]!.state.song.name).toBe("はじめての曲");

    const rev = await connection.send({ type: "toggleBand", band: "pianoRoll" });
    expect(rev).toBe(2);
    expect(r.states).toHaveLength(2);
    // 800 − 48 − 24（入力計が細い）− 24 − 24 − 40（ちぢんだアレンジ）
    expect(r.states[1]!.state.workspace.layout.lower).toEqual([{ band: "pianoRoll", height: 616 }]);

    expect(await connection.send({ type: "setScale", percent: 100 })).toBe(2);
    expect(r.states).toHaveLength(2);
  });

  it("the fake transport tells what just happened, until the next thing the user does", async () => {
    const r = recorder();
    const state = fakeScene("zoom")!.state;
    const connection = await connectWith(createFakeTransport(state), r.handlers);
    const now = () => r.states.at(-1)!.state;

    await connection.send({ type: "setScale", percent: 150 });
    expect(now().workspace.scale).toBe(125);
    expect(now().status.message).toEqual({ id: "scaleDoesNotFit", requested: 150, kept: 125 });

    await connection.send({ type: "toggleBand", band: "mixer" });
    expect(now().status.message?.id).toBe("newSong");
    await connection.send({ type: "toggleBand", band: "pianoRoll" });
    expect(now().status.message).toEqual({ id: "bandFoldedToOpen", opened: "pianoRoll", folded: "mixer" });
    // A resize is not something the user does about the notice.
    await connection.send({ type: "setViewport", width: 1280, height: 900 });
    expect(now().status.message).toEqual({ id: "bandFoldedToOpen", opened: "pianoRoll", folded: "mixer" });

    // 853x448 is too small for 125 %: the window grows to 1067x560.
    await connection.send({ type: "setScale", percent: 100 });
    await connection.send({ type: "setViewport", width: 853, height: 448 });
    await connection.send({ type: "setScale", percent: 125 });
    expect(now().workspace).toMatchObject({ scale: 125, viewportWidth: 1067, viewportHeight: 560 });
    expect(now().status.message).toEqual({ id: "scaleWindowGrown", scale: 125 });
  });
});

import { describe, expect, it } from "vitest";
import { LiveNotesBook } from "../src/bridge/live";
import type { LiveNote, LiveNotes, LiveNotesSnapshot } from "../src/bridge/types";

const note = (index: number, length: number | null, lap = 0): LiveNote => ({
  lap,
  index,
  key: 60 + index,
  velocity: 90,
  start: 480 * index,
  length,
});
const msg = (seq: number, recording: number, notes: LiveNote[]): LiveNotes => ({ seq, recording, notes });
const flush = () => new Promise((r) => setTimeout(r, 0));

function book(replies: LiveNotesSnapshot[]) {
  const fetched: number[] = [];
  const errors: unknown[] = [];
  const b = new LiveNotesBook(
    async () => {
      const next = replies.shift();
      if (!next) throw new Error("no reply");
      fetched.push(next.seq);
      return next;
    },
    (e) => errors.push(e),
  );
  return { b, fetched, errors };
}

describe("LiveNotesBook", () => {
  it("keeps what came before the first fetch, and drops what the fetch already had", async () => {
    const { b, fetched } = book([{ seq: 3, recording: 1, notes: [note(0, 100), note(1, null)] }]);
    b.receive(msg(3, 1, [note(1, null)]));
    b.receive(msg(4, 1, [note(1, 200), note(2, null)]));
    b.start();
    await flush();
    expect(fetched).toEqual([3]);
    expect(b.recording()).toBe(1);
    expect(b.notesOf(1, 0).map((n) => [n.index, n.length])).toEqual([
      [0, 100],
      [1, 200],
      [2, null],
    ]);
  });

  it("fetches the whole list again when a number is skipped", async () => {
    const { b, fetched } = book([
      { seq: 1, recording: 1, notes: [] },
      { seq: 5, recording: 1, notes: [note(0, 100), note(1, 100)] },
    ]);
    b.start();
    await flush();
    let changes = 0;
    b.listen(() => changes++);
    b.receive(msg(2, 1, [note(0, 100)]));
    expect(changes).toBe(1);
    b.receive(msg(4, 1, [note(1, 100)]));
    await flush();
    expect(fetched).toEqual([1, 5]);
    expect(b.notesOf(1, 0)).toHaveLength(2);
    b.receive(msg(5, 1, [note(9, 1)]));
    expect(b.notesOf(1, 0)).toHaveLength(2);
  });

  it("hands out the same list until the notes change (it is read every frame)", async () => {
    const { b } = book([{ seq: 0, recording: null, notes: [] }]);
    b.start();
    await flush();
    b.receive(msg(1, 1, [note(0, 100), note(1, null)]));
    const first = b.notesOf(1, 0);
    expect(b.notesOf(1, 0)).toBe(first);
    b.receive(msg(2, 1, [note(1, 50)]));
    const second = b.notesOf(1, 0);
    expect(second).not.toBe(first);
    expect(second.map((n) => n.length)).toEqual([100, 50]);
    expect(first.map((n) => n.length)).toEqual([100, null]);
  });

  it("starts over with the next recording, and keeps laps apart", async () => {
    const { b } = book([{ seq: 0, recording: null, notes: [] }]);
    b.start();
    await flush();
    b.receive(msg(1, 1, [note(0, 100), note(0, null, 1)]));
    expect(b.notesOf(1, 0)).toHaveLength(1);
    expect(b.notesOf(1, 1)).toHaveLength(1);
    b.receive(msg(2, 2, [note(0, null)]));
    expect(b.recording()).toBe(2);
    expect(b.notesOf(1, 0)).toEqual([]);
    expect(b.notesOf(2, 0).map((n) => n.length)).toEqual([null]);
  });

  it("tries the first fetch again on the next message after it failed", async () => {
    const { b, fetched, errors } = book([]);
    b.start();
    await flush();
    expect(errors).toHaveLength(1);
    b.receive(msg(7, 1, [note(0, null)]));
    await flush();
    expect(errors).toHaveLength(2);
    expect(fetched).toEqual([]);
  });
});

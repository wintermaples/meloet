// Input rules of the note fields (noteFields.ts): reading and writing text, range, ↑ ↓, intents sent, Tab order.
import { describe, expect, it } from "vitest";
import {
  commitCommand,
  nextField,
  noteFieldSpec,
  parseField,
  parseKeyName,
  parseNoteLength,
  parseNotePosition,
  parseVelocity,
  rejectCommand,
  stepField,
} from "../src/components/pianoroll/noteFields";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import { take } from "./pianoroll-fixture";

const M44 = { num: 4, den: 4 };
const M68 = { num: 6, den: 8 };
const BAR = 4 * TPQ;
const at = (bar: number, beat: number, tick: number) => (bar - 1) * BAR + (beat - 1) * TPQ + tick;

describe("position 「3.1.0000」", () => {
  it("bar . beat . tick to a tick in the song. Beat and tick can be left out", () => {
    expect(parseNotePosition("3.1.0000", M44)).toBe(at(3, 1, 0));
    expect(parseNotePosition("3.2.0960", M44)).toBe(at(3, 2, TPQ / 2));
    expect(parseNotePosition("3.2", M44)).toBe(at(3, 2, 0));
    expect(parseNotePosition("3", M44)).toBe(at(3, 1, 0));
    expect(parseNotePosition("３.１.００００", M44)).toBe(at(3, 1, 0));
  });

  it("ticks ignore the digit count (a number of 1 to 4 digits is read as that many ticks)", () => {
    expect(parseNotePosition("3.1.0", M44)).toBe(at(3, 1, 0));
    expect(parseNotePosition("3.1.240", M44)).toBe(at(3, 1, 240));
    expect(parseNotePosition("3.1.0240", M44)).toBe(at(3, 1, 240));
    expect(parseNoteLength("1.5", M44)).toBe(TPQ + 5);
  });

  it("cannot read: beat outside the meter, tick of 1 beat or more, tick of 5 digits or more, bar 0, letters", () => {
    for (const s of ["3.5.0000", "3.1.1920", "3.1.00000", "0.1.0000", "3.0.0000", "abc", "", "3.1.0000.0", "-1"]) {
      expect(parseNotePosition(s, M44), s).toBeNull();
    }
  });

  it("6/8: 1 beat is an eighth note (TPQ / 2 ticks). The tick is the tick inside that beat as it is", () => {
    expect(parseNotePosition("1.2.0000", M68)).toBe(TPQ / 2);
    expect(parseNotePosition("1.1.0480", M68)).toBe(TPQ / 4);
    expect(parseNotePosition("1.1.0959", M68)).toBe(TPQ / 2 - 1);
    expect(parseNotePosition("1.1.0960", M68)).toBeNull();
    expect(parseNotePosition("1.6.0000", M68)).toBe((5 * TPQ) / 2);
    expect(parseNotePosition("1.7.0000", M68)).toBeNull();
  });
});

describe("length 「1.0960」", () => {
  it("beat . tick to ticks", () => {
    expect(parseNoteLength("1.0960", M44)).toBe((3 * TPQ) / 2);
    expect(parseNoteLength("0.1440", M44)).toBe((3 * TPQ) / 4);
    expect(parseNoteLength("2", M44)).toBe(2 * TPQ);
    expect(parseNoteLength("1.0480", M68)).toBe((3 * TPQ) / 4);
  });

  it("cannot read: tick of 5 digits or more, 1 beat or more, letters", () => {
    for (const s of ["1.00000", "1.1920", "x", "", "1.2.0000"]) expect(parseNoteLength(s, M44), s).toBeNull();
    expect(parseNoteLength("1.0960", M68)).toBeNull();
  });
});

describe("pitch 「D#4」", () => {
  it("C4 = 60. # and b, lowercase, octave −1", () => {
    expect(parseKeyName("D#4")).toBe(63);
    expect(parseKeyName("Eb4")).toBe(63);
    expect(parseKeyName("d#4")).toBe(63);
    expect(parseKeyName("C-1")).toBe(0);
    expect(parseKeyName("G9")).toBe(127);
    expect(parseKeyName("Ｃ４")).toBe(60);
  });

  it("cannot read: outside 0 to 127, unknown note names", () => {
    for (const s of ["G#9", "Cb-1", "H4", "C", "4", "C#10"]) expect(parseKeyName(s), s).toBeNull();
  });
});

describe("velocity", () => {
  it("integers only (the range is checked by the field's rule)", () => {
    expect(parseVelocity("96")).toBe(96);
    expect(parseVelocity("200")).toBe(200);
    expect(parseVelocity("9.5")).toBeNull();
    expect(parseVelocity("")).toBeNull();
  });
});

// The clip is bars 1 to 5 (0 to 4 * BAR). Notes are 3.1.0000 D4 0.1440 80, 3.1.1440 E4 0.1440 83, 3.2.0960 F4 0.0960 77.
const clip = { start: 0, end: 4 * BAR, takeOffset: 0 };
const notes = take([
  [at(3, 1, 0), (3 * TPQ) / 4, 62, 80],
  [at(3, 1, (3 * TPQ) / 4), (3 * TPQ) / 4, 64, 83],
  [at(3, 2, TPQ / 2), TPQ / 2, 65, 77],
]);

describe("fields of one note", () => {
  it("position: as far as the note's length fits in the clip. The range text is in display form", () => {
    const s = noteFieldSpec("position", notes, clip, [0], M44)!;
    expect(s.initial).toBe("3.1.0000");
    expect(s.range).toBe("1.1.0000〜4.4.0480");
    expect(parseField(s, "4.4.0480")).toBe(at(4, 4, TPQ / 4));
    expect(parseField(s, "4.4.0481")).toBeNull();
    expect(parseField(s, "5.1.0000")).toBeNull();
  });

  it("pitch: C-1 to G9", () => {
    const s = noteFieldSpec("key", notes, clip, [1], M44)!;
    expect(s.initial).toBe("E4");
    expect(s.range).toBe("C-1〜G9");
  });

  it("length: from 1 shown tick to the end of the clip", () => {
    const s = noteFieldSpec("length", notes, clip, [2], M44)!;
    expect(s.initial).toBe("0.0960");
    expect(s.range).toBe("0.0001〜6.0960");
    expect(parseField(s, "0.0000")).toBeNull();
    expect(parseField(s, "6.0961")).toBeNull();
  });

  it("velocity: 1 to 127. The range text is screen text (value.velocityRange), so null", () => {
    const s = noteFieldSpec("velocity", notes, clip, [0], M44)!;
    expect(s.initial).toBe("80");
    expect(s.range).toBeNull();
    expect(parseField(s, "200")).toBeNull();
    expect(parseField(s, "0")).toBeNull();
    expect(parseField(s, "127")).toBe(127);
  });

  it("position is in the song (take 0 is the clip's start + takeOffset)", () => {
    const s = noteFieldSpec("position", notes, { start: BAR, end: 5 * BAR, takeOffset: 0 }, [0], M44)!;
    expect(s.initial).toBe("4.1.0000");
    expect(s.range).toBe("2.1.0000〜5.4.0480");
  });
});

describe("fields of 2 or more notes (as SetNotesField means)", () => {
  const all = [0, 1, 2];

  it("position: the earliest position. As far as the whole group fits in the clip", () => {
    const s = noteFieldSpec("position", notes, clip, all, M44)!;
    expect(s.initial).toBe("3.1.0000");
    expect(s.range).toBe("1.1.0000〜4.3.0000");
  });

  it("pitch: the lowest pitch. As far as the highest does not go over G9", () => {
    const s = noteFieldSpec("key", notes, clip, all, M44)!;
    expect(s.initial).toBe("D4");
    expect(s.range).toBe("C-1〜E9");
  });

  it("length: if mixed, holds the length of the earliest note. Changes for any value", () => {
    const s = noteFieldSpec("length", notes, clip, all, M44)!;
    expect(s.initial).toBe("0.1440");
    expect(s.current).toBeNull();
    expect(s.range).toBe("0.0001〜6.0960");
    expect(commitCommand(s, (3 * TPQ) / 4, 12, all)).toEqual({ type: "setNotesField", take: 12, notes: all, field: "length", value: (3 * TPQ) / 4 });
  });

  it("velocity: if mixed, holds the smallest value", () => {
    const s = noteFieldSpec("velocity", notes, clip, all, M44)!;
    expect(s.initial).toBe("77");
    expect(s.current).toBeNull();
  });
});

describe("↑ ↓ and intents sent", () => {
  it("position and length step by the lowest shown digit (1 tick). Never out of range", () => {
    const s = noteFieldSpec("position", notes, clip, [0], M44)!;
    expect(s.format(stepField(s, at(3, 1, 0), 1))).toBe("3.1.0001");
    expect(s.format(stepField(s, at(3, 1, 0), -10))).toBe("2.4.1910");
    expect(s.format(stepField(s, 5, -10))).toBe("1.1.0000");
    const l = noteFieldSpec("length", notes, clip, [0], M44)!;
    expect(l.format(stepField(l, 1, -1))).toBe("0.0001");
  });

  it("pitch by a semitone, velocity by 1. Stops at 127", () => {
    const k = noteFieldSpec("key", notes, clip, [0], M44)!;
    expect(k.format(stepField(k, 62, 1))).toBe("D#4");
    const v = noteFieldSpec("velocity", notes, clip, [0], M44)!;
    expect(stepField(v, 120, 10)).toBe(127);
  });

  it("if the value does not change, nothing is sent (no undo step). If it changes, setNotesField", () => {
    const s = noteFieldSpec("position", notes, clip, [1], M44)!;
    expect(commitCommand(s, at(3, 1, (3 * TPQ) / 4), 12, [1])).toBeNull();
    expect(commitCommand(s, at(2, 1, 0), 12, [1])).toEqual({ type: "setNotesField", take: 12, notes: [1], field: "start", value: at(2, 1, 0) });
    const k = noteFieldSpec("key", notes, clip, [1], M44)!;
    expect(commitCommand(k, 63, 12, [1])).toEqual({ type: "setNotesField", take: 12, notes: [1], field: "key", value: 63 });
  });

  it("a value that cannot be used: the field, the old value, the range text (if mixed, the text shown in the field)", () => {
    const p = noteFieldSpec("position", notes, clip, [0], M44)!;
    expect(rejectCommand(p, "3.1.0000")).toEqual({ type: "rejectValue", field: "position", kept: "3.1.0000", range: "1.1.0000〜4.4.0480" });
    const v = noteFieldSpec("velocity", notes, clip, [0, 1, 2], M44)!;
    expect(rejectCommand(v, "77 – 83")).toEqual({ type: "rejectValue", field: "velocity", kept: "77 – 83", range: null });
  });
});

describe("Tab order", () => {
  it("position -> pitch -> length -> Vel -> position of the next row. Shift Tab goes back", () => {
    expect(nextField("position", false)).toEqual({ name: "key", row: 0 });
    expect(nextField("length", false)).toEqual({ name: "velocity", row: 0 });
    expect(nextField("velocity", false)).toEqual({ name: "position", row: 1 });
    expect(nextField("position", true)).toEqual({ name: "velocity", row: -1 });
  });
});

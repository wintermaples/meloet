// ノートの欄の打ちこみの決まり（noteFields.ts）: 文字の読み書き、範囲、↑ ↓、送る意図、Tab の順。
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

describe("位置「3.1.0000」", () => {
  it("小節 . 拍 . ティック を曲の中のティックに。拍とティックは省ける", () => {
    expect(parseNotePosition("3.1.0000", M44)).toBe(at(3, 1, 0));
    expect(parseNotePosition("3.2.0960", M44)).toBe(at(3, 2, TPQ / 2));
    expect(parseNotePosition("3.2", M44)).toBe(at(3, 2, 0));
    expect(parseNotePosition("3", M44)).toBe(at(3, 1, 0));
    expect(parseNotePosition("３.１.００００", M44)).toBe(at(3, 1, 0));
  });

  it("ティックはけた数を見ない（1〜4 けたの数を、そのままティックの数として読む）", () => {
    expect(parseNotePosition("3.1.0", M44)).toBe(at(3, 1, 0));
    expect(parseNotePosition("3.1.240", M44)).toBe(at(3, 1, 240));
    expect(parseNotePosition("3.1.0240", M44)).toBe(at(3, 1, 240));
    expect(parseNoteLength("1.5", M44)).toBe(TPQ + 5);
  });

  it("読めない: 拍が拍子の外、ティックが 1 拍以上、ティックが 5 けた以上、0 小節、文字", () => {
    for (const s of ["3.5.0000", "3.1.1920", "3.1.00000", "0.1.0000", "3.0.0000", "abc", "", "3.1.0000.0", "-1"]) {
      expect(parseNotePosition(s, M44), s).toBeNull();
    }
  });

  it("6/8: 1 拍は 8 分音符（TPQ / 2 ティック）。ティックはその拍の中のティックそのまま", () => {
    expect(parseNotePosition("1.2.0000", M68)).toBe(TPQ / 2);
    expect(parseNotePosition("1.1.0480", M68)).toBe(TPQ / 4);
    expect(parseNotePosition("1.1.0959", M68)).toBe(TPQ / 2 - 1);
    expect(parseNotePosition("1.1.0960", M68)).toBeNull();
    expect(parseNotePosition("1.6.0000", M68)).toBe((5 * TPQ) / 2);
    expect(parseNotePosition("1.7.0000", M68)).toBeNull();
  });
});

describe("長さ「1.0960」", () => {
  it("拍 . ティック をティックに", () => {
    expect(parseNoteLength("1.0960", M44)).toBe((3 * TPQ) / 2);
    expect(parseNoteLength("0.1440", M44)).toBe((3 * TPQ) / 4);
    expect(parseNoteLength("2", M44)).toBe(2 * TPQ);
    expect(parseNoteLength("1.0480", M68)).toBe((3 * TPQ) / 4);
  });

  it("読めない: ティックが 5 けた以上、1 拍以上、文字", () => {
    for (const s of ["1.00000", "1.1920", "x", "", "1.2.0000"]) expect(parseNoteLength(s, M44), s).toBeNull();
    expect(parseNoteLength("1.0960", M68)).toBeNull();
  });
});

describe("音「D#4」", () => {
  it("C4 = 60。# と b、小文字、−1 オクターブ", () => {
    expect(parseKeyName("D#4")).toBe(63);
    expect(parseKeyName("Eb4")).toBe(63);
    expect(parseKeyName("d#4")).toBe(63);
    expect(parseKeyName("C-1")).toBe(0);
    expect(parseKeyName("G9")).toBe(127);
    expect(parseKeyName("Ｃ４")).toBe(60);
  });

  it("読めない: 0〜127 の外、知らない音名", () => {
    for (const s of ["G#9", "Cb-1", "H4", "C", "4", "C#10"]) expect(parseKeyName(s), s).toBeNull();
  });
});

describe("ベロシティ", () => {
  it("整数だけ（範囲は欄の決まりで見る）", () => {
    expect(parseVelocity("96")).toBe(96);
    expect(parseVelocity("200")).toBe(200);
    expect(parseVelocity("9.5")).toBeNull();
    expect(parseVelocity("")).toBeNull();
  });
});

// クリップは 1〜5 小節（0〜4 * BAR）。ノートは 3.1.0000 D4 0.1440 80 ・ 3.1.1440 E4 0.1440 83 ・ 3.2.0960 F4 0.0960 77。
const clip = { start: 0, end: 4 * BAR, takeOffset: 0 };
const notes = take([
  [at(3, 1, 0), (3 * TPQ) / 4, 62, 80],
  [at(3, 1, (3 * TPQ) / 4), (3 * TPQ) / 4, 64, 83],
  [at(3, 2, TPQ / 2), TPQ / 2, 65, 77],
]);

describe("1 つのノートの欄", () => {
  it("位置: そのノートの長さがクリップに入る所まで。範囲の文は表示の形", () => {
    const s = noteFieldSpec("position", notes, clip, [0], M44)!;
    expect(s.initial).toBe("3.1.0000");
    expect(s.range).toBe("1.1.0000〜4.4.0480");
    expect(parseField(s, "4.4.0480")).toBe(at(4, 4, TPQ / 4));
    expect(parseField(s, "4.4.0481")).toBeNull();
    expect(parseField(s, "5.1.0000")).toBeNull();
  });

  it("音: C-1〜G9", () => {
    const s = noteFieldSpec("key", notes, clip, [1], M44)!;
    expect(s.initial).toBe("E4");
    expect(s.range).toBe("C-1〜G9");
  });

  it("長さ: 表示の 1 ティックから、クリップの終わりまで", () => {
    const s = noteFieldSpec("length", notes, clip, [2], M44)!;
    expect(s.initial).toBe("0.0960");
    expect(s.range).toBe("0.0001〜6.0960");
    expect(parseField(s, "0.0000")).toBeNull();
    expect(parseField(s, "6.0961")).toBeNull();
  });

  it("ベロシティ: 1〜127。範囲の文は画面の文（value.velocityRange）なので null", () => {
    const s = noteFieldSpec("velocity", notes, clip, [0], M44)!;
    expect(s.initial).toBe("80");
    expect(s.range).toBeNull();
    expect(parseField(s, "200")).toBeNull();
    expect(parseField(s, "0")).toBeNull();
    expect(parseField(s, "127")).toBe(127);
  });

  it("位置は曲の中（テイクの 0 は、クリップの start + takeOffset）", () => {
    const s = noteFieldSpec("position", notes, { start: BAR, end: 5 * BAR, takeOffset: 0 }, [0], M44)!;
    expect(s.initial).toBe("4.1.0000");
    expect(s.range).toBe("2.1.0000〜5.4.0480");
  });
});

describe("2 つ以上のノートの欄（SetNotesField の意味のとおり）", () => {
  const all = [0, 1, 2];

  it("位置: いちばん早い位置。まとまり全体がクリップに入る所まで", () => {
    const s = noteFieldSpec("position", notes, clip, all, M44)!;
    expect(s.initial).toBe("3.1.0000");
    expect(s.range).toBe("1.1.0000〜4.3.0000");
  });

  it("音: いちばん低い音。いちばん高い音が G9 をこえない所まで", () => {
    const s = noteFieldSpec("key", notes, clip, all, M44)!;
    expect(s.initial).toBe("D4");
    expect(s.range).toBe("C-1〜E9");
  });

  it("長さ: ばらばらなら、いちばん早いノートの長さを入れておく。どの値でも変わる", () => {
    const s = noteFieldSpec("length", notes, clip, all, M44)!;
    expect(s.initial).toBe("0.1440");
    expect(s.current).toBeNull();
    expect(s.range).toBe("0.0001〜6.0960");
    expect(commitCommand(s, (3 * TPQ) / 4, 12, all)).toEqual({ type: "setNotesField", take: 12, notes: all, field: "length", value: (3 * TPQ) / 4 });
  });

  it("ベロシティ: ばらばらなら、いちばん小さい値を入れておく", () => {
    const s = noteFieldSpec("velocity", notes, clip, all, M44)!;
    expect(s.initial).toBe("77");
    expect(s.current).toBeNull();
  });
});

describe("↑ ↓ と送る意図", () => {
  it("位置と長さは表示のいちばん下のけた（1 ティック）ずつ。範囲の外へは出ない", () => {
    const s = noteFieldSpec("position", notes, clip, [0], M44)!;
    expect(s.format(stepField(s, at(3, 1, 0), 1))).toBe("3.1.0001");
    expect(s.format(stepField(s, at(3, 1, 0), -10))).toBe("2.4.1910");
    expect(s.format(stepField(s, 5, -10))).toBe("1.1.0000");
    const l = noteFieldSpec("length", notes, clip, [0], M44)!;
    expect(l.format(stepField(l, 1, -1))).toBe("0.0001");
  });

  it("音は半音、ベロシティは 1 ずつ。127 で止まる", () => {
    const k = noteFieldSpec("key", notes, clip, [0], M44)!;
    expect(k.format(stepField(k, 62, 1))).toBe("D#4");
    const v = noteFieldSpec("velocity", notes, clip, [0], M44)!;
    expect(stepField(v, 120, 10)).toBe(127);
  });

  it("値が変わらなければ送らない（取り消しの手を作らない）。変われば setNotesField", () => {
    const s = noteFieldSpec("position", notes, clip, [1], M44)!;
    expect(commitCommand(s, at(3, 1, (3 * TPQ) / 4), 12, [1])).toBeNull();
    expect(commitCommand(s, at(2, 1, 0), 12, [1])).toEqual({ type: "setNotesField", take: 12, notes: [1], field: "start", value: at(2, 1, 0) });
    const k = noteFieldSpec("key", notes, clip, [1], M44)!;
    expect(commitCommand(k, 63, 12, [1])).toEqual({ type: "setNotesField", take: 12, notes: [1], field: "key", value: 63 });
  });

  it("使えない値: 欄、もとの値、範囲の文（ばらばらなら欄に見えていた文字）", () => {
    const p = noteFieldSpec("position", notes, clip, [0], M44)!;
    expect(rejectCommand(p, "3.1.0000")).toEqual({ type: "rejectValue", field: "position", kept: "3.1.0000", range: "1.1.0000〜4.4.0480" });
    const v = noteFieldSpec("velocity", notes, clip, [0, 1, 2], M44)!;
    expect(rejectCommand(v, "77 – 83")).toEqual({ type: "rejectValue", field: "velocity", kept: "77 – 83", range: null });
  });
});

describe("Tab の順", () => {
  it("位置 → 音 → 長さ → Vel → 次の行の位置。Shift Tab は逆", () => {
    expect(nextField("position", false)).toEqual({ name: "key", row: 0 });
    expect(nextField("length", false)).toEqual({ name: "velocity", row: 0 });
    expect(nextField("velocity", false)).toEqual({ name: "position", row: 1 });
    expect(nextField("position", true)).toEqual({ name: "velocity", row: -1 });
  });
});

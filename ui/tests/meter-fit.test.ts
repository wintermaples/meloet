import { describe, expect, it } from "vitest";
import { charCount, meterFit, nameWidthPx, sentences, type MeterFitInput } from "../src/components/meterFit";

const ALL = { narrowKeys: false, instrument: true, targetLabel: true, sustain: true, sentences: 0, typed: 0 };
import { heldText, noteName, pad3, signed4 } from "../src/components/meterFormat";

const base: MeterFitInput = {
  meter: "full",
  scale: 100,
  nameChars: 5,
  available: 2000,
  rest: 800,
  name: 60,
  pitchBend: 90,
  modulation: 110,
};

describe("name column of the input meter (spec 1 (size.meter.name), spec 3)", () => {
  it("full: 108 up to 8 characters, 148 over 8 characters", () => {
    expect(meterFit({ ...base, nameChars: 8 }).nameWidth).toBe("size.meter.name");
    expect(meterFit({ ...base, nameChars: 9 }).nameWidth).toBe("part.meter.name.long");
    expect(nameWidthPx("size.meter.name", 100)).toBe(108);
    expect(nameWidthPx("part.meter.name.long", 100)).toBe(148);
    expect(charCount("Keystation 49 MK3")).toBe(17);
    expect(charCount("鍵盤 49")).toBe(5);
  });

  it("slim: up to 176. Down to 128 if the width is not enough", () => {
    expect(meterFit({ ...base, meter: "slim", name: 300 }).nameWidth).toBe("size.meter.name.slim");
    expect(nameWidthPx("size.meter.name.slim", 100)).toBe(176);
    expect(nameWidthPx("part.meter.name.slim-narrow", 100)).toBe(128);
    expect(nameWidthPx("size.meter.name.slim", 150)).toBe(264);
    expect(nameWidthPx("part.meter.name.slim-narrow", 150)).toBe(192);
  });

  it("before narrowing, hide modulation, then pitch bend", () => {
    const slim = { ...base, meter: "slim" as const, name: 200, rest: 500 };
    // All: 500 + 176 + 90 + 110 = 876
    expect(meterFit({ ...slim, available: 876 })).toEqual({
      nameWidth: "size.meter.name.slim",
      pitchBend: true,
      modulation: true,
      ...ALL,
    });
    expect(meterFit({ ...slim, available: 875 })).toEqual({
      nameWidth: "size.meter.name.slim",
      pitchBend: true,
      modulation: false,
      ...ALL,
    });
    expect(meterFit({ ...slim, available: 676 })).toEqual({
      nameWidth: "size.meter.name.slim",
      pitchBend: false,
      modulation: false,
      ...ALL,
    });
    expect(meterFit({ ...slim, available: 675 })).toEqual({
      nameWidth: "part.meter.name.slim-narrow",
      pitchBend: false,
      modulation: false,
      ...ALL,
    });
  });

  it("slim: a short name counts only its own width", () => {
    const slim = { ...base, meter: "slim" as const, name: 60, rest: 500 };
    expect(meterFit({ ...slim, available: 760 }).modulation).toBe(true);
  });

  it("full: the name column keeps its width, only the readouts are hidden", () => {
    const fit = meterFit({ ...base, nameChars: 17, available: 100 });
    expect(fit).toMatchObject({ nameWidth: "part.meter.name.long", pitchBend: false, modulation: false });
  });

  it("at 150 %, compare with the 150 % width (the 853px example in F-MeterLongName)", () => {
    // 853 × 1.5 ≒ 1280 CSS px. 1050 + 230 does not fit, 1050 + 192 does.
    const fit = meterFit({
      meter: "slim",
      scale: 150,
      nameChars: 17,
      available: 1280 - 36,
      rest: 1050,
      name: 230,
      pitchBend: 120,
      modulation: 150,
    });
    expect(fit).toEqual({ nameWidth: "part.meter.name.slim-narrow", pitchBend: false, modulation: false, ...ALL });
  });
});

describe("full input meter without enough width (spec 1 (Rules for the scale))", () => {
  // All: 800 + 108 + 90 + 110 = 1108. Width saved: keys 100, instrument name 60, label 40, sustain 70.
  const full = { ...base, narrowKeys: 100, instrument: 60, targetLabel: 40, sustain: 70 };
  const at = (available: number) => meterFit({ ...full, available });

  it("what plays when you play: modulation -> pitch bend -> narrow keys -> instrument name -> label -> sustain, in that order", () => {
    expect(at(1108)).toMatchObject({ modulation: true, pitchBend: true, narrowKeys: false, instrument: true, targetLabel: true, sustain: true });
    expect(at(1107)).toMatchObject({ modulation: false, pitchBend: true, narrowKeys: false });
    expect(at(997)).toMatchObject({ modulation: false, pitchBend: false, narrowKeys: false });
    expect(at(907)).toMatchObject({ pitchBend: false, narrowKeys: true, instrument: true });
    expect(at(807)).toMatchObject({ narrowKeys: true, instrument: false, targetLabel: true });
    expect(at(747)).toMatchObject({ instrument: false, targetLabel: false, sustain: true });
    expect(at(707)).toMatchObject({ instrument: false, targetLabel: false, sustain: false });
    expect(at(10)).toMatchObject({ narrowKeys: true, instrument: false, targetLabel: false, sustain: false, nameWidth: "size.meter.name" });
  });

  // The 3 hint sentences (100, 80, 60) are inside rest's 800.
  const message = { ...base, narrowKeys: 100, sustain: 70, sentences: [100, 80, 60] };
  const msgAt = (available: number) => meterFit({ ...message, available });

  it("hint text: modulation -> pitch bend -> narrow keys -> from the 2nd sentence on (from the end) -> sustain, in that order", () => {
    expect(msgAt(1108)).toMatchObject({ modulation: true, sentences: 3, sustain: true });
    expect(msgAt(907)).toMatchObject({ pitchBend: false, narrowKeys: true, sentences: 3 });
    expect(msgAt(808)).toMatchObject({ narrowKeys: true, sentences: 3 });
    expect(msgAt(807)).toMatchObject({ narrowKeys: true, sentences: 2, sustain: true });
    expect(msgAt(748)).toMatchObject({ sentences: 2, sustain: true });
    expect(msgAt(747)).toMatchObject({ sentences: 1, sustain: true });
    expect(msgAt(668)).toMatchObject({ sentences: 1, sustain: true });
    expect(msgAt(667)).toMatchObject({ sentences: 1, sustain: false });
    expect(msgAt(10)).toMatchObject({ sentences: 1, sustain: false, instrument: true, targetLabel: true });
  });

  it("slim: keys, instrument name, label and sustain are not hidden", () => {
    expect(meterFit({ ...full, meter: "slim", available: 10 })).toMatchObject(ALL);
  });
});

describe("hint text at the right end of the input meter", () => {
  it("split into sentences", () => {
    expect(sentences("鍵盤を USB でつなぐと、ここに出ます。つながなくても、マウスで音を書けます。")).toEqual([
      "鍵盤を USB でつなぐと、ここに出ます。",
      "つながなくても、マウスで音を書けます。",
    ]);
    expect(sentences("音の出口がまだです。下でえらんでください")).toEqual(["音の出口がまだです。", "下でえらんでください"]);
  });

  it("slim: after the readouts and the name column, hide sentences from the end and keep the first", () => {
    // 500 + 176 + 90 + 110 = 876 (sentences 100, 80, 60 are inside rest's 500)
    const slim = { ...base, meter: "slim" as const, name: 200, rest: 500, sentences: [100, 80, 60] };
    expect(meterFit({ ...slim, available: 876 })).toMatchObject({ modulation: true, sentences: 3 });
    expect(meterFit({ ...slim, available: 676 })).toMatchObject({ pitchBend: false, nameWidth: "size.meter.name.slim", sentences: 3 });
    expect(meterFit({ ...slim, available: 675 })).toMatchObject({ nameWidth: "part.meter.name.slim-narrow", sentences: 3 });
    expect(meterFit({ ...slim, available: 627 })).toMatchObject({ sentences: 2 });
    expect(meterFit({ ...slim, available: 567 })).toMatchObject({ sentences: 1 });
    expect(meterFit({ ...slim, available: 5 })).toMatchObject({ sentences: 1 });
  });
});

describe("how the input meter writes numbers", () => {
  it("note names: C4 = 60, low to high", () => {
    expect(noteName(60)).toBe("C4");
    expect(noteName(36)).toBe("C2");
    expect(noteName(84)).toBe("C6");
    expect(noteName(61)).toBe("C#4");
    expect(heldText([67, 60, 64])).toBe("C4 E4 G4");
  });

  it("3 digits, and 4 digits with a sign", () => {
    expect(pad3(12)).toBe("012");
    expect(pad3(0)).toBe("000");
    expect(signed4(0)).toBe("+0000");
    expect(signed4(512)).toBe("+0512");
    expect(signed4(-8192)).toBe("−8192");
  });
});

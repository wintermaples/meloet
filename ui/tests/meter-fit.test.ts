import { describe, expect, it } from "vitest";
import { charCount, meterFit, nameWidthPx, type MeterFitInput } from "../src/components/meterFit";
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

describe("入力計の名前の欄（仕様書 1 章 size.meter.name、3 章）", () => {
  it("太いとき: 8 文字までは 108、8 文字をこえたら 148", () => {
    expect(meterFit({ ...base, nameChars: 8 }).nameWidth).toBe("size.meter.name");
    expect(meterFit({ ...base, nameChars: 9 }).nameWidth).toBe("part.meter.name.long");
    expect(nameWidthPx("size.meter.name", 100)).toBe(108);
    expect(nameWidthPx("part.meter.name.long", 100)).toBe(148);
    expect(charCount("Keystation 49 MK3")).toBe(17);
    expect(charCount("鍵盤 49")).toBe(5);
  });

  it("細いとき: 176 まで。横が足りなければ 128 まで", () => {
    expect(meterFit({ ...base, meter: "slim", name: 300 }).nameWidth).toBe("size.meter.name.slim");
    expect(nameWidthPx("size.meter.name.slim", 100)).toBe(176);
    expect(nameWidthPx("part.meter.name.slim-narrow", 100)).toBe(128);
    expect(nameWidthPx("size.meter.name.slim", 150)).toBe(264);
    expect(nameWidthPx("part.meter.name.slim-narrow", 150)).toBe(192);
  });

  it("ちぢめる前に、モジュレーション、ピッチベンドの順に引っこめる", () => {
    const slim = { ...base, meter: "slim" as const, name: 200, rest: 500 };
    // 全部: 500 + 176 + 90 + 110 = 876
    expect(meterFit({ ...slim, available: 876 })).toEqual({
      nameWidth: "size.meter.name.slim",
      pitchBend: true,
      modulation: true,
    });
    expect(meterFit({ ...slim, available: 875 })).toEqual({
      nameWidth: "size.meter.name.slim",
      pitchBend: true,
      modulation: false,
    });
    expect(meterFit({ ...slim, available: 676 })).toEqual({
      nameWidth: "size.meter.name.slim",
      pitchBend: false,
      modulation: false,
    });
    expect(meterFit({ ...slim, available: 675 })).toEqual({
      nameWidth: "part.meter.name.slim-narrow",
      pitchBend: false,
      modulation: false,
    });
  });

  it("細いとき、短い名前は自分の幅だけで数える", () => {
    const slim = { ...base, meter: "slim" as const, name: 60, rest: 500 };
    expect(meterFit({ ...slim, available: 760 }).modulation).toBe(true);
  });

  it("太いときは名前の欄の幅を変えず、読み値だけ引っこめる", () => {
    const fit = meterFit({ ...base, nameChars: 17, available: 100 });
    expect(fit).toEqual({ nameWidth: "part.meter.name.long", pitchBend: false, modulation: false });
  });

  it("150 % では 150 % の幅でくらべる（F-MeterLongName の 853px の例）", () => {
    // 853 × 1.5 ≒ 1280 CSS px。1050 + 230 は入らず、1050 + 192 なら入る。
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
    expect(fit).toEqual({ nameWidth: "part.meter.name.slim-narrow", pitchBend: false, modulation: false });
  });
});

describe("入力計の数の書きかた", () => {
  it("音名: C4 = 60、低い順", () => {
    expect(noteName(60)).toBe("C4");
    expect(noteName(36)).toBe("C2");
    expect(noteName(84)).toBe("C6");
    expect(noteName(61)).toBe("C#4");
    expect(heldText([67, 60, 64])).toBe("C4 E4 G4");
  });

  it("3 けた、符号つき 4 けた", () => {
    expect(pad3(12)).toBe("012");
    expect(pad3(0)).toBe("000");
    expect(signed4(0)).toBe("+0000");
    expect(signed4(512)).toBe("+0512");
    expect(signed4(-8192)).toBe("−8192");
  });
});

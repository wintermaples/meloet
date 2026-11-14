import { describe, expect, it } from "vitest";
import { charCount, meterFit, nameWidthPx, sentences, sentencesShown, type MeterFitInput } from "../src/components/meterFit";

const ALL = { narrowKeys: false, targetLabel: true, sustain: true };
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

  it("細いとき、短い名前は自分の幅だけで数える", () => {
    const slim = { ...base, meter: "slim" as const, name: 60, rest: 500 };
    expect(meterFit({ ...slim, available: 760 }).modulation).toBe(true);
  });

  it("太いときは名前の欄の幅を変えず、読み値だけ引っこめる", () => {
    const fit = meterFit({ ...base, nameChars: 17, available: 100 });
    expect(fit).toMatchObject({ nameWidth: "part.meter.name.long", pitchBend: false, modulation: false });
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
    expect(fit).toEqual({ nameWidth: "part.meter.name.slim-narrow", pitchBend: false, modulation: false, ...ALL });
  });
});

describe("太い入力計で横が足りないとき（仕様書 1 章「拡大率の決まり」）", () => {
  // 全部: 800 + 108 + 90 + 110 = 1108。浮く幅: 鍵 100、ラベル 40、サステイン 70。
  const full = { ...base, narrowKeys: 100, targetLabel: 40, sustain: 70 };
  const at = (available: number) => meterFit({ ...full, available });

  it("モジュレーション → ピッチベンド → 鍵盤を細い鍵に → 「弾くと鳴る先」のラベル → サステインの順", () => {
    expect(at(1108)).toMatchObject({ modulation: true, pitchBend: true, narrowKeys: false, targetLabel: true, sustain: true });
    expect(at(1107)).toMatchObject({ modulation: false, pitchBend: true, narrowKeys: false });
    expect(at(997)).toMatchObject({ modulation: false, pitchBend: false, narrowKeys: false });
    expect(at(907)).toMatchObject({ pitchBend: false, narrowKeys: true, targetLabel: true });
    expect(at(807)).toMatchObject({ narrowKeys: true, targetLabel: false, sustain: true });
    expect(at(767)).toMatchObject({ narrowKeys: true, targetLabel: false, sustain: false });
    expect(at(10)).toMatchObject({ narrowKeys: true, targetLabel: false, sustain: false, nameWidth: "size.meter.name" });
  });

  it("細いときは鍵盤、ラベル、サステインを引っこめない", () => {
    expect(meterFit({ ...full, meter: "slim", available: 10 })).toMatchObject(ALL);
  });
});

describe("入力計の右はしの案内の文", () => {
  it("文に分ける", () => {
    expect(sentences("鍵盤を USB でつなぐと、ここに出ます。つながなくても、マウスで音を書けます。")).toEqual([
      "鍵盤を USB でつなぐと、ここに出ます。",
      "つながなくても、マウスで音を書けます。",
    ]);
    expect(sentences("音の出口がまだです。下でえらんでください")).toEqual(["音の出口がまだです。", "下でえらんでください"]);
  });

  it("うしろの文から引っこめ、1 文目は残す", () => {
    expect(sentencesShown([100, 80, 60], 240)).toBe(3);
    expect(sentencesShown([100, 80, 60], 239)).toBe(2);
    expect(sentencesShown([100, 80, 60], 179)).toBe(1);
    expect(sentencesShown([100, 80, 60], 5)).toBe(1);
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

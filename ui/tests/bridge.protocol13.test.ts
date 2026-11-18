// 約束 13: トラックは id で指す。録音待機は、いつも選んだトラック。音源の種類と、棚の状態。
import { describe, expect, it } from "vitest";
import { createFakeTransport, fakeScene, fakeState } from "../src/bridge/fake";
import { INSTRUMENT_GROUPS, type AppState, type Command, type ServerMessage } from "../src/bridge/types";
import { fill, t, texts, type TextKey } from "../src/text/ja";
import { mockScreenText } from "./design-sources";

const state = (scene: string) => fakeScene(scene)!.state;

describe("約束 13 の形", () => {
  it("音源は 11、グループは 5。並びは棚の順（仕様書 3 章）で、どれにも名前がある", () => {
    expect(INSTRUMENT_GROUPS.map((g) => [t(`instrumentGroup.${g.group}`), g.instruments.map((i) => t(`instrument.${i}`))])).toEqual([
      ["ピアノ ・ 鍵盤", ["グランドピアノ", "エレクトリックピアノ", "オルガン"]],
      ["ベース", ["フィンガーベース", "シンセベース"]],
      ["ドラム", ["スタジオキット", "エレクトロキット"]],
      ["パッド ・ 弦", ["ストリングス", "あたたかいパッド"]],
      ["シンセ", ["シンセリード", "サイン波"]],
    ]);
  });

  it("新しい曲の 1 本目はグランドピアノ。棚はとじていて、お気に入りはグランドピアノとスタジオキット", () => {
    const s = fakeState();
    expect(s.song.tracks.map((x) => [x.id, x.number, x.instrument])).toEqual([[1, 1, "grandPiano"]]);
    expect(s.song.playTarget).toBe(1);
    expect(s.shelf).toMatchObject({ mode: { kind: "newTrack" }, favorites: ["grandPiano", "studioKit"], selected: null, current: null, loading: null, sample: null });
  });
});

describe("約束 13 の偽物の場面", () => {
  it("tracks6（モック R7-Arrange6）: 6 本。番号は並びの順、id は番号とちがう。色は 6 本目で 1 に戻る", () => {
    const s = state("tracks6");
    expect(s.song.tracks.map((x) => [x.number, x.name, t(`instrument.${x.instrument}`), x.color])).toEqual([
      [1, "ピアノ", "グランドピアノ", 1],
      [2, "ベース", "フィンガーベース", 2],
      [3, "ドラム", "スタジオキット", 3],
      [4, "パッド", "ストリングス", 4],
      [5, "シンセ", "シンセリード", 5],
      [6, "ベース 2", "シンセベース", 1],
    ]);
    expect(s.song.tracks.map((x) => x.id)).toEqual([11, 12, 13, 14, 15, 16]);
    // ● で「2 ベース」を選んだ所。選んだトラックは id で言う。
    expect(s.song.playTarget).toBe(12);
    expect(s.status.message).toEqual({ id: "armMoved", track: { number: 2, name: "ベース" } });
    // モックの見出しの文と同じ。
    const mock = mockScreenText("R7-Arrange6");
    for (const x of s.song.tracks) expect(mock).toContain(`${x.name}MS●${t(`instrument.${x.instrument}`)}`);
    expect(mock).toContain(t("status.armMoved", { recordKey: t("key.record"), track: "2 ベース" }));
  });

  it("shelfNewTrack: 棚が「新しいトラック」でひらき、何もえらんでいない。行は 11 ぜんぶ", () => {
    const s = state("shelfNewTrack");
    expect(s.workspace.layout.rightFront).toBe("shelf");
    expect(s.shelf.mode).toEqual({ kind: "newTrack" });
    expect(s.shelf.available).toEqual(INSTRUMENT_GROUPS.flatMap((g) => g.instruments));
    expect([s.shelf.selected, s.shelf.current, s.shelf.sample]).toEqual([null, null, null]);
    expect(s.song.tracks).toHaveLength(4);
    expect(s.transport.canRecord).toBe(true);
  });

  it("shelfAudition（モック F4-ShelfAudition）: シンセベースをためし弾き中。録音はできない", () => {
    const s = state("shelfAudition");
    expect(s.shelf).toMatchObject({ mode: { kind: "newTrack" }, selected: "synthBass", current: null });
    expect(s.transport.canRecord).toBe(false);
    expect(s.status.message).toEqual({ id: "auditionNewTrack", instrument: "synthBass" });
    const mock = mockScreenText("F4-ShelfAudition");
    expect(mock).toContain(t("status.auditionNewTrack", { instrument: t("instrument.synthBass"), key: t("key.enter") }));
    expect(mock).toContain(t("button.makeTrack", { number: s.song.tracks.length + 1 }));
  });

  it("shelfReplace（モック F4-ShelfReplace）: 1 ピアノのエレクトリックピアノをためし弾き中。いまの音源はグランドピアノ", () => {
    const s = state("shelfReplace");
    const track = s.song.tracks[0]!;
    expect(s.shelf).toMatchObject({ mode: { kind: "replace", track: track.id }, selected: "electricPiano", current: "grandPiano" });
    // 曲は変えていない。
    expect(track.instrument).toBe("grandPiano");
    expect(s.status.message).toEqual({
      id: "auditionReplace",
      instrument: "electricPiano",
      track: { number: 1, name: "ピアノ" },
      current: "grandPiano",
    });
    const mock = mockScreenText("F4-ShelfReplace");
    expect(mock).toContain(t("shelf.titleReplace", { track: t("track.label", { number: track.number, name: track.name }) }));
    expect(mock).toContain(
      t("status.auditionReplace", {
        instrument: t("instrument.electricPiano"),
        track: "1 ピアノ",
        current: t("instrument.grandPiano"),
        key: t("key.enter"),
      }),
    );
  });
});

describe("偽物の Rust: トラックは id で指す", () => {
  it("selectTrack ・ toggleMute は id。番号で送っても、どのトラックにも当たらない", async () => {
    const scene = fakeScene("tracks6")!;
    const transport = createFakeTransport(structuredClone(scene.state));
    let s: AppState = structuredClone(scene.state);
    await transport.subscribe(
      (m: ServerMessage) => {
        if (m.kind !== "patch") return;
        s = { ...s };
        for (const c of m.changes) (s as Record<string, unknown>)[c.section] = c.value;
      },
      () => {},
    );
    const send = async (c: Command) => void (await transport.send(c));
    await send({ type: "selectTrack", track: 3, fromArm: false });
    expect(s.song.playTarget).toBe(12);
    await send({ type: "selectTrack", track: 13, fromArm: true });
    expect(s.song.playTarget).toBe(13);
    await send({ type: "toggleMute", track: 14 });
    expect(s.song.tracks.map((x) => x.muted)).toEqual([false, false, false, true, false, false]);
  });
});

describe("文面の穴は、約束の種類の名前と合っている", () => {
  it("instrument.* と instrumentGroup.* は、約束にある種類だけ", () => {
    const keys = (prefix: string) => (Object.keys(texts) as TextKey[]).filter((k) => k.startsWith(prefix)).map((k) => k.slice(prefix.length));
    expect(keys("instrument.")).toEqual(INSTRUMENT_GROUPS.flatMap((g) => g.instruments));
    expect(keys("instrumentGroup.")).toEqual(INSTRUMENT_GROUPS.map((g) => g.group));
    expect(fill(texts["shelf.keys"].text, { sampleKey: t("key.playStop"), favoriteKey: "F" })).toBe("サンプル Space ・ 印 F");
  });
});

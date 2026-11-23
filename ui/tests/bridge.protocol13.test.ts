// Protocol 13: tracks are pointed to by id. Record arm is always the selected track. Instrument kinds and the shelf state.
import { describe, expect, it } from "vitest";
import { createFakeTransport, fakeScene, fakeState } from "../src/bridge/fake";
import { INSTRUMENT_GROUPS, type AppState, type Command, type ServerMessage } from "../src/bridge/types";
import { fill, t, type TextKey } from "../src/text";
import { texts } from "../src/text/ja";
import { mockScreenText } from "./design-sources";

const state = (scene: string) => fakeScene(scene)!.state;

describe("shape of protocol 13", () => {
  it("31 instruments, 7 groups. In shelf order (spec 3 (84)), and each has a name", () => {
    expect(INSTRUMENT_GROUPS.map((g) => [t(`instrumentGroup.${g.group}`), g.instruments.map((i) => t(`instrument.${i}`))])).toEqual([
      [
        "ピアノ ・ 鍵盤",
        [
          "グランドピアノ",
          "エレクトリックピアノ",
          "ドローバーオルガン",
          "パーカッシブオルガン",
          "ロックオルガン",
          "パイプオルガン",
          "パイプオルガン（やわらかい）",
          "チャーチオルガン",
          "リードオルガン",
          "ビブラフォン",
          "マリンバ",
        ],
      ],
      ["ギター", ["ナイロンギター", "スチールギター", "エレキギター", "ひずんだギター"]],
      ["ベース", ["フィンガーベース", "ピックベース", "ウッドベース", "シンセベース"]],
      ["ドラム", ["スタジオキット", "エレクトロキット", "ブラシキット"]],
      ["パッド ・ 弦", ["ストリングス", "ピチカート", "あたたかいパッド"]],
      ["管楽器", ["トランペット", "ブラスセクション", "アルトサックス", "フルート"]],
      ["シンセ", ["シンセリード", "サイン波"]],
    ]);
    const all = INSTRUMENT_GROUPS.flatMap((g) => g.instruments);
    expect(new Set(all).size).toBe(31);
    expect(new Set(all.map((i) => t(`instrument.${i}`))).size).toBe(31);
  });

  it("the first track of a new song is the grand piano. The shelf is closed, and the favorites are the grand piano and the studio kit", () => {
    const s = fakeState();
    expect(s.song.tracks.map((x) => [x.id, x.number, x.instrument])).toEqual([[1, 1, "grandPiano"]]);
    expect(s.song.playTarget).toBe(1);
    expect(s.shelf).toMatchObject({ mode: { kind: "newTrack" }, favorites: ["grandPiano", "studioKit"], selected: null, current: null, loading: null, sample: null });
  });
});

describe("fake scenes of protocol 13", () => {
  it("tracks6 (mock R7-Arrange6): 6 tracks. Numbers follow the order, ids differ from numbers. Colors go back to 1 at the 6th track", () => {
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
    // 「2 ベース」 selected with ●. The selected track is given by id.
    expect(s.song.playTarget).toBe(12);
    expect(s.status.message).toEqual({ id: "armMoved", track: { number: 2, name: "ベース" } });
    // the same as the mock's heading text.
    const mock = mockScreenText("R7-Arrange6");
    for (const x of s.song.tracks) expect(mock).toContain(`${x.name}MS●${t(`instrument.${x.instrument}`)}`);
    expect(mock).toContain(t("status.armMoved", { recordKey: t("key.record"), track: "2 ベース" }));
  });

  it("shelfNewTrack: the shelf opens on 「新しいトラック」 with nothing chosen. All 11 rows", () => {
    const s = state("shelfNewTrack");
    expect(s.workspace.layout.rightFront).toBe("shelf");
    expect(s.shelf.mode).toEqual({ kind: "newTrack" });
    expect(s.shelf.available).toEqual(INSTRUMENT_GROUPS.flatMap((g) => g.instruments));
    expect([s.shelf.selected, s.shelf.current, s.shelf.sample]).toEqual([null, null, null]);
    expect(s.song.tracks).toHaveLength(4);
    expect(s.transport.canRecord).toBe(true);
  });

  it("shelfAudition (mock F4-ShelfAudition): auditioning the synth bass. Cannot record", () => {
    const s = state("shelfAudition");
    expect(s.shelf).toMatchObject({ mode: { kind: "newTrack" }, selected: "synthBass", current: null });
    expect(s.transport.canRecord).toBe(false);
    expect(s.status.message).toEqual({ id: "auditionNewTrack", instrument: "synthBass" });
    const mock = mockScreenText("F4-ShelfAudition");
    expect(mock).toContain(t("status.auditionNewTrack", { instrument: t("instrument.synthBass"), key: t("key.enter") }));
    expect(mock).toContain(t("button.makeTrack", { number: s.song.tracks.length + 1 }));
  });

  it("shelfReplace (mock F4-ShelfReplace): auditioning the electric piano on track 1 piano. The current instrument is the grand piano", () => {
    const s = state("shelfReplace");
    const track = s.song.tracks[0]!;
    expect(s.shelf).toMatchObject({ mode: { kind: "replace", track: track.id }, selected: "electricPiano", current: "grandPiano" });
    // the song is not changed.
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

describe("fake Rust: tracks are pointed to by id", () => {
  it("selectTrack and toggleMute use the id. Sending a number hits no track", async () => {
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

describe("holes in the wording match the kind names in the protocol", () => {
  it("instrument.* and instrumentGroup.* have only the kinds in the protocol", () => {
    const keys = (prefix: string) => (Object.keys(texts) as TextKey[]).filter((k) => k.startsWith(prefix)).map((k) => k.slice(prefix.length));
    expect(keys("instrument.")).toEqual(INSTRUMENT_GROUPS.flatMap((g) => g.instruments));
    expect(keys("instrumentGroup.")).toEqual(INSTRUMENT_GROUPS.map((g) => g.group));
    expect(fill(texts["shelf.keys"].text, { sampleKey: t("key.playStop"), favoriteKey: "F" })).toBe("サンプル Space ・ 印 F");
  });
});

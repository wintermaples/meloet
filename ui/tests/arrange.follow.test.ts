// 画面の「曲のティック → ファイルのこま」（src/components/arrange/follow.ts）が、Rust の式
// （crates/meloet-model/src/audio/follow.rs の FileClock）と同じ答えを出すか。
// 答えの表は `cargo test -p meloet-model --test follow` が書く。
import { describe, expect, it } from "vitest";
import type { ClipView } from "../src/bridge";
import { fakeScene } from "../src/bridge/fake";
import { fileFrameAt } from "../src/components/arrange/audioClipGeom";
import { clipFrameAt, followClipOf, sourceMapOf, stretchRatioAt } from "../src/components/arrange/follow";
import table from "../src/components/arrange/follow.table.json";
import { TempoMap } from "../src/time/tempoMap";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";

const BAR = 4 * TPQ;

/** こまは 1000 分の 1 こままで（JS の log1p は最後の桁が Rust とちがうことがある。波形の 1 px は何百こま）。 */
const nearFrame = (got: number, want: number, what: string) => expect(Math.abs(got - want), `${what}: ${got} / ${want}`).toBeLessThanOrEqual(1e-3 + 1e-9 * Math.abs(want));

describe("ついていくクリップの式（約束 25、docs/architecture.md 22-19）", () => {
  it("Rust の表と同じ: ついていかない ・ ついていく ・ だんだんが両方にある", () => {
    expect(table.cases.length).toBe(6);
    expect(table.cases.filter((c) => c.clip.sourceTempo !== null).length).toBe(4);
    for (const c of table.cases) {
      const song = new TempoMap(c.song);
      const source = c.clip.sourceTempo === null ? null : sourceMapOf(c.clip.sourceTempo);
      expect(c.rows.length).toBeGreaterThan(40);
      for (const [tick, frame, ratio] of c.rows) {
        nearFrame(clipFrameAt(song, source, c.clip, tick!), frame!, `${c.name} @${tick}`);
        const got = stretchRatioAt(song, source, c.clip, tick!);
        if (ratio === null) expect(got, c.name).toBeNull();
        else expect(Math.abs(got! - ratio!), `${c.name} ×@${tick}`).toBeLessThanOrEqual(1e-12);
      }
    }
  });

  it("ついていかないクリップは、今の fileFrameAt と同じ答え", () => {
    const s = fakeScene("tempo")!.state;
    const map = new TempoMap(s.timeline);
    const clip = { id: 1, name: "ドラム", start: 3 * BAR, end: 9 * BAR, take: 1, takes: 1, takeId: 2, takeOffset: -TPQ, noteCount: 0, audio: null } as ClipView;
    const audio = fakeScene("stretchNoTempo")!.state.song.tracks.flatMap((t) => t.clips).find((c) => c.audio !== null)!.audio!;
    const withAudio: ClipView = { ...clip, audio: { ...audio, firstFrame: 777 } };
    const shape = followClipOf(withAudio)!;
    expect(followClipOf(clip)).toBeNull();
    for (let tick = 2 * BAR; tick <= 10 * BAR; tick += 333) expect(clipFrameAt(map, null, shape, tick)).toBe(fileFrameAt(map, withAudio, tick));
  });

  it("ついていくと、曲のテンポを変えても、同じティックに同じこまが来る（クリップの長さはティックのまま）", () => {
    const source = sourceMapOf({ tempoBpm: 96, headRamp: false, tempoPoints: [] });
    const clip = { start: 4 * BAR, takeOffset: 0, firstFrame: 0, sampleRate: 48000 };
    for (const tempoBpm of [60, 96, 120, 181.5]) {
      const song = new TempoMap({ tempoBpm, meterNum: 4, meterDen: 4, headRamp: false, tempoPoints: [], meterChanges: [] });
      // 96 で歌った 2 小節は、ファイルの 5 秒。
      expect(clipFrameAt(song, source, clip, 6 * BAR)).toBeCloseTo(240000, 6);
      expect(stretchRatioAt(song, source, clip, 4 * BAR)).toBeCloseTo(tempoBpm / 96, 12);
      // ついていかなければ、ファイルの速さのまま（曲のテンポでこまが変わる）。
      expect(clipFrameAt(song, null, clip, 6 * BAR)).toBeCloseTo((240000 * 96) / tempoBpm, 6);
    }
  });

  it("偽物の場面の数は、式の答えと合う（×1.25、だんだんの中の 1.125、録ったままは 1）", () => {
    for (const name of ["stretch", "stretchRendering", "stretchPitch", "stretchRamp", "audioEdit", "audioKinds"]) {
      const s = fakeScene(name)!.state;
      const song = new TempoMap(s.timeline);
      for (const clip of s.song.tracks.flatMap((t) => t.clips)) {
        if (clip.audio === null) continue;
        const source = clip.audio.sourceTempo === null ? null : sourceMapOf(clip.audio.sourceTempo);
        const ratio = stretchRatioAt(song, source, followClipOf(clip)!, clip.start);
        if (ratio === null) expect(clip.audio.stretch.ratio, `${name} ${clip.id}`).toBeNull();
        else expect(clip.audio.stretch.ratio, `${name} ${clip.id}`).toBeCloseTo(ratio, 9);
      }
    }
    const ramp = fakeScene("stretchRamp")!.state;
    const [recorded, sung] = ramp.song.tracks.find((t) => t.kind.kind === "vocal")!.clips;
    expect([recorded!.audio!.stretch.ratio, sung!.audio!.stretch.ratio]).toEqual([1, 1.125]);
    // この地図の中で録ったクリップは、ついていっても、ファイルの速さのままと同じ所（だんだんの中でも）。
    const song = new TempoMap(ramp.timeline);
    const source = sourceMapOf(recorded!.audio!.sourceTempo!);
    for (let tick = recorded!.start; tick <= recorded!.end; tick += 111) {
      nearFrame(clipFrameAt(song, source, followClipOf(recorded!)!, tick), fileFrameAt(song, recorded!, tick), `録ったまま @${tick}`);
    }
    // だんだんの先では、比が大きくなる。
    const shape = followClipOf(sung!)!;
    const own = sourceMapOf(sung!.audio!.sourceTempo!);
    expect(stretchRatioAt(song, own, shape, 8 * BAR)).toBeCloseTo(120 / 96, 12);
  });
});

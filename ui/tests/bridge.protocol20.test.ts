// Protocol 20: the `timeline` section, fake scenes (F6-Tempo, R10-Markers), status line text.
import { flushSync, mount, unmount } from "svelte";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppState, Command, StatusMessage, TimelineView } from "../src/bridge";
import { PROTOCOL_VERSION } from "../src/bridge";
import { fakeScene, fakeState } from "../src/bridge/fake";
import StatusLine from "../src/components/StatusLine.svelte";
import { t } from "../src/text/ja";
import { TempoMap } from "../src/time/tempoMap";
import { TICKS_PER_QUARTER as TPQ } from "../src/time/ticks";
import * as app from "./fake-app.svelte";

vi.mock("../src/state/app.svelte", () => import("./fake-app.svelte"));

const scene = (name: string): AppState => fakeScene(name)!.state;
const BAR = 4 * TPQ;

describe("shape of protocol 20", () => {
  it("version is 23 (after 22). A new song's timeline has only the tempo and time signature at the song start. song no longer has tempo or time signature", () => {
    expect(PROTOCOL_VERSION).toBeGreaterThanOrEqual(26);
    const s = fakeState();
    expect(s.timeline).toEqual({
      tempoBpm: 96,
      meterNum: 4,
      meterDen: 4,
      headRamp: false,
      tempoPoints: [],
      meterChanges: [],
      markers: [],
      selected: null,
      renaming: null,
      open: false,
    } satisfies TimelineView);
    for (const gone of ["tempoBpm", "meterNum", "meterDen", "tempoChanges"]) expect(s.song).not.toHaveProperty(gone);
  });

  it("intents point to a tempo point by tick, a time signature flag by bar (from 0), and a marker by id", () => {
    const commands: Command[] = [
      { type: "setTimelineOpen", open: true },
      { type: "addTempoPoint", tick: 4 * BAR, bpm: 104 },
      { type: "moveTempoPoint", tick: 4 * BAR, toTick: 4 * BAR + TPQ, bpm: 104.5 },
      { type: "setTempoRamp", tick: 0, on: true },
      { type: "addMeterChange", bar: 8, num: 3, den: 4 },
      { type: "moveMeterChange", bar: 8, toBar: 10 },
      { type: "setMeterChange", bar: 0, num: 6, den: 8 },
      { type: "selectTimeline", target: { kind: "tempoPoint", tick: 4 * BAR } },
      { type: "selectTimeline", target: null },
      { type: "deleteTimeline", target: { kind: "meterChange", bar: 8 } },
      { type: "addMarker", name: t("marker.newName", { count: 4 }), tick: null },
      { type: "openMarkerName", id: 2 },
      { type: "renameMarker", id: 2, name: "Aメロ ・ 1 回目" },
      { type: "closeMarkerName" },
      { type: "moveMarker", id: 2, tick: 5 * BAR },
      { type: "goToMarker", id: 2 },
      { type: "stepMarker", forward: true },
      { type: "loopFromMarker", id: 2 },
    ];
    expect(commands.length).toBe(18);
    expect(commands[10]).toEqual({ type: "addMarker", name: "マーカー 4", tick: null });
  });
});

describe("fake scenes", () => {
  it("tempo (F6-Tempo): the band is open, and the transport tempo at bar 7 of the play line is 112.00", () => {
    const s = scene("tempo");
    const tl = s.timeline;
    expect(tl.open).toBe(true);
    expect(tl.tempoPoints.map((p) => [p.tick, p.tempoBpm, p.ramp])).toEqual([
      [4 * BAR, 104, true],
      [8 * BAR, 120, false],
      [8 * BAR + 4 * 3 * TPQ, 96, false],
    ]);
    expect(tl.meterChanges).toEqual([
      { bar: 8, meterNum: 3, meterDen: 4 },
      { bar: 12, meterNum: 4, meterDen: 4 },
    ]);
    expect(tl.selected).toEqual({ kind: "tempoPoint", tick: 4 * BAR });

    const map = new TempoMap(tl);
    expect(map.barBeatTick(s.transport.position)).toEqual({ bar: 7, beat: 1, tick: 0 });
    expect(map.tempoAt(s.transport.position).toFixed(2)).toBe("112.00");
    expect(map.meterAt(s.transport.position)).toMatchObject({ num: 4, den: 4 });
    // Ruler in the mock: tempo points at bars 5, 9, 13; markers at bars 1, 5, 13.
    expect(tl.tempoPoints.map((p) => map.barBeatTick(p.tick).bar)).toEqual([5, 9, 13]);
    expect(tl.markers.map((m) => [m.name, map.barBeatTick(m.tick).bar])).toEqual([
      ["イントロ", 1],
      ["Aメロ", 5],
      ["サビ", 13],
    ]);
    // Clips stay at bar starts even when the time signature changes.
    const lead = s.song.tracks[4]!.clips;
    expect(lead.map((c) => [c.name, map.barBeatTick(c.start).bar, map.barBeatTick(c.end).bar])).toEqual([
      ["リード", 9, 13],
      ["リード サビ", 13, 17],
    ]);
    for (const clip of s.song.tracks.flatMap((track) => track.clips)) {
      expect(map.meterAt(clip.start).barTick).toBe(clip.start);
      expect(map.meterAt(clip.end).barTick).toBe(clip.end);
    }
    expect(s.song.undoCount).toBe(18);
    expect(s.song.file).toBe("changed");
  });

  it("tempoFolded: the band is folded, with 1 change", () => {
    const tl = scene("tempoFolded").timeline;
    expect(tl.open).toBe(false);
    expect(tl.tempoPoints.length + tl.meterChanges.length).toBe(1);
    expect(new TempoMap(tl).tempoAt(scene("tempoFolded").transport.position)).toBe(96);
  });

  it("markerAdded, markerRenaming, markerMoving (R10-Markers)", () => {
    const added = scene("markerAdded");
    expect(added.timeline.markers.map((m) => m.name)).toEqual(["イントロ", "Aメロ", "マーカー 4", "サビ"]);
    expect(added.timeline.markers.map((m) => m.tick)).toEqual([0, 4 * BAR, 6 * BAR, 9 * BAR]);
    expect(added.timeline.markers[2]!.tick).toBe(added.transport.position);
    expect(added.timeline.selected).toEqual({ kind: "marker", id: 4 });
    expect(added.timeline.renaming).toBe(4);
    expect(added.status.message).toEqual({ id: "markerAdded", name: "マーカー 4" });

    const renaming = scene("markerRenaming").timeline;
    expect(renaming.renaming).toBe(2);
    expect(renaming.markers.find((m) => m.id === 2)!.name).toBe("Aメロ");

    const moving = scene("markerMoving").timeline;
    expect(moving.selected).toEqual({ kind: "marker", id: 2 });
    expect(moving.renaming).toBeNull();
  });
});

describe("status line text", () => {
  let target: HTMLElement;
  beforeEach(() => {
    app.reset();
    target = document.createElement("div");
    target.className = "app";
    document.body.append(target);
  });
  afterEach(() => target.remove());

  const says = (message: StatusMessage, base = scene("tracks6")) => {
    base.status.message = message;
    app.setState(base);
    const mounted = mount(StatusLine, { target });
    flushSync();
    const out = (target.querySelector(".message")?.textContent ?? "").replace(/\s+/g, " ").trim();
    unmount(mounted);
    return out;
  };

  it("the two in spec 6", () => {
    expect(says(scene("tempo").status.message!, scene("tempo"))).toBe(
      "5 小節目の変更をえらんでいます ・ 104.00 から 9 小節目の 120.00 へ、だんだん速く",
    );
    expect(says({ id: "markerAdded", name: "マーカー 4" })).toBe("マーカー 4 を足しました。名前を打って Enter");
  });

  it("the tempo at the song start begins a ramp (spec 3 94-10, spec 6)", () => {
    expect(says({ id: "tempoRampSelected", bar: 1, beat: 1, tempoBpm: 96, toBar: 5, toBeat: 1, toTempoBpm: 120 })).toBe(
      "曲の頭のテンポをえらんでいます ・ 96.00 から 5 小節目の 120.00 へ、だんだん速く",
    );
  });

  it("temporary text (B-92): a slowing ramp, a point that is not a ramp, a time signature flag, a point changed by tapping in the transport", () => {
    expect(says({ id: "tempoRampSelected", bar: 9, beat: 1, tempoBpm: 120, toBar: 13, toBeat: 3, toTempoBpm: 96 })).toBe(
      "9 小節目の変更をえらんでいます ・ 120.00 から 13 小節目の 3 拍目の 96.00 へ、だんだん遅く",
    );
    expect(says({ id: "tempoRampSelected", bar: 9, beat: 1, tempoBpm: 120, toBar: 13, toBeat: 1, toTempoBpm: 120 })).toBe(
      "9 小節目の変更をえらんでいます ・ 120.00",
    );
    expect(says({ id: "tempoPointSelected", bar: 5, beat: 1, tempoBpm: 120 })).toBe("5 小節目の変更をえらんでいます ・ 120.00");
    expect(says({ id: "meterChangeSelected", bar: 9, meterNum: 3, meterDen: 4 })).toBe("9 小節目の変更をえらんでいます ・ 3/4");
    expect(says({ id: "tempoPointSet", bar: 5, beat: 1, tempoBpm: 120 })).toBe("5 小節目のテンポを 120.00 にしました。");
    expect(says({ id: "tempoPointSet", bar: 1, beat: 1, tempoBpm: 96 })).toBe("曲の頭のテンポを 96.00 にしました。");
    expect(says({ id: "meterChangeSet", bar: 9, meterNum: 3, meterDen: 4 })).toBe("9 小節目の拍子を 3/4 にしました。");
    expect(says({ id: "meterChangeSet", bar: 1, meterNum: 6, meterDen: 8 })).toBe("曲の頭の拍子を 6/8 にしました。");
  });
});

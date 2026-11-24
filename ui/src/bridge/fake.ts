// A Transport with no Rust behind it: for tests, and for looking at the screen in a plain
// browser (`npm run dev`, `vite preview`). `?scene=<name>&scale=<100|125>` picks a fixed
// scene (see SCENES). It answers the few commands below; the band heights follow the same
// numbers as crates/meloet-core/src/layout.rs, but meloet-core stays the rule book.

import type { Transport } from "./session";
import { LANE_HEADER_LEN, LANE_KIND, LANE_POINT_LEN, LANE_VERSION } from "./lane";
import { CONTROL_CC, CONTROL_PITCH_BEND, NO_FROM, TAKE_HEADER_LEN, TAKE_KIND, TAKE_NOTE_LEN, TAKE_VERSION } from "./take";
import { TELEMETRY_HEADER_LEN, TELEMETRY_KIND, TELEMETRY_MIC_LEN, TELEMETRY_NO_VALUE, TELEMETRY_VERSION, type MicMeter, type TransportReading } from "./telemetry";
import { TICKS_PER_QUARTER as TPQ } from "../time/ticks";
import { FLUID, emptyLibrary, fullLibrary, lv2Key, row as pluginRow, vst3Key } from "./fakeLibrary";
import { FAKE_TABLES, FakeParamTables, SURGE_AUTOMATED, fakeParamCount } from "./fakeParams";
import { FAKE_SONG_BPM, fakeAudioClip, fakePeaks, fakeStretch, fakeVocalClip, steadyTempo, type FakeAudio } from "./fakeAudio";
import { advancePack, fakeClipsPanel, fakeSolo, fakeSounds, packAsking, packFailed, packFetching, reduceClips27, reducePack, withPack } from "./fakeRound8";
import { reduceRound10 } from "./fakeRound10";
import { reduceSections, withSections } from "./fakeSections";
import { FAKE_PATTERNS, reduceP4 } from "./fakeP4";
import { reduceUpdates } from "./fakeUpdates";
import { FakeTypedKeys, typedOn } from "./fakeTyped";
import { CLICK_VOLUME_MAX_DB, CLICK_VOLUME_MIN_DB, REPEAT_COUNT_MAX } from "./types";
import { fakeLoudnessOf } from "./fakeLoudness";
import { fakeMidiTaken } from "./fakeMidi";
import { reduceA7 } from "./fakeA7";
import { reduceA9 } from "./fakeA9";
import { fakeCompWindow, fakeTakes, reduceComp } from "./fakeComp";
import { fakeUserPreset, presetOf, reducePresets } from "./fakePresets";
import { FAKE_ARP_OFF, fakeKnobs, fakeSetSoundKnob, fakeSpectrum, languageFromUrl, reduceArp, reduceLanguage, reduceMonoListen, reduceSpectrum } from "./fakeWave3";
import { fakeFadeOut } from "./fakeFade";
import { fakeMuteSet, reduceMuteSets, withMuteSets, type FakeMuteSet } from "./fakeMuteSets";
import {
  CLIP_CENTS_MAX,
  TUNE_STRENGTH_MAX,
  CLIP_SEMITONES_MAX,
  EFFECT_PARAMS,
  FACTORY_PRESETS,
  EXPORT_FORMATS,
  EXPORT_MP3_DEFAULT_KBPS,
  EXPORT_MP3_KBPS,
  EXPORT_WAVE_BARS,
  INSTRUMENT_GROUPS,
  LATENCY_CLICKS,
  MUTE_SETS_MAX,
  PROTOCOL_VERSION,
  SOURCE_BPM_MAX,
  SOURCE_BPM_MIN,
  type AppState as WireState,
  type Arp,
  type AudioClipView,
  type AutoTarget,
  type SourceTempoView,
  type Band,
  type Change,
  type ClipView,
  type Command,
  type EffectKind,
  type EffectRef,
  type EndpointView,
  type Event,
  type ExportDialogView,
  type ExportFormat,
  type ExportRunView,
  type PackFailure,
  type HistoryRow,
  type HistoryView,
  type IndexRange,
  type InsertRef,
  type InsertView,
  type InstrumentKind,
  type KeyCount,
  type LaneName,
  type LaneRef,
  type LaneView,
  type LayoutView,
  type LibraryView,
  type LiveNote,
  type LiveNotesSnapshot,
  type MissingView,
  type PianoRollView,
  type PluginPlaceView,
  type PluginSlotRef,
  type PluginEffectCategory,
  type PluginState,
  type PluginView,
  type PluginWindowView,
  type PluginsView,
  type SelectedPlugin,
  type ServerMessage,
  type ShelfView as WireShelf,
  type SongDialogKind,
  type SongView as WireSong,
  type SoundName,
  type SoundRef,
  type SoundView,
  type ArrangeView,
  type StatusMessage,
  type EffectName,
  type NamedInsert,
  type ParamValues,
  type StoppedPluginView,
  type StripRef,
  type Theme,
  type TimelineView,
  type TrackRef,
  type TrackView as WireTrack,
} from "./types";
import { TempoMap } from "../time/tempoMap";
import { THEME_IDS } from "../design/themes";

// The scenes and the commands below keep, next to the contract's fields, fields of their own that
// name only what comes with meloet (the instrument of a track, the effects in its slots, the row
// the shelf auditions…). `settleSong` and `settleShelf` bring the contract's fields in line with
// them. The screen reads only the contract's fields; these ride along on the state the fake holds.
type Inserts = [EffectKind | null, EffectKind | null, EffectKind | null, EffectKind | null];
interface OwnStrip {
  /** The effects that come with meloet, top first; `null` for an empty slot and for a plugin's. */
  inserts: Inserts;
}
export type FakeTrack = WireTrack & OwnStrip & { /** "sine" for an SF2 preset and for a plugin. */ instrument: InstrumentKind };
export type FakeSong = Omit<WireSong, "tracks" | "returns" | "master"> & {
  tracks: FakeTrack[];
  returns: [WireSong["returns"][0] & OwnStrip, WireSong["returns"][1] & OwnStrip];
  master: WireSong["master"] & OwnStrip;
  /** What the mute sets hold; `muteSets` is made from them (`withMuteSets`). */
  muteSetsKept?: FakeMuteSet[];
};
export type FakeShelf = WireShelf & {
  favorites: InstrumentKind[];
  selected: InstrumentKind | null;
  current: InstrumentKind | null;
  loading: InstrumentKind | null;
  sample: { instrument: InstrumentKind; startedUs: number; lengthUs: number } | null;
  selectedEffect: EffectKind | null;
};
export type FakeState = Omit<WireState, "song" | "shelf"> & { song: FakeSong; shelf: FakeShelf };
type AppState = FakeState;
type SongView = FakeSong;
type ShelfView = FakeShelf;
type TrackView = FakeTrack;

// ---- the band rules (a copy of the numbers in meloet-core/src/layout.rs) ----

const MENU = 24;
const TRANSPORT = 48;
const BAND_ROW = 24;
const STATUS = 24;
const METER_FULL = 52;
const METER_SLIM = 24;
const OVERVIEW = 40;
const ARRANGE_ROWS = 136;
const SMALLEST: Partial<Record<Band, number>> = { pianoRoll: 292, mixer: 516, history: 248 };
const LARGEST: Partial<Record<Band, number>> = { pianoRoll: 760, mixer: 660, history: 248 };
const BAND_TOOLS = 32;
const MAX_BOTTOM_OPEN = 2;
const MIN_WINDOW = { width: 1024, height: 640 };
const SCALES = [100, 125];
const BOTTOM_ORDER: Band[] = ["pianoRoll", "mixer", "history"];
const isBottom = (b: Band) => BOTTOM_ORDER.includes(b);

/** The smallest window in CSS px at this zoom. */
export function minWindow(scale: number): { width: number; height: number } {
  return { width: Math.ceil((MIN_WINDOW.width * scale) / 100), height: Math.ceil((MIN_WINDOW.height * scale) / 100) };
}

/** The zooms whose smallest window does not fit this screen space. 100 is never one of them. */
export function unfitScales(screen: { width: number; height: number }): number[] {
  return SCALES.slice(1).filter((s) => minWindow(s).width > screen.width || minWindow(s).height > screen.height);
}

export class FakeBands {
  /** Oldest first. */
  bottom: Band[] = [];
  /** Oldest first; the last one is in front. */
  side: Band[] = [];

  /** Window height in px at 100 %. */
  constructor(public height: number) {}

  private meterHeight(): number {
    return this.bottom.length === 0 ? METER_FULL : METER_SLIM;
  }

  /** What is left with the arrange view at 40 and every open band at its smallest. */
  private spare(): number {
    const fixed = MENU + TRANSPORT + this.meterHeight() + BAND_ROW + STATUS;
    return this.height - fixed - OVERVIEW - this.bottom.reduce((sum, b) => sum + (SMALLEST[b] ?? 0), 0);
  }

  /** Folds the older band when two do not fit, and returns it. The newest band is never folded. */
  private fit(): Band | null {
    return this.bottom.length > 1 && this.spare() < 0 ? this.bottom.shift()! : null;
  }

  /** Returns the band that was folded because the window got too low for both. */
  setHeight(height: number): Band | null {
    this.height = height;
    return this.fit();
  }

  /**
   * Returns the band folded to open this one, and why ("third": two at most; "tooLow": they do not
   * fit together). When two are folded, the later fold is returned.
   */
  toggle(band: Band): { band: Band; why: "third" | "tooLow" } | null {
    if (isBottom(band)) {
      const at = this.bottom.indexOf(band);
      if (at >= 0) this.bottom.splice(at, 1);
      else {
        this.bottom.push(band);
        const third = this.bottom.length > MAX_BOTTOM_OPEN ? this.bottom.shift()! : null;
        const low = this.fit();
        if (low) return { band: low, why: "tooLow" };
        return third ? { band: third, why: "third" } : null;
      }
    } else if (this.side.at(-1) === band) {
      this.side.pop();
    } else {
      this.side = [...this.side.filter((b) => b !== band), band];
    }
    return null;
  }

  goArrange(): void {
    this.bottom = [];
  }

  back(): void {
    if (this.side.pop() === undefined) this.bottom.pop();
  }

  layout(): LayoutView {
    let spare = this.spare();
    const heights = new Map<Band, number>();
    // Newest first: it grows first.
    for (const band of [...this.bottom].reverse()) {
      const smallest = SMALLEST[band] ?? 0;
      const grow = Math.max(0, Math.min(spare, (LARGEST[band] ?? smallest) - smallest));
      spare -= grow;
      heights.set(band, smallest + grow);
    }
    // A band that is open alone and still does not fit gets what is left.
    if (this.bottom.length === 1 && spare < 0) {
      const only = this.bottom[0]!;
      heights.set(only, Math.max(BAND_TOOLS, heights.get(only)! + spare));
    }
    const arrangeHeight = OVERVIEW + Math.max(0, spare);
    return {
      meter: this.meterHeight() === METER_FULL ? "full" : "slim",
      arrangeHeight,
      arrangeOverview: arrangeHeight < ARRANGE_ROWS,
      lower: BOTTOM_ORDER.filter((b) => heights.has(b)).map((band) => ({ band, height: heights.get(band)! })),
      rightOpen: [...this.side],
      rightFront: this.side.at(-1) ?? null,
      showBackHint: this.bottom.length > 0,
    };
  }
}

// ---- the scenes ----

const RYZEN = "Ryzen HD Audio Controller";
const GB206 = "GB206 High Definition Audio Controller";
const RADEON = "Radeon High Definition Audio Controller";

const HEADPHONES: EndpointView = {
  id: "out-headphones",
  name: "アナログヘッドフォン",
  friendly: "headphones",
  availability: "yes",
  link: "jack",
  nodeName: "alsa_output.pci-0000_0c_00.6.analog-stereo",
  device: RYZEN,
  unusable: null,
  channels: null,
};
const USB_AUDIO: EndpointView = {
  id: "out-usb",
  name: "USB オーディオ",
  friendly: null,
  availability: "unknown",
  link: "plug",
  nodeName: "alsa_output.usb-0d8c_USB_Audio-00.analog-stereo",
  device: "USB Audio Device",
  unusable: null,
  channels: null,
};
const LINE_OUT: EndpointView = {
  id: "out-line",
  name: "ライン出力",
  friendly: "lineOut",
  availability: "no",
  link: "jack",
  nodeName: HEADPHONES.nodeName,
  device: RYZEN,
  unusable: "switchedByPc",
  channels: null,
};
const HDMI: EndpointView = {
  id: "out-hdmi2",
  name: "HDMI / DisplayPort 2",
  friendly: "hdmi",
  availability: "no",
  link: "plug",
  nodeName: null,
  device: RADEON,
  unusable: "stoppedByPc",
  channels: null,
};
const OUTPUTS = [HEADPHONES, LINE_OUT, HDMI];
const hdmi = (id: string, name: string, device: string): EndpointView => ({ ...HDMI, id, name, device });
/**
 * The outputs of a PC with two cards of HDMI / DisplayPort, in the order meloet-core gives:
 * the usable ones, then the 8 that cannot be chosen.
 */
const PC_OUTPUTS = [
  HEADPHONES,
  USB_AUDIO,
  LINE_OUT,
  hdmi("out-hdmi-a1", "HDMI / DisplayPort", GB206),
  hdmi("out-hdmi-a2", "HDMI / DisplayPort 2", GB206),
  hdmi("out-hdmi-a3", "HDMI / DisplayPort 3", GB206),
  hdmi("out-hdmi-a4", "HDMI / DisplayPort 4", GB206),
  hdmi("out-hdmi-b1", "HDMI / DisplayPort", RADEON),
  hdmi("out-hdmi-b2", "HDMI / DisplayPort 2", RADEON),
  hdmi("out-hdmi-b3", "HDMI / DisplayPort 3", RADEON),
];

/** The voice inputs of the same PC: the rear mic, then the 2 that cannot be chosen. */
const MIC: EndpointView = {
  id: "in-rear-mic",
  name: "リアマイクロフォン",
  friendly: "rearMic",
  availability: "yes",
  link: "jack",
  nodeName: "alsa_input.pci-0000_0c_00.6.analog-stereo",
  device: RYZEN,
  unusable: null,
  channels: null,
};
const PC_INPUTS = [
  MIC,
  { ...MIC, id: "in-front-mic", name: "フロントマイクロフォン", friendly: null, availability: "no", unusable: "switchedByPc" },
  { ...MIC, id: "in-line", name: "ライン入力", friendly: null, availability: "no", unusable: "stoppedByPc" },
] satisfies EndpointView[];

const keyboard = (name: string, n = 1) => ({ id: `kb-${n}`, name, portName: name, address: `${23 + n}:0` });

const PIANO: TrackRef = { number: 1, name: "ピアノ" };
const NEW_SONG: StatusMessage = { id: "newSong", tempoBpm: 96, meterNum: 4, meterDen: 4, track: PIANO };

const FAKE_HOME = "/home/yachiyo";
const place = (shownPath: string, added = false): PluginPlaceView => ({ path: shownPath.replace(/^~/, FAKE_HOME), shownPath, added });

/** The usual places of Linux for LV2 and VST3 (protocol 16): no scan running, nothing stopped. */
export function fakePlugins(): PluginsView {
  return {
    scan: { state: "idle" },
    stopped: [],
    places: ["~/.lv2", "/usr/lib/lv2", "/usr/local/lib/lv2", "~/.vst3", "/usr/lib/vst3", "/usr/local/lib/vst3"].map((p) => place(p)),
    stoppedOpen: false,
    placesOpen: false,
  };
}

// The rows of the mock F5-ScanStopped. Its third row is a CLAP plugin (~/.clap/TapeWobble.clap); CLAP is not read in round 5
// (docs/architecture.md 20-9), so that row is an LV2 bundle here. Tape Wobble is a whole bundle that could not be read (id null);
// the others are one plugin each of their bundles.
const stoppedPlugin = (
  name: string,
  format: StoppedPluginView["format"],
  reason: StoppedPluginView["reason"],
  shownPath: string,
  id: string | null,
): StoppedPluginView => ({
  name,
  format,
  reason,
  path: shownPath.replace(/^~/, FAKE_HOME),
  id,
  shownPath,
  retrying: false,
});
const STOPPED_PLUGINS: StoppedPluginView[] = [
  stoppedPlugin("Old Chorus", "vst3", "crashed", "~/.vst3/OldChorus.vst3", "4F4C4443484F52555330303030303031"),
  stoppedPlugin("Granular Fog", "lv2", "timedOut", "/usr/lib/lv2/granfog.lv2", "urn:granfog:fog"),
  stoppedPlugin("Tape Wobble", "lv2", "broken", "~/.lv2/TapeWobble.lv2", null),
  stoppedPlugin("Brick EQ 1.0", "vst3", "crashed", "/usr/lib/vst3/BrickEQ.vst3", "4252494B455131303030303030303031"),
];

/** One clip selected (and shown in the piano roll), or none; nothing to paste. */
export function arrangeOf(clip: number | null): ArrangeView {
  return { selectedClips: clip === null ? [] : [clip], selectedClip: clip, pianoRollClip: clip, clipboardClips: 0, tallTrack: null };
}

/** Every instrument, in the order of the shelf: the fake has them all, so that the mocks can be drawn. */
const ALL_INSTRUMENTS: InstrumentKind[] = INSTRUMENT_GROUPS.flatMap((g) => [...g.instruments]);

/** The shelf as it is while closed, and when opened with 「＋ トラックを追加」 before a row is picked. */
function closedShelf(): ShelfView {
  return {
    mode: { kind: "newTrack" },
    available: [...ALL_INSTRUMENTS],
    favorites: ["grandPiano", "studioKit"],
    selected: null,
    current: null,
    loading: null,
    sample: null,
    selectedEffect: null,
    selectedSound: null,
    currentSound: null,
    loadingSound: null,
    soundSample: null,
    chosenEffect: null,
    currentEffect: null,
    unreadable: null,
    favoriteSounds: [builtinSound("grandPiano"), builtinSound("studioKit")],
    favoriteEffects: [builtinEffect("reverb"), builtinEffect("delay")],
    missing: [],
    notYet: [],
    patterns: [...FAKE_PATTERNS],
    selectedPattern: null,
    patternSample: null,
  };
}

type Slots = TrackView["slots"];
const noInserts = (): Inserts => [null, null, null, null];
const firstInsert = (effect: EffectKind): Inserts => [effect, null, null, null];

function builtinSound(instrument: InstrumentKind): Extract<SoundRef, { kind: "builtin" }> {
  return { kind: "builtin", instrument };
}
function builtinEffect(effect: EffectKind): EffectRef {
  return { kind: "builtin", effect };
}

/** An insert that has no id yet: `settleSong` gives it one. */
const NO_INSERT_ID = 0;

/** A song has at most this many tracks (`MAX_TRACKS` in meloet-core/src/play/seats.rs). */
const MAX_TRACKS = 64;

/**
 * The slots of a strip that holds only effects that come with meloet, none passed by. The inserts get the ids
 * `firstId`, `firstId` + 1, …; without it they get theirs when the song is settled.
 */
export function builtinSlots(inserts: Inserts, firstId = NO_INSERT_ID): Slots {
  let id = firstId;
  return inserts.map((effect): InsertView | null =>
    effect === null ? null : { id: firstId === NO_INSERT_ID ? NO_INSERT_ID : id++, effect: { kind: "builtin", effect }, bypassed: false },
  ) as Slots;
}

/** The counts of `NoteSelection.names`: each key once, lowest first, five at most. */
export function keyCounts(keys: number[]): KeyCount[] {
  const counts = new Map<number, number>();
  for (const k of keys) counts.set(k, (counts.get(k) ?? 0) + 1);
  return [...counts].sort((a, b) => a[0] - b[0]).slice(0, 5).map(([key, count]) => ({ key, count }));
}

// The three functions below bring the contract's fields (`sound`, `slots`, `selectedSound`…) in line
// with the fake's own (`instrument`, `inserts`, the shelf's `selected`…), and leave alone what names
// an SF2 preset or a plugin.

function settleSlots(inserts: Inserts, slots: Slots): Slots {
  return inserts.map((effect, i): InsertView | null => {
    const had = slots[i] ?? null;
    if (effect === null) return had?.effect.kind === "plugin" ? had : null;
    return had?.effect.kind === "builtin" && had.effect.effect === effect ? had : { id: NO_INSERT_ID, effect: { kind: "builtin", effect }, bypassed: false };
  }) as Slots;
}

/** Gives the inserts that have no id one above every id of the song, in the order of the strips. */
function giveInsertIds(song: SongView): SongView {
  const strips = [...song.tracks, ...song.returns, song.master];
  let next = Math.max(0, ...strips.flatMap((s) => s.slots.map((i) => i?.id ?? 0))) + 1;
  const give = <T extends { slots: Slots }>(s: T): T =>
    s.slots.some((i) => i?.id === NO_INSERT_ID) ? { ...s, slots: s.slots.map((i) => (i?.id === NO_INSERT_ID ? { ...i, id: next++ } : i)) as Slots } : s;
  const tracks = song.tracks.map(give);
  return { ...song, tracks, returns: [give(song.returns[0]), give(song.returns[1])], master: give(song.master) };
}

function settleSong(song: SongView): SongView {
  const strip = <T extends { inserts: Inserts; slots: Slots }>(s: T): T => ({ ...s, slots: settleSlots(s.inserts, s.slots) });
  // The keys go to the selected track, or while a vocal track is selected stay on the keys track they were on (B-94-1).
  const keys = (id: number | null) => song.tracks.find((t) => t.id === id && t.kind.kind === "keys")?.id ?? null;
  return giveInsertIds({
    ...song,
    keysTarget: keys(song.playTarget) ?? keys(song.keysTarget),
    tracks: song.tracks.map((t) => {
      const own = t.sound.kind === "builtin" || t.instrument !== "sine";
      return strip({
        ...t,
        sound: own ? builtinSound(t.instrument) : t.sound,
        standIn: own && t.standIn !== "packMissing" ? null : t.standIn,
        drumKeys: own ? null : t.drumKeys,
      });
    }),
    returns: [strip(song.returns[0]), strip(song.returns[1])],
    master: strip(song.master),
  });
}

function settleShelf(shelf: ShelfView): ShelfView {
  const sound = (own: InstrumentKind | null, any: SoundRef | null): SoundRef | null =>
    own !== null ? builtinSound(own) : any?.kind === "builtin" ? null : any;
  return {
    ...shelf,
    selectedSound: sound(shelf.selected, shelf.selectedSound),
    currentSound: sound(shelf.current, shelf.currentSound),
    loadingSound: sound(shelf.loading, shelf.loadingSound),
    soundSample: shelf.sample
      ? { sound: builtinSound(shelf.sample.instrument), playing: { startedUs: shelf.sample.startedUs, lengthUs: shelf.sample.lengthUs } }
      : shelf.soundSample?.sound.kind === "builtin"
        ? null
        : shelf.soundSample,
    chosenEffect: shelf.selectedEffect !== null ? builtinEffect(shelf.selectedEffect) : shelf.chosenEffect?.kind === "builtin" ? null : shelf.chosenEffect,
    favoriteSounds: [...shelf.favorites.map(builtinSound), ...shelf.favoriteSounds.filter((s) => s.kind !== "builtin")],
    patterns: shelf.mode.kind === "newTrack" ? [...FAKE_PATTERNS] : [],
    selectedPattern: shelf.mode.kind === "newTrack" ? shelf.selectedPattern : null,
    patternSample: shelf.sample || shelf.soundSample ? null : shelf.patternSample,
  };
}

/** The values of an effect that is new, in the order of its table. */
export const effectDefaults = (effect: EffectKind): number[] => EFFECT_PARAMS[effect].map((p) => p.default);

/** The same in a slot of `strip`: a reverb or a delay on a track or on the master starts with its mix at 30 % (`Insert::new_on`). */
export const insertDefaults = (effect: EffectKind, strip: StripRef): number[] =>
  EFFECT_PARAMS[effect].map((p) => (p.id === "mix" && (effect === "reverb" || effect === "delay") && strip.kind !== "return" ? 30 : p.default));

/** What a new track sends to return A and B (spec 3, Y9). */
const newTrackSends = (instrument: InstrumentKind): TrackView["sends"] => {
  const group = INSTRUMENT_GROUPS.find((g) => g.instruments.includes(instrument))!.group;
  return [group === "bass" ? null : group === "drums" ? -24 : -15, null];
};

/** The returns and the master of a new song: a reverb on A, a delay on B, a limiter on the master (spec 3, Y9). */
function newSongMixer(): Pick<SongView, "returns" | "master"> {
  return {
    returns: [
      { gainDb: 0, pan: 0, muted: false, inserts: firstInsert("reverb"), slots: builtinSlots(firstInsert("reverb"), 1) },
      { gainDb: 0, pan: 0, muted: false, inserts: firstInsert("delay"), slots: builtinSlots(firstInsert("delay"), 2) },
    ],
    master: { gainDb: 0, pan: 0, inserts: firstInsert("limiter"), slots: builtinSlots(firstInsert("limiter"), 3), lanes: [] },
  };
}

export function fakeState(): AppState {
  const bands = new FakeBands(800);
  return {
    song: {
      serial: 1,
      name: "はじめての曲",
      undoCount: 0,
      redoCount: 0,
      noteCount: 0,
      tracks: [
        {
          id: 1,
          number: 1,
          name: "ピアノ",
          color: 1,
          instrument: "grandPiano",
          kind: { kind: "keys" },
          sound: builtinSound("grandPiano"),
          standIn: null,
          drumKeys: null,
          gainDb: 0,
          pan: 0,
          sends: newTrackSends("grandPiano"),
          sendDefaults: newTrackSends("grandPiano"),
          inserts: noInserts(),
          slots: builtinSlots(noInserts()),
          muted: false,
          soloed: false,
          lanes: [],
          clips: [],
          arp: FAKE_ARP_OFF,
        },
      ],
      ...newSongMixer(),
      playTarget: 1,
      keysTarget: 1,
      loopRange: null,
      loopOn: false,
      file: "neverSaved",
      muteSets: [],
    },
    timeline: {
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
    },
    automation: { folded: [], armed: [], writing: [], held: [], selected: null },
    history: { total: 0, redo: 0, current: 0, newest: 0 },
    transport: {
      state: "stopped",
      position: 0,
      metronome: false,
      countInBars: 1,
      countInBeats: 4,
      canPlay: false,
      canRecord: false,
      canKeepPlayed: false,
      recording: null,
      recordingVocals: [],
      recordingFaults: null,
      overdub: false,
      clickVolumeDb: 0,
      clickSound: "beep",
      clickWhen: "always",
    },
    arrange: arrangeOf(null),
    pianoRoll: {
      take: null,
      selectedNotes: [],
      selectedControls: [],
      grid: { division: 16, triplet: false },
      scale: { root: 0, mode: "major", lock: false, highlight: true },
      quantize: { division: 16, triplet: false, strength: 100, swing: 0, align: "start" },
      clipboardNotes: 0,
    },
    workspace: {
      scale: 100,
      theme: "dark",
      viewportWidth: 1280,
      viewportHeight: 800,
      unfitScales: [],
      layout: bands.layout(),
      settings: null,
      chooser: null,
      hint: null,
      songDialog: null,
      recovery: null,
      fileDrop: null,
      latencyDialog: null,
      noteGlow: true,
      spectrum: true,
      monoListen: false,
      language: "ja",
    },
    shelf: closedShelf(),
    library: emptyLibrary(),
    params: { selected: null, clips: null },
    plugins: fakePlugins(),
    windows: { list: [] },
    presets: { user: [] },
    keyboards: { list: [], active: null, lost: null, inputStopped: false, velocityFixed: { on: false, value: 100 }, typedKeys: false },
    outputs: { list: OUTPUTS.map((o) => ({ ...o })), chosen: null, chosenState: null },
    inputs: { list: [], chosen: null, open: false },
    audio: {
      running: false,
      system: "PipeWire",
      sampleRate: null,
      requestedFrames: 256,
      actualFrames: null,
      forcedQuantum: null,
      latencyMs: null,
      latencyMsAtRequested: null,
      outputDevice: null,
      midiInput: null,
      bufferChoices: [64, 128, 256, 512, 1024],
      requestedSampleRate: 48000,
      sampleRateChoices: [44100, 48000, 88200, 96000],
      recordFormat: "bits24",
      latency: { reportedMs: null, measuredMs: null, manualMs: null, used: "reported", usedMs: null },
    },
    audioWork: { jobs: [], bounce: null },
    export: { dialog: null, run: null },
    sounds: fakeSounds(),
    aiLink: { on: false, port: "closed" },
    recent: [],
    updates: { check: true, newVersion: null },
    status: { message: NEW_SONG },
  };
}

/** What the meters show in a scene. Sent again and again, so the WebGL side has something to draw. */
export interface FakeMeters {
  keys?: number[];
  velocity?: number;
  sustain?: number;
  modulation?: number;
  pitchBend?: number;
  loadPermille?: number;
  /** 1.0 = 0 dBFS */
  master?: [number, number];
  /** Return A, then return B. */
  returns?: [[number, number], [number, number]];
  tracks?: [number, number][];
  /** The inputs of the vocal tracks that are metered. */
  mics?: MicMeter[];
  /** Where the listener is when the scene starts. It moves by `ticksPerUs`; scenes that stand for a mock use 0. */
  position?: TransportReading;
  /** M2: dB, in 0.1 steps. */
  masterReductionDb?: number;
  /** E7: 24 bands, 0..255 each. */
  spectrum?: number[];
  /** M3; none: null. */
  lufsShort?: number | null;
  lufsIntegrated?: number | null;
  truePeakDb?: number | null;
}

interface Scene {
  state: AppState;
  bands: FakeBands;
  meters: FakeMeters;
  /** Keep every scrolling box (`data-gl-scroll`) scrolled to its end. */
  scrollToEnd?: boolean;
  live: LiveNotesSnapshot;
  /** The rows of the history band, oldest first; rows past its end are made up (`fakeHistoryRows`). */
  history: HistoryRow[];
  /** Sent on the messages Channel once the screen listens (where files are held over the window). */
  events: Event[];
}

interface SceneDef {
  setup(s: AppState): void;
  bands?: Band[];
  meters: FakeMeters;
  scrollToEnd?: boolean;
  /** The zoom the scene starts at, when the URL gives none. */
  scale?: number;
  /** What the screen can give to the inside of the window, in CSS px. */
  screen?: { width: number; height: number };
  /** The status line after the bands are open (a notice about what just happened). */
  after?(s: AppState): void;
  /** The notes of the recording going on. */
  live?: LiveNotesSnapshot;
  /** The rows of the history band, oldest first. */
  history?: HistoryRow[];
  events?: Event[];
}

const dbfs = (db: number) => 10 ** (db / 20);

function withKeyboard(s: AppState, name = "Keystation 49 MK3"): void {
  const kb = keyboard(name);
  s.keyboards = { ...s.keyboards, list: [kb], active: kb.id, lost: null, inputStopped: false };
  s.audio.midiInput = kb.portName;
}

function withOutput(s: AppState): void {
  s.outputs.chosen = HEADPHONES.id;
  s.outputs.chosenState = "ready";
  s.transport.canPlay = true;
  s.transport.canRecord = s.song.playTarget !== null;
  Object.assign(s.audio, {
    running: true,
    sampleRate: 48000,
    actualFrames: 256,
    latencyMs: 13.5,
    latencyMsAtRequested: 13.5,
    outputDevice: HEADPHONES.nodeName,
  });
}

/** The second one was plugged in last; the first one was played last. */
function twoKeyboards(s: AppState): void {
  ready(s);
  const second = keyboard("microKEY2-37", 2);
  s.keyboards.list.push(second);
}

function ready(s: AppState): void {
  withKeyboard(s);
  withOutput(s);
  s.status.message = NEW_SONG;
}

function bufferFixed(s: AppState): void {
  ready(s);
  Object.assign(s.audio, { forcedQuantum: 1024, actualFrames: 1024, latencyMs: 53, latencyMsAtRequested: 13.5 });
  s.status.message = { id: "bufferFixed", actual: 1024, requested: 256, latencyMs: 53, latencyMsAtRequested: 13.5 };
}

function settings(s: AppState): void {
  ready(s);
  s.outputs.list = PC_OUTPUTS.map((o) => ({ ...o }));
  s.inputs.list = PC_INPUTS.map((i) => ({ ...i }));
  s.workspace.settings = { draftOutput: HEADPHONES.id, advancedOpen: false };
  s.status.message = { id: "settingsOpen" };
}

function settingsAdvFixed(s: AppState): void {
  bufferFixed(s);
  s.outputs.list = PC_OUTPUTS.map((o) => ({ ...o }));
  s.workspace.settings = { draftOutput: HEADPHONES.id, advancedOpen: true };
}

const QUIET: FakeMeters = { velocity: 83, loadPermille: 90 };

/** The arrange of the mock F-Takes, 3 steps in, never saved. */
function changedSong(s: AppState): void {
  ready(s);
  s.song.undoCount = 3;
  s.song.file = "changed";
  s.song.tracks[0]!.clips = [fakeClip(1, "イントロ", 1, 5, 1, [2]), fakeClip(3, "Aメロ", 5, 9, 2, [5, 6, 7])];
  s.transport.position = 4 * BAR;
  s.status.message = { id: "summary", loopBars: null, nothingSelected: true, notes: null };
}

function songDialog(kind: SongDialogKind, title = "はじめての曲", previous = false) {
  return (s: AppState): void => {
    changedSong(s);
    s.workspace.songDialog = { id: 1, kind, title, previous };
  };
}

// ---- takes (scenes recording, takes) ----

/** Ticks in a bar of 4/4. */
const BAR = 4 * TPQ;

export interface FakeTake {
  id: number;
  length: number;
  /** The take a quantized take was made from. */
  origin?: number;
  /** `from`: the index of the note of the origin take it came from. */
  notes: { start: number; length: number; key: number; velocity: number; from?: number }[];
  /** In order of tick. `number` 0 for pitch bend. */
  controls?: { tick: number; kind: typeof CONTROL_CC | typeof CONTROL_PITCH_BEND; number: number; value: number }[];
}

/** Bytes of one control in the take binary (docs/architecture.md 3-4). */
const FAKE_CONTROL_LEN = 8;

/** A made-up phrase of `count` notes over `bars` bars; the same seed gives the same notes. */
function phrase(id: number, bars: number, count: number, seed: number): FakeTake {
  const length = bars * BAR;
  const step = Math.floor(length / count);
  const notes = Array.from({ length: count }, (_, i) => {
    const r = (seed * 7919 + i * 104729) % 97;
    return { start: i * step, length: Math.max(TPQ / 8, step - TPQ / 12 - ((r % 3) * TPQ) / 8), key: 55 + ((r * 5 + i * 3) % 24), velocity: 60 + (r % 50) };
  });
  return { id, length, notes };
}

/** Clip and take ids come from one counter, as in the song model. */
const FAKE_TAKES = new Map(
  [phrase(2, 4, 23, 1), phrase(4, 2, 12, 2), phrase(5, 4, 19, 3), phrase(6, 4, 23, 4), phrase(7, 4, 17, 5)].map((t) => [t.id, t]),
);

function fakeClip(id: number, name: string, fromBar: number, toBar: number, take: number, takeIds: number[]): ClipView {
  const takeId = takeIds[take - 1]!;
  return {
    id,
    name,
    start: (fromBar - 1) * BAR,
    end: (toBar - 1) * BAR,
    take,
    takes: takeIds.length,
    takeId,
    takeOffset: 0,
    noteCount: FAKE_TAKES.get(takeId)!.notes.length,
    audio: null,
    comp: [],
  };
}

/** Writes the take binary (docs/architecture.md 3-4). */
export function encodeTake(t: FakeTake): ArrayBuffer {
  const controls = t.controls ?? [];
  const buf = new ArrayBuffer(TAKE_HEADER_LEN + TAKE_NOTE_LEN * t.notes.length + FAKE_CONTROL_LEN * controls.length);
  const v = new DataView(buf);
  v.setUint8(0, TAKE_KIND);
  v.setUint8(1, TAKE_VERSION);
  v.setUint32(4, t.id, true);
  v.setUint32(8, t.length, true);
  v.setUint32(12, t.notes.length, true);
  v.setUint32(16, controls.length, true);
  v.setUint32(20, t.origin ?? 0, true);
  t.notes.forEach((n, i) => {
    const p = TAKE_HEADER_LEN + TAKE_NOTE_LEN * i;
    v.setUint32(p, n.start, true);
    v.setUint32(p + 4, n.length, true);
    v.setUint8(p + 8, n.key);
    v.setUint8(p + 9, n.velocity);
    v.setUint32(p + 12, n.from ?? NO_FROM, true);
  });
  controls.forEach((c, i) => {
    const p = TAKE_HEADER_LEN + TAKE_NOTE_LEN * t.notes.length + FAKE_CONTROL_LEN * i;
    v.setUint32(p, c.tick, true);
    v.setUint8(p + 4, c.kind);
    v.setUint8(p + 5, c.number);
    v.setInt16(p + 6, c.value, true);
  });
  return buf;
}

const NO_LIVE: LiveNotesSnapshot = { seq: 0, recording: null, notes: [] };

/** 「11 月 15 日 22:41」 in the PC's own time, as the mocks F-Recover1 and Opt3-Recover show it. */
const RECOVERY_WRITTEN = new Date(2026, 10, 15, 22, 41).getTime();

/** Recording from bar 5; the listener is at 7.3.240, with C4 E4 G4 held since 7.3.000. */
const RECORDING_LIVE: LiveNotesSnapshot = {
  seq: 14,
  recording: 1,
  notes: [
    ...Array.from({ length: 11 }, (_, i): LiveNote => {
      const r = (i * 37) % 11;
      return { lap: 0, index: i, key: 57 + ((r * 7) % 19), velocity: 70 + r * 3, start: 4 * BAR + (i * 11 * TPQ) / 12, length: (5 * TPQ) / 12 + (r * TPQ) / 24 };
    }),
    ...[60, 64, 67].map((key, i): LiveNote => ({ lap: 0, index: 11 + i, key, velocity: 87, start: 6 * BAR + 2 * TPQ, length: null })),
  ],
};

// ---- the piano roll (mocks A-PianoRoll, F-PR720, F-PR150, F-WidePR, F-PRPlay, F-PRRec) ----

const NOTE_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
/** The mocks write the tick of a position or a length as 480 to a beat. */
const MOCK_TICKS_PER_BEAT = 480;
/** "3.1.360" (bar . beat . tick, 4/4) to ticks from the song's start. */
const at = (text: string) => {
  const [bar, beat, tick] = text.split(".").map(Number) as [number, number, number];
  return (bar - 1) * BAR + (beat - 1) * TPQ + (tick * TPQ) / MOCK_TICKS_PER_BEAT;
};
/** "1.240" (beats . ticks) to ticks. */
const span = (text: string) => {
  const [beats, ticks] = text.split(".").map(Number) as [number, number];
  return beats * TPQ + (ticks * TPQ) / MOCK_TICKS_PER_BEAT;
};
/** "D#4" to the MIDI number (C4 = 60). */
const key = (text: string) => NOTE_NAMES.indexOf(text.slice(0, -1)) + 12 * (Number(text.slice(-1)) + 1);

/** The note list of the mocks (F-WidePR, left), as it is written there: position, key, length, Vel. */
const MOCK_NOTES: [string, string, string, number][] = [
  ["1.1.000", "C3", "4.000", 70],
  ["1.1.000", "G3", "4.000", 64],
  ["1.1.000", "E4", "1.000", 87],
  ["1.2.000", "G4", "1.000", 92],
  ["1.3.000", "C5", "1.240", 101],
  ["1.4.240", "G4", "0.240", 74],
  ["2.1.000", "F3", "4.000", 66],
  ["2.1.000", "A3", "4.000", 62],
  ["2.1.000", "A4", "1.000", 84],
  ["2.2.000", "G4", "0.240", 70],
  ["2.2.240", "F4", "0.240", 72],
  ["2.3.000", "E4", "2.000", 90],
  ["3.1.000", "G3", "4.000", 71],
  ["3.1.000", "B3", "4.000", 60],
  ["3.1.000", "D4", "0.360", 80],
  ["3.1.360", "E4", "0.360", 83],
  ["3.2.240", "F4", "0.240", 77],
  ["3.3.000", "G4", "2.000", 96],
  ["4.1.000", "C3", "4.000", 73],
  ["4.1.000", "E3", "4.000", 61],
  ["4.1.000", "G3", "4.000", 63],
  ["4.1.000", "C4", "3.000", 88],
  ["4.4.000", "E4", "0.360", 58],
];
/** D4 E4 F4 of bar 3: quantized, and selected. */
const QUANTIZED: IndexRange = { start: 14, end: 17 };
/** Where D4 E4 F4 were played, before they were quantized (read off the gray.7 frames of A-PianoRoll). */
const PLAYED_AT: Record<number, number> = { 14: at("3.1.036"), 15: at("3.1.312"), 16: at("3.2.276") };

const PLAYED_TAKE = 11;
const QUANTIZED_TAKE = 12;
FAKE_TAKES.set(PLAYED_TAKE, {
  id: PLAYED_TAKE,
  length: 4 * BAR,
  notes: MOCK_NOTES.map(([start, k, length, velocity], i) => ({
    start: PLAYED_AT[i] ?? at(start),
    length: span(length),
    key: key(k),
    velocity,
  })),
});
/**
 * The CC tags of A-PianoRoll: sustain pressed 5 times, 6 pitch bend points (shaped like R3-Lanes), no modulation;
 * and CC 7 for 「そのほかの CC」.
 */
const MOCK_CONTROLS: NonNullable<FakeTake["controls"]> = [
  ...[
    ["1.1.000", "1.4.240"],
    ["2.1.000", "2.4.240"],
    ["3.1.000", "3.2.240"],
    ["3.3.000", "3.4.240"],
    ["4.1.000", "4.4.240"],
  ].flatMap(([down, up]) => [
    { tick: at(down!), kind: CONTROL_CC, number: 64, value: 127 } as const,
    { tick: at(up!), kind: CONTROL_CC, number: 64, value: 0 } as const,
  ]),
  ...(
    [
      ["2.1.000", 0],
      ["2.1.240", 1536],
      ["2.2.000", 4096],
      ["2.3.000", 3072],
      ["2.4.000", 1024],
      ["2.4.240", 0],
    ] as const
  ).map(([tick, value]) => ({ tick: at(tick), kind: CONTROL_PITCH_BEND, number: 0, value }) as const),
  ...(
    [
      ["1.1.000", 100],
      ["3.1.000", 90],
      ["4.3.000", 110],
    ] as const
  ).map(([tick, value]) => ({ tick: at(tick), kind: CONTROL_CC, number: 7, value }) as const),
].sort((a, b) => a.tick - b.tick);

FAKE_TAKES.set(QUANTIZED_TAKE, {
  id: QUANTIZED_TAKE,
  length: 4 * BAR,
  origin: PLAYED_TAKE,
  controls: MOCK_CONTROLS,
  notes: MOCK_NOTES.map(([start, k, length, velocity], i) => ({
    start: at(start),
    length: span(length),
    key: key(k),
    velocity,
    from: i,
  })),
});
for (const t of [phrase(14, 4, 21, 6), phrase(15, 4, 19, 7)]) FAKE_TAKES.set(t.id, t);
const NO_NOTES_TAKE = 16;
FAKE_TAKES.set(NO_NOTES_TAKE, { id: NO_NOTES_TAKE, length: 4 * BAR, notes: [] });

/** The quantized take of A-PianoRoll with G4 written at 5.1.000, after the clip's end (B-82): the clip grew to bar 5. */
const GROWN_TAKE = 18;
FAKE_TAKES.set(GROWN_TAKE, {
  ...FAKE_TAKES.get(QUANTIZED_TAKE)!,
  id: GROWN_TAKE,
  length: 4 * BAR + BAR / 4,
  notes: [...FAKE_TAKES.get(QUANTIZED_TAKE)!.notes, { start: 4 * BAR, length: BAR / 4, key: key("G4"), velocity: 100 }],
});

/** The other tracks of F-WidePR (MIDI clips only: the fake cannot show the sung clips). */
const WIDE_TRACKS: { name: string; gainDb: number; clips: [string, number, number][] }[] = [
  { name: "ベース", gainDb: -7.5, clips: [["ベース A", 5, 9], ["ベース A", 9, 13], ["ベース サビ", 13, 17]] },
  { name: "ドラム", gainDb: -6.0, clips: [["フィル", 3, 5], ["8ビート", 5, 13], ["サビ ビート", 13, 17]] },
  { name: "パッド", gainDb: -12.0, clips: [["コード", 1, 5], ["コード サビ", 13, 17]] },
  { name: "ボーカル", gainDb: -3.2, clips: [] },
];

function pianoRollView(take: number, scaleLock: boolean): PianoRollView {
  return {
    take,
    selectedNotes: [{ ...QUANTIZED }],
    selectedControls: [],
    grid: { division: 16, triplet: false },
    scale: { root: 0, mode: "major", lock: scaleLock, highlight: true },
    quantize: { division: 16, triplet: false, strength: 75, swing: 0, align: "start" },
    clipboardNotes: 0,
  };
}

/** A-PianoRoll: the clip 「イントロ」 (bars 1–4, one take) in the piano roll, D4 E4 F4 selected, stopped at 3.1.000. */
function pianoRollSong(s: AppState): void {
  ready(s);
  s.song.undoCount = 14;
  s.song.file = "changed";
  s.song.tracks[0]!.clips = [fakeClip(10, "イントロ", 1, 5, 1, [QUANTIZED_TAKE])];
  s.song.noteCount = MOCK_NOTES.length;
  s.arrange = arrangeOf(10);
  s.pianoRoll = pianoRollView(QUANTIZED_TAKE, true);
  s.transport.position = at("3.1.000");
  s.status.message = {
    id: "summary",
    loopBars: null,
    nothingSelected: false,
    notes: { count: 3, keys: [key("D4"), key("E4"), key("F4")], quantized: true, names: keyCounts([key("D4"), key("E4"), key("F4")]) },
  };
}

function pianoRollWide(s: AppState): void {
  pianoRollSong(s);
  s.song.tracks[0]!.gainDb = -4.8;
  let id = 20;
  WIDE_TRACKS.forEach((t, i) => {
    const clips = t.clips.map(([name, from, to]) => {
      const take = phrase(++id, to - from, 4 * (to - from), id);
      FAKE_TAKES.set(take.id, take);
      return fakeClip(++id, name, from, to, 1, [take.id]);
    });
    s.song.tracks.push({ ...s.song.tracks[0]!, id: i + 2, number: i + 2, name: t.name, color: i + 2, gainDb: t.gainDb, clips });
  });
}

/** F-PRPlay and F-PRRec: the clip 「Aメロ」 (bars 1–4) with 3 takes, the second shown. */
function aMelody(s: AppState, takes: number[]): void {
  ready(s);
  s.song.undoCount = 14;
  s.song.file = "saved";
  s.song.tracks[0]!.clips = [fakeClip(13, "Aメロ", 1, 5, 2, takes)];
  s.song.noteCount = MOCK_NOTES.length;
  s.arrange = arrangeOf(13);
  s.pianoRoll = pianoRollView(QUANTIZED_TAKE, false);
  s.status.message = null;
}

/** F-PRRec: recording take 3 from bar 1; at 4.2.000, C3 E3 G3 held since 4.1.000 and C4 since 4.1.120. */
const PIANO_ROLL_LIVE: LiveNotesSnapshot = {
  seq: 22,
  recording: 1,
  notes: [
    ...MOCK_NOTES.slice(0, 18).map(([start, k, length, velocity], index): LiveNote => ({
      lap: 0,
      index,
      key: key(k),
      velocity,
      start: at(start),
      length: span(length),
    })),
    ...(
      [
        ["4.1.000", "C3"],
        ["4.1.000", "E3"],
        ["4.1.000", "G3"],
        ["4.1.120", "C4"],
      ] as const
    ).map(([start, k], i): LiveNote => ({ lap: 0, index: 18 + i, key: key(k), velocity: 88, start: at(start), length: null })),
  ],
};

const PIANO_ROLL_PLAYING: FakeMeters = {
  velocity: 83,
  loadPermille: 160,
  master: [dbfs(-7.1), dbfs(-7.6)],
  tracks: [[dbfs(-7.1), dbfs(-7.6)]],
  position: { tick: at("4.1.240"), ticksPerUs: 0, state: "playing", countInBeatsLeft: 0, lap: 0 },
};

/**
 * R3-Arrange: two tracks. The piano has 「イントロ」 (bars 1–4) and 「Aメロ」 with its first bar trimmed off (bars 6–8);
 * the bass has 「ベース A」 (bars 5–8) and a copy of it trimmed to 2.5 bars (bars 14–16.2), selected. The trimmed notes are not drawn.
 */
function arrangeTrimmed(s: AppState): void {
  ready(s);
  const bass = phrase(40, 4, 11, 6);
  FAKE_TAKES.set(bass.id, bass);
  s.song.undoCount = 4;
  s.song.tracks[0]!.clips = [
    fakeClip(1, "イントロ", 1, 5, 1, [2]),
    { ...fakeClip(3, "Aメロ", 5, 9, 1, [5]), start: 5 * BAR, takeOffset: -BAR },
  ];
  s.song.tracks.push({
    ...s.song.tracks[0]!,
    id: 2,
    number: 2,
    name: "ベース",
    color: 2,
    gainDb: -7.5,
    clips: [fakeClip(41, "ベース A", 5, 9, 1, [40]), { ...fakeClip(42, "ベース A", 14, 18, 1, [40]), end: 13 * BAR + BAR * 2.5 }],
  });
  s.arrange = arrangeOf(42);
  s.status.message = null;
}

export const SCENES: Record<string, SceneDef> = {
  connect1: { setup: (s) => (s.status.message = { id: "searchingKeyboard" }), meters: { loadPermille: 90 } },
  connect2: {
    setup(s) {
      withKeyboard(s);
      s.outputs.list = PC_OUTPUTS.map((o) => ({ ...o }));
      s.workspace.chooser = { highlighted: HEADPHONES.id };
      s.status.message = { id: "keyboardFound", keyboard: "Keystation 49 MK3" };
    },
    meters: { keys: [60, 64, 67], velocity: 87, sustain: 127, loadPermille: 90 },
  },
  connect3: {
    setup(s) {
      withKeyboard(s);
      withOutput(s);
      s.workspace.hint = "soundCameOut";
      s.status.message = { id: "outputChosen", output: HEADPHONES.name, track: PIANO };
    },
    meters: {
      keys: [60, 64, 67],
      velocity: 87,
      sustain: 127,
      modulation: 12,
      loadPermille: 60,
      master: [dbfs(-8.4), dbfs(-9.1)],
      tracks: [[dbfs(-8.4), dbfs(-9.1)]],
    },
  },
  empty: { setup: (s) => (ready(s), (s.workspace.hint = "emptyArrange")), meters: QUIET },
  bufferFixed: { setup: bufferFixed, meters: QUIET },
  settings: { setup: settings, meters: QUIET },
  settingsAdvFixed: { setup: settingsAdvFixed, meters: QUIET },
  settingsEnd: { setup: settings, meters: QUIET, scrollToEnd: true },
  settingsAdvFixedEnd: { setup: settingsAdvFixed, meters: QUIET, scrollToEnd: true },
  twoKeyboards: { setup: twoKeyboards, meters: QUIET },
  longName: { setup: (s) => (ready(s), withKeyboard(s, "Arturia KeyLab Essential 61 mk3 MIDI 1")), meters: QUIET },
  pianoRoll: { setup: pianoRollSong, bands: ["pianoRoll"], meters: QUIET },
  // The same at a window of 1280x720: open it in a window of that size.
  pianoRoll720: { setup: pianoRollSong, bands: ["pianoRoll"], meters: QUIET },
  pianoRoll125: { setup: pianoRollSong, bands: ["pianoRoll"], meters: QUIET, scale: 125 },
  // Open it in a window of 1920x1080.
  pianoRollWide: { setup: pianoRollWide, bands: ["pianoRoll"], meters: QUIET },
  // A-PianoRoll with the pitch bend point at 2.2.000 selected, for the CC lanes (R3-Lanes). The lanes are opened by clicking their tags.
  pianoRollCc: {
    setup(s) {
      pianoRollSong(s);
      const point = MOCK_CONTROLS.findIndex((c) => c.kind === CONTROL_PITCH_BEND && c.tick === at("2.2.000"));
      s.pianoRoll.selectedControls = toRanges([point]);
    },
    bands: ["pianoRoll"],
    meters: QUIET,
  },
  pianoRollPlay: {
    setup(s) {
      aMelody(s, [14, QUANTIZED_TAKE, 15]);
      s.transport.state = "playing";
    },
    bands: ["pianoRoll"],
    meters: PIANO_ROLL_PLAYING,
  },
  // Round 12 E1 (no mock board): F-PRPlay playing at 1.3.240, where C3 G3 and C5 sound (E4 and G4 have just ended) and light in the track's color.
  pianoRollGlow: {
    setup(s) {
      aMelody(s, [14, QUANTIZED_TAKE, 15]);
      s.transport.state = "playing";
    },
    bands: ["pianoRoll"],
    meters: { ...PIANO_ROLL_PLAYING, position: { ...PIANO_ROLL_PLAYING.position!, tick: at("1.3.240") } },
  },
  // Takes 1 and 2 are in the clip; take 3 is being recorded.
  pianoRollRec: {
    setup(s) {
      aMelody(s, [14, QUANTIZED_TAKE]);
      s.transport.state = "recording";
      s.transport.recording = { id: 1, track: 1, from: 0, take: 3, clip: 13, lap: 0, overdub: false };
      s.status.message = { id: "recording", track: PIANO, take: 3 };
    },
    bands: ["pianoRoll"],
    meters: {
      ...PIANO_ROLL_PLAYING,
      keys: [key("C3"), key("E3"), key("G3"), key("C4")],
      velocity: 88,
      sustain: 127,
      position: { tick: at("4.2.000"), ticksPerUs: 0, state: "recording", countInBeatsLeft: 0, lap: 0 },
    },
    live: PIANO_ROLL_LIVE,
  },
  // The mock F-PREmpty: a new song with no clip, the piano roll open (spec 3: only the grid, and the hint at the top left).
  pianoRollEmpty: {
    setup: ready,
    bands: ["pianoRoll"],
    after: (s) => (s.status.message = { id: "summary", loopBars: null, nothingSelected: true, notes: null }),
    meters: QUIET,
  },
  // Not from a mock: A-PianoRoll after every note was deleted (the clip stays, its take has no note).
  pianoRollNoNotes: {
    setup(s) {
      pianoRollSong(s);
      s.song.tracks[0]!.clips = [fakeClip(10, "イントロ", 1, 5, 1, [NO_NOTES_TAKE])];
      s.song.noteCount = 0;
      s.pianoRoll = { ...s.pianoRoll, take: NO_NOTES_TAKE, selectedNotes: [], selectedControls: [] };
      s.status.message = { id: "summary", loopBars: null, nothingSelected: true, notes: null };
    },
    bands: ["pianoRoll"],
    meters: QUIET,
  },
  // Not from a mock (B-82): A-PianoRoll after G4 was written at 5.1.000; the clip grew to bars 1–5, the note is selected.
  pianoRollGrown: {
    setup(s) {
      pianoRollSong(s);
      s.song.undoCount = 15;
      s.song.tracks[0]!.clips = [fakeClip(10, "イントロ", 1, 6, 1, [GROWN_TAKE])];
      s.song.noteCount = MOCK_NOTES.length + 1;
      const last = MOCK_NOTES.length;
      s.pianoRoll = { ...s.pianoRoll, take: GROWN_TAKE, selectedNotes: [{ start: last, end: last + 1 }] };
      s.transport.position = 4 * BAR;
      s.status.message = { id: "summary", loopBars: null, nothingSelected: false, notes: { count: 1, keys: [key("G4")], quantized: false, names: keyCounts([key("G4")]) } };
    },
    bands: ["pianoRoll"],
    meters: QUIET,
  },
  // Not from a mock (B-82): the write tool after the clip, where it cannot grow over the next clip.
  notesNotWritten: {
    setup: pianoRollSong,
    bands: ["pianoRoll"],
    after: (s) => (s.status.message = { id: "notesNotWritten" }),
    meters: QUIET,
  },
  mixer: { setup: ready, bands: ["mixer"], meters: QUIET },
  history: { setup: ready, bands: ["history"], meters: QUIET },
  shelf: { setup: ready, bands: ["shelf"], meters: QUIET },
  params: { setup: ready, bands: ["params"], meters: QUIET },
  // Not from a mock: 16 tracks, every meter showing a level, to check that the WebGL contexts do not grow with the tracks.
  tracks16: {
    setup(s) {
      ready(s);
      s.song.tracks = Array.from({ length: 16 }, (_, i) => ({ ...s.song.tracks[0]!, id: i + 1, number: i + 1, color: (i % 5) + 1 }));
    },
    meters: {
      keys: [60, 64, 67],
      velocity: 87,
      loadPermille: 120,
      master: [dbfs(-6.2), dbfs(-7.0)],
      tracks: Array.from({ length: 16 }, (_, i): [number, number] => [dbfs(-3 - 2 * i), dbfs(-4 - 2 * i)]),
    },
  },
  // Not from a mock: the same 16 tracks with the mixer open. Over 10 tracks the mixer's meters are shown as a left and a
  // right part in turn (docs/architecture.md, chapter 7).
  mixer16: {
    setup(s) {
      ready(s);
      s.song.tracks = Array.from({ length: 16 }, (_, i) => ({ ...s.song.tracks[0]!, id: i + 1, number: i + 1, color: (i % 5) + 1 }));
    },
    bands: ["mixer"],
    meters: {
      loadPermille: 120,
      master: [dbfs(-6.2), dbfs(-7.0)],
      tracks: Array.from({ length: 16 }, (_, i): [number, number] => [dbfs(-3 - 2 * i), dbfs(-4 - 2 * i)]),
    },
  },
  // The mock F-ArrangeRec without the loop: recording to the piano from bar 5, over a clip with one take.
  recording: {
    setup(s) {
      ready(s);
      s.song.undoCount = 12;
      s.song.loopRange = { start: 4 * BAR, end: 8 * BAR };
      s.song.tracks[0]!.clips = [fakeClip(1, "イントロ", 1, 5, 1, [2]), fakeClip(3, "ピアノ", 5, 7, 1, [4])];
      s.transport.state = "recording";
      s.transport.position = 4 * BAR;
      s.transport.metronome = true;
      s.transport.recording = { id: 1, track: 1, from: 4 * BAR, take: 2, clip: 3, lap: 0, overdub: false };
      s.status.message = { id: "recording", track: PIANO, take: 2 };
    },
    meters: {
      keys: [60, 64, 67],
      velocity: 87,
      sustain: 127,
      modulation: 12,
      loadPermille: 140,
      master: [dbfs(-6.2), dbfs(-7.0)],
      tracks: [[dbfs(-6.2), dbfs(-7.0)]],
      position: { tick: 6 * BAR + 2 * TPQ + TPQ / 2, ticksPerUs: 0, state: "recording", countInBeatsLeft: 0, lap: 0 },
    },
    live: RECORDING_LIVE,
  },
  // The mock F-Connect4: a new song, the third beat of a one-bar count-in.
  countIn: {
    setup(s) {
      ready(s);
      s.transport.state = "countIn";
      s.transport.metronome = true;
      s.transport.recording = { id: 1, track: 1, from: 0, take: 1, clip: null, lap: 0, overdub: false };
      s.status.message = { id: "countingIn", beats: 4 };
    },
    meters: { ...QUIET, position: { tick: -(3 * TPQ) / 2, ticksPerUs: 0, state: "countIn", countInBeatsLeft: 2, lap: 0 } },
  },
  // The mock F-Takes: the clip 「Aメロ」 has 3 takes, shows the second, and is selected; the piano roll shows it.
  takes: {
    setup(s) {
      ready(s);
      s.song.undoCount = 3;
      s.song.tracks[0]!.clips = [fakeClip(1, "イントロ", 1, 5, 1, [2]), fakeClip(3, "Aメロ", 5, 9, 2, [5, 6, 7])];
      s.song.noteCount = FAKE_TAKES.get(6)!.notes.length;
      s.arrange = arrangeOf(3);
      s.transport.position = 4 * BAR;
      s.status.message = null;
    },
    bands: ["pianoRoll"],
    meters: { ...QUIET, position: { tick: 4 * BAR, ticksPerUs: 0, state: "stopped", countInBeatsLeft: 0, lap: 0 } },
  },
  arrangeTrimmed: { setup: arrangeTrimmed, meters: QUIET },
  // The same with the piano roll open on the trimmed clip: the notes trimmed off are not shown there either.
  arrangeTrimmedRoll: { setup: arrangeTrimmed, bands: ["pianoRoll"], meters: QUIET },
  // The mock 「拡大率をえらぶ所」: a 1200x700 screen, 125 % does not fit, 100 % is on.
  zoom: { setup: ready, scale: 100, screen: { width: 1200, height: 700 }, meters: QUIET },
  // The mixer was open, then the piano roll was opened in a window too low for both.
  bandFolded: {
    setup: ready,
    bands: ["mixer", "pianoRoll"],
    after: (s) => (s.status.message = { id: "bandFoldedToOpen", opened: "pianoRoll", folded: "mixer" }),
    meters: QUIET,
  },
  // The arrange of the mock F-Takes with no band open: the clip 「Aメロ」 (take 2 of 3) is selected.
  takesArrange: {
    setup(s) {
      ready(s);
      s.song.tracks[0]!.clips = [fakeClip(1, "イントロ", 1, 5, 1, [2]), fakeClip(3, "Aメロ", 5, 9, 2, [5, 6, 7])];
      s.arrange = arrangeOf(3);
      s.transport.position = 4 * BAR;
      s.status.message = null;
    },
    meters: QUIET,
  },
  // Not from a mock: the song dialogs (spec 2 「ダイアログ」, spec 6).
  unsaved: { setup: songDialog("unsaved"), meters: QUIET },
  cannotSave: { setup: songDialog("cannotSave"), meters: QUIET },
  broken: { setup: songDialog("broken", "はじめての曲.meloet", true), meters: QUIET },
  brokenNoPrevious: { setup: songDialog("broken", "はじめての曲.meloet"), meters: QUIET },
  // The mock F-Recover1: a new song never saved, brought back from the recovery file written as the PC ended.
  // Bringing it back is the first step ("1 / 1").
  recover1: {
    setup(s) {
      ready(s);
      s.song.undoCount = 1;
      s.song.file = "changed";
      s.song.tracks[0]!.clips = [fakeClip(1, "ピアノ", 1, 5, 1, [2]), fakeClip(3, "ピアノ", 5, 7, 1, [4])];
      s.workspace.recovery = { kind: "recovered", writtenMs: RECOVERY_WRITTEN, cause: "pcEnding", song: null, takes: 2, vocals: null };
      s.status.message = { id: "recovered" };
    },
    meters: QUIET,
  },
  // The mock F-Recover2: 「戻さない」 was pressed; the song is as last saved, and the history band is open.
  recover2: {
    setup(s) {
      changedSong(s);
      s.song.undoCount = 2;
      s.song.file = "saved";
      s.status.message = { id: "recoveryDiscarded" };
    },
    bands: ["history"],
    meters: QUIET,
  },
  // The mock F-Recover3: the recovery file did not read; the song last saved is open.
  recover3: {
    setup(s) {
      changedSong(s);
      s.song.undoCount = 0;
      s.song.file = "saved";
      s.workspace.recovery = { kind: "unreadable", song: "はじめての曲" };
      s.status.message = { id: "recoveryUnreadable", savedSong: true };
    },
    meters: QUIET,
  },
  // The mock R9-Scan: the first start, the shelf open, 37 of 112 bundles looked at (「スキャン中 37 / 112」).
  scanning: {
    setup(s) {
      ready(s);
      s.plugins.scan = { state: "running", done: 37, total: 112 };
    },
    bands: ["shelf"],
    meters: QUIET,
  },
  // R9-Scan's status line when that scan is over: 667 plugins in the shelf, 4 bundles stopped (「止まった 4 個を見る」).
  scanFinished: {
    setup(s) {
      ready(s);
      s.plugins.scan = { state: "finished", added: 667, stopped: 4 };
      s.plugins.stopped = structuredClone(STOPPED_PLUGINS);
    },
    bands: ["shelf"],
    after: (s) => (s.status.message = { id: "scanFinished", added: 667, stopped: 4 }),
    meters: QUIET,
  },
  // R9-Scan's status line from the second start on: 「新しいプラグインが 3 個見つかりました。…」
  pluginsFound: {
    setup(s) {
      ready(s);
      s.status.message = { id: "pluginsFound", count: 3 };
    },
    meters: QUIET,
  },
  // R9-Scan: 「音の設定 ▸ くわしい設定 ▸ プラグインの場所」, with the two places of the mock added by hand. The mock also
  // lists ~/.clap and /usr/lib/clap; CLAP is not read in round 5, so they are not here.
  pluginPlaces: {
    setup(s) {
      settings(s);
      s.workspace.settings = { draftOutput: HEADPHONES.id, advancedOpen: true };
      s.plugins.places.push(place("~/音楽/プラグイン", true), place("/opt/surge-xt/lib/vst3", true));
      s.plugins.placesOpen = true;
    },
    meters: QUIET,
    scrollToEnd: true,
  },
  // The mock F5-ScanStopped: the dialog 「スキャンで止まったプラグイン」 over the saved song.
  scanStopped: {
    setup(s) {
      changedSong(s);
      s.song.undoCount = 14;
      s.song.file = "saved";
      s.plugins.stopped = structuredClone(STOPPED_PLUGINS);
      s.plugins.stoppedOpen = true;
      s.status.message = { id: "scanStoppedOpen" };
    },
    meters: QUIET,
  },
  // The same with 「もう一度ためす」 pressed on the second row (「ためしています…」).
  scanStoppedRetrying: {
    setup(s) {
      ready(s);
      s.plugins.stopped = structuredClone(STOPPED_PLUGINS);
      s.plugins.stopped[1]!.retrying = true;
      s.plugins.stoppedOpen = true;
      s.status.message = { id: "scanStoppedOpen" };
    },
    meters: QUIET,
  },
  // Spec 6 「もう一度ためして入った」: the row is gone and the status line says so.
  scanRetryAdded: {
    setup(s) {
      ready(s);
      s.plugins.stopped = structuredClone(STOPPED_PLUGINS).filter((p) => p.name !== "Granular Fog");
      s.plugins.stoppedOpen = true;
      s.status.message = { id: "scanRetryAdded", name: "Granular Fog" };
    },
    meters: QUIET,
  },
  // Spec 6 「もう一度ためしても止まった」.
  scanRetryStopped: {
    setup(s) {
      ready(s);
      s.plugins.stopped = structuredClone(STOPPED_PLUGINS);
      s.plugins.stoppedOpen = true;
      s.status.message = { id: "scanRetryStopped", name: "Granular Fog" };
    },
    meters: QUIET,
  },
  // Spec 6 「スキャンが終わった」 with nothing stopped: only the first half of the sentence (docs/history/design-notes.ja.md A-16).
  scanFinishedNone: {
    setup(s) {
      ready(s);
      s.plugins.scan = { state: "finished", added: 667, stopped: 0 };
    },
    bands: ["shelf"],
    after: (s) => (s.status.message = { id: "scanFinished", added: 667, stopped: 0 }),
    meters: QUIET,
  },
  // 「プラグインの場所」 folded (▸ and the count).
  pluginPlacesFolded: {
    setup(s) {
      settings(s);
      s.workspace.settings = { draftOutput: HEADPHONES.id, advancedOpen: true };
      s.plugins.places.push(place("~/音楽/プラグイン", true));
      s.plugins.placesOpen = false;
    },
    meters: QUIET,
    scrollToEnd: true,
  },
  // More stopped bundles than fit in a low window: only the rows scroll (spec 2 「ダイアログ」).
  scanStoppedMany: {
    setup(s) {
      ready(s);
      s.plugins.stopped = Array.from({ length: 5 }, () => structuredClone(STOPPED_PLUGINS)).flatMap((list, n) =>
        list.map((p) => ({ ...p, path: `${p.path}.${n}`, shownPath: `${p.shownPath}.${n}` })),
      );
      s.plugins.stoppedOpen = true;
      s.status.message = { id: "scanStoppedOpen" };
    },
    meters: QUIET,
  },
  // Spec 6 「プラグインが止まった」 in the status line: an instrument plugin of track 1.
  pluginStopped: {
    setup(s) {
      ready(s);
      s.status.message = { id: "pluginStopped", name: "Surge XT", slot: { kind: "instrument", track: s.song.tracks[0]!.id } };
    },
    meters: QUIET,
  },
  // The song name of the mock Main: saved, 14 steps (「保存済み ・ 取り消し 14」).
  saved: {
    setup(s) {
      changedSong(s);
      s.song.undoCount = 14;
      s.song.file = "saved";
    },
    meters: QUIET,
  },
  // A5: 「ファイル ▸ 最近ひらいた曲 ▸」 with songs in it (shoot_ui.py menuRecent).
  recentSongs: {
    setup(s) {
      changedSong(s);
      s.song.undoCount = 14;
      s.song.file = "saved";
      s.recent = [
        { id: 3, name: "はじめての曲", place: "~/ミュージック/はじめての曲/" },
        { id: 2, name: "夜のピアノ", place: "~/ミュージック/夜のピアノ/" },
        { id: 1, name: "ドラムの練習", place: "~/ミュージック/練習/ドラムの練習/" },
      ];
    },
    meters: QUIET,
  },
  // 2 of 14 steps taken back: the history band label "12 / 14" and the status line 「2 手ぶん取り消してあります…」.
  undone: {
    setup(s) {
      changedSong(s);
      s.song.undoCount = 12;
      s.song.redoCount = 2;
      s.status.message = { id: "undoPending", count: 2 };
    },
    meters: QUIET,
  },
  // The loop of the mock Main: bars 5–9, on, stopped at bar 5.
  loop: {
    setup(s) {
      ready(s);
      s.song.tracks[0]!.clips = [fakeClip(1, "イントロ", 1, 5, 1, [2]), fakeClip(3, "Aメロ", 5, 9, 2, [5, 6, 7])];
      s.song.loopRange = { start: 4 * BAR, end: 8 * BAR };
      s.song.loopOn = true;
      s.transport.position = 4 * BAR;
      s.status.message = null;
    },
    meters: QUIET,
  },
  // The mock Main: the loop scene with the other tracks of F-WidePR, 「4 パッド」 muted (「… ・ 4 パッド はミュート」).
  muted: {
    setup(s) {
      ready(s);
      s.song.tracks[0]!.clips = [fakeClip(1, "イントロ", 1, 5, 1, [2]), fakeClip(3, "Aメロ", 5, 9, 2, [5, 6, 7])];
      let id = 200;
      WIDE_TRACKS.forEach((t, i) => {
        const clips = t.clips.map(([name, from, to]) => {
          const take = phrase(++id, to - from, 4 * (to - from), id);
          FAKE_TAKES.set(take.id, take);
          return fakeClip(++id, name, from, to, 1, [take.id]);
        });
        s.song.tracks.push({ ...s.song.tracks[0]!, id: i + 2, number: i + 2, name: t.name, color: i + 2, gainDb: t.gainDb, clips });
      });
      s.song.tracks[3]!.muted = true;
      s.song.undoCount = 14;
      s.song.file = "saved";
      s.song.loopRange = { start: 4 * BAR, end: 8 * BAR };
      s.song.loopOn = true;
      s.transport.position = 4 * BAR;
      s.status.message = { id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: true, notes: null };
    },
    meters: QUIET,
  },
  // Not from a mock (B-70, B-71): three tracks; the bass is selected (it is where the keys play) and muted, the drums
  // soloed. 「Aメロ」 of the piano and 「ベース A」 are selected together, 「ベース A」 clicked last. One clip is ready to paste.
  arrangeSelect: { setup: arrangeSelect, meters: QUIET },
  // Not from a mock (B-66): 「ベロシティ固定」 is on at 100.
  velocityFixed: {
    setup(s) {
      ready(s);
      s.keyboards.velocityFixed = { on: true, value: 100 };
    },
    meters: { ...QUIET, velocity: 100 },
  },
  // Not from a mock (B-68, B-67): the piano roll after a velocity out of range was typed, and after 「そろえる」.
  valueClamped: {
    setup: pianoRollSong,
    bands: ["pianoRoll"],
    after: (s) => (s.status.message = { id: "valueClamped", field: "velocity", value: "127", range: null }),
    meters: QUIET,
  },
  meterClamped: {
    setup: ready,
    after: (s) => (s.status.message = { id: "valueClamped", field: "meterDen", value: "4", range: null }),
    meters: QUIET,
  },
  velocityMatched: {
    setup: pianoRollSong,
    bands: ["pianoRoll"],
    after: (s) => (s.status.message = { id: "velocityMatched", count: 3, value: 100 }),
    meters: QUIET,
  },
  lengthMatched: {
    setup: pianoRollSong,
    bands: ["pianoRoll"],
    after: (s) => (s.status.message = { id: "lengthMatched", count: 3, value: TPQ / 2 }),
    meters: QUIET,
  },
  // Not from a mock (B-68): a part of a position put in its range.
  positionClamped: {
    setup: ready,
    after: (s) => (s.status.message = { id: "valueClamped", field: "beat", value: "3. 4. 000", range: "1〜4" }),
    meters: QUIET,
  },
  barClamped: {
    setup: ready,
    after: (s) => (s.status.message = { id: "valueClamped", field: "bar", value: "1. 1. 000", range: null }),
    meters: QUIET,
  },
  // Not from a mock (B-71): clips that would overlap other clips were not put there.
  clipsNotMoved: {
    setup: arrangeSelect,
    after: (s) => (s.status.message = { id: "clipsNotMoved" }),
    meters: QUIET,
  },
  clipsNotPasted: {
    setup: arrangeSelect,
    after: (s) => (s.status.message = { id: "clipsNotPasted" }),
    meters: QUIET,
  },
  clipsNotDuplicated: {
    setup: arrangeSelect,
    after: (s) => (s.status.message = { id: "clipsNotDuplicated" }),
    meters: QUIET,
  },
};

function arrangeSelect(s: AppState): void {
  ready(s);
  s.song.undoCount = 5;
  s.song.file = "changed";
  const track = s.song.tracks[0]!;
  track.clips = [fakeClip(1, "イントロ", 1, 5, 1, [2]), fakeClip(3, "Aメロ", 5, 9, 2, [5, 6, 7])];
  const bass = phrase(40, 4, 11, 6);
  FAKE_TAKES.set(bass.id, bass);
  s.song.tracks.push(
    { ...track, id: 2, number: 2, name: "ベース", color: 2, gainDb: -7.5, muted: true, clips: [fakeClip(41, "ベース A", 5, 9, 1, [40])] },
    { ...track, id: 3, number: 3, name: "ドラム", color: 3, gainDb: -6, soloed: true, clips: [fakeClip(43, "8ビート", 1, 9, 1, [7])] },
  );
  s.song.playTarget = 2;
  s.arrange = { selectedClips: [3, 41], selectedClip: 41, pianoRollClip: 41, clipboardClips: 1, tallTrack: null };
  s.transport.position = 4 * BAR;
  s.status.message = { id: "summary", loopBars: null, nothingSelected: false, notes: null };
}

// Round 4. The song of the mock R7-Arrange6: six tracks, each with its instrument; 「2 ベース」 selected. The ids are
// not the numbers (11 to 16), as after tracks were added and moved: a command must carry the id.
const BAND_TRACKS: { name: string; instrument: InstrumentKind; gainDb: number; clips: [string, number, number][] }[] = [
  { name: "ピアノ", instrument: "grandPiano", gainDb: -4.8, clips: [["イントロ", 1, 5], ["サビ", 13, 17]] },
  { name: "ベース", instrument: "fingerBass", gainDb: -7.5, clips: [["ベース A", 5, 9], ["ベース A", 9, 13], ["ベース サビ", 13, 17]] },
  { name: "ドラム", instrument: "studioKit", gainDb: -6.0, clips: [["フィル", 3, 5], ["8ビート", 5, 13], ["サビ ビート", 13, 17]] },
  { name: "パッド", instrument: "strings", gainDb: -12.0, clips: [["コード", 1, 5], ["コード サビ", 13, 17]] },
  { name: "シンセ", instrument: "synthLead", gainDb: -9.0, clips: [["リード", 9, 13], ["リード サビ", 13, 17]] },
  { name: "ベース 2", instrument: "synthBass", gainDb: -10.5, clips: [["サビ うら", 13, 17]] },
];
const BAND_TRACK_ID = 10;

function bandSong(s: AppState, tracks = BAND_TRACKS): void {
  ready(s);
  let id = 300;
  s.song.tracks = tracks.map((t, i) => ({
    id: BAND_TRACK_ID + i + 1,
    number: i + 1,
    name: t.name,
    color: (i % 5) + 1,
    instrument: t.instrument,
    kind: { kind: "keys" },
    sound: builtinSound(t.instrument),
    standIn: null,
    drumKeys: null,
    gainDb: t.gainDb,
    pan: 0,
    sends: newTrackSends(t.instrument),
    sendDefaults: newTrackSends(t.instrument),
    inserts: noInserts(),
    slots: builtinSlots(noInserts()),
    muted: false,
    soloed: false,
    lanes: [],
    clips: t.clips.map(([name, from, to]) => {
      const take = phrase(++id, to - from, 4 * (to - from), id);
      FAKE_TAKES.set(take.id, take);
      return fakeClip(++id, name, from, to, 1, [take.id]);
    }),
    arp: FAKE_ARP_OFF,
  }));
  s.song.playTarget = BAND_TRACK_ID + 1;
  s.song.undoCount = 14;
  s.song.file = "saved";
  s.song.loopRange = { start: 4 * BAR, end: 8 * BAR };
  s.song.loopOn = true;
  s.transport.position = 4 * BAR;
  s.status.message = { id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: true, notes: null };
}

/** The four tracks of the mocks F4-Shelf*, with the shelf open. */
function shelfSong(s: AppState, shelf: Partial<ShelfView>): void {
  bandSong(s, BAND_TRACKS.slice(0, 4));
  s.shelf = { ...closedShelf(), ...shelf };
}

// R7-Arrange6: the ● of 「2 ベース」 was pressed, so it is the selected track now.
// Round 12 P1: A-PianoRoll in D blues with scale lock on (rows outside in row.out).
SCENES.scaleLocked = {
  setup(s) {
    pianoRollSong(s);
    s.pianoRoll.scale = { root: 2, mode: "blues", lock: true, highlight: true };
  },
  bands: ["pianoRoll"],
  meters: QUIET,
};

// Round 12 P2: F-WidePR (「1 ピアノ」 in the piano roll) for 「ほかのトラックも見る」, which the shot turns on (screen-only state).
// 「3 ドラム」 is a kit and 「5 ボーカル」 sings, so the list offers 「2 ベース」 and 「4 パッド」. The bass has one more clip, bars 2–4
// with its first bar trimmed off (take offset −1 bar): its notes line up by the offset, and those of the trimmed bar are not drawn.
const GHOST_BASS_TAKE = 56;
FAKE_TAKES.set(GHOST_BASS_TAKE, phrase(GHOST_BASS_TAKE, 4, 16, 11));
SCENES.pianoRollGhost = {
  setup(s) {
    pianoRollWide(s);
    const [, bass, drums, , vocal] = s.song.tracks;
    drums!.instrument = "studioKit";
    drums!.sound = builtinSound("studioKit");
    vocal!.instrument = "sine";
    vocal!.sound = builtinSound("sine");
    vocal!.kind = { kind: "vocal", input: "mono1", inputReady: true, armed: false, monitor: false };
    bass!.clips = [{ ...fakeClip(57, "ベース イントロ", 2, 5, 1, [GHOST_BASS_TAKE]), takeOffset: -BAR }, ...bass!.clips];
  },
  bands: ["pianoRoll"],
  meters: QUIET,
};

SCENES.tracks6 = {
  setup(s) {
    bandSong(s);
    s.song.playTarget = BAND_TRACK_ID + 2;
  },
  after: (s) => (s.status.message = { id: "armMoved", track: { number: 2, name: "ベース" } }),
  meters: QUIET,
};
// A3: 「トラックを複製」 on 「2 ベース」: the copy 「3 ベース 3」 (「ベース 2」 is taken) right below it, with its clips, selected.
SCENES.trackDuplicated = {
  setup(s) {
    bandSong(s);
    s.song.playTarget = BAND_TRACK_ID + 2;
    s.song = reduceTracks(s, new FakeBands(800), { type: "duplicateTrack", track: BAND_TRACK_ID + 2 })!.song;
  },
  meters: QUIET,
};
// R1: something was played while stopped: 「さっきのを残す」 can be pressed.
SCENES.keepPlayedReady = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    s.transport.canKeepPlayed = true;
  },
  meters: QUIET,
};
// R2: 「キーで弾く」 on and played: 「パソコンのキーボード」 (with 「+1」) in the input meter, the octave, the status line.
SCENES.typedKeys = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    s.keyboards = typedOn(s.keyboards, true);
    s.status.message = { id: "typedKeysOn" };
  },
  meters: { ...QUIET, keys: [48, 52, 55], velocity: 100 },
};
// R2: the sound settings while 「キーで弾く」 is on: 「パソコンのキーボード」 in the list of keyboards.
SCENES.typedKeysSettings = {
  setup(s) {
    settings(s);
    s.keyboards = typedOn(s.keyboards, false);
  },
  meters: QUIET,
};
// R1: Shift R with nothing played.
SCENES.nothingToKeep = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
  },
  after: (s) => (s.status.message = { id: "nothingToKeep" }),
  meters: QUIET,
};
// A3: 「トラックを複製」 in a song of 64 tracks.
SCENES.tooManyTracks = {
  setup(s) {
    bandSong(s, Array.from({ length: MAX_TRACKS }, (_, i) => ({ ...BAND_TRACKS[i % BAND_TRACKS.length]!, name: `トラック ${i + 1}`, clips: [] })));
    s.song.playTarget = BAND_TRACK_ID + 1;
  },
  after: (s) => (s.status.message = { id: "tooManyTracks" }),
  meters: QUIET,
};
// F4-ShelfSample without the sample: 「＋ トラックを追加」 opened the shelf, no row picked yet.
SCENES.shelfNewTrack = { setup: (s) => shelfSong(s, {}), bands: ["shelf"], meters: QUIET };
// P4 (round 12): 「パターン」 of the shelf (tools/design/shoot_ui.py clicks the switch, a screen-only state). 「3 ドラム」 is
// selected and the play line is at bar 17: 「16 ビート」 is chosen and its ▶ plays.
SCENES.shelfPatterns = {
  setup(s) {
    shelfSong(s, { selectedPattern: "sixteenBeat", patternSample: { id: "sixteenBeat", playing: { startedUs: 0, lengthUs: 2_000_000 } } });
    s.song.playTarget = BAND_TRACK_ID + 3;
    s.transport.position = 16 * BAR;
  },
  bands: ["shelf"],
  meters: QUIET,
};
// P4: 「1 ピアノ」 is selected, so 「ここに置く」 makes a studio kit track; the play line is in a bar of 3/4, which says so.
SCENES.shelfPatternsNewTrack = {
  setup(s) {
    shelfSong(s, { selectedPattern: "tomFill" });
    s.song.playTarget = BAND_TRACK_ID + 1;
    s.timeline.meterChanges = [{ bar: 8, meterNum: 3, meterDen: 4 }];
    s.transport.position = 8 * BAR;
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "patternNeedsFourFour" }),
  meters: QUIET,
};
// R8-Shelf84: the 31 instruments of the shelf (spec 3, 84). The folded groups are the screen's own
// (tools/design/shoot_ui.py folds all but 「ピアノ ・ 鍵盤」 and 「ギター」 by clicking their headings).
SCENES.shelf84 = { setup: (s) => shelfSong(s, {}), bands: ["shelf"], meters: QUIET };
// F4-ShelfAudition: 「シンセベース」 is auditioned for a new track (it would be track 5).
SCENES.shelfAudition = {
  setup(s) {
    shelfSong(s, { selected: "synthBass" });
    s.transport.canRecord = false;
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "auditionNewTrack", instrument: "synthBass" }),
  meters: { ...QUIET, keys: [60, 64, 67], velocity: 87, sustain: 127, modulation: 12 },
};
// F4-ShelfReplace: 「エレクトリックピアノ」 is auditioned in place of the instrument of 「1 ピアノ」.
SCENES.shelfReplace = {
  setup(s) {
    shelfSong(s, { mode: { kind: "replace", track: BAND_TRACK_ID + 1 }, selected: "electricPiano", current: "grandPiano" });
    s.transport.canRecord = false;
  },
  bands: ["shelf"],
  after: (s) =>
    (s.status.message = {
      id: "auditionReplace",
      instrument: "electricPiano",
      track: { number: 1, name: "ピアノ" },
      current: "grandPiano",
    }),
  meters: { ...QUIET, keys: [60, 64, 67], velocity: 87, sustain: 127, modulation: 12 },
};

// F4-Mixer: five tracks, the two returns and the master, playing at 9.2.000. The reverb of return A is the selected
// insert, and its first row 「響きの長さ」 is chosen in the parameters panel.
const RETURN_A: StripRef = { kind: "return", bus: "a" };
const RETURN_B: StripRef = { kind: "return", bus: "b" };
const MIXER_STRIPS: Pick<TrackView, "gainDb" | "pan" | "sends" | "inserts">[] = [
  { gainDb: 0, pan: 0, sends: [-15, null], inserts: noInserts() },
  { gainDb: 0, pan: 0, sends: [null, null], inserts: firstInsert("compressor") },
  { gainDb: 0, pan: 0, sends: [-24, null], inserts: firstInsert("compressor") },
  { gainDb: -3, pan: -20, sends: [-15, null], inserts: firstInsert("eq4") },
  { gainDb: 0, pan: 20, sends: [-15, -18], inserts: noInserts() },
];
function mixerSong(s: AppState, selected: InsertRef, effect: EffectKind, row: number | null): void {
  bandSong(s, BAND_TRACKS.slice(0, 5));
  s.song.tracks = s.song.tracks.map((t, i) => ({ ...t, ...structuredClone(MIXER_STRIPS[i]!) }));
  s.params = { selected: { kind: "builtin", insert: selected, effect, values: effectDefaults(effect), row, preset: null }, clips: null };
}
const MIXER_METERS: FakeMeters = {
  velocity: 83,
  loadPermille: 120,
  master: [dbfs(-2.5), dbfs(-4)],
  returns: [
    [dbfs(-13), dbfs(-15)],
    [dbfs(-28), dbfs(-34)],
  ],
  tracks: [
    [dbfs(-8), dbfs(-9.5)],
    [dbfs(-11), dbfs(-13)],
    [dbfs(-5), dbfs(-6.5)],
    [dbfs(-17), dbfs(-20)],
    [dbfs(-10), dbfs(-12)],
  ],
  position: { tick: 8 * BAR + TPQ, ticksPerUs: 0, state: "playing", countInBeatsLeft: 0, lap: 0 },
};
SCENES.mixer = {
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", 0);
    s.transport.state = "playing";
  },
  bands: ["mixer", "params"],
  after: (s) =>
    (s.status.message = { id: "paramSelected", loopBars: { from: 5, to: 9 }, insert: { strip: RETURN_A, slot: 0 }, effect: "reverb", param: 0 }),
  meters: MIXER_METERS,
};
// R6-Delay: the delay of return B in the parameters panel, no row chosen.
SCENES.paramsDelay = {
  setup: (s) => mixerSong(s, { strip: RETURN_B, slot: 0 }, "delay", null),
  bands: ["mixer", "params"],
  meters: QUIET,
};
// Round 12 M1 (the mock of the spec): the compressor of track 2 has the values of 「歌をそろえる」, and the PC keeps
// 「いつものピアノ」. shoot_ui.py opens 「プリセット ▾」 (presetsMenu) and 「いまの設定を残す…」 (presetsSaving).
SCENES.presets = {
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", null);
    const insert: InsertRef = { strip: { kind: "track", id: s.song.tracks[1]!.id }, slot: 0 };
    const values = [...FACTORY_PRESETS.compressor[0]!.values];
    s.params.selected = { kind: "builtin", insert, effect: "compressor", values, row: null, preset: { kind: "factory", id: "vocalEven" } };
    fakeUserPreset(s, "compressor", "いつものピアノ", effectDefaults("compressor"));
  },
  bands: ["mixer", "params"],
  meters: QUIET,
};
// Round 12 M1: a gate with its values as it is new, which are not a preset's: 「プリセット ▾」.
SCENES.presetsNone = {
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", null);
    s.song.tracks[0]!.inserts = firstInsert("gate");
    const insert: InsertRef = { strip: { kind: "track", id: s.song.tracks[0]!.id }, slot: 0 };
    s.params.selected = { kind: "builtin", insert, effect: "gate", values: effectDefaults("gate"), row: null, preset: null };
  },
  bands: ["mixer", "params"],
  meters: QUIET,
};
// Round 12 M6 (no mock): the gate, the de-esser and the saturation in the first slot of track 1, as they are new.
const newEffectScene = (effect: EffectKind): SceneDef => ({
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", null);
    s.song.tracks[0]!.inserts = firstInsert(effect);
    const insert: InsertRef = { strip: { kind: "track", id: s.song.tracks[0]!.id }, slot: 0 };
    s.params.selected = { kind: "builtin", insert, effect, values: effectDefaults(effect), row: null, preset: null };
  },
  bands: ["mixer", "params"],
  meters: QUIET,
});
SCENES.paramsGate = newEffectScene("gate");
SCENES.paramsDeEsser = newEffectScene("deEsser");
SCENES.paramsSaturation = newEffectScene("saturation");
// Round 12, the owner's answers (B-97-3; no mock): the mixer band, its song with three mute sets for the menu at the
// head of the band. 「シンセなし」 is as the song is now (「5 シンセ」 is muted); the others are not.
function mixerWithSets(s: AppState): void {
  mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", null);
  s.params = { selected: null, clips: null };
  const [piano, , , pad, synth] = s.song.tracks.map((t) => t.id);
  s.song.muteSetsKept = [
    fakeMuteSet(s.song, "ドラムとベースだけ", { muted: [piano!, pad!, synth!], soloed: [] }),
    fakeMuteSet(s.song, "シンセなし", { muted: [synth!], soloed: [] }),
    fakeMuteSet(s.song, "ピアノだけ", { muted: [], soloed: [piano!] }),
  ];
  s.song.tracks[4]!.muted = true;
}
SCENES.mixerMuteSets = { setup: mixerWithSets, bands: ["mixer"], meters: QUIET };
// The same with as many sets as a song holds (MUTE_SETS_MAX): no more can be saved.
SCENES.mixerMuteSetsFull = {
  setup(s) {
    mixerWithSets(s);
    const more = Array.from({ length: MUTE_SETS_MAX - 3 }, (_, i) => fakeMuteSet(s.song, `セット ${i + 4}`, { muted: [], soloed: [] }));
    s.song.muteSetsKept = [...s.song.muteSetsKept!, ...more];
  },
  bands: ["mixer"],
  meters: QUIET,
};
// Not from a mock: 「空き ＋」 of the second slot of 「1 ピアノ」 opened the shelf, which lists the effects; 「コンプレッサー」 is chosen.
SCENES.shelfEffects = {
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", null);
    s.shelf = { ...closedShelf(), mode: { kind: "insertEffect", strip: { kind: "track", id: BAND_TRACK_ID + 1 }, slot: 1 }, selectedEffect: "compressor" };
  },
  bands: ["mixer", "shelf"],
  meters: QUIET,
};

// Not from a mock: the scene recording, with the listener moving at 96.00 (the clip being recorded grows).
SCENES.recordingMoving = {
  ...SCENES.recording!,
  meters: {
    ...SCENES.recording!.meters,
    position: { tick: 6 * BAR, ticksPerUs: (96 * TPQ) / 60e6, state: "recording", countInBeatsLeft: 0, lap: 0 },
  },
};

// R7-Drum: 「3 ドラム」 (「スタジオキット」) with its clip 「8ビート」 (bars 1–4, 53 notes) in the piano roll. An 8-beat for three
// bars and a half, a tom fill in the last two beats; the snares of 1.2 and 2.2 selected; 「キック」 and 「スネア」 held.
const DRUM_TAKE = 80;
const DRUM_CLIP = 81;
const GM = { kick: 36, snare: 38, lowTom: 45, midTom: 47, closedHiHat: 42, crash: 49, highTom: 50 } as const;
const DRUM_NOTES: { start: number; length: number; key: number; velocity: number }[] = (() => {
  const hit = (start: number, key: number, velocity: number) => ({ start, length: TPQ / 4, key, velocity });
  const out = [hit(0, GM.crash, 110)];
  for (let eighth = 0; eighth < 30; eighth++) out.push(hit((eighth * TPQ) / 2, GM.closedHiHat, eighth % 2 === 0 ? 88 : 72));
  for (let bar = 0; bar < 4; bar++) {
    const b = bar * BAR;
    out.push(hit(b, GM.kick, 104), hit(b + (3 * TPQ) / 2, GM.kick, 104), hit(b + TPQ, GM.snare, 96));
    if (bar < 3) out.push(hit(b + 2 * TPQ, GM.kick, 104), hit(b + 3 * TPQ, GM.snare, 96));
  }
  const fill = 3 * BAR + 3 * TPQ;
  out.push(hit(fill, GM.highTom, 100), hit(fill + TPQ / 4, GM.highTom, 84), hit(fill + TPQ / 2, GM.midTom, 100), hit(fill + (3 * TPQ) / 4, GM.lowTom, 104));
  return out.sort((a, b) => a.start - b.start || a.key - b.key);
})();
FAKE_TAKES.set(DRUM_TAKE, { id: DRUM_TAKE, length: 4 * BAR, notes: DRUM_NOTES });
const DRUM_SELECTED = DRUM_NOTES.flatMap((n, i) => (n.key === GM.snare && (n.start === TPQ || n.start === BAR + TPQ) ? [i] : []));
SCENES.drumRoll = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 5));
    const drum = s.song.tracks[2]!;
    drum.clips = [fakeClip(DRUM_CLIP, "8ビート", 1, 5, 1, [DRUM_TAKE]), ...drum.clips.slice(1)];
    s.song.playTarget = drum.id;
    s.song.loopOn = false;
    s.song.noteCount = DRUM_NOTES.length;
    s.arrange = arrangeOf(DRUM_CLIP);
    s.pianoRoll = {
      ...pianoRollView(DRUM_TAKE, false),
      selectedNotes: DRUM_SELECTED.map((i) => ({ start: i, end: i + 1 })),
      quantize: { division: 16, triplet: false, strength: 100, swing: 0, align: "start" },
    };
    s.transport.position = 0;
    s.status.message = {
      id: "summary",
      loopBars: null,
      nothingSelected: false,
      notes: { count: DRUM_SELECTED.length, keys: DRUM_SELECTED.map((i) => DRUM_NOTES[i]!.key), quantized: false, names: keyCounts(DRUM_SELECTED.map((i) => DRUM_NOTES[i]!.key)) },
    };
  },
  bands: ["pianoRoll"],
  meters: { ...QUIET, keys: [GM.kick, GM.snare], velocity: 104, modulation: 12 },
};

// Not from a mock: one clip of 10 000 notes over 64 bars, the first 200 selected, for measuring the piano roll.
const TEN_THOUSAND = 90;
FAKE_TAKES.set(TEN_THOUSAND, phrase(TEN_THOUSAND, 64, 10_000, 9));
SCENES.pianoRoll10k = {
  setup(s) {
    pianoRollSong(s);
    s.song.tracks[0]!.clips = [fakeClip(91, "イントロ", 1, 65, 1, [TEN_THOUSAND])];
    s.song.noteCount = 10_000;
    s.arrange = arrangeOf(91);
    s.pianoRoll = { ...pianoRollView(TEN_THOUSAND, false), selectedNotes: [{ start: 0, end: 200 }] };
    s.status.message = null;
  },
  bands: ["pianoRoll"],
  meters: QUIET,
};
// The same while playing from bar 1 (the play line moves, the keys light, the roll pages).
SCENES.pianoRoll10kPlay = {
  ...SCENES.pianoRoll10k,
  setup(s) {
    SCENES.pianoRoll10k!.setup(s);
    s.transport.state = "playing";
    s.transport.position = 0;
  },
  meters: { ...QUIET, position: { tick: 0, ticksPerUs: (96 * TPQ) / 60e6, state: "playing", countInBeatsLeft: 0, lap: 0 } },
};

// ---- round 5, step 2: 「あとから入れたもの」 (protocol 17) ----

const SURGE = lv2Key("Surge XT");
const SURGE_SOUND: SoundRef = { kind: "plugin", key: SURGE };
const CLEAN_GUITAR: Extract<SoundRef, { kind: "sf2" }> = { kind: "sf2", file: FLUID, bank: 0, program: 27 };
const playingKeys: Partial<FakeMeters> = { keys: [60, 64, 67], velocity: 87, sustain: 127, modulation: 12 };

const pluginView = (name: string, state: PluginState = "ready"): PluginView => ({ key: lv2Key(name), name, format: "lv2", state });
const pluginSlot = (name: string, state: PluginState = "ready", bypassed = false): InsertView => ({
  id: NO_INSERT_ID,
  effect: { kind: "plugin", ...pluginView(name, state) },
  bypassed,
});

/** The song of the mocks F5-Plugin*: the four tracks of the band and "5 Surge XT", a track of the plugin. */
function surgeSong(s: AppState, state: PluginState = "ready"): void {
  bandSong(s, BAND_TRACKS.slice(0, 5));
  const surge = s.song.tracks[4]!;
  Object.assign(surge, { name: "Surge XT", instrument: "sine", sound: { kind: "plugin", ...pluginView("Surge XT", state) } satisfies SoundView });
  s.song.loopOn = false;
  s.status.message = { id: "summary", loopBars: null, nothingSelected: true, notes: null };
}

function shelfOpen(s: AppState, shelf: Partial<ShelfView>, library: LibraryView): void {
  s.shelf = { ...closedShelf(), ...shelf };
  s.library = library;
  s.transport.canRecord = s.shelf.selected === null && (s.shelf.selectedSound ?? null) === null;
}

// F5-SF2Shelf: FluidR3_GM.sf2 was added (its heading is the open one); "Clean Guitar" is auditioned for a new track.
SCENES.sf2Shelf = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    shelfOpen(s, { selectedSound: CLEAN_GUITAR }, { ...emptyLibrary(), sf2: fullLibrary().sf2, openedSf2: FLUID });
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "auditionNewTrackSound", sound: { kind: "sf2", name: "Clean Guitar", file: FLUID } }),
  meters: { ...QUIET, ...playingKeys },
};
// Not from a mock (spec 6 「SF2 を足した」): right after the file was added, nothing auditioned yet.
SCENES.sf2Added = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    shelfOpen(s, {}, { ...emptyLibrary(), sf2: fullLibrary().sf2, openedSf2: FLUID });
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "sf2Added", file: FLUID, presets: 128, kits: 9, skipped: 0 }),
  meters: QUIET,
};
// Not from a mock (spec 6 「SF2 が読めない」): the file was not added.
SCENES.sf2Unreadable = {
  setup: (s) => bandSong(s, BAND_TRACKS.slice(0, 4)),
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "sf2Unreadable", file: FLUID }),
  meters: QUIET,
};
// F5-SF2Drop: an SF2 file is held over the window; nothing was added yet.
SCENES.sf2DropAddable = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    s.workspace.fileDrop = { name: FLUID, addable: true, files: 1, sf2: 1, audio: 0 };
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "sf2Dragging" }),
  meters: QUIET,
};
// The same with a file that is no SF2 (spec 2: 「足せない」, spec 6 「SF2 でないファイルを持ってきている」).
SCENES.sf2DropNotAddable = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    s.workspace.fileDrop = { name: "piano-demo.wav", addable: false, files: 1, sf2: 0, audio: 0 };
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "notSf2Dragging" }),
  meters: QUIET,
};
// Not from a mock (spec 3 89-10, spec 6 「ファイルをいくつも持ってきている」): three files, two of them SF2 files.
SCENES.sf2DropMany = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    s.workspace.fileDrop = { name: FLUID, addable: true, files: 3, sf2: 2, audio: 0 };
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "sf2Dragging" }),
  meters: QUIET,
};
// Not from a mock (spec 6 「SF2 をいくつも足した」): two SF2 files added, one file passed over.
SCENES.sf2AddedMany = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    shelfOpen(s, {}, { ...emptyLibrary(), sf2: fullLibrary().sf2, openedSf2: FLUID });
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "sf2AddedMany", added: 2, first: FLUID, skipped: 1 }),
  meters: QUIET,
};

/** F5-ShelfFx: the inserts of the five strips, with the plugins among them ("Brick EQ" has stopped). */
function surgeMixer(s: AppState): void {
  surgeSong(s);
  const [piano, bass, drums, pad, surge] = s.song.tracks as [TrackView, TrackView, TrackView, TrackView, TrackView];
  piano.inserts = ["eq4", "compressor", null, null];
  bass.inserts = ["compressor", null, null, null];
  bass.sends = [null, null];
  drums.inserts = ["compressor", null, null, null];
  drums.slots = [null, pluginSlot("Brick EQ", "stopped"), null, null];
  pad.inserts = [null, "eq4", null, null];
  pad.slots = [pluginSlot("MVerb"), null, null, null];
  Object.assign(pad, { pan: -20, gainDb: -3 });
  surge.slots = [pluginSlot("Calf Reverb"), null, null, null];
  Object.assign(surge, { pan: 20, gainDb: 0, sends: [-15, -18] });
  for (const t of [piano, bass, drums]) t.gainDb = 0;
}

// F5-ShelfFx: 「空き ＋」 of the second slot of "5 Surge XT" opened the shelf on the effects; "Dragonfly Hall Reverb" is chosen.
SCENES.shelfFx = {
  setup(s) {
    surgeMixer(s);
    const surge = s.song.tracks[4]!;
    shelfOpen(s, { mode: { kind: "insertEffect", strip: { kind: "track", id: surge.id }, slot: 1 }, chosenEffect: { kind: "plugin", key: lv2Key("Dragonfly Hall Reverb") } }, fullLibrary());
    s.transport.state = "playing";
    s.transport.position = at("9.2.000");
  },
  bands: ["mixer", "shelf"],
  meters: { ...QUIET, loadPermille: 240, master: [dbfs(-6.2), dbfs(-7)], position: { tick: at("9.2.000"), ticksPerUs: 0, state: "playing", countInBeatsLeft: 0, lap: 0 } },
};
// F5-ShelfSearch: the same before a row is chosen. What is typed into the search field is the screen's own
// (tools/design/shoot_ui.py types "Dragonfly").
SCENES.shelfSearch = {
  ...SCENES.shelfFx,
  setup(s) {
    SCENES.shelfFx!.setup(s);
    s.shelf.chosenEffect = null;
  },
};
// Opt5-Plugins (14-A): the shelf with FluidR3 and the 540 plugins, opened for a new track. Which of the two
// lists is shown, and which headings are unfolded, is the screen's own.
SCENES.shelfPlugins = {
  setup(s) {
    surgeSong(s);
    shelfOpen(s, {}, fullLibrary());
  },
  bands: ["shelf"],
  meters: QUIET,
};
// F5-PluginLoading: "Surge XT" was selected and is being read.
SCENES.pluginLoading = {
  setup(s) {
    surgeSong(s);
    shelfOpen(s, { selectedSound: SURGE_SOUND, loadingSound: SURGE_SOUND }, fullLibrary());
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "pluginLoading", name: "Surge XT" }),
  meters: QUIET,
};
// F5-PluginAudition: it is read, and the keys sound with it (「窓をひらく」).
SCENES.pluginAudition = {
  setup(s) {
    surgeSong(s);
    shelfOpen(s, { selectedSound: SURGE_SOUND }, fullLibrary());
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "auditionNewTrackSound", sound: { kind: "plugin", name: "Surge XT" } }),
  meters: { ...QUIET, ...playingKeys },
};
// Not from a mock (spec 6 「プラグインを読みこめない」): it did not load; nothing is selected.
SCENES.pluginUnreadable = {
  setup(s) {
    surgeSong(s);
    shelfOpen(s, { unreadable: SURGE_SOUND }, fullLibrary());
  },
  bands: ["shelf"],
  meters: QUIET,
};
// F5-PluginTrack: "5 Surge XT" is the selected track; two of sixteen steps are undone. (What the history band lists is not made yet.)
SCENES.pluginTrack = {
  setup(s) {
    surgeSong(s);
    s.library = fullLibrary();
    Object.assign(s.song, { playTarget: s.song.tracks[4]!.id, undoCount: 14, redoCount: 2, file: "changed" });
    s.transport.position = at("9.1.000");
  },
  bands: ["history"],
  after: (s) => (s.status.message = { id: "undoPending", count: 2 }),
  meters: QUIET,
};
// R8-Missing: the sound pack is not found. The four tracks keep their sounds' names and sound as the sine wave.
SCENES.missingPack = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    for (const t of s.song.tracks) t.standIn = "packMissing";
    shelfOpen(s, { available: ["sine"] }, { ...emptyLibrary(), packMissing: true });
    s.song.loopOn = false;
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "soundsMissing", tracks: 4 }),
  meters: QUIET,
};

const MISSING_PLUGINS: MissingView[] = [
  { kind: "plugin", key: SURGE, name: "Surge XT", vendor: "Surge Synth Team", format: "lv2", role: "instrument", shownPath: "/usr/lib/lv2/Surge XT.lv2" },
  { kind: "plugin", key: lv2Key("MVerb"), name: "MVerb", vendor: "DISTRHO", format: "lv2", role: "effect", shownPath: "~/.lv2/MVerb.lv2" },
];

// Not from a mock (spec 3 「SF2 が見つからないとき」): a song from another PC. Two of its tracks use presets of a file
// this PC does not have.
SCENES.missingSf2 = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    const preset = (program: number, name: string): Partial<TrackView> => ({
      instrument: "sine",
      sound: { kind: "sf2", file: FLUID, bank: 0, program, name, family: "guitar" },
      standIn: "sf2Missing",
    });
    Object.assign(s.song.tracks[0]!, preset(27, "Clean Guitar"), { name: "ギター" });
    Object.assign(s.song.tracks[3]!, preset(24, "Nylon String Guitar"), { name: "ギター 2" });
    shelfOpen(s, { missing: [{ kind: "sf2", file: FLUID, tracks: 2 }] }, emptyLibrary());
    s.song.loopOn = false;
  },
  bands: ["shelf"],
  after: (s) => (s.status.message = { id: "sf2Missing", file: FLUID, tracks: 2 }),
  meters: QUIET,
};
// Not from a mock (spec 6 「プラグインが見つからない」): the song uses a plugin instrument and a plugin effect this PC
// does not have. The track sounds as the sine wave; the slot is passed by.
SCENES.missingPlugin = {
  setup(s) {
    surgeMixer(s);
    Object.assign(s.song.tracks[4]!, { sound: { kind: "plugin", ...pluginView("Surge XT", "missing") } satisfies SoundView, standIn: "pluginMissing" });
    s.song.tracks[3]!.slots = [pluginSlot("MVerb", "missing"), null, null, null];
    s.song.tracks[2]!.slots = [null, null, null, null];
    s.song.tracks[4]!.slots = [null, null, null, null];
    shelfOpen(s, { missing: MISSING_PLUGINS.map((m) => ({ ...m })) }, emptyLibrary());
  },
  bands: ["mixer", "shelf"],
  meters: QUIET,
};
// R8-DrumPlay: the scene drumRoll while it plays, at 2.2.000: the closed hi-hat and the snare sound.
SCENES.drumPlay = {
  ...SCENES.drumRoll!,
  setup(s) {
    SCENES.drumRoll!.setup(s);
    s.pianoRoll.selectedNotes = [];
    s.transport.state = "playing";
    s.transport.position = 0;
    s.status.message = null;
  },
  meters: { ...QUIET, loadPermille: 140, master: [dbfs(-8), dbfs(-8.6)], position: { tick: BAR + TPQ, ticksPerUs: 0, state: "playing", countInBeatsLeft: 0, lap: 0 } },
};
/** A kit with a sound on every key from 35 to 81, as `meloet_model::drums::gm_kit_rows` orders its rows. */
const SF2_KIT_ROWS = [
  ...[49, 57, 55, 52, 51, 59, 53, 46, 44, 42, 50, 48, 47, 45, 43, 41, 39, 38, 40, 37, 36, 35],
  ...[54, 56, 58],
  ...Array.from({ length: 22 }, (_, i) => 60 + i),
];
// Not from a mock (spec 3 「SF2 のドラムのキット」): 「3 ドラム」 sounds with the standard kit of FluidR3; its rows are the
// keys the kit has, in the order of the spec.
SCENES.drumRollSf2 = {
  ...SCENES.drumRoll!,
  setup(s) {
    SCENES.drumRoll!.setup(s);
    s.library = { ...emptyLibrary(), sf2: fullLibrary().sf2 };
    Object.assign(s.song.tracks[2]!, {
      instrument: "sine",
      sound: { kind: "sf2", file: FLUID, bank: 128, program: 0, name: "Standard", family: "drumKit" } satisfies SoundView,
      drumKeys: SF2_KIT_ROWS,
    });
  },
};

/** `theme`: the scene in other colors (to shoot each theme); else the scene's own, which is dark. */
export function fakeScene(name: string, scale?: number, viewport = { width: 1280, height: 800 }, theme?: Theme): Scene | null {
  const def = SCENES[name];
  if (!def) return null;
  const state = fakeState();
  const unfit = def.screen ? unfitScales(def.screen) : [];
  const asked = scale ?? def.scale ?? 100;
  const s = SCALES.includes(asked) && !unfit.includes(asked) ? asked : 100;
  const bands = new FakeBands((viewport.height * 100) / s);
  for (const b of def.bands ?? []) bands.toggle(b);
  def.setup(state);
  state.workspace.scale = s;
  state.workspace.viewportWidth = viewport.width;
  state.workspace.viewportHeight = viewport.height;
  state.workspace.unfitScales = unfit;
  state.workspace.layout = bands.layout();
  def.after?.(state);
  if (theme) state.workspace.theme = theme;
  state.song = withMuteSets(settleSong(state.song));
  state.shelf = settleShelf(state.shelf);
  state.history = historyOf(state.song);
  const meters = { ...def.meters, spectrum: def.meters.spectrum ?? fakeSpectrum(def.meters.master) };
  return { state, bands, meters, scrollToEnd: def.scrollToEnd, live: def.live ?? NO_LIVE, history: def.history ?? [], events: def.events ?? [] };
}

/** The section `history` of the fake: the counts of the song; a row's `step` is its `index`. */
function historyOf(song: SongView): HistoryView {
  const total = song.undoCount + song.redoCount;
  return { total, redo: song.redoCount, current: song.undoCount, newest: total };
}

/** The rows `from`..`to` of `total`: those of the scene, and made-up ones past its list. */
export function fakeHistoryRows(rows: readonly HistoryRow[], total: number, from: number, to: number): HistoryRow[] {
  const out: HistoryRow[] = [];
  for (let index = Math.max(1, from); index <= Math.min(to, total); index++) {
    out.push(rows[index - 1] ?? { index, step: index, name: { kind: "moveNotes" }, what: { kind: "notes", count: 1 + (index % 4) }, place: { kind: "pianoRoll" } });
  }
  return out;
}

// ---- round 5, step 3: plugin windows, a plugin in the parameters panel, what is left of 86 (protocol 18) ----

const SURGE_VST3: PluginView = { key: vst3Key("Surge XT"), name: "Surge XT", format: "vst3", state: "ready" };
const trackStrip = (number: number): StripRef => ({ kind: "track", id: BAND_TRACK_ID + number });
const pluginSelected = (slot: PluginSlotRef, table: number, more: Partial<SelectedPlugin> = {}): AppState["params"] => ({
  selected: { kind: "plugin", slot, window: "opens", table, count: fakeParamCount(table), row: null, automated: [], ...more },
  clips: null,
});
const playingAt = (text: string): FakeMeters["position"] => ({ tick: at(text), ticksPerUs: 0, state: "playing", countInBeatsLeft: 0, lap: 0 });

/**
 * The song of the mocks F-WindowOpen, F-ParamFew and F-ParamNoWindow, playing at 9.2.000: "Brick EQ" on 「3 ドラム」
 * has stopped, "Dragonfly Reverb" sits on return A. The mocks' fifth track is a voice from the microphone,
 * which is not made yet: here it keeps the band's fifth instrument.
 */
function windowSong(s: AppState): void {
  bandSong(s, BAND_TRACKS.slice(0, 5));
  const [piano, bass, drums, pad, vocal] = s.song.tracks as [TrackView, TrackView, TrackView, TrackView, TrackView];
  Object.assign(piano, { inserts: ["eq4", "compressor", null, null], sends: [-12, null], pan: -12 });
  Object.assign(bass, { inserts: firstInsert("compressor"), sends: [null, null] });
  Object.assign(drums, { inserts: firstInsert("compressor"), slots: [null, pluginSlot("Brick EQ", "stopped"), null, null], sends: [-18, null] });
  Object.assign(pad, { instrument: "warmPad", sends: [-6, -15], pan: 24 });
  Object.assign(vocal, { name: "ボーカル", inserts: ["eq4", "compressor", null, null], sends: [-9, -18], gainDb: -3.2 });
  Object.assign(s.song.returns[0], { inserts: noInserts(), slots: [pluginSlot("Dragonfly Reverb"), null, null, null] });
  s.song.returns[1].gainDb = -2;
  // The shelf knows the plugins of the song: the return takes the kind of its plugin as its name (「A リバーブ」, user:2026-11-19).
  const effect = (category: PluginEffectCategory) => ({ role: "effect", category }) as const;
  s.library = {
    ...emptyLibrary(),
    plugins: [
      pluginRow("Dragonfly Reverb", "Michael Willis", effect("reverb"), ["lv2"]),
      pluginRow("Mossy Verb", "Example Audio", effect("reverb"), ["lv2"]),
      pluginRow("Brick EQ", "Example Audio", effect("eq"), ["lv2"]),
    ],
  };
  s.transport.state = "playing";
  s.transport.position = at("9.2.000");
}
const WINDOW_METERS: FakeMeters = { ...QUIET, loadPermille: 210, master: [dbfs(-6.2), dbfs(-7)], position: playingAt("9.2.000") };
const DRAGONFLY_SLOT: PluginSlotRef = { kind: "insert", insert: { strip: RETURN_A, slot: 0 } };
const SURGE_SLOT: PluginSlotRef = { kind: "instrument", track: BAND_TRACK_ID + 1 };
const SURGE_WINDOW: PluginWindowView = { slot: SURGE_SLOT, name: "Surge XT", width: 1597, height: 995, largerThanScreen: true };

// F-WindowOpen: the windows of Surge XT (「1 ピアノ」) and of Dragonfly Reverb (return A) are open. The list above
// 「窓 2」 is the screen's own (tools/design/shoot_ui.py clicks it open). The mock gives no size for the second window.
SCENES.windowOpen = {
  setup(s) {
    windowSong(s);
    Object.assign(s.song.tracks[0]!, { instrument: "sine", sound: { kind: "plugin", ...SURGE_VST3 } satisfies SoundView });
    s.windows = { list: [SURGE_WINDOW, { slot: DRAGONFLY_SLOT, name: "Dragonfly Reverb", width: 640, height: 480, largerThanScreen: false }] };
    s.params = pluginSelected(SURGE_SLOT, FAKE_TABLES.surge);
  },
  bands: ["mixer"],
  meters: WINDOW_METERS,
};
// F-ParamFew: Dragonfly Reverb (LV2, 18 parameters, no groups) in the panel; its window is not open.
// 「…の Decay を動かしています」 of the mock's status line is the screen's own, while a row is dragged.
SCENES.paramFew = {
  setup(s) {
    windowSong(s);
    s.params = pluginSelected(DRAGONFLY_SLOT, FAKE_TABLES.dragonfly);
  },
  bands: ["mixer", "params"],
  meters: WINDOW_METERS,
};
// F-ParamNoWindow: Mossy Verb on 「4 パッド」 has a window meloet cannot open yet; a list of steps and switches among its 12 parameters.
SCENES.paramNoWindow = {
  setup(s) {
    windowSong(s);
    s.song.tracks[3]!.slots = [pluginSlot("Mossy Verb"), null, null, null];
    s.params = pluginSelected({ kind: "insert", insert: { strip: trackStrip(4), slot: 0 } }, FAKE_TABLES.mossy, { window: "notYet" });
  },
  bands: ["mixer", "params"],
  after: (s) => (s.status.message = { id: "pluginWindowNotYet", loopBars: { from: 5, to: 9 }, name: "Mossy Verb" }),
  meters: WINDOW_METERS,
};
// F-ParamMany: Surge XT (VST3, 2855 parameters in six groups) is the sound of 「1 ピアノ」; its window is open and
// larger than the screen. Two of its parameters are written by automation lanes, which are not made yet.
SCENES.paramMany = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 5));
    Object.assign(s.song.tracks[0]!, { instrument: "sine", sound: { kind: "plugin", ...SURGE_VST3 } satisfies SoundView });
    s.windows = { list: [SURGE_WINDOW] };
    s.params = pluginSelected(SURGE_SLOT, FAKE_TABLES.surge, { automated: [...SURGE_AUTOMATED] });
  },
  bands: ["params"],
  after: (s) => (s.status.message = { id: "pluginWindowLarge", name: "Surge XT", slot: SURGE_SLOT, width: 1597, height: 995 }),
  meters: QUIET,
};
// F6-ParamMany: the same panel. The 2080 parameters that only take a keyboard's knobs are not listed (91-3); the
// line under the list says so.
SCENES.paramMany6 = { ...SCENES.paramMany };
// F-ParamSearch: the same while "cutoff" is in the search field, which is the screen's own, as is what the status
// line says then (tools/design/shoot_ui.py types it).
SCENES.paramSearch = {
  ...SCENES.paramMany,
  after: (s) => (s.status.message = { id: "summary", loopBars: null, nothingSelected: true, notes: null }),
};
// F5-MixerBypass: MVerb on 「4 パッド」 is passed by (13-A) and selected; "Brick EQ" on 「3 ドラム」 has stopped. When the
// mock's status line (「… をとばしています ・ … は止まっています…」) is said is not decided.
SCENES.mixerBypass = {
  setup(s) {
    surgeMixer(s);
    s.song.tracks[3]!.slots = [pluginSlot("MVerb", "ready", true), null, null, null];
    s.params = pluginSelected({ kind: "insert", insert: { strip: trackStrip(4), slot: 0 } }, FAKE_TABLES.mverb);
    s.transport.state = "playing";
    s.transport.position = at("9.2.000");
  },
  bands: ["mixer", "params"],
  meters: { ...WINDOW_METERS, loadPermille: 240 },
};
// R9-Inserts: 「入れかえる…」 on the compressor of 「2 ベース」 opened the shelf on the effects; 「EQ 4 バンド」 is chosen.
SCENES.insertReplace = {
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", null);
    s.params = { selected: null, clips: null };
    s.shelf = { ...closedShelf(), mode: { kind: "replaceEffect", strip: trackStrip(2), slot: 0 }, selectedEffect: "eq4", currentEffect: builtinEffect("compressor") };
  },
  bands: ["mixer", "shelf"],
  meters: QUIET,
};
// R8-MixerFx: 「空き ＋」 of the second slot of 「2 ベース」 opened the shelf on the effects; 「リバーブ」 is chosen.
SCENES.mixerFx = {
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", null);
    s.params = { selected: null, clips: null };
    s.shelf = { ...closedShelf(), mode: { kind: "insertEffect", strip: trackStrip(2), slot: 1 }, selectedEffect: "reverb" };
  },
  bands: ["mixer", "shelf"],
  meters: QUIET,
};
// R9-Params (86-5): the parameters panel with nothing selected.
SCENES.paramsNone = {
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", null);
    s.params = { selected: null, clips: null };
  },
  bands: ["mixer", "params"],
  meters: QUIET,
};
// R9-Params (86-8): no output is chosen, so the master's output slot says 「出口なし ▸」 and no meter moves.
SCENES.noOutput = {
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", null);
    s.params = { selected: null, clips: null };
    s.outputs = { ...s.outputs, chosen: null, chosenState: null };
    Object.assign(s.audio, { running: false, outputDevice: null });
    Object.assign(s.transport, { canPlay: false, canRecord: false });
  },
  bands: ["mixer"],
  meters: { velocity: 83, loadPermille: 0 },
};
// R9-Params (86-6, 86-10): "40" was typed for the ratio of the compressor of 「2 ベース」, and put at 20 : 1.
SCENES.paramClamped = {
  setup(s) {
    mixerSong(s, { strip: trackStrip(2), slot: 0 }, "compressor", 1);
    if (s.params.selected?.kind === "builtin") s.params.selected.values[1] = 20;
  },
  bands: ["mixer", "params"],
  after: (s) => (s.status.message = { id: "paramClamped", to: { kind: "builtin", effect: "compressor", param: 1, value: 20 } }),
  meters: QUIET,
};
// Opt5-Undo (15-C): back from the window of Surge XT, where three knobs were turned: one more step, which undo takes
// back. (What the history band lists is not made yet.)
SCENES.windowUndo = {
  setup(s) {
    surgeSong(s);
    s.library = fullLibrary();
    Object.assign(s.song, { playTarget: s.song.tracks[4]!.id, undoCount: 15, redoCount: 0, file: "changed" });
  },
  bands: ["history"],
  meters: QUIET,
};

// ---- round 6, step 1: tempo and meter changes, markers (protocol 20) ----

const MARKERS: TimelineView["markers"] = [
  { id: 1, tick: 0, name: "イントロ", section: "usable" as const },
  { id: 2, tick: 4 * BAR, name: "Aメロ", section: "usable" as const },
  { id: 3, tick: 9 * BAR, name: "サビ", section: "usable" as const },
];

/** The band's song with the three markers of R10-Markers (bars 1, 5 and 10), the play line at bar 7. */
function markerSong(s: AppState): void {
  bandSong(s);
  s.timeline.markers = MARKERS.map((m) => ({ ...m }));
  s.transport.position = 6 * BAR;
}

// F6-Tempo: the row 「テンポ ・ 拍子の変更」 is open. 104.00 at bar 5 goes to 120.00 at bar 9 in a ramp, 3/4 from
// bar 9, 96.00 and 4/4 from bar 13; the point of bar 5 is selected and the play line is at bar 7, where the
// transport reads 112.00 (the screen's tempo map gives it). The clips keep their bars under the changed meters.
SCENES.tempo = {
  setup(s) {
    surgeSong(s);
    const threeFour = 8 * BAR;
    const fourFour = threeFour + 4 * 3 * TPQ;
    s.timeline = {
      ...s.timeline,
      tempoPoints: [
        { tick: 4 * BAR, tempoBpm: 104, ramp: true },
        { tick: threeFour, tempoBpm: 120, ramp: false },
        { tick: fourFour, tempoBpm: 96, ramp: false },
      ],
      meterChanges: [
        { bar: 8, meterNum: 3, meterDen: 4 },
        { bar: 12, meterNum: 4, meterDen: 4 },
      ],
      markers: [
        { id: 1, tick: 0, name: "イントロ", section: "usable" as const },
        { id: 2, tick: 4 * BAR, name: "Aメロ", section: "usable" as const },
        { id: 3, tick: fourFour, name: "サビ", section: "usable" as const },
      ],
      selected: { kind: "tempoPoint", tick: 4 * BAR },
      open: true,
    };
    const map = new TempoMap(s.timeline);
    for (const track of s.song.tracks) {
      track.clips = track.clips.map((c) => ({ ...c, start: map.tickOfBar(c.start / BAR), end: map.tickOfBar(c.end / BAR) }));
    }
    Object.assign(s.song, { undoCount: 18, file: "changed", loopOn: true });
    s.transport.position = 6 * BAR;
    s.status.message = { id: "tempoRampSelected", bar: 5, beat: 1, tempoBpm: 104, toBar: 9, toBeat: 1, toTempoBpm: 120 };
  },
  meters: QUIET,
};

// F6-AutoRec, its row 「テンポ ・ 拍子の変更」: folded, with one change (120.00 from bar 13).
SCENES.tempoFolded = {
  setup(s) {
    bandSong(s);
    s.timeline.tempoPoints = [{ tick: 12 * BAR, tempoBpm: 120, ramp: false }];
    s.timeline.markers = MARKERS.map((m) => ({ ...m }));
  },
  meters: QUIET,
};

// R10-Markers, 「足した直後」: K at bar 7. The marker is selected and its name field is open.
SCENES.markerAdded = {
  setup(s) {
    markerSong(s);
    const added = { id: 4, tick: 6 * BAR, name: "マーカー 4", section: "usable" as const };
    s.timeline.markers = [MARKERS[0]!, MARKERS[1]!, added, MARKERS[2]!].map((m) => ({ ...m }));
    s.timeline.selected = { kind: "marker", id: added.id };
    s.timeline.renaming = added.id;
    s.status.message = { id: "markerAdded", name: added.name };
  },
  meters: QUIET,
};

// Round 12 A2 (no mock board): the markers of R10-Markers and two more, so that every state of a section is there.
// 「イントロ」 (bars 1–4) can be used; 「Aメロ」 ends at 「ブリッジ」, which is inside bar 7, so neither can; 「サビ」
// (bars 10–16) can; 「おわり」, at the song's end, has nothing after it.
SCENES.sections = {
  setup(s) {
    markerSong(s);
    const end = Math.max(...s.song.tracks.flatMap((t) => t.clips.map((c) => c.end)));
    s.timeline.markers = [
      ...MARKERS,
      { id: 4, tick: 6 * BAR + TPQ, name: "ブリッジ", section: "usable" as const },
      { id: 5, tick: Math.ceil(end / BAR) * BAR, name: "おわり", section: "usable" as const },
    ]
      .map((m) => ({ ...m }))
      .sort((a, b) => a.tick - b.tick);
    s.timeline.markers = withSections(s.song, s.timeline);
    s.transport.position = 0;
  },
  meters: QUIET,
};

// Round 12 A2: a section step that would make the song longer than it can be did nothing.
SCENES.songTooLong = {
  setup: (s) => SCENES.sections!.setup(s),
  after: (s) => (s.status.message = { id: "songTooLong" }),
  meters: QUIET,
};

// R10-Markers, 「名前を変えている」: the name field of 「Aメロ」 is open (what is typed in it is the screen's own).
SCENES.markerRenaming = {
  setup(s) {
    markerSong(s);
    s.timeline.selected = { kind: "marker", id: 2 };
    s.timeline.renaming = 2;
  },
  meters: QUIET,
};

// R10-Markers, 「動かしている」: 「Aメロ」 is selected; the drag to 6. 1. 000 is the screen's own, until it is let go.
SCENES.markerMoving = {
  setup(s) {
    markerSong(s);
    s.timeline.selected = { kind: "marker", id: 2 };
  },
  meters: QUIET,
};

// R10-Markers, 「ピアノロールの目盛」: the piano roll of A-PianoRoll with markers in its ruler (names only, not pressable).
SCENES.pianoRollMarkers = {
  setup(s) {
    pianoRollSong(s);
    s.timeline.markers = [
      { id: 1, tick: 0, name: "イントロ", section: "usable" as const },
      { id: 2, tick: 2 * BAR + 2 * TPQ, name: "Aメロ", section: "usable" as const },
      { id: 3, tick: 4 * BAR, name: "サビ", section: "usable" as const },
    ];
  },
  bands: ["pianoRoll"],
  meters: QUIET,
};

// ---- round 6, steps 2 and 3: automation and the history band (protocol 21) ----

/** The points of a lane: `[tick, value]`, the value the place of the knob (0..1). */
interface FakeLane {
  id: number;
  step: boolean;
  points: [number, number][];
}

/** Writes the lane binary (docs/architecture.md 21-15). */
export function encodeLane(l: FakeLane): ArrayBuffer {
  const out = new ArrayBuffer(LANE_HEADER_LEN + LANE_POINT_LEN * l.points.length);
  const view = new DataView(out);
  view.setUint8(0, LANE_KIND);
  view.setUint8(1, LANE_VERSION);
  view.setUint8(2, l.step ? 1 : 0);
  view.setUint32(4, l.id, true);
  view.setUint32(8, l.points.length, true);
  l.points.forEach(([tick, value], i) => {
    const p = LANE_HEADER_LEN + LANE_POINT_LEN * i;
    view.setUint32(p, tick % 2 ** 32, true);
    view.setUint32(p + 4, Math.floor(tick / 2 ** 32), true);
    view.setFloat32(p + 8, value, true);
  });
  return out;
}

const FAKE_LANES = new Map<number, FakeLane>();
const fakeLane = (id: number, points: [string, number][], step = false): FakeLane => {
  const lane: FakeLane = { id, step, points: points.map(([place, value]) => [at(place), value]) };
  FAKE_LANES.set(id, lane);
  return lane;
};
const laneView = (target: AutoTarget, name: LaneName, lane: FakeLane, resting = false): LaneView => ({
  target,
  name,
  laneId: lane.id,
  points: lane.points.length,
  step: lane.step,
  resting,
});

// The fader's places are those of spec 2 「メーターとフェーダーの目盛」: 0.80 is 0 dB, 0.72 −2.4, 0.64 −4.8,
// 0.52 −8.4, 0.45 −10.5, 0.40 −12.
/** The volume of 「1 ピアノ」: −4.8 dB at bar 8 (F6-AutoRec, R10-AutoEdit 「いま −4.8 dB」), −2.4 dB at 9.2.000 (F6-MixerAuto). 5 points (F6-ParamMany 「点 5」). */
const PIANO_VOLUME = fakeLane(501, [["1.1.000", 0.8], ["5.1.000", 0.64], ["8.1.000", 0.64], ["9.2.000", 0.72], ["13.1.000", 0.6]]);
const PIANO_PAN = fakeLane(502, [["1.1.000", 0.5], ["9.1.000", 0.4]]);
const PIANO_THRESHOLD = fakeLane(503, [["1.1.000", 0.6], ["9.1.000", 0.45], ["13.1.000", 0.6]]);
/** The volume of 「4 パッド」: −10.5 dB at 9.2.000, where the hand of F6-MixerRest takes it to −12. */
const PAD_VOLUME = fakeLane(504, [["1.1.000", 0.4], ["9.1.000", 0.45], ["13.1.000", 0.45]]);
const SURGE_CUTOFF = fakeLane(505, [["1.1.000", 0.6], ["5.1.000", 0.75], ["9.1.000", 0.5]]);
const SURGE_PITCH = fakeLane(506, [["1.1.000", 0.5], ["13.1.000", 0.5]]);
/** R10-AutoEdit: the line of the board over bars 1 to 8, and the same after a press at 5.3.000 put a point at −8.4 dB. */
const EDIT_VOLUME = fakeLane(510, [["1.1.000", 0.64], ["3.1.000", 0.73], ["4.1.000", 0.6], ["6.1.000", 0.45], ["7.3.000", 0.64]]);
const EDIT_VOLUME_ADDED = fakeLane(511, [["1.1.000", 0.64], ["3.1.000", 0.73], ["4.1.000", 0.6], ["5.3.000", 0.52], ["6.1.000", 0.45], ["7.3.000", 0.64]]);

const VOLUME: AutoTarget = { kind: "volume" };
const laneOf = (number: number, target: AutoTarget): LaneRef => ({ home: { kind: "track", track: BAND_TRACK_ID + number }, target });
const PIANO_REF: TrackRef = { number: 1, name: "ピアノ" };
const PAD_REF: TrackRef = { number: 4, name: "パッド" };

/**
 * The song of the mocks F6-Auto* and F6-Mixer*: the strips of F5-ShelfFx (EQ and compressor on 「1 ピアノ」, MVerb on
 * 「4 パッド」), playing. 「1 ピアノ」 has three lanes: the volume, and folded the pan and the compressor's threshold
 * (the mock's 「▸ パン ▸ プラグインのパラメータ 1」).
 */
function autoSong(s: AppState, position: string): void {
  surgeMixer(s);
  s.song = settleSong(s.song);
  const piano = s.song.tracks[0]!;
  const threshold: AutoTarget = { kind: "insert", insert: piano.slots[1]!.id, param: 0 };
  piano.lanes = [
    laneView(VOLUME, { kind: "volume" }, PIANO_VOLUME),
    laneView({ kind: "pan" }, { kind: "pan" }, PIANO_PAN),
    laneView(threshold, { kind: "builtin", effect: "compressor", param: 0 }, PIANO_THRESHOLD),
  ];
  s.automation.folded = [laneOf(1, { kind: "pan" }), laneOf(1, threshold)];
  s.timeline.tempoPoints = [{ tick: 12 * BAR, tempoBpm: 120, ramp: false }];
  s.timeline.markers = MARKERS.map((m) => ({ ...m }));
  s.song.loopOn = true;
  s.transport.state = "playing";
  s.transport.position = at(position);
}
const autoMeters = (position: string): FakeMeters => ({ ...QUIET, loadPermille: 140, master: [dbfs(-6.2), dbfs(-7)], position: playingAt(position) });

// F6-AutoRec: 「● 書く」 of the volume lane of 「1 ピアノ」 is on, the song plays and the fader is held: what is
// written since 6.3.000 is in the rec frame, up to the play line at bar 8.
SCENES.autoRec = {
  setup(s) {
    autoSong(s, "8.1.000");
    Object.assign(s.song, { undoCount: 16, file: "changed" });
    s.automation.armed = [laneOf(1, VOLUME)];
    s.automation.writing = [{ lane: laneOf(1, VOLUME), fromTick: at("6.3.000") }];
  },
  after: (s) => (s.status.message = { id: "automationWriting", track: PIANO_REF, target: { kind: "volume" } }),
  meters: autoMeters("8.1.000"),
};
// F6-AutoRest: the fader of 「1 ピアノ」 was moved by hand to −12 dB: its lane rests (17-A).
SCENES.autoRest = {
  setup(s) {
    autoSong(s, "8.1.000");
    Object.assign(s.song, { undoCount: 17, file: "changed" });
    const piano = s.song.tracks[0]!;
    piano.gainDb = -12;
    piano.lanes[0]!.resting = true;
  },
  after: (s) => (s.status.message = { id: "automationRested", track: PIANO_REF, target: { kind: "volume" } }),
  meters: autoMeters("8.1.000"),
};
// F6-MixerAuto: the mixer while the lanes move the faders of 「1 ピアノ」 (−2.4 dB at the play line) and 「4 パッド」.
SCENES.mixerAuto = {
  setup(s) {
    autoSong(s, "9.2.000");
    Object.assign(s.song, { undoCount: 16, file: "saved" });
    s.song.tracks[3]!.lanes = [laneView(VOLUME, { kind: "volume" }, PAD_VOLUME)];
  },
  bands: ["mixer"],
  after: (s) => (s.status.message = { id: "automationMoving", tracks: [PIANO_REF, PAD_REF], master: false }),
  meters: autoMeters("9.2.000"),
};
// F6-MixerRest: the fader of 「4 パッド」 was taken to −12 dB by hand; under it 「休み」 and 「もどす」.
SCENES.mixerRest = {
  setup(s) {
    autoSong(s, "9.2.000");
    Object.assign(s.song, { undoCount: 16, file: "saved" });
    const pad = s.song.tracks[3]!;
    pad.gainDb = -12;
    pad.lanes = [laneView(VOLUME, { kind: "volume" }, PAD_VOLUME, true)];
  },
  bands: ["mixer"],
  after: (s) => (s.status.message = { id: "automationRested", track: PAD_REF, target: { kind: "volume" } }),
  meters: autoMeters("9.2.000"),
};

// Round 12 M5 (the spec's mock is not a board): the master's lane, of its volume.
const MASTER_LANE: LaneRef = { home: { kind: "master" }, target: VOLUME };
/** The master's volume in the song of F6-Mixer*: flat at 0 dB, then down from bar 9 to −12 dB at bar 13 (−6 dB at 11.1.000). */
const MASTER_VOLUME = fakeLane(521, [["1.1.000", 0.8], ["9.1.000", 0.8], ["13.1.000", 0.4]]);
/** The band's song with a fade-out of `bars` bars put on the master by 「フェードアウトをつける」. */
function masterFadeSong(s: AppState, bars: number): void {
  bandSong(s);
  const fade = fakeFadeOut(s.song, s.timeline, null, bars);
  if (fade === null || fade === "noClips") return;
  const lane: FakeLane = { id: 520, step: false, points: fade.points };
  FAKE_LANES.set(lane.id, lane);
  s.song.master.lanes = [laneView(VOLUME, { kind: "volume" }, lane)];
  s.transport.position = 0;
  Object.assign(s.song, { undoCount: 15, file: "changed" });
}
// The arrange with the row 「マスター」 at the foot of the tracks: a fade-out over the last 4 bars of the song.
SCENES.masterFade = { setup: (s) => masterFadeSong(s, 4), meters: QUIET };
// The same with the master's lane folded: the row stays, as a line that opens it again.
SCENES.masterFadeFolded = {
  setup(s) {
    masterFadeSong(s, 4);
    s.automation.folded = [MASTER_LANE];
  },
  meters: QUIET,
};
// 「● 書く」 of the master's lane is on and the song plays: the fader is held since 6.3.000.
SCENES.masterFadeWriting = {
  setup(s) {
    masterFadeSong(s, 4);
    s.transport.state = "playing";
    s.transport.position = at("8.1.000");
    s.automation.armed = [MASTER_LANE];
    s.automation.writing = [{ lane: MASTER_LANE, fromTick: at("6.3.000") }];
  },
  after: (s) => (s.status.message = { id: "masterAutomationWriting" }),
  meters: { ...QUIET, position: playingAt("8.1.000") },
};
// 「編集 ▸ 曲の終わりにフェードアウトをつける…」 in a song with no clip.
SCENES.fadeOutNoClips = { setup: () => {}, after: (s) => (s.status.message = { id: "fadeOutNoClips" }), meters: QUIET };
// The mixer while the master's lane moves its fader (−6 dB at the play line), with the lanes of F6-MixerAuto.
SCENES.mixerMasterAuto = {
  setup(s) {
    autoSong(s, "11.1.000");
    Object.assign(s.song, { undoCount: 16, file: "saved" });
    s.song.master.lanes = [laneView(VOLUME, { kind: "volume" }, MASTER_VOLUME)];
  },
  bands: ["mixer"],
  after: (s) => (s.status.message = { id: "automationMoving", tracks: [PIANO_REF], master: true }),
  meters: autoMeters("11.1.000"),
};
// The master's fader was taken to −3 dB by hand: its lane rests; under the fader 「休み」 and 「もどす」.
SCENES.mixerMasterRest = {
  setup(s) {
    autoSong(s, "11.1.000");
    Object.assign(s.song, { undoCount: 17, file: "changed" });
    s.song.master.gainDb = -3;
    s.song.master.lanes = [laneView(VOLUME, { kind: "volume" }, MASTER_VOLUME, true)];
  },
  bands: ["mixer"],
  after: (s) => (s.status.message = { id: "masterAutomationRested" }),
  meters: autoMeters("11.1.000"),
};

/** R10-AutoEdit: the band's song with the volume lane of 「1 ピアノ」 open, the play line at bar 8. */
function autoEditSong(s: AppState, lane: FakeLane, selected: IndexRange[]): void {
  bandSong(s);
  s.song.tracks[0]!.lanes = [laneView(VOLUME, { kind: "volume" }, lane)];
  s.automation.selected = selected.length > 0 ? { laneId: lane.id, points: selected } : null;
  s.transport.position = 7 * BAR;
  Object.assign(s.song, { undoCount: 15, file: "changed" });
}
// R10-AutoEdit, 「書いている」: a press on the line at 5.3.000 put a point there (−8.4 dB); it is selected.
SCENES.autoAdd = { setup: (s) => autoEditSong(s, EDIT_VOLUME_ADDED, [{ start: 3, end: 4 }]), meters: QUIET };
// R10-AutoEdit, 「動かしている」: the point of bar 3 is selected; the drag itself is the screen's own
// (tools/design/shoot_ui.py drags it), with the reading 「+0. 1. 000 ・ −2.1 → −0.6 dB」.
SCENES.autoMove = { setup: (s) => autoEditSong(s, EDIT_VOLUME, [{ start: 1, end: 2 }]), meters: QUIET };
// R10-AutoEdit, 「囲んでえらんだ」: three points are selected.
SCENES.autoBox = { setup: (s) => autoEditSong(s, EDIT_VOLUME, [{ start: 1, end: 4 }]), meters: QUIET };

// F6-ParamMany: Surge XT is the sound of 「1 ピアノ」; the lanes of its two parameters are folded under the volume
// lane (「▸ プラグインのパラメータ 2」), and the panel lists them first, under 「オートメーションに書いている」.
SCENES.paramMany6 = {
  ...SCENES.paramMany!,
  setup(s) {
    SCENES.paramMany!.setup(s);
    const [cutoff, pitch] = SURGE_AUTOMATED.map((param): AutoTarget => ({ kind: "instrument", param })) as [AutoTarget, AutoTarget];
    s.song.tracks[0]!.lanes = [
      laneView(VOLUME, { kind: "volume" }, PIANO_VOLUME),
      laneView(cutoff, { kind: "plugin", plugin: "Surge XT", param: "A Filter 1 Cutoff" }, SURGE_CUTOFF),
      laneView(pitch, { kind: "plugin", plugin: "Surge XT", param: "A Osc 1 Pitch" }, SURGE_PITCH),
    ];
    s.automation.folded = [laneOf(1, cutoff), laneOf(1, pitch)];
    s.transport.position = 7 * BAR;
  },
};

const row = (index: number, name: HistoryRow["name"], what: HistoryRow["what"], place: HistoryRow["place"]): HistoryRow => ({ index, step: index, name, what, place });
const trackRef = (number: number): TrackRef => ({ number, name: BAND_TRACKS[number - 1]!.name });
const ARRANGE: HistoryRow["place"] = { kind: "arrange" };
const PIANO_ROLL: HistoryRow["place"] = { kind: "pianoRoll" };
const PAD_GAIN: HistoryRow["what"] = { kind: "gain", strip: { kind: "track", track: PAD_REF }, fromDb: -10.5, toDb: -12 };
/** The first steps of the band's song; the mocks list only what comes after them. */
const FIRST_STEPS: HistoryRow[] = [
  row(1, { kind: "addTrack" }, { kind: "track", track: trackRef(2) }, ARRANGE),
  row(2, { kind: "addTrack" }, { kind: "track", track: trackRef(3) }, ARRANGE),
  row(3, { kind: "addTrack" }, { kind: "track", track: trackRef(4) }, ARRANGE),
  row(4, { kind: "record" }, { kind: "take", number: 1 }, { kind: "track", track: trackRef(3) }),
  row(5, { kind: "record" }, { kind: "take", number: 1 }, { kind: "track", track: trackRef(2) }),
  row(6, { kind: "moveNotes" }, { kind: "notes", count: 4 }, PIANO_ROLL),
  row(7, { kind: "quantize" }, { kind: "notes", count: 32 }, PIANO_ROLL),
  row(8, { kind: "duplicateClips" }, { kind: "clips", count: 1 }, ARRANGE),
];
// F-History: 14 steps, the last 2 undone. The rows 7 to 14 are the mock's, in the wording of Y19.
SCENES.historyList = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 5));
    Object.assign(s.song, { undoCount: 12, redoCount: 2, file: "changed" });
    s.song.loopOn = false;
  },
  bands: ["history"],
  after: (s) => (s.status.message = { id: "undoPending", count: 2 }),
  meters: QUIET,
  history: [
    ...FIRST_STEPS,
    row(9, { kind: "addMarker" }, { kind: "name", name: "サビ" }, ARRANGE),
    row(10, { kind: "setLoop" }, { kind: "bars", span: { from: 5, to: 9 } }, ARRANGE),
    row(11, { kind: "record" }, { kind: "take", number: 1 }, { kind: "track", track: PIANO_REF }),
    row(12, { kind: "record" }, { kind: "take", number: 2 }, { kind: "track", track: PIANO_REF }),
    row(13, { kind: "quantize" }, { kind: "notes", count: 3 }, PIANO_ROLL),
    row(14, { kind: "setGain" }, PAD_GAIN, { kind: "mixer" }),
  ],
};
// R10-History: 16 steps, the last 3 undone (「いまここ」 is 13). The rows 10 to 16 are the board's; which row the
// pointer or the keys are on (the board: 11) is the screen's own.
SCENES.historyUndone3 = {
  setup(s) {
    surgeMixer(s);
    Object.assign(s.song, { undoCount: 13, redoCount: 3, file: "changed" });
  },
  bands: ["history"],
  after: (s) => (s.status.message = { id: "undoPending", count: 3 }),
  meters: QUIET,
  history: [
    ...FIRST_STEPS,
    row(9, { kind: "setLoop" }, { kind: "bars", span: { from: 5, to: 9 } }, ARRANGE),
    row(10, { kind: "setTempo" }, { kind: "tempo", bar: 5, beat: 1, fromBpm: 96, toBpm: 120 }, ARRANGE),
    row(11, { kind: "addMarker" }, { kind: "name", name: "サビ" }, ARRANGE),
    row(12, { kind: "bypassEffect" }, { kind: "effect", effect: { kind: "plugin", name: "MVerb" } }, { kind: "track", track: PAD_REF }),
    row(13, { kind: "setGain" }, PAD_GAIN, { kind: "mixer" }),
    row(14, { kind: "quantize" }, { kind: "notes", count: 12 }, PIANO_ROLL),
    row(15, { kind: "resizeNotes" }, { kind: "notes", count: 2 }, PIANO_ROLL),
    row(16, { kind: "moveNotes" }, { kind: "notes", count: 3 }, PIANO_ROLL),
  ],
};
// 5000 steps, none undone: for timing the band (open, scroll, jump 500). The rows after the first ones are made up.
SCENES.history5000 = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 5));
    Object.assign(s.song, { undoCount: 5000, redoCount: 0, file: "changed" });
  },
  bands: ["history"],
  meters: QUIET,
  history: FIRST_STEPS,
};
// Round 12, the owner's answers (B-97-1, B-97-2; no mock): the rows of mute sets and of a batch of the AI link.
SCENES.historyMuteSets = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 5));
    Object.assign(s.song, { undoCount: 13, redoCount: 0, file: "changed" });
  },
  bands: ["history"],
  meters: QUIET,
  history: [
    ...FIRST_STEPS,
    row(9, { kind: "saveMuteSet" }, { kind: "name", name: "サビ" }, { kind: "mixer" }),
    row(10, { kind: "recallMuteSet" }, { kind: "name", name: "サビ" }, { kind: "mixer" }),
    row(11, { kind: "renameMuteSet" }, { kind: "renamed", from: "サビ", to: "大サビ" }, { kind: "mixer" }),
    row(12, { kind: "deleteMuteSet" }, { kind: "name", name: "ドラムだけ" }, { kind: "mixer" }),
    row(13, { kind: "batch", count: 15 }, { kind: "none" }, null),
  ],
};

// ---- round 7, step 0-2: the answers to 92 (spec 11th edition, F7-Answers92 and R11-Answers92) ----

/** F7-Answers92 and Opt7-LaneScale: the volume of 「1 ピアノ」 moves only between −9.0 and −3.0 dB (5 points, −4.8 dB at bar 5). */
const ANSWERS_VOLUME = fakeLane(520, [["1.1.000", 0.64], ["5.1.000", 0.64], ["6.1.000", 0.7], ["7.3.000", 0.66], ["9.1.000", 0.5]]);
// F7-Answers92: the band's song, 「1 ピアノ」 (「グランドピアノ」) has its volume lane open; the lane shows the points'
// range (21-B, 「上 −3.0 dB」「下 −9.0 dB」) and 「＋ 帯を足す ▾」 under it (20-A; tools/design/shoot_ui.py opens the list).
SCENES.laneScale = {
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 5));
    s.song.tracks[0]!.lanes = [laneView(VOLUME, { kind: "volume" }, ANSWERS_VOLUME)];
    s.timeline.markers = MARKERS.map((m) => ({ ...m }));
    s.song.loopOn = true;
    s.transport.position = 4 * BAR;
    Object.assign(s.song, { undoCount: 14, file: "saved" });
  },
  bands: ["params"],
  meters: QUIET,
};
// R11-Answers92, 「履歴の帯の見出し」: no step undone, every step undone, no step at all (spec 6).
const historyHead = (undo: number, redo: number): SceneDef => ({
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 5));
    Object.assign(s.song, { undoCount: undo, redoCount: redo, file: undo + redo > 0 ? "changed" : "saved" });
  },
  bands: ["history"],
  meters: QUIET,
  history: undo + redo > 0 ? SCENES.historyList!.history : [],
});
SCENES.historyLatest = historyHead(14, 0);
SCENES.historyAllUndone = historyHead(0, 14);
SCENES.historyEmpty = historyHead(0, 0);
// R11-Answers92, 「曲の頭」: the tempo point at the head of the song is selected (92-2).
SCENES.headTempo = {
  ...SCENES.tempo!,
  after: (s) => {
    s.timeline.selected = { kind: "tempoPoint", tick: 0 };
    s.status.message = { id: "tempoPointSelected", bar: 1, beat: 1, tempoBpm: 96 };
  },
};
// R11-Answers92, 「ピアノロールの目盛のマーカー」: a marker at bar 2 in the piano roll's ruler, over the beat numbers.
SCENES.pianoRollMarker = {
  setup(s) {
    pianoRollSong(s);
    s.timeline.markers = [{ id: 1, tick: BAR, name: "サビ", section: "usable" as const }];
  },
  bands: ["pianoRoll"],
  meters: QUIET,
};

/**
 * Protocol 21, enough for pictures and tests: the lanes' own switches (fold, 「● 書く」, the selection, 「もどす」),
 * lanes added and deleted, and the jump of the history band. Points are not added, moved or deleted here, and no
 * step is listed. `null`: not one of these commands.
 */
function reduceRound6(state: AppState, command: Command, rows: readonly HistoryRow[]): Change[] | null {
  const same = (a: LaneRef, b: LaneRef) => JSON.stringify(a.home) === JSON.stringify(b.home) && JSON.stringify(a.target) === JSON.stringify(b.target);
  const without = (list: LaneRef[], lane: LaneRef) => list.filter((l) => !same(l, lane));
  const automation = structuredClone(state.automation);
  const song = structuredClone(state.song);
  let message: StatusMessage | null = null;
  /** The track of a lane's home, or the master (round 12 M5: it has a lane of its volume only). */
  const homeOf = (lane: LaneRef) => (lane.home.kind === "master" ? song.master : song.tracks.find((t) => lane.home.kind === "track" && t.id === lane.home.track));
  const found = (lane: LaneRef) => {
    const track = homeOf(lane);
    const index = track ? track.lanes.findIndex((l) => JSON.stringify(l.target) === JSON.stringify(lane.target)) : -1;
    return track && index >= 0 ? { track, index, view: track.lanes[index]! } : null;
  };
  const isTrack = (home: (typeof song.tracks)[number] | typeof song.master): home is (typeof song.tracks)[number] => "clips" in home;
  const step = () => {
    song.undoCount += 1;
    song.redoCount = 0;
  };
  switch (command.type) {
    case "foldLane": {
      const lane: LaneRef = { home: command.home, target: command.target };
      if (!found(lane)) return [];
      automation.folded = command.folded ? [...without(automation.folded, lane), lane] : without(automation.folded, lane);
      break;
    }
    case "setLaneWrite": {
      const lane: LaneRef = { home: command.home, target: command.target };
      const had = found(lane);
      if (!had || had.view.resting) return [];
      automation.armed = command.on ? [...without(automation.armed, lane), lane] : without(automation.armed, lane);
      break;
    }
    case "selectAutoPoints":
      if (![...song.tracks, song.master].some((t) => t.lanes.some((l) => l.laneId === command.lane))) return [];
      automation.selected = command.points.length > 0 ? { laneId: command.lane, points: toRanges(command.points) } : null;
      break;
    case "restoreLane": {
      const had = found({ home: command.home, target: command.target });
      if (!had?.view.resting) return [];
      had.view.resting = false;
      song.undoCount += 1;
      song.redoCount = 0;
      message =
        isTrack(had.track)
          ? { id: "automationRestored", track: { number: had.track.number, name: had.track.name }, target: had.view.name }
          : { id: "masterAutomationRestored" };
      break;
    }
    case "deleteLane": {
      const lane: LaneRef = { home: command.home, target: command.target };
      const had = found(lane);
      if (!had) return [];
      had.track.lanes.splice(had.index, 1);
      song.undoCount += 1;
      song.redoCount = 0;
      automation.folded = without(automation.folded, lane);
      automation.armed = without(automation.armed, lane);
      if (automation.selected?.laneId === had.view.laneId) automation.selected = null;
      break;
    }
    case "addLane": {
      const lane: LaneRef = { home: command.home, target: command.target };
      const track = homeOf(lane);
      if (!track) return [];
      automation.folded = without(automation.folded, lane);
      if (found(lane)) break;
      const name = isTrack(track) ? fakeLaneName(track, lane.target) : lane.target.kind === "volume" ? ({ kind: "volume" } as const) : null;
      if (!name) return [];
      const id = Math.max(600, ...FAKE_LANES.keys()) + 1;
      const made: FakeLane = { id, step: lane.target.kind === "mute", points: [] };
      FAKE_LANES.set(id, made);
      track.lanes.push(laneView(lane.target, name, made));
      song.undoCount += 1;
      song.redoCount = 0;
      break;
    }
    case "historyJump": {
      const { total, current } = state.history;
      const to = fakeHistoryRows(rows, total, command.index, command.index)[0];
      if (!to || to.step !== command.step || command.index === current) return [];
      song.undoCount = command.index;
      song.redoCount = total - command.index;
      message = { id: "historyJumped", index: command.index, name: to.name, steps: Math.abs(command.index - current), forward: command.index > current };
      break;
    }
    case "addAutoPoint":
    case "moveAutoPoints":
    case "deleteAutoPoints":
      return [];
    // Round 12 M5: the points of the master's lane are made anew under a new id, as Rust does; no row of the history.
    case "addFadeOut": {
      if (state.transport.state === "recording" || state.transport.state === "countIn") return [];
      const lane: LaneRef = { home: { kind: "master" }, target: { kind: "volume" } };
      const had = found(lane);
      const old = had ? FAKE_LANES.get(had.view.laneId) : undefined;
      const fade = fakeFadeOut(song, state.timeline, had ? { points: old?.points ?? [], resting: had.view.resting } : null, command.bars);
      if (fade === null) return [];
      if (fade === "noClips") {
        message = { id: "fadeOutNoClips" };
        break;
      }
      if (had && !had.view.resting && JSON.stringify(old?.points) === JSON.stringify(fade.points)) return [];
      const made: FakeLane = { id: Math.max(600, ...FAKE_LANES.keys()) + 1, step: false, points: fade.points };
      FAKE_LANES.set(made.id, made);
      song.master.lanes = [laneView(lane.target, { kind: "volume" }, made)];
      if (automation.selected?.laneId === had?.view.laneId) automation.selected = null;
      step();
      break;
    }
    default:
      return null;
  }
  const changes: Change[] = [];
  if (JSON.stringify(song) !== JSON.stringify(state.song)) {
    changes.push({ section: "song", value: song });
    changes.push({ section: "history", value: { ...historyOf(song), newest: state.history.newest } });
  }
  if (JSON.stringify(automation) !== JSON.stringify(state.automation)) changes.push({ section: "automation", value: automation });
  if (message) changes.push({ section: "status", value: { message } });
  return changes;
}

/** What a new lane of the fake is called; `null` for a target the track does not have. */
function fakeLaneName(track: TrackView, target: AutoTarget): LaneName | null {
  switch (target.kind) {
    case "volume":
    case "pan":
    case "mute":
      return { kind: target.kind };
    case "send":
      return { kind: "send", bus: target.bus };
    case "instrument":
      return track.sound.kind === "plugin" ? { kind: "plugin", plugin: track.sound.name, param: null } : null;
    case "insert": {
      const effect = track.slots.find((i) => i?.id === target.insert)?.effect;
      if (!effect) return null;
      return effect.kind === "builtin" ? { kind: "builtin", effect: effect.effect, param: target.param } : { kind: "plugin", plugin: effect.name, param: null };
    }
  }
}

function sceneFromUrl(): Scene | null {
  if (typeof location === "undefined") return null;
  const q = new URLSearchParams(location.search);
  const name = q.get("scene");
  if (!name) return null;
  const vp = typeof window === "undefined" ? undefined : { width: window.innerWidth, height: window.innerHeight };
  const scale = q.get("scale");
  return fakeScene(name, scale === null ? undefined : Number(scale), vp, themeFromUrl() ?? undefined);
}

/** `?theme=<light|pinkDark|…>` (the names of design/themes.ts): the fake starts in that theme, with a scene or without. */
function themeFromUrl(): Theme | null {
  if (typeof location === "undefined") return null;
  const asked = new URLSearchParams(location.search).get("theme");
  return THEME_IDS.find((id) => id === asked) ?? null;
}

// ---- round 7, step 2: vocal tracks, the input and the latency (protocol 23) ----
// The states of the mocks F7-VocalTrack, F7-VocalRec, F7-TwoRec, F7-Latency and F7-SettingsAdv. The
// fake answers none of the new commands yet.

const VOCAL_TRACK_ID = BAND_TRACK_ID + 6;
const VOCAL_REF: TrackRef = { number: 6, name: "歌" };
const USB_MIC: EndpointView = {
  id: "in-usb-mic",
  name: "USB マイク",
  friendly: null,
  availability: "yes",
  link: "plug",
  nodeName: "alsa_input.usb-USB_Microphone-00.analog-stereo",
  device: "USB Microphone",
  unusable: null,
  channels: 2,
};

/** What comes in on the vocal track of the scenes: a mono input at `db`. */
const micAt = (db: number, clipped = false): MicMeter => ({ track: VOCAL_TRACK_ID, peak: [dbfs(db), dbfs(db)], clipped, stereo: false });

/**
 * The five keys tracks of the band and 「6 歌」 below them, with a USB microphone of two channels as the input.
 * The keys stay on 「1 ピアノ」, the keys track selected last (B-94-1; the mocks say 「弾くと鳴る先 6 歌」).
 */
function vocalSong(s: AppState, vocal: { armed: boolean; monitor: boolean; selected: boolean; inputReady?: boolean }): void {
  bandSong(s, BAND_TRACKS.slice(0, 5));
  const keys = s.song.tracks[0]!;
  s.song.tracks.push({
    ...keys,
    id: VOCAL_TRACK_ID,
    number: 6,
    name: "歌",
    color: 1,
    kind: { kind: "vocal", input: "mono1", inputReady: vocal.inputReady ?? true, armed: vocal.armed, monitor: vocal.monitor },
    instrument: "sine",
    sound: builtinSound("sine"),
    gainDb: 0,
    sends: [null, null],
    sendDefaults: [null, null],
    clips: [],
  });
  s.song.playTarget = vocal.selected ? VOCAL_TRACK_ID : keys.id;
  s.song.keysTarget = keys.id;
  s.song.file = "changed";
  const ready = vocal.inputReady ?? true;
  s.inputs = { list: ready ? [{ ...USB_MIC }, ...PC_INPUTS.map((i) => ({ ...i }))] : [], chosen: ready ? USB_MIC.id : null, open: ready };
  Object.assign(s.audio, { latency: { reportedMs: 8, measuredMs: null, manualMs: null, used: "reported", usedMs: 8 } });
}

// F7-VocalTrack: 「＋ 歌のトラック」 just made 「6 歌」; it is selected, so its ● is on. The list under 「入口 1 ・ モノ ▾」
// is the screen's own.
SCENES.vocalTrack = {
  setup(s) {
    vocalSong(s, { armed: true, monitor: false, selected: true });
    s.song.undoCount = 15;
  },
  after: (s) => (s.status.message = { id: "vocalTrackAdded", track: VOCAL_REF, input: "mono1" }),
  meters: { ...QUIET, mics: [micAt(-32)] },
};

// The same with no input on the PC: 「入口なし ▾」, and the status line says what to do.
SCENES.vocalNoInput = {
  setup: (s) => vocalSong(s, { armed: true, monitor: false, selected: true, inputReady: false }),
  after: (s) => (s.status.message = { id: "noInput" }),
  meters: QUIET,
};

// The first time 「モニター」 is turned on.
SCENES.vocalMonitorOn = {
  setup: (s) => vocalSong(s, { armed: true, monitor: true, selected: true }),
  after: (s) => (s.status.message = { id: "monitorFirstOn", latencyMs: 13 }),
  meters: { ...QUIET, mics: [micAt(-18)] },
};

// The input reached 0 dB: the right end of the meter and the dB reading are rec.
SCENES.vocalTooLoud = {
  setup: (s) => vocalSong(s, { armed: true, monitor: false, selected: true }),
  after: (s) => (s.status.message = { id: "micTooLoud" }),
  meters: { ...QUIET, mics: [micAt(0, true)] },
};

// E with 「6 歌」 selected.
SCENES.vocalNoPianoRoll = {
  setup: (s) => vocalSong(s, { armed: true, monitor: false, selected: true }),
  after: (s) => (s.status.message = { id: "vocalHasNoPianoRoll" }),
  meters: { ...QUIET, mics: [micAt(-32)] },
};

/** Recording from bar 5, the play line at 7. 2. 0000 (the mocks F7-VocalRec and F7-TwoRec). */
function vocalRecording(s: AppState, keys: boolean): void {
  s.transport.state = "recording";
  s.transport.position = 4 * BAR;
  s.transport.metronome = true;
  s.transport.recording = keys ? { id: 1, track: s.song.keysTarget!, from: 4 * BAR, take: 2, clip: null, lap: 0, overdub: false } : null;
  s.transport.recordingVocals = [{ track: VOCAL_TRACK_ID, from: 4 * BAR, take: 1, clip: null, audio: RECORDING_AUDIO.id, sampleRate: 48000, firstFrame: 0 }];
}
const VOCAL_REC_AT: TransportReading = { tick: 6 * BAR + TPQ, ticksPerUs: 0, state: "recording", countInBeatsLeft: 0, lap: 0 };

// F7-VocalRec: only the voice is recorded (「6 歌」 is selected and no keys are played), the monitor is on.
SCENES.vocalRec = {
  setup(s) {
    vocalSong(s, { armed: true, monitor: true, selected: true });
    vocalRecording(s, false);
    s.status.message = { id: "recordingVocals", keys: null, vocals: [{ track: VOCAL_REF, input: "mono1" }], monitor: true };
  },
  meters: { ...QUIET, loadPermille: 90, mics: [micAt(-12)], position: VOCAL_REC_AT },
};

// F7-TwoRec: 「1 ピアノ」 is selected and the ● of 「6 歌」 is on; R records the keys and the voice at once.
SCENES.twoRec = {
  setup(s) {
    vocalSong(s, { armed: true, monitor: true, selected: false });
    vocalRecording(s, true);
    s.status.message = {
      id: "recordingVocals",
      keys: { number: 1, name: "ピアノ" },
      vocals: [{ track: VOCAL_REF, input: "mono1" }],
      monitor: true,
    };
  },
  meters: { ...QUIET, keys: [60, 64, 67], mics: [micAt(-14)], position: VOCAL_REC_AT },
  live: {
    seq: 3,
    recording: 1,
    notes: [
      { lap: 0, index: 0, key: 60, velocity: 83, start: 4 * BAR, length: TPQ },
      { lap: 0, index: 1, key: 64, velocity: 83, start: 5 * BAR, length: TPQ },
      { lap: 0, index: 2, key: 67, velocity: 83, start: 6 * BAR, length: null },
    ],
  },
};

/** The four tracks of the mocks F7-Latency and F7-SettingsAdv, with the advanced sound settings open. */
function settingsAdv7(s: AppState): void {
  bandSong(s, BAND_TRACKS.slice(0, 4));
  s.inputs = { list: [{ ...USB_MIC }, ...PC_INPUTS.map((i) => ({ ...i }))], chosen: USB_MIC.id, open: false };
  s.workspace.settings = { draftOutput: HEADPHONES.id, advancedOpen: true };
  Object.assign(s.audio, { latency: { reportedMs: 8, measuredMs: null, manualMs: null, used: "reported", usedMs: 8 } });
  s.status.message = { id: "settingsOpen" };
}

// F7-SettingsAdv: meloet asks for 128 frames and the PC runs 256.
SCENES.settingsAdv7 = {
  setup(s) {
    settingsAdv7(s);
    Object.assign(s.audio, { requestedFrames: 128, actualFrames: 256, forcedQuantum: 256 });
  },
  meters: QUIET,
};

// F7-Latency: three of the five clicks came back.
SCENES.latency = {
  setup(s) {
    settingsAdv7(s);
    s.workspace.latencyDialog = { measure: { state: "measuring", done: 3, of: 5, meanMs: 12.4 } };
    s.transport.canRecord = false;
    s.status.message = { id: "latencyMeasureOpen" };
  },
  meters: QUIET,
};

// All five came back: 「この数を使う」 can be pressed.
SCENES.latencyDone = {
  setup(s) {
    settingsAdv7(s);
    s.workspace.latencyDialog = { measure: { state: "done", measuredMs: 12.4 } };
    s.status.message = { id: "latencyMeasureOpen" };
  },
  meters: QUIET,
};

// The clicks were too quiet: 「もう一度」.
SCENES.latencyFailed = {
  setup(s) {
    settingsAdv7(s);
    s.workspace.latencyDialog = { measure: { state: "failed", reason: "tooQuiet" } };
    s.status.message = { id: "latencyMeasureOpen" };
  },
  meters: QUIET,
};

// A song opened after meloet ended while a voice was being recorded: the hint about the recovery file has a
// line more.
SCENES.recoverVocal = {
  setup(s) {
    vocalSong(s, { armed: false, monitor: false, selected: false });
    s.workspace.recovery = {
      kind: "recovered",
      writtenMs: RECOVERY_WRITTEN,
      cause: "sudden",
      song: "はじめての曲",
      takes: 14,
      vocals: { count: 1, until: 8 * BAR + 2 * TPQ },
    };
    s.status.message = { id: "recovered" };
  },
  meters: QUIET,
};

// 19-A: the name of a sound that comes with meloet was pressed in a track's header.
SCENES.paramsSound = {
  setup(s) {
    bandSong(s);
    s.params.selected = { kind: "sound", track: BAND_TRACK_ID + 1, sound: { kind: "builtin", instrument: "grandPiano" }, knobs: null };
  },
  bands: ["params"],
  after: (s) => (s.status.message = { id: "noParams", sound: { kind: "builtin", instrument: "grandPiano" } }),
  meters: QUIET,
};

// Round 12 E3: the knobs of the piano, three of them turned, 「明るさ」 chosen.
SCENES.paramsSoundKnobs = {
  setup(s) {
    bandSong(s);
    const knobs = { ...fakeKnobs("grandPiano"), values: [35, 50, 70, 25] as [number, number, number, number], row: 0 };
    s.params.selected = { kind: "sound", track: BAND_TRACK_ID + 1, sound: { kind: "builtin", instrument: "grandPiano" }, knobs };
  },
  bands: ["params"],
  after: (s) =>
    (s.status.message = { id: "soundKnobSelected", loopBars: null, track: { number: 1, name: "ピアノ" }, knob: "brightness" }),
  meters: QUIET,
};
// Round 12 E3: a drum kit uses only 「明るさ」 and 「のび」; the other two rows are disabled.
SCENES.paramsSoundDrums = {
  setup(s) {
    bandSong(s);
    const knobs = { ...fakeKnobs("studioKit"), values: [50, 50, 30, 0] as [number, number, number, number] };
    s.params.selected = { kind: "sound", track: BAND_TRACK_ID + 3, sound: { kind: "builtin", instrument: "studioKit" }, knobs };
  },
  bands: ["params"],
  meters: QUIET,
};

// ---- Protocol 24: vocal clips, audio files brought in, files that are not found (mocks F7-AudioEdit, F7-AudioDrop,
// F7-AudioMissing, R11-Audio). The screen's parts come later; these are the states they will show. ----

/** Two bars at 96 a minute are five seconds. */
const TWO_BARS_FRAMES = 5 * 48000;
/** The file of the voice being recorded in `vocalRec` and `twoRec`: nine beats of it are written. */
const RECORDING_AUDIO: FakeAudio = { id: 59, file: "歌 59.wav", sampleRate: 48000, bits: 24, channels: 1, frames: 0, growingTo: 270000 };
const TAKE_1: FakeAudio = { id: 60, file: "歌 テイク 1.wav", sampleRate: 48000, bits: 24, channels: 1, frames: 2 * TWO_BARS_FRAMES };
const HARMONY: FakeAudio = { id: 61, file: "ハモり.wav", sampleRate: 48000, bits: 24, channels: 1, frames: TWO_BARS_FRAMES };
const LONG_MP3: FakeAudio = { id: 62, file: "ドラム.mp3", sampleRate: 44100, bits: null, channels: 2, frames: 180 * 44100, growingTo: 72 * 44100 };
const DRUMS_WAV: FakeAudio = { id: 63, file: "ドラム.wav", sampleRate: 44100, bits: 16, channels: 2, frames: 5 * 44100 };
/** What the `peaks` invoke of the fake knows, by the file's number in the audio table. */
/** The three takes of 「歌 1」 in `compTakes` (round 12, A8): four bars each, one file a take. */
const COMP_TAKES: FakeAudio[] = [66, 67, 68].map((id) => ({ ...TAKE_1, id, file: `歌 ${id}.wav` }));
export const FAKE_AUDIO = new Map<number, FakeAudio>(
  [RECORDING_AUDIO, TAKE_1, HARMONY, LONG_MP3, DRUMS_WAV, { ...TAKE_1, id: 64, missing: true }, { ...HARMONY, id: 65, missing: true }, ...COMP_TAKES].map((a) => [a.id, a]),
);
const LEFT_CLIP = 70;
const RIGHT_CLIP = 72;

const vocalTrackOf = (s: AppState) => s.song.tracks.find((t) => t.id === VOCAL_TRACK_ID)!;

/** 「6 歌」 with 「歌 テイク 1」 cut in two at bar 7 (S): the left half fades out, the right one fades in and is at −3.0 dB. */
function audioEditSong(s: AppState): void {
  vocalSong(s, { armed: true, monitor: false, selected: true });
  vocalTrackOf(s).clips = [
    fakeVocalClip(LEFT_CLIP, "歌 テイク 1", 4 * BAR, 6 * BAR, fakeAudioClip(TAKE_1, { fadeOut: TPQ })),
    // The same take: its tick 0 is two bars before this clip's start.
    fakeVocalClip(RIGHT_CLIP, "歌 テイク 1", 6 * BAR, 8 * BAR, fakeAudioClip(TAKE_1, { fadeIn: TPQ / 4, gainDb: -3 }), -2 * BAR),
  ];
  vocalTrackOf(s).clips[1]!.takeId = LEFT_CLIP + 1;
  s.arrange = { ...arrangeOf(RIGHT_CLIP), pianoRollClip: null };
  s.params.selected = { kind: "clip", track: VOCAL_TRACK_ID, clip: RIGHT_CLIP, missing: null, tuneUsable: true };
  s.transport.position = 6 * BAR;
  s.song.undoCount = 22;
  s.status.message = { id: "clipSplit", name: "歌 テイク 1", tick: 6 * BAR, count: 1 };
}

// F7-AudioEdit: the cut clips, the fades, the volume line, and the panel 「選んだクリップ」. (The mock's status line is
// the drag of the volume line, which is the screen's own.)
SCENES.audioEdit = { setup: audioEditSong, bands: ["params"], meters: QUIET };
// The same after a double click on a clip: the row of 「6 歌」 is 160 high.
SCENES.audioTall = {
  setup(s) {
    audioEditSong(s);
    s.arrange.tallTrack = VOCAL_TRACK_ID;
    s.status.message = { id: "summary", loopBars: { from: 5, to: 9 }, nothingSelected: false, notes: null };
  },
  bands: ["params"],
  meters: QUIET,
};

/** Files with audio among them are held over the window, the pointer at (`x`, `y`) of the page. */
const audioDrop = (name: string, files: number, sf2: number, x: number, y: number): SceneDef => ({
  setup(s) {
    vocalSong(s, { armed: false, monitor: false, selected: false });
    s.song.file = "saved";
    s.workspace.fileDrop = { name, addable: true, files, sf2, audio: files - sf2 };
  },
  after: (s) => (s.status.message = { id: "audioDragging", audio: files - sf2, sf2 }),
  meters: QUIET,
  events: [{ type: "fileDrag", x, y }],
});
// One file over 「6 歌」: the gray.7 frame is on that track, at the beat under the pointer.
SCENES.audioDropOne = audioDrop("ドラム.wav", 1, 0, 600, 540);
// F7-AudioDrop: eight files, the pointer over the left box (「別々のトラックに」) and over a keys track, so the frame is
// on a new row below the tracks.
SCENES.audioDrop = audioDrop("ドラム キック.wav", 8, 0, 520, 385);
// The pointer over the right box (「1 本にならべる」).
SCENES.audioDropRight = audioDrop("ドラム キック.wav", 8, 0, 760, 385);
// An SF2 file among them: 「SF2 は 1 つ棚へ ・ 音のファイル 3 つはアレンジへ」.
SCENES.audioDropMixed = audioDrop("ドラム キック.wav", 4, 1, 520, 385);

// F7-AudioMissing: a song opened on a PC where two of its audio files are not found.
SCENES.audioMissing = {
  setup(s) {
    vocalSong(s, { armed: false, monitor: false, selected: true });
    s.song.file = "saved";
    vocalTrackOf(s).clips = [
      fakeVocalClip(LEFT_CLIP, "歌 テイク 1", 4 * BAR, 6 * BAR, fakeAudioClip(FAKE_AUDIO.get(64)!)),
      fakeVocalClip(RIGHT_CLIP, "ハモり", 8 * BAR, 10 * BAR, fakeAudioClip(FAKE_AUDIO.get(65)!)),
    ];
    s.arrange = { ...arrangeOf(LEFT_CLIP), pianoRollClip: null };
    s.params.selected = {
      kind: "clip",
      track: VOCAL_TRACK_ID,
      clip: LEFT_CLIP,
      missing: { audio: 64, file: TAKE_1.file, wasAt: `はじめての曲/音/${TAKE_1.file}` },
      tuneUsable: false,
    };
  },
  bands: ["params"],
  after: (s) => (s.status.message = { id: "audioMissing", count: 2 }),
  meters: QUIET,
};

// A long mp3 was brought in: the clip is there and silent while the file is read (「読みこんでいます 40 %」).
SCENES.audioLoading = {
  setup(s) {
    vocalSong(s, { armed: false, monitor: false, selected: true });
    vocalTrackOf(s).clips = [fakeVocalClip(LEFT_CLIP, "ドラム", 4 * BAR, 76 * BAR, fakeAudioClip(LONG_MP3, fakeStretch(null, FAKE_SONG_BPM)))];
    s.audioWork = { jobs: [{ kind: "loading", audio: LONG_MP3.id, clip: null, percent: 40 }], bounce: null };
    s.song.undoCount = 15;
  },
  after: (s) => (s.status.message = { id: "audioImported", file: LONG_MP3.file, track: VOCAL_REF, tick: 4 * BAR, count: 1, skipped: 0 }),
  meters: QUIET,
};

// R11-Audio: the kinds of vocal clip side by side: plain, with fades and its volume line, with the marks of the
// stretching and 「音を作っています 34 %」, and one whose file is not found.
SCENES.audioKinds = {
  setup(s) {
    vocalSong(s, { armed: false, monitor: false, selected: true });
    const stretched = fakeStretch(steadyTempo(76.8), FAKE_SONG_BPM, { semitones: 2 });
    vocalTrackOf(s).clips = [
      fakeVocalClip(LEFT_CLIP, "歌 テイク 1", 0, 2 * BAR, fakeAudioClip(TAKE_1)),
      fakeVocalClip(RIGHT_CLIP, "歌 テイク 1", 2 * BAR, 4 * BAR, fakeAudioClip(TAKE_1, { fadeIn: TPQ, fadeOut: 2 * TPQ, gainDb: -3 }), -2 * BAR),
      fakeVocalClip(74, "ドラム", 4 * BAR, 6 * BAR, fakeAudioClip(DRUMS_WAV, stretched)),
      fakeVocalClip(76, "歌 テイク 1", 6 * BAR, 8 * BAR, fakeAudioClip(FAKE_AUDIO.get(64)!)),
    ];
    s.arrange = { ...arrangeOf(RIGHT_CLIP), pianoRollClip: null };
    s.params.selected = { kind: "clip", track: VOCAL_TRACK_ID, clip: RIGHT_CLIP, missing: null, tuneUsable: true };
    s.audioWork = { jobs: [{ kind: "rendering", audio: DRUMS_WAV.id, clip: 74, percent: 34 }], bounce: null };
  },
  bands: ["params"],
  meters: QUIET,
};

/** The clip of `compTakes`; its takes have the three ids after it. */
export const COMP_CLIP = 90;
// Round 12, A8: 「歌 1」 over bars 5–8 with three takes and 「いいとこ取り」: take 1, then take 3, then take 2 (the mock of the
// round's spec). The clip shows take 3. The band under the track is the screen's own (opened from the clip's menu).
SCENES.compTakes = {
  setup(s) {
    vocalSong(s, { armed: false, monitor: false, selected: true });
    const audio = fakeAudioClip(COMP_TAKES[2]!);
    audio.takes = fakeTakes(COMP_CLIP + 1, COMP_TAKES, audio);
    const clip = fakeVocalClip(COMP_CLIP, "歌 1", 4 * BAR, 8 * BAR, audio);
    Object.assign(clip, { take: 3, takes: 3, takeId: COMP_CLIP + 3 });
    clip.comp = [
      { from: 0, take: COMP_CLIP + 1 },
      { from: BAR + TPQ, take: COMP_CLIP + 3 },
      { from: 3 * BAR, take: COMP_CLIP + 2 },
    ];
    vocalTrackOf(s).clips = [clip];
    s.arrange = { ...arrangeOf(COMP_CLIP), pianoRollClip: null };
    s.transport.position = 4 * BAR;
    s.song.undoCount = 23;
  },
  meters: QUIET,
};

// Round 12, A9: 「ノーマライズ」 of the right clip, which is too quiet to reach −1.0 dB (+12.0 dB, as far as it goes).
SCENES.audioNormalized = {
  setup(s) {
    audioEditSong(s);
    vocalTrackOf(s).clips[1]!.audio!.gainDb = 12;
    s.song.undoCount = 23;
  },
  bands: ["params"],
  after: (s) => (s.status.message = { id: "normalizeLimited", count: 1 }),
  meters: QUIET,
};

// Round 12, A7: 「このトラックを音にする」 of 「1 ピアノ」 while it runs (「音にしています… 40 %」 with 「やめる」), and its step: 「ピアノ（音）」
// right below with the audio, the piano muted.
SCENES.bouncing = {
  setup(s) {
    SCENES.muted!.setup(s);
    const first = s.song.tracks[0]!;
    s.audioWork = { ...s.audioWork, bounce: { track: { number: first.number, name: first.name }, percent: 40 } };
  },
  meters: QUIET,
};
SCENES.bounced = {
  setup(s) {
    SCENES.muted!.setup(s);
    const changes = reduceA7(s, { type: "bounceTrack", track: s.song.tracks[0]!.id }, FAKE_AUDIO, () => {});
    for (const c of changes ?? []) if (c.section === "song") s.song = c.value as SongView;
  },
  after(s) {
    const made = s.song.tracks[1]!;
    s.status.message = { id: "bounced", track: { number: made.number, name: made.name } };
  },
  meters: QUIET,
};

// ---- Protocol 25: following the song's tempo, the tempo of the audio, the pitch (mock F7-Stretch; spec 3, 28-A and
// Y22). The screen's parts come later; these are the states they will show. ----

/** 「6 歌」 with one clip over bars 5 and 6, selected, the panel 「選んだクリップ」 open. */
function stretchSong(s: AppState, name: string, audio: AudioClipView): void {
  vocalSong(s, { armed: true, monitor: false, selected: true });
  vocalTrackOf(s).clips = [fakeVocalClip(LEFT_CLIP, name, 4 * BAR, 6 * BAR, audio)];
  s.arrange = { ...arrangeOf(LEFT_CLIP), pianoRollClip: null };
  s.params.selected = { kind: "clip", track: VOCAL_TRACK_ID, clip: LEFT_CLIP, missing: null, tuneUsable: true };
  s.transport.position = 4 * BAR;
  s.song.undoCount = 14;
}
/** Sung at 76.80, in a song at 96.00: "×1.25". Two semitones up: "+2". */
const STRETCHED = () => fakeAudioClip(TAKE_1, { fadeIn: TPQ / 4, ...fakeStretch(steadyTempo(76.8), FAKE_SONG_BPM, { semitones: 2 }) });

// A stretched clip whose audio is made: the marks "×1.25" and "+2".
SCENES.stretch = { setup: (s) => stretchSong(s, "歌 テイク 1", STRETCHED()), bands: ["params"], meters: QUIET };
// F7-Stretch: the same while its audio is being made: 「音を作っています 34 %」 under the clip, the bar on top of the
// panel, and the status line (which the screen makes from `audioWork`).
SCENES.stretchRendering = {
  setup(s) {
    stretchSong(s, "歌 テイク 1", STRETCHED());
    s.audioWork = { jobs: [{ kind: "rendering", audio: TAKE_1.id, clip: LEFT_CLIP, percent: 34 }], bounce: null };
  },
  bands: ["params"],
  meters: QUIET,
};
// Only the pitch: recorded in this song, a semitone down and 30 cents up, the voice not kept (「−1 ・ 30」, no "×").
SCENES.stretchPitch = {
  setup: (s) => stretchSong(s, "歌 テイク 1", fakeAudioClip(TAKE_1, fakeStretch(steadyTempo(FAKE_SONG_BPM), FAKE_SONG_BPM, { semitones: -1, cents: 30, keepFormant: false }))),
  bands: ["params"],
  meters: QUIET,
};
// Round 12, M7 「音程を寄せる」: tuned by 60 % toward A minor, the scale the clip took, while the song is in C major
// now (「いまのスケールにする」 can be pressed).
SCENES.stretchTune = {
  setup(s) {
    const audio = fakeAudioClip(TAKE_1, fakeStretch(steadyTempo(FAKE_SONG_BPM), FAKE_SONG_BPM));
    audio.stretch.tune = { root: 9, mode: "minor", strength: 60 };
    stretchSong(s, "歌 テイク 1", audio);
  },
  bands: ["params"],
  meters: QUIET,
};
// The same clip where the rows cannot be used (`tuneUsable` false): both shown disabled, at strength 0 with the song's scale.
SCENES.stretchTuneUnusable = {
  setup(s) {
    stretchSong(s, "歌 テイク 1", fakeAudioClip(TAKE_1, fakeStretch(steadyTempo(FAKE_SONG_BPM), FAKE_SONG_BPM)));
    s.params.selected = { kind: "clip", track: VOCAL_TRACK_ID, clip: LEFT_CLIP, missing: null, tuneUsable: false };
  },
  bands: ["params"],
  meters: QUIET,
};
// A file brought in: it does not follow, 「この音のテンポ」 is 「—」 with 「さがす」.
SCENES.stretchNoTempo = {
  setup: (s) => stretchSong(s, "ドラム", fakeAudioClip(DRUMS_WAV, fakeStretch(null, FAKE_SONG_BPM))),
  bands: ["params"],
  meters: QUIET,
};
// Following through a ramp: 96.00, from bar 5 (104.00) to bar 9 (120.00) in a ramp. The first clip was recorded in
// this very map from bar 3 (its tempo points are the song's, so nothing is stretched); the second was sung at 96.00
// and starts at bar 6, where the song is at 108.00 ("×1.13" there, more further on).
SCENES.stretchRamp = {
  setup(s) {
    stretchSong(s, "歌 テイク 1", fakeAudioClip(TAKE_1));
    s.timeline = {
      ...s.timeline,
      tempoPoints: [
        { tick: 4 * BAR, tempoBpm: 104, ramp: true },
        { tick: 8 * BAR, tempoBpm: 120, ramp: false },
      ],
      open: true,
    };
    const recorded: SourceTempoView = {
      tempoBpm: FAKE_SONG_BPM,
      headRamp: false,
      tempoPoints: [
        { tick: 2 * BAR, tempoBpm: 104, ramp: true },
        { tick: 6 * BAR, tempoBpm: 120, ramp: false },
      ],
    };
    vocalTrackOf(s).clips = [
      fakeVocalClip(LEFT_CLIP, "歌 テイク 1", 2 * BAR, 5 * BAR, fakeAudioClip(TAKE_1, fakeStretch(recorded, FAKE_SONG_BPM))),
      fakeVocalClip(RIGHT_CLIP, "歌 テイク 2", 5 * BAR, 9 * BAR, fakeAudioClip(HARMONY, fakeStretch(steadyTempo(FAKE_SONG_BPM), 108))),
    ];
    s.arrange = { ...arrangeOf(RIGHT_CLIP), pianoRollClip: null };
    s.params.selected = { kind: "clip", track: VOCAL_TRACK_ID, clip: RIGHT_CLIP, missing: null, tuneUsable: true };
  },
  bands: ["params"],
  meters: QUIET,
};

// ---- round 7, step 5: writing the song out (protocol 26) ----
// The states of the mocks F7-Export and F7-Exporting. The numbers of a check or of a run
// move only when a scene sets them.

/**
 * The dialog of F7-Export for the song of the band: the check is done, nothing clips, no plugin has stopped.
 * 「音の大きさ」 (round 12 F1) follows the check and the range as Rust has it; `over` gives only the choice. So does
 * 「入れるトラック」 of MIDI (protocol 36): what it takes follows the song and `midiTracks`, `midiNumbers`.
 */
function fakeExportDialog(song: SongView, over: Partial<ExportDialogView> = {}): ExportDialogView {
  const end = Math.max(0, ...song.tracks.flatMap((t) => t.clips.map((c) => c.end)));
  const d: ExportDialogView = {
    range: "song",
    loopRange: song.loopRange,
    tracks: song.tracks.length,
    span: { start: 0, end },
    format: "wav",
    // pass 2: "midi" is listed too (Rust lists it last), once ExportDialog.svelte has a name for it.
    formats: EXPORT_FORMATS.map((f) => f.format).filter((f) => f !== "midi"),
    place: `~/音楽/${song.name}.wav`,
    check: { state: "done", peakDb: -0.6, clips: false, over: 0, lufs: -12.5, truePeakDb: -0.5 },
    solo: fakeSolo({ song }),
    stoppedPlugins: [],
    advancedOpen: false,
    sampleRate: 48000,
    sampleRateChoices: [44100, 48000, 88200, 96000],
    bits: "bits24",
    bitsChoices: [...EXPORT_FORMATS[0]!.bits],
    mp3Kbps: EXPORT_MP3_DEFAULT_KBPS,
    mp3KbpsChoices: [...EXPORT_MP3_KBPS],
    title: song.name,
    artist: "",
    picture: null,
    pictureUsable: false,
    tail: "untilSilent",
    realtime: false,
    realtimeForced: false,
    canExport: true,
    loudness: "asIs",
    canMatch: false,
    loudnessResult: null,
    midiTracks: "all",
    midiNumbers: "",
    midiSelectedUsable: false,
    midiTaken: [],
    midiVocal: [],
    midiProblem: null,
    ...over,
  };
  // Round 12 F2: a wav holds no picture, nor does a MIDI file.
  return { ...d, pictureUsable: holdsPicture(d.format), ...fakeLoudnessOf(d.check, d.range, d.loudness), ...fakeMidiOf(song, d) };
}

const holdsPicture = (format: ExportFormat): boolean => format === "flac" || format === "mp3";
const extensionOf = (format: ExportFormat): string => (format === "midi" ? "mid" : format);

/** What follows the song and 「入れるトラック」; under MIDI also `tracks` (what 「トラックごと」 writes) and `canExport`. */
function fakeMidiOf(song: SongView, d: ExportDialogView): Partial<ExportDialogView> {
  const taken = fakeMidiTaken(song, d.midiTracks, d.midiNumbers);
  return d.format === "midi" ? { ...taken, tracks: taken.midiTaken.length, canExport: taken.midiProblem === null } : taken;
}

const exportDialogScene = (over: Partial<ExportDialogView> = {}): SceneDef => ({
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    s.song.name = "はじめての曲";
    s.export.dialog = fakeExportDialog(s.song, over);
  },
  after: (s) => (s.status.message = { id: "exportDialogOpen" }),
  meters: QUIET,
});

// F7-Export: 「いちばん大きい所 −0.6 dB ・ 音割れなし」, 「止まっているプラグインはありません」.
SCENES.exportDialog = exportDialogScene();
// The dialog just opened: nothing of the range is written yet.
SCENES.exportWaiting = exportDialogScene({ check: { state: "waiting" }, canExport: false });
// The check is on its way: the numbers of what is written so far.
SCENES.exportCounting = exportDialogScene({ check: { state: "counting", percent: 38, peakDb: -3.5, clips: false, over: 0, lufs: null, truePeakDb: null } });
// Spec 6 「書き出しで音が割れる」: the button is 「このまま書き出す」.
SCENES.exportClips = exportDialogScene({ check: { state: "done", peakDb: 1.4, clips: true, over: 1, lufs: null, truePeakDb: null } });
// Mock F-Export: 「Brick EQ ・ 3 ドラム」 has stopped and is passed by.
SCENES.exportStopped = exportDialogScene({
  stoppedPlugins: [{ name: "Brick EQ", slot: { kind: "insert", insert: { strip: { kind: "track", id: BAND_TRACK_ID + 3 }, slot: 0 } } }],
});
// 「くわしく」 unfolded, the loop as a flac file.
SCENES.exportAdvanced = exportDialogScene({
  range: "loop",
  span: { start: 4 * BAR, end: 8 * BAR },
  format: "flac",
  place: "~/音楽/はじめての曲.flac",
  bitsChoices: [...EXPORT_FORMATS[1]!.bits],
  advancedOpen: true,
});
// The check could not run: a plugin did not load for the export.
SCENES.exportCheckFailed = exportDialogScene({ check: { state: "failed", reason: { kind: "plugin", name: "Surge XT" } }, canExport: false });
// Round 12 F1 「音の大きさ」: 「公開先に合わせる」 makes a quiet song 4.6 dB louder.
SCENES.exportLoudness = exportDialogScene({ check: { state: "done", peakDb: -6.8, clips: false, over: 0, lufs: -18.6, truePeakDb: -6.0 }, loudness: "match" });
// The true peak stops it: 「−1 dBTP を越えないよう、−15.2 LUFS になります」.
SCENES.exportLoudnessLimited = exportDialogScene({ check: { state: "done", peakDb: -3.4, clips: false, over: 0, lufs: -17.4, truePeakDb: -3.2 }, loudness: "match" });
// A song that goes over 0 dB, matched: 4.4 dB quieter, and nothing clips in the file.
SCENES.exportLoudnessQuieter = exportDialogScene({ check: { state: "done", peakDb: 0.5, clips: false, over: 2, lufs: -9.6, truePeakDb: 0.8 }, loudness: "match" });
// Still counting: the loudness so far; what matching does is said once the check is done.
SCENES.exportLoudnessCounting = exportDialogScene({ check: { state: "counting", percent: 38, peakDb: -3.5, clips: false, over: 0, lufs: -19.0, truePeakDb: -3.1 }, loudness: "match" });
// A song near silence: 「公開先に合わせる」 cannot be chosen.
SCENES.exportLoudnessSilent = exportDialogScene({ check: { state: "done", peakDb: null, clips: false, over: 0, lufs: null, truePeakDb: null } });
// Round 12 F2 「曲の情報」 in 「くわしく」: a flac with its title, artist and picture.
const songInfoFlac: Partial<ExportDialogView> = {
  format: "flac",
  place: "~/音楽/はじめての曲.flac",
  bitsChoices: [...EXPORT_FORMATS[1]!.bits],
  advancedOpen: true,
  artist: "わたし",
  picture: "表紙.png",
};
SCENES.exportSongInfo = exportDialogScene(songInfoFlac);
// A wav holds no picture: the row is there and cannot be used.
SCENES.exportSongInfoWav = exportDialogScene({ advancedOpen: true, artist: "わたし" });
// The picture chosen is not a png or a jpg: it is not taken, and the status line says so.
SCENES.exportPictureUnreadable = {
  ...exportDialogScene({ ...songInfoFlac, picture: null }),
  after: (s) => (s.status.message = { id: "exportPictureUnreadable", file: "表紙.bmp" }),
};
// mp3: 「ビットレート」 in place of 「ビット」, at 44100 or 48000 Hz.
SCENES.exportMp3 = exportDialogScene({
  format: "mp3",
  place: "~/音楽/はじめての曲.mp3",
  bitsChoices: [],
  sampleRateChoices: [44100, 48000],
  advancedOpen: true,
  artist: "わたし",
  picture: "表紙.png",
});

// ---- MIDI export (protocol 36; spec 3 "MIDI export", spec 7 A-31) ----

/** The band's 「1 ピアノ」 to 「3 ドラム」, the vocal track 「4 歌」, then 「5 パッド」; 「1 ピアノ」 selected. */
function midiSong(s: AppState): void {
  bandSong(s, BAND_TRACKS.slice(0, 4));
  const pad = s.song.tracks[3]!;
  s.song.tracks.splice(3, 0, {
    ...pad,
    id: VOCAL_TRACK_ID,
    name: "歌",
    color: 1,
    kind: { kind: "vocal", input: "mono1", inputReady: true, armed: false, monitor: false },
    instrument: "sine",
    sound: builtinSound("sine"),
    gainDb: 0,
    sends: [null, null],
    sendDefaults: [null, null],
    clips: [],
  });
  s.song.tracks.forEach((t, i) => (t.number = i + 1));
  s.song.name = "はじめての曲";
}
const midiDialogScene = (over: Partial<ExportDialogView>): SceneDef => ({
  setup(s) {
    midiSong(s);
    s.export.dialog = fakeExportDialog(s.song, { format: "midi", place: "~/音楽/はじめての曲.mid", bitsChoices: [], ...over });
  },
  after: (s) => (s.status.message = { id: "exportDialogOpen" }),
  meters: QUIET,
});
// 「ぜんぶ」: 「1 ピアノ ・ 2 ベース ほか 2 本」; 「4 歌」 is passed by without a word.
SCENES.exportMidi = midiDialogScene({});
// 「番号で」 「1-2, 4」: two tracks, and 「4 歌 は声のトラックなので入りません」 in gray.
SCENES.exportMidiNumbers = midiDialogScene({ midiTracks: "numbers", midiNumbers: "1-2, 4" });
// 「番号で」 「1-3, 9」: 「9 番のトラックはありません」 in color.rec, and 「書き出す」 is disabled.
SCENES.exportMidiProblem = midiDialogScene({ midiTracks: "numbers", midiNumbers: "1-3, 9" });

/** The song of F7-Exporting: the band, "5 Surge XT" and 「6 歌」 with one clip; saved, the keys on 「1 ピアノ」. */
function exportingSong(s: AppState, run: ExportRunView): void {
  vocalSong(s, { armed: false, monitor: false, selected: false });
  const surge = s.song.tracks[4]!;
  Object.assign(surge, { name: "Surge XT", instrument: "sine", sound: { kind: "plugin", ...pluginView("Surge XT", "ready") } satisfies SoundView });
  vocalTrackOf(s).clips = [fakeVocalClip(LEFT_CLIP, "歌 テイク 1", 4 * BAR, 6 * BAR, fakeAudioClip(TAKE_1))];
  s.song.name = "はじめての曲";
  s.song.file = "saved";
  s.export.run = run;
}
const exportingScene = (run: ExportRunView): SceneDef => ({
  setup: (s) => exportingSong(s, run),
  after: (s) => (s.status.message = null),
  meters: QUIET,
});
const EXPORTED_FILE = "はじめての曲.wav";

// F7-Exporting: 「書き出しています 46 % ・ はじめての曲.wav ・ 残り 約 40 秒 ・ 弾けます（書き出しの音は聞こえません）」.
SCENES.exporting = exportingScene({ state: "running", percent: 46, file: EXPORTED_FILE, remainingS: 40, realtime: false, realtimeForced: false, lengthS: 182 });
// Spec 6 「実時間で書き出している」: 「速く書き出せないプラグインがあるので、実時間で書き出します（3:02）。」
SCENES.exportingRealtime = exportingScene({ state: "running", percent: 12, file: EXPORTED_FILE, remainingS: 160, realtime: true, realtimeForced: true, lengthS: 182 });
/** A made-up song for 「書き出しました」 (round 12 E9): a quiet start, two louder parts, and the tail dying away. */
export function fakeExportWave(): number[] {
  return Array.from({ length: EXPORT_WAVE_BARS }, (_, i) => {
    const x = i / EXPORT_WAVE_BARS;
    const shape = x < 0.1 ? 0.25 + x * 3 : x < 0.45 ? 0.62 : x < 0.55 ? 0.45 : x < 0.9 ? 0.85 : 0.85 * (1 - (x - 0.9) * 9);
    const beat = 0.85 + 0.15 * Math.abs(Math.sin(i * 1.7));
    return Math.max(0, Math.min(255, Math.round(255 * shape * beat)));
  });
}
// Spec 6 「書き出した」 and 「書き出しが終わった」, with 「フォルダをひらく」; and the dialog 「書き出しました」 (round 12 E9).
SCENES.exportDone = exportingScene({
  state: "done",
  file: EXPORTED_FILE,
  files: 1,
  place: `~/ミュージック/はじめての曲/${EXPORTED_FILE}`,
  lengthS: 182,
  sampleRate: 48000,
  mp3Kbps: null,
  bits: "bits24",
  wave: fakeExportWave(),
  lufs: -14.0,
  truePeakDb: -1.0,
  midiTracks: null,
});
// 「トラックごと」: 「5 つのファイル」, no waveform, no loudness, 「PC のプレーヤーで聞く」 off.
SCENES.exportDoneTracks = exportingScene({
  state: "done",
  file: "ピアノ.wav",
  files: 5,
  place: "~/ミュージック/はじめての曲/",
  lengthS: 182,
  sampleRate: 48000,
  mp3Kbps: null,
  bits: "bits24",
  wave: [],
  lufs: null,
  truePeakDb: null,
  midiTracks: null,
});
// An mp3 (round 12 F2): 「3 分 2 秒 ・ 48000 Hz ・ 192 kbps」.
SCENES.exportDoneMp3 = exportingScene({
  state: "done",
  file: "はじめての曲.mp3",
  files: 1,
  place: "~/ミュージック/はじめての曲/はじめての曲.mp3",
  lengthS: 182,
  sampleRate: 48000,
  mp3Kbps: 192,
  bits: "bits24",
  wave: fakeExportWave(),
  lufs: -14.0,
  truePeakDb: -1.0,
  midiTracks: null,
});
// MIDI (protocol 36): 「はじめての曲.mid を書き出しました（MIDI ・ 3 トラック）。」 in the status line; no dialog 「書き出しました」.
SCENES.exportDoneMidi = exportingScene({
  state: "done",
  file: "はじめての曲.mid",
  files: 1,
  place: "~/ミュージック/はじめての曲/はじめての曲.mid",
  lengthS: 182,
  sampleRate: 48000,
  mp3Kbps: null,
  bits: "bits24",
  wave: [],
  lufs: null,
  truePeakDb: null,
  midiTracks: 3,
});
// MIDI 「トラックごと」: 「3 つのファイルを書き出しました（MIDI）。」
SCENES.exportDoneMidiTracks = exportingScene({
  state: "done",
  file: "ピアノ.mid",
  files: 3,
  place: "~/ミュージック/はじめての曲/",
  lengthS: 182,
  sampleRate: 48000,
  mp3Kbps: null,
  bits: "bits24",
  wave: [],
  lufs: null,
  truePeakDb: null,
  midiTracks: 3,
});
// Spec 6 「書き出しをやめた」.
SCENES.exportCancelled = exportingScene({ state: "cancelled" });
SCENES.exportFailed = exportingScene({ state: "failed", reason: { kind: "cannotWrite" } });

// ---- round 8 (protocol 27): the sound pack, several vocal clips, the check and the solo of the export ----
// The states of the mocks F8-PackAsk, F8-PackGet, F8-PackFail, F8-PackSettings, F8-MultiClip, F8-ExportCheck and
// F8-ExportClip. A scene's fetch stands still; 「取りにいく」 starts one that moves.

const MB = 1_000_000;
const packScene = (setup: (s: AppState) => void, message: StatusMessage | null, bands?: Band[]): SceneDef => ({
  setup(s) {
    ready(s);
    setup(s);
  },
  after: (s) => (s.status.message = message),
  bands,
  meters: QUIET,
});

// F8-PackAsk: the first start; the band asks, 「1 ピアノ」 sounds as the sine wave.
SCENES.packAsk = packScene(packAsking, { id: "packAsking" });
// F8-PackGet: the grand piano and the studio kit sound; the electric piano is coming. The shelf shows the rest as 「まだ」.
SCENES.packGet = packScene(
  (s) => packFetching(s, ["grandPiano", "studioKit", "fingerBass", "strings"], "electricPiano", 92 * MB, 120),
  { id: "soundReady", instrument: "grandPiano" },
  ["shelf"],
);
// F8-PackFail and the other three failures of spec 6, with the grand piano there.
const failedScene = (failure: PackFailure) =>
  packScene((s) => packFailed(s, ["grandPiano"], failure, 92 * MB), { id: "packFailed", failure, doneBytes: 92 * MB, totalBytes: 240 * MB });
SCENES.packFailOffline = failedScene({ kind: "offline" });
SCENES.packFailCutOff = failedScene({ kind: "cutOff" });
SCENES.packFailDiskFull = failedScene({ kind: "diskFull", neededBytes: 160 * MB, freeBytes: 10 * MB });
SCENES.packFailCorrupt = failedScene({ kind: "corrupt" });
// F8-PackSettings: 「音の素材」 in the advanced sound settings, 「まだ 12 / 31」.
SCENES.packSettings = {
  setup(s) {
    settingsAdv7(s);
    withPack(s, ["grandPiano", "studioKit", "fingerBass", "strings", "electricPiano", "organ", "organDrawbar"]);
  },
  meters: QUIET,
};
// Spec 6 「音の素材 ・ 新しい包み」: said once at a start.
SCENES.packNew = packScene((s) => (s.sounds.newPackBytes = 12 * MB), { id: "newPack", bytes: 12 * MB });

// ---- round 10 (protocol 29): the row 「AI とつなぐ」 of the advanced sound settings (P10-AiLink, option B) ----
const aiLinkScene = (aiLink: AppState["aiLink"]): SceneDef => ({
  setup(s) {
    settingsAdv7(s);
    s.aiLink = aiLink;
  },
  meters: QUIET,
  scrollToEnd: true,
});
SCENES.aiLinkOff = aiLinkScene({ on: false, port: "closed" });
SCENES.aiLinkWaiting = aiLinkScene({ on: true, port: "waiting" });
SCENES.aiLinkConnected = aiLinkScene({ on: true, port: "connected" });

// ---- round 12 (D1): 「新しい版をたしかめる」 ----
const NEW_VERSION = "0.4.0";
// The look found a newer version: the status line says so while stopped (「meloet について」 has the same line: the shot aboutNewVersion).
SCENES.newVersion = {
  setup(s) {
    bandSong(s);
    s.updates = { check: true, newVersion: NEW_VERSION };
  },
  after: (s) => (s.status.message = { id: "newVersion", version: NEW_VERSION }),
  meters: QUIET,
};
// The row 「新しい版をたしかめる」 of the advanced sound settings, turned off.
SCENES.checkUpdatesOff = {
  setup(s) {
    settingsAdv7(s);
    s.updates = { check: false, newVersion: null };
  },
  meters: QUIET,
  scrollToEnd: true,
};
// ---- round 12 R3: the metronome on, −12 dB, 「ピッ」, 「録音のときだけ」 (the mock of the spec). The window opens from the ▾ (shoot_ui.py metronomeWindow).
SCENES.metronome = {
  setup(s) {
    ready(s);
    Object.assign(s.transport, { metronome: true, clickVolumeDb: -12, clickSound: "beep", clickWhen: "recordingOnly" });
  },
  meters: QUIET,
};
// ---- round 12 E2: the arpeggio of 「1 ピアノ」 (the track the keyboards play) on: 「アルペジオ」 lit in the input meter and in the
// heading of its strip. arpSettings: other values, for the ▾ window (shoot_ui.py arpWindow opens it).
const arpScene = (arp: Partial<Arp>, bands: Band[]): SceneDef => ({
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 5));
    s.song.tracks[0]!.arp = { ...FAKE_ARP_OFF, on: true, ...arp };
  },
  bands,
  meters: { ...QUIET, keys: [48, 52, 55], velocity: 96 },
});
SCENES.arpOn = arpScene({}, ["mixer"]);
SCENES.arpSettings = arpScene({ shape: "upDown", rate: "eighth", triplet: true, octaves: 2, hold: true }, ["mixer"]);
// ---- round 12 M2: 「音が大きすぎます」 while the band plays. With the master's limiter holding the sound down (the mock),
// and with the limiter passed by, the master going over 0 dB.
const tooLoudScene = (limiter: boolean): SceneDef => ({
  setup(s) {
    bandSong(s);
    s.transport.state = "playing";
    const held = s.song.master.slots[0];
    if (!limiter && held) held.bypassed = true;
  },
  after: (s) =>
    (s.status.message = limiter ? { id: "tooLoud", byDb: 5.6, lowerDb: 6, limiter: true } : { id: "tooLoud", byDb: 2.1, lowerDb: 3, limiter: false }),
  meters: {
    ...QUIET,
    master: limiter ? [dbfs(-0.3), dbfs(-0.5)] : [dbfs(2.1), dbfs(1.4)],
    masterReductionDb: limiter ? 5.6 : 0,
    position: playingAt("9.1.000"),
  },
});
SCENES.tooLoudLimiter = tooLoudScene(true);
SCENES.tooLoudOver = tooLoudScene(false);
// ---- round 12 E7: the bars of the master's sound left of 「マスター」 while the band plays (the mock of the spec), and with
// 「表示 ▸ 音の動きを見せる」 off. The bars are the scene's master as a song would sound (`fakeSpectrum`).
const spectrumScene = (on: boolean, at = "9.1.000"): SceneDef => ({
  setup(s) {
    bandSong(s);
    s.transport.state = "playing";
    s.workspace.spectrum = on;
  },
  meters: { ...QUIET, loadPermille: 140, master: [dbfs(-6.2), dbfs(-7)], position: playingAt(at) },
});
SCENES.spectrum = spectrumScene(true);
SCENES.spectrumOff = spectrumScene(false);
// The same at bars 10 and 100, where the position readout is one and two characters longer.
SCENES.spectrumBar10 = spectrumScene(true, "10.1.000");
SCENES.spectrumBar100 = spectrumScene(true, "100.1.000");
// ---- round 12 M3: 「仕上げのメーター」 right of the master strip while the band plays (the numbers of the spec's picture); with
// 「モノで聞く」 on; and stopped with nothing heard yet (「−∞」).
const finishScene = (how: "playing" | "mono" | "silent"): SceneDef => ({
  setup(s) {
    mixerSong(s, { strip: RETURN_A, slot: 0 }, "reverb", null);
    s.params.selected = null;
    s.transport.state = how === "silent" ? "stopped" : "playing";
    s.workspace.monoListen = how === "mono";
  },
  bands: ["mixer"],
  after: (s) => {
    if (how === "mono") s.status.message = { id: "monoListening" };
  },
  meters:
    how === "silent"
      ? QUIET
      : how === "mono"
        ? { ...MIXER_METERS, master: [dbfs(-3.2), dbfs(-3.2)], lufsShort: -14.6, lufsIntegrated: -15.8, truePeakDb: -1.4, position: playingAt("9.2.000") }
        : { ...MIXER_METERS, lufsShort: -13.2, lufsIntegrated: -15.8, truePeakDb: -1.4, position: playingAt("9.2.000") },
});
SCENES.finishMeters = finishScene("playing");
SCENES.finishMetersMono = finishScene("mono");
SCENES.finishMetersSilent = finishScene("silent");

// F8-MultiClip: the two clips of 「6 歌」 and one of 「7 歌 2」 selected; their volumes differ.
SCENES.multiClip = {
  setup(s) {
    audioEditSong(s);
    const sung = vocalTrackOf(s);
    s.song.tracks.push({
      ...sung,
      id: VOCAL_TRACK_ID + 1,
      number: 7,
      name: "歌 2",
      color: 2,
      clips: [fakeVocalClip(LEFT_CLIP + 10, "ハモり", 6 * BAR, 8 * BAR, fakeAudioClip(HARMONY))],
    });
    s.arrange = { ...arrangeOf(null), selectedClips: [LEFT_CLIP, RIGHT_CLIP, LEFT_CLIP + 10] };
    s.params = { selected: null, clips: fakeClipsPanel(s) };
    s.song.undoCount = 30;
    s.song.file = "changed";
  },
  after: (s) => (s.status.message = null),
  bands: ["params"],
  meters: QUIET,
};

/** The dialog of F8-ExportCheck and F8-ExportClip; in F8-ExportCheck 「2 ベース」 is soloed. */
const checkScene = (check: ExportDialogView["check"], soloed: boolean): SceneDef => ({
  setup(s) {
    bandSong(s, BAND_TRACKS.slice(0, 4));
    s.song.name = "はじめての曲";
    s.song.tracks[1]!.soloed = soloed;
    s.export.dialog = fakeExportDialog(s.song, { check, place: `~/ミュージック/${s.song.name}.wav` });
  },
  after: (s) => (s.status.message = { id: "exportDialogOpen" }),
  meters: QUIET,
});
// F8-ExportCheck: counting, 「いま 2 ベース をソロにしています。…」
SCENES.exportCheck = checkScene({ state: "counting", percent: 38, peakDb: -3.5, clips: false, over: 0, lufs: null, truePeakDb: null }, true);
// F8-ExportClip: 「0 dB をこえる所が 3 か所あります（いちばん大きい所 +1.2 dB）。音が割れます。」
SCENES.exportClip = checkScene({ state: "done", peakDb: 1.2, clips: true, over: 3, lufs: null, truePeakDb: null }, false);

/**
 * Protocol 26, enough for pictures and tests: the dialog opens, takes its choices and closes; 「書き出す」 starts a run
 * that stays at the percent of the check; 「やめる」 ends it. The fake's check is done at once, and 「えらぶ…」 and
 * 「フォルダをひらく」 are taken and do nothing. `null`: not one of these commands.
 */
export function reduceExport(state: AppState, command: Command): Change[] | null {
  const view = structuredClone(state.export);
  const busy = state.transport.state === "recording" || state.transport.state === "countIn";
  let message: StatusMessage | null | undefined = undefined;
  switch (command.type) {
    case "openExport":
      if (view.dialog || view.run?.state === "running" || busy) return [];
      view.dialog = fakeExportDialog(state.song, { canExport: state.song.tracks.some((t) => t.clips.length > 0) });
      view.run = null;
      message = { id: "exportDialogOpen" };
      break;
    case "closeExport":
      // With no dialog, an ended run goes away (the dialog 「書き出しました」 closes), as any command makes it in core.
      if (!view.dialog) {
        if (!view.run || view.run.state === "running") return [];
        view.run = null;
        break;
      }
      view.dialog = null;
      message = null;
      break;
    case "setExport": {
      const d = view.dialog;
      const c = command.change;
      if (!d) return [];
      if (c.kind === "range") {
        if ((c.range === "loop" && !d.loopRange) || (c.range === "tracks" && d.tracks === 0)) return [];
        d.range = c.range;
        d.span = c.range === "loop" ? d.loopRange! : fakeExportDialog(state.song).span;
        // Round 12 F1: one file for each track has no loudness, and is not matched.
        const base = fakeExportDialog(state.song).check;
        const measured = c.range !== "tracks" && base.state === "done" ? base : { lufs: null, truePeakDb: null };
        if (d.check.state === "done" || d.check.state === "counting") d.check = { ...d.check, lufs: measured.lufs, truePeakDb: measured.truePeakDb };
        Object.assign(d, fakeLoudnessOf(d.check, d.range, d.loudness));
      } else if (c.kind === "format") {
        // Back from MIDI: 「トラックごと」 counts every track again, and 「書き出す」 waits for the check.
        if (d.format === "midi" && c.format !== "midi") {
          d.tracks = state.song.tracks.length;
          d.canExport = (d.check.state === "done" || d.check.state === "counting") && state.song.tracks.some((t) => t.clips.length > 0);
        }
        d.format = c.format;
        d.bitsChoices = [...EXPORT_FORMATS.find((f) => f.format === c.format)!.bits];
        // Round 12 F2: mp3 (and MIDI) have no bits (the ones chosen stay); mp3 is written at 44100 or 48000 Hz
        // (EXPORT_MP3_SAMPLE_RATES of Rust).
        if (d.bitsChoices.length > 0 && !d.bitsChoices.includes(d.bits)) d.bits = "bits24";
        d.sampleRateChoices = c.format === "mp3" ? [44100, 48000] : [44100, 48000, 88200, 96000];
        if (!d.sampleRateChoices.includes(d.sampleRate)) d.sampleRate = d.sampleRate % 11025 === 0 ? 44100 : 48000;
        d.pictureUsable = holdsPicture(c.format);
      } else if (c.kind === "midiTracks") {
        if (c.tracks === "selected" && !d.midiSelectedUsable) return [];
        d.midiTracks = c.tracks;
      } else if (c.kind === "midiNumbers") d.midiNumbers = c.text;
      else if (c.kind === "mp3Kbps") d.mp3Kbps = d.mp3KbpsChoices.includes(c.kbps) ? c.kbps : d.mp3Kbps;
      else if (c.kind === "title") d.title = c.title;
      else if (c.kind === "artist") d.artist = c.artist;
      else if (c.kind === "removePicture") d.picture = null;
      else if (c.kind === "advancedOpen") d.advancedOpen = c.open;
      else if (c.kind === "sampleRate") d.sampleRate = d.sampleRateChoices.includes(c.hz) ? c.hz : d.sampleRate;
      else if (c.kind === "bits") d.bits = d.bitsChoices.includes(c.bits) ? c.bits : d.bits;
      else if (c.kind === "tail") d.tail = c.tail;
      else if (c.kind === "realtime") {
        if (!d.realtimeForced) d.realtime = c.on;
      } else if (c.kind === "loudness") {
        if (c.loudness === "asIs" || d.canMatch) d.loudness = c.loudness;
      }
      const folder = d.place.slice(0, d.place.lastIndexOf("/") + 1);
      d.place = d.range === "tracks" ? folder : `${folder}${state.song.name}.${extensionOf(d.format)}`;
      Object.assign(d, fakeMidiOf(state.song, d));
      break;
    }
    case "chooseExportPlace":
    case "showExported":
    case "playExported":
      return [];
    case "startExport": {
      const d = view.dialog;
      if (!d?.canExport) return [];
      if (d.format === "midi") {
        // Writing MIDI takes no time: the run is done at once, and no dialog 「書き出しました」 is meant to open.
        const map = new TempoMap(state.timeline);
        const perTrack = d.range === "tracks";
        view.run = {
          state: "done",
          file: perTrack ? `${d.midiTaken[0]!.name}.mid` : d.place.slice(d.place.lastIndexOf("/") + 1),
          files: perTrack ? d.midiTaken.length : 1,
          place: d.place,
          lengthS: Math.round(map.secondsAt(d.span.end) - map.secondsAt(d.span.start)),
          sampleRate: d.sampleRate,
          bits: d.bits,
          wave: [],
          lufs: null,
          truePeakDb: null,
          mp3Kbps: null,
          midiTracks: d.midiTaken.length,
        };
        view.dialog = null;
        message = null;
        break;
      }
      const percent = d.check.state === "counting" ? d.check.percent : 99;
      view.run = { state: "running", percent, file: d.place.slice(d.place.lastIndexOf("/") + 1), remainingS: null, realtime: d.realtime, realtimeForced: d.realtimeForced, lengthS: 0 };
      view.dialog = null;
      message = null;
      break;
    }
    case "cancelExport":
      if (view.run?.state !== "running") return [];
      view.run = { state: "cancelled" };
      break;
    default:
      return null;
  }
  const changes: Change[] = [];
  if (JSON.stringify(view) !== JSON.stringify(state.export)) changes.push({ section: "export", value: view });
  if (message !== undefined) changes.push({ section: "status", value: { message } });
  return changes;
}

/** 「さがす」 as Rust does it, in 4/4: the audio as 1, 2, 4 or 8 bars, the tempos from 60 to 180, the one nearest to `songBpm`. */
function fakeFoundTempo(frames: number, sampleRate: number, songBpm: number): { bpm: number; bars: number } | null {
  const far = (bpm: number) => Math.abs(Math.log(bpm / songBpm));
  let best: { bpm: number; bars: number } | null = null;
  for (const bars of [1, 2, 4, 8]) {
    const bpm = Math.round(((bars * 4 * 60 * sampleRate) / frames) * 100) / 100;
    if (bpm >= 60 && bpm <= 180 && (best === null || far(bpm) < far(best.bpm))) best = { bpm, bars };
  }
  return best;
}

/**
 * Protocol 24 and 25, enough for pictures and tests: the 160 row, a value of a vocal clip, a row of its stretching
 * and pitch, 「さがす」 of the tempo of its audio, and 「曲から外す」 of an audio file. The fake's ratio is with the tempo of the song's head.
 * Cutting, bringing files in and 「さがす…」 are taken and do nothing. `null`: not one of these commands.
 */
export function reduceAudio(state: AppState, command: Command): Change[] | null {
  const vocalClip = (song: SongView, id: number) => song.tracks.flatMap((t) => t.clips).find((c) => c.id === id && c.audio !== null);
  /** The changed song as one step; nothing when it is as it was. */
  const stepped = (song: SongView): Change[] => {
    if (JSON.stringify(song) === JSON.stringify(state.song)) return [];
    Object.assign(song, { undoCount: song.undoCount + 1, redoCount: 0, file: "changed" });
    return [{ section: "song", value: song }];
  };
  /** One tempo for the clip's audio: a new take in the same place, as Rust makes one. */
  const setSourceBpm = (clip: ClipView, bpm: number) => {
    const audio = clip.audio!;
    if (audio.stretch.sourceBpm === bpm && (audio.sourceTempo?.tempoPoints.length ?? 0) === 0) return;
    Object.assign(audio, fakeStretch(steadyTempo(bpm), state.timeline.tempoBpm, audio.stretch));
    clip.takeId = Math.max(...state.song.tracks.flatMap((t) => t.clips.flatMap((c) => [c.id, c.takeId]))) + 1;
  };
  switch (command.type) {
    case "setTallTrack": {
      const track = state.song.tracks.find((t) => t.id === command.track);
      const tallTrack = track?.kind.kind === "vocal" && state.arrange.tallTrack !== track.id ? track.id : null;
      return tallTrack === state.arrange.tallTrack ? [] : [{ section: "arrange", value: { ...state.arrange, tallTrack } }];
    }
    case "setClipField": {
      const song = structuredClone(state.song);
      const clip = vocalClip(song, command.clip);
      if (!clip?.audio || Number.isNaN(command.value)) return [];
      const length = clip.end - clip.start;
      const ticks = Math.max(0, Math.round(command.value));
      if (command.field === "gainDb") clip.audio.gainDb = Math.min(12, Math.max(-60, Math.round(command.value * 100) / 100));
      else if (command.field === "fadeIn") clip.audio.fadeIn = Math.min(ticks, length - clip.audio.fadeOut);
      else if (command.field === "fadeOut") clip.audio.fadeOut = Math.min(ticks, length - clip.audio.fadeIn);
      else if (command.field === "start") Object.assign(clip, { start: ticks, end: ticks + length });
      else clip.end = clip.start + Math.max(1, ticks);
      return stepped(song);
    }
    case "setClipStretch": {
      const song = structuredClone(state.song);
      const clip = vocalClip(song, command.clip);
      if (!clip?.audio) return [];
      const { stretch } = clip.audio;
      const change = command.change;
      const whole = (value: number, most: number) => Math.min(most, Math.max(-most, Math.round(value)));
      if (change.kind === "keepFormant") stretch.keepFormant = change.on;
      else if (change.kind === "follow") {
        const source = stretch.sourceBpm === null ? null : (clip.audio.sourceTempo ?? steadyTempo(stretch.sourceBpm));
        Object.assign(clip.audio, fakeStretch(source, state.timeline.tempoBpm, { ...stretch, follow: change.on }));
      } else if (change.kind === "sourceBpm") {
        if (!Number.isNaN(change.bpm)) setSourceBpm(clip, Math.min(SOURCE_BPM_MAX, Math.max(SOURCE_BPM_MIN, Math.round(change.bpm * 100) / 100)));
      } else if (change.kind === "semitones") {
        if (!Number.isNaN(change.semitones)) stretch.semitones = whole(change.semitones, CLIP_SEMITONES_MAX);
      } else if (change.kind === "tuneStrength") {
        // As Rust: the nearest whole number inside 0..=100; leaving 0 the clip takes the song's scale, at 0 it keeps none.
        if (Number.isNaN(change.percent)) return [];
        const strength = Math.min(TUNE_STRENGTH_MAX, Math.max(0, Math.round(change.percent)));
        const { root, mode } = stretch.tune ?? state.pianoRoll.scale;
        const tune = strength === 0 ? null : { root, mode, strength };
        if (JSON.stringify(tune) === JSON.stringify(stretch.tune)) return [];
        stretch.tune = tune;
      } else if (change.kind === "tuneScale") {
        const { root, mode } = state.pianoRoll.scale;
        if (!stretch.tune || (stretch.tune.root === root && stretch.tune.mode === mode)) return [];
        stretch.tune = { root, mode, strength: stretch.tune.strength };
      } else if (!Number.isNaN(change.cents)) stretch.cents = whole(change.cents, CLIP_CENTS_MAX);
      return stepped(song);
    }
    case "findSourceTempo": {
      const song = structuredClone(state.song);
      const clip = vocalClip(song, command.clip);
      if (!clip?.audio) return [];
      const found = fakeFoundTempo(clip.audio.frames, clip.audio.sampleRate, state.timeline.tempoBpm);
      const message: StatusMessage = found ? { id: "sourceTempoFound", name: clip.name, bpm: found.bpm, bars: found.bars, same: clip.audio.stretch.sourceBpm === found.bpm } : { id: "sourceTempoNotFound", name: clip.name };
      if (found) setSourceBpm(clip, found.bpm);
      return [...stepped(song), { section: "status", value: { message } }];
    }
    case "removeMissing": {
      if (command.missing.kind !== "audio") return null;
      const gone = command.missing.audio;
      const song = structuredClone(state.song);
      for (const t of song.tracks) t.clips = t.clips.filter((c) => c.audio?.audio !== gone);
      if (JSON.stringify(song) === JSON.stringify(state.song)) return [];
      Object.assign(song, { undoCount: song.undoCount + 1, redoCount: 0, file: "changed" });
      const left = new Set(song.tracks.flatMap((t) => t.clips.map((c) => c.id)));
      const changes: Change[] = [{ section: "song", value: song }];
      const selected = state.arrange.selectedClips.filter((id) => left.has(id));
      if (selected.length !== state.arrange.selectedClips.length) {
        changes.push({ section: "arrange", value: { ...state.arrange, selectedClips: selected, selectedClip: null } });
      }
      const shown = state.params.selected;
      if (shown?.kind === "clip" && !left.has(shown.clip)) changes.push({ section: "params", value: { selected: null, clips: null } });
      return changes;
    }
    case "splitClips":
    case "importAudio":
    case "dropAudio":
    case "findAudio":
      return [];
    default:
      return null;
  }
}

/** What the fake remembers between the commands of round 7. */
export interface FakeVocalMemo {
  /** 「モニター」 was turned on once (the status line says how it sounds only the first time). */
  monitorSaid: boolean;
}

/**
 * Protocol 23, enough for pictures and tests: vocal tracks (add, the input, the ●, the monitor, selecting one), E on a
 * vocal track, the advanced sound settings and the latency dialog (measuring moves only when a scene sets it).
 * `null`: not one of these commands.
 */
export function reduceRound7(state: AppState, command: Command, memo: FakeVocalMemo): Change[] | null {
  const song = structuredClone(state.song);
  const ws = structuredClone(state.workspace);
  const audio = structuredClone(state.audio);
  const inputs = structuredClone(state.inputs);
  let message: StatusMessage | null | undefined = undefined;
  const vocal = (id: number) => {
    const t = song.tracks.find((x) => x.id === id);
    return t && t.kind.kind === "vocal" ? { track: t, kind: t.kind } : null;
  };
  const busy = state.transport.state === "recording" || state.transport.state === "countIn";
  const ref = (t: FakeTrack): TrackRef => ({ number: t.number, name: t.name });
  switch (command.type) {
    case "addVocalTrack": {
      if (busy) return [];
      if (song.tracks.length >= MAX_TRACKS) {
        message = { id: "tooManyTracks" };
        break;
      }
      const names = new Set(song.tracks.map((t) => t.name));
      let name = "歌";
      for (let n = 2; names.has(name); n++) name = `歌 ${n}`;
      const id = Math.max(0, ...song.tracks.map((t) => t.id)) + 1;
      const place = song.tracks.length;
      song.tracks.push({
        id,
        number: place + 1,
        name,
        color: (place % 5) + 1,
        kind: { kind: "vocal", input: "mono1", inputReady: inputs.chosen !== null, armed: true, monitor: false },
        sound: builtinSound("sine"),
        standIn: null,
        drumKeys: null,
        gainDb: 0,
        pan: 0,
        sends: [null, null],
        sendDefaults: [null, null],
        slots: [null, null, null, null],
        muted: false,
        soloed: false,
        lanes: [],
        clips: [],
        arp: FAKE_ARP_OFF,
        instrument: "sine",
        inserts: noInserts(),
      });
      song.playTarget = id;
      song.undoCount += 1;
      song.redoCount = 0;
      song.file = "changed";
      const added = song.tracks.at(-1)!;
      message = inputs.chosen === null ? { id: "noInput" } : { id: "vocalTrackAdded", track: ref(added), input: "mono1" };
      break;
    }
    case "setVocalInput": {
      const v = vocal(command.track);
      if (!v || busy || v.kind.input === command.input) return [];
      v.kind.input = command.input;
      song.undoCount += 1;
      song.redoCount = 0;
      song.file = "changed";
      break;
    }
    case "toggleVocalArm": {
      const v = vocal(command.track);
      if (!v || busy) return [];
      v.kind.armed = !v.kind.armed;
      break;
    }
    case "toggleMonitor": {
      const v = vocal(command.track);
      if (!v) return [];
      v.kind.monitor = !v.kind.monitor;
      if (v.kind.monitor && !memo.monitorSaid) {
        memo.monitorSaid = true;
        message = { id: "monitorFirstOn", latencyMs: audio.latency.usedMs };
      }
      break;
    }
    case "selectTrack": {
      const v = vocal(command.track);
      if (!v || busy) return null;
      song.playTarget = command.track;
      v.kind.armed = true;
      break;
    }
    case "toggleBand": {
      const selected = song.tracks.find((t) => t.id === song.playTarget);
      if (command.band !== "pianoRoll" || selected?.kind.kind !== "vocal" || state.workspace.layout.lower.some((b) => b.band === "pianoRoll")) return null;
      message = { id: "vocalHasNoPianoRoll" };
      break;
    }
    case "settingsSelectInput":
      inputs.chosen = command.input;
      break;
    case "settingsSetBuffer":
      audio.requestedFrames = command.frames;
      if (audio.forcedQuantum === null) audio.actualFrames = command.frames;
      break;
    case "settingsSetSampleRate":
      audio.requestedSampleRate = command.hz;
      break;
    case "settingsSetRecordFormat":
      audio.recordFormat = command.format;
      break;
    case "settingsSetManualLatency":
      audio.latency.manualMs = command.ms;
      break;
    case "openLatencyMeasure":
      ws.latencyDialog = { measure: { state: "idle" } };
      message = { id: "latencyMeasureOpen" };
      break;
    // The fake plays no clicks: measuring starts and stays at 0 of 5 until it is stopped.
    case "startLatencyMeasure":
      if (!ws.latencyDialog) return [];
      ws.latencyDialog.measure = { state: "measuring", done: 0, of: LATENCY_CLICKS, meanMs: null };
      break;
    case "stopLatencyMeasure":
      if (!ws.latencyDialog) return [];
      ws.latencyDialog.measure = { state: "idle" };
      break;
    case "useMeasuredLatency": {
      const m = ws.latencyDialog?.measure;
      if (m?.state !== "done") return [];
      audio.latency.measuredMs = m.measuredMs;
      ws.latencyDialog = null;
      break;
    }
    case "closeLatencyMeasure":
      ws.latencyDialog = null;
      break;
    case "back":
      if (!ws.latencyDialog) return null;
      ws.latencyDialog = null;
      break;
    default:
      return null;
  }
  const { latency } = audio;
  latency.used = latency.manualMs !== null ? "manual" : latency.measuredMs !== null ? "measured" : "reported";
  latency.usedMs = latency.manualMs ?? latency.measuredMs ?? latency.reportedMs;
  for (const t of song.tracks) if (t.kind.kind === "vocal") t.kind.inputReady = inputs.chosen !== null;
  inputs.open = inputs.chosen !== null && song.tracks.some((t) => t.kind.kind === "vocal" && (t.kind.armed || t.id === song.playTarget));
  const changes: Change[] = [];
  const settled = settleSong(song);
  if (JSON.stringify(settled) !== JSON.stringify(state.song)) changes.push({ section: "song", value: settled });
  if (JSON.stringify(ws) !== JSON.stringify(state.workspace)) changes.push({ section: "workspace", value: ws });
  if (JSON.stringify(audio) !== JSON.stringify(state.audio)) changes.push({ section: "audio", value: audio });
  if (JSON.stringify(inputs) !== JSON.stringify(state.inputs)) changes.push({ section: "inputs", value: inputs });
  if (message !== undefined) changes.push({ section: "status", value: { message } });
  return changes;
}

// ---- telemetry ----

const STATE_BYTES = { stopped: 0, countIn: 1, playing: 2, recording: 3 } as const;

/** Writes the telemetry binary (docs/architecture.md 3-3). `position` is what is heard at `sentUs`. */
export function encodeTelemetry(
  m: FakeMeters,
  seq: number,
  sentUs: number,
  trackCount: number,
  position: TransportReading | null = m.position ?? null,
): ArrayBuffer {
  const H = TELEMETRY_HEADER_LEN;
  const mics = m.mics ?? [];
  const buf = new ArrayBuffer(H + 8 * trackCount + TELEMETRY_MIC_LEN * mics.length);
  const v = new DataView(buf);
  v.setUint8(0, TELEMETRY_KIND);
  v.setUint8(1, TELEMETRY_VERSION);
  v.setUint16(2, trackCount, true);
  v.setUint32(4, seq, true);
  v.setBigUint64(8, BigInt(Math.round(sentUs)) * 1000n, true);
  const words = [0, 0, 0, 0];
  for (const n of m.keys ?? []) words[n >>> 5]! |= 1 << (n & 31);
  words.forEach((w, i) => v.setUint32(16 + 4 * i, w >>> 0, true));
  v.setUint8(32, m.velocity ?? 0);
  v.setUint8(33, m.sustain ?? 0);
  v.setUint8(34, m.modulation ?? 0);
  v.setUint8(35, (m.master && (m.master[0] > 0 || m.master[1] > 0) ? 1 : 0) | (position ? 4 : 0));
  v.setInt16(36, m.pitchBend ?? 0, true);
  v.setUint16(38, m.loadPermille ?? 0, true);
  v.setFloat32(40, m.master?.[0] ?? 0, true);
  v.setFloat32(44, m.master?.[1] ?? 0, true);
  if (position) {
    v.setFloat64(48, position.tick, true);
    v.setFloat64(56, position.ticksPerUs, true);
    v.setUint8(64, STATE_BYTES[position.state]);
    v.setUint16(66, position.countInBeatsLeft, true);
    v.setUint32(68, position.lap, true);
  }
  (m.returns ?? []).flat().forEach((peak, i) => v.setFloat32(72 + 4 * i, peak, true));
  v.setUint16(88, mics.length, true);
  const hundredths = (db: number | null | undefined) => (db === null || db === undefined ? TELEMETRY_NO_VALUE : Math.round(db * 100));
  v.setInt16(92, hundredths(m.lufsShort), true);
  v.setInt16(94, hundredths(m.lufsIntegrated), true);
  v.setInt16(96, hundredths(m.truePeakDb), true);
  v.setUint8(98, Math.round((m.masterReductionDb ?? 0) * 10));
  (m.spectrum ?? []).forEach((band, i) => v.setUint8(100 + i, band));
  mics.forEach((mic, i) => {
    const p = H + 8 * trackCount + TELEMETRY_MIC_LEN * i;
    v.setUint32(p, mic.track, true);
    v.setFloat32(p + 4, mic.peak[0], true);
    v.setFloat32(p + 8, mic.peak[1], true);
    v.setUint8(p + 12, (mic.clipped ? 1 : 0) | (mic.stereo ? 2 : 0));
  });
  for (let i = 0; i < trackCount; i++) {
    const [l, r] = m.tracks?.[i] ?? [0, 0];
    v.setFloat32(H + 8 * i, l, true);
    v.setFloat32(H + 4 + 8 * i, r, true);
  }
  return buf;
}

// ---- commands ----

/** The status messages that tell what just happened. The next thing the user does takes them away. */
const NOTICES: StatusMessage["id"][] = [
  "bandFoldedWindowLow",
  "bandFoldedToOpen",
  "bandFoldedThird",
  "bandFoldedScale",
  "valueRejected",
  "quantized",
  "scaleDoesNotFit",
  "scaleWindowGrown",
  "scaleLoweredForScreen",
  "bufferFixed",
  "valueClamped",
  "velocityMatched",
  "lengthMatched",
  "clipsNotMoved",
  "clipsNotPasted",
  "clipsNotDuplicated",
  "notesNotWritten",
  "notesNotResized",
  "patternNeedsFourFour",
  "armMoved",
  "trackAdded",
  "trackDeleted",
  "tooManyTracks",
  "nothingToKeep",
  "songTooLong",
  "fadeOutNoClips",
  "instrumentReplaced",
  "paramClamped",
  "paramRejected",
  "effectInserted",
  "effectRemoved",
  "insertsReordered",
  "insertReplaced",
  "effectBypassed",
  "pluginWindowOpen",
  "pluginWindowsOpen",
  "mixerPassesBy",
];
const isNotice = (m: StatusMessage | null) => m !== null && NOTICES.includes(m.id);

/** Sorted indexes to ranges `[start, end)`, the touching ones joined. */
export function toRanges(indexes: Iterable<number>): IndexRange[] {
  const out: IndexRange[] = [];
  for (const i of [...new Set(indexes)].sort((a, b) => a - b)) {
    const last = out.at(-1);
    if (last && last.end === i) last.end = i + 1;
    else out.push({ start: i, end: i + 1 });
  }
  return out;
}

const fromRanges = (ranges: IndexRange[]) => ranges.flatMap((r) => Array.from({ length: r.end - r.start }, (_, k) => r.start + k));

/** The selection commands (of notes and of control points), the grid, the scale and the quantize settings. The fake does not change notes or points. */
function reducePianoRoll(pr: PianoRollView, command: Command): void {
  const count = pr.take === null ? 0 : (FAKE_TAKES.get(pr.take)?.notes.length ?? 0);
  const points = pr.take === null ? 0 : (FAKE_TAKES.get(pr.take)?.controls?.length ?? 0);
  switch (command.type) {
    case "selectNotes": {
      if (command.take !== pr.take) break;
      const picked = command.notes.filter((i) => i < count);
      const had = new Set(fromRanges(pr.selectedNotes));
      if (command.mode === "replace") had.clear();
      for (const i of picked) {
        if (command.mode === "toggle" && had.has(i)) had.delete(i);
        else had.add(i);
      }
      pr.selectedNotes = toRanges(had);
      break;
    }
    case "selectControls": {
      if (command.take !== pr.take) break;
      const had = new Set(command.mode === "replace" ? [] : fromRanges(pr.selectedControls));
      for (const i of command.points.filter((p) => p < points)) {
        if (command.mode === "toggle" && had.has(i)) had.delete(i);
        else had.add(i);
      }
      pr.selectedControls = toRanges(had);
      break;
    }
    case "selectAllNotes":
      pr.selectedNotes = count > 0 ? [{ start: 0, end: count }] : [];
      break;
    case "clearNoteSelection":
      pr.selectedNotes = [];
      break;
    case "setGrid":
      pr.grid = { division: command.division, triplet: command.triplet };
      break;
    case "setMusicalScale":
      pr.scale = { root: command.root, mode: command.mode, lock: command.lock, highlight: command.highlight };
      break;
    case "setQuantize": {
      const { division, triplet, strength, swing, align } = command;
      pr.quantize = { division, triplet, strength, swing, align };
      break;
    }
    default:
      break;
  }
}

/** The status line's summary for the selection of the piano roll (NOTE_SELECTION_KEYS in meloet-protocol is 4). */
function noteSelection(pr: PianoRollView): Extract<StatusMessage, { id: "summary" }>["notes"] {
  const take = pr.take === null ? undefined : FAKE_TAKES.get(pr.take);
  const picked = fromRanges(pr.selectedNotes);
  if (!take || picked.length === 0) return null;
  const keys = picked.map((i) => take.notes[i]!.key);
  return { count: picked.length, keys: keys.slice(0, 4), quantized: (take.origin ?? 0) !== 0, names: keyCounts(keys) };
}

/** What `copyClips` and `cutClips` keep: each clip with its row and start counted from the first row and the earliest start. */
export interface FakeClipboard {
  clips: { row: number; at: number; clip: ClipView }[];
}

const CLIP_COMMANDS: Command["type"][] = [
  "selectClip",
  "selectClips",
  "selectAllClips",
  "selectTrack",
  "toggleMute",
  "toggleSolo",
  "moveClips",
  "duplicateClips",
  "trimClip",
  "deleteTake",
  "deleteOtherTakes",
  "deleteSelected",
  "copyClips",
  "cutClips",
  "pasteClips",
  "repeatClips",
];

/**
 * The clip and track commands of docs/architecture.md 17 and protocol 8, enough for pictures and tests: no history,
 * no undo count. Returns the new song and arrange, or null when nothing changes (as Rust does on an overlap).
 */
export function reduceClips(
  state: AppState,
  command: Command,
  clipboard: FakeClipboard = { clips: [] },
): { song: SongView; arrange: ArrangeView; notice?: StatusMessage } | null {
  if (!CLIP_COMMANDS.includes(command.type)) return null;
  const song = structuredClone(state.song);
  const arrange = structuredClone(state.arrange);
  const rowOf = (id: number) => song.tracks.findIndex((t) => t.clips.some((c) => c.id === id));
  const clipOf = (id: number) => song.tracks.flatMap((t) => t.clips).find((c) => c.id === id);
  const inOrder = (ids: Iterable<number>) => {
    const set = new Set(ids);
    return song.tracks.flatMap((t) => t.clips.filter((c) => set.has(c.id)).map((c) => c.id));
  };
  const select = (ids: number[], last: number | null) => {
    arrange.selectedClips = inOrder(ids);
    arrange.selectedClip = last !== null && arrange.selectedClips.includes(last) ? last : arrange.selectedClips.length === 1 ? arrange.selectedClips[0]! : null;
    if (arrange.selectedClip !== null) arrange.pianoRollClip = arrange.selectedClip;
  };
  const done = () => ({ song, arrange });
  const track = (id: number) => song.tracks.find((t) => t.id === id);
  switch (command.type) {
    case "selectClip":
      if (command.clip === null) select([], null);
      else if (clipOf(command.clip)) {
        select([command.clip], command.clip);
        song.playTarget = song.tracks[rowOf(command.clip)]!.id;
      }
      return done();
    case "selectClips": {
      const picked = command.clips.filter((id) => clipOf(id));
      const had = new Set(command.mode === "replace" ? [] : arrange.selectedClips);
      for (const id of picked) {
        if (command.mode === "toggle" && had.has(id)) had.delete(id);
        else had.add(id);
      }
      // Shift + click (one clip added) makes it the clip clicked last; a box does not.
      const clicked = command.mode !== "replace" && picked.length === 1 ? picked[0]! : null;
      select([...had], clicked ?? arrange.selectedClip);
      if (clicked !== null && had.has(clicked)) song.playTarget = song.tracks[rowOf(clicked)]!.id;
      return done();
    }
    case "selectAllClips":
      select(
        song.tracks.flatMap((t) => t.clips.map((c) => c.id)),
        arrange.selectedClip,
      );
      return done();
    case "selectTrack": {
      const t = track(command.track);
      if (!t || busy(state)) return done();
      const moved = command.fromArm && song.playTarget !== command.track;
      song.playTarget = command.track;
      return moved ? { ...done(), notice: { id: "armMoved", track: { number: t.number, name: t.name } } } : done();
    }
    case "toggleMute":
    case "toggleSolo": {
      const t = track(command.track);
      if (!t) return null;
      if (command.type === "toggleMute") t.muted = !t.muted;
      else t.soloed = !t.soloed;
      return done();
    }
    case "copyClips":
    case "cutClips": {
      const ids = inOrder(command.clips);
      if (ids.length === 0) return null;
      const rows = ids.map(rowOf);
      const top = Math.min(...rows);
      const first = Math.min(...ids.map((id) => clipOf(id)!.start));
      clipboard.clips = ids.map((id, i) => ({ row: rows[i]! - top, at: clipOf(id)!.start - first, clip: { ...clipOf(id)! } }));
      arrange.clipboardClips = ids.length;
      if (command.type === "copyClips") return done();
      if (busy(state)) return null;
      for (const t of song.tracks) t.clips = t.clips.filter((c) => !ids.includes(c.id));
      select([], null);
      return done();
    }
    default:
      break;
  }
  if (busy(state)) return null;
  const nextId = () => Math.max(0, ...FAKE_TAKES.keys(), ...song.tracks.flatMap((t) => t.clips.map((c) => c.id))) + 1;
  const byStart = (a: ClipView, b: ClipView) => a.start - b.start;
  /**
   * Puts `items` (copies when `fresh`) where they say. When one is off the tracks nothing changes; when one would
   * overlap another clip nothing changes either, and the status line says `overlap` (as core does).
   */
  const place = (
    items: { row: number; start: number; clip: ClipView }[],
    fresh: boolean,
    taken: number[],
    overlap: StatusMessage,
  ): ReturnType<typeof reduceClips> => {
    if (!items.every(({ row, start }) => row >= 0 && row < song.tracks.length && start >= 0)) return null;
    for (const t of song.tracks) t.clips = t.clips.filter((c) => !taken.includes(c.id));
    const overlaps = items.some(({ row, start, clip }) =>
      song.tracks[row]!.clips.some((c) => c.start < start + clip.end - clip.start && start < c.end),
    );
    if (overlaps) return { song: state.song, arrange: state.arrange, notice: overlap };
    const placed: number[] = [];
    let last: number | null = null;
    for (const { row, start, clip } of items) {
      const id = fresh ? nextId() : clip.id;
      song.tracks[row]!.clips.push({ ...clip, id, start, end: start + clip.end - clip.start });
      song.tracks[row]!.clips.sort(byStart);
      placed.push(id);
      if (clip.id === arrange.selectedClip || last === null) last = id;
    }
    select(placed, last);
    return done();
  };
  switch (command.type) {
    case "trimClip": {
      const from = rowOf(command.clip);
      if (from < 0) return null;
      const clips = song.tracks[from]!.clips;
      const at = clips.findIndex((c) => c.id === command.clip);
      const clip = clips[at]!;
      const before = clips.filter((c) => c.end <= clip.start).reduce((m, c) => Math.max(m, c.end), 0);
      const after = clips.filter((c) => c.start >= clip.end).reduce((m, c) => Math.min(m, c.start), Infinity);
      const start = command.edge === "start" ? Math.min(Math.max(clip.start + command.ticks, before), clip.end - 1) : clip.start;
      const end = command.edge === "end" ? Math.max(Math.min(clip.end + command.ticks, after), clip.start + 1) : clip.end;
      if (start === clip.start && end === clip.end) return null;
      const shift = start - clip.start;
      // Every take stays where it is in the song, and 「いいとこ取り」 is cut to the clip (round 12, A8).
      const audio = clip.audio && { ...clip.audio, takes: clip.audio.takes.map((x) => ({ ...x, takeOffset: x.takeOffset - shift })) };
      clips[at] = { ...clip, start, end, takeOffset: clip.takeOffset - shift, audio, comp: fakeCompWindow(clip.comp, shift, end - start) };
      return done();
    }
    case "deleteTake": {
      // The fake keeps no other take ids: the number moves, the id stays.
      const clip = clipOf(command.clip);
      if (!clip || clip.takes < 2) return null;
      clip.takes -= 1;
      clip.take = Math.max(1, clip.take - 1);
      return done();
    }
    case "deleteOtherTakes": {
      // The shown take stays, as take 1.
      const clip = clipOf(command.clip);
      if (!clip || clip.takes < 2) return null;
      const shown = clip.audio?.takes[clip.take - 1];
      if (clip.audio && shown) clip.audio.takes = [shown];
      clip.takes = 1;
      clip.take = 1;
      return done();
    }
    case "deleteSelected": {
      const ids = arrange.selectedClips;
      if (ids.length === 0) return null;
      for (const t of song.tracks) t.clips = t.clips.filter((c) => !ids.includes(c.id));
      select([], null);
      return done();
    }
    case "moveClips":
    case "duplicateClips": {
      const ids = inOrder(command.clips);
      if (ids.length === 0) return null;
      const clips = ids.map((id) => clipOf(id)!);
      const copy = command.type === "duplicateClips" || command.copy;
      const rows = command.type === "duplicateClips" ? 0 : command.tracks;
      const earliest = Math.min(...clips.map((c) => c.start));
      // The group stops at the song's start.
      const ticks = command.type === "duplicateClips" ? Math.max(...clips.map((c) => c.end)) - earliest : Math.max(command.ticks, -earliest);
      if (!copy && ticks === 0 && rows === 0) return null;
      const items = clips.map((clip) => ({ row: rowOf(clip.id) + rows, start: clip.start + ticks, clip }));
      return place(items, copy, copy ? [] : ids, { id: copy ? "clipsNotDuplicated" : "clipsNotMoved" });
    }
    case "pasteClips": {
      if (clipboard.clips.length === 0) return null;
      const perBar = ((4 * TPQ) / state.timeline.meterDen) * state.timeline.meterNum;
      const to = command.to ?? { track: song.playTarget ?? 0, tick: Math.floor(state.transport.position / perBar) * perBar };
      const top = song.tracks.findIndex((t) => t.id === to.track);
      if (top < 0) return null;
      const items = clipboard.clips.map(({ row, at, clip }) => ({ row: top + row, start: to.tick + at, clip }));
      return place(items, true, [], { id: "clipsNotPasted" });
    }
    // Round 12 A1: `count` copies of the block (the earliest start to the latest end), all or nothing.
    case "repeatClips": {
      const ids = inOrder(command.clips);
      if (ids.length === 0 || command.count < 1 || command.count > REPEAT_COUNT_MAX) return null;
      const clips = ids.map((id) => clipOf(id)!);
      const first = Math.min(...clips.map((c) => c.start));
      const length = Math.max(...clips.map((c) => c.end)) - first;
      const items = Array.from({ length: command.count }, (_, k) => clips.map((clip) => ({ row: rowOf(clip.id), start: clip.start + (k + 1) * length, clip })));
      return place(items.flat(), true, [], { id: "clipsNotDuplicated" });
    }
    default:
      return null;
  }
}

const busy = (state: AppState) => state.transport.state === "recording" || state.transport.state === "countIn";

const SHELF_MESSAGES: StatusMessage["id"][] = [
  "auditionNewTrack",
  "auditionReplace",
  "samplePlaying",
  "auditionNewTrackSound",
  "auditionReplaceSound",
  "samplePlayingSound",
  "pluginLoading",
];
const isShelfMessage = (m: StatusMessage | null) => m !== null && SHELF_MESSAGES.includes(m.id);
/** The short names of new tracks, by group; the organs are 「オルガン」 and the mallets 「マレット」 (spec 3 (84), track names). */
const GROUP_TRACK_NAMES: Record<string, string> = {
  pianoKeys: "ピアノ",
  guitar: "ギター",
  bass: "ベース",
  drums: "ドラム",
  padStrings: "パッド",
  wind: "管楽器",
  synth: "シンセ",
};
const groupOf = (i: InstrumentKind) => INSTRUMENT_GROUPS.find((g) => g.instruments.includes(i))!.group;
const trackNameOf = (i: InstrumentKind) =>
  i.startsWith("organ") ? "オルガン" : i === "vibraphone" || i === "marimba" ? "マレット" : GROUP_TRACK_NAMES[groupOf(i)]!;
const SAMPLE_US = 4_000_000;

/** P4: a new studio kit track at `place`, as `shelfConfirm` makes one; the tracks after it are numbered again. */
function newKitTrack(tracks: TrackView[], place: number): TrackView {
  const base = trackNameOf("studioKit");
  const names = new Set(tracks.map((t) => t.name));
  let name = base;
  for (let n = 2; names.has(name); n++) name = `${base} ${n}`;
  const track: TrackView = {
    id: Math.max(0, ...tracks.map((t) => t.id)) + 1,
    number: place + 1,
    name,
    color: (place % 5) + 1,
    instrument: "studioKit",
    arp: FAKE_ARP_OFF,
    kind: { kind: "keys" },
    sound: builtinSound("studioKit"),
    standIn: null,
    drumKeys: null,
    gainDb: 0,
    pan: 0,
    sends: newTrackSends("studioKit"),
    sendDefaults: newTrackSends("studioKit"),
    inserts: noInserts(),
    slots: builtinSlots(noInserts()),
    muted: false,
    soloed: false,
    lanes: [],
    clips: [],
  };
  tracks.splice(place, 0, track);
  tracks.forEach((t, i) => (t.number = i + 1));
  return track;
}

/** Whether the shelf has the sound: one that comes with meloet, a preset of a file that was added, or a plugin in one of its formats. */
function inLibrary(sound: SoundRef, library: LibraryView): boolean {
  if (sound.kind === "builtin") return true;
  if (sound.kind === "plugin") return library.plugins.some((r) => r.kind.role === "instrument" && r.formats.some((f) => f.key === sound.key));
  return library.sf2.some((f) => f.file === sound.file && f.presets.some((p) => p.bank === sound.bank && p.program === sound.program));
}

/** The name the status line gives a sound of the shelf. */
function soundName(sound: SoundRef, library: LibraryView): SoundName {
  if (sound.kind === "builtin") return sound;
  if (sound.kind === "plugin") return { kind: "plugin", name: library.plugins.find((r) => r.formats.some((f) => f.key === sound.key))?.name ?? sound.key };
  const preset = library.sf2.find((f) => f.file === sound.file)?.presets.find((p) => p.bank === sound.bank && p.program === sound.program);
  return { kind: "sf2", name: preset?.name ?? "", file: sound.file };
}

function soundNameOf(view: SoundView): SoundName {
  if (view.kind === "builtin") return view;
  return view.kind === "plugin" ? { kind: "plugin", name: view.name } : { kind: "sf2", name: view.name, file: view.file };
}

/** What the shelf says on the status line while it auditions or plays a sample. */
function shelfMessage(shelf: ShelfView, song: SongView, library: LibraryView): StatusMessage | null {
  const replaced = shelf.mode.kind === "replace" ? song.tracks.find((x) => x.id === (shelf.mode as { track: number }).track) : undefined;
  if (shelf.selected !== null) {
    if (shelf.mode.kind === "newTrack") return { id: "auditionNewTrack", instrument: shelf.selected };
    if (replaced) return { id: "auditionReplace", instrument: shelf.selected, track: { number: replaced.number, name: replaced.name }, current: replaced.instrument };
  }
  const picked = shelf.selectedSound;
  if (picked !== null && picked.kind !== "builtin") {
    const sound = soundName(picked, library);
    if (shelf.loadingSound !== null && sound.kind === "plugin") return { id: "pluginLoading", name: sound.name };
    if (shelf.mode.kind === "newTrack") return { id: "auditionNewTrackSound", sound };
    if (replaced) return { id: "auditionReplaceSound", sound, track: { number: replaced.number, name: replaced.name }, current: soundNameOf(replaced.sound) };
  }
  return shelf.sample ? { id: "samplePlaying", instrument: shelf.sample.instrument } : null;
}

/** 「曲から外す」: the tracks that use it get the sine wave, and the slots that hold it are emptied. */
function withoutMissing(song: SongView, gone: Extract<Command, { type: "removeMissing" }>["missing"]): SongView {
  const is = (p: PluginView) => gone.kind === "plugin" && p.key === gone.key;
  const strip = <T extends { slots: Slots }>(s: T): T => ({ ...s, slots: s.slots.map((x) => (x?.effect.kind === "plugin" && is(x.effect) ? null : x)) as Slots });
  return {
    ...song,
    tracks: song.tracks.map((t) => {
      const uses = t.sound.kind === "plugin" ? is(t.sound) : t.sound.kind === "sf2" && gone.kind === "sf2" && t.sound.file === gone.file;
      return strip(uses ? { ...t, instrument: "sine", kind: { kind: "keys" }, sound: builtinSound("sine"), standIn: null, drumKeys: null } : t);
    }),
    returns: [strip(song.returns[0]), strip(song.returns[1])],
    master: strip(song.master),
  };
}

/** The commands for any sound or effect, as the ones for what comes with meloet when they name one of those. */
function ownCommand(command: Command): Command {
  switch (command.type) {
    case "shelfSelectSound":
      return command.sound === null || command.sound.kind === "builtin" ? { type: "shelfSelect", instrument: command.sound?.instrument ?? null } : command;
    case "playSampleOf":
      return command.sound.kind === "builtin" ? { type: "playSample", instrument: command.sound.instrument } : command;
    case "toggleFavoriteSound":
      return command.sound.kind === "builtin" ? { type: "toggleFavorite", instrument: command.sound.instrument } : command;
    case "shelfChooseEffect":
      return command.effect === null || command.effect.kind === "builtin" ? { type: "shelfSelectEffect", effect: command.effect?.effect ?? null } : command;
    default:
      return command;
  }
}

const sameRef = (a: SoundRef | EffectRef, b: SoundRef | EffectRef) => JSON.stringify(a) === JSON.stringify(b);

/**
 * Protocol 17: SF2 presets and plugins in the shelf, enough for pictures and tests. A plugin is read at once
 * (the scenes show the reading), `shelfConfirm` puts none of them into the song, and nothing sounds.
 * Changes `shelf`, `library` and `fileDrop` in place.
 */
function reduceAdded(command: Command, shelf: ShelfView, library: LibraryView, ws: AppState["workspace"]): void {
  const toggle = <T extends SoundRef | EffectRef>(list: T[], item: T): T[] =>
    list.some((x) => sameRef(x, item)) ? list.filter((x) => !sameRef(x, item)) : [...list, item];
  switch (command.type) {
    // What ends an audition, or starts one of a sound of meloet, puts away the SF2 preset or plugin that was auditioned.
    case "shelfSelect":
    case "shelfCancel":
    case "shelfConfirm":
    case "openShelf":
      shelf.selectedSound = shelf.loadingSound = shelf.unreadable = null;
      if (command.type !== "shelfSelect") shelf.chosenEffect = null;
      break;
    case "shelfSelectEffect":
      shelf.chosenEffect = null;
      break;
    case "shelfSelectSound":
      if (shelf.mode.kind === "insertEffect" || shelf.mode.kind === "replaceEffect") break;
      shelf.selected = shelf.loading = shelf.loadingSound = shelf.unreadable = null;
      shelf.selectedSound = command.sound !== null && inLibrary(command.sound, library) ? command.sound : null;
      break;
    case "shelfChooseEffect": {
      const effect = command.effect;
      if (shelf.mode.kind !== "insertEffect" && shelf.mode.kind !== "replaceEffect") break;
      const has = effect?.kind === "plugin" && library.plugins.some((r) => r.kind.role === "effect" && r.formats.some((f) => f.key === effect.key));
      shelf.selectedEffect = null;
      shelf.chosenEffect = has ? effect : null;
      break;
    }
    case "toggleFavoriteSound":
      if (inLibrary(command.sound, library)) shelf.favoriteSounds = toggle(shelf.favoriteSounds, command.sound);
      break;
    case "toggleFavoriteEffect": {
      const effect = command.effect;
      if (effect.kind === "builtin" || library.plugins.some((r) => r.formats.some((f) => f.key === effect.key))) {
        shelf.favoriteEffects = toggle(shelf.favoriteEffects, effect);
      }
      break;
    }
    case "usePluginFormat": {
      const row = library.plugins.find((r) => r.formats.some((f) => f.key === command.key));
      if (!row || row.key === command.key) break;
      // What named the row by the format it used names it by the new one: the star stays on the row.
      const moved = <T extends SoundRef | EffectRef>(x: T): T => (x.kind === "plugin" && x.key === row.key ? { ...x, key: command.key } : x);
      shelf.favoriteSounds = shelf.favoriteSounds.map(moved);
      shelf.favoriteEffects = shelf.favoriteEffects.map(moved);
      row.key = command.key;
      break;
    }
    case "cancelFileDrop":
      ws.fileDrop = null;
      break;
    case "removeMissing": {
      const gone = command.missing;
      shelf.missing = shelf.missing.filter((m) => !(m.kind === gone.kind && (m.kind === "sf2" ? gone.kind === "sf2" && m.file === gone.file : gone.kind === "plugin" && m.key === gone.key)));
      break;
    }
    default:
      break;
  }
}

/**
 * Protocol 13: the track commands and the shelf, enough for pictures and tests (no history, no undo count).
 * Returns what changed, or null when the command is none of these.
 */
export function reduceTracks(
  state: AppState,
  bands: FakeBands,
  command: Command,
  nowUs = 0,
): { song: SongView; shelf: ShelfView; canRecord: boolean; notice?: StatusMessage } | null {
  const song = structuredClone(state.song);
  const shelf = structuredClone(state.shelf);
  const at = (id: number) => song.tracks.findIndex((t) => t.id === id);
  const ref = (i: number): TrackRef => ({ number: i + 1, name: song.tracks[i]!.name });
  const renumber = () => song.tracks.forEach((t, i) => (t.number = i + 1));
  const audition = (selected: InstrumentKind | null) => {
    shelf.selected = selected;
    shelf.loading = null;
  };
  let notice: StatusMessage | undefined;
  const change = !busy(state);
  switch (command.type) {
    case "deleteTrack": {
      const i = at(command.track);
      if (i < 0 || !change) return null;
      notice = { id: "trackDeleted", track: ref(i) };
      song.tracks.splice(i, 1);
      renumber();
      if (song.playTarget === command.track) song.playTarget = (song.tracks[i] ?? song.tracks[i - 1])?.id ?? null;
      break;
    }
    case "duplicateTrack": {
      // A3: right below, everything but the solo, the ●'s own state and the monitor; new ids, the takes shared.
      const i = at(command.track);
      if (i < 0 || !change) return null;
      if (song.tracks.length >= MAX_TRACKS) {
        notice = { id: "tooManyTracks" };
        break;
      }
      const copy = structuredClone(song.tracks[i]!);
      const named = /^(.+) ([1-9][0-9]*)$/.exec(copy.name);
      const base = named && Number(named[2]) >= 2 ? named[1]! : copy.name;
      const names = new Set(song.tracks.map((t) => t.name));
      let n = 2;
      while (names.has(`${base} ${n}`)) n++;
      copy.name = `${base} ${n}`;
      let next =
        Math.max(0, ...song.tracks.flatMap((t) => [t.id, ...t.clips.map((c) => c.id), ...t.slots.map((s) => s?.id ?? 0), ...t.lanes.map((l) => l.laneId)])) + 1;
      copy.id = next++;
      copy.soloed = false;
      if (copy.kind.kind === "vocal") copy.kind = { ...copy.kind, armed: true, monitor: false };
      for (const c of copy.clips) c.id = next++;
      const inserts = new Map<number, number>();
      copy.slots = copy.slots.map((s) => {
        if (s === null || s.id === NO_INSERT_ID) return s;
        inserts.set(s.id, next);
        return { ...s, id: next++ };
      }) as typeof copy.slots;
      copy.lanes = copy.lanes.map((l) => {
        const laneId = next++;
        const points = FAKE_LANES.get(l.laneId);
        if (points) FAKE_LANES.set(laneId, { ...points, id: laneId });
        const target = l.target.kind === "insert" ? { ...l.target, insert: inserts.get(l.target.insert) ?? l.target.insert } : l.target;
        return { ...l, laneId, target };
      });
      song.tracks.splice(i + 1, 0, copy);
      renumber();
      song.playTarget = copy.id;
      break;
    }
    case "renameTrack": {
      const i = at(command.track);
      const name = command.name.trim();
      if (i < 0 || !change || name === "") return null;
      song.tracks[i]!.name = name;
      break;
    }
    case "moveTrack": {
      const i = at(command.track);
      if (i < 0 || !change) return null;
      const [t] = song.tracks.splice(i, 1);
      song.tracks.splice(Math.min(command.to, song.tracks.length), 0, t!);
      renumber();
      break;
    }
    case "setTrackColor": {
      const i = at(command.track);
      if (i < 0 || !change || command.color < 1 || command.color > 5) return null;
      song.tracks[i]!.color = command.color;
      break;
    }
    case "openShelf": {
      if (!change) return null;
      const mode = command.mode;
      if (mode.kind === "replace" && at(mode.track) < 0) return null;
      const held = mode.kind === "replaceEffect" ? (stripOf(song, mode.strip)?.slots[mode.slot] ?? null) : null;
      if (mode.kind === "replaceEffect" && !held) return null;
      shelf.mode = mode;
      shelf.current = mode.kind === "replace" ? song.tracks[at(mode.track)]!.instrument : null;
      shelf.currentEffect = held ? (held.effect.kind === "builtin" ? builtinEffect(held.effect.effect) : { kind: "plugin", key: held.effect.key }) : null;
      audition(null);
      if (bands.side.at(-1) !== "shelf") bands.side = [...bands.side.filter((b) => b !== "shelf"), "shelf"];
      break;
    }
    case "shelfSelect":
      audition(command.instrument !== null && shelf.available.includes(command.instrument) ? command.instrument : null);
      break;
    case "shelfCancel":
      audition(null);
      break;
    case "shelfConfirm": {
      const picked = shelf.selected;
      if (picked === null || !change) return null;
      // The row stays auditioned, as in core.
      if (shelf.mode.kind === "newTrack" && song.tracks.length >= MAX_TRACKS) return { song, shelf, canRecord: false, notice: { id: "tooManyTracks" } };
      if (shelf.mode.kind === "newTrack") {
        const base = trackNameOf(picked);
        const names = new Set(song.tracks.map((t) => t.name));
        let name = base;
        for (let n = 2; names.has(name); n++) name = `${base} ${n}`;
        const sel = song.playTarget === null ? -1 : at(song.playTarget);
        const place = sel < 0 ? song.tracks.length : sel + 1;
        const id = Math.max(0, ...song.tracks.map((t) => t.id)) + 1;
        song.tracks.splice(place, 0, {
          id,
          number: place + 1,
          name,
          color: (place % 5) + 1,
          instrument: picked,
          kind: { kind: "keys" },
          sound: builtinSound(picked),
          standIn: null,
          drumKeys: null,
          gainDb: 0,
          pan: 0,
          sends: newTrackSends(picked),
          sendDefaults: newTrackSends(picked),
          inserts: noInserts(),
          slots: builtinSlots(noInserts()),
          muted: false,
          soloed: false,
          lanes: [],
          clips: [],
          arp: FAKE_ARP_OFF,
        });
        renumber();
        song.playTarget = id;
        notice = { id: "trackAdded", track: ref(place) };
      } else if (shelf.mode.kind === "replace") {
        const i = at(shelf.mode.track);
        if (i < 0) return null;
        if (song.tracks[i]!.instrument !== picked) {
          song.tracks[i]!.instrument = picked;
          notice = { id: "instrumentReplaced", track: ref(i), instrument: picked, lanesRemoved: 0 };
        }
        shelf.current = picked;
      }
      audition(null);
      break;
    }
    case "toggleFavorite": {
      const had = shelf.favorites.includes(command.instrument);
      const all = INSTRUMENT_GROUPS.flatMap((g) => [...g.instruments]);
      shelf.favorites = all.filter((i) => (i === command.instrument ? !had : shelf.favorites.includes(i)));
      break;
    }
    case "playSample":
      shelf.sample = { instrument: command.instrument, startedUs: nowUs, lengthUs: SAMPLE_US };
      break;
    case "stopSample":
      shelf.sample = null;
      break;
    default:
      return null;
  }
  const canRecord = shelf.selected === null && song.playTarget !== null && state.transport.canPlay;
  return { song, shelf, canRecord, notice };
}

/** The table of each fake plugin that has parameters, by its name. */
const PLUGIN_TABLES: Record<string, number> = {
  "Dragonfly Reverb": FAKE_TABLES.dragonfly,
  "Mossy Verb": FAKE_TABLES.mossy,
  MVerb: FAKE_TABLES.mverb,
  "Surge XT": FAKE_TABLES.surge,
};

/** The strip `strip` names in `song`: a track, a return or the master. */
function stripOf(song: SongView, strip: StripRef) {
  if (strip.kind === "master") return song.master;
  if (strip.kind === "return") return song.returns[strip.bus === "a" ? 0 : 1];
  const id = strip.id;
  return song.tracks.find((t) => t.id === id);
}

/** A level as Rust keeps it: `null` (−∞) below −60, at most +6, in steps of 0.01. */
const clampDb = (db: number | null) => (db === null || db < -60 ? null : Math.min(6, Math.round(db * 100) / 100));

/**
 * Protocol 14: the mixer, enough for pictures and tests (no history, no undo count, no previews: a command with
 * `commit: false` changes nothing here, as the state never shows a preview). Returns what changed, or null when
 * the command is none of these. The values of the effects are kept only for the selected insert. Taken while
 * recording and counting in too (docs/history/design-notes.ja.md A-14).
 */
export function reduceMixer(
  state: AppState,
  shelfNow: ShelfView,
  songNow: SongView,
  command: Command,
): {
  song: SongView;
  params: AppState["params"];
  shelf: ShelfView;
  windows: AppState["windows"];
  showParams: boolean;
  notice: StatusMessage | null;
} | null {
  const song = structuredClone(songNow);
  let notice: StatusMessage | null = null;
  const params = structuredClone(state.params);
  const shelf = structuredClone(shelfNow);
  const windows = structuredClone(state.windows);
  let showParams = false;
  const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
  /** The selected insert when it holds an effect that comes with meloet. */
  const own = () => (params.selected?.kind === "builtin" ? params.selected : null);
  const pluginAt = (slot: PluginSlotRef): PluginView | null => {
    if (slot.kind === "instrument") {
      const sound = song.tracks.find((t) => t.id === slot.track)?.sound;
      return sound?.kind === "plugin" ? sound : null;
    }
    const held = stripOf(song, slot.insert.strip)?.slots[slot.insert.slot]?.effect;
    return held?.kind === "plugin" ? held : null;
  };
  const openWindow = (slot: PluginSlotRef, name: string) => {
    if (windows.list.some((w) => same(w.slot, slot))) return;
    windows.list.push({ slot, name, width: 800, height: 600, largerThanScreen: false });
    notice = windows.list.length === 1 ? { id: "pluginWindowOpen", name } : { id: "pluginWindowsOpen", count: windows.list.length };
  };
  /** The effect in a slot, as the status line names it. */
  const nameAt = (insert: InsertRef): EffectName | null => {
    const held = stripOf(song, insert.strip)?.slots[insert.slot];
    if (!held) return null;
    return held.effect.kind === "builtin" ? { kind: "builtin", effect: held.effect.effect } : { kind: "plugin", name: held.effect.name };
  };
  // A plugin's window opens with it; one that has none the fake could open brings the panel to the front.
  const selectPlugin = (slot: PluginSlotRef, plugin: PluginView) => {
    if (params.selected?.kind !== "plugin" || !same(params.selected.slot, slot)) {
      const table = PLUGIN_TABLES[plugin.name] ?? null;
      const window = plugin.name === "Mossy Verb" ? "notYet" : "opens";
      params.selected = { kind: "plugin", slot, window, table, count: table === null ? 0 : fakeParamCount(table), row: null, automated: [] };
    }
    if (plugin.state !== "ready") return;
    if (params.selected.window === "opens") openWindow(slot, plugin.name);
    else showParams = true;
  };
  const select = (insert: InsertRef | null) => {
    const plugin = insert === null ? null : pluginAt({ kind: "insert", insert });
    if (insert !== null && plugin) return selectPlugin({ kind: "insert", insert }, plugin);
    const effect = insert === null ? null : (stripOf(song, insert.strip)?.inserts[insert.slot] ?? null);
    if (insert === null || effect === null) params.selected = null;
    else if (!same(own()?.insert, insert)) {
      params.selected = { kind: "builtin", insert, effect, values: insertDefaults(effect, insert.strip), row: null, preset: null };
    }
    showParams = params.selected !== null;
  };
  switch (command.type) {
    case "setGain": {
      const strip = stripOf(song, command.strip);
      if (strip && command.commit) strip.gainDb = clampDb(command.db);
      break;
    }
    case "setPan": {
      const strip = stripOf(song, command.strip);
      if (strip && "pan" in strip && command.commit) strip.pan = Math.max(-100, Math.min(100, Math.round(command.pan)));
      break;
    }
    // M2: every track's fader `db` lower, −∞ staying and none below −60 dB; refused while counting in or recording. A
    // track whose volume lane moves keeps its fader (Rust lowers the lane's points, which the fake does not hold).
    case "lowerAllTracks": {
      if (!(command.db > 0) || state.transport.state === "countIn" || state.transport.state === "recording") break;
      for (const track of song.tracks) {
        if (track.gainDb === null || track.lanes.some((l) => l.target.kind === "volume" && !l.resting && l.points > 0)) continue;
        track.gainDb = Math.round(Math.max(-60, track.gainDb - command.db) * 100) / 100;
      }
      break;
    }
    case "setSend": {
      const track = song.tracks.find((t) => t.id === command.track);
      if (track && command.commit) track.sends[command.bus === "a" ? 0 : 1] = clampDb(command.db);
      break;
    }
    case "setParam": {
      const sel = own();
      const spec = EFFECT_PARAMS[command.effect][command.param];
      const there = stripOf(song, command.insert.strip)?.inserts[command.insert.slot];
      if (!spec || there !== command.effect || !command.commit) break;
      if (sel && JSON.stringify(sel.insert) === JSON.stringify(command.insert)) {
        const v = Math.max(spec.min, Math.min(spec.max, command.value));
        sel.values[command.param] = spec.shape === "continuous" ? v : Math.round(v);
      }
      break;
    }
    case "cancelMixPreview":
      break;
    case "setSoundKnob":
      fakeSetSoundKnob(params.selected, command);
      break;
    // A typed value of an effect that comes with meloet: the number in it, put at an end of the range (and said so); a
    // text with no number changes nothing and says so with the value as a bare number ("-24"), as core does. A plugin's is the
    // transport's (FakeParamTables).
    case "typeParam": {
      const p = command.param;
      if (p.kind !== "builtin") return null;
      const spec = EFFECT_PARAMS[p.effect][p.param];
      const sel = own();
      if (!spec || stripOf(song, p.insert.strip)?.inserts[p.insert.slot] !== p.effect) break;
      const now = sel && same(sel.insert, p.insert) ? (sel.values[p.param] ?? spec.default) : spec.default;
      const n = Number.parseFloat(command.text.trim().replace(/^[−ー]/, "-"));
      if (!Number.isFinite(n)) {
        notice = { id: "paramRejected", param: p, kept: String(now) };
        break;
      }
      const v = Math.max(spec.min, Math.min(spec.max, spec.shape === "continuous" ? n : Math.round(n)));
      if (v !== n) notice = { id: "paramClamped", to: { kind: "builtin", effect: p.effect, param: p.param, value: v } };
      if (sel && same(sel.insert, p.insert)) sel.values[p.param] = v;
      break;
    }
    case "toggleReturnMute": {
      const ret = song.returns[command.bus === "a" ? 0 : 1];
      ret.muted = !ret.muted;
      break;
    }
    case "removeEffect": {
      const strip = stripOf(song, command.insert.strip);
      if (!strip) break;
      const at: PluginSlotRef = { kind: "insert", insert: command.insert };
      const gone = nameAt(command.insert);
      if (gone) notice = { id: "effectRemoved", insert: command.insert, effect: gone, lanesRemoved: 0 };
      strip.inserts[command.insert.slot] = null;
      strip.slots[command.insert.slot] = null;
      if (same(own()?.insert, command.insert) || (params.selected?.kind === "plugin" && same(params.selected.slot, at))) params.selected = null;
      windows.list = windows.list.filter((w) => !same(w.slot, at));
      break;
    }
    case "selectInsert":
      select(command.insert);
      break;
    case "selectInstrument": {
      const slot: PluginSlotRef = { kind: "instrument", track: command.track };
      const plugin = pluginAt(slot);
      if (plugin) {
        selectPlugin(slot, plugin);
        break;
      }
      // 19-A: a sound of an SF2 file has no parameters; the panel says so. One that comes with meloet has its knobs (E3).
      const t = song.tracks.find((x) => x.id === command.track);
      if (!t || t.kind.kind !== "keys" || t.standIn) break;
      if (t.sound.kind === "plugin") break;
      const sound = soundNameOf(t.sound);
      const was = params.selected;
      const kept = was?.kind === "sound" && was.track === t.id && same(was.sound, sound) ? was.knobs : null;
      const knobs = sound.kind === "builtin" ? (kept ?? fakeKnobs(sound.instrument)) : null;
      params.selected = { kind: "sound", track: t.id, sound, knobs };
      showParams = true;
      if (!knobs) notice = { id: "noParams", sound };
      break;
    }
    case "selectParam": {
      const sel = params.selected;
      if (sel?.kind === "builtin") sel.row = command.param !== null && EFFECT_PARAMS[sel.effect][command.param] ? command.param : null;
      else if (sel?.kind === "plugin") sel.row = command.param;
      else if (sel?.kind === "sound" && sel.knobs) sel.knobs.row = command.param !== null && sel.knobs.usable[command.param] ? command.param : null;
      break;
    }
    case "toggleBypass": {
      const held = stripOf(song, command.insert.strip)?.slots[command.insert.slot];
      if (!held || (held.effect.kind === "plugin" && held.effect.state === "stopped")) break;
      held.bypassed = !held.bypassed;
      notice = { id: "effectBypassed", insert: command.insert, effect: nameAt(command.insert)!, bypassed: held.bypassed };
      break;
    }
    case "moveInsert": {
      const strip = stripOf(song, command.insert.strip);
      const from = command.insert.slot;
      const to = Math.min(strip ? strip.slots.length - 1 : 0, command.to);
      if (!strip || !strip.slots[from] || to === from) break;
      for (const list of [strip.inserts, strip.slots] as unknown[][]) list.splice(to, 0, ...list.splice(from, 1));
      // Where a slot of this strip is afterwards: the selection and the windows stay with their effects.
      const moved = (slot: number) => (slot === from ? to : slot > from && slot <= to ? slot - 1 : slot < from && slot >= to ? slot + 1 : slot);
      const follow = (insert: InsertRef) => {
        if (same(insert.strip, command.insert.strip)) insert.slot = moved(insert.slot);
      };
      const sel = params.selected;
      if (sel?.kind === "builtin") follow(sel.insert);
      else if (sel?.kind === "plugin" && sel.slot.kind === "insert") follow(sel.slot.insert);
      for (const w of windows.list) if (w.slot?.kind === "insert") follow(w.slot.insert);
      const order = strip.slots.flatMap((_, i) => nameAt({ strip: command.insert.strip, slot: i }) ?? []);
      notice = { id: "insertsReordered", strip: command.insert.strip, order };
      break;
    }
    case "openPluginWindow": {
      const plugin = command.slot && pluginAt(command.slot);
      if (command.slot && plugin && plugin.state === "ready" && plugin.name !== "Mossy Verb") openWindow(command.slot, plugin.name);
      break;
    }
    case "closePluginWindow":
      windows.list = windows.list.filter((w) => !same(w.slot, command.slot));
      break;
    case "shelfSelectEffect":
      if (shelf.mode.kind === "insertEffect" || shelf.mode.kind === "replaceEffect") shelf.selectedEffect = command.effect;
      break;
    case "shelfConfirm": {
      if (shelf.mode.kind !== "insertEffect" && shelf.mode.kind !== "replaceEffect") return null;
      const { kind, strip, slot } = shelf.mode;
      const to = stripOf(song, strip);
      if (!to || shelf.selectedEffect === null || (to.slots[slot] !== null) !== (kind === "replaceEffect")) break;
      if (same(shelf.currentEffect, builtinEffect(shelf.selectedEffect))) break;
      const effect: EffectName = { kind: "builtin", effect: shelf.selectedEffect };
      notice = kind === "replaceEffect" ? { id: "insertReplaced", insert: { strip, slot }, effect, lanesRemoved: 0 } : { id: "effectInserted", insert: { strip, slot }, effect };
      to.inserts[slot] = shelf.selectedEffect;
      to.slots[slot] = null;
      windows.list = windows.list.filter((w) => !same(w.slot, { kind: "insert", insert: { strip, slot } }));
      shelf.mode = { kind: "newTrack" };
      shelf.selectedEffect = shelf.currentEffect = null;
      params.selected = null;
      select({ strip, slot });
      break;
    }
    default:
      return null;
  }
  return { song, params, shelf, windows, showParams, notice };
}

/** The slots the mixer passes by as it opens (MixerPassesBy): in the mixer's order, the bypassed ones and the stopped ones. */
function passedBy(song: SongView): StatusMessage | null {
  const strips: { strip: StripRef; slots: SongView["master"]["slots"] }[] = [
    ...song.tracks.map((t) => ({ strip: { kind: "track", id: t.id } as StripRef, slots: t.slots })),
    { strip: RETURN_A, slots: song.returns[0].slots },
    { strip: { kind: "return", bus: "b" }, slots: song.returns[1].slots },
    { strip: { kind: "master" }, slots: song.master.slots },
  ];
  const bypassed: NamedInsert[] = [];
  const stopped: NamedInsert[] = [];
  for (const { strip, slots } of strips)
    slots.forEach((held, slot) => {
      if (!held) return;
      const effect: EffectName = held.effect.kind === "builtin" ? { kind: "builtin", effect: held.effect.effect } : { kind: "plugin", name: held.effect.name };
      if (held.effect.kind === "plugin" && held.effect.state === "stopped") stopped.push({ insert: { strip, slot }, effect });
      else if (held.bypassed) bypassed.push({ insert: { strip, slot }, effect });
    });
  return bypassed.length + stopped.length > 0 ? { id: "mixerPassesBy", bypassed, stopped } : null;
}

/** `rest` is the status line when no notice is up. */
function reduce(state: AppState, bands: FakeBands, rest: StatusMessage | null, sent: Command, clipboard: FakeClipboard, nowUs = 0): Change[] {
  const command = ownCommand(sent);
  const library = structuredClone(state.library);
  const ws = structuredClone(state.workspace);
  const outputs = structuredClone(state.outputs);
  const keyboards = structuredClone(state.keyboards);
  const transport = structuredClone(state.transport);
  const pianoRoll = structuredClone(state.pianoRoll);
  const plugins = structuredClone(state.plugins);
  reducePianoRoll(pianoRoll, command);
  const tracks = reduceTracks(state, bands, command, nowUs);
  if (tracks) transport.canRecord = tracks.canRecord;
  const choosable = outputs.list.filter((o) => o.unusable === null).map((o) => o.id);
  let notice = isNotice(state.status.message) && command.type === "setViewport" ? state.status.message : null;
  const fitWindow = (scale?: number) => {
    const folded = bands.setHeight((ws.viewportHeight * 100) / ws.scale);
    if (folded) notice = scale ? { id: "bandFoldedScale", scale, band: folded } : { id: "bandFoldedWindowLow", band: folded };
  };
  switch (command.type) {
    case "setViewport":
      ws.viewportWidth = command.width;
      ws.viewportHeight = command.height;
      fitWindow();
      break;
    case "setScale": {
      const percent = command.percent;
      if (!SCALES.includes(percent) || percent === ws.scale) break;
      if (ws.unfitScales.includes(percent)) {
        notice = { id: "scaleDoesNotFit", requested: percent, kept: ws.scale };
        break;
      }
      ws.scale = percent;
      const min = minWindow(percent);
      if (ws.viewportWidth < min.width || ws.viewportHeight < min.height) {
        ws.viewportWidth = Math.max(ws.viewportWidth, min.width);
        ws.viewportHeight = Math.max(ws.viewportHeight, min.height);
        notice = { id: "scaleWindowGrown", scale: percent };
      }
      fitWindow(percent);
      break;
    }
    case "setTheme":
      ws.theme = command.theme;
      break;
    case "setNoteGlow":
      ws.noteGlow = command.on;
      break;
    case "toggleBand": {
      const mixerWasOpen = bands.bottom.includes("mixer");
      const fold = bands.toggle(command.band);
      if (command.band === "mixer" && !mixerWasOpen && bands.bottom.includes("mixer")) notice = passedBy(state.song);
      if (fold?.why === "third") notice = { id: "bandFoldedThird", opened: command.band, folded: fold.band };
      if (fold?.why === "tooLow") notice = { id: "bandFoldedToOpen", opened: command.band, folded: fold.band };
      break;
    }
    case "clampValue":
      notice = { id: "valueClamped", field: command.field, value: command.value, range: command.range };
      break;
    // The fake does not change notes; it only says what Rust says.
    case "matchVelocity":
      if (command.take === pianoRoll.take && command.notes.length > 0) notice = { id: "velocityMatched", count: command.notes.length, value: command.value };
      break;
    case "matchLength":
      if (command.take === pianoRoll.take && command.notes.length > 0) notice = { id: "lengthMatched", count: command.notes.length, value: Math.max(1, command.value) };
      break;
    case "scaleVelocity":
      if (command.take === pianoRoll.take && command.notes.length > 0)
        notice = { id: "velocityScaled", count: command.notes.length, percent: Math.min(300, Math.max(10, command.percent)) };
      break;
    case "rampVelocity": {
      const v = (x: number) => Math.min(127, Math.max(1, x));
      if (command.take === pianoRoll.take && command.notes.length > 0)
        notice = { id: "velocityRamped", count: command.notes.length, from: v(command.from), to: v(command.to) };
      break;
    }
    case "randomVelocity":
      if (command.take === pianoRoll.take && command.notes.length > 0)
        notice = { id: "velocityRandomized", count: command.notes.length, amount: Math.min(32, Math.max(1, command.amount)) };
      break;
    case "legato":
      if (command.take === pianoRoll.take && command.notes.length > 0) notice = { id: "notesLegato", count: command.notes.length };
      break;
    case "setVelocityFixed":
      keyboards.velocityFixed = { on: command.on, value: Math.min(127, Math.max(1, command.value)) };
      break;
    // R3: only what is kept shows; the fake makes no sound.
    case "setClick":
      if (!command.commit) break;
      if (command.volumeDb !== null && Number.isFinite(command.volumeDb))
        transport.clickVolumeDb = Math.min(CLICK_VOLUME_MAX_DB, Math.max(CLICK_VOLUME_MIN_DB, command.volumeDb));
      transport.clickSound = command.sound ?? transport.clickSound;
      transport.clickWhen = command.when ?? transport.clickWhen;
      break;
    case "locate":
      if (transport.state === "stopped") transport.position = command.tick;
      break;
    case "rejectValue":
      notice = { id: "valueRejected", field: command.field, kept: command.kept, range: command.range };
      break;
    // The fake does not move notes; it only says what Rust says.
    case "quantize": {
      const { division, triplet, strength } = pianoRoll.quantize;
      const all = command.take === pianoRoll.take ? (FAKE_TAKES.get(command.take)?.notes.length ?? 0) : 0;
      notice = { id: "quantized", division, triplet, strength, count: command.notes.length || all };
      break;
    }
    case "goArrange":
      bands.goArrange();
      break;
    case "back":
      if (plugins.stoppedOpen) plugins.stoppedOpen = false;
      else if (ws.songDialog) ws.songDialog = null;
      else if (ws.settings) ws.settings = null;
      else bands.back();
      break;
    // The fake keeps no file: every answer just puts the dialog away.
    case "answerSongDialog":
      if (ws.songDialog?.id === command.dialog) ws.songDialog = null;
      break;
    case "openSettings":
      if (ws.settings) ws.settings.advancedOpen ||= command.advanced;
      else ws.settings = { draftOutput: outputs.chosen, advancedOpen: command.advanced };
      break;
    case "settingsSelectOutput":
      if (ws.settings && choosable.includes(command.output)) ws.settings.draftOutput = command.output;
      break;
    case "settingsToggleAdvanced":
      if (ws.settings) ws.settings.advancedOpen = !ws.settings.advancedOpen;
      break;
    case "settingsApply":
      if (ws.settings?.draftOutput) {
        outputs.chosen = ws.settings.draftOutput;
        outputs.chosenState = "ready";
      }
      ws.settings = null;
      break;
    case "settingsCancel":
      ws.settings = null;
      break;
    case "chooserMove":
      if (ws.chooser) {
        const at = choosable.indexOf(ws.chooser.highlighted ?? "");
        if (at >= 0) ws.chooser.highlighted = choosable[Math.max(0, Math.min(choosable.length - 1, at + command.delta))]!;
      }
      break;
    case "chooserSelect":
      if (ws.chooser && choosable.includes(command.output)) ws.chooser.highlighted = command.output;
      break;
    case "chooserConfirm":
      if (ws.chooser?.highlighted) {
        outputs.chosen = ws.chooser.highlighted;
        outputs.chosenState = "ready";
        ws.chooser = null;
      }
      break;
    case "dismissHint":
      if (ws.recovery?.kind === "unreadable") ws.recovery = null;
      else if (!ws.recovery) ws.hint = null;
      break;
    case "discardRecovered":
      if (ws.recovery?.kind === "recovered") ws.recovery = null;
      break;
    // The fake scans nothing: a retry stays 「ためしています…」, and 「場所を足す…」 and 「もう一度スキャン」 do nothing.
    case "openScanStopped":
      plugins.stoppedOpen = true;
      break;
    case "closeScanStopped":
      plugins.stoppedOpen = false;
      break;
    case "openPluginPlaces":
      plugins.stoppedOpen = false;
      plugins.placesOpen = true;
      ws.settings = { draftOutput: ws.settings?.draftOutput ?? outputs.chosen, advancedOpen: true };
      break;
    case "togglePluginPlaces":
      plugins.placesOpen = !plugins.placesOpen;
      break;
    case "removePluginPlace":
      plugins.places = plugins.places.filter((p) => !p.added || p.path !== command.path);
      break;
    case "retryStoppedPlugin":
      for (const p of plugins.stopped) if (p.path === command.path && p.id === command.id) p.retrying = true;
      break;
    default:
      break;
  }
  ws.layout = bands.layout();
  const changes: Change[] = [];
  const clips = reduceClips(state, command, clipboard);
  if (clips) {
    if (JSON.stringify(clips.song) !== JSON.stringify(state.song)) changes.push({ section: "song", value: clips.song });
    if (JSON.stringify(clips.arrange) !== JSON.stringify(state.arrange)) changes.push({ section: "arrange", value: clips.arrange });
  }
  const mixer = reduceMixer(state, tracks?.shelf ?? state.shelf, tracks?.song ?? clips?.song ?? state.song, command);
  if (mixer?.showParams && bands.side.at(-1) !== "params") {
    bands.side = [...bands.side.filter((b) => b !== "params"), "params"];
    ws.layout = bands.layout();
  }
  let song = mixer?.song ?? tracks?.song ?? clips?.song ?? state.song;
  if ((tracks || mixer) && JSON.stringify(song) !== JSON.stringify(state.song)) changes.push({ section: "song", value: song });
  if (command.type === "removeMissing") {
    song = withoutMissing(song, command.missing);
    if (JSON.stringify(song) !== JSON.stringify(state.song)) changes.push({ section: "song", value: song });
  }
  for (const c of changes) if (c.section === "song") song = c.value = settleSong(c.value as SongView);
  // Round 12 M1: 「プリセット ▾」, and the name it shows.
  const params = mixer?.params ?? structuredClone(state.params);
  const sel = params.selected?.kind === "builtin" ? params.selected : null;
  const presetsDone = reducePresets(state.presets, sel, transport.state === "recording" || transport.state === "countIn", command);
  const presets = presetsDone?.presets ?? state.presets;
  if (sel) sel.preset = presetOf(sel.effect, sel.values, presets);
  if (JSON.stringify(params) !== JSON.stringify(state.params)) changes.push({ section: "params", value: params });
  if (JSON.stringify(presets) !== JSON.stringify(state.presets)) changes.push({ section: "presets", value: presets });
  if (mixer && JSON.stringify(mixer.windows) !== JSON.stringify(state.windows)) changes.push({ section: "windows", value: mixer.windows });
  let shelf = structuredClone(mixer?.shelf ?? tracks?.shelf ?? state.shelf);
  reduceAdded(command, shelf, library, ws);
  // A closed shelf has no mode, audition or sample (protocol 13, ShelfView). Its stars and what is missing stay.
  if (!ws.layout.rightOpen.includes("shelf")) {
    const { available, notYet, favorites, favoriteSounds, favoriteEffects, missing } = shelf;
    shelf = { ...closedShelf(), available, notYet, favorites, favoriteSounds, favoriteEffects, missing };
  }
  shelf = settleShelf(shelf);
  if (JSON.stringify(shelf.selectedSound) !== JSON.stringify(state.shelf.selectedSound)) {
    transport.canRecord = shelf.selectedSound === null && song.playTarget !== null && transport.canPlay;
  }
  if (JSON.stringify(shelf) !== JSON.stringify(state.shelf)) changes.push({ section: "shelf", value: shelf });
  if (JSON.stringify(library) !== JSON.stringify(state.library)) changes.push({ section: "library", value: library });
  if (JSON.stringify(ws) !== JSON.stringify(state.workspace)) changes.push({ section: "workspace", value: ws });
  if (JSON.stringify(outputs) !== JSON.stringify(state.outputs)) changes.push({ section: "outputs", value: outputs });
  if (JSON.stringify(keyboards) !== JSON.stringify(state.keyboards)) changes.push({ section: "keyboards", value: keyboards });
  if (JSON.stringify(transport) !== JSON.stringify(state.transport)) changes.push({ section: "transport", value: transport });
  if (JSON.stringify(pianoRoll) !== JSON.stringify(state.pianoRoll)) changes.push({ section: "pianoRoll", value: pianoRoll });
  if (JSON.stringify(plugins) !== JSON.stringify(state.plugins)) changes.push({ section: "plugins", value: plugins });
  // The fake has no song files: 「最近ひらいた曲 ▸」 opens nothing; 「一覧を消す」 empties the list.
  if (command.type === "clearRecent" && state.recent.length > 0) changes.push({ section: "recent", value: [] });
  // M2: 「閉じる」 and 「下げる」 put 「音が大きすぎます」 away (下げる not while counting in or recording, when it is not shown).
  const busy = state.transport.state === "countIn" || state.transport.state === "recording";
  const tooLoudGone =
    state.status.message?.id === "tooLoud" && (command.type === "dismissTooLoud" || (command.type === "lowerAllTracks" && !busy));
  // Scenes set the rest of the status line by hand; only the notices and the selected notes move here.
  let message =
    notice ??
    mixer?.notice ??
    presetsDone?.notice ??
    clips?.notice ??
    tracks?.notice ??
    shelfMessage(shelf, song, library) ??
    (tooLoudGone ? null : isNotice(state.status.message) || isShelfMessage(state.status.message) ? rest : state.status.message);
  if (message?.id === "summary") {
    const notes = noteSelection(pianoRoll);
    message = { ...message, notes, nothingSelected: notes === null && (clips?.arrange ?? state.arrange).selectedClips.length === 0 };
  }
  if (JSON.stringify(message) !== JSON.stringify(state.status.message)) changes.push({ section: "status", value: { message } });
  return changes;
}

/** With no `initial`, the scene comes from the URL (`?scene=`), or else the plain fakeState(). */
/** How often the fake's fetch of the sound pack moves (a scene's fetch stands still until 「取りにいく」 is sent). */
const FETCH_TICK_MS = 250;

export function createFakeTransport(initial?: AppState): Transport {
  const scene = initial ? null : sceneFromUrl();
  let state = initial ?? scene?.state ?? fakeState();
  const urlTheme = initial || scene ? null : themeFromUrl();
  if (urlTheme) state.workspace.theme = urlTheme;
  const urlLanguage = initial ? null : languageFromUrl();
  if (urlLanguage) state.workspace.language = urlLanguage;
  const bands = scene?.bands ?? new FakeBands((state.workspace.viewportHeight * 100) / state.workspace.scale);
  if (!scene) for (const l of state.workspace.layout.lower) bands.bottom.push(l.band);
  if (!scene) bands.side = [...state.workspace.layout.rightOpen];
  const rest = isNotice(state.status.message) || isShelfMessage(state.status.message) ? null : state.status.message;
  const clipboard: FakeClipboard = { clips: [] };
  const vocalMemo: FakeVocalMemo = { monitorSaid: false };
  const tables = new FakeParamTables();
  const typed = new FakeTypedKeys();
  // The rows of the history band: the scene's, and those of the steps the fake made since.
  const historyRows: HistoryRow[] = [...(scene?.history ?? [])];
  let rev = 1;
  let listener: ((message: ServerMessage) => void) | null = null;
  const start = performance.now();
  const nowUs = () => Math.round((performance.now() - start) * 1000);
  const land = (changes: Change[]) => {
    if (changes.length === 0) return;
    rev += 1;
    state = { ...state };
    for (const c of changes) (state as Record<string, unknown>)[c.section] = c.value;
    listener?.({ kind: "patch", rev, changes });
  };
  // The fetch of the sound pack moves by itself, as Rust's does, a few times a second until it ends or stops.
  let fetchTimer: ReturnType<typeof setInterval> | null = null;
  const fetchTick = () => {
    land(advancePack(state, FETCH_TICK_MS / 1000));
    if (state.sounds.fetch.state !== "fetching" && fetchTimer !== null) {
      clearInterval(fetchTimer);
      fetchTimer = null;
    }
  };

  return {
    async hello(protocolVersion) {
      return { protocolVersion: PROTOCOL_VERSION, ok: protocolVersion === PROTOCOL_VERSION, monoUs: nowUs(), language: state.workspace.language };
    },
    async subscribe(onMessage, onTelemetry) {
      listener = onMessage;
      // Only scenes send meters, so tests that use the plain fake leave no timer behind.
      if (scene) {
        let seq = 0;
        setInterval(() => {
          const sent = nowUs();
          const p = scene.meters.position;
          const position = p ? { ...p, tick: p.tick + sent * p.ticksPerUs } : null;
          onTelemetry(encodeTelemetry(scene.meters, ++seq, sent, state.song.tracks.length, position));
          if (scene.scrollToEnd) for (const el of document.querySelectorAll("[data-gl-scroll]")) el.scrollTop = el.scrollHeight;
        }, 1000 / 60);
        for (const event of scene.events) setTimeout(() => onMessage({ kind: "event", ...event }), 0);
      }
    },
    async snapshot() {
      return { rev, state };
    },
    async send(command) {
      const typedDone = typed.reduce(state, command, rest);
      if (typedDone) {
        land(typedDone.changes);
        for (const event of typedDone.events) listener?.({ kind: "event", ...event });
        return { rev };
      }
      const round10 =
        reduceRound10(state, bands, command, FAKE_TAKES) ?? reduceP4(state, command, FAKE_TAKES, newKitTrack, nowUs()) ?? reduceSections(state, command, FAKE_TAKES) ??
        reduceArp(state, command) ??
        reduceMuteSets(state, command);
      if (round10?.row) {
        historyRows.length = Math.min(historyRows.length, round10.row.index - 1);
        historyRows[round10.row.index - 1] = round10.row;
      }
      const changes =
        round10?.changes ??
        reducePack(state, command) ??
        reduceUpdates(state, command) ??
        reduceSpectrum(state, command) ??
        reduceMonoListen(state, command) ??
        reduceLanguage(state, command) ??
        reduceClips27(state, command) ??
        reduceComp(state, command, FAKE_AUDIO) ??
        reduceA9(state, command, FAKE_AUDIO) ??
        reduceA7(state, command, FAKE_AUDIO, (from, to) => {
          const points = FAKE_LANES.get(from);
          if (points) FAKE_LANES.set(to, { ...points, id: to });
        }) ??
        reduceExport(state, command) ??
        reduceAudio(state, command) ??
        reduceRound7(state, command, vocalMemo) ??
        reduceRound6(state, command, historyRows) ??
        reduce(state, bands, rest, command, clipboard, nowUs());
      // Two or more vocal clips selected: the panel 「まとめて変える」 in place of the one clip's.
      const arranged = (changes.find((c) => c.section === "arrange")?.value ?? state.arrange) as ArrangeView;
      const songNow = (changes.find((c) => c.section === "song")?.value ?? state.song) as SongView;
      const paramsNow = (changes.find((c) => c.section === "params")?.value ?? state.params) as AppState["params"];
      const clips = fakeClipsPanel({ ...state, arrange: arranged, song: songNow });
      if (JSON.stringify(clips) !== JSON.stringify(paramsNow.clips ?? null)) {
        const params = { selected: clips ? null : paramsNow.selected, clips };
        const at = changes.findIndex((c) => c.section === "params");
        if (at >= 0) changes.splice(at, 1);
        changes.push({ section: "params", value: params });
      }
      // Whether each mute set is as the song is now follows every change of the song.
      const songChange = changes.find((c) => c.section === "song");
      if (songChange) songChange.value = withMuteSets(songChange.value as SongView);
      // The counts of the history band follow the song's.
      const song = changes.find((c) => c.section === "song");
      if (song && !changes.some((c) => c.section === "history")) {
        const history = historyOf(song.value as SongView);
        if (JSON.stringify(history) !== JSON.stringify(state.history)) changes.push({ section: "history", value: history });
      }
      // The fake plugins' parameters: the value and its text come back on the messages Channel; a typed one may be put at an end.
      let values: ParamValues | null = null;
      if (command.type === "setPluginParam") values = tables.set(command.table, command.param, command.value);
      if (command.type === "typeParam" && command.param.kind === "plugin") {
        const p = command.param;
        const typed = tables.type(p.table, p.param, command.text);
        values = typed.values;
        const said: StatusMessage | null = typed.rejected !== null
          ? { id: "paramRejected", param: p, kept: typed.rejected }
          : typed.clamped
            ? { id: "paramClamped", to: { kind: "plugin", ...typed.clamped } }
            : null;
        if (said) {
          const at = changes.findIndex((c) => c.section === "status");
          if (at >= 0) changes.splice(at, 1);
          changes.push({ section: "status", value: { message: said } });
        }
      }
      land(changes);
      if (state.sounds.fetch.state === "fetching") fetchTimer ??= setInterval(fetchTick, FETCH_TICK_MS);
      if (values) listener?.({ kind: "paramValues", ...values });
      return { rev };
    },
    async post(command) {
      await this.send(command);
    },
    async clock() {
      return nowUs();
    },
    async takeNotes(id) {
      const take = FAKE_TAKES.get(id);
      if (!take) throw new Error(`fake: no take ${id}`);
      return encodeTake(take);
    },
    async lane(laneId) {
      const lane = FAKE_LANES.get(laneId);
      if (!lane) throw new Error(`fake: no lane ${laneId}`);
      return encodeLane(lane);
    },
    async peaks(audio, level, first, count) {
      const file = FAKE_AUDIO.get(audio);
      if (!file) throw new Error(`fake: no audio ${audio}`);
      return fakePeaks(file, level, first, count);
    },
    async historyRows(from, to) {
      return fakeHistoryRows(historyRows, state.history.total, from, to);
    },
    async liveNotes() {
      return scene?.live ?? NO_LIVE;
    },
    async pluginParams(table) {
      return tables.table(table);
    },
  };
}

/** The fake has nothing to measure, except with `?measure=1` in a browser: then the frame profile is kept on `window.__meloetMeasure`. */
export async function fakeMeasureEnabled(): Promise<boolean> {
  return typeof location !== "undefined" && new URLSearchParams(location.search).get("measure") === "1";
}

export async function fakeMeasureReport(name: string, json: string): Promise<void> {
  if (typeof window === "undefined") return;
  const w = window as unknown as { __meloetMeasure?: Record<string, string> };
  (w.__meloetMeasure ??= {})[name] = json;
}

/** With `?measure=1&uiScript=pr-scroll:10` in a browser, that script. */
export async function fakeUiScript(): Promise<string | null> {
  if (typeof location === "undefined" || !(await fakeMeasureEnabled())) return null;
  return new URLSearchParams(location.search).get("uiScript");
}

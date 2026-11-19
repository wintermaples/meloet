// 「あとから入れたもの」 of the fake scenes: one SF2 file and the plugins of the mocks F5-ShelfFx and
// F5-PluginLoading. The rows the mocks show are the mocks'; the rest fills the headings up to the
// mocks' counts ("数は例") and is no real plugin.

import {
  GM_FAMILIES,
  PLUGIN_EFFECT_CATEGORIES,
  PLUGIN_INSTRUMENT_CATEGORIES,
  SF2_DRUM_BANK,
  type LibraryView,
  type PluginEffectCategory,
  type PluginFormat,
  type PluginInstrumentCategory,
  type PluginKindView,
  type PluginRowView,
  type Sf2FileView,
  type Sf2PresetView,
} from "./types";

export const FLUID = "FluidR3_GM.sf2";

/** Names in the manner of General MIDI, eight to a family. The guitars are as the mock F5-SF2Shelf has them. */
const GM_NAMES: string[][] = [
  ["Acoustic Grand Piano", "Bright Acoustic Piano", "Electric Grand Piano", "Honky-tonk Piano", "Electric Piano 1", "Electric Piano 2", "Harpsichord", "Clavinet"],
  ["Celesta", "Glockenspiel", "Music Box", "Vibraphone", "Marimba", "Xylophone", "Tubular Bells", "Dulcimer"],
  ["Drawbar Organ", "Percussive Organ", "Rock Organ", "Church Organ", "Reed Organ", "Accordion", "Harmonica", "Tango Accordion"],
  ["Nylon String Guitar", "Steel String Guitar", "Jazz Guitar", "Clean Guitar", "Palm Muted Guitar", "Overdrive Guitar", "Distortion Guitar", "Guitar Harmonics"],
  ["Acoustic Bass", "Fingered Bass", "Picked Bass", "Fretless Bass", "Slap Bass 1", "Slap Bass 2", "Synth Bass 1", "Synth Bass 2"],
  ["Violin", "Viola", "Cello", "Contrabass", "Tremolo Strings", "Pizzicato Strings", "Orchestral Harp", "Timpani"],
  ["String Ensemble 1", "String Ensemble 2", "Synth Strings 1", "Synth Strings 2", "Choir Aahs", "Voice Oohs", "Synth Voice", "Orchestra Hit"],
  ["Trumpet", "Trombone", "Tuba", "Muted Trumpet", "French Horn", "Brass Section", "Synth Brass 1", "Synth Brass 2"],
  ["Soprano Sax", "Alto Sax", "Tenor Sax", "Baritone Sax", "Oboe", "English Horn", "Bassoon", "Clarinet"],
  ["Piccolo", "Flute", "Recorder", "Pan Flute", "Blown Bottle", "Shakuhachi", "Whistle", "Ocarina"],
  ["Square Lead", "Saw Lead", "Calliope Lead", "Chiff Lead", "Charang", "Voice Lead", "Fifths Lead", "Bass & Lead"],
  ["New Age Pad", "Warm Pad", "Polysynth", "Choir Pad", "Bowed Pad", "Metallic Pad", "Halo Pad", "Sweep Pad"],
  ["Rain", "Soundtrack", "Crystal", "Atmosphere", "Brightness", "Goblins", "Echoes", "Sci-Fi"],
  ["Sitar", "Banjo", "Shamisen", "Koto", "Kalimba", "Bagpipe", "Fiddle", "Shanai"],
  ["Tinkle Bell", "Agogo", "Steel Drums", "Woodblock", "Taiko Drum", "Melodic Tom", "Synth Drum", "Reverse Cymbal"],
  ["Fret Noise", "Breath Noise", "Seashore", "Bird Tweet", "Telephone", "Helicopter", "Applause", "Gunshot"],
];

/** Nine kits, as the mock counts them ("ドラムのキット 9"). */
const KITS: [number, string][] = [
  [0, "Standard"],
  [8, "Room"],
  [16, "Power"],
  [24, "Electronic"],
  [25, "TR-808"],
  [32, "Jazz"],
  [40, "Brush"],
  [48, "Orchestra"],
  [56, "SFX"],
];

/** 128 presets in bank 0 and 9 drum kits: "FluidR3_GM.sf2 SF2 137". */
export function fluidFile(): Sf2FileView {
  const presets: Sf2PresetView[] = GM_NAMES.flatMap((names, f) =>
    names.map((name, i) => ({ bank: 0, program: f * 8 + i, name, family: GM_FAMILIES[f]! })),
  );
  for (const [program, name] of KITS) presets.push({ bank: SF2_DRUM_BANK, program, name, family: "drumKit" });
  return { file: FLUID, presets };
}

const slug = (name: string) => name.toLowerCase().replace(/[^a-z0-9]+/g, "-");
/** Keys in the shapes of the real ones (20-5); the ids are made from the names. */
export const lv2Key = (name: string) => `lv2:urn:fake:${slug(name)}`;
export function vst3Key(name: string): string {
  let hex = "";
  for (let seed = 0; seed < 4; seed++) {
    let h = 2166136261 ^ seed;
    for (const c of name) h = Math.imul(h ^ c.charCodeAt(0), 16777619);
    hex += (h >>> 0).toString(16).toUpperCase().padStart(8, "0");
  }
  return `vst3:${hex}`;
}

function row(name: string, vendor: string, kind: PluginKindView, formats: PluginFormat[], used: PluginFormat = formats[0]!): PluginRowView {
  const keyOf = (f: PluginFormat) => (f === "lv2" ? lv2Key(name) : vst3Key(name));
  return { name, vendor, kind, key: keyOf(used), formats: formats.map((format) => ({ format, key: keyOf(format) })) };
}

const BOTH: PluginFormat[] = ["vst3", "lv2"];
const fx = (category: PluginEffectCategory): PluginKindView => ({ role: "effect", category });
const inst = (category: PluginInstrumentCategory): PluginKindView => ({ role: "instrument", category });

/** The rows the mocks show. Where a mock has "＋2" (LV2, VST3 and CLAP), the row here has two formats: meloet reads no CLAP. */
const SHOWN: PluginRowView[] = [
  row("Calf Reverb", "Calf", fx("reverb"), ["lv2"]),
  row("Dragonfly Early Reflections", "Michael Willis", fx("reverb"), BOTH, "lv2"),
  row("Dragonfly Hall Reverb", "Michael Willis", fx("reverb"), BOTH, "lv2"),
  row("Dragonfly Plate Reverb", "Michael Willis", fx("reverb"), BOTH, "lv2"),
  row("Dragonfly Room Reverb", "Michael Willis", fx("reverb"), BOTH, "lv2"),
  row("LSP Room Builder", "LSP", fx("reverb"), ["lv2"]),
  row("MVerb", "DISTRHO", fx("reverb"), ["lv2"]),
  row("ZamVerb", "Damien Zammit", fx("reverb"), ["lv2"]),
  // The stopped insert of "3 ドラム" in F5-ShelfFx.
  row("Brick EQ", "Example Audio", fx("eq"), ["lv2"]),
  row("Dexed", "Digital Suburban", inst("synth"), BOTH, "lv2"),
  row("Helm", "Matt Tytel", inst("synth"), ["lv2"]),
  row("Odin 2", "TheWaveWarden", inst("synth"), BOTH, "lv2"),
  row("Surge XT", "Surge Synth Team", inst("synth"), BOTH, "lv2"),
  row("Vital", "Vital Audio", inst("synth"), BOTH, "vst3"),
];

/** How many rows each heading has in F5-ShelfFx (506 effects) and F5-PluginLoading (34 instruments). */
const EFFECT_COUNTS: Record<PluginEffectCategory, number> = {
  eq: 64,
  dynamics: 88,
  reverb: 23,
  delay: 31,
  modulation: 52,
  distortion: 40,
  filter: 36,
  pitch: 9,
  spatial: 14,
  analysis: 70,
  other: 79,
};
const INSTRUMENT_COUNTS: Record<PluginInstrumentCategory, number> = { synth: 21, sampler: 6, drum: 4, keys: 2, other: 1 };

const title = (id: string) => id[0]!.toUpperCase() + id.slice(1);

/** The shown rows and "Example …" rows up to the counts, in the order of their names. */
export function fakePluginRows(): PluginRowView[] {
  const rows = [...SHOWN];
  const fill = (kind: PluginKindView, count: number) => {
    const same = (k: PluginKindView) => k.role === kind.role && k.category === kind.category;
    const have = SHOWN.filter((r) => same(r.kind)).length;
    const word = kind.role === "effect" ? title(kind.category) : `${title(kind.category)} Instrument`;
    for (let n = 1; n <= count - have; n++) rows.push(row(`Example ${word} ${String(n).padStart(2, "0")}`, "Example Audio", kind, n % 3 === 0 ? BOTH : ["lv2"]));
  };
  for (const c of PLUGIN_EFFECT_CATEGORIES) fill(fx(c), EFFECT_COUNTS[c]);
  for (const c of PLUGIN_INSTRUMENT_CATEGORIES) fill(inst(c), INSTRUMENT_COUNTS[c]);
  const fold = (r: PluginRowView) => r.name.toLowerCase();
  return rows.sort((a, b) => (fold(a) < fold(b) ? -1 : fold(a) > fold(b) ? 1 : a.vendor < b.vendor ? -1 : a.vendor > b.vendor ? 1 : 0));
}

/** Nothing added: the shelf of a meloet that was just installed. */
export const emptyLibrary = (): LibraryView => ({ sf2: [], plugins: [], openedSf2: null, packMissing: false });

/** F5-ShelfFx, F5-PluginLoading: FluidR3 and 540 plugins. */
export const fullLibrary = (): LibraryView => ({ sf2: [fluidFile()], plugins: fakePluginRows(), openedSf2: null, packMissing: false });

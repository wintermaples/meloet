// Values. The names are those of docs/spec/1-tokens.md ("1 List of values"). Values are px at scale 100 %.

export const SCALES = [100, 125] as const;
export type Scale = (typeof SCALES)[number];
type Pair = readonly [number, number];

export const colors = {
  "color.gray.0": "#000000",
  "color.gray.1": "#1A1A1A",
  "color.gray.2": "#212121",
  "color.gray.3": "#282828",
  "color.gray.4": "#2F2F2F",
  "color.gray.5": "#3C3C3C",
  "color.gray.6": "#686868",
  "color.gray.7": "#717171",
  "color.gray.8": "#9A9A9A",
  "color.gray.9": "#D6D6D6",
  "color.gray.10": "#F2F2F2",
  "color.accent": "#FFB000",
  "color.rec": "#FF6158",
  "color.meter.ok": "#4CD964",
  "color.meter.hot": "#FFD60A",
  "color.meter.ground": "#000000",
  "color.meter.ground.mic": "#2F2F2F",
  "color.meter.clip": "#FF6158",
  "color.meter.peak": "#F2F2F2",
  "color.track.1": "#69E4E4",
  "color.track.2": "#5ABDF2",
  "color.track.3": "#6F8BED",
  "color.track.4": "#9561C7",
  "color.track.5": "#FFD9F1",
  "color.tip.bg": "#F2F2F2",
  "color.tip.fg": "#000000",
  "color.key.white": "#3C3C3C",
  "color.key.black": "#000000",
  "color.key.meter.white": "#282828",
  "color.key.meter.black": "#000000",
  "color.row.white": "#212121",
  "color.row.black": "#1A1A1A",
  "color.row.out": "#000000",
  "color.line.bar": "#6B6B6B",
  "color.line.beat": "#3E3E3E",
  "color.line.grid": "#2F2F2F",
  "color.line.thin": "#4A4A4A",
  "color.line.row": "#353535",
} as const;
export type ColorName = keyof typeof colors;

/** weights: the weights that may be used at that size (the same at every scale). */
export interface TextToken {
  size: Pair;
  lineHeight: Pair;
  weights: readonly number[];
}

export const text = {
  "text.s": { size: [12, 15], lineHeight: [16, 20], weights: [400, 500, 600] },
  "text.m": { size: [16, 20], lineHeight: [20, 25], weights: [400, 500] },
  "text.l": { size: [20, 25], lineHeight: [24, 30], weights: [600] },
} as const satisfies Record<string, TextToken>;
export type TextName = keyof typeof text;

export const space = {
  "space.1": [4, 5],
  "space.2": [8, 10],
  "space.3": [12, 15],
  "space.4": [16, 20],
  "space.5": [20, 25],
  "space.6": [24, 30],
} as const satisfies Record<string, Pair>;
export type SpaceName = keyof typeof space;

export const size = {
  "size.menubar": [24, 30],
  "size.transport": [48, 60],
  "size.meter.full": [52, 65],
  "size.meter.slim": [24, 30],
  "size.meter.name": [108, 135],
  "size.meter.name.slim": [176, 220],
  "size.bandrow": [24, 30],
  "size.status": [24, 30],
  "size.row": [24, 30],
  "size.row.s": [20, 25],
  "size.note": [12, 15],
  "size.toolbar": [32, 40],
  "size.track": [64, 80],
  "size.lane": [48, 60],
  "size.lane.vel": [72, 90],
  "size.overview": [40, 50],
  "size.col.head": [220, 275],
  "size.col.keys": [104, 130],
  "size.panel.list": [216, 270],
  "size.panel.insp": [240, 300],
  "size.shelf": [320, 400],
  "size.tab.v": [24, 30],
  "size.strip": [108, 135],
} as const satisfies Record<string, Pair>;
export type SizeName = keyof typeof size;

/** line.hair is one physical px at every scale. Only the 100 % value is kept here; it is divided by devicePixelRatio when made into CSS px. */
export const line = {
  "line.hair": 1,
  "line.mark": [2, 2],
  "line.track": [4, 5],
} as const;
export type LineName = keyof typeof line;

/** radius.1 stays 1 at every scale. radius.round is a ratio. */
export const radius = {
  "radius.0": 0,
  "radius.1": 1,
  "radius.round": "50%",
} as const;
export type RadiusName = keyof typeof radius;

export const fontFamily = {
  sans: "'IBM Plex Sans JP','Noto Sans JP',sans-serif",
  mono: "'IBM Plex Mono',ui-monospace,Menlo,monospace",
} as const;

/**
 * Sizes that are not in the spec tables, only in its text (px at 100 %).
 * spec: which chapter (s1 / s2), which row (the left cell of the table), and which words. A test checks them against the spec.
 * Those with section "sample" are values in the CSS of the spec 2 samples (parts drawn at real size).
 */
type PartSection = "s1" | "s2" | "s3" | "sample";
export interface PartToken {
  value: number;
  /** When row is null, the value is in the text of the chapter (for sample, in the sample CSS), not in a table. */
  spec: { section: PartSection; row: string | null; quote: string };
}
const p = (value: number, section: PartSection, row: string | null, quote: string): PartToken => ({
  value,
  spec: { section, row, quote },
});

export const part = {
  "part.button.height": p(24, "s2", "Button", "Height 24"),
  "part.button.pad-x": p(8, "s2", "Button", "padding 8 on left and right"),
  "part.button.icon": p(10, "s2", "Button", "10px icon"),
  "part.toggle.size": p(20, "s2", "Toggle M, S, ●", "20×20"),
  "part.insert.height": p(24, "s2", "Insert slot", "Height 24"),
  "part.bandtab.height": p(24, "s2", "Band tab (folded heading)", "Height 24"),
  "part.listrow.height": p(24, "s2", "List row", "Height 20 or 24"),
  "part.listrow.height-s": p(20, "s2", "List row", "Height 20 or 24"),
  "part.input.height": p(24, "s2", "Input box", "Height 24"),
  "part.input.pad-x": p(8, "sample", null, ".in{height:24px;padding:0 8px;"),
  "part.fader.knob-w": p(24, "s2", "Fader", "Handle 24×12"),
  "part.fader.knob-h": p(12, "s2", "Fader", "Handle 24×12"),
  "part.pan.size": p(24, "s2", "Pan knob", "diameter 24"),
  "part.rec.dot": p(12, "s2", "Record button", "diameter 12"),
  "part.meter.horizontal": p(4, "s2", "Meter", "Horizontal 4px"),
  "part.meter.vertical": p(6, "s2", "Meter", "2 bars 6px wide"),
  "part.meter.clip": p(2, "s2", "Meter", "meter.clip 2px"),
  "part.meter.mic": p(200, "s2", "Input meter band (split left and right)", "a 200 meter"),
  "part.note.height": p(11, "s2", "Note", "Height 11"),
  "part.note.edge": p(2, "s2", "Note", "2px gray.10"),
  "part.tip.width": p(248, "s2", "Term tip", "Width 248"),
  "part.tip.pad": p(8, "s2", "Term tip", "padding 8"),
  "part.choice.size": p(12, "s2", "Choice mark (in settings)", "12×12"),
  "part.choice.dot": p(6, "s2", "Choice mark (in settings)", "6×6"),
  "part.shelfrow.height": p(24, "s2", "Instrument row (shelf)", "Height 24."),
  "part.shelfrow.star": p(12, "s2", "Instrument row (shelf)", "12px star"),
  "part.shelfrow.sample": p(20, "s2", "Instrument row (shelf)", "sample button (20×20)"),
  "part.shelfrow.group": p(24, "s2", "Instrument row (shelf)", "Group headings are 24 high"),
  "part.shelfrow.vendor": p(96, "s2", "Plugin row in the shelf", "The maker is at most 96"),
  "part.filedrop.width": p(400, "s2", "Notice while a file is dragged in", "400 wide"),
  "part.filedrop.half": p(240, "s2", "Split box while dragging in", "240 each"),
  "part.colorlist.swatch-w": p(24, "s2", "Color list row", "a 24×12 color sample"),
  "part.colorlist.swatch-h": p(12, "s2", "Color list row", "a 24×12 color sample"),
  "part.endpoint.name": p(168, "s2", "Output and input row", "real name (168)"),
  "part.endpoint.friendly": p(150, "s2", "Output and input row", "friendly name (150"),
  "part.endpoint.link": p(6, "s2", "Output and input row", "plugged-in mark (6×6"),
  "part.endpoint.height-connect": p(32, "s2", "Output and input row", "In flow 2, 32 high"),
  "part.endpoint.height-settings": p(24, "s2", "Output and input row", "in sound settings, 24 high"),
  "part.param.height": p(24, "s2", "Parameter row", "Height 24"),
  "part.param.name": p(104, "s2", "Parameter row", "name (104"),
  "part.param.control": p(88, "s2", "Parameter row", "control (88)"),
  "part.param.value": p(68, "s2", "Parameter row", "value (68"),
  "part.param.automation": p(20, "s2", "Parameter row", "mark for writing automation (20×20)"),
  "part.param.knob-w": p(6, "s2", "Parameter row", "handle 6×12"),
  "part.param.knob-h": p(12, "s2", "Parameter row", "handle 6×12"),
  "part.param.step": p(6, "s2", "Parameter row", "a row of 6×6 squares"),
  "part.param.switch": p(12, "s2", "Parameter row", "a 12×12 square mark"),
  "part.paramgroup.height": p(24, "s2", "Parameter group heading", "Height 24"),
  "part.windowmark.size": p(10, "s2", "Window-open mark", "10×10"),
  "part.windowlist.width": p(380, "s2", "「窓」 in the status line", "width 380"),
  "part.windowlist.row": p(24, "s2", "「窓」 in the status line", "rows are 24 high"),
  "part.zoomlist.width": p(320, "s2", "「拡大率」 in the status line", "width 320"),
  "part.zoomlist.row": p(24, "s2", "「拡大率」 in the status line", "rows are 24 high"),
  "part.list.width": p(220, "s2", "List", "Width 220"),
  "part.list.row": p(24, "s2", "List", "Rows are 24 high"),
  "part.songmenu.width": p(28, "s2", "Song ▾", "28×24"),
  "part.metronomemenu.width": p(20, "s2", "Metronome ▾", "20 wide"),
  // The beat squares on the button during the count-in, and the space between them.
  "part.countin.beat": p(8, "s3", null, "8×8 squares (space 4)"),
  "part.countin.gap": p(4, "s3", null, "8×8 squares (space 4)"),
  "part.dialog.width": p(560, "s2", "Dialog", "Width 560"),
  "part.dialog.head": p(32, "s2", "Dialog", "Heading 32"),
  "part.dialog.foot": p(48, "s2", "Dialog", "bottom row 48"),
  "part.dialog.margin": p(24, "s2", "Dialog", "24 above and below"),
  "part.scrollbar.width": p(8, "s2", "Scroll bar", "Width 8"),
  "part.dialognext.height": p(32, "s2", "Row to the second page of a dialog", "Height 32"),
  "part.unusable.height": p(24, "s2", "Output row that cannot be chosen", "Height 24"),
  "part.meter.name.long": p(148, "s1", "size.meter.name", "148 for a long name"),
  "part.meter.name.slim-narrow": p(128, "s1", "size.meter.name.slim", "128 in a narrow window"),
  "part.meter.name.split-narrow": p(88, "s3", null, "the keyboard name (cut with 「…」 at 88)"),
  "part.track.vocal": p(160, "s1", "size.track", "160 when editing a vocal"),
  "part.audioclip.handle": p(6, "s2", "Audio clip", "6×6"),
  "part.strip.master": p(124, "s1", "size.strip", "124 for the master"),
  "part.take.mark": p(6, "s2", "Take mark and switch", "a 6×6 gray.10 square"),
  "part.take.tag-min-width": p(28, "s2", "Take mark and switch", "the clip is narrower than 28"),
  "part.take.switch-height": p(24, "s2", "Take mark and switch", "switch tag (height 24"),
  "part.meter.black-key-slim-w": p(5, "s3", null, "draws black keys (width 5, height 7)"),
  "part.meter.black-key-slim-h": p(7, "s3", null, "draws black keys (width 5, height 7)"),
  // The piano roll band
  "part.reading.height": p(20, "s2", "Reading in place", "Height 20"),
  "part.reading.pad-x": p(6, "s2", "Reading in place", "6 on left and right"),
  "part.reading.offset": p(6, "s2", "Reading in place", "6 away to the upper left of the cursor"),
  "part.cursor.size": p(16, "s2", "Cursor (pencil, eraser)", "16×16"),
  "part.note.resize": p(6, "s3", null, "the right 6px of a note"),
  "part.cc.point": p(6, "s3", null, "Points are 6×6 gray.10"),
  "part.clip.edge": p(6, "s3", null, "The 6px at the left and right ends"),
  // The drum piano roll: the width of the key column, the hit squares (width one grid cell − 2, at least 6; height row − 8), the note list panel.
  "part.drum.keys": p(132, "s3", null, "The key column is 132 wide"),
  "part.drum.hit-inset": p(2, "s2", "Drum hit square", "one grid cell − 2"),
  "part.drum.hit-min": p(6, "s2", "Drum hit square", "at least 6"),
  "part.drum.hit-gap": p(8, "s2", "Drum hit square", "the row height − 8"),
  "part.drum.list-position": p(68, "s3", null, "「位置」 68"),
  "part.drum.list-velocity": p(40, "s3", null, "Vel 40"),
  // Export: the length of the progress bar on the left of the status line.
  "part.exporting.bar": p(160, "s2", "Export progress", "a 160 bar"),
  // The bar in the input meter band while the sound pack is being fetched.
  "part.packband.bar": p(160, "s2", "Hint band (sound pack)", "a 160 bar"),
} as const satisfies Record<string, PartToken>;
export type PartName = keyof typeof part;

/** Sizes that only the mocks have (px at 100 %). board is the mock the value was read from (docs/design/mock/<board>.dc.html). */
export interface MockToken {
  value: number;
  board: string;
}
const m = (value: number, board: string): MockToken => ({ value, board });

export const mock = {
  // Input meter, output chooser, sound settings, 「音が出ました」
  // The keyboard box is (white key width × number of white keys + line.hair for the left edge line) wide.
  "mock.input-meter.key-full-w": m(12, "F-Connect3"),
  "mock.input-meter.keys-full-h": m(32, "F-Connect3"),
  "mock.input-meter.key-slim-w": m(8, "F-MeterLongName"),
  "mock.input-meter.keys-slim-h": m(12, "F-MeterLongName"),
  "mock.input-meter.black-key-w": m(7, "F-Connect3"),
  "mock.input-meter.black-key-h": m(19, "F-Connect3"),
  "mock.input-meter.black-key-narrow-w": m(5, "F-MeterLongName"),
  "mock.input-meter.velocity": m(96, "F-Connect3"),
  "mock.input-meter.velocity-bar": m(4, "F-Connect3"),
  "mock.arrange-note.left": m(24, "F-Connect2"),
  "mock.arrange-note.top": m(176, "F-Connect2"),
  "mock.chooser.width": m(640, "F-Connect2"),
  "mock.sound-hint.width": m(420, "F-Connect3"),
  "mock.recovery-hint.width": m(480, "F-Recover1"),
  "mock.settings.label": m(132, "F-Settings"),
  "mock.settings.meter": m(160, "F-Settings"),
  "mock.settings.adv-label": m(120, "F7-SettingsAdv"),
  "mock.latency.width": m(480, "F7-Latency"),
  "mock.latency.progress": m(4, "F7-Latency"),
  "mock.transport.name": m(132, "F-Empty"),
  "mock.transport.readout": m(40, "F-Empty"),
  "mock.transport.master": m(132, "F-Empty"),
  // The height of the position box and the width of the master when the position label is hidden.
  "mock.transport.readout-narrow": m(32, "F-PR150"),
  "mock.transport.master-narrow": m(100, "F-PR150"),
  "mock.arrange.bar": m(64, "F-Empty"),
  "mock.arrange.add-track": m(32, "F-Empty"),
  "mock.arrange.add-keys": m(120, "F7-VocalTrack"),
  "mock.track.number": m(16, "F-Empty"),
  // A clip sits in its row at `top:3px; bottom:3px`. Small notes are 2 high. The loop range is a 3-high accent on the top edge of the ruler.
  "mock.arrange.clip-inset": m(3, "Main"),
  "mock.arrange.note": m(2, "Main"),
  "mock.arrange.loop": m(3, "Main"),
  // The waveform of an audio clip: 1-wide bars every 3px, 2 high even where there is no sound.
  "mock.wave.pitch": m(3, "F7-AudioEdit"),
  "mock.wave.bar": m(1, "F7-AudioEdit"),
  "mock.wave.min": m(2, "F7-AudioEdit"),
  "mock.overview.label": m(104, "A-PianoRoll"),
  // The space above and below the frame of the visible range (`top:1px; height:37px`; the band is 39 high).
  "mock.range-frame.inset": m(1, "A-PianoRoll"),
  // The width of the scale column in the scale list.
  "mock.zoom.rate": m(56, "F-Zoom"),
  // The note list panel (`grid-template-columns:76px 40px 52px 1fr`, panel sides `padding:0 6px`).
  "mock.notelist.position": m(76, "A-PianoRoll"),
  "mock.notelist.key": m(40, "A-PianoRoll"),
  "mock.notelist.length": m(52, "A-PianoRoll"),
  "mock.notelist.cell-pad": m(6, "A-PianoRoll"),
  // The left indent of the selected note rows (`.rw{padding:0 12px 0 28px}`), the label and value widths of the knob bars, the fill thickness (knobs are part.param.knob-*).
  "mock.inspector.indent": m(28, "A-PianoRoll"),
  "mock.inspector.slider-label": m(60, "A-PianoRoll"),
  "mock.inspector.slider-value": m(40, "A-PianoRoll"),
  "mock.inspector.slider-fill": m(3, "A-PianoRoll"),
  // The space above and below the button row (`padding:6px 12px 6px 28px`).
  "mock.inspector.buttons-pad-y": m(6, "A-PianoRoll"),
  // The rows that change many at once (same length, change velocity together). The spec 2 numbers, written as the mock does (height 32, text width 60,
  // input box 72 wide, 20 high, 6 on each side).
  "mock.inspector.bulk-row": m(32, "A-PianoRoll"),
  "mock.inspector.bulk-label": m(60, "A-PianoRoll"),
  "mock.inspector.bulk-field": m(72, "A-PianoRoll"),
  "mock.inspector.bulk-field-h": m(20, "A-PianoRoll"),
  "mock.inspector.bulk-field-pad": m(6, "A-PianoRoll"),
  // The piano roll band: the ruler height, the note name column of the key column.
  "mock.pianoroll.ruler": m(20, "A-PianoRoll"),
  "mock.pianoroll.key-label": m(48, "A-PianoRoll"),
  // The bars of the velocity band (3 wide, a 7-wide line on top, 64 high at 127) and the middle line (36 from the top of the band).
  "mock.pianoroll.vel-bar": m(3, "A-PianoRoll"),
  "mock.pianoroll.vel-cap": m(7, "A-PianoRoll"),
  "mock.pianoroll.vel-full": m(64, "A-PianoRoll"),
  "mock.pianoroll.vel-mid": m(36, "A-PianoRoll"),
  // The height of the gray.7 line at the old end while a length is being stretched.
  "mock.pianoroll.end-mark": m(16, "R3-PRMouse"),
  // In the CC band, the distance of the point centers at 127 and 0 (both ends for pitch bend) from the band top and bottom (6×6 points at `top:6px`; 9 and 31 in a 40-high band).
  "mock.pianoroll.cc-pad": m(9, "R3-Lanes"),
  // The vertical − + in the top corner of the key column (`.q` 20 wide, `height:18px`, `gap:2px` between).
  "mock.pianoroll.row-zoom-h": m(18, "A-PianoRoll"),
  "mock.pianoroll.row-zoom-gap": m(2, "A-PianoRoll"),
  // The width of the hint box where there are no clips (`width:460px`).
  "mock.pianoroll.empty-hint": m(460, "F-PREmpty"),
  // The shelf: the height of the search row and the 「すべて ・ お気に入り」 row (`height:32px`), the area of the star you can press (`width:16px;height:16px`).
  "mock.shelf.bar": m(32, "F4-ShelfSample"),
  "mock.shelf.star-hit": m(16, "F4-ShelfSample"),
  // The indent of the heading of an SF2's sounds (`padding-left:28px`).
  "mock.shelf.indent": m(28, "F5-SF2Shelf"),
  // 「meloet について」: the text column of the first page (`width:132px`), the space above the name box (`padding:24px 16px 16px`).
  "mock.about.label": m(132, "R7-About"),
  "mock.about.name-pad-top": m(24, "R7-About"),
  // Second page: the search row (`height:40px`), the row indent (`padding-left:28px`), the text column of the sound pack (`width:120px`) and
  // the panel top and bottom (`padding:6px 0 8px`, rows `padding:2px …`), the height of the full text box (`height:72px`).
  "mock.about.search": m(40, "R7-AboutLicense"),
  "mock.about.indent": m(28, "R7-AboutLicense"),
  "mock.about.field": m(120, "R7-AboutLicense"),
  "mock.about.fields-pad-top": m(6, "R7-AboutLicense"),
  "mock.about.field-pad-y": m(2, "R7-AboutLicense"),
  "mock.about.text": m(72, "R7-AboutLicense"),
  // Scan: the progress bar in the band under the shelf (`height:2px`), the width of the dialog 「スキャンで止まったプラグイン」 (`width:640px`; spec 3 (Y13) is also 640).
  "mock.scan.progress": m(2, "R9-Scan"),
  "mock.scan-stopped.width": m(640, "F5-ScanStopped"),
  // The undo history rows: the number column (`width:24px`, right-aligned) and the text column (`width:420px`).
  "mock.history.number": m(24, "F-History"),
  "mock.history.name": m(420, "F-History"),
  // Export progress: the bar thickness in the status line (`height:4px`) and the height of 「やめる」 (`height:20px`).
  "mock.exporting.bar": m(4, "F7-Exporting"),
  "mock.exporting.stop": m(20, "F7-Exporting"),
  "mock.packband.bar": m(4, "F8-PackGet"),
} as const satisfies Record<string, MockToken>;
export type MockName = keyof typeof mock;

/**
 * Numbers that are not px (ratios, times, scale marks). Not multiplied by the scale, and not made into CSS variables.
 * spec is read the same way as for part.*.
 */
export const plain = {
  /** Where the meter color changes from ok to hot (%). */
  "meter.hot-from": { value: 70, spec: { section: "s2", row: "Meter", quote: "Up to −3 dB (70 % of the length) is meter.ok" } },
  /** How long the clipping mark stays (seconds). */
  "meter.clip-hold": { value: 2, spec: { section: "s2", row: "Meter", quote: "2 seconds" } },
} as const;

/**
 * The table in spec 2 ("Scale marks of meters and faders"): [dB, length (%; the bottom is 0)]. Loudest first.
 * Points are joined by straight lines; sound below the lowest point is 0 % (−∞).
 */
export const meterScale: readonly (readonly [number, number])[] = [
  [6, 100],
  [0, 80],
  [-3, 70],
  [-6, 60],
  [-12, 40],
  [-24, 20],
  [-60, 0],
];

/** From the text under the same spec heading: a meter falls fallDb over fallSeconds, and rises at once. */
export const meterFall = { fallDb: 20, fallSeconds: 1.7 } as const;

const scaleIndex = (scale: Scale): 0 | 1 => (scale === 100 ? 0 : 1);

/** A part size times the scale, rounded to an integer (.5 rounds up; those that round up are listed in tests/design). */
export function partAt(name: PartName, scale: Scale): number {
  return Math.round((part[name].value * scale) / 100);
}

/** A value for every color name (the colors of a theme, design/themes.ts). */
export type ColorValues = Readonly<Record<ColorName, string>>;

/** Name → value at that scale. Numbers are CSS px, colors are hex (from `palette`), weights are numbers. */
export function tokensAt(scale: Scale, devicePixelRatio: number, palette: ColorValues = colors): Record<string, number | string> {
  const i = scaleIndex(scale);
  const out: Record<string, number | string> = {};
  for (const k of Object.keys(colors) as ColorName[]) out[k] = palette[k];
  for (const [k, v] of Object.entries(text) as [TextName, TextToken][]) {
    out[`${k}.size`] = v.size[i];
    out[`${k}.line-height`] = v.lineHeight[i];
    for (const w of v.weights) out[`${k}.weight-${w}`] = w;
  }
  for (const [k, v] of Object.entries(space)) out[k] = v[i];
  for (const [k, v] of Object.entries(size)) out[k] = v[i];
  out["line.hair"] = line["line.hair"] / devicePixelRatio;
  out["line.mark"] = line["line.mark"][i];
  out["line.track"] = line["line.track"][i];
  for (const [k, v] of Object.entries(radius)) out[k] = v;
  for (const k of Object.keys(part) as PartName[]) out[k] = partAt(k, scale);
  // As for part.*, round when the scaled value is not an integer.
  for (const [k, v] of Object.entries(mock)) out[k] = Math.round((v.value * scale) / 100);
  return out;
}

const unitless = (name: string) => name.includes(".weight");

/** CSS variable pairs such as `--color-gray-4: #222222`. */
export function cssVariables(scale: Scale, devicePixelRatio: number, palette: ColorValues = colors): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(tokensAt(scale, devicePixelRatio, palette))) {
    out[`--${k.replaceAll(".", "-")}`] = typeof v === "string" ? v : unitless(k) ? String(v) : `${v}px`;
  }
  out["--font-sans"] = fontFamily.sans;
  out["--font-mono"] = fontFamily.mono;
  return out;
}

/** Writes the CSS variables on :root (or the given element). Call again when the scale, devicePixelRatio or theme changes. */
export function writeCssVariables(
  scale: Scale,
  devicePixelRatio: number = window.devicePixelRatio || 1,
  palette: ColorValues = colors,
  root: HTMLElement = document.documentElement,
): void {
  for (const [k, v] of Object.entries(cssVariables(scale, devicePixelRatio, palette))) root.style.setProperty(k, v);
}

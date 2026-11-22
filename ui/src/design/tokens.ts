// 数値。名前は docs/spec/1-tokens.md の「1 List of values」のまま。値は拡大率 100 % のときの px。

export const SCALES = [100, 125, 150] as const;
export type Scale = (typeof SCALES)[number];
type Triple = readonly [number, number, number];

export const colors = {
  "color.gray.0": "#0A0A0A",
  "color.gray.1": "#121212",
  "color.gray.2": "#161616",
  "color.gray.3": "#1A1A1A",
  "color.gray.4": "#222222",
  "color.gray.5": "#2C2C2C",
  "color.gray.6": "#3A3A3A",
  "color.gray.7": "#5E5E5E",
  "color.gray.8": "#9A9A9A",
  "color.gray.9": "#D6D6D6",
  "color.gray.10": "#F2F2F2",
  "color.accent": "#FFB000",
  "color.rec": "#FF3B30",
  "color.meter.ok": "#4CD964",
  "color.meter.hot": "#FFD60A",
  "color.track.1": "#69E4E4",
  "color.track.2": "#5ABDF2",
  "color.track.3": "#6F8BED",
  "color.track.4": "#8C54C3",
  "color.track.5": "#FFD9F1",
  "color.tip.bg": "#F2F2F2",
  "color.tip.fg": "#0A0A0A",
} as const;
export type ColorName = keyof typeof colors;

/** weights: その大きさで使ってよい太さの組（率によらない）。 */
export interface TextToken {
  size: Triple;
  lineHeight: Triple;
  weights: readonly number[];
}

export const text = {
  "text.s": { size: [12, 15, 18], lineHeight: [16, 20, 24], weights: [400, 500, 600] },
  "text.m": { size: [16, 20, 24], lineHeight: [20, 25, 30], weights: [400, 500] },
  "text.l": { size: [20, 25, 30], lineHeight: [24, 30, 36], weights: [600] },
} as const satisfies Record<string, TextToken>;
export type TextName = keyof typeof text;

export const space = {
  "space.1": [4, 5, 6],
  "space.2": [8, 10, 12],
  "space.3": [12, 15, 18],
  "space.4": [16, 20, 24],
  "space.5": [20, 25, 30],
  "space.6": [24, 30, 36],
} as const satisfies Record<string, Triple>;
export type SpaceName = keyof typeof space;

export const size = {
  "size.menubar": [24, 30, 36],
  "size.transport": [48, 60, 72],
  "size.meter.full": [52, 65, 78],
  "size.meter.slim": [24, 30, 36],
  "size.meter.name": [108, 135, 162],
  "size.meter.name.slim": [176, 220, 264],
  "size.bandrow": [24, 30, 36],
  "size.status": [24, 30, 36],
  "size.row": [24, 30, 36],
  "size.row.s": [20, 25, 30],
  "size.note": [12, 15, 18],
  "size.toolbar": [32, 40, 48],
  "size.track": [64, 80, 96],
  "size.lane": [48, 60, 72],
  "size.lane.vel": [72, 90, 108],
  "size.overview": [40, 50, 60],
  "size.col.head": [220, 275, 330],
  "size.col.keys": [104, 130, 156],
  "size.panel.list": [216, 270, 324],
  "size.panel.insp": [240, 300, 360],
  "size.shelf": [320, 400, 480],
  "size.tab.v": [24, 30, 36],
  "size.strip": [108, 135, 162],
} as const satisfies Record<string, Triple>;
export type SizeName = keyof typeof size;

/** line.hair は率によらず物理 1px。ここでは 100 % の値だけを持ち、CSS px にするときに devicePixelRatio で割る。 */
export const line = {
  "line.hair": 1,
  "line.mark": [2, 2, 3],
  "line.track": [4, 5, 6],
} as const;
export type LineName = keyof typeof line;

/** radius.1 は拡大しても 1。radius.round は割合。 */
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
 * 仕様書の表にない、文の中にしかない寸法（100 % の px）。
 * spec: どの章（s1 / s2）の、どの行（表の左はしのセル）の、どの言葉か。テストが仕様書と照らし合わせる。
 * section が "sample" のものは、仕様書 2 章の見本（実物の大きさの部品）の CSS にある値。
 */
type PartSection = "s1" | "s2" | "s3" | "sample";
export interface PartToken {
  value: number;
  /** row が null なら、表ではなく、その章の文（sample なら見本の CSS）の中にある。 */
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
  "part.meter.clip": p(2, "s2", "Meter", "rec 2px"),
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
  // カウントイン中のボタンの拍の四角と、そのすきま。
  "part.countin.beat": p(8, "s3", null, "8×8 の四角（すきま 4）"),
  "part.countin.gap": p(4, "s3", null, "8×8 の四角（すきま 4）"),
  "part.dialog.width": p(560, "s2", "Dialog", "Width 560"),
  "part.dialog.head": p(32, "s2", "Dialog", "Heading 32"),
  "part.dialog.foot": p(48, "s2", "Dialog", "bottom row 48"),
  "part.dialog.margin": p(24, "s2", "Dialog", "24 above and below"),
  "part.scrollbar.width": p(8, "s2", "Scroll bar", "Width 8"),
  "part.dialognext.height": p(32, "s2", "Row to the second page of a dialog", "Height 32"),
  "part.unusable.height": p(24, "s2", "Output row that cannot be chosen", "Height 24"),
  "part.meter.name.long": p(148, "s1", "size.meter.name", "148 for a long name"),
  "part.meter.name.slim-narrow": p(128, "s1", "size.meter.name.slim", "128 in a narrow window"),
  "part.meter.name.split-narrow": p(88, "s3", null, "鍵盤の名前（88 で「…」）"),
  "part.track.vocal": p(160, "s1", "size.track", "160 when editing a vocal"),
  "part.audioclip.handle": p(6, "s2", "Audio clip", "6×6"),
  "part.strip.master": p(124, "s1", "size.strip", "124 for the master"),
  "part.take.mark": p(6, "s2", "Take mark and switch", "a 6×6 gray.10 square"),
  "part.take.tag-min-width": p(28, "s2", "Take mark and switch", "the clip is narrower than 28"),
  "part.take.switch-height": p(24, "s2", "Take mark and switch", "switch tag (height 24"),
  "part.meter.black-key-slim-w": p(5, "s3", null, "黒鍵を描く（幅 5、高さ 7）"),
  "part.meter.black-key-slim-h": p(7, "s3", null, "黒鍵を描く（幅 5、高さ 7）"),
  // ピアノロールの帯（第 3 回）
  "part.reading.height": p(20, "s2", "Reading in place", "Height 20"),
  "part.reading.pad-x": p(6, "s2", "Reading in place", "6 on left and right"),
  "part.reading.offset": p(6, "s2", "Reading in place", "6 away to the upper left of the cursor"),
  "part.cursor.size": p(16, "s2", "Cursor (pencil, eraser)", "16×16"),
  "part.note.resize": p(6, "s3", null, "ノートの右はし 6px"),
  "part.cc.point": p(6, "s3", null, "点は 6×6 の gray.10"),
  "part.clip.edge": p(6, "s3", null, "左右のはし 6px"),
  // ドラムのピアノロール（第 4 回）: 鍵の列の幅、叩いた所の四角（幅はグリッドの 1 マス − 2、最小 6。高さは行 − 8）、ノートの一覧の欄。
  "part.drum.keys": p(132, "s3", null, "鍵の列は幅 132"),
  "part.drum.hit-inset": p(2, "s2", "Drum hit square", "one grid cell − 2"),
  "part.drum.hit-min": p(6, "s2", "Drum hit square", "at least 6"),
  "part.drum.hit-gap": p(8, "s2", "Drum hit square", "the row height − 8"),
  "part.drum.list-position": p(68, "s3", null, "位置 68"),
  "part.drum.list-velocity": p(40, "s3", null, "Vel 40"),
  // 書き出し（第 7 回）。状態行の左の進みの棒の長さ。
  "part.exporting.bar": p(160, "s2", "Export progress", "a 160 bar"),
  // 音の素材を取っている間の、入力計の帯の棒（第 8 回）。
  "part.packband.bar": p(160, "s2", "Hint band (sound pack)", "a 160 bar"),
} as const satisfies Record<string, PartToken>;
export type PartName = keyof typeof part;

/** モックにしかない寸法（100 % の px）。board は値を読んだモック（docs/design/mock/<board>.dc.html）。 */
export interface MockToken {
  value: number;
  board: string;
}
const m = (value: number, board: string): MockToken => ({ value, board });

export const mock = {
  // 入力計 ・ 出口えらび ・ 音の設定 ・ 「音が出ました」
  // 鍵盤の箱の幅は「白鍵の幅 × 白鍵の数 ＋ line.hair（左はしの線）」。
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
  // 位置のラベルを引っこめたときの、位置の枠の高さとマスターの幅。
  "mock.transport.readout-narrow": m(32, "F-PR150"),
  "mock.transport.master-narrow": m(100, "F-PR150"),
  "mock.arrange.bar": m(64, "F-Empty"),
  "mock.arrange.add-track": m(32, "F-Empty"),
  "mock.arrange.add-keys": m(120, "F7-VocalTrack"),
  "mock.track.number": m(16, "F-Empty"),
  // クリップは行の中で `top:3px; bottom:3px`。小さなノートは高さ 2。ループの範囲は目盛の上辺に高さ 3 の accent。
  "mock.arrange.clip-inset": m(3, "Main"),
  "mock.arrange.note": m(2, "Main"),
  "mock.arrange.loop": m(3, "Main"),
  // 歌のクリップの波形: 幅 1 の棒が 3px おき、音のない所でも高さ 2。
  "mock.wave.pitch": m(3, "F7-AudioEdit"),
  "mock.wave.bar": m(1, "F7-AudioEdit"),
  "mock.wave.min": m(2, "F7-AudioEdit"),
  "mock.overview.label": m(104, "A-PianoRoll"),
  // 見えている範囲の枠（`top:1px; height:37px`。帯の高さは 39）の、上下のあき。
  "mock.range-frame.inset": m(1, "A-PianoRoll"),
  // 拡大率の一覧の、率の欄の幅。
  "mock.zoom.rate": m(56, "F-Zoom"),
  // ノートの一覧の欄（`grid-template-columns:76px 40px 52px 1fr`、欄の左右 `padding:0 6px`）。
  "mock.notelist.position": m(76, "A-PianoRoll"),
  "mock.notelist.key": m(40, "A-PianoRoll"),
  "mock.notelist.length": m(52, "A-PianoRoll"),
  "mock.notelist.cell-pad": m(6, "A-PianoRoll"),
  // 選んだノートの行の左の字下げ（`.rw{padding:0 12px 0 28px}`）、つまみの棒のラベルと値の幅、ぬりの太さ（つまみは part.param.knob-*）。
  "mock.inspector.indent": m(28, "A-PianoRoll"),
  "mock.inspector.slider-label": m(60, "A-PianoRoll"),
  "mock.inspector.slider-value": m(40, "A-PianoRoll"),
  "mock.inspector.slider-fill": m(3, "A-PianoRoll"),
  // ボタンの列の上下のあき（`padding:6px 12px 6px 28px`）。
  "mock.inspector.buttons-pad-y": m(6, "A-PianoRoll"),
  // まとめて変える行（長さをそろえる ・ ベロシティをまとめて変える）。仕様書 2 章の数を、モックの書きかたで（高さ 32、字の幅 60、
  // 入力の枠は幅 72 ・ 高さ 20 ・ 左右 6）。
  "mock.inspector.bulk-row": m(32, "A-PianoRoll"),
  "mock.inspector.bulk-label": m(60, "A-PianoRoll"),
  "mock.inspector.bulk-field": m(72, "A-PianoRoll"),
  "mock.inspector.bulk-field-h": m(20, "A-PianoRoll"),
  "mock.inspector.bulk-field-pad": m(6, "A-PianoRoll"),
  // ピアノロールの帯（第 3 回）。目盛の高さ、鍵盤の列の音の名前の欄。
  "mock.pianoroll.ruler": m(20, "A-PianoRoll"),
  "mock.pianoroll.key-label": m(48, "A-PianoRoll"),
  // ベロシティの帯の棒（幅 3、上はしの横線 7、127 の高さ 64）と、まん中の線（帯の上から 36）。
  "mock.pianoroll.vel-bar": m(3, "A-PianoRoll"),
  "mock.pianoroll.vel-cap": m(7, "A-PianoRoll"),
  "mock.pianoroll.vel-full": m(64, "A-PianoRoll"),
  "mock.pianoroll.vel-mid": m(36, "A-PianoRoll"),
  // 長さをのばしている間の、もとの終わりの gray.7 の線の高さ。
  "mock.pianoroll.end-mark": m(16, "R3-PRMouse"),
  // CC の帯の、127 と 0（ピッチベンドは両はし）の点の中心の、帯の上下からのあき（`top:6px` の 6×6 の点、高さ 40 の帯で 9 と 31）。
  "mock.pianoroll.cc-pad": m(9, "R3-Lanes"),
  // 鍵盤の列の上の角の、縦の − ＋（`.q` の幅 20 に `height:18px`、間 `gap:2px`）。
  "mock.pianoroll.row-zoom-h": m(18, "A-PianoRoll"),
  "mock.pianoroll.row-zoom-gap": m(2, "A-PianoRoll"),
  // クリップがない所の案内の箱の幅（`width:460px`）。
  "mock.pianoroll.empty-hint": m(460, "F-PREmpty"),
  // 棚（第 4 回）。さがす枠の行と「すべて ・ お気に入り」の行の高さ（`height:32px`）、星を押せる所（`width:16px;height:16px`）。
  "mock.shelf.bar": m(32, "F4-ShelfSample"),
  "mock.shelf.star-hit": m(16, "F4-ShelfSample"),
  // 第 5 回。SF2 の仲間の見出しの字下げ（`padding-left:28px`）。
  "mock.shelf.indent": m(28, "F5-SF2Shelf"),
  // meloet について（第 4 回）。1 段目の字の欄（`width:132px`）、名前の箱の上のあき（`padding:24px 16px 16px`）。
  "mock.about.label": m(132, "R7-About"),
  "mock.about.name-pad-top": m(24, "R7-About"),
  // 2 段目: さがす枠の行（`height:40px`）、行の字下げ（`padding-left:28px`）、音の素材の字の欄（`width:120px`）と
  // 欄の上下（`padding:6px 0 8px`、行は `padding:2px …`）、全文の枠の高さ（`height:72px`）。
  "mock.about.search": m(40, "R7-AboutLicense"),
  "mock.about.indent": m(28, "R7-AboutLicense"),
  "mock.about.field": m(120, "R7-AboutLicense"),
  "mock.about.fields-pad-top": m(6, "R7-AboutLicense"),
  "mock.about.field-pad-y": m(2, "R7-AboutLicense"),
  "mock.about.text": m(72, "R7-AboutLicense"),
  // スキャン（第 5 回）。棚の下の帯の進みの棒（`height:2px`）、ダイアログ「スキャンで止まったプラグイン」の幅（`width:640px`。仕様書 3 章 Y13 も 640）。
  "mock.scan.progress": m(2, "R9-Scan"),
  "mock.scan-stopped.width": m(640, "F5-ScanStopped"),
  // 取り消しの履歴の行（第 6 回）。番号の欄（`width:24px`、右よせ）と、文の欄（`width:420px`）。
  "mock.history.number": m(24, "F-History"),
  "mock.history.name": m(420, "F-History"),
  // 書き出しの進み（第 7 回）。状態行の棒の太さ（`height:4px`）と「やめる」の高さ（`height:20px`）。
  "mock.exporting.bar": m(4, "F7-Exporting"),
  "mock.exporting.stop": m(20, "F7-Exporting"),
  "mock.packband.bar": m(4, "F8-PackGet"),
} as const satisfies Record<string, MockToken>;
export type MockName = keyof typeof mock;

/**
 * px でない数（割合、時間、目盛）。率をかけず、CSS の変数にもしない。
 * spec は part.* と同じ読みかた。
 */
export const plain = {
  /** メーターの色が ok から hot に変わる所（%）。 */
  "meter.hot-from": { value: 70, spec: { section: "s2", row: "Meter", quote: "Up to −3 dB (70 % of the length) is meter.ok" } },
  /** 音割れの印を出しておく時間（秒）。 */
  "meter.clip-hold": { value: 2, spec: { section: "s2", row: "Meter", quote: "2 seconds" } },
} as const;

/**
 * 仕様書 2 章「Scale marks of meters and faders」の表: [dB, 長さ（%。いちばん下が 0）]。大きい音から。
 * 点のあいだは直線でつなぎ、いちばん下の点より小さい音は 0 %（−∞）。
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

/** 同じ見出しの文の、落ちる速さ: fallDb を fallSeconds かけて。上がるときはすぐ。 */
export const meterFall = { fallDb: 20, fallSeconds: 1.7 } as const;

const scaleIndex = (scale: Scale): 0 | 1 | 2 => (scale === 100 ? 0 : scale === 125 ? 1 : 2);

/** 部品の寸法に率をかけ、四捨五入した整数（.5 は切り上げ。切り上げた物は tests/design で一覧にしている）。 */
export function partAt(name: PartName, scale: Scale): number {
  return Math.round((part[name].value * scale) / 100);
}

/** 名前 → その率での値。数は CSS px、色は 16 進数、太さは数。 */
export function tokensAt(scale: Scale, devicePixelRatio: number): Record<string, number | string> {
  const i = scaleIndex(scale);
  const out: Record<string, number | string> = {};
  for (const [k, v] of Object.entries(colors)) out[k] = v;
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
  // part.* と同じく、率をかけて整数にならなければ四捨五入する。
  for (const [k, v] of Object.entries(mock)) out[k] = Math.round((v.value * scale) / 100);
  return out;
}

const unitless = (name: string) => name.includes(".weight");

/** `--color-gray-4: #222222` のような CSS の変数の組。 */
export function cssVariables(scale: Scale, devicePixelRatio: number): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(tokensAt(scale, devicePixelRatio))) {
    out[`--${k.replaceAll(".", "-")}`] = typeof v === "string" ? v : unitless(k) ? String(v) : `${v}px`;
  }
  out["--font-sans"] = fontFamily.sans;
  out["--font-mono"] = fontFamily.mono;
  return out;
}

/** :root（または渡した要素）に CSS の変数を書く。率や devicePixelRatio が変わったら呼びなおす。 */
export function writeCssVariables(
  scale: Scale,
  devicePixelRatio: number = window.devicePixelRatio || 1,
  root: HTMLElement = document.documentElement,
): void {
  for (const [k, v] of Object.entries(cssVariables(scale, devicePixelRatio))) root.style.setProperty(k, v);
}

#!/usr/bin/env python3
"""Shoot the screen (fake scenes) and put each shot next to its mock board in one PNG.

usage: python3 -I tools/design/shoot_ui.py OUT_DIR [SCENE ...]

Runs `npm run build` in ui/, serves ui/dist with `vite preview`, and shoots each scene with
headless google-chrome at 1280x800 and 1920x1080, scale 100 and 150. Next to each shot it puts
the mock board for that scene (rendered by render_mocks.py), cropped or padded to the same size.
Out: OUT_DIR/<scene>-<w>x<h>-<scale>.png (side by side) and OUT_DIR/shots/ (the shots alone).
Names in CLICKS shoot a scene after clicking something (a menu open), through the DevTools protocol.
"""
import json, os, socket, subprocess, sys, time, urllib.request

from PIL import Image, ImageDraw

root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
ui = os.path.join(root, "ui")
out = os.path.abspath(sys.argv[1])
shots = os.path.join(out, "shots")
mocks = os.path.join(out, "mock")

# scene -> mock board (None: no board to compare with)
BOARDS = {
    "connect1": "F-Connect1",
    "connect2": "F-Connect2",
    "connect3": "F-Connect3",
    "empty": "F-Empty",
    "bufferFixed": "F-BufferFixed",
    "settings": "F-Settings",
    "settingsAdvFixed": "F-SettingsAdvFixed",
    "settingsEnd": None,
    "settingsAdvFixedEnd": None,
    "longName": "F-MeterLongName",
    "pianoRoll": "A-PianoRoll",
    "pianoRoll720": "F-PR720",
    "pianoRoll150": "F-PR150",
    "pianoRollWide": "F-WidePR",
    "pianoRollPlay": "F-PRPlay",
    "pianoRollRec": "F-PRRec",
    "pianoRollEmpty": "F-PREmpty",
    "pianoRollNoNotes": None,
    "pianoRollNoNotesScrolled": None,
    "pianoRollGrown": None,
    "notesNotWritten": None,
    "mixer": "F4-Mixer",
    "history": "F-History",
    "shelf": "F-ShelfArrange",
    "params": "F-ParamFew",
    "recording": "F-ArrangeRec",
    "countIn": "F-Connect4",
    "takesArrange": "F-Takes",
    "loop": "Main",
    "muted": "Main",
    "menuFile": "R5-Menus",
    "menuZoom": "R5-Menus",
    "menuClip": "R5-Menus",
    "recover1": "F-Recover1",
    "recover2": "F-Recover2",
    "recover3": "F-Recover3",
    "scanning": "R9-Scan",
    "scanFinished": "R9-Scan",
    "pluginPlaces": "R9-Scan",
    "scanStopped": "F5-ScanStopped",
    "arrangeZoomIn": "R3-Arrange",
    "arrangeZoomBeats": "R3-Arrange",
    "scaleMenu": "A-PianoRoll",
    "noteListTyping": "R3-Lanes",
    "noteListBroken": "R3-Lanes",
    "ccLanes": "R3-Lanes",
    "ccOtherMenu": "A-PianoRoll",
    "inspectorBulk": "A-PianoRoll",
    "ccList": "R5-Lists",
    "ccListFolded": "R5-Lists",
    "masterTip150": "F-PR150",
    "arrangeTrimmed": "R3-Arrange",
    "arrangeTrimmedRoll": None,
    "arrangeMoveDrag": "R3-Arrange",
    "arrangeCopyDrag": "R3-Arrange",
    "arrangeTrimDrag": "R3-Arrange",
    # 第 4 回: トラックと棚。
    "tracks6": "R7-Arrange6",
    "shelfNewTrack": "F4-ShelfSample",
    "shelfSample": "F4-ShelfSample",
    "shelfAudition": "F4-ShelfAudition",
    "shelfReplace": "F4-ShelfReplace",
    "trackRename": "R6-Tracks",
    "trackDrag": "R6-Tracks",
    "trackColorMenu": "R6-Tracks",
    "drumRoll": "R7-Drum",
    # 第 4 回 区切り2: meloet について。
    "about": "R7-About",
    "aboutLicenses": "R7-AboutLicense",
    "aboutPart": "R7-AboutLicense",
    # 第 4 回 区切り3: パラメータの欄と棚のエフェクト。
    "paramsDelay": "R6-Delay",
    "shelfEffects": None,
    # 第 5 回の前段: 最初から入っている音色を 31 に（84）。
    "shelf84": "R8-Shelf84",
    # 第 5 回 区切り1: スキャンの画面とダイアログを動かす。
    "scanStoppedRetrying": "F5-ScanStopped",
    "scanStoppedMany": "F5-ScanStopped",
    "scanRetryAdded": "R9-Scan",
    "scanRetryStopped": "R9-Scan",
    "scanFinishedNone": "R9-Scan",
    "pluginsFound": "R9-Scan",
    "pluginPlacesFolded": "R9-Scan",
    "pluginStopped": None,
    "mixer16": None,
    "settingsAdv": "F-SettingsAdv",
    "settingsMoved": "R8-Clip",
    "scanStoppedMoved": "R8-Clip",
    # 第 5 回 区切り2: 棚の「あとから入れたもの」（SF2 とプラグイン）、見つからない物、ドラムの再生中。
    "sf2Shelf": "F5-SF2Shelf",
    "sf2Added": "F5-SF2Shelf",
    "sf2Unreadable": None,
    "sf2DropAddable": "F5-SF2Drop",
    "sf2DropNotAddable": "F5-SF2Drop",
    "shelfFx": "F5-ShelfFx",
    "shelfSearch": "F5-ShelfSearch",
    "shelfPlugins": "Opt5-Plugins",
    "pluginLoading": "F5-PluginLoading",
    "pluginAudition": "F5-PluginAudition",
    "pluginUnreadable": "F5-PluginLoading",
    "pluginTrack": "F5-PluginTrack",
    "missingPack": "R8-Missing",
    "missingSf2": "R8-Missing",
    "missingPlugin": None,
    "drumPlay": "R8-DrumPlay",
    "drumRollSf2": "R7-Drum",
    # 第 5 回 区切り3: プラグインの窓、パラメータの欄のプラグイン、ミキサーの 86 の残り。
    "windowOpen": "F-WindowOpen",
    "paramFew": "F-ParamFew",
    "paramMany": "F-ParamMany",
    "paramSearch": "F-ParamSearch",
    "paramNoWindow": "F-ParamNoWindow",
    "mixerBypass": "F5-MixerBypass",
    "insertReplace": "R9-Inserts",
    "mixerFx": "R8-MixerFx",
    "paramsNone": "R9-Params",
    "noOutput": "R9-Params",
    "paramClamped": "R9-Params",
    "windowUndo": "Opt5-Undo",
    # デザインの第 9 版の 88〜91（第 6 回 区切り0）。
    "paramMany6": "F6-ParamMany",
    "paramKnobsSearch": None,
    "sf2DropMany": "F5-SF2Drop",
    "sf2AddedMany": None,
    "shelfBuiltinOpen": "F5-PluginLoading",
    # 第 6 回 区切り1: テンポと拍子の変更、マーカー（約束 20）。
    "tempo": "F6-Tempo",
    "tempoFolded": "F6-AutoRec",
    "markerAdded": "R10-Markers",
    "markerRenaming": "R10-Markers",
    "markerMoving": "R10-Markers",
    "tempoDrag": "F6-Tempo",
    "pianoRollMarkers": "R10-Markers",
    # 第 6 回 区切り2 ・ 3: オートメーション、取り消しの履歴の帯（約束 21）。paramMany6（F6-ParamMany）は上にある。
    "autoRec": "F6-AutoRec",
    "autoRest": "F6-AutoRest",
    "mixerAuto": "F6-MixerAuto",
    "mixerRest": "F6-MixerRest",
    "autoAdd": "R10-AutoEdit",
    "autoMove": "R10-AutoEdit",
    "autoBox": "R10-AutoEdit",
    "historyList": "F-History",
    "historyUndone3": "R10-History",
}
# Double-clicks the Vel cell of a row of the note list and types into the field (R3-Lanes: "96|" and "200").
TYPE_IN_LIST = """
const cell = document.querySelectorAll('[data-testid="pr-note-list"] .row')[%d].querySelector('[data-field="velocity"]');
cell.dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
await new Promise((r) => setTimeout(r, 100));
const input = document.querySelector('[data-test="note-list-field"]');
input.value = "%s";
input.dispatchEvent(new Event("input", { bubbles: true }));
"""

# Folds headings of the shelf. The shelf makes only the rows it shows, so it scrolls down to a heading that is not there yet.
FOLD_SHELF = """
const list = document.querySelector('[data-test="shelf-list"]');
for (const g of %s) {
  for (let i = 0; i < 40 && !document.querySelector(`[data-group="${g}"]`); i++) {
    list.scrollTop += 100;
    await new Promise((r) => setTimeout(r, 50));
  }
  document.querySelector(`[data-group="${g}"]`).click();
  await new Promise((r) => setTimeout(r, 50));
}
list.scrollTop = 0;
"""

# Types into the search field of the parameters panel (F-ParamSearch).
TYPE_PARAM_SEARCH = """
const input = document.querySelector('[data-test="params-search"]');
if (input) {
  input.value = "%s";
  input.dispatchEvent(new Event("input", { bubbles: true }));
}
"""

# Types into the shelf's search field (F5-ShelfSearch).
TYPE_SEARCH = """
const input = document.querySelector('[data-test="shelf-search"]');
input.value = "%s";
input.dispatchEvent(new Event("input", { bubbles: true }));
"""

# Presses "ベース A" (track 2, bars 5–8) of the scene arrangeTrimmed and drags it without letting go (R3-Arrange).
# Arguments: x pressed, x moved to (CSS px from the left of the lanes, 1 bar = 64 at 100 %), and the PointerEvent options.
DRAG_CLIP = """
const lanes = document.querySelector(".lanes");
const r = lanes.getBoundingClientRect();
const ev = (type, x, o) => lanes.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, clientX: r.left + x, clientY: r.top + 94, ...o }));
ev("pointerdown", %d, %s);
ev("pointermove", (%d + %d) / 2, %s);
ev("pointermove", %d, %s);
"""


# Presses the bar of a parameter row (by its name) and holds it a little to the right (F-ParamFew: "… を動かしています").
DRAG_PARAM = """
const row = [...document.querySelectorAll('[data-test="param-row"]')].find((r) => r.querySelector(".name")?.textContent === "%s");
const bar = row.querySelector('[data-test="param-slider"]');
const r = bar.getBoundingClientRect();
const ev = (type, f) => bar.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, clientX: r.left + r.width * f, clientY: r.top + 6 }));
ev("pointerdown", %f);
ev("pointermove", %f);
"""


# Drags the head row of a dialog by (dx, dy) and lets go (spec 3, 82).
DRAG_DIALOG = """
const head = document.querySelector('%s [data-dialog-head]');
const r = head.getBoundingClientRect();
const ev = (type, x, y) => head.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, clientX: x, clientY: y }));
ev("pointerdown", r.left + 40, r.top + 10);
ev("pointermove", r.left + 40 + %d, r.top + 10 + %d);
ev("pointerup", r.left + 40 + %d, r.top + 10 + %d);
"""


def drag_dialog(dialog, dx, dy):
    return "js:" + DRAG_DIALOG % (dialog, dx, dy, dx, dy)


# Puts the pointer on an element and waits for its tip.
HOVER = """
document.querySelector('%s').dispatchEvent(new PointerEvent("pointerenter"));
await new Promise((r) => setTimeout(r, 800));
"""


# Right-clicks "Aメロ" of the piano (bars 5–8) to open the clip's list (R5-Menus).
CONTEXT_CLIP = """
const lanes = document.querySelector(".lanes");
const r = lanes.getBoundingClientRect();
lanes.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, button: 2, clientX: r.left + 300, clientY: r.top + 30 }));
"""


# Turns the wheel sideways over the notes area of the piano roll by dx CSS px.
WHEEL_ROLL = """
document.querySelector(".roll .middle").dispatchEvent(new WheelEvent("wheel", { bubbles: true, cancelable: true, deltaX: %d }));
"""


# Double-clicks the name of track 2 and types into the field (R6-Tracks: "ベース 2").
RENAME_TRACK = """
document.querySelectorAll(".track-name")[1].dispatchEvent(new MouseEvent("dblclick", { bubbles: true }));
await new Promise((r) => setTimeout(r, 100));
const input = document.querySelector('[data-test="track-name-field"]');
input.value = "ベース 2";
input.dispatchEvent(new Event("input", { bubbles: true }));
"""

# Presses the header of track 4 and drags it up between tracks 1 and 2 without letting go (R6-Tracks).
DRAG_TRACK = """
const head = document.querySelectorAll('[data-test="track-head"]')[3];
const r = head.getBoundingClientRect();
const ev = (type, y) => head.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, clientX: r.left + 100, clientY: y }));
ev("pointerdown", r.top + 20);
ev("pointermove", r.top + 10);
ev("pointermove", r.top + 20 - r.height * 2.4);
"""

# Right-clicks the header of track 2 and opens 色 ▸ (R6-Tracks).
TRACK_COLORS = """
const head = document.querySelectorAll('[data-test="track-head"]')[1];
const r = head.getBoundingClientRect();
head.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, button: 2, clientX: r.left + 120, clientY: r.top + 30 }));
await new Promise((r) => setTimeout(r, 100));
const row = [...document.querySelectorAll('[data-test="context-menu"] button')].find((b) => b.textContent.includes("色"));
row.dispatchEvent(new PointerEvent("pointerenter"));
"""


def drag_clip(x0, x1, opts="{}"):
    return "js:" + DRAG_CLIP % (x0, opts, x0, x1, opts, x1, opts)


# Types into the open name field of a marker (R10-Markers: "Aメロ ・ 1 回目").
TYPE_MARKER = """
// The arrange canvas of a scene with many clips is not done when the page first settles; wait for it.
await new Promise((r) => setTimeout(r, 2000));
const input = document.querySelector('[data-test="marker-name-field"]');
input.value = "%s";
input.dispatchEvent(new Event("input", { bubbles: true }));
"""

# Presses the marker tag with the given name and drags it dx CSS px without letting go (R10-Markers: "6. 1. 000").
DRAG_MARKER = """
await new Promise((r) => setTimeout(r, 2000));
const tag = [...document.querySelectorAll('[data-test="marker-tag"]')].find((t) => t.textContent === "%s");
const row = document.querySelector('[data-test="marker-row"]');
const r = tag.getBoundingClientRect();
const ev = (el, type, x) => el.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, clientX: x, clientY: r.top + 6 }));
ev(tag, "pointerdown", r.left + 6);
ev(row, "pointermove", r.left + 6 + %d / 2);
ev(row, "pointermove", r.left + 6 + %d);
"""

# Presses the tempo point at a bar line (x CSS px from the left of the lanes) and drags it by (dx, dy) without letting go.
DRAG_TEMPO = """
const lane = document.querySelector('[data-test="tempo-row"]');
const r = lane.getBoundingClientRect();
const y0 = %f;
const ev = (type, x, y) => lane.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, clientX: r.left + x, clientY: r.top + y }));
ev("pointerdown", %d, y0);
ev("pointermove", %d + %d / 2, y0 + %d / 2);
ev("pointermove", %d + %d, y0 + %d);
"""


def drag_tempo(x, y, dx, dy):
    return "js:" + DRAG_TEMPO % (y, x, x, dx, dy, x, dx, dy)


# Opens the second row under the first licence heading of the about dialog and scrolls it to the top.
OPEN_PART = """
const row = document.querySelectorAll('[data-test="about-group"] ~ [data-test="about-part"]')[1];
row.click();
await new Promise((r) => setTimeout(r, 300));
row.previousElementSibling.previousElementSibling.scrollIntoView({ block: "start" });
"""

# Shots that need a click first (a menu is screen-only state): name -> (scene, CSS selector to click).
# A selector that starts with "js:" is run as a script instead (it may await).
# Several selectors (a tuple) are clicked one after another.
CLICKS = {
    "songMenu": ("saved", '[data-test="transport-song"]'),
    "countInMenu": ("empty", '[data-test="transport-count-in"]'),
    "arrangeZoomIn": ("loop", '[aria-label="横に拡大"]'),
    # Two steps in: 1 beat is as wide as the gap between bar numbers, so beat lines show (B-61).
    "arrangeZoomBeats": ("loop", ('[aria-label="横に拡大"]', '[aria-label="横に拡大"]')),
    "scaleMenu": ("pianoRoll", '[data-test="inspector-scale"]'),
    "noteListTyping": ("pianoRoll", "js:" + TYPE_IN_LIST % (2, "96")),
    "noteListBroken": ("pianoRoll", "js:" + TYPE_IN_LIST % (4, "200")),
    "ccLanes": ("pianoRollCc", ('[data-test="pr-cc-sustain"]', '[data-test="pr-cc-pitchBend"]', '[data-test="pr-cc-other"]',
                                '[data-test="pr-cc-other-menu"] [data-row="0"]')),
    "ccOtherMenu": ("pianoRollCc", '[data-test="pr-cc-other"]'),
    # 「長さをそろえる」と「ベロシティをまとめて変える」をひらく（まとめて変える行）。
    "inspectorBulk": ("pianoRoll", ('[data-test="inspector-same-length"]', '[data-test="inspector-velocity-all"]')),
    # ひらいた「CC の一覧」（R5-Lists）。
    "ccList": ("pianoRollCc", '[data-test="pr-cc-list"]'),
    # サステインの見出しを押してたたむ。
    "ccListFolded": ("pianoRollCc", ('[data-test="pr-cc-list"]', '[data-group="cc64"]')),
    # 細い形のマスターの値に乗せる（「マスター ・ −∞ dB」）。
    "masterTip150": ("pianoRoll150", "js:" + HOVER % '[data-test="transport-master-db"]'),
    "arrangeMoveDrag": ("arrangeTrimmed", drag_clip(300, 364)),
    "arrangeCopyDrag": ("arrangeTrimmed", drag_clip(300, 556, "{ altKey: true }")),
    "arrangeTrimDrag": ("arrangeTrimmed", drag_clip(509, 413, "{ shiftKey: true }")),
    "menuFile": ("saved", '[data-menu="0"]'),
    "menuZoom": ("muted", ('[data-menu="2"]', '[data-test="menubar-list"] [data-row="10"]')),
    "menuClip": ("muted", "js:" + CONTEXT_CLIP),
    # ノートをぜんぶ消したクリップで、横に 1 小節ぶん送る（送った先でも目盛と方眼が出る）。
    "pianoRollNoNotesScrolled": ("pianoRollNoNotes", "js:" + WHEEL_ROLL % 160),
    # F4-ShelfSample: グランドピアノの ▶ を押した所。
    "shelfSample": ("shelfNewTrack", '[data-row="grandPiano"] [data-test="shelf-sample"]'),
    # R8-Shelf84: 「ピアノ ・ 鍵盤」と「ギター」のほかの見出しをたたむ。
    "shelf84": ("shelf84", "js:" + FOLD_SHELF % '["bass", "drums", "padStrings", "wind", "synth"]'),
    "trackRename": ("tracks6", "js:" + RENAME_TRACK),
    "trackDrag": ("tracks6", "js:" + DRAG_TRACK),
    "trackColorMenu": ("tracks6", "js:" + TRACK_COLORS),
    # ヘルプ ▸ meloet について、オープンソースのライセンス ▸、いちばん多いライセンスの見出しと、その 2 つめの行。
    "about": ("tracks6", ('[data-menu="6"]', '[data-test="menubar-list"] [data-row="0"]')),
    "aboutLicenses": ("tracks6", ('[data-menu="6"]', '[data-test="menubar-list"] [data-row="0"]', '[data-test="about-licenses"]')),
    "aboutPart": ("tracks6", ('[data-menu="6"]', '[data-test="menubar-list"] [data-row="0"]', '[data-test="about-licenses"]',
                               '[data-test="about-group"]', "js:" + OPEN_PART)),
    "settingsAdv": ("settings", '[data-test="settings-advanced"]'),
    "settingsMoved": ("settings", drag_dialog('[data-test="settings-dialog"]', -200, -40)),
    "scanStoppedMoved": ("scanStopped", drag_dialog('[data-test="scan-stopped-dialog"]', 260, 120)),
    # 第 5 回 区切り2: 棚の見出しをモックのとおりにひらく（はじめはどれもたたむ。仕様書 3 章 Y12）。
    "sf2Shelf": ("sf2Shelf", '[data-group="ff:guitar:FluidR3_GM.sf2"]'),
    "shelfFx": ("shelfFx", '[data-group="c:effect:reverb"]'),
    "shelfSearch": ("shelfSearch", "js:" + TYPE_SEARCH % "Dragonfly"),
    "shelfPlugins": ("shelfPlugins", ('[data-test="shelf-effects"]', '[data-group="c:effect:reverb"]')),
    "pluginLoading": ("pluginLoading", '[data-group="c:instrument:synth"]'),
    "pluginAudition": ("pluginAudition", '[data-group="c:instrument:synth"]'),
    "pluginUnreadable": ("pluginUnreadable", '[data-group="c:instrument:synth"]'),
    "paramSearch": ("paramSearch", "js:" + TYPE_PARAM_SEARCH % "cutoff"),
    "paramKnobsSearch": ("paramMany6", "js:" + TYPE_PARAM_SEARCH % "cc 1|"),
    "shelfBuiltinOpen": ("shelfPlugins", ('[data-group="builtin:sounds"]', '[data-group="guitar"]')),
    "windowOpen": ("windowOpen", '[data-test="status-windows"]'),
    "paramFew": ("paramFew", "js:" + DRAG_PARAM % ("Decay", 0.1, 0.12)),
    # 第 6 回 区切り1: マーカーの名前を打っている途中、マーカーを 1 小節ぶん動かしている途中（R10-Markers）。
    "markerRenaming": ("markerRenaming", "js:" + TYPE_MARKER % "Aメロ ・ 1 回目"),
    "markerMoving": ("markerMoving", "js:" + DRAG_MARKER % ("Aメロ", 64, 64)),
    # 9 小節目の点（120.00）を 1 拍右、少し上へ動かしている途中（もとの所に accent の枠、読み値は位置）。
    "tempoDrag": ("tempo", drag_tempo(512, 12, 16, -6)),
}
SIZES = [(1280, 800), (1920, 1080)]
SCALES = [100, 150]
# Scenes that stand for one board at one window size and zoom: name -> [(w, h, scale)].
ONLY = {
    "sf2Shelf": [(1280, 800, 100), (1280, 800, 150)],
    "sf2Added": [(1280, 800, 100)],
    "sf2Unreadable": [(1280, 800, 100)],
    "sf2DropAddable": [(1280, 800, 100), (1280, 800, 150)],
    "sf2DropNotAddable": [(1280, 800, 100)],
    "shelfFx": [(1280, 800, 100)],
    "shelfSearch": [(1280, 800, 100)],
    "shelfPlugins": [(1280, 800, 100)],
    "pluginLoading": [(1280, 800, 100)],
    "pluginAudition": [(1280, 800, 100), (1920, 1080, 150)],
    "pluginUnreadable": [(1280, 800, 100)],
    "pluginTrack": [(1280, 800, 100)],
    "missingPack": [(1280, 800, 100)],
    "missingSf2": [(1280, 800, 100)],
    "missingPlugin": [(1280, 800, 100)],
    "drumPlay": [(1280, 800, 100)],
    "drumRollSf2": [(1280, 800, 100)],
    "windowOpen": [(1280, 800, 100)],
    "paramFew": [(1280, 800, 100)],
    "paramMany": [(1280, 800, 100)],
    "paramSearch": [(1280, 800, 100)],
    "paramNoWindow": [(1280, 800, 100)],
    "mixerBypass": [(1280, 800, 100)],
    "insertReplace": [(1280, 800, 100)],
    "mixerFx": [(1280, 800, 100)],
    "paramsNone": [(1280, 800, 100)],
    "noOutput": [(1280, 800, 100)],
    "paramClamped": [(1280, 800, 100)],
    "windowUndo": [(1280, 800, 100)],
    "paramMany6": [(1280, 800, 100)],
    "paramKnobsSearch": [(1280, 800, 100)],
    "sf2DropMany": [(1280, 800, 100)],
    "sf2AddedMany": [(1280, 800, 100)],
    "shelfBuiltinOpen": [(1280, 800, 100)],
    "pianoRoll": [(1280, 800, 100)],
    "pianoRoll720": [(1280, 720, 100)],
    "pianoRoll150": [(1280, 800, 150)],
    "pianoRollWide": [(1920, 1080, 100)],
    "pianoRollPlay": [(1280, 800, 100)],
    "pianoRollRec": [(1280, 800, 100)],
    "pianoRollEmpty": [(1280, 800, 100)],
    "pianoRollNoNotes": [(1280, 800, 100)],
    "pianoRollNoNotesScrolled": [(1280, 800, 100)],
    "pianoRollGrown": [(1280, 800, 100)],
    "notesNotWritten": [(1280, 800, 100)],
    "noteListTyping": [(1280, 800, 100)],
    "noteListBroken": [(1280, 800, 100)],
    "ccLanes": [(1280, 800, 100), (1920, 1080, 100), (1920, 1080, 150)],
    "ccOtherMenu": [(1920, 1080, 100)],
    "inspectorBulk": [(1280, 800, 100), (1920, 1080, 150)],
    "ccList": [(1280, 800, 100)],
    "ccListFolded": [(1280, 800, 100)],
    "masterTip150": [(1280, 800, 150)],
    "arrangeTrimmed": [(1280, 800, 100), (1280, 800, 150)],
    "arrangeZoomBeats": [(1280, 800, 100), (1280, 800, 150)],
    # At 1280x800 150 % the right panel is folded, so there is no scale button to press.
    "scaleMenu": [(1280, 800, 100), (1920, 1080, 150)],
    "arrangeTrimmedRoll": [(1280, 800, 100)],
    "arrangeMoveDrag": [(1280, 800, 100)],
    "arrangeCopyDrag": [(1280, 800, 100)],
    "arrangeTrimDrag": [(1280, 800, 100)],
    "menuFile": [(1280, 800, 100), (1280, 800, 150)],
    "menuZoom": [(1280, 800, 100)],
    "menuClip": [(1280, 800, 100)],
    "tracks6": [(1280, 800, 100), (1280, 800, 150), (1920, 1080, 100)],
    "shelfNewTrack": [(1280, 800, 100), (1280, 800, 150)],
    "shelfSample": [(1280, 800, 100)],
    "shelfAudition": [(1280, 800, 100), (1920, 1080, 150)],
    "shelfReplace": [(1280, 800, 100)],
    "trackRename": [(1280, 800, 100)],
    "trackDrag": [(1280, 800, 100)],
    "trackColorMenu": [(1280, 800, 100)],
    "about": [(1280, 800, 100), (1280, 800, 150)],
    "aboutLicenses": [(1280, 800, 100), (1280, 800, 150)],
    "aboutPart": [(1280, 800, 100), (1920, 1080, 150)],
    # 第 4 回 区切り3: ミキサー（F4-Mixer は 1280x800 の 100 %）。150 % は形が入るかを見るため。
    "mixer": [(1280, 800, 100), (1280, 800, 150), (1920, 1080, 100)],
    "paramsDelay": [(1280, 800, 100), (1280, 800, 150)],
    "shelfEffects": [(1280, 800, 100), (1920, 1080, 150)],
    "shelf84": [(1280, 800, 100), (1280, 800, 150), (1920, 1080, 100)],
    # 第 5 回 区切り1: スキャン。R9-Scan と F5-ScanStopped は 1280x720 ・ 1280x800 の 100 %。150 % は入るかを見るため。
    "scanning": [(1280, 800, 100), (1280, 800, 150)],
    "scanFinished": [(1280, 800, 100)],
    "scanFinishedNone": [(1280, 800, 100)],
    "pluginsFound": [(1280, 800, 100)],
    "pluginPlaces": [(1280, 800, 100), (1280, 800, 150)],
    "pluginPlacesFolded": [(1280, 800, 100)],
    "scanStopped": [(1280, 800, 100), (1280, 800, 150)],
    "scanStoppedRetrying": [(1280, 800, 100)],
    "scanStoppedMany": [(1280, 720, 100), (1280, 800, 150)],
    "scanRetryAdded": [(1280, 800, 100)],
    "scanRetryStopped": [(1280, 800, 100)],
    "pluginStopped": [(1280, 800, 100)],
    "mixer16": [(1920, 1080, 100)],
    "settingsAdv": [(1280, 800, 100)],
    "settingsMoved": [(1280, 800, 100)],
    "scanStoppedMoved": [(1280, 800, 100)],
    "tempo": [(1280, 800, 100), (1280, 800, 150), (1920, 1080, 100)],
    "tempoFolded": [(1280, 800, 100)],
    "tempoDrag": [(1280, 800, 100)],
    "markerAdded": [(1280, 800, 100), (1280, 800, 150)],
    "markerRenaming": [(1280, 800, 100)],
    "markerMoving": [(1280, 800, 100)],
    "pianoRollMarkers": [(1280, 800, 100)],
    "autoRec": [(1280, 800, 100), (1280, 800, 150)],
    "autoRest": [(1280, 800, 100)],
    "mixerAuto": [(1280, 800, 100)],
    "mixerRest": [(1280, 800, 100)],
    "autoAdd": [(1280, 800, 100)],
    "autoMove": [(1280, 800, 100)],
    "autoBox": [(1280, 800, 100)],
    "historyList": [(1280, 800, 100), (1280, 800, 150)],
    "historyUndone3": [(1280, 800, 100)],
}
scenes = sys.argv[2:] or list(BOARDS)

# Drives headless Chrome over the DevTools protocol (Node's own WebSocket): open, click, shoot.
CLICK_SHOT = r"""
const [port, url, selectors, out, w, h] = process.argv.slice(1);
const { spawn } = await import("node:child_process");
const { writeFileSync, mkdtempSync } = await import("node:fs");
const { tmpdir } = await import("node:os");
const { join } = await import("node:path");
const chrome = spawn("google-chrome", ["--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1",
  `--window-size=${w},${h}`, `--remote-debugging-port=${port}`, `--user-data-dir=${mkdtempSync(join(tmpdir(), "shoot-"))}`, url],
  { stdio: "ignore" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
try {
  let page;
  for (let i = 0; i < 100 && !page; i++) {
    await sleep(100);
    try { page = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === "page"); } catch {}
  }
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((r) => (ws.onopen = r));
  let id = 0;
  const waiting = new Map();
  ws.onmessage = (m) => { const d = JSON.parse(m.data); waiting.get(d.id)?.(d.result); };
  const call = (method, params = {}) => new Promise((r) => { waiting.set(++id, r); ws.send(JSON.stringify({ id, method, params })); });
  await call("Emulation.setDeviceMetricsOverride", { width: Number(w), height: Number(h), deviceScaleFactor: 1, mobile: false });
  // The first tab can still be about:blank; load the scene in the tab we hold.
  await call("Page.enable");
  await call("Page.navigate", { url });
  const evaluate = async (expression) => (await call("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true }))?.result?.value;
  // Waits (in the page, up to 60 s) until `test` is true, then until the page stops changing: fonts loaded, no change
  // to the DOM for 300 ms (at most 5 s more: a playing scene never stops), and two frames for WebGL to paint.
  const settle = async (test) => {
    const ok = await evaluate(`(async () => {
      const end = Date.now() + 60000;
      while (!(${test}) && Date.now() < end) await new Promise((r) => setTimeout(r, 50));
      if (!(${test})) return false;
      await document.fonts.ready;
      await new Promise((done) => {
        let t = setTimeout(stop, 300);
        const cap = setTimeout(stop, 5000);
        const mo = new MutationObserver(() => { clearTimeout(t); t = setTimeout(stop, 300); });
        mo.observe(document, { subtree: true, childList: true, attributes: true, characterData: true });
        function stop() { mo.disconnect(); clearTimeout(t); clearTimeout(cap); done(); }
      });
      await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
      return true;
    })()`);
    if (!ok) throw new Error(`the page did not get ready: ${test}`);
  };
  await settle(`location.href === ${JSON.stringify(url)} && document.readyState === "complete" && document.querySelector(".app .status") !== null`);
  for (const selector of JSON.parse(selectors)) {
    if (selector.startsWith("js:")) {
      await evaluate(`(async () => {${selector.slice(3)}})()`);
      await settle("true");
    } else {
      await settle(`document.querySelector(${JSON.stringify(selector)}) !== null`);
      await evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
      await settle("true");
    }
  }
  const shot = await call("Page.captureScreenshot", { format: "png" });
  writeFileSync(out, Buffer.from(shot.data, "base64"));
  ws.close();
} finally {
  chrome.kill();
}
"""


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def run(cmd, **kw):
    subprocess.run(cmd, check=True, **kw)


os.makedirs(shots, exist_ok=True)
env = dict(os.environ)
env["PATH"] = os.path.expanduser("~/.local/node/bin") + os.pathsep + env.get("PATH", "")
run(["npm", "run", "build"], cwd=ui, env=env, stdout=subprocess.DEVNULL)

boards = sorted({BOARDS[s] for s in scenes if BOARDS.get(s)})
if boards:
    run([sys.executable, "-I", os.path.join(root, "tools", "design", "render_mocks.py"), mocks, *boards],
        stdout=subprocess.DEVNULL)

port = free_port()
preview = subprocess.Popen(
    [os.path.join(ui, "node_modules", ".bin", "vite"), "preview", "--port", str(port), "--strictPort", "--host", "127.0.0.1"],
    cwd=ui, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
try:
    for _ in range(100):
        try:
            urllib.request.urlopen(f"http://127.0.0.1:{port}/", timeout=1)
            break
        except OSError:
            time.sleep(0.1)
    else:
        sys.exit("vite preview did not start")

    for scene in scenes:
        for w, h, scale in ONLY.get(scene) or [(w, h, s) for w, h in SIZES for s in SCALES]:
            stem = f"{scene}-{w}x{h}-{scale}"
            shot = os.path.join(shots, stem + ".png")
            if scene in CLICKS:
                base, selector = CLICKS[scene]
                selectors = json.dumps([selector] if isinstance(selector, str) else list(selector))
                done = subprocess.run(["node", "--input-type=module", "-e", CLICK_SHOT, str(free_port()),
                                       f"http://127.0.0.1:{port}/?scene={base}&scale={scale}", selectors, shot, str(w), str(h)],
                                      env=env, capture_output=True, text=True, timeout=180)
                if done.returncode != 0:
                    sys.exit(f"{stem}: {done.stderr.strip() or done.returncode}")
            else:
                run(["google-chrome", "--headless=new", "--disable-gpu", "--hide-scrollbars",
                     "--force-device-scale-factor=1", "--virtual-time-budget=3000",
                     f"--window-size={w},{h}", f"--screenshot={shot}",
                     f"http://127.0.0.1:{port}/?scene={scene}&scale={scale}"],
                    capture_output=True, timeout=60)
            left = Image.open(shot).convert("RGB")
            board = BOARDS.get(scene)
            right = Image.new("RGB", (w, h), (40, 0, 40))
            if board and os.path.exists(os.path.join(mocks, board + ".png")):
                m = Image.open(os.path.join(mocks, board + ".png")).convert("RGB")
                right.paste(m.crop((0, 0, min(w, m.width), min(h, m.height))), (0, 0))
            sheet = Image.new("RGB", (w * 2 + 8, h + 24), (255, 0, 255))
            sheet.paste(left, (0, 24))
            sheet.paste(right, (w + 8, 24))
            d = ImageDraw.Draw(sheet)
            d.text((4, 6), f"screen  {stem}", fill=(0, 0, 0))
            d.text((w + 12, 6), f"mock  {board or '-'} (100 %)", fill=(0, 0, 0))
            sheet.save(os.path.join(out, stem + ".png"))
            print(os.path.join(out, stem + ".png"))
finally:
    preview.terminate()
    preview.wait()

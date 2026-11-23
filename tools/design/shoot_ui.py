#!/usr/bin/env python3
"""Shoot the screen (fake scenes) and put each shot next to its mock board in one PNG.

usage: python3 -I tools/design/shoot_ui.py OUT_DIR [--jobs=N] [--theme=ID[,ID...]|all] [--build] [SCENE ...]

Runs `npm run build` in ui/ (only when the build inputs changed since the last build of this tool, or
with --build; the hash of the inputs is kept in ui/dist/.shoot-stamp), serves ui/dist with `vite preview`,
and shoots each scene with headless google-chrome at 1280x800 and 1920x1080, scale 100 and 150. Next to each shot it puts
the mock board for that scene (rendered by render_mocks.py), cropped or padded to the same size.
Out: OUT_DIR/<scene>-<w>x<h>-<scale>.png (side by side) and OUT_DIR/shots/ (the shots alone).
Names in CLICKS shoot a scene after clicking something (a menu open), through the DevTools protocol.
--jobs=N shoots N at a time (default 8; each shot has its own Chrome, profile and port). The paths
are printed in the same order as with --jobs=1.
--theme= takes one or more ids of THEME_IDS in ui/src/design/themes.ts (comma list), or all; the
files of each theme end in -<id>. All themes share one build and one vite preview.
"""
import concurrent.futures, hashlib, json, os, re, shutil, signal, socket, subprocess, sys, tempfile, threading, time, urllib.request

from PIL import Image, ImageDraw

root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
ui = os.path.join(root, "ui")
args = [a for a in sys.argv[1:] if not a.startswith(("--theme=", "--jobs=")) and a != "--build"]
force_build = "--build" in sys.argv[1:]
theme_arg = next((a.split("=", 1)[1] for a in sys.argv[1:] if a.startswith("--theme=")), "")
jobs = int(next((a.split("=", 1)[1] for a in sys.argv[1:] if a.startswith("--jobs=")), "8"))
if theme_arg == "all":
    with open(os.path.join(ui, "src", "design", "themes.ts"), encoding="utf-8") as f:
        themes = re.findall(r'"([^"]+)"', re.search(r"THEME_IDS\s*=\s*\[([^\]]*)\]", f.read()).group(1))
else:
    themes = [t for t in theme_arg.split(",") if t] or [""]
out = os.path.abspath(args[0])
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
    "scaleHighlight": None,
    "scaleHighlightLocked": None,
    "pianoRollGhost": None,
    "pianoRollGhostMenu": None,
    "pianoRollGhostFolded": None,
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
    "menuRecent": None,
    "menuRecentEmpty": None,
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
    # Tracks and the shelf.
    "tracks6": "R7-Arrange6",
    "shelfNewTrack": "F4-ShelfSample",
    "shelfSample": "F4-ShelfSample",
    "shelfAudition": "F4-ShelfAudition",
    "shelfReplace": "F4-ShelfReplace",
    "trackRename": "R6-Tracks",
    "trackDrag": "R6-Tracks",
    "trackColorMenu": "R6-Tracks",
    # Round 12, A3.
    "trackDuplicated": None,
    "tooManyTracks": None,
    "drumRoll": "R7-Drum",
    # About meloet.
    "about": "R7-About",
    "aboutLicenses": "R7-AboutLicense",
    "aboutPart": "R7-AboutLicense",
    # The key list (round 12 G1; no mock board).
    "keyList": None,
    "keyListFiltered": None,
    # The parameters panel and the effects in the shelf.
    "paramsDelay": "R6-Delay",
    "shelfEffects": None,
    # The 31 built-in sounds (84).
    "shelf84": "R8-Shelf84",
    # The scan screens, and moving dialogs.
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
    # The shelf's 「あとから入れたもの」 (SF2 and plugins), missing things, drums while playing.
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
    # Plugin windows, plugins in the parameters panel, the rest of 86 in the mixer.
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
    # 88 to 91 of the design's 9th edition.
    "paramMany6": "F6-ParamMany",
    "paramKnobsSearch": None,
    "sf2DropMany": "F5-SF2Drop",
    "sf2AddedMany": None,
    "shelfBuiltinOpen": "F5-PluginLoading",
    # Tempo and meter changes, markers.
    "tempo": "F6-Tempo",
    "tempoFolded": "F6-AutoRec",
    "markerAdded": "R10-Markers",
    "markerRenaming": "R10-Markers",
    "markerMoving": "R10-Markers",
    "tempoDrag": "F6-Tempo",
    "pianoRollMarkers": "R10-Markers",
    # Automation, the history band. paramMany6 (F6-ParamMany) is above.
    "autoRec": "F6-AutoRec",
    "autoRest": "F6-AutoRest",
    "mixerAuto": "F6-MixerAuto",
    "mixerRest": "F6-MixerRest",
    "autoAdd": "R10-AutoEdit",
    "autoMove": "R10-AutoEdit",
    "autoBox": "R10-AutoEdit",
    "autoPress": "R10-AutoEdit",
    "autoBoxDrag": "R10-AutoEdit",
    "historyList": "F-History",
    "historyUndone3": "R10-History",
    # The history band: the row under the pointer, the picked row, the row picked with keys (all step 11), and 5000 steps.
    "historyPointer": "R10-History",
    "historyPicked": "R10-History",
    "historyKeys": "R10-History",
    "history5000": None,
    # The answers to 92 (F7-Answers92, R11-Answers92).
    "laneScale": "F7-Answers92",
    "answers92": "F7-Answers92",
    "historyLatest": "R11-Answers92",
    "historyAllUndone": "R11-Answers92",
    "historyEmpty": "R11-Answers92",
    "headTempo": "R11-Answers92",
    "pianoRollMarker": "R11-Answers92",
    "meterFlagMenu": "R11-Answers92",
    # Vocal tracks, the two sides of the input meter, recording, the advanced sound settings, measuring the latency, 19-A.
    "vocalTrack": "F7-VocalTrack",
    "vocalInputMenu": "F7-VocalTrack",
    "vocalNoInput": None,
    "vocalMonitorOn": None,
    "vocalTooLoud": None,
    "vocalNoPianoRoll": None,
    "vocalRec": "F7-VocalRec",
    "twoRec": "F7-TwoRec",
    "settingsAdv7": "F7-SettingsAdv",
    "latency": "F7-Latency",
    "latencyDone": None,
    "latencyFailed": None,
    "vocalFit": "R11-Fit",
    "paramsSound": "F7-Answers92",
    # Audio clips, bringing in audio files, missing audio.
    "audioEdit": "F7-AudioEdit",
    "audioTall": "F7-AudioEdit",
    "audioDropOne": "F7-AudioDrop",
    "audioDrop": "F7-AudioDrop",
    "audioDropRight": "F7-AudioDrop",
    "audioDropMixed": "F7-AudioDrop",
    "audioMissing": "F7-AudioMissing",
    "audioLoading": None,
    "audioKinds": "R11-Audio",
    # 「ノーマライズ」 and 「逆にする」 (round 12, A9).
    "audioNormalized": None,
    "menuAudioClip": None,
    # Repeating clips (round 12, A1; the spec's mock is not a board).
    "arrangeRepeatHover": None,
    "arrangeRepeatDrag": None,
    "arrangeRepeatBox": None,
    "arrangeRepeated": None,
    # Stretch and pitch.
    "stretch": "F7-Stretch",
    "stretchRendering": "F7-Stretch",
    "stretchPitch": None,
    "stretchNoTempo": None,
    "stretchRamp": None,
    "recoverVocal": None,
    # Export.
    "exportDialog": "F7-Export",
    "exportWaiting": None,
    "exportCounting": None,
    "exportClips": None,
    "exportStopped": "F-Export",
    "exportAdvanced": None,
    "exportCheckFailed": None,
    # 「音の大きさ」 of the export (round 12 F1; the spec's mock is not a board).
    "exportLoudness": None,
    "exportLoudnessLimited": None,
    "exportLoudnessQuieter": None,
    "exportLoudnessCounting": None,
    "exportLoudnessSilent": None,
    # Two or more clips, the check before export.
    "multiClip": "F8-MultiClip",
    "exportCheck": "F8-ExportCheck",
    "exportClip": "F8-ExportClip",
    "exportFit": "R11-Fit",
    # The sound pack.
    "packAsk": "F8-PackAsk",
    "packGet": "F8-PackGet",
    "packFailOffline": "F8-PackFail",
    "packSettings": "F8-PackSettings",
    "exporting": "F7-Exporting",
    "exportingRealtime": "F7-Exporting",
    "exportDone": "F7-Exporting",
    "exportCancelled": None,
    "exportFailed": None,
    # Round 12 R3: the metronome ▾ window (the mock is in docs/history/round12-spec.ja.html, not a board).
    "metronomeWindow": None,
    # Round 12 M1: 「プリセット ▾」, its list, the name box; a gate with no preset of its own (the mock is in the spec).
    "presets": None,
    "presetsMenu": None,
    "presetsSaving": None,
    "presetsNone": None,
}
# Double-clicks the Vel cell of a row of the note list and types into the field (R3-Lanes: "96|" and "200").
TYPE_KEY_FILTER = """
const input = document.querySelector('[data-test="keylist-search"]');
input.value = "%s";
input.dispatchEvent(new Event("input", { bubbles: true }));
"""

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

# Right-clicks the name strip of the first audio clip of the scene (its menu ends with 「逆にする」「ノーマライズ」).
CONTEXT_AUDIO_CLIP = """
const label = [...document.querySelectorAll(".labels > .clip-label")].find((l) => l.textContent.includes("歌 テイク 1"));
const r = label.getBoundingClientRect();
document.querySelector(".lanes").dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, button: 2, clientX: r.left + 4, clientY: r.top + 4 }));
"""


# The repeat handle of 「Aメロ」 (bars 6–8, its right end at x 512) in the scene arrangeTrimmed: the pointer on it (⟳),
# and pressed and moved right by dx without letting go (the frames of the copies, 「2 回くり返します」).
REPEAT_HANDLE = """
const lanes = document.querySelector(".lanes");
const r = lanes.getBoundingClientRect();
const ev = (type, x) => lanes.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, clientX: r.left + x, clientY: r.top + 10 }));
ev("pointermove", 506);
if (%d !== 0) {
  ev("pointerdown", 506);
  ev("pointermove", 506 + %d / 2);
  ev("pointermove", 506 + %d);
}
"""


def repeat_handle(dx):
    return "js:" + REPEAT_HANDLE % (dx, dx, dx)


# Right-clicks 「Aメロ」 and presses 「くり返す…」 (the box opens at its end with 3); then the given lines (Enter, for the answer).
REPEAT_BOX = """
const lanes = document.querySelector(".lanes");
const r = lanes.getBoundingClientRect();
lanes.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, button: 2, clientX: r.left + 400, clientY: r.top + 30 }));
await new Promise((r) => setTimeout(r, 100));
[...document.querySelectorAll('[data-test="context-menu"] button')].find((b) => b.textContent.includes("くり返す")).click();
await new Promise((r) => setTimeout(r, 200));
%s
"""
PRESS_ENTER_IN_REPEAT_BOX = """
document.querySelector('[data-test="repeat-count-field"]').dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true, cancelable: true }));
await new Promise((r) => setTimeout(r, 300));
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


# Presses the first automation lane at a song place (bar, beat) and moves by (beats, dy) without letting go: on a point
# that drags it (R10-AutoEdit 「動かしている」), on an empty place that shows the reading of the point it would add
# (「書いている」), and with box=True from an empty place it draws the selection box (「囲んでえらんだ」).
DRAG_LANE = """
// The takes and the lane's points come a moment after the page settles; wait for them (as TYPE_MARKER).
await new Promise((r) => setTimeout(r, 5000));
const lane = document.querySelector('[data-test="auto-lane"]');
const r = lane.getBoundingClientRect();
const bars = [...document.querySelectorAll(".bar-number")];
const xOf = (n) => parseFloat(bars.find((b) => b.textContent === String(n)).style.left);
const barPx = xOf(2) - xOf(1);
const x0 = xOf(1) + barPx * (%d - 1 + (%d - 1) / 4);
const y0 = %s;
const ev = (type, x, y) => lane.dispatchEvent(new PointerEvent(type, { bubbles: true, button: 0, pointerId: 1, clientX: r.left + x, clientY: r.top + y }));
ev("pointerdown", x0, y0);
if (%f !== 0 || %d !== 0) {
  ev("pointermove", x0 + (barPx / 4) * %f / 2, y0 + %d / 2);
  ev("pointermove", x0 + (barPx / 4) * %f, y0 + %d);
}
"""


def drag_lane(bar, beat, y, beats=0.0, dy=0):
    return "js:" + DRAG_LANE % (bar, beat, y, beats, dy, beats, dy, beats, dy)


def drag_tempo(x, y, dx, dy):
    return "js:" + DRAG_TEMPO % (y, x, x, dx, dy, x, dx, dy)


# Opens the second row under the first licence heading of the about dialog and scrolls it to the top.
OPEN_PART = """
const row = document.querySelectorAll('[data-test="about-group"] ~ [data-test="about-part"]')[1];
row.click();
await new Promise((r) => setTimeout(r, 300));
row.previousElementSibling.previousElementSibling.scrollIntoView({ block: "start" });
"""

# Right-clicks the second meter flag (bar 9 of the scene "tempo").
CONTEXT_FLAG = """
const flag = document.querySelectorAll('[data-test="meter-flag"]')[1];
const r = flag.getBoundingClientRect();
flag.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: r.left + 4, clientY: r.bottom }));
"""

# Sends a pointer event (pointerenter: on it, pointerdown: pressed) to a row of the history band, by its number.
HISTORY_ROW = """
const row = document.querySelector('[data-test="history-list"] .row[data-index="%d"]');
row.dispatchEvent(new PointerEvent("%s", { bubbles: true, button: 0 }));
"""

# Puts the keys on the history band and presses these keys.
HISTORY_KEYS = """
const list = document.querySelector('[data-test="history-list"]');
list.focus();
for (const key of %s) list.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
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
    # Opens 「長さをそろえる」 and 「ベロシティをまとめて変える」 (the rows that change many notes at once).
    "inspectorBulk": ("pianoRoll", ('[data-test="inspector-same-length"]', '[data-test="inspector-velocity-all"]')),
    # P6 (round 12): the list of 「進行から並べる ▾」.
    "progressionMenu": ("pianoRoll", '[data-test="pr-progression-open"]'),
    # 「CC の一覧」 opened (R5-Lists).
    "ccList": ("pianoRollCc", '[data-test="pr-cc-list"]'),
    # Folds the sustain heading with a click.
    "ccListFolded": ("pianoRollCc", ('[data-test="pr-cc-list"]', '[data-group="cc64"]')),
    # The pointer over the master value of the slim form (「マスター ・ −∞ dB」).
    "masterTip150": ("pianoRoll150", "js:" + HOVER % '[data-test="transport-master-db"]'),
    "arrangeMoveDrag": ("arrangeTrimmed", drag_clip(300, 364)),
    "arrangeCopyDrag": ("arrangeTrimmed", drag_clip(300, 556, "{ altKey: true }")),
    "arrangeTrimDrag": ("arrangeTrimmed", drag_clip(509, 413, "{ shiftKey: true }")),
    "menuFile": ("saved", '[data-menu="0"]'),
    "menuZoom": ("muted", ('[data-menu="2"]', '[data-test="menubar-list"] [data-row="10"]')),
    "menuClip": ("muted", "js:" + CONTEXT_CLIP),
    # A5: 「ファイル ▸ 最近ひらいた曲 ▸」 with songs, and with none.
    "menuRecent": ("recentSongs", ('[data-menu="0"]', '[data-test="menubar-list"] [data-row="2"]')),
    "menuRecentEmpty": ("saved", ('[data-menu="0"]', '[data-test="menubar-list"] [data-row="2"]')),
    "menuAudioClip": ("audioEdit", "js:" + CONTEXT_AUDIO_CLIP),
    # Round 12 A1: the pointer on the repeat handle, the handle dragged by 2 copies of 3 bars, the box, and its answer.
    "arrangeRepeatHover": ("arrangeTrimmed", repeat_handle(0)),
    "arrangeRepeatDrag": ("arrangeTrimmed", repeat_handle(384)),
    "arrangeRepeatBox": ("arrangeTrimmed", "js:" + REPEAT_BOX % ""),
    "arrangeRepeated": ("arrangeTrimmed", "js:" + REPEAT_BOX % PRESS_ENTER_IN_REPEAT_BOX),
    # A clip with every note deleted, scrolled 1 bar sideways (the ruler and the grid show there too).
    "pianoRollNoNotesScrolled": ("pianoRollNoNotes", "js:" + WHEEL_ROLL % 160),
    # F4-ShelfSample: the ▶ of the grand piano pressed.
    "shelfSample": ("shelfNewTrack", '[data-row="grandPiano"] [data-test="shelf-sample"]'),
    # P4 (round 12): the shelf's 「パターン」.
    "shelfPatterns": ("shelfPatterns", '[data-test="shelf-patterns"]'),
    "shelfPatternsNewTrack": ("shelfPatternsNewTrack", '[data-test="shelf-patterns"]'),
    # R8-Shelf84: folds every heading but 「ピアノ ・ 鍵盤」 and 「ギター」.
    "shelf84": ("shelf84", "js:" + FOLD_SHELF % '["bass", "drums", "padStrings", "wind", "synth"]'),
    "trackRename": ("tracks6", "js:" + RENAME_TRACK),
    "trackDrag": ("tracks6", "js:" + DRAG_TRACK),
    "trackColorMenu": ("tracks6", "js:" + TRACK_COLORS),
    # Help ▸ About meloet, the open source licenses ▸, the heading of the most common license and its second row.
    "about": ("tracks6", ('[data-menu="6"]', '[data-test="menubar-list"] [data-row="3"]')),
    "aboutLicenses": ("tracks6", ('[data-menu="6"]', '[data-test="menubar-list"] [data-row="3"]', '[data-test="about-licenses"]')),
    "aboutPart": ("tracks6", ('[data-menu="6"]', '[data-test="menubar-list"] [data-row="3"]', '[data-test="about-licenses"]',
                               '[data-test="about-group"]', "js:" + OPEN_PART)),
    # Help ▸ the key list, and the same with 「ループ」 typed in its filter box.
    "keyList": ("tracks6", ('[data-menu="6"]', '[data-test="menubar-list"] [data-row="0"]')),
    "keyListFiltered": ("tracks6", ('[data-menu="6"]', '[data-test="menubar-list"] [data-row="0"]', "js:" + TYPE_KEY_FILTER % "ループ")),
    # 「meloet について」 with the line of a newer version (round 12, D1).
    "aboutNewVersion": ("newVersion", ('[data-menu="6"]', '[data-test="menubar-list"] [data-row="3"]')),
    "settingsAdv": ("settings", '[data-test="settings-advanced"]'),
    "settingsMoved": ("settings", drag_dialog('[data-test="settings-dialog"]', -200, -40)),
    "scanStoppedMoved": ("scanStopped", drag_dialog('[data-test="scan-stopped-dialog"]', 260, 120)),
    # Opens the shelf's headings as in the mock (all are folded at first; spec 3 Y12).
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
    # Typing a marker's name, and a marker being moved by 1 bar (R10-Markers).
    "markerRenaming": ("markerRenaming", "js:" + TYPE_MARKER % "Aメロ ・ 1 回目"),
    "markerMoving": ("markerMoving", "js:" + DRAG_MARKER % ("Aメロ", 64, 64)),
    # The point of bar 9 (120.00) being moved 1 beat right and a little up (an accent frame where it was; the readout is the position).
    "tempoDrag": ("tempo", drag_tempo(512, 12, 16, -6)),
    # The three of R10-AutoEdit: the point of bar 3 (−2.1 dB) moved 1 beat right and up (−0.6 dB); the empty place at
    # 5. 3. 000 held down; a box from the end of bar 2 to bar 6. A point's height is its position in the lane. The lane's
    # height is the points' 0.45 to 0.73 with a tenth more above and below (0.422 to 0.758, spec 3 21-B), so
    # 4 + (1 − (position − 0.422) / 0.336) × 39.
    "autoMove": ("autoMove", drag_lane(3, 1, "4 + (1 - (0.73 - 0.422) / 0.336) * 39", 1, -4)),
    "autoPress": ("autoMove", drag_lane(5, 3, "4 + (1 - (0.52 - 0.422) / 0.336) * 39")),
    "autoBoxDrag": ("autoMove", drag_lane(2, 3, "4", 14, 40)),
    # R10-History: the pointer over step 11, a click on it, and ↓ ↓ with the focus in the band (from step 13, the current one, to 11).
    "historyPointer": ("historyUndone3", "js:" + HISTORY_ROW % (11, "pointerenter")),
    "historyPicked": ("historyUndone3", "js:" + HISTORY_ROW % (11, "pointerdown")),
    "historyKeys": ("historyUndone3", "js:" + HISTORY_KEYS % '["ArrowDown", "ArrowDown"]'),
    # Opens the list of 「＋ 帯を足す ▾」. Right-clicks the meter flag of bar 9.
    "answers92": ("laneScale", '[data-test="lane-add"]'),
    "meterFlagMenu": ("tempo", "js:" + CONTEXT_FLAG),
    "vocalInputMenu": ("vocalTrack", '[data-test="track-input"]'),
    "metronomeWindow": ("metronome", '[data-test="transport-metronome-stub"]'),
    "presetsMenu": ("presets", '[data-test="preset-button"]'),
    "presetsSaving": ("presets", ('[data-test="preset-button"]', '[data-test="preset-save"]')),
    # P2 (round 12): 「ほかのトラックも見る」 on, then its list of tracks (the label shows at 1920x1080 100 %). Folded: where the label does not fit
    # (1280x800), only the ▾ is left and the switch is the first row of its list.
    "pianoRollGhost": ("pianoRollGhost", '[data-test="pr-ghost"]'),
    "pianoRollGhostMenu": ("pianoRollGhost", ('[data-test="pr-ghost"]', '[data-test="pr-ghost-menu"]')),
    "pianoRollGhostFolded": ("pianoRollGhost", ('[data-test="pr-ghost-menu"]', '[data-test="pr-ghost-tracks"] [data-row="0"]')),
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
    "shelfPatterns": [(1280, 800, 100), (1280, 800, 150)],
    "shelfPatternsNewTrack": [(1280, 800, 100)],
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
    "scaleHighlight": [(1280, 800, 100), (1280, 800, 150)],
    "scaleHighlightLocked": [(1280, 800, 100)],
    "pianoRollGhost": [(1920, 1080, 100)],
    "pianoRollGhostMenu": [(1920, 1080, 100)],
    "pianoRollGhostFolded": [(1280, 800, 100), (1280, 800, 150)],
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
    "menuRecent": [(1280, 800, 100), (1280, 800, 150)],
    "menuRecentEmpty": [(1280, 800, 100)],
    "menuAudioClip": [(1280, 800, 100)],
    "audioNormalized": [(1280, 800, 100), (853, 448, 100)],
    "arrangeRepeatHover": [(1280, 800, 100)],
    "arrangeRepeatDrag": [(1280, 800, 100)],
    "arrangeRepeatBox": [(1280, 800, 100)],
    "arrangeRepeated": [(1280, 800, 100)],
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
    "keyList": [(1280, 800, 100), (1280, 800, 150)],
    "keyListFiltered": [(1280, 800, 100)],
    "aboutNewVersion": [(1280, 800, 100), (1280, 800, 150)],
    # The mixer (F4-Mixer is 1280x800 at 100 %). 150 % is to see that it fits.
    "mixer": [(1280, 800, 100), (1280, 800, 150), (1920, 1080, 100)],
    "paramsDelay": [(1280, 800, 100), (1280, 800, 150)],
    "shelfEffects": [(1280, 800, 100), (1920, 1080, 150)],
    "shelf84": [(1280, 800, 100), (1280, 800, 150), (1920, 1080, 100)],
    # The scan. R9-Scan and F5-ScanStopped are 1280x720 and 1280x800 at 100 %. 150 % is to see that it fits.
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
    "autoPress": [(1280, 800, 100)],
    "autoBoxDrag": [(1280, 800, 100)],
    "historyList": [(1280, 800, 100), (1280, 800, 150)],
    "historyUndone3": [(1280, 800, 100)],
    "historyPointer": [(1280, 800, 100)],
    "historyPicked": [(1280, 800, 100)],
    "historyKeys": [(1280, 800, 100), (1280, 800, 150)],
    "history5000": [(1280, 800, 100)],
    "laneScale": [(1280, 800, 100), (1280, 800, 150)],
    "answers92": [(1280, 800, 100), (1280, 800, 150)],
    "historyLatest": [(1280, 800, 100)],
    "historyAllUndone": [(1280, 800, 100)],
    "historyEmpty": [(1280, 800, 100)],
    "headTempo": [(1280, 800, 100)],
    "pianoRollMarker": [(1280, 800, 100), (1280, 800, 150)],
    "meterFlagMenu": [(1280, 800, 100)],
    "vocalTrack": [(1280, 800, 100), (1280, 800, 150)],
    "vocalInputMenu": [(1280, 800, 100)],
    "vocalNoInput": [(1280, 800, 100)],
    "vocalMonitorOn": [(1280, 800, 100)],
    "vocalTooLoud": [(1280, 800, 100)],
    "vocalNoPianoRoll": [(1280, 800, 100)],
    "vocalRec": [(1280, 800, 100), (1280, 800, 150)],
    "twoRec": [(1280, 800, 100), (1280, 800, 150)],
    "settingsAdv7": [(1280, 800, 100), (1280, 800, 150)],
    "latency": [(1280, 800, 100), (1280, 800, 150)],
    "latencyDone": [(1280, 800, 100)],
    "latencyFailed": [(1280, 800, 100)],
    "vocalFit": [(853, 448, 100), (1280, 800, 150)],
    "paramsSound": [(1280, 800, 100), (1280, 800, 150)],
    "exportDialog": [(1280, 800, 100), (1280, 800, 150)],
    "exportFit": [(853, 448, 100), (1280, 800, 150)],
    "packAsk": [(1280, 800, 100), (1280, 800, 150), (853, 448, 100)],
    "packGet": [(1280, 800, 100), (1280, 800, 150), (853, 448, 100)],
    "packFailOffline": [(1280, 800, 100), (1280, 800, 150), (853, 448, 100)],
    "packSettings": [(1280, 800, 100), (1280, 800, 150)],
    "exporting": [(1280, 800, 100), (1280, 800, 150)],
    # At 150 % the count-in button is hidden, so the window has the count-in row too.
    "metronomeWindow": [(1280, 800, 100), (1280, 800, 150)],
    "presets": [(1280, 800, 100)],
    "presetsMenu": [(1280, 800, 100), (1280, 800, 150)],
    "presetsSaving": [(1280, 800, 100)],
    "presetsNone": [(1280, 800, 100)],
}
scenes = args[1:] or list(BOARDS)

# Drives headless Chrome over the DevTools protocol (Node's own WebSocket): open, click, shoot.
# Every Chrome here gets --password-store=basic: without it, Chrome waits 11 to 25 s for the desktop keyring
# (over D-Bus) before its first request.
CLICK_SHOT = r"""
const [port, url, selectors, out, w, h, profile] = process.argv.slice(1);
const { spawn } = await import("node:child_process");
const { writeFileSync } = await import("node:fs");
const chrome = spawn("google-chrome", ["--headless=new", "--disable-gpu", "--hide-scrollbars", "--force-device-scale-factor=1", "--password-store=basic",
  `--window-size=${w},${h}`, `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, url],
  { stdio: "ignore" });
const exited = new Promise((r) => { chrome.once("exit", r); chrome.once("error", r); });
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
  // The caller deletes the profile, so Chrome must be gone first.
  await exited;
}
"""


ports_given = set()
ports_lock = threading.Lock()


def free_port():
    # Two threads must never get the same port, even if the system hands it out twice.
    with ports_lock:
        while True:
            with socket.socket() as s:
                s.bind(("127.0.0.1", 0))
                port = s.getsockname()[1]
            if port not in ports_given:
                ports_given.add(port)
                return port


def run(cmd, **kw):
    subprocess.run(cmd, check=True, **kw)


# What `npm run build` reads: the screen's sources and config, and what tools/licenses/inventory.mjs reads for
# dist/licenses/texts.json (the Rust and npm parts, licenses/). The stamp in ui/dist holds their hash.
BUILD_INPUTS = ["ui/src", "ui/public", "ui/index.html", "ui/package.json", "ui/package-lock.json", "ui/vite.config.ts",
                "ui/svelte.config.js", "ui/tsconfig.json", "tools/licenses", "licenses", "Cargo.toml", "Cargo.lock",
                "tools/sounds/build.py", "tools/sounds/sources.json"]
STAMP = os.path.join(ui, "dist", ".shoot-stamp")


def build_hash():
    files = []
    for rel in BUILD_INPUTS + [os.path.join("crates", c, "Cargo.toml") for c in sorted(os.listdir(os.path.join(root, "crates")))]:
        path = os.path.join(root, rel)
        if os.path.isdir(path):
            for d, dirs, names in os.walk(path):
                dirs.sort()
                files += [os.path.join(d, n) for n in sorted(names)]
        elif os.path.isfile(path):
            files.append(path)
    h = hashlib.sha256(f"MELOET_DEV_HOOKS={os.environ.get('MELOET_DEV_HOOKS', '')}\n".encode())
    for f in files:
        h.update(os.path.relpath(f, root).encode() + b"\0")
        with open(f, "rb") as fh:
            h.update(hashlib.sha256(fh.read()).digest())
    return h.hexdigest()


# Every child runs in its own process group, so a stop also ends the Chrome that a node script started.
running = set()
running_lock = threading.Lock()
stopping = threading.Event()


class Stopped(Exception):
    pass


def kill_group(p):
    if p.returncode is None:
        try:
            os.killpg(p.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass


def call(cmd, timeout, **kw):
    with running_lock:
        if stopping.is_set():
            raise Stopped()
        p = subprocess.Popen(cmd, start_new_session=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, **kw)
        running.add(p)
    try:
        _, err = p.communicate(timeout=timeout)
    except BaseException:
        kill_group(p)
        p.communicate()
        raise
    finally:
        with running_lock:
            running.discard(p)
    if stopping.is_set():
        raise Stopped()
    return p.returncode, err


def shoot(port, theme, scene, w, h, scale):
    """Shoots one scene into shots/ and puts it next to its mock board. Returns the path of the sheet."""
    try:
        return shoot_one(port, theme, scene, w, h, scale)
    except (Stopped, RuntimeError):
        raise
    except Exception as e:
        raise RuntimeError(f"{scene}-{w}x{h}-{scale}{f'-{theme}' if theme else ''}: {e!r}") from e


def shoot_one(port, theme, scene, w, h, scale):
    theme_query = f"&theme={theme}" if theme else ""
    stem = f"{scene}-{w}x{h}-{scale}{f'-{theme}' if theme else ''}"
    shot = os.path.join(shots, stem + ".png")
    profile = tempfile.mkdtemp(prefix="shoot-")
    try:
        if scene in CLICKS:
            base, selector = CLICKS[scene]
            selectors = json.dumps([selector] if isinstance(selector, str) else list(selector))
            code, err = call(["node", "--input-type=module", "-e", CLICK_SHOT, str(free_port()),
                              f"http://127.0.0.1:{port}/?scene={base}&scale={scale}{theme_query}", selectors, shot, str(w), str(h), profile],
                             timeout=180, env=env)
            if code != 0:
                raise RuntimeError(f"{stem}: {err.strip() or code}")
        else:
            code, err = call(["google-chrome", "--headless=new", "--disable-gpu", "--hide-scrollbars",
                              "--force-device-scale-factor=1", "--virtual-time-budget=3000", "--password-store=basic",
                              f"--window-size={w},{h}", f"--screenshot={shot}", f"--user-data-dir={profile}",
                              f"http://127.0.0.1:{port}/?scene={scene}&scale={scale}{theme_query}"],
                             timeout=60)
            if code != 0:
                raise RuntimeError(f"{stem}: google-chrome exited with {code}: {err.strip()[-2000:]}")
    finally:
        shutil.rmtree(profile, ignore_errors=True)
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
    return os.path.join(out, stem + ".png")


started = time.time()
os.makedirs(shots, exist_ok=True)
env = dict(os.environ)
env["PATH"] = os.path.expanduser("~/.local/node/bin") + os.pathsep + env.get("PATH", "")
want = build_hash()
have = None
if os.path.isfile(STAMP):
    with open(STAMP, encoding="utf-8") as f:
        have = f.read().strip()
if have == want and not force_build:
    print("ui/dist is up to date: build skipped (--build to build anyway)")
else:
    if os.path.exists(STAMP):
        os.remove(STAMP)
    run(["npm", "run", "build"], cwd=ui, env=env, stdout=subprocess.DEVNULL)
    with open(STAMP, "w", encoding="utf-8") as f:
        f.write(want + "\n")

boards = sorted({BOARDS[s] for s in scenes if BOARDS.get(s)})
with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, jobs)) as mock_pool:
    list(mock_pool.map(lambda b: run([sys.executable, "-I", os.path.join(root, "tools", "design", "render_mocks.py"), mocks, b],
                                     stdout=subprocess.DEVNULL), boards))

tasks = [(theme, scene, w, h, scale) for theme in themes for scene in scenes
         for w, h, scale in ONLY.get(scene) or [(w, h, s) for w, h in SIZES for s in SCALES]]
port = free_port()
preview = subprocess.Popen(
    [os.path.join(ui, "node_modules", ".bin", "vite"), "preview", "--port", str(port), "--strictPort", "--host", "127.0.0.1"],
    cwd=ui, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
pool = concurrent.futures.ThreadPoolExecutor(max_workers=max(1, jobs))
try:
    for _ in range(100):
        try:
            urllib.request.urlopen(f"http://127.0.0.1:{port}/", timeout=1)
            break
        except OSError:
            time.sleep(0.1)
    else:
        sys.exit("vite preview did not start")

    futures = [pool.submit(shoot, port, *t) for t in tasks]
    index = {f: i for i, f in enumerate(futures)}
    sheets = [None] * len(tasks)
    printed = 0
    for f in concurrent.futures.as_completed(futures):
        sheets[index[f]] = f.result()
        # The paths come out in the order of a serial run.
        while printed < len(sheets) and sheets[printed] is not None:
            print(sheets[printed], flush=True)
            printed += 1
    print(f"{len(tasks)} shots in {time.time() - started:.0f} s (jobs={jobs})")
except RuntimeError as e:
    sys.exit(str(e))
finally:
    stopping.set()
    pool.shutdown(wait=False, cancel_futures=True)
    with running_lock:
        for p in running:
            kill_group(p)
    pool.shutdown(wait=True)
    preview.terminate()
    preview.wait()

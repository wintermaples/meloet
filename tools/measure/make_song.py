#!/usr/bin/env python3
"""Writes a song file for measuring: one track, one clip with one take of N notes.

usage: python3 -I tools/measure/make_song.py OUT.meloet [--notes 200] [--bars 16] [--tempo 96]
                                             [--cc 0] [--seed 1] [--tracks N]
                                             [--instruments plugin,sf2,builtin]
                                             [--scan SCAN.json] [--plugin NAME] [--plugin-format vst3|lv2]
                                             [--sf2 FILE.sf2[:BANK:PROGRAM[:NAME]]] [--effect-plugin NAME]
                                             [--effect-format vst3|lv2]
                                             [--stagger TICKS] [--hard-pan]
                                             [--tempo-points TICK:BPM[:ramp],...] [--head-ramp]
                                             [--meter-changes BAR:NUM/DEN,...]
                                             [--markers TICK:NAME,...]
                                             [--lanes volume,pan,plugin:PARAM,effect:PARAM]
                                             [--lane-points 16] [--lane-resting]

--tracks N (1..64): a version 3 song with N tracks, each with one clip and one take of --notes
notes. The instruments go round BAND (the groups mixed), with the sends a new track of that
group gets, an EQ and a compressor on some tracks, a reverb on return A, a delay on return B
and the limiter on the master (every parameter at its default). The first track keeps the ids
1, 2, 3 (track, clip, take), so `selectClip 2` works for both shapes.

--instruments (with --tracks): what each track sounds with, in the order of the tracks; the last
word goes on for the tracks after it ("plugin": every track; "plugin,builtin": the first one).
The song is then version 4. "plugin" is the instrument --plugin names and "sf2" the preset --sf2
names (the file itself has to be in meloet's SF2 folder, $MELOET_SF2_DIR or
$XDG_DATA_HOME/meloet/sf2; only its name, its sha256 and the preset go into the song). Plugins
are looked up by their name in a scan record (--scan, <XDG_CACHE_HOME>/meloet/plugins/scan.json);
the song gets the key, name, vendor and path found there and no state. --effect-plugin: that
(--effect-format: its format when it is not --plugin-format)
effect in the first free insert slot of every "plugin" and "sf2" track. --stagger: track i
starts its notes i * TICKS later (16 plugins that all start a chord in the same block are a
case of their own, docs/measurements/2026-11-19-round5-plughost.md). --hard-pan: the "plugin"
tracks all the way left, the others all the way right, no sends and no built-in inserts, so
that a recording of the output tells the two apart (plugin-stop.sh).

--tempo-points, --head-ramp, --meter-changes and --markers make the song version 5 (either
shape): tempo points at ticks (480 to a quarter note; "ramp": a straight line to the next
point; --head-ramp: from the song's head to the first point), meter changes at the heads of
bars (bars count from 0, so 8 is the bar shown as 9), markers at ticks. The notes stay where
they are in ticks: --bars still counts bars of 4/4. Example, the spec's 104 to 120 over bars
5 to 9 and 3/4 from bar 9: --tempo-points 7680:104:ramp,15360:120 --meter-changes 8:3/4

--lanes makes the song version 5 too: automation lanes on every track, in the order given.
"volume" and "pan"; "plugin:PARAM" a parameter of the instrument by the plugin's own number
(only on the "plugin" tracks); "effect:PARAM" a parameter of the --effect-plugin insert (only
on the tracks that have it). Each lane has --lane-points points spread evenly over the clip
from its start, with values (the place of the knob, 0..1) from --seed. --lane-resting: the
lanes rest (spec 3, 17-A).

Without --tracks the shape is crates/meloet-core/src/songfile.rs, version 1. The clip starts at the song's start
and is --bars bars of 4/4 long. Notes are spread over the clip in start order, keys 48..84,
lengths of a 16th to a half note (never past the clip's end), velocity 40..100. --cc puts that
many sustain (CC 64) points in the take, evenly. The same arguments always write the same file.
"""
import argparse, hashlib, json, os, random, sys

TICKS_PER_QUARTER = 480
TICKS_PER_BAR = 4 * TICKS_PER_QUARTER
TRACK, CLIP, TAKE = 1, 2, 3
LENGTHS = [120, 240, 480, 960]


# name in the file, name on the screen, group, keys, inserts.
BAND = [
    ("grandPiano", "グランドピアノ", "pianoKeys", (48, 85), ["eq4", "compressor"]),
    ("fingerBass", "フィンガーベース", "bass", (28, 53), ["compressor"]),
    ("studioKit", "スタジオキット", "drums", (36, 52), ["eq4", "compressor"]),
    ("strings", "ストリングス", "padStrings", (48, 85), ["eq4"]),
    ("electricPiano", "エレクトリックピアノ", "pianoKeys", (48, 85), []),
    ("synthLead", "シンセリード", "synth", (60, 85), ["compressor"]),
    ("warmPad", "あたたかいパッド", "padStrings", (48, 73), []),
    ("organ", "パイプオルガン", "pianoKeys", (48, 85), ["eq4", "compressor"]),
]
# meloet_model::mixer::new_track_sends: [A, B] in dB, None is -inf.
NEW_TRACK_SENDS = {"pianoKeys": -15.0, "guitar": -15.0, "padStrings": -15.0, "wind": -15.0, "synth": -15.0,
                   "drums": -24.0, "bass": None}
INSERT_SLOTS = 4
TRACK_COLORS = 5
PANS = [0, -20, 0, 30, -40, 20, -10, 40]


def take_notes(rng, notes, length, keys):
    made = []
    for i in range(notes):
        start = (i * length) // notes + rng.randrange(0, 60)
        start = min(start, length - 1)
        note_len = min(rng.choice(LENGTHS), length - start)
        made.append([start, note_len, rng.randrange(*keys), rng.randrange(40, 101)])
    made.sort(key=lambda n: (n[0], n[2]))
    return made


def slots(effects):
    made = [{"effect": e, "params": {}} for e in effects]
    return made + [None] * (INSERT_SLOTS - len(made))


def scanned_plugin(scan, name, fmt, kind):
    """The fields a song keeps of the plugin called `name` in the scan record at `scan`."""
    with open(scan, encoding="utf-8") as f:
        record = json.load(f)
    for bundle in record["bundles"]:
        for plugin in bundle["plugins"]:
            if (plugin["name"] == name and plugin["key"].startswith(fmt + ":")
                    and plugin["kind"]["role"] == kind):
                return {"key": plugin["key"], "name": plugin["name"], "vendor": plugin["vendor"],
                        "path": bundle["path"]}
    sys.exit(f"make_song.py: no {fmt} {kind} named {name!r} in {scan}")


def sf2_preset(text):
    """--sf2 FILE[:BANK:PROGRAM[:NAME]] as the fields a song keeps."""
    parts = text.split(":")
    path = parts[0]
    bank, program = (int(parts[1]), int(parts[2])) if len(parts) >= 3 else (0, 0)
    name = parts[3] if len(parts) >= 4 else f"{bank}:{program}"
    digest = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            digest.update(chunk)
    return {"file": os.path.basename(path), "sha256": digest.hexdigest(), "bank": bank,
            "program": program, "name": name}


def band_song(tracks, notes, bars, tempo, cc, seed, instruments=(), plugin=None, sf2=None,
              effect=None, stagger=0, hard_pan=False):
    """Version 3: `tracks` tracks, ids 1 + 3 i (track), 2 + 3 i (clip), 3 + 3 i (take).

    Version 4 when `instruments` (the words of --instruments) names what the tracks sound with.
    """
    length = bars * TICKS_PER_BAR
    controls = [[(i * length) // max(cc, 1), 64, 127 if i % 2 == 0 else 0] for i in range(cc)]
    track_list, takes = [], []
    for i in range(tracks):
        file_name, name, group, keys, effects = BAND[i % len(BAND)]
        track, clip, take = 1 + 3 * i, 2 + 3 * i, 3 + 3 * i
        word = instruments[min(i, len(instruments) - 1)] if instruments else "builtin"
        instrument = {"kind": "builtin", "name": file_name}
        inserts = slots([] if hard_pan else effects)
        if word == "plugin":
            instrument, name = {"kind": "plugin", **plugin}, plugin["name"]
        elif word == "sf2":
            instrument, name = {"kind": "sf2", **sf2}, sf2["name"]
        if word != "builtin":
            if group in ("drums", "bass"):
                keys = (48, 73)
            if effect:
                inserts[inserts.index(None)] = {"plugin": dict(effect)}
        made = take_notes(random.Random(seed * 1000 + i), notes, length, keys)
        if stagger:
            made = [[min(start + i * stagger, length - 1), n_len, key, vel] for start, n_len, key, vel in made]
            made = [[start, min(n_len, length - start), key, vel] for start, n_len, key, vel in made]
        takes.append({"id": take, "length": length, "notes": made, "controls": controls if i == 0 else []})
        track_list.append({
            "id": track,
            "name": (name if i < len(BAND) else f"{name} {i // len(BAND) + 1}") if word == "builtin"
            else f"{name} {i + 1}",
            "color": i % TRACK_COLORS + 1,
            "instrument": instrument,
            "gainDb": -9.0,
            "pan": (-100 if word == "plugin" else 100) if hard_pan else PANS[i % len(PANS)],
            "sends": {"a": None if hard_pan else NEW_TRACK_SENDS[group], "b": None},
            "inserts": inserts,
            "clips": [{"id": clip, "name": name, "start": 0, "end": length, "active": 0,
                       "takes": [{"take": take, "offset": 0}]}],
            "plugins": [],
            "automation": [],
        })
    return {
        "format": "meloet-song",
        "version": 4 if instruments else 3,
        "app": "tools/measure/make_song.py",
        "song": {
            "name": f"測る曲 {tracks} トラック {notes} ノート",
            "tempo": round(tempo * 100),
            "meter": [4, 4],
            "tempoChanges": [],
            "nextId": 3 * tracks + 1,
            "tracks": track_list,
            "mixer": {
                "returns": {"a": {"gainDb": 0.0, "pan": 0, "inserts": slots(["reverb"])},
                            "b": {"gainDb": 0.0, "pan": 0, "inserts": slots(["delay"])}},
                "master": {"gainDb": 0.0, "pan": 0, "inserts": slots(["limiter"])},
            },
            "audio": [],
        },
        "takes": takes,
        "session": {"playTarget": TRACK, "loop": None, "loopOn": False},
    }


def song(notes, bars, tempo, cc, seed):
    rng = random.Random(seed)
    length = bars * TICKS_PER_BAR
    made = take_notes(rng, notes, length, (48, 85))
    controls = []
    for i in range(cc):
        controls.append([(i * length) // max(cc, 1), 64, 127 if i % 2 == 0 else 0])
    return {
        "format": "meloet-song",
        "version": 1,
        "app": "tools/measure/make_song.py",
        "song": {
            "name": f"測る曲 {notes} ノート",
            "tempo": round(tempo * 100),
            "meter": [4, 4],
            "tempoChanges": [],
            "nextId": TAKE + 1,
            "tracks": [{
                "id": TRACK,
                "name": "ピアノ",
                "color": 1,
                "instrument": {"kind": "placeholder"},
                "gainDb": 0.0,
                "clips": [{
                    "id": CLIP,
                    "name": "ピアノ",
                    "start": 0,
                    "end": length,
                    "active": 0,
                    "takes": [{"take": TAKE, "offset": 0}],
                }],
                "plugins": [],
                "automation": [],
            }],
            "audio": [],
        },
        "takes": [{"id": TAKE, "length": length, "notes": made, "controls": controls}],
        "session": {"playTarget": TRACK, "armed": TRACK, "loop": None, "loopOn": False},
    }


def tempo_points(text):
    points = []
    for word in text.split(","):
        parts = word.split(":")
        if len(parts) not in (2, 3) or (len(parts) == 3 and parts[2] != "ramp"):
            raise ValueError(f"tempo point {word!r}: TICK:BPM or TICK:BPM:ramp")
        tick, tempo = int(parts[0]), round(float(parts[1]) * 100)
        if tick < 1 or not 3000 <= tempo <= 30000:
            raise ValueError(f"tempo point {word!r}: tick 1 or more, tempo 30..300")
        points.append({"tick": tick, "tempo": tempo, "ramp": len(parts) == 3})
    points.sort(key=lambda p: p["tick"])
    if any(a["tick"] == b["tick"] for a, b in zip(points, points[1:])):
        raise ValueError("two tempo points at one tick")
    return points


def meter_changes(text):
    changes = []
    for word in text.split(","):
        bar, _, meter = word.partition(":")
        num, _, den = meter.partition("/")
        bar, num, den = int(bar), int(num), int(den)
        if bar < 1 or not 1 <= num <= 12 or den not in (2, 4, 8, 16):
            raise ValueError(f"meter change {word!r}: bar 1 or more, NUM 1..12, DEN 2, 4, 8 or 16")
        changes.append({"bar": bar, "meter": [num, den]})
    changes.sort(key=lambda c: c["bar"])
    if any(a["bar"] == b["bar"] for a, b in zip(changes, changes[1:])):
        raise ValueError("two meter changes at one bar")
    return changes


def markers(text, first_id):
    made = []
    for word in text.split(","):
        tick, sep, name = word.partition(":")
        if not sep or not name.strip() or int(tick) < 0:
            raise ValueError(f"marker {word!r}: TICK:NAME")
        made.append({"tick": int(tick), "name": name.strip()})
    made.sort(key=lambda m: m["tick"])
    return [{"id": first_id + i, **m} for i, m in enumerate(made)]


def with_timeline(made, points, head_ramp, changes, marks):
    """The song as version 5 (crates/meloet-core/src/songfile.rs): the loop's range is in the
    song, and the seat `tempoChanges` of the older versions is gone."""
    doc, session = made["song"], made["session"]
    made["version"] = 5
    del doc["tempoChanges"]
    doc["headRamp"] = head_ramp
    doc["tempoPoints"] = points
    doc["meterChanges"] = changes
    doc["markers"] = markers(marks, doc["nextId"]) if marks else []
    doc["nextId"] += len(doc["markers"])
    doc["loop"] = session.pop("loop")
    return made


def lane_words(text):
    words = []
    for word in text.split(","):
        kind, sep, param = word.partition(":")
        if kind in ("volume", "pan") and not sep:
            words.append((kind, None))
        elif kind in ("plugin", "effect") and sep and param.isdigit():
            words.append((kind, int(param)))
        else:
            raise ValueError(f"lane {word!r}: volume, pan, plugin:PARAM or effect:PARAM")
    if len(set(words)) != len(words):
        raise ValueError("two lanes of one target")
    return words


def with_lanes(made, words, count, resting, seed):
    """The automation lanes (crates/meloet-core/src/songfile.rs, `LaneFile`). An insert a lane
    names gets an id of its own, from `nextId`."""
    doc = made["song"]
    for i, track in enumerate(doc["tracks"]):
        rng = random.Random(seed * 7919 + i)
        length = max((c["end"] for c in track["clips"]), default=TICKS_PER_BAR)
        lanes = []
        for kind, param in words:
            if kind == "volume":
                target = {"kind": "volume"}
            elif kind == "pan":
                target = {"kind": "pan"}
            elif kind == "plugin":
                if track["instrument"].get("kind") != "plugin":
                    continue
                target = {"kind": "instrument", "param": param}
            else:
                held = [s for s in track.get("inserts") or [] if s and "plugin" in s]
                if not held:
                    continue
                if "id" not in held[0]:
                    held[0]["id"] = doc["nextId"]
                    doc["nextId"] += 1
                target = {"kind": "insert", "insert": held[0]["id"], "param": param}
            points = [[(n * length) // max(count, 1), round(rng.random(), 3)] for n in range(count)]
            lanes.append({"target": target, "step": False, "resting": resting, "points": points})
        track["automation"] = lanes
    return made


def main(argv):
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("out")
    p.add_argument("--notes", type=int, default=200)
    p.add_argument("--bars", type=int, default=16)
    p.add_argument("--tempo", type=float, default=96.0)
    p.add_argument("--cc", type=int, default=0)
    p.add_argument("--seed", type=int, default=1)
    p.add_argument("--tracks", type=int)
    p.add_argument("--instruments")
    p.add_argument("--scan")
    p.add_argument("--plugin")
    p.add_argument("--plugin-format", default="vst3", choices=["vst3", "lv2"])
    p.add_argument("--sf2")
    p.add_argument("--effect-plugin")
    p.add_argument("--effect-format", choices=["vst3", "lv2"])
    p.add_argument("--stagger", type=int, default=0)
    p.add_argument("--hard-pan", action="store_true")
    p.add_argument("--tempo-points")
    p.add_argument("--head-ramp", action="store_true")
    p.add_argument("--meter-changes")
    p.add_argument("--markers")
    p.add_argument("--lanes")
    p.add_argument("--lane-points", type=int, default=16)
    p.add_argument("--lane-resting", action="store_true")
    a = p.parse_args(argv)
    if a.lane_points < 0:
        p.error("lane-points >= 0")
    if a.notes < 0 or a.bars < 1 or a.cc < 0 or not 30 <= a.tempo <= 300:
        p.error("notes >= 0, bars >= 1, cc >= 0, tempo 30..300")
    if a.tracks is not None and not 1 <= a.tracks <= 64:
        p.error("tracks 1..64")
    words = a.instruments.split(",") if a.instruments else []
    if any(w not in ("plugin", "sf2", "builtin") for w in words):
        p.error("--instruments: plugin, sf2 or builtin, with commas between")
    if (words or a.effect_plugin or a.stagger or a.hard_pan) and a.tracks is None:
        p.error("--instruments, --effect-plugin, --stagger and --hard-pan need --tracks")
    if a.effect_plugin and not words:
        p.error("--effect-plugin needs --instruments")
    if "plugin" in words and not a.plugin or "sf2" in words and not a.sf2:
        p.error("--instruments plugin needs --plugin, sf2 needs --sf2")
    if (a.plugin or a.effect_plugin) and not a.scan:
        p.error("--plugin and --effect-plugin need --scan")
    if a.stagger < 0:
        p.error("stagger >= 0")
    if a.tracks is None:
        made = song(a.notes, a.bars, a.tempo, a.cc, a.seed)
    else:
        made = band_song(
            a.tracks, a.notes, a.bars, a.tempo, a.cc, a.seed, words,
            scanned_plugin(a.scan, a.plugin, a.plugin_format, "instrument") if a.plugin else None,
            sf2_preset(a.sf2) if a.sf2 else None,
            scanned_plugin(a.scan, a.effect_plugin, a.effect_format or a.plugin_format, "effect") if a.effect_plugin else None,
            a.stagger, a.hard_pan)
    if a.tempo_points or a.head_ramp or a.meter_changes or a.markers or a.lanes:
        try:
            made = with_timeline(
                made,
                tempo_points(a.tempo_points) if a.tempo_points else [],
                a.head_ramp,
                meter_changes(a.meter_changes) if a.meter_changes else [],
                a.markers)
            if a.lanes:
                made = with_lanes(made, lane_words(a.lanes), a.lane_points, a.lane_resting, a.seed)
        except ValueError as e:
            p.error(str(e))
    data = json.dumps(made, ensure_ascii=False, separators=(",", ":"))
    tmp = a.out + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(data + "\n")
    os.replace(tmp, a.out)
    print(a.out)


if __name__ == "__main__":
    main(sys.argv[1:])

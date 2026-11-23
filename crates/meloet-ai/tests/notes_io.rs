//! Many notes with little effort: what write_notes says of notes it left out, the short
//! form of a note, clips of a given length, MIDI files in and out, and copies of a clip's
//! notes.

mod common;
use common::*;

use std::path::Path;
use std::time::Instant;

use meloet_core::ai::{AiAnswer, AiAsk, AiNote, AiPlace};
use meloet_model::ClipId;
use serde_json::{Value, json};

const Q: u64 = 1920;
const BAR: u64 = 4 * Q;

/// The notes a clip shows, as (start in the song, length, key, velocity).
fn notes_of(r: &Running, clip: u64) -> Vec<(i64, u64, u8, u8)> {
    let song = r.core.ai_song().unwrap().song;
    let (_, c) = song.doc.clip(ClipId(clip as u32)).expect("the clip");
    let shown = c.active_take();
    let zero = c.zero(shown);
    (shown.take.notes.iter())
        .map(|n| (zero + n.start as i64, n.length, n.key, n.velocity))
        .collect()
}

fn range_of(r: &Running, clip: u64) -> (u64, u64) {
    let song = r.core.ai_song().unwrap().song;
    let (_, c) = song.doc.clip(ClipId(clip as u32)).expect("the clip");
    (c.start, c.end)
}

fn id(value: &Value) -> u64 {
    value["clip"].as_u64().expect("a clip id")
}

/// 266 notes as the owner sent them: 262 short ones over four bars, one that runs past
/// the end of bar 4 (index 262), an 8-tick note lying under the next note of its key
/// (263 and 264), and the last at the head of bar 5 (265).
fn the_owners_notes() -> Vec<Value> {
    let mut notes: Vec<Value> = (0..262u64)
        .map(|i| json!([i * 116, 100, 36 + i % 24, 90]))
        .collect();
    notes.push(json!([4 * BAR - 100, Q, 70, 90]));
    notes.push(json!([4 * Q, 8, 60, 100]));
    notes.push(json!([4 * Q + 4, Q, 60, 100]));
    notes.push(json!([4 * BAR, Q, 62, 100]));
    notes
}

#[test]
fn a_note_not_written_comes_back_with_the_reason() {
    let r = running("why");
    let mut c = Client::start(&r.socket);
    let notes = the_owners_notes();
    assert_eq!(notes.len(), 266);

    // Into a track with no clip: a 4-bar clip is made, and the note at its end is left out.
    let before = steps(&r);
    let wrote = ok(&mut c, "write_notes", json!({ "track": 1, "notes": notes }));
    assert_eq!(steps(&r), before + 1);
    assert_eq!(wrote["written"], 265, "{wrote}");
    assert_eq!(wrote["not_written_count"], 1);
    assert_eq!(wrote["not_written_more"], 0);
    assert_eq!(
        wrote["not_written"],
        json!([{ "index": 265, "start": 4 * BAR, "length": Q, "key": 62,
                 "reason": "past_new_clip_end" }])
    );
    assert_eq!(
        wrote["changed"],
        json!([{ "index": 262, "start": 4 * BAR - 100, "key": 70, "length_asked": Q,
                 "length": 100, "reason": "cut_at_new_clip_end" }])
    );
    let clip = id(&wrote);
    assert_eq!(
        wrote["created_clip"],
        json!({ "id": clip, "track": 1,
                "start": { "at": "1.1.0", "tick": 0 },
                "end": { "at": "5.1.0", "tick": 4 * BAR } })
    );
    assert_eq!(wrote["grew_to"], Value::Null);
    assert_eq!(wrote["step"]["name"]["kind"], "writeNote");
    // The 8-tick note under the next note of its key is there: both are written.
    let got = notes_of(&r, clip);
    assert_eq!(got.len(), 265);
    assert!(got.contains(&(4 * Q as i64, 8, 60, 100)));
    assert!(got.contains(&(4 * Q as i64 + 4, Q, 60, 100)));
    ok(&mut c, "undo", json!({}));

    // Into a clip made long enough: every note is written, none changed.
    let made = ok(
        &mut c,
        "create_clip",
        json!({ "track": 1, "at": "1.1.0", "bars": 8 }),
    );
    let wrote = ok(
        &mut c,
        "write_notes",
        json!({ "clip": id(&made), "notes": notes }),
    );
    assert_eq!(wrote["written"], 266, "{wrote}");
    assert_eq!(wrote["not_written"], json!([]));
    assert_eq!(wrote["changed"], json!([]));
    assert_eq!(wrote["created_clip"], Value::Null);
    assert_eq!(wrote["grew_to"], Value::Null);
    assert_eq!(wrote["clip_place"]["end"]["at"], "9.1.0");
    assert_eq!(notes_of(&r, id(&made)).len(), 266);

    // Past the clip's end it grows, and the answer says to where.
    let wrote = ok(
        &mut c,
        "write_notes",
        json!({ "clip": id(&made), "notes": [["10.2.0", 960, 60]] }),
    );
    assert_eq!(wrote["grew_to"]["end"]["at"], "11.1.0", "{wrote}");
    assert_eq!(wrote["grew_to"]["start"]["tick"], 0);

    // A clip that would grow into another: nothing is written, and the clip is named.
    let other = ok(
        &mut c,
        "write_notes",
        json!({ "track": 1, "notes": [["13.1.0", 960, 60]] }),
    );
    assert_eq!(other["created_clip"]["start"]["at"], "13.1.0");
    let now = steps(&r);
    let said = refused(
        &mut c,
        "write_notes",
        json!({ "clip": id(&made), "notes": [["12.4.0", 3840, 60]] }),
    );
    assert!(
        said.contains(&format!(
            "clip {} of the track (13.1.0 to 17.1.0)",
            id(&other)
        )),
        "{said}"
    );
    assert_eq!(steps(&r), now);

    // More than 200 left out: the first 200 and how many more.
    let late: Vec<Value> = (0..300u64).map(|i| json!([20 * BAR + i, 10, 60])).collect();
    let mut notes = vec![json!(["17.1.0", 10, 60])];
    notes.extend(late);
    let wrote = ok(&mut c, "write_notes", json!({ "track": 1, "notes": notes }));
    assert_eq!(wrote["written"], 1, "{wrote}");
    assert_eq!(wrote["not_written_count"], 300);
    assert_eq!(wrote["not_written_more"], 100);
    assert_eq!(wrote["not_written"].as_array().unwrap().len(), 200);
    assert_eq!(wrote["not_written"][199]["index"], 200);
}

#[test]
fn a_note_is_an_object_or_an_array() {
    let r = running("short");
    let mut c = Client::start(&r.socket);
    let list = c.request("tools/list", json!({}), &mut Vec::new());
    let tools = list["result"]["tools"].as_array().unwrap();
    let write = tools.iter().find(|t| t["name"] == "write_notes").unwrap();
    let schema = &write["inputSchema"];
    assert_eq!(schema["type"], "object", "{schema}");
    let forms = schema["properties"]["notes"]["items"]["anyOf"]
        .as_array()
        .unwrap_or_else(|| panic!("{schema}"));
    assert_eq!(forms[0]["$ref"], "#/$defs/NoteArgs", "{schema}");
    assert_eq!(forms[1]["type"], "array");
    let object = &schema["$defs"]["NoteArgs"];
    assert_eq!(object["required"], json!(["length", "key", "velocity"]));
    assert_eq!(object["properties"]["key"]["maximum"], 127);

    let wrote = ok(
        &mut c,
        "write_notes",
        json!({ "track": 1, "notes": [
            { "at": "1.1.0", "length": 960, "key": 60, "velocity": 90 },
            { "start": 1920, "length": 960, "key": 62, "velocity": 91 },
            [3840, 960, 64, 92],
            ["1.4.0", 960, 65],
        ] }),
    );
    let clip = id(&wrote);
    assert_eq!(
        notes_of(&r, clip),
        [
            (0, 960, 60, 90),
            (1920, 960, 62, 91),
            (3840, 960, 64, 92),
            (5760, 960, 65, 100)
        ]
    );

    // With an origin the starts count from it.
    let wrote = ok(
        &mut c,
        "write_notes",
        json!({ "clip": clip, "origin": "3.1.0", "notes": [[0, 100, 40], [480, 100, 41, 7]] }),
    );
    assert_eq!(wrote["written"], 2);
    let got = notes_of(&r, clip);
    assert_eq!(
        got[4..],
        [
            (2 * BAR as i64, 100, 40, 100),
            (2 * BAR as i64 + 480, 100, 41, 7)
        ]
    );
    let wrote = ok(
        &mut c,
        "write_notes",
        json!({ "clip": clip, "origin": 100, "notes": [{ "start": 5, "length": 1, "key": 1, "velocity": 1 }] }),
    );
    assert_eq!(wrote["written"], 1);
    assert_eq!(notes_of(&r, clip)[1], (105, 1, 1, 1));

    let now = steps(&r);
    for bad in [
        json!([[0, 100]]),
        json!([[0, 100, 60, 100, 1]]),
        json!([[0, 0, 60]]),
        json!([[0, 100, 128]]),
        json!([[0, 100, 60, 0]]),
        json!([[-1, 100, 60]]),
        json!([[0, 100.5, 60]]),
        json!(["1.1.0"]),
        json!([{ "start": 0, "length": 100, "key": 60 }]),
    ] {
        let answer = c.request(
            "tools/call",
            json!({ "name": "write_notes", "arguments": { "clip": clip, "notes": bad } }),
            &mut Vec::new(),
        );
        let failed = answer["error"].is_object() || answer["result"]["isError"] == true;
        assert!(failed, "{bad}: {answer}");
    }
    let said = refused(
        &mut c,
        "write_notes",
        json!({ "clip": clip, "origin": "2.1.0", "notes": [["1.1.0", 100, 60]] }),
    );
    assert!(said.contains("origin"), "{said}");
    assert_eq!(steps(&r), now);
}

#[test]
fn a_clip_is_made_as_long_as_asked() {
    let r = running("length");
    let mut c = Client::start(&r.socket);
    let before = r.core.ai_song().unwrap();

    // As before: 4 bars from the head of the bar.
    let plain = ok(&mut c, "create_clip", json!({ "track": 1, "at": "1.3.0" }));
    assert_eq!(range_of(&r, id(&plain)), (0, 4 * BAR));
    assert_eq!(plain["start"]["at"], "1.1.0", "{plain}");
    assert_eq!(plain["end"]["at"], "5.1.0");
    assert_eq!(plain["step"]["name"]["kind"], "writeClip");

    let bars = ok(
        &mut c,
        "create_clip",
        json!({ "track": 1, "at": "9.2.0", "bars": 16 }),
    );
    assert_eq!(range_of(&r, id(&bars)), (8 * BAR, 24 * BAR));
    assert_eq!(bars["end"], json!({ "at": "25.1.0", "tick": 24 * BAR }));

    // An end inside a bar goes on to the next bar's head; the start comes after the clip
    // before.
    let end = ok(
        &mut c,
        "create_clip",
        json!({ "track": 1, "at": "5.1.0", "end": "6.2.0" }),
    );
    assert_eq!(range_of(&r, id(&end)), (4 * BAR, 6 * BAR));
    assert_eq!(steps(&r), before.undo_count + 3);

    // Another clip in the way: nothing is made, and the clip is named.
    for args in [
        json!({ "track": 1, "at": "7.1.0", "bars": 3 }),
        json!({ "track": 1, "at": "7.1.0", "end": "10.1.0" }),
        json!({ "track": 1, "at": "10.1.0" }),
        json!({ "track": 1, "at": "10.1.0", "bars": 1 }),
    ] {
        let said = refused(&mut c, "create_clip", args.clone());
        assert!(
            said.contains(&format!(
                "clip {} of the track (9.1.0 to 25.1.0)",
                id(&bars)
            )),
            "{args}: {said}"
        );
    }
    for args in [
        json!({ "track": 1, "at": "7.1.0", "bars": 0 }),
        json!({ "track": 1, "at": "7.1.0", "bars": 1, "end": "9.1.0" }),
        json!({ "track": 1, "at": "7.3.0", "end": "7.1.0" }),
        json!({ "track": 9, "at": "7.1.0", "bars": 1 }),
    ] {
        refused(&mut c, "create_clip", args);
    }
    assert_eq!(steps(&r), before.undo_count + 3);
    // Up to the clip after it is fine.
    let fits = ok(
        &mut c,
        "create_clip",
        json!({ "track": 1, "at": "7.1.0", "bars": 2 }),
    );
    assert_eq!(range_of(&r, id(&fits)), (6 * BAR, 8 * BAR));
    undo_all(&mut c, &r, &before);
}

fn quantity(out: &mut Vec<u8>, value: u32) {
    for shift in [21, 14, 7] {
        if value >> shift != 0 {
            out.push((value >> shift) as u8 & 0x7F | 0x80);
        }
    }
    out.push(value as u8 & 0x7F);
}

fn midi_file(division: u16, tracks: &[Vec<u8>]) -> Vec<u8> {
    let mut out = b"MThd\0\0\0\x06\0\x01".to_vec();
    out.extend((tracks.len() as u16).to_be_bytes());
    out.extend(division.to_be_bytes());
    for track in tracks {
        out.extend(b"MTrk");
        out.extend((track.len() as u32 + 4).to_be_bytes());
        out.extend(track);
        out.extend([0x00, 0xFF, 0x2F, 0x00]);
    }
    out
}

/// A format 1 file of 480 ticks a quarter note: a first track with a tempo and a meter, a
/// track "Bass" of 400 sixteenth notes on channel 1 (written with running status and ended
/// by velocity 0), and a track of 4 drum hits on channel 10, the last never ended.
fn four_hundred_notes() -> Vec<u8> {
    let mut first = vec![0x00, 0xFF, 0x03, 0x04, b's', b'o', b'n', b'g'];
    first.extend([0x00, 0xFF, 0x51, 0x03, 0x06, 0x8A, 0x1B]);
    first.extend([0x00, 0xFF, 0x58, 0x04, 0x03, 0x02, 0x18, 0x08]);
    let mut bass = vec![0x00, 0xFF, 0x03, 0x04, b'B', b'a', b's', b's'];
    bass.push(0x00);
    bass.push(0x90);
    for i in 0..400u32 {
        if i > 0 {
            quantity(&mut bass, 20);
        }
        bass.extend([36 + (i % 48) as u8, 1 + (i % 127) as u8]);
        quantity(&mut bass, 100);
        bass.extend([36 + (i % 48) as u8, 0]);
    }
    let mut drums = Vec::new();
    for _ in 0..4 {
        drums.extend([0x00, 0x99, 36, 100]);
        quantity(&mut drums, 480);
        drums.extend([0x89, 36, 0]);
    }
    // Without the last hit's end and the time before it.
    drums.truncate(drums.len() - 5);
    midi_file(480, &[first, bass, drums])
}

fn write_mid(dir: &Path, name: &str, bytes: &[u8]) -> String {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path.display().to_string()
}

#[test]
fn a_midi_file_of_400_notes_comes_in_with_one_call() {
    let r = running("import");
    let mut c = Client::start(&r.socket);
    let dir = scratch("import-files");
    let path = write_mid(&dir, "bass.mid", &four_hundred_notes());
    let before = r.core.ai_song().unwrap();

    let listed = ok(
        &mut c,
        "import_midi",
        json!({ "path": path, "list_only": true }),
    );
    assert_eq!(steps(&r), before.undo_count, "list_only changes nothing");
    assert_eq!(listed["format"], 1, "{listed}");
    assert_eq!(listed["ticks_per_quarter"], 480);
    assert_eq!(listed["scaled_exactly"], true);
    assert_eq!(listed["notes"], 404);
    assert_eq!(listed["tempo"], json!([{ "tick": 0, "bpm": 140.0 }]));
    assert_eq!(listed["meters"], json!([{ "tick": 0, "meter": "3/4" }]));
    assert_eq!(
        listed["tracks"][1],
        json!({ "number": 2, "name": "Bass", "channels": [1], "notes": 400,
                "lowest_key": 36, "highest_key": 83, "length_ticks": 399 * 480 + 400,
                "length_quarters": 99.96, "hanging_notes": 0 })
    );
    assert_eq!(listed["tracks"][2]["channels"], json!([10]));
    assert_eq!(listed["tracks"][2]["hanging_notes"], 1);
    assert_eq!(listed["tracks"][0]["name"], "song");

    // One call, one step, 400 notes.
    let came = ok(
        &mut c,
        "import_midi",
        json!({ "path": path, "track": 1, "at": "3.1.0", "midi_track": 2 }),
    );
    assert_eq!(steps(&r), before.undo_count + 1);
    assert_eq!(came["written"], 400, "{came}");
    assert_eq!(came["asked"], 400);
    assert_eq!(came["not_written"], json!([]));
    assert_eq!(came["step"]["name"]["kind"], "writeNote");
    assert_eq!(came["tempo_and_meter_applied"], false);
    assert_eq!(came["file"]["tempo"][0]["bpm"], 140.0);
    assert_eq!(came["origin"]["tick"], 2 * BAR);
    let clip = id(&came);
    // 399 sixteenths and 400 ticks from bar 3: the clip ends with the bar that holds them.
    assert_eq!(
        came["created_clip"],
        json!({ "id": clip, "track": 1,
                "start": { "at": "3.1.0", "tick": 2 * BAR },
                "end": { "at": "28.1.0", "tick": 27 * BAR } })
    );
    let got = notes_of(&r, clip);
    let wanted: Vec<(i64, u64, u8, u8)> = (0..400u64)
        .map(|i| {
            (
                (2 * BAR + i * 480) as i64,
                400,
                36 + (i % 48) as u8,
                1 + (i % 127) as u8,
            )
        })
        .collect();
    assert_eq!(got, wanted);
    // The song keeps its own tempo and meter.
    let now = r.core.ai_song().unwrap();
    assert_eq!(now.song.doc.timeline, before.song.doc.timeline);

    // The file's start inside that clip: the notes go into it, and it grows back.
    let more = ok(
        &mut c,
        "import_midi",
        json!({ "path": path, "track": 1, "tick": 3 * BAR, "channel": 10, "transpose": 2 }),
    );
    assert_eq!(more["written"], 4, "{more}");
    assert_eq!(more["hanging_notes"], 1);
    assert_eq!(more["created_clip"], Value::Null);
    assert_eq!(id(&more), clip);
    let got = notes_of(&r, clip);
    assert_eq!(got.len(), 404);
    // The hit that never ended ends where its track ends: at once, so it gets one tick.
    let hits: Vec<_> = got.iter().filter(|n| n.2 == 38 && n.3 == 100).collect();
    assert_eq!(
        hits,
        [
            &(3 * BAR as i64, Q, 38, 100),
            &((3 * BAR + Q) as i64, Q, 38, 100),
            &((3 * BAR + 2 * Q) as i64, Q, 38, 100),
            &((3 * BAR + 3 * Q) as i64, 1, 38, 100),
        ]
    );

    // Notes moved off the keyboard are left out, with the reason; all of them: an error.
    ok(&mut c, "add_track", json!({ "sound": "sine" }));
    let high = ok(
        &mut c,
        "import_midi",
        json!({ "path": path, "track": 2, "midi_track": 2, "transpose": 60 }),
    );
    // Keys 68 to 83 go over 127: 16 of every 48.
    assert_eq!(high["written"], 400 - 16 * 8, "{high}");
    assert_eq!(high["not_written_count"], 16 * 8);
    assert_eq!(high["not_written"][0]["reason"], "key_out_of_range");
    assert_eq!(high["not_written"][0]["index"], 32);
    assert_eq!(high["not_written"][0]["key"], 128);
    let now = steps(&r);
    let said = refused(
        &mut c,
        "import_midi",
        json!({ "path": path, "track": 2, "at": "40.1.0", "channel": 10, "transpose": 100 }),
    );
    assert!(said.contains("outside 0 to 127"), "{said}");

    // A clip in the way: nothing comes in, and the clip is named.
    ok(
        &mut c,
        "create_clip",
        json!({ "track": 2, "at": "60.1.0", "bars": 1 }),
    );
    let now = now + 1;
    let said = refused(
        &mut c,
        "import_midi",
        json!({ "path": path, "track": 2, "at": "50.1.0", "midi_track": 2 }),
    );
    assert!(said.contains("(60.1.0 to 61.1.0) is in the way"), "{said}");

    // What cannot be read says why; nothing changes.
    let format2 = write_mid(&dir, "two.mid", b"MThd\0\0\0\x06\0\x02\0\0\x01\xE0");
    let smpte = write_mid(&dir, "smpte.mid", b"MThd\0\0\0\x06\0\x01\0\0\xE7\x28");
    let text = write_mid(&dir, "text.mid", b"this is no MIDI file at all");
    for (path, word) in [
        (format2.as_str(), "format 2"),
        (smpte.as_str(), "SMPTE"),
        (text.as_str(), "not a standard MIDI file"),
        ("/nonexistent/k11notes.mid", "cannot be read"),
        ("relative.mid", "absolute"),
    ] {
        let said = refused(&mut c, "import_midi", json!({ "path": path, "track": 1 }));
        assert!(said.contains(word), "{path}: {said}");
    }
    for args in [
        json!({ "path": path }),
        json!({ "path": path, "track": 1, "midi_track": 9 }),
        json!({ "path": path, "track": 1, "midi_track": 1 }),
        json!({ "path": path, "track": 1, "channel": 5 }),
        json!({ "path": path, "track": 7 }),
    ] {
        refused(&mut c, "import_midi", args);
    }
    assert_eq!(steps(&r), now);
    undo_all(&mut c, &r, &before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_clip_is_copied_to_another_track_transposed() {
    let r = running("copy");
    let mut c = Client::start(&r.socket);
    let wrote = ok(
        &mut c,
        "write_notes",
        json!({ "track": 1, "notes": [
            ["1.2.0", 960, 40, 90], ["1.3.0", 960, 43, 91], ["2.1.0", 960, 45, 92],
            ["4.4.0", 960, 47, 93],
        ] }),
    );
    let source = id(&wrote);
    let take = wrote["take"].as_u64().unwrap();
    let source_notes = notes_of(&r, source);
    ok(&mut c, "add_track", json!({ "sound": "sine" }));
    let before = r.core.ai_song().unwrap();

    // The same phrase on another track, an octave up: one step, a new clip.
    let copied = ok(
        &mut c,
        "copy_notes",
        json!({ "clip": source, "to_track": 2, "transpose": 12 }),
    );
    assert_eq!(steps(&r), before.undo_count + 1);
    assert_eq!(copied["written"], 4, "{copied}");
    assert_eq!(copied["not_written"], json!([]));
    assert_eq!(copied["created_clip"]["track"], 2);
    assert_eq!(copied["created_clip"]["start"]["at"], "1.1.0");
    assert_eq!(copied["created_clip"]["end"]["at"], "5.1.0");
    let twin = id(&copied);
    let up: Vec<_> = (source_notes.iter())
        .map(|&(s, l, k, v)| (s, l, k + 12, v))
        .collect();
    assert_eq!(notes_of(&r, twin), up);
    assert_eq!(notes_of(&r, source), source_notes, "the source stays");
    assert_eq!(
        r.core.ai_song().unwrap().song.doc.tracks[0],
        before.song.doc.tracks[0]
    );

    // Moved by bars into a clip that is there: it grows.
    let later = ok(
        &mut c,
        "copy_notes",
        json!({ "clip": source, "to_clip": twin, "shift_bars": 4, "transpose": -5 }),
    );
    assert_eq!(later["written"], 4);
    assert_eq!(later["created_clip"], Value::Null);
    assert_eq!(later["grew_to"]["end"]["at"], "9.1.0", "{later}");
    let got = notes_of(&r, twin);
    assert_eq!(got.len(), 8);
    assert_eq!(got[4], (4 * BAR as i64 + 1920, 960, 35, 90));

    // `at` is where the clip's start goes: the rest before the first note is kept. Only
    // the notes of a range; the range's start goes to `at`.
    let at = ok(
        &mut c,
        "copy_notes",
        json!({ "clip": source, "to_track": 2, "at": "21.1.0" }),
    );
    assert_eq!(
        notes_of(&r, id(&at))[0],
        (20 * BAR as i64 + 1920, 960, 40, 90)
    );
    let part = ok(
        &mut c,
        "copy_notes",
        json!({ "clip": source, "to_track": 2, "from": "1.3.0", "to": "3.1.0", "at": 30 * BAR }),
    );
    assert_eq!(
        notes_of(&r, id(&part)),
        [
            (30 * BAR as i64, 960, 43, 91),
            (30 * BAR as i64 + 2 * 1920, 960, 45, 92)
        ]
    );
    // By index, with the take's id; earlier by ticks.
    let picked = ok(
        &mut c,
        "copy_notes",
        json!({ "clip": source, "take": take, "notes": [3, 0], "to_track": 2,
                "shift_ticks": 40 * BAR as i64 - 1920 }),
    );
    assert_eq!(
        notes_of(&r, id(&picked)),
        [
            (40 * BAR as i64, 960, 40, 90),
            ((43 * BAR + 3 * 1920) as i64 - 1920, 960, 47, 93)
        ]
    );

    // What cannot be written comes back with the reason.
    let low = ok(
        &mut c,
        "copy_notes",
        json!({ "clip": source, "to_track": 2, "shift_bars": 50, "transpose": -41 }),
    );
    assert_eq!(low["written"], 3, "{low}");
    assert_eq!(
        low["not_written"],
        json!([{ "index": 0, "start": 50 * BAR + 1920, "length": 960, "key": -1,
                 "reason": "key_out_of_range" }])
    );
    let early = ok(
        &mut c,
        "copy_notes",
        json!({ "clip": source, "to_track": 2, "shift_ticks": -(1920 * 3) }),
    );
    assert_eq!(early["written"], 2, "{early}");
    assert_eq!(early["not_written_count"], 2);
    assert_eq!(early["not_written"][1]["reason"], "before_song_start");
    assert_eq!(early["not_written"][1]["index"], 1);
    assert_eq!(early["not_written"][1]["start"], -1920);

    // A trimmed clip: only what sounds is copied, as long as it sounds.
    let short = ok(
        &mut c,
        "write_notes",
        json!({ "track": 1, "notes": [["70.1.0", 960, 60], ["71.4.0", 3840, 62], ["73.1.0", 960, 64]] }),
    );
    let short_clip = id(&short);
    ok(
        &mut c,
        "set_clip",
        json!({ "clip": short_clip, "field": "length", "value": 2 * BAR }),
    );
    assert_eq!(range_of(&r, short_clip), (69 * BAR, 71 * BAR));
    let heard = ok(
        &mut c,
        "copy_notes",
        json!({ "clip": short_clip, "to_track": 2 }),
    );
    assert_eq!(heard["written"], 2, "{heard}");
    assert_eq!(
        notes_of(&r, id(&heard)),
        [
            (69 * BAR as i64, 960, 60, 100),
            ((70 * BAR + 3 * 1920) as i64, 1920, 62, 100)
        ]
    );
    let take_now = ok(&mut c, "get_notes", json!({ "clip": short_clip }))["take"].clone();
    let hidden = ok(
        &mut c,
        "copy_notes",
        json!({ "clip": short_clip, "take": take_now, "notes": [0, 2], "to_track": 2,
                "shift_bars": 10 }),
    );
    assert_eq!(hidden["written"], 1, "{hidden}");
    assert_eq!(hidden["not_written"][0]["reason"], "outside_source_clip");
    assert_eq!(hidden["not_written"][0]["index"], 2);

    let now = steps(&r);
    for (args, word) in [
        // The take changed since it was read.
        (
            json!({ "clip": source, "take": take + 100, "notes": [0], "to_track": 2 }),
            "take",
        ),
        (
            json!({ "clip": source, "notes": [0], "to_track": 2 }),
            "take",
        ),
        (
            json!({ "clip": source, "take": take, "notes": [9], "to_track": 2 }),
            "no note 9",
        ),
        // Onto itself, unmoved.
        (json!({ "clip": source, "to_clip": source }), "as it is"),
        (json!({ "clip": source, "to_track": 1 }), "as it is"),
        (json!({ "clip": source }), "to_track"),
        (
            json!({ "clip": source, "to_track": 2, "to_clip": twin }),
            "to_track",
        ),
        (
            json!({ "clip": source, "to_track": 2, "shift_bars": 1, "at": "9.1.0" }),
            "at most one",
        ),
        (json!({ "clip": 9999, "to_track": 2 }), "no clip 9999"),
        (json!({ "clip": source, "to_track": 9 }), "no track 9"),
        (
            json!({ "clip": source, "to_track": 2, "shift_bars": -3 }),
            "before the song",
        ),
        // Bars 21 to 24 of track 2 hold a copy made above: it is named.
        (
            json!({ "clip": source, "to_track": 2, "shift_bars": 18 }),
            "(21.1.0 to 25.1.0) is in the way",
        ),
    ] {
        let said = refused(&mut c, "copy_notes", args.clone());
        assert!(said.contains(word), "{args}: {said}");
    }
    assert_eq!(steps(&r), now);
}

#[test]
fn what_is_written_out_comes_back_the_same() {
    let r = running("export");
    let mut c = Client::start(&r.socket);
    let dir = scratch("export-files");
    // Odd ticks, chords, notes of one key that touch, cross and start together.
    let mut notes: Vec<Value> = (0..120u64)
        .map(|i| {
            json!([
                BAR + i * 233 + i % 7,
                37 + i * 3 % 400,
                30 + i * 7 % 60,
                1 + i % 127
            ])
        })
        .collect();
    notes.extend([
        json!([2 * BAR, 500, 100, 10]),
        json!([2 * BAR + 500, 300, 100, 20]),
        json!([2 * BAR + 700, 300, 100, 30]),
        json!([3 * BAR, 100, 101, 40]),
        json!([3 * BAR, 200, 101, 50]),
    ]);
    let made = ok(
        &mut c,
        "create_clip",
        json!({ "track": 1, "at": "2.1.0", "bars": 8 }),
    );
    let clip = id(&made);
    ok(
        &mut c,
        "write_notes",
        json!({ "clip": clip, "notes": notes }),
    );
    ok(&mut c, "set_tempo", json!({ "bpm": 133.0 }));
    ok(&mut c, "set_tempo", json!({ "bpm": 90.0, "at": "4.1.0" }));
    ok(&mut c, "add_track", json!({ "sound": "sine" }));
    ok(&mut c, "add_track", json!({ "sound": "sine" }));
    let source = notes_of(&r, clip);
    assert_eq!(source.len(), 125);
    let before = r.core.ai_song().unwrap();

    // The clip into the private folder, and back at the place the answer names.
    let out = ok(&mut c, "export_midi", json!({ "clip": clip }));
    assert_eq!(steps(&r), before.undo_count, "the song is not changed");
    assert_eq!(out["notes"], 125, "{out}");
    assert_eq!(out["ticks_per_quarter"], 1920);
    assert_eq!(out["origin"], json!({ "at": "2.1.0", "tick": BAR }));
    assert_eq!(out["end"]["at"], "10.1.0");
    let first = &before.song.doc.tracks[0].name;
    assert_eq!(
        out["tracks"],
        json!([{ "number": 1, "name": first, "notes": 125, "channel": 1 }])
    );
    let file = out["file"].as_str().unwrap().to_owned();
    assert!(
        file.ends_with(".mid") && file.contains("meloet-ai-"),
        "{file}"
    );
    let listed = ok(
        &mut c,
        "import_midi",
        json!({ "path": file, "list_only": true }),
    );
    assert_eq!(listed["format"], 1, "{listed}");
    assert_eq!(listed["tracks"][0]["name"], before.song.doc.name);
    assert_eq!(listed["tracks"][1]["name"], *first);
    assert_eq!(listed["tracks"][1]["length_ticks"], 8 * BAR);
    // The tempo at the clip's start, and the change two bars into it.
    assert_eq!(
        listed["tempo"],
        json!([{ "tick": 0, "bpm": 133.0 }, { "tick": 2 * BAR, "bpm": 90.0 }])
    );
    assert_eq!(listed["meters"], json!([{ "tick": 0, "meter": "4/4" }]));
    let back = ok(
        &mut c,
        "import_midi",
        json!({ "path": file, "track": 2, "tick": out["origin"]["tick"] }),
    );
    assert_eq!(back["written"], 125, "{back}");
    assert_eq!(back["created_clip"]["start"]["at"], "2.1.0");
    assert_eq!(back["created_clip"]["track"], 2);
    assert_eq!(notes_of(&r, id(&back)), source);

    // A range of the song to a path, at 480 ticks a quarter note: the notes that start in
    // it, cut at its end; a file that is there is kept unless asked.
    let path = dir.join("part.mid").display().to_string();
    let args = json!({ "from": "2.1.0", "to": "3.1.0", "tracks": [1, 3], "path": path,
                       "ticks_per_quarter": 480 });
    let part = ok(&mut c, "export_midi", args.clone());
    let in_range: Vec<_> = (source.iter())
        .filter(|n| (BAR as i64..2 * BAR as i64).contains(&n.0))
        .collect();
    assert_eq!(part["notes"], in_range.len(), "{part}");
    assert_eq!(part["file"], path);
    let third = &before.song.doc.tracks[2].name;
    assert_eq!(
        part["tracks"][1],
        json!({ "number": 3, "name": third, "notes": 0, "channel": 1 })
    );
    let said = refused(&mut c, "export_midi", args.clone());
    assert!(said.contains("overwrite"), "{said}");
    std::fs::write(&path, b"old").unwrap();
    let mut again = args.clone();
    again["overwrite"] = json!(true);
    ok(&mut c, "export_midi", again);
    let listed = ok(
        &mut c,
        "import_midi",
        json!({ "path": path, "list_only": true }),
    );
    assert_eq!(listed["ticks_per_quarter"], 480, "{listed}");
    assert_eq!(listed["tracks"].as_array().unwrap().len(), 3);
    assert_eq!(listed["tracks"][1]["notes"], in_range.len());
    assert_eq!(listed["tracks"][1]["length_ticks"], BAR);
    let rough = ok(
        &mut c,
        "import_midi",
        json!({ "path": path, "track": 3, "at": "2.1.0", "midi_track": 2 }),
    );
    let got = notes_of(&r, id(&rough));
    assert_eq!(got.len(), in_range.len());
    for (g, w) in got.iter().zip(&in_range) {
        // To the nearest of 480 a quarter note: within 2 ticks, never past the range.
        assert!(
            (g.0 - w.0).abs() <= 2 && (g.2, g.3) == (w.2, w.3),
            "{g:?} {w:?}"
        );
        assert!(g.0 as u64 + g.1 <= 2 * BAR);
    }

    for (args, word) in [
        (json!({ "clip": clip, "from": "1.1.0" }), "not both"),
        (json!({ "clip": 9999 }), "no clip 9999"),
        (json!({ "from": "5.1.0", "to": "5.1.0" }), "empty"),
        (json!({ "tracks": [9] }), "no track 9"),
        (
            json!({ "path": dir.join("x.wav").display().to_string() }),
            ".mid",
        ),
        (
            json!({ "path": "/nonexistent-k11notes/x.mid" }),
            "not there",
        ),
        (json!({ "path": "x.mid" }), "absolute"),
        (json!({ "ticks_per_quarter": 96 }), "480, 960 or 1920"),
    ] {
        let said = refused(&mut c, "export_midi", args.clone());
        assert!(said.contains(word), "{args}: {said}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// How long the core takes for one step of very many notes: the tools' largest import.
#[test]
fn fifty_thousand_notes_are_one_step() {
    let r = running("many");
    let mut c = Client::start(&r.socket);
    let before = r.core.ai_song().unwrap();
    let notes: Vec<AiNote> = (0..50_000u64)
        .map(|i| AiNote {
            start: (i * 97) as i64,
            length: 60 + i % 300,
            key: 20 + (i * 7 % 90) as i32,
            velocity: 1 + (i % 127) as u8,
        })
        .collect();
    let ask = AiAsk::PutNotes {
        at: AiPlace::Track(1),
        notes,
    };
    let started = Instant::now();
    let answer = r.core.ai(ask);
    let took = started.elapsed();
    let AiAnswer::Done(done) = answer else {
        panic!("{answer:?}");
    };
    let told = done.notes.unwrap();
    assert_eq!((told.written, told.not_written_count), (50_000, 0));
    assert_eq!(steps(&r), before.undo_count + 1);
    let started = Instant::now();
    ok(&mut c, "undo", json!({}));
    eprintln!(
        "k11notes: 50000 notes as one step: the core answered in {took:?}; undo through the \
         socket in {:?}",
        started.elapsed()
    );
    assert_eq!(
        r.core.ai_song().unwrap().song.doc.tracks,
        before.song.doc.tracks
    );
    assert!(took.as_secs() < 5, "{took:?}");
}

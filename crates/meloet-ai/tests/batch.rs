//! `batch` through the socket: several tools as one undo step or nothing, and "no change"
//! as a success.

mod common;
use common::*;

use meloet_core::ai::AiSong;
use serde_json::{Value, json};

fn op(tool: &str, arguments: Value) -> Value {
    json!({ "tool": tool, "arguments": arguments })
}

fn song(r: &Running) -> AiSong {
    r.core.ai_song().unwrap()
}

/// The song is as `before`, but for the ids it has given out.
fn same_song(r: &Running, before: &AiSong) {
    let now = song(r);
    assert_eq!(now.undo_count, before.undo_count);
    assert_eq!(now.song.doc.tracks, before.song.doc.tracks);
    assert_eq!(now.song.doc.returns, before.song.doc.returns);
    assert_eq!(now.song.doc.master, before.song.doc.master);
    assert_eq!(now.song.doc.timeline, before.song.doc.timeline);
    assert_eq!(now.song.doc.markers, before.song.doc.markers);
    assert_eq!(now.song.doc.loop_range, before.song.doc.loop_range);
    assert_eq!(now.song.session.soloed, before.song.session.soloed);
    assert_eq!(now.song.session.loop_on, before.song.session.loop_on);
}

fn rows(r: &Running) -> u32 {
    r.core.snapshot().state.history.total
}

#[test]
fn a_mix_pass_over_several_tracks_is_one_step() {
    let r = running("batch-mix");
    let mut c = Client::start(&r.socket);
    ok(&mut c, "add_track", json!({ "sound": "grandPiano" }));
    ok(&mut c, "add_track", json!({ "sound": "grandPiano" }));
    let before = song(&r);
    assert_eq!(before.song.doc.tracks.len(), 3);

    let mut operations = Vec::new();
    for (track, name) in [(1, "ドラム"), (2, "ベース"), (3, "パッド")] {
        operations.push(op("rename_track", json!({ "track": track, "name": name })));
        operations.push(op("set_mute", json!({ "track": track, "on": true })));
        operations.push(op(
            "set_pan",
            json!({ "track": track, "pan": track * 10 - 20 }),
        ));
        let send = json!({ "track": track, "send": "a", "db": -18 - track });
        operations.push(op("set_send", send));
        operations.push(op(
            "set_volume",
            json!({ "track": track, "db": -3 * track }),
        ));
    }
    let done = ok(&mut c, "batch", json!({ "operations": operations }));
    assert_eq!(done["changed"], true, "{done}");
    assert_eq!(done["operations"], 15);
    assert_eq!(done["undo_steps"], before.undo_count + 1);
    assert_eq!(done["step"]["index"], before.undo_count + 1, "{done}");
    let results = done["results"].as_array().unwrap();
    assert_eq!(results.len(), 15);
    assert_eq!(results[0]["tool"], "rename_track");
    assert_eq!(results[14]["result"]["changed"], true);
    assert!(results[14]["result"].get("step").is_none(), "{done}");

    assert_eq!(steps(&r), before.undo_count + 1);
    assert_eq!(
        rows(&r),
        before.undo_count + 1,
        "one row in the history band"
    );
    let now = song(&r);
    let doc = &now.song.doc;
    let names: Vec<&str> = doc.tracks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["ドラム", "ベース", "パッド"]);
    assert!(doc.tracks.iter().all(|t| t.muted));
    let mixer = ok(&mut c, "get_mixer", json!({}));
    assert_eq!(mixer["tracks"][2]["volume_db"], -9.0, "{mixer}");
    assert_eq!(mixer["tracks"][0]["pan"], -10, "{mixer}");

    // One undo brings everything back.
    undo_all(&mut c, &r, &before);
    assert_eq!(steps(&r), before.undo_count);
}

#[test]
fn a_clip_made_in_a_batch_gets_its_notes_in_the_same_batch() {
    let r = running("batch-clip");
    let mut c = Client::start(&r.socket);
    let before = song(&r);
    let made = json!({ "result_of": 1, "field": "clip" });
    let notes = json!([
        { "at": "5.1.0", "length": 480, "key": 60, "velocity": 90 },
        { "at": "5.2.0", "length": 480, "key": 64, "velocity": 80 },
        { "start": 32640, "length": 480, "key": 67, "velocity": 70 },
    ]);
    let done = ok(
        &mut c,
        "batch",
        json!({ "operations": [
            op("create_clip", json!({ "track": 1, "at": "5.1.0" })),
            op("write_notes", json!({ "clip": made, "notes": notes })),
            op("rename_clip", json!({ "clip": made, "name": "イントロ" })),
            op("set_tempo", json!({ "bpm": 101 })),
        ] }),
    );
    assert_eq!(done["undo_steps"], 1, "{done}");
    let clip = done["results"][0]["result"]["clip"].as_u64().unwrap();
    assert_eq!(done["results"][1]["result"]["clip"], clip);
    let take = done["results"][1]["result"]["take"].as_u64().unwrap();
    let got = ok(&mut c, "get_notes", json!({ "clip": clip }));
    assert_eq!(got["take"], take, "{got}");
    assert_eq!(got["total"], 3);
    let overview = ok(&mut c, "song_overview", json!({}));
    assert_eq!(overview["clips"][0]["name"], "イントロ", "{overview}");
    assert_eq!(rows(&r), 1);

    // The take and the note indexes the batch gave are good afterwards.
    ok(
        &mut c,
        "delete_notes",
        json!({ "take": take, "notes": [0] }),
    );
    ok(&mut c, "undo", json!({}));
    undo_all(&mut c, &r, &before);
    assert!(song(&r).song.doc.tracks[0].clips.is_empty());
}

#[test]
fn a_batch_whose_fifth_operation_fails_changes_nothing() {
    let r = running("batch-fail");
    let mut c = Client::start(&r.socket);
    ok(&mut c, "set_volume", json!({ "track": 1, "db": -2 }));
    ok(&mut c, "set_volume", json!({ "track": 1, "db": -4 }));
    ok(&mut c, "undo", json!({}));
    let before = song(&r);
    let history = r.core.snapshot().state.history;
    assert_eq!((history.current, history.redo), (1, 1));

    let answer = c.call(
        "batch",
        json!({ "operations": [
            op("rename_track", json!({ "track": 1, "name": "ベース" })),
            op("set_mute", json!({ "track": 1, "on": true })),
            op("set_solo", json!({ "track": 1, "on": true })),
            op("create_clip", json!({ "track": 1, "at": "1.1.0" })),
            op("set_volume", json!({ "track": 9, "db": -6 })),
            op("set_pan", json!({ "track": 1, "pan": 20 })),
        ] }),
    );
    assert_eq!(answer["isError"], true, "{answer}");
    let text = answer["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("operation 5 of 6 (set_volume)"), "{text}");
    assert!(text.contains("there is no track 9"), "{text}");
    assert!(text.contains("Nothing was changed"), "{text}");
    let value = &answer["structuredContent"];
    assert_eq!(value["taken_back"], true, "{value}");
    assert_eq!(value["changed"], false);
    assert_eq!(value["failed"]["operation"], 5);
    assert_eq!(value["failed"]["tool"], "set_volume");
    assert_eq!(value["undo_steps"], before.undo_count);

    same_song(&r, &before);
    // The step that could be redone before the batch still can, and nothing of the batch.
    let history = r.core.snapshot().state.history;
    assert_eq!((history.current, history.redo, history.total), (1, 1, 2));
    assert_eq!(song(&r).unsaved, before.unsaved);
    r.core.send(meloet_protocol::Command::Redo);
    wait_for("the redo", || steps(&r) == 2);
    let mixer = ok(&mut c, "get_mixer", json!({}));
    assert_eq!(mixer["tracks"][0]["volume_db"], -4.0, "{mixer}");

    // The batch is closed: single calls go on.
    ok(&mut c, "set_pan", json!({ "track": 1, "pan": 20 }));
}

#[test]
fn no_change_is_a_success_and_a_wrong_track_is_an_error() {
    let r = running("batch-nochange");
    let mut c = Client::start(&r.socket);
    ok(
        &mut c,
        "set_send",
        json!({ "track": 1, "send": "a", "db": -9 }),
    );
    ok(&mut c, "set_volume", json!({ "track": 1, "db": -6 }));
    let before = steps(&r);
    let bpm = song(&r).song.doc.timeline.tempo.bpm();
    for (tool, args) in [
        ("set_send", json!({ "track": 1, "send": "a", "db": -9 })),
        ("set_volume", json!({ "track": 1, "db": -6 })),
        ("set_pan", json!({ "track": 1, "pan": 0 })),
        ("set_mute", json!({ "track": 1, "on": false })),
        ("set_solo", json!({ "track": 1, "on": false })),
        ("set_tempo", json!({ "bpm": bpm })),
        ("move_track", json!({ "track": 1, "to": 1 })),
    ] {
        let answer = c.call(tool, args.clone());
        assert_eq!(answer["isError"], false, "{tool} {args}: {answer}");
        let text = answer["content"][0]["text"].as_str().unwrap();
        assert!(text.starts_with("No change"), "{tool}: {text}");
        let value = &answer["structuredContent"];
        assert_eq!(value["changed"], false, "{tool}: {value}");
        assert_eq!(value["undo_steps"], before);
        assert!(value["step"].is_null());
    }
    assert_eq!(steps(&r), before);
    let name = song(&r).song.doc.tracks[0].name.clone();
    let same = ok(&mut c, "rename_track", json!({ "track": 1, "name": name }));
    assert_eq!(same["changed"], false);

    // A change says so.
    let changed = ok(&mut c, "set_volume", json!({ "track": 1, "db": -7 }));
    assert_eq!(changed["changed"], true);
    assert_eq!(changed["undo_steps"], before + 1);

    // What is not there is an error, not "no change".
    for (tool, args, why) in [
        ("set_volume", json!({ "track": 9, "db": -6 }), "no track 9"),
        (
            "set_send",
            json!({ "track": 9, "send": "a", "db": -6 }),
            "no track 9",
        ),
        (
            "rename_track",
            json!({ "track": 9, "name": "x" }),
            "no track 9",
        ),
        ("set_mute", json!({ "track": 9, "on": true }), "no track 9"),
        ("delete_marker", json!({ "marker": 99 }), "no such thing"),
        (
            "rename_marker",
            json!({ "marker": 99, "name": "x" }),
            "no such thing",
        ),
        ("delete_meter_change", json!({ "bar": 7 }), "no such thing"),
        ("split_clip", json!({ "clip": 99, "tick": 0 }), "no clip 99"),
        ("join_clips", json!({ "clips": [98, 99] }), "no clip 98"),
        (
            "set_clip",
            json!({ "clip": 99, "field": "start", "value": 0 }),
            "no clip 99",
        ),
        (
            "rename_clip",
            json!({ "clip": 99, "name": "x" }),
            "no clip 99",
        ),
    ] {
        let text = refused(&mut c, tool, args.clone());
        assert!(text.contains(why), "{tool} {args}: {text}");
        assert!(!text.contains("leaves the song as it is"), "{tool}: {text}");
    }
    assert_eq!(steps(&r), before + 1);
}

#[test]
fn what_cannot_be_in_a_batch_is_refused_before_anything_runs() {
    let r = running("batch-refused");
    let mut c = Client::start(&r.socket);
    let before = song(&r);
    let good = op("set_volume", json!({ "track": 1, "db": -6 }));
    for (bad, why) in [
        (op("undo", json!({})), "undo"),
        (op("save_song", json!({})), "save"),
        (op("new_song", json!({})), "another song"),
        (
            op("open_song", json!({ "path": "/nowhere/a.meloet" })),
            "another song",
        ),
        (
            op("import_audio", json!({ "path": "/nowhere/a.wav" })),
            "ends later",
        ),
        (op("export_song", json!({})), "export"),
        (op("get_notes", json!({ "clip": 1 })), "only reads"),
        (op("song_overview", json!({})), "only reads"),
        (
            op("measure_mix", json!({ "from_bar": 1, "to_bar": 2 })),
            "only reads",
        ),
        (
            op("render_note", json!({ "track": 1, "key": 60 })),
            "only reads",
        ),
        (
            op("batch", json!({ "operations": [good.clone()] })),
            "batch",
        ),
        (op("no_such_tool", json!({})), "no such tool"),
        (
            op(
                "rename_clip",
                json!({ "clip": { "result_of": 2, "field": "clip" }, "name": "x" }),
            ),
            "not an operation before",
        ),
        (
            op(
                "rename_clip",
                json!({ "clip": { "result_of": 0, "field": "clip" }, "name": "x" }),
            ),
            "a reference is",
        ),
    ] {
        let text = refused(
            &mut c,
            "batch",
            json!({ "operations": [good.clone(), bad.clone()] }),
        );
        assert!(text.contains("Operation 2"), "{bad}: {text}");
        assert!(text.contains(why), "{bad}: {text}");
        assert!(text.contains("Nothing was run"), "{bad}: {text}");
        same_song(&r, &before);
    }
    let text = refused(&mut c, "batch", json!({ "operations": [] }));
    assert!(text.contains("1 to 200"), "{text}");

    // A value an earlier operation did not give: found when its turn comes, and taken back.
    let answer = c.call(
        "batch",
        json!({ "operations": [
            good.clone(),
            op("rename_clip", json!({ "clip": { "result_of": 1, "field": "clip" }, "name": "x" })),
        ] }),
    );
    assert_eq!(answer["isError"], true, "{answer}");
    assert_eq!(answer["structuredContent"]["taken_back"], true, "{answer}");
    same_song(&r, &before);

    // Arguments are checked as in a call of the tool's own.
    let answer = c.call(
        "batch",
        json!({ "operations": [good.clone(), op("set_volume", json!({ "track": 1 }))] }),
    );
    assert_eq!(answer["isError"], true, "{answer}");
    assert_eq!(answer["structuredContent"]["failed"]["operation"], 2);
    same_song(&r, &before);
    let text = refused(
        &mut c,
        "batch",
        json!({ "operations": [op("set_pan", json!({ "track": 1, "pan": 300 }))] }),
    );
    assert!(text.contains("the pan is -100 to 100"), "{text}");
    same_song(&r, &before);
}

#[test]
fn a_batch_is_one_call_however_long_and_no_change_inside_it_is_no_failure() {
    let r = running("batch-long");
    let mut c = Client::start(&r.socket);
    let before = song(&r);
    // More operations than calls may come in a burst.
    let mut operations = Vec::new();
    for i in 0..80 {
        operations.push(op("set_volume", json!({ "track": 1, "db": -(i % 7) - 1 })));
        // The same value again.
        operations.push(op("set_volume", json!({ "track": 1, "db": -(i % 7) - 1 })));
    }
    let done = ok(&mut c, "batch", json!({ "operations": operations }));
    assert_eq!(done["operations"], 160);
    assert_eq!(done["undo_steps"], 1, "{done}");
    assert_eq!(done["results"][0]["result"]["changed"], true);
    assert_eq!(done["results"][1]["result"]["changed"], false);
    assert_eq!(rows(&r), 1);

    // Only what the song has already: a success that says so, and no step.
    let answer = c.call(
        "batch",
        json!({ "operations": [
            op("set_volume", json!({ "track": 1, "db": -3 })),
            op("set_mute", json!({ "track": 1, "on": false })),
        ] }),
    );
    assert_eq!(answer["isError"], false, "{answer}");
    assert_eq!(answer["structuredContent"]["changed"], false, "{answer}");
    assert!(answer["structuredContent"]["step"].is_null());
    assert_eq!(steps(&r), 1);

    // Solo is no step, in a batch as outside it.
    let done = ok(
        &mut c,
        "batch",
        json!({ "operations": [op("set_solo", json!({ "track": 1, "on": true }))] }),
    );
    assert_eq!(done["changed"], true);
    assert!(done["step"].is_null());
    assert_eq!(steps(&r), 1);
    ok(&mut c, "set_solo", json!({ "track": 1, "on": false }));
    undo_all(&mut c, &r, &before);
}

#[test]
fn another_call_waits_for_the_batch_and_the_person_ends_it() {
    use meloet_core::ai::{AiAnswer, AiAsk, AiRefusal};
    let r = running("batch-others");
    let mut c = Client::start(&r.socket);
    // A batch of another call is open in the core.
    assert!(matches!(
        r.core.ai(AiAsk::Begin { batch: u64::MAX }),
        AiAnswer::Done(_)
    ));
    let text = refused(&mut c, "set_volume", json!({ "track": 1, "db": -6 }));
    assert!(text.contains("a batch of another call"), "{text}");
    let text = refused(
        &mut c,
        "batch",
        json!({ "operations": [op("set_volume", json!({ "track": 1, "db": -6 }))] }),
    );
    assert!(text.contains("a batch of another call"), "{text}");
    ok(&mut c, "song_overview", json!({}));
    assert!(matches!(
        r.core.ai(AiAsk::Rollback { batch: u64::MAX }),
        AiAnswer::Done(_)
    ));
    ok(&mut c, "set_volume", json!({ "track": 1, "db": -6 }));

    // The person changes the song while a batch is open: its next ask is refused.
    assert!(matches!(
        r.core.ai(AiAsk::Begin { batch: u64::MAX }),
        AiAnswer::Done(_)
    ));
    let track = song(&r).song.doc.tracks[0].id.0;
    r.core.send(meloet_protocol::Command::ToggleMute { track });
    wait_for("the person's step", || steps(&r) == 2);
    let ask = AiAsk::InBatch {
        batch: u64::MAX,
        ask: Box::new(AiAsk::SetTempo { tick: 0, bpm: 90.0 }),
    };
    assert_eq!(r.core.ai(ask), AiAnswer::Refused(AiRefusal::BatchLost));
    assert_eq!(
        r.core.ai(AiAsk::Rollback { batch: u64::MAX }),
        AiAnswer::Refused(AiRefusal::BatchLost)
    );
    assert_eq!(steps(&r), 2, "the person's step stays");
    ok(&mut c, "set_volume", json!({ "track": 1, "db": -5 }));
}

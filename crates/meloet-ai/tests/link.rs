//! The AI link through its socket, as `meloet-mcp` reaches it: a core with no devices, the
//! link's thread, and a small MCP client of lines.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use meloet_ai::{AiLink, CoreLink, Meloet};
use meloet_core::Core;
use meloet_core::ai::{
    AiAnswer, AiAsk, AiBusy, AiLevels, AiMeasure, AiOutputLevels, AiOutputsMeasure, AiRefusal,
    AiRender, AiRendered, AiSong,
};
use meloet_protocol::AiPort;
use serde_json::{Value, json};

mod common;
use common::*;

#[test]
fn two_apps_at_once_and_the_switch_closes_them() {
    let r = running("two");
    let mut a = Client::start(&r.socket);
    let mut b = Client::start(&r.socket);
    wait_for("Connected", || port_of(&r.core) == AiPort::Connected);
    for c in [&mut a, &mut b] {
        let list = c.request("tools/list", json!({}), &mut Vec::new());
        let tools = list["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
        for name in [
            "song_overview",
            "get_notes",
            "list_sounds",
            "write_notes",
            "delete_notes",
            "create_clip",
            "rename_clip",
            "set_tempo",
            "add_marker",
            "add_track",
            "undo",
            "measure_mix",
        ] {
            assert!(names.contains(&name), "{name} in {names:?}");
        }
        for t in tools {
            assert_eq!(t["annotations"]["openWorldHint"], false, "{t}");
        }
        let overview = tools.iter().find(|t| t["name"] == "song_overview").unwrap();
        assert_eq!(overview["annotations"]["readOnlyHint"], true);
        let write = tools.iter().find(|t| t["name"] == "write_notes").unwrap();
        let note = &write["inputSchema"]["$defs"]["NoteArgs"]["properties"];
        assert_eq!(note["key"]["maximum"], 127, "{write}");
        assert_eq!(note["velocity"]["minimum"], 1, "{write}");
    }
    let one = a.call("song_overview", json!({}));
    let two = b.call("song_overview", json!({}));
    assert_eq!(one["structuredContent"], two["structuredContent"]);
    assert_eq!(one["structuredContent"]["tracks"][0]["number"], 1);

    drop(a);
    drop(b);
    wait_for("Waiting again", || port_of(&r.core) == AiPort::Waiting);

    let mut c = Client::start(&r.socket);
    wait_for("Connected", || port_of(&r.core) == AiPort::Connected);
    r.core
        .send(meloet_protocol::Command::SetAiLink { on: false });
    wait_for("the socket gone", || !r.socket.exists());
    assert!(
        c.line().is_none(),
        "the connection is closed with the switch"
    );
    assert_eq!(port_of(&r.core), AiPort::Closed);
    assert!(UnixStream::connect(&r.socket).is_err());

    // On again: the same place.
    r.core
        .send(meloet_protocol::Command::SetAiLink { on: true });
    wait_for("the socket again", || r.socket.exists());
    wait_for("Waiting", || port_of(&r.core) == AiPort::Waiting);
    r.core.stop();
    assert!(!r.socket.exists(), "the socket goes with the core");
}

#[test]
fn another_uid_is_refused_before_any_word() {
    let runtime = scratch("uid");
    let mut options = options(&runtime);
    options.expect_uid = my_uid().wrapping_add(1);
    let socket = options.socket_path().unwrap();
    let core = Arc::new(Core::start(offline(&runtime)));
    let link = AiLink::start(Arc::new(CoreLink(Arc::downgrade(&core))), options).unwrap();
    link.switch(true);
    wait_for("the socket", || socket.exists());
    let mut c = Client::connect(&socket);
    let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"0"}}});
    let _ = writeln!(c.writer, "{init}");
    assert!(c.line().is_none(), "closed with no answer");
    assert_ne!(port_of(&core), AiPort::Connected);
}

fn note(at: &str, key: u32) -> Value {
    json!({ "at": at, "length": 960, "key": key, "velocity": 100 })
}

#[test]
fn a_tool_call_is_one_step_and_undo_takes_it_back() {
    let r = running("steps");
    let mut c = Client::start(&r.socket);
    let before = r.core.ai_song().unwrap();

    let wrote = c.call(
        "write_notes",
        json!({ "track": 1, "notes": [note("1.1.0", 60), note("1.2.0", 64), note("1.3.0", 67)] }),
    );
    assert_eq!(wrote["isError"], false, "{wrote}");
    let after = r.core.ai_song().unwrap();
    assert_eq!(after.undo_count, before.undo_count + 1);
    let clip = wrote["structuredContent"]["clip"].as_u64().unwrap();
    let take = wrote["structuredContent"]["take"].as_u64().unwrap();
    assert_eq!(
        wrote["structuredContent"]["step"]["name"]["kind"], "writeNote",
        "{wrote}"
    );

    let notes = c.call("get_notes", json!({ "clip": clip }));
    let got = &notes["structuredContent"];
    assert_eq!(got["take"], take);
    assert_eq!(got["total"], 3);
    assert_eq!(got["notes"][1]["name"], "E4");
    assert_eq!(got["notes"][1]["at"], "1.2.0");

    // Out of range: refused here, before the core.
    let bad = c.call(
        "write_notes",
        json!({ "track": 1, "notes": [note("1.1.0", 128)] }),
    );
    assert_eq!(bad["isError"], true, "{bad}");
    let none = c.call("write_notes", json!({ "notes": [note("1.1.0", 60)] }));
    assert_eq!(none["isError"], true);
    assert_eq!(r.core.ai_song().unwrap().undo_count, after.undo_count);

    let deleted = c.call("delete_notes", json!({ "take": take, "notes": [0] }));
    assert_eq!(deleted["isError"], false, "{deleted}");
    // The take written first is not shown any more.
    let old = c.call("delete_notes", json!({ "take": take, "notes": [0] }));
    assert_eq!(old["isError"], true);

    for (tool, args) in [
        ("rename_clip", json!({ "clip": clip, "name": "サビ" })),
        ("set_tempo", json!({ "bpm": 128.5 })),
        ("add_marker", json!({ "name": "Verse", "at": "5.1.0" })),
        ("add_track", json!({ "sound": "synthBass" })),
        ("create_clip", json!({ "track": 2, "at": "9.1.0" })),
    ] {
        let done = c.call(tool, args);
        assert_eq!(done["isError"], false, "{tool}: {done}");
    }
    let song = r.core.ai_song().unwrap();
    assert_eq!(song.undo_count, before.undo_count + 7);
    let overview = c.call("song_overview", json!({}))["structuredContent"].clone();
    assert_eq!(overview["tempo"][0]["bpm"], 128.5);
    assert_eq!(overview["markers"][0]["name"], "Verse");
    assert_eq!(overview["tracks"][1]["sound"], "synthBass");
    assert_eq!(overview["clips"][0]["name"], "サビ");

    let unknown = c.call("add_track", json!({ "sound": "kazoo" }));
    assert_eq!(unknown["isError"], true);

    for _ in 0..7 {
        let undone = c.call("undo", json!({}));
        assert_eq!(undone["isError"], false, "{undone}");
    }
    let back = r.core.ai_song().unwrap();
    assert_eq!(back.undo_count, before.undo_count);
    assert_eq!(back.song.doc.tracks, before.song.doc.tracks);
    assert_eq!(back.song.doc.timeline, before.song.doc.timeline);
    assert_eq!(back.song.doc.markers, before.song.doc.markers);
    let nothing = c.call("undo", json!({}));
    assert_eq!(nothing["isError"], true);
}

#[test]
fn a_mix_is_measured_with_progress() {
    let r = running("measure");
    let mut c = Client::start(&r.socket);
    let added = c.call("add_track", json!({ "sound": "sine" }));
    assert_eq!(added["isError"], false, "{added}");
    let wrote = c.call(
        "write_notes",
        json!({ "track": 2, "notes": [note("1.1.0", 69), note("1.3.0", 72)] }),
    );
    assert_eq!(wrote["isError"], false, "{wrote}");
    let steps = r.core.ai_song().unwrap().undo_count;

    let mut seen = Vec::new();
    let answer = c.request(
        "tools/call",
        json!({
            "name": "measure_mix",
            "arguments": { "from_bar": 1, "to_bar": 1, "tracks": [2] },
            "_meta": { "progressToken": "p1" },
        }),
        &mut seen,
    );
    let result = &answer["result"];
    assert_eq!(result["isError"], false, "{answer}");
    let levels = &result["structuredContent"];
    let peak = levels["master"]["peak_dbfs"].as_f64().expect("not silent");
    assert!(peak < 0.0 && peak > -40.0, "{levels}");
    assert!(levels["master"]["rms_dbfs"].as_f64().unwrap() < peak);
    assert_eq!(levels["tracks"].as_array().unwrap().len(), 1);
    assert_eq!(levels["tracks"][0]["number"], 2);
    let progress: Vec<&Value> = (seen.iter())
        .filter(|m| m["method"] == "notifications/progress")
        .collect();
    assert!(!progress.is_empty(), "{seen:?}");
    assert!(
        progress
            .iter()
            .all(|p| p["params"]["progressToken"] == "p1")
    );
    // Measuring is not a step.
    assert_eq!(r.core.ai_song().unwrap().undo_count, steps);

    let bad = c.call("measure_mix", json!({ "from_bar": 3, "to_bar": 2 }));
    assert_eq!(bad["isError"], true);
    let gone = c.call(
        "measure_mix",
        json!({ "from_bar": 1, "to_bar": 1, "tracks": [9] }),
    );
    assert_eq!(gone["isError"], true, "{gone}");
}

/// A meloet that is recording, for what the tools say then.
struct Recording {
    ports: Mutex<Vec<AiPort>>,
}

impl Meloet for Recording {
    fn song(&self) -> Option<AiSong> {
        Some(AiSong {
            song: meloet_model::Song::new_default(),
            undo_count: 0,
            busy: Some(AiBusy::Recording),
            library: Default::default(),
            file: None,
            recent: Vec::new(),
            songs_dir: None,
            unsaved: false,
            sample_rate: 48_000,
            outcomes: Vec::new(),
            instrument_sets: Vec::new(),
        })
    }

    fn ask(&self, _: AiAsk) -> AiAnswer {
        AiAnswer::Refused(AiRefusal::Busy(AiBusy::Recording))
    }

    fn measure(
        &self,
        _: AiMeasure,
        _: &mut dyn FnMut(u8),
        _: &dyn Fn() -> bool,
    ) -> Result<AiLevels, AiRefusal> {
        Err(AiRefusal::Busy(AiBusy::Recording))
    }

    fn measure_outputs(
        &self,
        _: AiOutputsMeasure,
        _: &mut dyn FnMut(u8),
        _: &dyn Fn() -> bool,
    ) -> Result<AiOutputLevels, AiRefusal> {
        Err(AiRefusal::Busy(AiBusy::Recording))
    }

    fn render(
        &self,
        _: AiRender,
        _: &mut dyn FnMut(u8),
        _: &dyn Fn() -> bool,
    ) -> Result<AiRendered, AiRefusal> {
        Err(AiRefusal::Busy(AiBusy::Recording))
    }

    fn port(&self, port: AiPort) {
        self.ports.lock().unwrap().push(port);
    }
}

#[test]
fn while_recording_writes_are_errors_with_the_reason() {
    let runtime = scratch("recording");
    let options = options(&runtime);
    let socket = options.socket_path().unwrap();
    let meloet = Arc::new(Recording {
        ports: Mutex::new(Vec::new()),
    });
    let link = AiLink::start(meloet.clone(), options).unwrap();
    link.switch(true);
    wait_for("the socket", || socket.exists());
    let mut c = Client::start(&socket);
    let answer = c.call(
        "write_notes",
        json!({ "track": 1, "notes": [note("1.1.0", 60)] }),
    );
    assert_eq!(answer["isError"], true);
    let text = answer["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("recording"), "{text}");
    let overview = c.call("song_overview", json!({}));
    assert_eq!(overview["isError"], false);
    assert_eq!(overview["structuredContent"]["busy"], "recording");
    drop(c);
    drop(link);
    let ports = meloet.ports.lock().unwrap().clone();
    assert_eq!(ports.first(), Some(&AiPort::Closed));
    assert!(ports.contains(&AiPort::Waiting));
    assert!(ports.contains(&AiPort::Connected));
    assert_eq!(ports.last(), Some(&AiPort::Closed));
}

#[test]
fn too_many_calls_are_turned_away() {
    let runtime = scratch("bucket");
    let options = options(&runtime);
    let socket = options.socket_path().unwrap();
    let meloet = Arc::new(Recording {
        ports: Mutex::new(Vec::new()),
    });
    let link = AiLink::start(meloet, options).unwrap();
    link.switch(true);
    wait_for("the socket", || socket.exists());
    let mut c = Client::start(&socket);
    let refused = (0..60)
        .map(|_| c.call_once("list_sounds"))
        .filter(|a| a["isError"] == true)
        .count();
    assert!(refused > 0, "some of 60 calls in a row are refused");
    drop(link);
}

/// `meloet-mcp`, built into the same target folder as this test.
fn bridge_exe() -> PathBuf {
    let test = std::env::current_exe().expect("the test's own path");
    let profile = test
        .ancestors()
        .find(|dir| {
            dir.file_name()
                .is_some_and(|n| n == "debug" || n == "release")
        })
        .expect("a target folder");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut build = std::process::Command::new(cargo);
    build.args(["build", "--quiet", "-p", "meloet-mcp", "--target-dir"]);
    build.arg(profile.parent().unwrap());
    if profile.ends_with("release") {
        build.arg("--release");
    }
    assert!(build.status().unwrap().success(), "meloet-mcp builds");
    profile.join("meloet-mcp")
}

struct Piped {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Piped {
    fn ask(&mut self, msg: Value) -> Value {
        writeln!(self.stdin, "{msg}").unwrap();
        loop {
            let mut line = String::new();
            assert!(self.stdout.read_line(&mut line).unwrap() > 0, "an answer");
            let got: Value = serde_json::from_str(&line).unwrap();
            if got.get("id") == msg.get("id") {
                return got;
            }
        }
    }
}

#[test]
fn the_bridge_reaches_the_tools_with_either_handshake() {
    let exe = bridge_exe();
    let r = running("bridge");
    let runtime = r.socket.parent().unwrap().parent().unwrap().to_path_buf();
    let start = || {
        let mut child = std::process::Command::new(&exe)
            .env("XDG_RUNTIME_DIR", &runtime)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        Piped {
            stdin: child.stdin.take().unwrap(),
            stdout: BufReader::new(child.stdout.take().unwrap()),
            child,
        }
    };

    let mut old = start();
    let init = old.ask(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"k10ai","version":"0"}}}));
    assert_eq!(init["result"]["serverInfo"]["name"], "meloet", "{init}");
    writeln!(
        old.stdin,
        r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
    )
    .unwrap();
    let overview = old.ask(json!({"jsonrpc":"2.0","id":2,"method":"tools/call",
        "params":{"name":"song_overview","arguments":{}}}));
    assert_eq!(overview["result"]["isError"], false, "{overview}");

    let meta = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientCapabilities": {},
    });
    let mut new = start();
    let discover = new.ask(json!({"jsonrpc":"2.0","id":"d","method":"server/discover",
        "params":{"_meta":meta}}));
    // What the bridge says on its own has the fields rmcp gives.
    let mut keys: Vec<&String> = discover["result"].as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "_meta",
            "cacheScope",
            "capabilities",
            "instructions",
            "resultType",
            "supportedVersions",
            "ttlMs"
        ],
        "{discover}"
    );
    assert_eq!(discover["result"]["cacheScope"], "private", "{discover}");
    let list =
        new.ask(json!({"jsonrpc":"2.0","id":3,"method":"tools/list","params":{"_meta":meta}}));
    assert_eq!(list["result"]["resultType"], "complete", "{list}");
    let call = new.ask(json!({"jsonrpc":"2.0","id":4,"method":"tools/call",
        "params":{"name":"list_sounds","arguments":{},"_meta":meta}}));
    assert_eq!(call["result"]["isError"], false, "{call}");
    assert_eq!(port_of(&r.core), AiPort::Connected);

    for mut p in [old, new] {
        drop(p.stdin);
        assert!(p.child.wait().unwrap().success());
    }
    wait_for("Waiting", || port_of(&r.core) == AiPort::Waiting);
}

#[test]
fn the_mixer_and_its_effects_are_steps() {
    let r = running("mixer");
    let mut c = Client::start(&r.socket);
    let before = r.core.ai_song().unwrap();
    one_step_each(
        &mut c,
        &r,
        &[
            ("add_track", json!({ "sound": "sine" })),
            ("set_volume", json!({ "track": 2, "db": -6.5 })),
            ("set_volume", json!({ "bus": "master", "db": -100 })),
            ("set_pan", json!({ "track": 2, "pan": -30 })),
            ("set_send", json!({ "track": 2, "send": "b", "db": -12 })),
            ("set_mute", json!({ "track": 2, "on": true })),
            ("set_mute", json!({ "bus": "return_a", "on": true })),
            (
                "insert_effect",
                json!({ "track": 2, "slot": 1, "effect": "reverb" }),
            ),
            (
                "set_param",
                json!({ "track": 2, "slot": 1, "param": "decay", "value": 3.5 }),
            ),
            (
                "set_param",
                json!({ "track": 2, "slot": 1, "param": 1, "value": 20 }),
            ),
            (
                "bypass_effect",
                json!({ "track": 2, "slot": 1, "on": true }),
            ),
            ("move_effect", json!({ "track": 2, "slot": 1, "to": 3 })),
            (
                "replace_effect",
                json!({ "track": 2, "slot": 3, "effect": "compressor" }),
            ),
            (
                "insert_effect",
                json!({ "bus": "master", "slot": 2, "effect": "eq4" }),
            ),
            ("remove_effect", json!({ "bus": "master", "slot": 2 })),
            (
                "set_instrument",
                json!({ "track": 2, "sound": "synthBass" }),
            ),
        ],
    );
    let mixer = ok(&mut c, "get_mixer", json!({}));
    let t = &mixer["tracks"][1];
    assert_eq!(t["volume_db"], -6.5, "{mixer}");
    assert_eq!(t["pan"], -30);
    assert_eq!(t["send_b_db"], -12.0);
    assert_eq!(t["muted"], true);
    assert_eq!(t["instrument"], "synthBass");
    assert_eq!(t["inserts"][0]["slot"], 3, "{t}");
    assert_eq!(t["inserts"][0]["effect"], "compressor");
    assert_eq!(mixer["master"]["volume_db"], Value::Null);
    assert_eq!(mixer["returns"][0]["muted"], true);

    // Solo is no step, and the same state again changes nothing.
    let at = steps(&r);
    ok(&mut c, "set_solo", json!({ "track": 2, "on": true }));
    assert_eq!(steps(&r), at);
    assert_eq!(r.core.ai_song().unwrap().song.session.soloed.len(), 1);
    let same = ok(&mut c, "set_solo", json!({ "track": 2, "on": true }));
    assert_eq!(same["changed"], false);
    assert_eq!(r.core.ai_song().unwrap().song.session.soloed.len(), 1);
    ok(&mut c, "set_solo", json!({ "track": 2, "on": false }));
    let same = ok(&mut c, "set_mute", json!({ "track": 2, "on": true }));
    assert_eq!(same["changed"], false);
    for (tool, args) in [
        ("set_mute", json!({ "bus": "master", "on": true })),
        (
            "insert_effect",
            json!({ "track": 2, "slot": 3, "effect": "delay" }),
        ),
        ("remove_effect", json!({ "track": 2, "slot": 4 })),
        (
            "insert_effect",
            json!({ "track": 2, "slot": 4, "effect": "vst3:none" }),
        ),
        (
            "set_param",
            json!({ "track": 2, "slot": 3, "param": "nope", "value": 1 }),
        ),
        ("set_param", json!({ "track": 2, "param": 1, "value": 0.5 })),
        ("set_volume", json!({ "track": 9, "db": 0 })),
        ("set_volume", json!({ "db": 0 })),
        ("set_instrument", json!({ "track": 2, "sound": "kazoo" })),
        (
            "add_track",
            json!({ "sf2": { "file": "none.sf2", "bank": 0, "program": 0 } }),
        ),
        ("get_plugin_params", json!({ "track": 2 })),
    ] {
        refused(&mut c, tool, args);
    }
    assert_eq!(steps(&r), at);

    let effects = ok(&mut c, "list_effects", json!({}));
    assert_eq!(effects["builtin"].as_array().unwrap().len(), 9);
    assert_eq!(effects["builtin"][2]["params"][0]["name"], "decay");
    let instruments = ok(&mut c, "list_instruments", json!({}));
    assert!(instruments["builtin"].as_array().unwrap().len() > 20);
    undo_all(&mut c, &r, &before);
}

#[test]
fn notes_are_moved_and_quantized_as_steps() {
    let r = running("notes");
    let mut c = Client::start(&r.socket);
    let before = r.core.ai_song().unwrap();
    let wrote = ok(
        &mut c,
        "write_notes",
        json!({ "track": 1, "notes": [
            { "start": 100, "length": 900, "key": 60, "velocity": 80 },
            { "start": 2000, "length": 900, "key": 64, "velocity": 80 },
        ] }),
    );
    let clip = wrote["clip"].as_u64().unwrap();
    let take = |c: &mut Client| ok(c, "get_notes", json!({ "clip": clip }))["take"].clone();
    let calls: Vec<(&str, Value)> = vec![
        (
            "move_notes",
            json!({ "notes": [0], "ticks": 1920, "keys": 2 }),
        ),
        (
            "resize_notes",
            json!({ "notes": [1], "edge": "end", "ticks": 960 }),
        ),
        ("set_note_length", json!({ "notes": [0, 1], "length": 480 })),
        ("set_velocity", json!({ "notes": [0, 1], "value": 110 })),
        ("set_velocity", json!({ "notes": [0], "delta": -20 })),
        (
            "quantize_notes",
            json!({ "notes": [0, 1], "grid": 8, "strength": 100 }),
        ),
        ("unquantize_notes", json!({ "notes": [0, 1] })),
        (
            "move_notes",
            json!({ "notes": [0], "ticks": 0, "keys": 12, "copy": true }),
        ),
    ];
    for (tool, mut args) in calls {
        args["take"] = take(&mut c);
        let at = steps(&r);
        ok(&mut c, tool, args.clone());
        assert_eq!(steps(&r), at + 1, "{tool} {args}");
    }
    let notes = ok(&mut c, "get_notes", json!({ "clip": clip }));
    assert_eq!(notes["total"], 3, "{notes}");
    // An old take does nothing.
    let old = wrote["take"].clone();
    refused(
        &mut c,
        "move_notes",
        json!({ "take": old, "notes": [0], "ticks": 10 }),
    );
    let now = take(&mut c);
    refused(&mut c, "set_velocity", json!({ "take": now, "notes": [0] }));
    refused(
        &mut c,
        "quantize_notes",
        json!({ "take": now, "notes": [0], "grid": 3 }),
    );
    undo_all(&mut c, &r, &before);
}

#[test]
fn automation_and_time_are_steps() {
    let r = running("time");
    let mut c = Client::start(&r.socket);
    let before = r.core.ai_song().unwrap();
    one_step_each(
        &mut c,
        &r,
        &[
            ("add_lane", json!({ "track": 1, "target": "volume" })),
            (
                "write_automation",
                json!({ "track": 1, "target": "volume", "points": [
                    { "at": "1.1.0", "value": 0.8 }, { "at": "2.1.0", "value": 0.4 },
                    { "tick": 15360, "value": 1.0 },
                ] }),
            ),
            (
                "move_automation",
                json!({ "track": 1, "target": "volume", "points": [1], "ticks": 960, "value": 0.1 }),
            ),
            (
                "delete_automation",
                json!({ "track": 1, "target": "volume", "points": [2] }),
            ),
            ("set_meter", json!({ "bar": 3, "num": 7, "den": 8 })),
            ("set_meter", json!({ "bar": 3, "num": 6, "den": 8 })),
            ("set_meter", json!({ "bar": 1, "num": 3, "den": 4 })),
            ("delete_meter_change", json!({ "bar": 3 })),
            ("set_tempo", json!({ "bpm": 140, "at": "5.1.0" })),
            ("set_tempo_ramp", json!({ "on": true })),
            ("add_marker", json!({ "name": "A", "at": "2.1.0" })),
            ("set_loop", json!({ "from_bar": 2, "to_bar": 4 })),
        ],
    );
    let auto = ok(&mut c, "get_automation", json!({ "track": 1 }));
    let points = &auto["lanes"][0]["points"];
    assert_eq!(points.as_array().unwrap().len(), 2, "{auto}");
    assert_eq!(points[0]["means"]["db"], 0.0, "{auto}");
    let overview = ok(&mut c, "song_overview", json!({}));
    let marker = overview["markers"][0]["id"].clone();
    let tempo_at = overview["tempo"][1]["at"].clone();
    assert_eq!(overview["meters"][0]["meter"], "3/4", "{overview}");
    assert_eq!(overview["loop"]["end"]["at"], "5.1.0", "{overview}");
    one_step_each(
        &mut c,
        &r,
        &[
            ("move_marker", json!({ "marker": marker, "at": "3.1.0" })),
            ("rename_marker", json!({ "marker": marker, "name": "サビ" })),
            ("delete_marker", json!({ "marker": marker })),
            ("delete_tempo_change", json!({ "at": tempo_at })),
            ("delete_lane", json!({ "track": 1, "target": "volume" })),
        ],
    );
    // The loop's switch is no step.
    let at = steps(&r);
    ok(&mut c, "set_loop_on", json!({ "on": false }));
    assert_eq!(steps(&r), at);
    let same = ok(&mut c, "set_loop_on", json!({ "on": false }));
    assert_eq!(same["changed"], false);
    for (tool, args) in [
        (
            "write_automation",
            json!({ "track": 1, "target": "pan", "points": [{ "tick": 0, "value": 0.5 }] }),
        ),
        (
            "add_lane",
            json!({ "track": 1, "target": "insert", "slot": 4, "param": 0 }),
        ),
        ("delete_meter_change", json!({ "bar": 7 })),
        ("set_meter", json!({ "bar": 2, "num": 7, "den": 3 })),
    ] {
        refused(&mut c, tool, args);
    }
    assert_eq!(steps(&r), at);
    undo_all(&mut c, &r, &before);
}

#[test]
fn tracks_and_clips_are_steps() {
    let r = running("clips");
    let mut c = Client::start(&r.socket);
    let before = r.core.ai_song().unwrap();
    let wrote = ok(
        &mut c,
        "write_notes",
        json!({ "track": 1, "notes": [note("1.1.0", 60), note("3.1.0", 62)] }),
    );
    let clip = wrote["clip"].as_u64().unwrap();
    one_step_each(
        &mut c,
        &r,
        &[
            ("add_track", json!({ "sound": "sine" })),
            ("rename_track", json!({ "track": 2, "name": "ベース" })),
            ("move_track", json!({ "track": 2, "to": 1 })),
            ("add_vocal_track", json!({})),
            ("split_clip", json!({ "clip": clip, "at": "2.1.0" })),
        ],
    );
    let overview = ok(&mut c, "song_overview", json!({}));
    assert_eq!(overview["tracks"][0]["name"], "ベース", "{overview}");
    assert_eq!(overview["tracks"][2]["kind"], "vocal");
    let clips: Vec<u64> = (overview["clips"].as_array().unwrap().iter())
        .map(|c| c["id"].as_u64().unwrap())
        .collect();
    assert_eq!(clips.len(), 2, "{overview}");
    one_step_each(&mut c, &r, &[("join_clips", json!({ "clips": clips }))]);
    let joined = ok(&mut c, "song_overview", json!({}))["clips"][0]["id"].clone();
    one_step_each(
        &mut c,
        &r,
        &[
            ("duplicate_clips", json!({ "clips": [joined] })),
            (
                "move_clips",
                json!({ "clips": [joined], "ticks": 0, "tracks": -1 }),
            ),
            (
                "set_clip",
                json!({ "clip": joined, "field": "length", "value": 7680 }),
            ),
            ("delete_clips", json!({ "clips": [joined] })),
            ("delete_track", json!({ "track": 3 })),
        ],
    );
    for (tool, args) in [
        ("switch_take", json!({ "clip": clip, "take": 2 })),
        ("join_clips", json!({ "clips": [1] })),
        ("delete_track", json!({ "track": 9 })),
        ("set_clip_stretch", json!({ "clip": 1 })),
        ("import_audio", json!({ "path": "relative.wav" })),
        ("import_audio", json!({ "path": "/tmp/../etc/passwd" })),
        ("import_audio", json!({ "path": "/no/such/file.wav" })),
    ] {
        refused(&mut c, tool, args);
    }
    undo_all(&mut c, &r, &before);
}

/// Channels, rate, bits and the samples of a 16-bit WAV file.
fn wav(path: &Path) -> (u16, u32, u16, Vec<i16>) {
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    let (mut at, mut format) = (12, None);
    loop {
        let id = &bytes[at..at + 4];
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &bytes[at + 8..(at + 8 + size).min(bytes.len())];
        if id == b"fmt " {
            let channels = u16::from_le_bytes(body[2..4].try_into().unwrap());
            let rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
            let bits = u16::from_le_bytes(body[14..16].try_into().unwrap());
            format = Some((channels, rate, bits));
        }
        if id == b"data" {
            let (channels, rate, bits) = format.expect("fmt before data");
            let samples = (body.as_chunks::<2>().0.iter())
                .map(|b| i16::from_le_bytes(*b))
                .collect();
            return (channels, rate, bits, samples);
        }
        at += 8 + size + size % 2;
    }
}

#[test]
fn a_note_is_rendered_into_the_private_folder() {
    let r = running("render");
    let mut c = Client::start(&r.socket);
    ok(&mut c, "add_track", json!({ "sound": "sine" }));
    let steps_before = steps(&r);
    let answer = c.request(
        "tools/call",
        json!({
            "name": "render_note",
            "arguments": { "track": 2, "key": 69, "velocity": 100, "seconds": 0.5 },
            "_meta": { "progressToken": "n1" },
        }),
        &mut Vec::new(),
    );
    let result = &answer["result"];
    assert_eq!(result["isError"], false, "{answer}");
    let file = PathBuf::from(result["structuredContent"]["files"][0].as_str().unwrap());
    let runtime = r.socket.parent().unwrap().parent().unwrap();
    let private = runtime.join("tmp").join(format!("meloet-ai-{}", my_uid()));
    assert_eq!(file.parent(), Some(private.as_path()), "{file:?}");
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(&private).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o700);
    let name = file.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.contains("note-track2-A4"), "{name}");
    let (channels, rate, bits, samples) = wav(&file);
    assert_eq!((channels, bits), (2, 16));
    let said = result["structuredContent"]["sample_rate"].as_u64().unwrap();
    assert_eq!(u64::from(rate), said);
    let seconds = samples.len() as f64 / 2.0 / f64::from(rate);
    assert!((0.5..11.0).contains(&seconds), "{seconds} s");
    let loudest = samples.iter().map(|s| s.unsigned_abs()).max().unwrap();
    assert!(loudest > 1000, "the note sounds: {loudest}");
    assert_eq!(steps(&r), steps_before, "rendering is no step");

    let rendered = ok(
        &mut c,
        "render_audio",
        json!({ "from_bar": 1, "to_bar": 1, "per_track": true }),
    );
    assert_eq!(rendered["files"].as_array().unwrap().len(), 2, "{rendered}");
    let clip = ok(
        &mut c,
        "write_notes",
        json!({ "track": 2, "notes": [note("1.1.0", 69)] }),
    )["clip"]
        .clone();
    let audio = ok(&mut c, "get_clip_audio", json!({ "clip": clip }));
    let (_, _, _, samples) = wav(Path::new(audio["files"][0].as_str().unwrap()));
    assert!(samples.iter().any(|s| s.unsigned_abs() > 1000));
    refused(&mut c, "render_note", json!({ "track": 9, "key": 60 }));
    refused(
        &mut c,
        "render_note",
        json!({ "track": 2, "key": 60, "seconds": 60 }),
    );
    refused(&mut c, "get_clip_audio", json!({ "clip": 999 }));

    // Turning the link off takes the files away.
    r.core
        .send(meloet_protocol::Command::SetAiLink { on: false });
    wait_for("the files gone", || !file.exists());
    assert!(std::fs::read_dir(&private).unwrap().next().is_none());
}

#[test]
fn without_a_private_folder_rendering_cannot_be_used() {
    let runtime = scratch("noscratch");
    let options = options(&runtime);
    let blocked = options.temp_dir.join(format!("meloet-ai-{}", my_uid()));
    std::fs::write(blocked, b"x").unwrap();
    let socket = options.socket_path().unwrap();
    let core = Arc::new(Core::start(offline(&runtime)));
    let link = AiLink::start(Arc::new(CoreLink(Arc::downgrade(&core))), options).unwrap();
    link.switch(true);
    wait_for("the socket", || socket.exists());
    let mut c = Client::start(&socket);
    let text = refused(&mut c, "render_note", json!({ "track": 1, "key": 60 }));
    assert!(text.contains("cannot be used"), "{text}");
    let text = refused(
        &mut c,
        "render_audio",
        json!({ "from_bar": 1, "to_bar": 1 }),
    );
    assert!(text.contains("cannot be used"), "{text}");
    drop(c);
    drop(link);
}

#[test]
fn opening_and_new_songs_keep_unsaved_changes() {
    let r = running("files");
    let mut c = Client::start(&r.socket);
    let dir = scratch("files-songs");
    let text = refused(&mut c, "save_song", json!({}));
    assert!(text.contains("save_song_as"), "{text}");
    refused(
        &mut c,
        "save_song_as",
        json!({ "folder": "songs", "name": "a" }),
    );
    refused(
        &mut c,
        "save_song_as",
        json!({ "folder": dir.join("none"), "name": "a" }),
    );
    refused(
        &mut c,
        "save_song_as",
        json!({ "folder": dir, "name": "a/b" }),
    );

    ok(
        &mut c,
        "write_notes",
        json!({ "track": 1, "notes": [note("1.1.0", 60)] }),
    );
    let unsaved = "保存していない変更があります。先に save_song するか、人に聞いてください。";
    assert_eq!(refused(&mut c, "new_song", json!({})), unsaved);
    let saved = ok(
        &mut c,
        "save_song_as",
        json!({ "folder": dir, "name": "最初の曲" }),
    );
    let file = PathBuf::from(saved["file"].as_str().unwrap());
    assert_eq!(file, dir.join("最初の曲").join("最初の曲.meloet"));
    assert!(file.is_file());
    assert!(!r.core.ai_song().unwrap().unsaved);

    ok(
        &mut c,
        "write_notes",
        json!({ "track": 1, "notes": [note("2.1.0", 62)] }),
    );
    assert_eq!(
        refused(&mut c, "open_song", json!({ "path": file })),
        unsaved
    );
    refused(&mut c, "open_song", json!({ "path": dir }));
    let again = ok(&mut c, "save_song", json!({}));
    assert_eq!(again["file"], file.display().to_string());

    ok(&mut c, "new_song", json!({}));
    let fresh = r.core.ai_song().unwrap();
    assert!(fresh.file.is_none());
    assert!(fresh.song.doc.tracks.iter().all(|t| t.clips.is_empty()));
    let opened = ok(&mut c, "open_song", json!({ "path": file }));
    assert_eq!(opened["file"], file.display().to_string());
    let song = r.core.ai_song().unwrap();
    let notes = song.song.doc.tracks[0].clips[0]
        .active_take()
        .take
        .notes
        .len();
    assert_eq!(notes, 2);
}

#[test]
fn the_export_writes_over_a_file_only_when_asked() {
    let r = running("export");
    let mut c = Client::start(&r.socket);
    let dir = scratch("export-out");
    refused(&mut c, "export_song", json!({ "folder": dir }));
    ok(&mut c, "add_track", json!({ "sound": "sine" }));
    ok(
        &mut c,
        "write_notes",
        json!({ "track": 2, "notes": [note("1.1.0", 69)] }),
    );
    let steps_before = steps(&r);
    let args = json!({ "folder": dir, "name": "mix", "tail": false });
    let first = ok(&mut c, "export_song", args.clone());
    let file = dir.join("mix.wav");
    assert_eq!(first["files"][0], file.display().to_string(), "{first}");
    let bytes = std::fs::read(&file).unwrap();
    assert_eq!(u16::from_le_bytes([bytes[34], bytes[35]]), 24, "24 bits");
    let text = refused(&mut c, "export_song", args.clone());
    assert!(text.contains("overwrite"), "{text}");
    let mut over = args.clone();
    over["overwrite"] = json!(true);
    ok(&mut c, "export_song", over);
    let mut mp3 = args.clone();
    mp3["format"] = json!("mp3");
    refused(&mut c, "export_song", mp3);
    let flac = ok(
        &mut c,
        "export_song",
        json!({ "folder": dir, "format": "flac", "bits": 16, "range": "tracks" }),
    );
    assert_eq!(flac["files"].as_array().unwrap().len(), 2, "{flac}");
    assert_eq!(steps(&r), steps_before, "exporting is no step");
}

#[test]
fn every_tool_says_what_it_is() {
    let r = running("list");
    let mut c = Client::start(&r.socket);
    let list = c.request("tools/list", json!({}), &mut Vec::new());
    let tools = list["result"]["tools"].as_array().unwrap();
    assert!(tools.len() >= 60, "{} tools", tools.len());
    let named = |n: &str| {
        (tools.iter())
            .find(|t| t["name"] == n)
            .unwrap_or_else(|| panic!("{n}"))
    };
    for t in tools {
        assert_eq!(t["annotations"]["openWorldHint"], false, "{t}");
        let words = t["description"]
            .as_str()
            .unwrap()
            .split_whitespace()
            .count();
        assert!(words <= 60, "{} has {words} words", t["name"]);
    }
    for n in [
        "delete_track",
        "delete_clips",
        "remove_effect",
        "delete_lane",
        "delete_automation",
        "delete_marker",
        "replace_effect",
    ] {
        assert_eq!(named(n)["annotations"]["destructiveHint"], true, "{n}");
    }
    for n in [
        "set_volume",
        "insert_effect",
        "save_song",
        "export_song",
        "move_notes",
        "set_plugin_output",
    ] {
        assert_eq!(named(n)["annotations"]["destructiveHint"], false, "{n}");
    }
    for n in [
        "get_mixer",
        "list_effects",
        "list_instruments",
        "get_automation",
        "render_note",
        "render_audio",
        "get_clip_audio",
        "get_plugin_params",
        "measure_plugin_outputs",
    ] {
        assert_eq!(named(n)["annotations"]["readOnlyHint"], true, "{n}");
    }
}

/// A 16-bit stereo WAV of a 440 Hz sine.
fn sine_wav(path: &Path, seconds: f64) {
    let rate = 48_000u32;
    let frames = (f64::from(rate) * seconds) as u32;
    let mut bytes = Vec::new();
    let data = frames * 4;
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data.to_le_bytes());
    for i in 0..frames {
        let t = f64::from(i) / f64::from(rate);
        let s = ((t * 440.0 * std::f64::consts::TAU).sin() * 12_000.0) as i16;
        bytes.extend_from_slice(&s.to_le_bytes());
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn an_audio_file_comes_in_and_its_clip_is_changed() {
    let r = running("audio");
    let mut c = Client::start(&r.socket);
    let before = r.core.ai_song().unwrap();
    let dir = scratch("audio-in");
    let file = dir.join("voice.wav");
    sine_wav(&file, 1.0);
    let at = steps(&r);
    let brought = ok(
        &mut c,
        "import_audio",
        json!({ "path": file, "at": "2.1.0" }),
    );
    assert_eq!(steps(&r), at + 1, "{brought}");
    let overview = ok(&mut c, "song_overview", json!({}));
    let clip = (overview["clips"].as_array().unwrap().iter())
        .find(|c| c["kind"] == "audio")
        .unwrap_or_else(|| panic!("{overview}"));
    assert_eq!(clip["start"]["at"], "2.1.0");
    let id = clip["id"].clone();
    one_step_each(
        &mut c,
        &r,
        &[
            (
                "set_clip",
                json!({ "clip": id, "field": "gain_db", "value": -6 }),
            ),
            (
                "set_clip",
                json!({ "clip": id, "field": "fade_in", "value": 480 }),
            ),
            ("set_clip_stretch", json!({ "clip": id, "semitones": 2 })),
        ],
    );
    let audio = ok(&mut c, "get_clip_audio", json!({ "clip": id }));
    let (_, _, _, samples) = wav(Path::new(audio["files"][0].as_str().unwrap()));
    assert!(samples.iter().any(|s| s.unsigned_abs() > 1000), "{audio}");
    let broken = dir.join("broken.wav");
    std::fs::write(&broken, b"not audio").unwrap();
    refused(&mut c, "import_audio", json!({ "path": broken }));
    undo_all(&mut c, &r, &before);
}

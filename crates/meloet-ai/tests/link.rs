//! The AI link through its socket, as `meloet-mcp` reaches it: a core with no devices, the
//! link's thread, and a small MCP client of lines.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use meloet_ai::{AiLink, CoreLink, Meloet, Options};
use meloet_core::ai::{AiAnswer, AiAsk, AiBusy, AiLevels, AiMeasure, AiRefusal, AiSong};
use meloet_core::{Core, CoreOptions};
use meloet_protocol::AiPort;
use serde_json::{Value, json};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("k10ai-link-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn my_uid() -> u32 {
    // SAFETY: geteuid takes nothing and cannot fail.
    unsafe { libc::geteuid() }
}

fn options(runtime: &Path) -> Options {
    Options {
        runtime_dir: Some(runtime.to_path_buf()),
        expect_uid: my_uid(),
    }
}

fn wait_for(what: &str, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(10);
    while !ok() {
        assert!(Instant::now() < until, "waited too long for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

struct Client {
    writer: UnixStream,
    reader: BufReader<UnixStream>,
    next: u64,
}

impl Client {
    fn connect(path: &Path) -> Client {
        let writer = UnixStream::connect(path).expect("the socket answers");
        writer
            .set_read_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        let reader = BufReader::new(writer.try_clone().unwrap());
        Client {
            writer,
            reader,
            next: 1,
        }
    }

    fn line(&mut self) -> Option<Value> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(serde_json::from_str(&line).expect("one JSON message a line")),
        }
    }

    fn notify(&mut self, method: &str) {
        let msg = json!({ "jsonrpc": "2.0", "method": method });
        writeln!(self.writer, "{msg}").unwrap();
    }

    /// The answer to the request; notifications that come before it are kept in `seen`.
    fn request(&mut self, method: &str, params: Value, seen: &mut Vec<Value>) -> Value {
        let id = self.next;
        self.next += 1;
        let msg = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        writeln!(self.writer, "{msg}").unwrap();
        loop {
            let line = self.line().expect("an answer");
            if line["id"] == json!(id) {
                return line;
            }
            seen.push(line);
        }
    }

    fn start(path: &Path) -> Client {
        let mut c = Client::connect(path);
        let init = c.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": { "name": "k10ai-test", "version": "0" },
            }),
            &mut Vec::new(),
        );
        assert_eq!(init["result"]["serverInfo"]["name"], "meloet", "{init}");
        c.notify("notifications/initialized");
        c
    }

    fn call(&mut self, tool: &str, args: Value) -> Value {
        self.call_seeing(tool, args, &mut Vec::new())
    }

    fn call_seeing(&mut self, tool: &str, args: Value, seen: &mut Vec<Value>) -> Value {
        let answer = self.request(
            "tools/call",
            json!({ "name": tool, "arguments": args }),
            seen,
        );
        answer["result"].clone()
    }
}

fn port_of(core: &Core) -> AiPort {
    core.snapshot().state.ai_link.port
}

/// The core holds the link, as in meloet: the switch reaches the socket through the core.
struct Running {
    core: Arc<Core>,
    socket: PathBuf,
}

fn running(name: &str) -> Running {
    let runtime = scratch(name);
    let options = options(&runtime);
    let socket = options.socket_path().unwrap();
    let core = Arc::new(Core::start(CoreOptions::offline()));
    let link = AiLink::start(Arc::new(CoreLink(Arc::downgrade(&core))), options).unwrap();
    core.set_ai_switch(Box::new(move |on| link.switch(on)));
    core.send(meloet_protocol::Command::SetAiLink { on: true });
    wait_for("the socket", || socket.exists());
    wait_for("Waiting", || port_of(&core) == AiPort::Waiting);
    Running { core, socket }
}

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
    let core = Arc::new(Core::start(CoreOptions::offline()));
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
        .map(|_| c.call("list_sounds", json!({})))
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

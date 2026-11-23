//! What the tests of the AI link share: a core with no devices, the link's thread, and a
//! small MCP client of lines.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use meloet_ai::{AiLink, CoreLink, Options};
use meloet_core::ai::AiSong;
use meloet_core::{Core, CoreOptions};
use meloet_protocol::AiPort;
use serde_json::{Value, json};

pub fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("k10ai2-link-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn my_uid() -> u32 {
    // SAFETY: geteuid takes nothing and cannot fail.
    unsafe { libc::geteuid() }
}

pub fn options(runtime: &Path) -> Options {
    let temp = runtime.join("tmp");
    std::fs::create_dir_all(&temp).unwrap();
    Options {
        runtime_dir: Some(runtime.to_path_buf()),
        expect_uid: my_uid(),
        temp_dir: temp,
        presets: meloet_ai::PresetDirs::home(&runtime.join("home")),
    }
}

/// A core with no devices, whose export works in a folder of its own: tests run at once in
/// one process, and the AI link's jobs of each core count from the same number.
pub fn offline(runtime: &Path) -> CoreOptions {
    CoreOptions {
        state_dir: Some(runtime.join("state")),
        ..CoreOptions::offline()
    }
}

pub fn wait_for(what: &str, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(10);
    while !ok() {
        assert!(Instant::now() < until, "waited too long for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub struct Client {
    pub writer: UnixStream,
    pub reader: BufReader<UnixStream>,
    pub next: u64,
}

impl Client {
    pub fn connect(path: &Path) -> Client {
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

    pub fn line(&mut self) -> Option<Value> {
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(serde_json::from_str(&line).expect("one JSON message a line")),
        }
    }

    pub fn notify(&mut self, method: &str) {
        let msg = json!({ "jsonrpc": "2.0", "method": method });
        writeln!(self.writer, "{msg}").unwrap();
    }

    /// The answer to the request; notifications that come before it are kept in `seen`.
    pub fn request(&mut self, method: &str, params: Value, seen: &mut Vec<Value>) -> Value {
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

    pub fn start(path: &Path) -> Client {
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

    pub fn call(&mut self, tool: &str, args: Value) -> Value {
        self.call_seeing(tool, args, &mut Vec::new())
    }

    pub fn call_once(&mut self, tool: &str) -> Value {
        let params = json!({ "name": tool, "arguments": {} });
        self.request("tools/call", params, &mut Vec::new())["result"].clone()
    }

    /// Waits and calls again when the calls came too fast.
    pub fn call_seeing(&mut self, tool: &str, args: Value, seen: &mut Vec<Value>) -> Value {
        loop {
            let answer = self.request(
                "tools/call",
                json!({ "name": tool, "arguments": args }),
                seen,
            );
            let result = answer["result"].clone();
            let text = result["content"][0]["text"].as_str().unwrap_or("");
            if result["isError"] == true && text.starts_with("too many calls") {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            return result;
        }
    }
}

pub fn port_of(core: &Core) -> AiPort {
    core.snapshot().state.ai_link.port
}

/// The core holds the link, as in meloet: the switch reaches the socket through the core.
pub struct Running {
    pub core: Arc<Core>,
    pub socket: PathBuf,
}

pub fn running(name: &str) -> Running {
    let runtime = scratch(name);
    let options = options(&runtime);
    let socket = options.socket_path().unwrap();
    let core = Arc::new(Core::start(offline(&runtime)));
    let link = AiLink::start(Arc::new(CoreLink(Arc::downgrade(&core))), options).unwrap();
    core.set_ai_switch(Box::new(move |on| link.switch(on)));
    core.send(meloet_protocol::Command::SetAiLink { on: true });
    wait_for("the socket", || socket.exists());
    wait_for("Waiting", || port_of(&core) == AiPort::Waiting);
    Running { core, socket }
}

/// The structured answer of a call that must work.
pub fn ok(c: &mut Client, tool: &str, args: Value) -> Value {
    let answer = c.call(tool, args);
    assert_eq!(answer["isError"], false, "{tool}: {answer}");
    answer["structuredContent"].clone()
}

/// The text of a call that must fail.
pub fn refused(c: &mut Client, tool: &str, args: Value) -> String {
    let answer = c.call(tool, args);
    assert_eq!(answer["isError"], true, "{tool} must fail: {answer}");
    answer["content"][0]["text"].as_str().unwrap().to_owned()
}

pub fn steps(r: &Running) -> u32 {
    r.core.ai_song().unwrap().undo_count
}

/// Undoing every step since `before` brings the song back.
pub fn undo_all(c: &mut Client, r: &Running, before: &AiSong) {
    let now = steps(r);
    for _ in before.undo_count..now {
        ok(c, "undo", json!({}));
    }
    let back = r.core.ai_song().unwrap();
    assert_eq!(back.undo_count, before.undo_count);
    assert_eq!(back.song.doc.tracks, before.song.doc.tracks);
    assert_eq!(back.song.doc.returns, before.song.doc.returns);
    assert_eq!(back.song.doc.master, before.song.doc.master);
    assert_eq!(back.song.doc.timeline, before.song.doc.timeline);
    assert_eq!(back.song.doc.markers, before.song.doc.markers);
    assert_eq!(back.song.doc.loop_range, before.song.doc.loop_range);
}

/// Calls that each must be exactly one step.
pub fn one_step_each(c: &mut Client, r: &Running, calls: &[(&str, Value)]) {
    for (tool, args) in calls {
        let before = steps(r);
        ok(c, tool, args.clone());
        assert_eq!(steps(r), before + 1, "{tool} {args} is one step");
    }
}

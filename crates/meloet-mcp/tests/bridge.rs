//! `meloet-mcp` as an AI app starts it: on its own while meloet is away, then copying to a
//! meloet that comes later (here a pretend one of a few lines), and on its own again when
//! that one goes.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::{Value, json};

const OFF: &str = "meloet で「音の設定 ▸ くわしい設定 ▸ AI とつなぐ」をオンにしてください。";

fn runtime(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("k10ai-mcp-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("meloet")).unwrap();
    dir
}

struct Bridge {
    child: Child,
    stdin: Option<ChildStdin>,
    lines: mpsc::Receiver<Value>,
    next: u64,
    new: bool,
}

impl Bridge {
    fn start(runtime: &Path, new: bool) -> Bridge {
        let mut child = Command::new(env!("CARGO_BIN_EXE_meloet-mcp"))
            .env("XDG_RUNTIME_DIR", runtime)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout: ChildStdout = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let line = line.unwrap();
                let value: Value = serde_json::from_str(&line)
                    .unwrap_or_else(|_| panic!("only MCP on stdout: {line}"));
                if tx.send(value).is_err() {
                    break;
                }
            }
        });
        Bridge {
            child,
            stdin: Some(stdin),
            lines,
            next: 1,
            new,
        }
    }

    fn meta(&self) -> Value {
        json!({
            "io.modelcontextprotocol/protocolVersion": "2026-07-28",
            "io.modelcontextprotocol/clientCapabilities": {},
        })
    }

    fn send(&mut self, msg: &Value) {
        let stdin = self.stdin.as_mut().unwrap();
        writeln!(stdin, "{msg}").unwrap();
        stdin.flush().unwrap();
    }

    fn line(&self) -> Value {
        self.lines
            .recv_timeout(Duration::from_secs(10))
            .expect("a line from the bridge")
    }

    /// The answer, and the notifications before it.
    fn request(&mut self, method: &str, mut params: Value) -> (Value, Vec<Value>) {
        if self.new {
            params["_meta"] = self.meta();
        }
        let id = self.next;
        self.next += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        let mut before = Vec::new();
        loop {
            let line = self.line();
            if line["id"] == json!(id) {
                return (line, before);
            }
            before.push(line);
        }
    }

    fn handshake(&mut self) -> Value {
        if self.new {
            self.request("server/discover", json!({})).0
        } else {
            let (init, _) = self.request(
                "initialize",
                json!({"protocolVersion":"2025-11-25","capabilities":{},
                       "clientInfo":{"name":"k10ai","version":"0"}}),
            );
            self.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
            init
        }
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A pretend meloet: answers each request with what it was, and writes down every line.
struct Fake {
    seen: mpsc::Receiver<Value>,
    thread: Option<JoinHandle<()>>,
    path: PathBuf,
}

impl Fake {
    /// `hang_up_on`: a tools/call of this name is not answered, and the connection ends.
    fn start(runtime: &Path, hang_up_on: &'static str) -> Fake {
        let path = runtime.join("meloet").join("ai.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let (tx, seen) = mpsc::channel();
        let thread = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut out = stream.try_clone().unwrap();
            for line in BufReader::new(stream).lines() {
                let Ok(line) = line else { break };
                let msg: Value = serde_json::from_str(&line).unwrap();
                let _ = tx.send(msg.clone());
                let Some(id) = msg.get("id") else { continue };
                if msg["params"]["name"] == hang_up_on {
                    break;
                }
                let result = match msg["method"].as_str().unwrap() {
                    "initialize" => {
                        json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{}},
                                           "serverInfo":{"name":"meloet","version":"fake"}})
                    }
                    "tools/list" => {
                        json!({"tools":[{"name":"song_overview","inputSchema":{"type":"object"}}]})
                    }
                    method => {
                        json!({"content":[{"type":"text","text":format!("fake {method}")}],"isError":false})
                    }
                };
                let answer = json!({"jsonrpc":"2.0","id":id,"result":result});
                writeln!(out, "{answer}").unwrap();
            }
        });
        Fake {
            seen,
            thread: Some(thread),
            path,
        }
    }

    fn seen(&self) -> Vec<Value> {
        self.seen.try_iter().collect()
    }

    fn end(mut self) {
        if let Some(t) = self.thread.take() {
            t.join().unwrap();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

fn call(b: &mut Bridge, name: &str) -> (Value, Vec<Value>) {
    b.request("tools/call", json!({"name": name, "arguments": {}}))
}

fn the_stub_then_meloet_then_the_stub(new: bool) {
    let runtime = runtime(if new { "new" } else { "old" });
    let mut b = Bridge::start(&runtime, new);

    let hello = b.handshake();
    assert_eq!(hello["result"]["instructions"], OFF, "{hello}");
    if new {
        assert_eq!(hello["result"]["resultType"], "complete");
    }
    let (list, _) = b.request("tools/list", json!({}));
    assert_eq!(list["result"]["tools"][0]["name"], "meloet_status");
    let (off, _) = call(&mut b, "song_overview");
    assert_eq!(off["result"]["isError"], true);
    let socket = runtime.join("meloet").join("ai.sock");
    let said = off["result"]["content"][0]["text"].as_str().unwrap();
    assert!(said.starts_with(OFF), "{said}");
    assert!(said.contains("step: connecting"), "{said}");
    assert!(said.contains(socket.to_str().unwrap()), "{said}");
    assert!(said.contains("NotFound"), "{said}");

    // meloet comes: the next request finds it.
    let fake = Fake::start(&runtime, "hang_up");
    let (status, before) = call(&mut b, "meloet_status");
    assert_eq!(status["result"]["isError"], false, "{status}");
    assert!(
        before
            .iter()
            .any(|m| m["method"] == "notifications/tools/list_changed"),
        "{before:?}"
    );
    let (list, _) = b.request("tools/list", json!({}));
    assert_eq!(
        list["result"]["tools"][0]["name"], "song_overview",
        "{list}"
    );
    let (real, _) = call(&mut b, "song_overview");
    assert_eq!(real["result"]["content"][0]["text"], "fake tools/call");
    // The status tool is the bridge's own, also while meloet is there.
    let (status, _) = call(&mut b, "meloet_status");
    assert_eq!(status["result"]["isError"], false, "{status}");

    let seen = fake.seen();
    let methods: Vec<&str> = seen.iter().filter_map(|m| m["method"].as_str()).collect();
    if new {
        // Each request carries its own `_meta`: no greeting.
        assert_eq!(methods, ["tools/list", "tools/call"], "{seen:?}");
    } else {
        assert_eq!(
            methods,
            [
                "initialize",
                "notifications/initialized",
                "tools/list",
                "tools/call"
            ],
            "{seen:?}"
        );
        // The answer to that `initialize` was the bridge's, not the AI app's.
    }

    // meloet goes in the middle of a call: the call is answered all the same.
    let (cut, _) = call(&mut b, "hang_up");
    assert_eq!(cut["result"]["isError"], true, "{cut}");
    let said = cut["result"]["content"][0]["text"].as_str().unwrap();
    assert!(said.starts_with(OFF), "{said}");
    assert!(said.contains("step: waiting for the answer"), "{said}");
    fake.end();
    let (off, _) = call(&mut b, "song_overview");
    let said = off["result"]["content"][0]["text"].as_str().unwrap();
    assert!(said.starts_with(OFF), "{said}");
    assert!(said.contains("step: connecting"), "{said}");

    // Nothing but MCP came out, and stdin closing ends the bridge.
    drop(b.stdin.take());
    let status = b.child.wait().unwrap();
    assert!(status.success());
    assert!(b.lines.try_recv().is_err());
}

#[test]
fn the_old_handshake() {
    the_stub_then_meloet_then_the_stub(false);
}

#[test]
fn the_new_handshake() {
    the_stub_then_meloet_then_the_stub(true);
}

#[test]
fn meloet_there_from_the_start() {
    let runtime = runtime("there");
    let fake = Fake::start(&runtime, "hang_up");
    let mut b = Bridge::start(&runtime, false);
    let hello = b.handshake();
    assert_eq!(hello["result"]["serverInfo"]["version"], "fake", "{hello}");
    let (real, _) = call(&mut b, "song_overview");
    assert_eq!(real["result"]["isError"], false);
    let methods: Vec<String> = (fake.seen().iter())
        .filter_map(|m| m["method"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(
        methods,
        ["initialize", "notifications/initialized", "tools/call"]
    );
    drop(b);
    fake.end();
}

//! (d) The `meloet-mcp` role: std only, no tokio. Copies lines between stdin/stdout and
//! meloet's socket. When the socket is missing (meloet closed, or the AI link is off):
//!
//!   bridge --off exit    print a message on stderr and exit 1
//!   bridge --off error   answer every request with a JSON-RPC error carrying the message
//!   bridge --off stub    act as a tiny MCP server whose one tool explains how to turn it on,
//!                        and connect to meloet as soon as it appears (default)
//!
//! `--socket PATH` overrides $XDG_RUNTIME_DIR/s10-mcp/ai.sock.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

const OFF_TEXT: &str = "meloet で『AI とつなぐ』をオンにしてください（meloet が開いていないか、設定でオフになっています）。";

#[derive(PartialEq)]
enum Off {
    Exit,
    Error,
    Stub,
}

type Out = Arc<Mutex<std::io::Stdout>>;

fn emit(out: &Out, line: &str) {
    let mut o = out.lock().unwrap();
    let _ = o.write_all(line.as_bytes());
    let _ = o.write_all(b"\n");
    let _ = o.flush();
}

struct Link {
    sock: UnixStream,
    alive: Arc<AtomicBool>,
}

/// Starts copying meloet's lines to stdout. `alive` turns false when meloet hangs up.
fn pump(sock: &UnixStream, out: Out) -> Arc<AtomicBool> {
    let alive = Arc::new(AtomicBool::new(true));
    let flag = alive.clone();
    let reader = BufReader::new(sock.try_clone().expect("clone socket"));
    std::thread::spawn(move || {
        for line in reader.lines() {
            match line {
                Ok(l) => emit(&out, &l),
                Err(_) => break,
            }
        }
        flag.store(false, Ordering::SeqCst);
        eprintln!("bridge: meloet hung up");
    });
    alive
}

fn reply(out: &Out, id: &Value, result: Value) {
    emit(
        out,
        &json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string(),
    );
}

fn reply_error(out: &Out, id: &Value, code: i64, message: &str) {
    emit(
        out,
        &json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
            .to_string(),
    );
}

fn main() {
    let mut off = Off::Stub;
    let mut path = std::env::var_os("XDG_RUNTIME_DIR")
        .map(|d| PathBuf::from(d).join("s10-mcp").join("ai.sock"))
        .unwrap_or_default();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--off" => {
                off = match args.next().as_deref() {
                    Some("exit") => Off::Exit,
                    Some("error") => Off::Error,
                    _ => Off::Stub,
                }
            }
            "--socket" => path = args.next().map(PathBuf::from).unwrap_or_default(),
            _ => {}
        }
    }

    let out: Out = Arc::new(Mutex::new(std::io::stdout()));
    let mut link: Option<Link> = UnixStream::connect(&path).ok().map(|sock| {
        let alive = pump(&sock, out.clone());
        Link { sock, alive }
    });
    if link.is_none() && off == Off::Exit {
        eprintln!("meloet-mcp: {OFF_TEXT}");
        std::process::exit(1);
    }

    // The legacy handshake, kept so a meloet that appears later can be greeted the same way.
    let mut saved_init: Option<String> = None;
    let mut saved_initialized = false;
    let mut told_list = false;

    // S10_LOG=<file> records what the AI app sends, to see which handshake it used.
    let mut log = std::env::var_os("S10_LOG").and_then(|p| {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(p)
            .ok()
    });

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if let Some(f) = &mut log {
            let _ = writeln!(f, "{line}");
        }
        if link
            .as_ref()
            .is_some_and(|l| !l.alive.load(Ordering::SeqCst))
        {
            link = None;
        }
        if let Some(l) = &mut link {
            if writeln!(l.sock, "{line}").is_ok() {
                continue;
            }
            link = None;
        }

        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let id = msg.get("id").cloned();

        if method == "initialize" {
            saved_init = Some(line.clone());
        }
        if method == "notifications/initialized" {
            saved_initialized = true;
        }
        let Some(id) = id else { continue };

        if off == Off::Error {
            reply_error(&out, &id, -32000, OFF_TEXT);
            continue;
        }

        if !matches!(method, "initialize" | "server/discover" | "ping")
            && let Some(mut l) = try_late_connect(&path, &saved_init, saved_initialized, &out)
        {
            if told_list {
                emit(
                    &out,
                    r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#,
                );
            }
            if writeln!(l.sock, "{line}").is_ok() {
                link = Some(l);
                continue;
            }
        }

        match method {
            "initialize" => {
                let version = msg["params"]["protocolVersion"].clone();
                reply(
                    &out,
                    &id,
                    json!({
                        "protocolVersion": version,
                        "capabilities": {"tools": {"listChanged": true}},
                        "serverInfo": {"name": "meloet", "version": "0.0.0"},
                        "instructions": OFF_TEXT,
                    }),
                );
            }
            "ping" => reply(&out, &id, json!({})),
            "tools/list" => {
                told_list = true;
                reply(
                    &out,
                    &id,
                    json!({"tools": [{
                        "name": "meloet_status",
                        "description": "Tells whether meloet is reachable. Call it when meloet's tools are missing.",
                        "inputSchema": {"type": "object", "properties": {}},
                        "annotations": {"readOnlyHint": true, "openWorldHint": false},
                    }]}),
                );
            }
            "tools/call" => reply(
                &out,
                &id,
                json!({
                    "content": [{"type": "text", "text": OFF_TEXT}],
                    "isError": true,
                }),
            ),
            // server/discover and the rest: not offered while meloet is away, so a
            // 2026-07-28 client falls back to initialize.
            _ => reply_error(&out, &id, -32601, OFF_TEXT),
        }
    }
}

fn try_late_connect(
    path: &PathBuf,
    saved_init: &Option<String>,
    saved_initialized: bool,
    out: &Out,
) -> Option<Link> {
    let mut sock = UnixStream::connect(path).ok()?;
    if let Some(init) = saved_init {
        writeln!(sock, "{init}").ok()?;
        let mut first = String::new();
        BufReader::new(sock.try_clone().ok()?)
            .read_line(&mut first)
            .ok()?;
        if saved_initialized {
            writeln!(
                sock,
                r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
            )
            .ok()?;
        }
    }
    eprintln!("bridge: meloet appeared, connected");
    let alive = pump(&sock, out.clone());
    Some(Link { sock, alive })
}

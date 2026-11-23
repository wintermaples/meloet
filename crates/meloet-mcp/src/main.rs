//! `meloet-mcp`: what an AI app starts as its stdio MCP server. It copies lines between
//! stdin/stdout and the socket of a running meloet (`$XDG_RUNTIME_DIR/meloet/ai.sock`), and
//! while meloet is away answers by itself ([`stub`]). Every request looks for the socket
//! again, so turning 「AI とつなぐ」 on later needs no restart of the AI app.
//!
//! Only MCP goes to stdout; what is said for people goes to stderr.

mod lines;
mod stub;

use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use lines::{Line, MAX_LINE, read_line};

type Out = Arc<Mutex<std::io::Stdout>>;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn emit(out: &Out, line: &str) {
    let mut o = lock(out);
    let _ = o.write_all(line.as_bytes());
    let _ = o.write_all(b"\n");
    let _ = o.flush();
}

/// The id of the `initialize` the bridge sends for itself when meloet comes later.
const GREET_ID: &str = "meloet-mcp-greet";

/// What the socket's reader shares with the main thread.
#[derive(Default)]
struct Shared {
    alive: AtomicBool,
    /// Requests sent to meloet and not answered yet: their id (as JSON text) and whether
    /// it was a `tools/call` of the new protocol (`Some(new)`) or something else (`None`).
    pending: Mutex<HashMap<String, Option<bool>>>,
    /// The answer to the bridge's own `initialize` is not the AI app's.
    greeting: AtomicBool,
}

struct Link {
    sock: UnixStream,
    shared: Arc<Shared>,
}

impl Link {
    fn alive(&self) -> bool {
        self.shared.alive.load(Ordering::SeqCst)
    }

    /// Sends one line; a request is remembered until meloet answers it.
    fn send(&mut self, line: &str, msg: Option<&Value>) -> bool {
        if let Some(msg) = msg
            && let (Some(id), Some(method)) = (msg.get("id"), msg["method"].as_str())
        {
            let call = (method == "tools/call").then(|| stub::is_new(msg));
            lock(&self.shared.pending).insert(id.to_string(), call);
        }
        let sent = writeln!(self.sock, "{line}").is_ok();
        if !sent {
            self.shared.alive.store(false, Ordering::SeqCst);
        }
        sent
    }
}

/// Copies meloet's lines to stdout until it hangs up; then answers what it left unanswered.
fn pump(sock: &UnixStream, out: Out, shared: Arc<Shared>) -> std::io::Result<()> {
    let mut reader = BufReader::new(sock.try_clone()?);
    std::thread::Builder::new()
        .name("meloet-mcp-socket".into())
        .spawn(move || {
            loop {
                let line = match read_line(&mut reader, MAX_LINE) {
                    Ok(Line::Text(line)) => line,
                    Ok(Line::Skipped) => continue,
                    Ok(Line::End) | Err(_) => break,
                };
                let msg: Option<Value> = serde_json::from_str(&line).ok();
                let answer_to = msg
                    .as_ref()
                    .filter(|m| m.get("result").is_some() || m.get("error").is_some())
                    .and_then(|m| m.get("id"));
                if let Some(id) = answer_to {
                    if id.as_str() == Some(GREET_ID)
                        && shared.greeting.swap(false, Ordering::SeqCst)
                    {
                        continue;
                    }
                    lock(&shared.pending).remove(&id.to_string());
                }
                emit(&out, &line);
            }
            shared.alive.store(false, Ordering::SeqCst);
            eprintln!("meloet-mcp: meloet went away");
            for (id, call) in lock(&shared.pending).drain() {
                let Ok(id) = serde_json::from_str::<Value>(&id) else {
                    continue;
                };
                let line = match call {
                    Some(new) => stub::result(&id, stub::off_call(new)),
                    None => stub::error(&id, -32000, stub::OFF_TEXT),
                };
                emit(&out, &line);
            }
        })?;
    Ok(())
}

fn socket_path() -> Option<PathBuf> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty())?;
    Some(PathBuf::from(runtime).join("meloet").join("ai.sock"))
}

/// What the AI app said of the old handshake, to say it again to a meloet that comes later.
#[derive(Default)]
struct Greeting {
    initialize: Option<Value>,
    initialized: bool,
}

fn connect(path: Option<&PathBuf>, out: &Out, greeting: &Greeting) -> Option<Link> {
    let sock = UnixStream::connect(path?).ok()?;
    let shared = Arc::new(Shared::default());
    shared.alive.store(true, Ordering::SeqCst);
    let mut link = Link { sock, shared };
    if let Some(init) = &greeting.initialize {
        let mut init = init.clone();
        init["id"] = json!(GREET_ID);
        link.shared.greeting.store(true, Ordering::SeqCst);
        if !link.send(&init.to_string(), None) {
            return None;
        }
        if greeting.initialized {
            let note = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
            if !link.send(note, None) {
                return None;
            }
        }
    }
    pump(&link.sock, out.clone(), link.shared.clone()).ok()?;
    Some(link)
}

fn main() {
    let path = socket_path();
    if path.is_none() {
        eprintln!("meloet-mcp: XDG_RUNTIME_DIR is not set; meloet cannot be found");
    }
    let out: Out = Arc::new(Mutex::new(std::io::stdout()));
    let mut greeting = Greeting::default();
    let mut link = connect(path.as_ref(), &out, &greeting);
    if link.is_none() {
        eprintln!("meloet-mcp: {}", stub::OFF_TEXT);
    }
    // The AI app has the stub's list of tools.
    let mut stub_listed = false;
    let mut input = BufReader::new(std::io::stdin().lock());
    loop {
        let line = match read_line(&mut input, MAX_LINE) {
            Ok(Line::Text(line)) => line,
            Ok(Line::Skipped) => {
                eprintln!("meloet-mcp: a message was too long or not UTF-8; skipped");
                continue;
            }
            Ok(Line::End) | Err(_) => break,
        };
        let msg: Option<Value> = serde_json::from_str(&line).ok();
        let method = (msg
            .as_ref()
            .and_then(|m| m["method"].as_str())
            .unwrap_or(""))
        .to_owned();
        let method = method.as_str();
        match method {
            "initialize" => greeting.initialize = msg.clone(),
            "notifications/initialized" => greeting.initialized = true,
            _ => {}
        }
        if link.as_ref().is_some_and(|l| !l.alive()) {
            link = None;
        }
        if let Some(l) = &mut link {
            if l.send(&line, msg.as_ref()) {
                continue;
            }
            link = None;
        }
        let Some(msg) = msg else { continue };
        let is_request = msg.get("id").is_some();
        let handshake = matches!(method, "initialize" | "server/discover" | "ping");
        if is_request
            && !handshake
            && let Some(mut l) = connect(path.as_ref(), &out, &greeting)
        {
            eprintln!("meloet-mcp: meloet is there now");
            if stub_listed {
                emit(
                    &out,
                    r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#,
                );
            }
            let status_call = method == "tools/call" && msg["params"]["name"] == stub::STATUS_TOOL;
            if status_call {
                emit(
                    &out,
                    &stub::result(&msg["id"], stub::back_call(stub::is_new(&msg))),
                );
                link = Some(l);
                continue;
            }
            if l.send(&line, Some(&msg)) {
                link = Some(l);
                continue;
            }
        }
        if method == "tools/list" {
            stub_listed = true;
        }
        if let Some(answer) = stub::answer(&msg) {
            emit(&out, &answer);
        }
    }
}

//! `meloet-mcp`: what an AI app starts as its stdio MCP server. It copies lines between
//! stdin/stdout and the socket of a running meloet (`$XDG_RUNTIME_DIR/meloet/ai.sock`, see
//! [`place`]), and while meloet is away answers by itself ([`stub`]). Every request looks for
//! the socket again, so turning 「AI とつなぐ」 on later needs no restart of the AI app.
//!
//! Only MCP goes to stdout; what is said for people goes to stderr.

mod lines;
mod place;
mod stub;

use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use lines::{Line, MAX_LINE, read_line};
use place::Place;

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
        self.write(line).is_ok()
    }

    fn write(&mut self, line: &str) -> std::io::Result<()> {
        let sent = writeln!(self.sock, "{line}");
        if sent.is_err() {
            self.shared.alive.store(false, Ordering::SeqCst);
        }
        sent
    }
}

/// Copies meloet's lines to stdout until it hangs up; then answers what it left unanswered.
fn pump(sock: &UnixStream, at: String, out: Out, shared: Arc<Shared>) -> std::io::Result<()> {
    let mut reader = BufReader::new(sock.try_clone()?);
    std::thread::Builder::new()
        .name("meloet-mcp-socket".into())
        .spawn(move || {
            let end = loop {
                let line = match read_line(&mut reader, MAX_LINE) {
                    Ok(Line::Text(line)) => line,
                    Ok(Line::Skipped) => continue,
                    Ok(Line::End) => break "meloet closed the connection".to_owned(),
                    Err(e) => break format!("reading failed ({:?}: {e})", e.kind()),
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
            };
            shared.alive.store(false, Ordering::SeqCst);
            eprintln!("meloet-mcp: meloet went away");
            let why = place::report(
                "waiting for the answer",
                &[format!(
                    "{at}: connected, then {end} before it answered (meloet ended, the \
                     switch went off, or meloet runs as another user and refused this one)"
                )],
            );
            for (id, call) in lock(&shared.pending).drain() {
                let Ok(id) = serde_json::from_str::<Value>(&id) else {
                    continue;
                };
                let line = match call {
                    Some(new) => stub::result(&id, stub::off_call(new, &why)),
                    None => stub::error(&id, -32000, stub::OFF_TEXT),
                };
                emit(&out, &line);
            }
        })?;
    Ok(())
}

/// What the AI app said of the old handshake, to say it again to a meloet that comes later.
#[derive(Default)]
struct Greeting {
    initialize: Option<Value>,
    initialized: bool,
}

/// Connects to the first place that answers; the error is the report of what was tried.
fn connect(places: &[Place], out: &Out, greeting: &Greeting) -> Result<Link, String> {
    if places.is_empty() {
        let none = "XDG_RUNTIME_DIR is not set and the user id could not be read".to_owned();
        return Err(place::report("finding the socket", &[none]));
    }
    let mut tried = Vec::new();
    for place in places {
        let at = format!("{} (from {})", place.path.display(), place.from);
        match UnixStream::connect(&place.path) {
            Ok(sock) => {
                return greet(sock, &at, out, greeting).map_err(|e| {
                    let line = format!("{at}: connected, then {:?}: {e}", e.kind());
                    place::report("handshake", &[line])
                });
            }
            Err(e) => tried.push(format!(
                "{at}: {:?}: {e}; {}",
                e.kind(),
                place::seen(&place.path)
            )),
        }
    }
    Err(place::report("connecting", &tried))
}

fn greet(sock: UnixStream, at: &str, out: &Out, greeting: &Greeting) -> std::io::Result<Link> {
    let shared = Arc::new(Shared::default());
    shared.alive.store(true, Ordering::SeqCst);
    let mut link = Link { sock, shared };
    if let Some(init) = &greeting.initialize {
        let mut init = init.clone();
        init["id"] = json!(GREET_ID);
        link.shared.greeting.store(true, Ordering::SeqCst);
        link.write(&init.to_string())?;
        if greeting.initialized {
            link.write(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)?;
        }
    }
    pump(&link.sock, at.to_owned(), out.clone(), link.shared.clone())?;
    Ok(link)
}

fn main() {
    let places = place::places();
    let out: Out = Arc::new(Mutex::new(std::io::stdout()));
    let mut greeting = Greeting::default();
    // Why meloet could not be reached the last time it was tried.
    let mut why = String::new();
    let mut link = match connect(&places, &out, &greeting) {
        Ok(link) => Some(link),
        Err(report) => {
            eprintln!("meloet-mcp: {}\n{report}", stub::OFF_TEXT);
            why = report;
            None
        }
    };
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
        // meloet has no status tool of its own; an AI app may still hold the stub's list.
        let status_call = msg.as_ref().is_some_and(|m| {
            method == "tools/call"
                && m["params"]["name"] == stub::STATUS_TOOL
                && m.get("id").is_some()
        });
        if let Some(l) = &mut link {
            if status_call && let Some(msg) = &msg {
                emit(
                    &out,
                    &stub::result(&msg["id"], stub::back_call(stub::is_new(msg))),
                );
                continue;
            }
            if l.send(&line, msg.as_ref()) {
                continue;
            }
            link = None;
        }
        let Some(msg) = msg else { continue };
        let is_request = msg.get("id").is_some();
        let handshake = matches!(method, "initialize" | "server/discover" | "ping");
        let found = if is_request && !handshake {
            connect(&places, &out, &greeting)
                .map_err(|report| why = report)
                .ok()
        } else {
            None
        };
        if let Some(mut l) = found {
            eprintln!("meloet-mcp: meloet is there now");
            if stub_listed {
                emit(
                    &out,
                    r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#,
                );
            }
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
        if let Some(answer) = stub::answer(&msg, &why) {
            emit(&out, &answer);
        }
    }
}

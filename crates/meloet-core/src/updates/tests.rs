//! The look for a new version against a small HTTP server on this PC (never the net).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{CHECK_EVERY, Options, Version, check, newer, release_page};

/// How the server answers.
#[derive(Debug, Clone)]
enum Way {
    Json(String),
    Status(u16),
    /// Takes the request and never answers.
    Silent,
}

struct Server {
    url: String,
    way: Arc<Mutex<Way>>,
    /// The headers of every request, lower case.
    log: Arc<Mutex<Vec<HashMap<String, String>>>>,
}

impl Server {
    fn start(way: Way) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "http://{}/repos/meloet/releases/latest",
            listener.local_addr().unwrap()
        );
        let way = Arc::new(Mutex::new(way));
        let log = Arc::new(Mutex::new(Vec::new()));
        let (shared_way, shared_log) = (way.clone(), log.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (way, log) = (shared_way.clone(), shared_log.clone());
                std::thread::spawn(move || answer(stream, &way, &log));
            }
        });
        Server { url, way, log }
    }

    fn set(&self, way: Way) {
        *self.way.lock().unwrap() = way;
    }

    fn requests(&self) -> Vec<HashMap<String, String>> {
        self.log.lock().unwrap().clone()
    }
}

fn answer(stream: TcpStream, way: &Mutex<Way>, log: &Mutex<Vec<HashMap<String, String>>>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut headers = HashMap::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).is_err() || h.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = h.trim().split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_owned());
        }
    }
    log.lock().unwrap().push(headers);
    let way = way.lock().unwrap().clone();
    let mut out = stream;
    let (status, body) = match way {
        Way::Json(body) => ("200 OK".to_owned(), body),
        Way::Status(code) => (format!("{code} No"), String::new()),
        Way::Silent => {
            std::thread::sleep(Duration::from_secs(5));
            return;
        }
    };
    let _ = write!(
        out,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

fn release(tag: &str) -> Way {
    Way::Json(format!(
        r#"{{"url":"x","html_url":"x","tag_name":"{tag}","name":"meloet {tag}","draft":false,"prerelease":false,"assets":[]}}"#
    ))
}

fn options(server: &Server, stamp_dir: Option<std::path::PathBuf>) -> Options {
    let mut o = Options::new(server.url.clone(), stamp_dir);
    o.current = "0.3.0".into();
    o.timeout = Duration::from_millis(500);
    o
}

const DAY_MS: u64 = 24 * 60 * 60 * 1000;
const NOW: u64 = 1_800_000_000_000;

#[test]
fn versions_are_three_numbers() {
    assert_eq!(Version::parse("v0.4.0"), Some(Version(0, 4, 0)));
    assert_eq!(Version::parse("0.10.2"), Some(Version(0, 10, 2)));
    for bad in ["v0.4", "v0.4.0.1", "v0.4.0-rc1", "v0..1", "va.b.c", "", "v"] {
        assert_eq!(Version::parse(bad), None, "{bad}");
    }
    assert_eq!(newer("v0.4.0", "0.3.0"), Some("0.4.0".into()));
    assert_eq!(newer("v0.10.0", "0.9.9"), Some("0.10.0".into()));
    assert_eq!(newer("v1.0.0", "0.99.99"), Some("1.0.0".into()));
    assert_eq!(newer("v0.3.0", "0.3.0"), None);
    assert_eq!(newer("v0.2.9", "0.3.0"), None);
    assert_eq!(newer("nightly", "0.3.0"), None);
    assert_eq!(
        release_page("0.4.0"),
        "https://github.com/wintermaples/meloet/releases/tag/v0.4.0"
    );
}

#[test]
fn a_newer_release_is_found_and_only_the_user_agent_is_said() {
    let server = Server::start(release("v0.4.0"));
    assert_eq!(check(&options(&server, None), NOW), Some("0.4.0".into()));
    let requests = server.requests();
    assert_eq!(requests.len(), 1);
    let mut names: Vec<&str> = requests[0].keys().map(String::as_str).collect();
    names.sort();
    assert_eq!(names, ["host", "user-agent"]);
    assert_eq!(
        requests[0]["user-agent"],
        format!("meloet/{}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn the_same_or_an_older_release_is_nothing() {
    let server = Server::start(release("v0.3.0"));
    assert_eq!(check(&options(&server, None), NOW), None);
    server.set(release("v0.2.1"));
    assert_eq!(check(&options(&server, None), NOW), None);
}

#[test]
fn broken_answers_say_nothing() {
    let server = Server::start(Way::Json("{\"tag_name\": ".into()));
    assert_eq!(check(&options(&server, None), NOW), None);
    server.set(Way::Json(r#"{"name":"no tag"}"#.into()));
    assert_eq!(check(&options(&server, None), NOW), None);
    server.set(release("latest"));
    assert_eq!(check(&options(&server, None), NOW), None);
    server.set(Way::Status(404));
    assert_eq!(check(&options(&server, None), NOW), None);
    server.set(Way::Status(403));
    assert_eq!(check(&options(&server, None), NOW), None);
}

#[test]
fn a_server_that_does_not_answer_is_given_up_on_time() {
    let server = Server::start(Way::Silent);
    let started = Instant::now();
    assert_eq!(check(&options(&server, None), NOW), None);
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn no_server_at_all_says_nothing() {
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/latest", closed.local_addr().unwrap());
    drop(closed);
    let mut o = Options::new(url, None);
    o.timeout = Duration::from_millis(500);
    assert_eq!(check(&o, NOW), None);
}

#[test]
fn the_release_is_read_at_most_once_a_day() {
    let dir = crate::settings::temp_dir("updates");
    let server = Server::start(release("v0.4.0"));
    let o = options(&server, Some(dir.clone()));
    assert_eq!(check(&o, NOW), Some("0.4.0".into()));
    assert_eq!(server.requests().len(), 1);
    // Later the same day: not read again, and what it found is still said at a new start.
    server.set(release("v0.5.0"));
    assert_eq!(check(&o, NOW + DAY_MS - 1), Some("0.4.0".into()));
    assert_eq!(server.requests().len(), 1);
    // The person put in 0.4.0 that day: nothing to say.
    let mut put_in = o.clone();
    put_in.current = "0.4.0".into();
    assert_eq!(check(&put_in, NOW + 1000), None);
    assert_eq!(server.requests().len(), 1);
    // A day later it is read again.
    assert_eq!(
        check(&o, NOW + CHECK_EVERY.as_millis() as u64),
        Some("0.5.0".into())
    );
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn a_failed_read_counts_for_the_day_and_keeps_what_was_found() {
    let dir = crate::settings::temp_dir("updates");
    let server = Server::start(release("v0.4.0"));
    let o = options(&server, Some(dir.clone()));
    assert_eq!(check(&o, NOW), Some("0.4.0".into()));
    server.set(Way::Status(500));
    assert_eq!(check(&o, NOW + DAY_MS), Some("0.4.0".into()));
    assert_eq!(server.requests().len(), 2);
    server.set(release("v0.5.0"));
    assert_eq!(check(&o, NOW + DAY_MS + 1), Some("0.4.0".into()));
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn a_broken_stamp_or_a_clock_put_back_reads_again() {
    let dir = crate::settings::temp_dir("updates");
    let server = Server::start(release("v0.4.0"));
    let o = options(&server, Some(dir.clone()));
    std::fs::write(dir.join("update-check.json"), "{oops").unwrap();
    assert_eq!(check(&o, NOW), Some("0.4.0".into()));
    assert_eq!(check(&o, NOW - DAY_MS), Some("0.4.0".into()));
    assert_eq!(server.requests().len(), 2);
}

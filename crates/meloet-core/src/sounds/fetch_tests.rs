//! The fetch against a small HTTP server on this PC (never the net): Range, 302, 403, cuts,
//! broken files, a slow line, a full disk, no server at all, and the look at the catalog.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meloet_model::BuiltinInstrument as B;
use meloet_protocol::PackFailure;
use sha2::{Digest, Sha256};

use super::catalog::{self, Catalog, Trust};
use super::fetch::{Disk, Fetcher, Job, Options, PackNews, data_root, shown, start_catalog};
use super::untar::tests::tar_of;

/// How the server answers one request for a file.
#[derive(Debug, Clone, PartialEq)]
enum Way {
    Plain,
    /// The bytes in pieces of this size, with a pause after each.
    Slow(usize, Duration),
    /// The head says the whole length, and the connection ends after this many bytes.
    Cut(usize),
    /// One byte of the file is changed.
    Broken,
    Status(u16),
    /// 302 to `/real/<name>`, which answers the next way.
    Moved,
}

#[derive(Default)]
struct Served {
    files: HashMap<String, (Vec<u8>, VecDeque<Way>)>,
    /// Every request: the path and its headers, lower case.
    log: Vec<(String, HashMap<String, String>)>,
}

struct Server {
    origin: String,
    served: Arc<Mutex<Served>>,
}

impl Server {
    fn start() -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}/", listener.local_addr().unwrap());
        let served = Arc::new(Mutex::new(Served::default()));
        let shared = served.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let served = shared.clone();
                std::thread::spawn(move || answer(stream, &served));
            }
        });
        Server { origin, served }
    }

    /// `ways`: one for each request in turn; the last stays.
    fn put(&self, name: &str, bytes: Vec<u8>, ways: &[Way]) {
        let ways = ways.iter().cloned().collect();
        let mut served = self.served.lock().unwrap();
        served.files.insert(name.to_owned(), (bytes, ways));
    }

    fn log(&self) -> Vec<(String, HashMap<String, String>)> {
        self.served.lock().unwrap().log.clone()
    }

    fn requests(&self, name: &str) -> Vec<HashMap<String, String>> {
        let path = format!("/{name}");
        (self.log().into_iter())
            .filter(|(p, _)| *p == path || *p == format!("/real/{name}"))
            .map(|(_, h)| h)
            .collect()
    }
}

fn answer(stream: TcpStream, served: &Mutex<Served>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let path = line.split(' ').nth(1).unwrap_or("/").to_owned();
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
    let (body, way) = {
        let mut served = served.lock().unwrap();
        served.log.push((path.clone(), headers.clone()));
        let name = path.trim_start_matches("/real/").trim_start_matches('/');
        match served.files.get_mut(name) {
            Some((body, ways)) => {
                let way = if ways.len() > 1 {
                    ways.pop_front().unwrap()
                } else {
                    ways.front().cloned().unwrap_or(Way::Plain)
                };
                (body.clone(), way)
            }
            None => (Vec::new(), Way::Status(404)),
        }
    };
    let mut out = stream;
    let host = headers.get("host").cloned().unwrap_or_default();
    let head = |status: &str, extra: &str, len: usize| {
        format!("HTTP/1.1 {status}\r\nContent-Length: {len}\r\nConnection: close\r\n{extra}\r\n")
    };
    match way {
        Way::Status(code) => {
            let _ = out.write_all(head(&format!("{code} No"), "", 0).as_bytes());
            return;
        }
        Way::Moved if !path.starts_with("/real/") => {
            let to = format!("Location: http://{host}/real{path}\r\n");
            let _ = out.write_all(head("302 Found", &to, 0).as_bytes());
            return;
        }
        _ => {}
    }
    let mut body = body;
    if way == Way::Broken {
        body[0] ^= 0xff;
    }
    let from = headers
        .get("range")
        .and_then(|r| r.strip_prefix("bytes="))
        .and_then(|r| r.strip_suffix('-'))
        .and_then(|r| r.parse::<usize>().ok());
    let (status, extra, slice) = match from {
        Some(from) if from >= body.len() => {
            let _ = out.write_all(head("416 Range Not Satisfiable", "", 0).as_bytes());
            return;
        }
        Some(from) => (
            "206 Partial Content",
            format!(
                "Content-Range: bytes {from}-{}/{}\r\n",
                body.len() - 1,
                body.len()
            ),
            &body[from..],
        ),
        None => ("200 OK", String::new(), &body[..]),
    };
    let _ = out.write_all(head(status, &extra, slice.len()).as_bytes());
    match way {
        Way::Cut(n) => {
            let _ = out.write_all(&slice[..n.min(slice.len())]);
            let _ = out.flush();
            let _ = out.shutdown(std::net::Shutdown::Both);
        }
        Way::Slow(piece, pause) => {
            for chunk in slice.chunks(piece) {
                if out.write_all(chunk).is_err() {
                    return;
                }
                std::thread::sleep(pause);
            }
        }
        _ => {
            let _ = out.write_all(slice);
        }
    }
}

fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The tar of one part: an instrument (or the common part) with a file of `len` bytes.
fn part_tar(id: &str, len: usize) -> Vec<u8> {
    let filler: Vec<u8> = (0..len).map(|i| (i * 7 % 251) as u8).collect();
    if id == catalog::COMMON_ID {
        tar_of(&[
            ("credits.json", Some(b"[]")),
            ("licenses/x.txt", Some(&filler)),
        ])
    } else {
        tar_of(&[
            ("instrument.json", Some(b"{}")),
            ("samples/a.flac", Some(&filler)),
        ])
    }
}

struct Pack {
    catalog: Catalog,
    text: String,
    tars: Vec<(String, Vec<u8>)>,
}

/// A catalog of the common part and these instruments (version `v1` each, or `version`),
/// whose files are on `server`.
fn pack(server: &Server, parts: &[(&str, &str, usize)]) -> Pack {
    let mut entries = Vec::new();
    let mut tars = Vec::new();
    for (id, version, len) in parts {
        let tar = part_tar(id, *len);
        let file = format!("{id}-{version}.tar");
        entries.push(format!(
            r#"{{"id": "{id}", "version": "{version}", "file": "{file}", "url": "{}{file}", "bytes": {}, "sha256": "{}"}}"#,
            server.origin,
            tar.len(),
            sha(&tar)
        ));
        tars.push((file, tar));
    }
    let text = format!(
        r#"{{"format": "meloet-sounds-catalog", "version": 1, "pack": "2026.12.0", "common": {}, "instruments": [{}]}}"#,
        entries[0],
        entries[1..].join(", ")
    );
    let catalog = catalog::check(&text, &trust(server)).unwrap();
    Pack {
        catalog,
        text,
        tars,
    }
}

fn trust(server: &Server) -> Trust {
    Trust {
        urls_under: server.origin.clone(),
    }
}

const PARTS: [(&str, &str, usize); 3] = [
    ("common", "c1", 3000),
    ("studioKit", "s1", 20_000),
    ("grandPiano", "g1", 40_000),
];

struct Rig {
    dir: PathBuf,
    fetcher: Fetcher,
    news: Receiver<PackNews>,
    pack: Pack,
}

impl Rig {
    fn new(
        label: &str,
        server: &Server,
        ways: &[(&str, &[Way])],
        change: impl FnOnce(&mut Options),
    ) -> Rig {
        let pack = pack(server, &PARTS);
        for (file, tar) in &pack.tars {
            let id = file.split('-').next().unwrap();
            let way = (ways.iter())
                .find(|(w, _)| *w == id)
                .map_or(&[][..], |(_, w)| *w);
            server.put(file, tar.clone(), way);
        }
        Rig::with(label, server, pack, change)
    }

    fn with(label: &str, server: &Server, pack: Pack, change: impl FnOnce(&mut Options)) -> Rig {
        let dir = crate::settings::temp_dir(label);
        let disk = Disk {
            root: dir.join("sounds"),
            covered: Vec::new(),
        };
        let mut options = Options::new(disk, Some(dir.join("state")));
        options.trust = trust(server);
        options.online = format!("{}catalog.json", server.origin);
        options.https_only = false;
        options.connect_timeout = Duration::from_secs(2);
        options.read_timeout = Duration::from_secs(2);
        options.retry_pause = Duration::from_millis(10);
        change(&mut options);
        let (tx, news) = channel();
        let tell = move |n| {
            let _ = tx.send(n);
        };
        let fetcher = Fetcher::spawn(options, pack.catalog.clone(), Box::new(tell)).unwrap();
        Rig {
            dir,
            fetcher,
            news,
            pack,
        }
    }

    fn root(&self) -> PathBuf {
        self.dir.join("sounds")
    }

    /// Everything told until the fetch of `round` ends.
    fn fetch(&self, round: u64) -> Vec<PackNews> {
        self.fetcher.ask(Job::Fetch { round });
        self.until_end()
    }

    fn until_end(&self) -> Vec<PackNews> {
        let mut told = Vec::new();
        loop {
            let n = self
                .news
                .recv_timeout(Duration::from_secs(20))
                .expect("the fetch ends");
            let end = matches!(n, PackNews::Done { .. } | PackNews::Failed { .. });
            told.push(n);
            if end {
                return told;
            }
        }
    }

    fn installed(told: &[PackNews]) -> Vec<Option<B>> {
        (told.iter())
            .filter_map(|n| match n {
                PackNews::Installed { instrument, .. } => Some(*instrument),
                _ => None,
            })
            .collect()
    }

    fn here(&self, id: &str, version: &str) -> bool {
        let marker = if id == "common" {
            "credits.json"
        } else {
            "instrument.json"
        };
        self.root().join(id).join(version).join(marker).is_file()
    }

    fn parts_left(&self) -> Vec<String> {
        (std::fs::read_dir(self.root()).unwrap().flatten())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".part") || n.starts_with(".work-"))
            .collect()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn last(told: &[PackNews]) -> &PackNews {
    told.last().unwrap()
}

#[test]
fn a_fetch_brings_every_part_in_its_order_and_puts_it_in_place() {
    let server = Server::start();
    let rig = Rig::new(
        "pack-plain",
        &server,
        &[("grandPiano", &[Way::Moved])],
        |_| {},
    );
    let told = rig.fetch(1);
    assert_eq!(last(&told), &PackNews::Done { round: 1 });
    // The common part, then the urgent ones of B-95-1: the grand piano before the kit.
    assert_eq!(
        Rig::installed(&told),
        [None, Some(B::GrandPiano), Some(B::StudioKit)]
    );
    for (id, version, _) in PARTS {
        assert!(rig.here(id, version), "{id}");
    }
    assert!(rig.parts_left().is_empty());
    let json = told.iter().find_map(|n| match n {
        PackNews::Installed {
            instrument: Some(B::GrandPiano),
            json,
            ..
        } => Some(json.clone()),
        _ => None,
    });
    assert_eq!(json, Some(rig.root().join("grandPiano/g1/instrument.json")));
    // The progress counts up to the whole pack.
    let total = rig.pack.catalog.bytes();
    let first = told.iter().find_map(|n| match n {
        PackNews::Progress { numbers, .. } => Some(*numbers),
        _ => None,
    });
    assert_eq!(first.map(|n| (n.done, n.total)), Some((0, total)));
    // Only the name of meloet and its version are sent, and the 302 was followed.
    let piano = server.requests("grandPiano-g1.tar");
    assert_eq!(piano.len(), 2);
    for request in server.log().iter().map(|(_, h)| h) {
        assert_eq!(
            request.get("user-agent").map(String::as_str),
            Some(concat!("meloet/", env!("CARGO_PKG_VERSION")))
        );
        assert!(!request.contains_key("cookie") && !request.contains_key("referer"));
    }
    let disk = Disk {
        root: rig.root(),
        covered: Vec::new(),
    };
    let outlook = disk.outlook(&rig.pack.catalog);
    assert!(!outlook.missing && outlook.fetched_any);
    assert_eq!(disk.instruments(&rig.pack.catalog).len(), 2);
    // Nothing left: a fetch says so at once, without a request.
    let before = server.log().len();
    assert_eq!(rig.fetch(2), [PackNews::Done { round: 2 }]);
    assert_eq!(server.log().len(), before);
}

#[test]
fn a_slow_line_is_waited_for() {
    let server = Server::start();
    let slow = [Way::Slow(1000, Duration::from_millis(20))];
    let rig = Rig::new("pack-slow", &server, &[("grandPiano", &slow)], |_| {});
    let told = rig.fetch(1);
    assert_eq!(last(&told), &PackNews::Done { round: 1 });
    let progress = (told.iter())
        .filter(|n| {
            matches!(
                n,
                PackNews::Progress {
                    instrument: Some(B::GrandPiano),
                    ..
                }
            )
        })
        .count();
    assert!(
        progress >= 2,
        "the progress is told while it comes: {progress}"
    );
}

#[test]
fn a_cut_file_goes_on_from_where_it_was_cut() {
    let server = Server::start();
    // Cut once: taken up again quietly, with Range.
    let rig = Rig::new(
        "pack-cut",
        &server,
        &[("grandPiano", &[Way::Cut(10_000), Way::Plain])],
        |_| {},
    );
    let told = rig.fetch(1);
    assert_eq!(last(&told), &PackNews::Done { round: 1 });
    let ranges: Vec<_> = (server.requests("grandPiano-g1.tar").iter())
        .map(|h| h.get("range").cloned())
        .collect();
    assert_eq!(ranges, [None, Some("bytes=10000-".into())]);

    // Cut every time: after the quiet tries the fetch says so, and keeps what came.
    let server = Server::start();
    let rig = Rig::new(
        "pack-cut-again",
        &server,
        &[("grandPiano", &[Way::Cut(5000)])],
        |_| {},
    );
    let told = rig.fetch(1);
    let PackNews::Failed {
        failure: PackFailure::CutOff,
        numbers,
        ..
    } = last(&told)
    else {
        panic!("{told:?}");
    };
    assert!(numbers.done > 0 && numbers.done < numbers.total);
    let part = rig.root().join("grandPiano-g1.tar.part");
    let kept = std::fs::metadata(&part).unwrap().len();
    assert_eq!(kept, 15_000, "three tries of 5000 bytes");
    // 「続きから」: the next fetch asks for the rest.
    let (file, tar) = rig
        .pack
        .tars
        .iter()
        .find(|(f, _)| f.starts_with("grandPiano"))
        .unwrap();
    server.put(file, tar.clone(), &[Way::Plain]);
    let told = rig.fetch(2);
    assert_eq!(last(&told), &PackNews::Done { round: 2 });
    let asked = server.requests("grandPiano-g1.tar");
    assert_eq!(
        asked.last().unwrap().get("range").map(String::as_str),
        Some("bytes=15000-")
    );
    assert!(rig.here("grandPiano", "g1"));
}

#[test]
fn a_broken_file_is_fetched_again_once_quietly_then_said() {
    let server = Server::start();
    let rig = Rig::new(
        "pack-broken",
        &server,
        &[("studioKit", &[Way::Broken, Way::Plain])],
        |_| {},
    );
    let told = rig.fetch(1);
    assert_eq!(last(&told), &PackNews::Done { round: 1 });
    assert!(!told.iter().any(|n| matches!(n, PackNews::Failed { .. })));
    assert_eq!(server.requests("studioKit-s1.tar").len(), 2);

    let server = Server::start();
    let rig = Rig::new(
        "pack-broken-twice",
        &server,
        &[("studioKit", &[Way::Broken])],
        |_| {},
    );
    let told = rig.fetch(1);
    assert!(
        matches!(
            last(&told),
            PackNews::Failed {
                failure: PackFailure::Corrupt,
                ..
            }
        ),
        "{told:?}"
    );
    assert_eq!(server.requests("studioKit-s1.tar").len(), 2);
    assert!(rig.parts_left().is_empty(), "the broken file is not kept");
    assert!(rig.here("grandPiano", "g1"), "what came before stays");
    assert!(!rig.here("studioKit", "s1"));
}

#[test]
fn a_link_that_ran_out_is_followed_again_from_the_catalog() {
    let server = Server::start();
    let ways = [Way::Moved, Way::Status(403), Way::Moved, Way::Plain];
    let rig = Rig::new("pack-gone", &server, &[("grandPiano", &ways)], |_| {});
    let told = rig.fetch(1);
    assert_eq!(last(&told), &PackNews::Done { round: 1 });
    let paths: Vec<String> = (server.log().into_iter())
        .map(|(p, _)| p)
        .filter(|p| p.contains("grandPiano"))
        .collect();
    assert_eq!(
        paths,
        [
            "/grandPiano-g1.tar",
            "/real/grandPiano-g1.tar",
            "/grandPiano-g1.tar",
            "/real/grandPiano-g1.tar"
        ]
    );

    // Always 403: said as the net not answering.
    let server = Server::start();
    let rig = Rig::new(
        "pack-gone-always",
        &server,
        &[("common", &[Way::Status(403)])],
        |_| {},
    );
    let told = rig.fetch(1);
    assert!(matches!(
        last(&told),
        PackNews::Failed {
            failure: PackFailure::CutOff,
            ..
        }
    ));
    // 404 and 500: the same.
    for code in [404, 500] {
        let server = Server::start();
        let rig = Rig::new(
            "pack-refused",
            &server,
            &[("studioKit", &[Way::Status(code)])],
            |_| {},
        );
        let told = rig.fetch(1);
        assert!(
            matches!(
                last(&told),
                PackNews::Failed {
                    failure: PackFailure::CutOff,
                    ..
                }
            ),
            "{code}: {told:?}"
        );
    }
}

#[test]
fn a_full_disk_is_said_before_and_while_writing() {
    let server = Server::start();
    let rig = Rig::new("pack-full", &server, &[], |o| {
        o.hooks.free = Some(1_000_000)
    });
    let told = rig.fetch(1);
    let total = rig.pack.catalog.bytes();
    // The largest file is unpacked beside itself.
    let piano = rig.pack.catalog.part_of(B::GrandPiano).unwrap().bytes;
    assert_eq!(
        last(&told),
        &PackNews::Failed {
            round: 1,
            failure: PackFailure::DiskFull {
                needed_bytes: total + piano + super::fetch::ROOM_BYTES,
                free_bytes: 1_000_000,
            },
            numbers: super::fetch::Numbers { done: 0, total },
        }
    );
    assert!(server.log().is_empty(), "nothing is fetched");

    let server = Server::start();
    let rig = Rig::new("pack-enospc", &server, &[], |o| {
        o.hooks.free = Some(u64::MAX / 2);
        o.hooks.write_limit = Some(10_000);
    });
    let told = rig.fetch(1);
    assert!(
        matches!(last(&told), PackNews::Failed { failure: PackFailure::DiskFull { free_bytes, .. }, .. } if *free_bytes == u64::MAX / 2),
        "{told:?}"
    );
}

#[test]
fn without_the_net_the_fetch_says_so() {
    let server = Server::start();
    let pack = pack(&server, &PARTS);
    // A port nobody listens on any more.
    let closed = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = closed.local_addr().unwrap().port();
    drop(closed);
    let text = pack
        .text
        .replace(&server.origin, &format!("http://127.0.0.1:{port}/"));
    let away = Server {
        origin: format!("http://127.0.0.1:{port}/"),
        served: Arc::default(),
    };
    let pack = Pack {
        catalog: catalog::check(&text, &trust(&away)).unwrap(),
        text,
        tars: pack.tars,
    };
    let rig = Rig::with("pack-offline", &away, pack, |_| {});
    let told = rig.fetch(1);
    assert!(
        matches!(
            last(&told),
            PackNews::Failed {
                failure: PackFailure::Offline,
                ..
            }
        ),
        "{told:?}"
    );
}

#[test]
fn a_stopped_fetch_keeps_its_part_and_goes_on_later() {
    let server = Server::start();
    let slow = [Way::Slow(500, Duration::from_millis(10))];
    let rig = Rig::new("pack-stop", &server, &[("grandPiano", &slow)], |_| {});
    let common = rig.pack.catalog.parts[0].bytes;
    rig.fetcher.ask(Job::Fetch { round: 1 });
    // Wait until the piano is coming, then stop.
    loop {
        let n = rig.news.recv_timeout(Duration::from_secs(20)).unwrap();
        if matches!(n, PackNews::Progress { instrument: Some(B::GrandPiano), numbers, .. } if numbers.done > common + 2000)
        {
            break;
        }
    }
    rig.fetcher.stop(1);
    std::thread::sleep(Duration::from_millis(300));
    while let Ok(n) = rig.news.try_recv() {
        assert!(
            !matches!(n, PackNews::Done { .. } | PackNews::Failed { .. }),
            "{n:?}"
        );
    }
    let part = rig.root().join("grandPiano-g1.tar.part");
    let kept = std::fs::metadata(&part).unwrap().len();
    assert!(kept > 0);
    // A stopped round does not start again.
    rig.fetcher.ask(Job::Fetch { round: 1 });
    let (file, tar) = rig
        .pack
        .tars
        .iter()
        .find(|(f, _)| f.starts_with("grandPiano"))
        .unwrap();
    server.put(file, tar.clone(), &[Way::Plain]);
    let told = rig.fetch(2);
    assert_eq!(last(&told), &PackNews::Done { round: 2 });
    let range = server
        .requests("grandPiano-g1.tar")
        .last()
        .unwrap()
        .get("range")
        .cloned();
    assert_eq!(range, Some(format!("bytes={kept}-")));
}

#[test]
fn the_catalog_is_looked_at_once_a_day_and_a_newer_one_is_kept() {
    let server = Server::start();
    let now = Some(1_000_000_000);
    // Nothing fetched yet: no look.
    let first = Rig::new("pack-check", &server, &[], |o| o.hooks.now_ms = now);
    first.fetcher.ask(Job::Check);
    let told = first.fetch(1);
    assert_eq!(last(&told), &PackNews::Done { round: 1 });
    assert!(server.requests("catalog.json").is_empty());
    drop(first);

    // Something is here now. A newer pack changes the piano only.
    let rig = Rig::new("pack-check-2", &server, &[], |o| o.hooks.now_ms = now);
    rig.fetch(1);
    let newer = pack(
        &server,
        &[
            ("common", "c1", 3000),
            ("studioKit", "s1", 20_000),
            ("grandPiano", "g2", 41_000),
        ],
    );
    let text = newer.text.replace("2026.12.0", "2026.12.1");
    server.put("catalog.json", text.clone().into_bytes(), &[Way::Plain]);
    let (file, tar) = newer
        .tars
        .iter()
        .find(|(f, _)| f.starts_with("grandPiano"))
        .unwrap();
    server.put(file, tar.clone(), &[Way::Plain]);
    rig.fetcher.ask(Job::Check);
    let n = rig.news.recv_timeout(Duration::from_secs(20)).unwrap();
    let PackNews::NewPack {
        bytes, instruments, ..
    } = n
    else {
        panic!("{n:?}");
    };
    assert_eq!(bytes, tar.len() as u64, "only what changed");
    assert_eq!(instruments, [B::StudioKit, B::GrandPiano]);
    let kept = std::fs::read_to_string(rig.root().join("catalog.json")).unwrap();
    assert_eq!(kept, text);
    assert_eq!(server.requests("catalog.json").len(), 1);
    // The piano of before still sounds until the new one is here.
    let disk = Disk {
        root: rig.root(),
        covered: Vec::new(),
    };
    let newer = catalog::check(&text, &trust(&server)).unwrap();
    let old_piano = rig.root().join("grandPiano/g1/instrument.json");
    assert!(
        disk.instruments(&newer)
            .contains(&(B::GrandPiano, old_piano.clone()))
    );
    // 「取りにいく」 takes only the new piano.
    let told = rig.fetch(2);
    assert_eq!(Rig::installed(&told), [Some(B::GrandPiano)]);
    assert!(rig.here("grandPiano", "g2"));
    assert!(old_piano.is_file(), "kept while this run may still read it");
    // Within 24 hours: no look.
    rig.fetcher.ask(Job::Check);
    assert_eq!(rig.fetch(3), [PackNews::Done { round: 3 }]);
    assert_eq!(server.requests("catalog.json").len(), 1);
    // The next start starts from the kept catalog, and takes the old piano away.
    let start = start_catalog(&rig.root(), &rig.pack.text, &trust(&server)).unwrap();
    assert_eq!(start.pack, "2026.12.1");
    let mut options = Options::new(disk, None);
    options.https_only = false;
    let (tx, news) = channel();
    let next = Fetcher::spawn(options, start, Box::new(move |n| drop(tx.send(n)))).unwrap();
    next.ask(Job::Fetch { round: 1 });
    let n = news.recv_timeout(Duration::from_secs(20)).unwrap();
    assert_eq!(n, PackNews::Done { round: 1 });
    assert!(!old_piano.exists() && rig.here("grandPiano", "g2"));
}

#[test]
fn no_catalog_on_the_net_is_quiet() {
    let server = Server::start();
    let rig = Rig::new("pack-check-404", &server, &[], |o| o.hooks.now_ms = Some(5));
    rig.fetch(1);
    rig.fetcher.ask(Job::Check);
    assert_eq!(rig.fetch(2), [PackNews::Done { round: 2 }]);
    assert_eq!(
        server.requests("catalog.json").len(),
        1,
        "looked once, found nothing"
    );
    let stamp = std::fs::read_to_string(rig.dir.join("state/sounds-checked")).unwrap();
    assert_eq!(stamp, "5");
}

#[test]
fn a_kept_catalog_outside_the_rules_is_not_used() {
    let server = Server::start();
    let dir = crate::settings::temp_dir("pack-kept");
    let built_in = pack(&server, &PARTS).text;
    let other = built_in
        .replace(&server.origin, "https://example.com/")
        .replace("2026.12.0", "2027.1.0");
    std::fs::write(dir.join("catalog.json"), other).unwrap();
    let start = start_catalog(&dir, &built_in, &trust(&server)).unwrap();
    assert_eq!(start.pack, "2026.12.0", "the built-in one");
    // The built-in one of meloet itself names only the releases.
    assert!(start_catalog(&dir, catalog::BUILT_IN, &Trust::default()).is_some());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_place_follows_the_base_directory_rules() {
    let var = |pairs: &'static [(&'static str, &'static str)]| {
        move |name: &str| {
            (pairs.iter())
                .find(|(k, _)| *k == name)
                .map(|(_, v)| std::ffi::OsString::from(v))
        }
    };
    assert_eq!(
        data_root(var(&[("XDG_DATA_HOME", "/d"), ("HOME", "/h")])),
        Some(PathBuf::from("/d/meloet/sounds"))
    );
    assert_eq!(
        data_root(var(&[("XDG_DATA_HOME", "rel"), ("HOME", "/h")])),
        Some(PathBuf::from("/h/.local/share/meloet/sounds")),
        "a relative one is ignored"
    );
    assert_eq!(
        data_root(var(&[("XDG_DATA_HOME", ""), ("HOME", "/h")])),
        Some(PathBuf::from("/h/.local/share/meloet/sounds"))
    );
    assert_eq!(data_root(var(&[])), None);
    assert_eq!(
        shown(
            Path::new("/h/.local/share/meloet/sounds"),
            Some(Path::new("/h"))
        ),
        "~/.local/share/meloet/sounds"
    );
    assert_eq!(
        shown(Path::new("/d/meloet/sounds"), Some(Path::new("/h"))),
        "/d/meloet/sounds"
    );
}

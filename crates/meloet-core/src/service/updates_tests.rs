//! 「新しい版をたしかめる」 through the core's threads, against a server on this PC.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

use meloet_protocol::{Command, StatusMessage};

use super::*;

/// Answers every request with a release tagged `tag`.
fn server(tag: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/latest", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while reader.read_line(&mut line).is_ok_and(|n| n > 0) && !line.trim().is_empty() {
                line.clear();
            }
            let body = format!(r#"{{"tag_name":"{tag}"}}"#);
            let mut out = stream;
            let _ = write!(
                out,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    url
}

#[test]
fn the_page_up_looks_and_the_release_page_goes_to_the_shell() {
    let core = Core::start(CoreOptions {
        updates: Some(crate::updates::Options::new(server("v99.0.0"), None)),
        ..CoreOptions::offline()
    });
    let (shell, asked) = channel();
    core.set_shell(Box::new(move |request| {
        let _ = shell.send(request);
    }));
    assert_eq!(core.snapshot().state.updates.new_version, None);
    core.subscribe(Box::new(|_| true), Box::new(|_| true));
    let deadline = Instant::now() + Duration::from_secs(5);
    while core.snapshot().state.updates.new_version.is_none() {
        assert!(Instant::now() < deadline, "no new version was found");
        std::thread::sleep(Duration::from_millis(10));
    }
    let state = core.snapshot().state;
    assert_eq!(state.updates.new_version.as_deref(), Some("99.0.0"));
    assert!(matches!(
        state.status.message,
        Some(StatusMessage::NewVersion { ref version }) if version == "99.0.0"
    ));
    core.send(Command::OpenReleasePage);
    assert_eq!(
        asked.recv_timeout(Duration::from_secs(2)).unwrap(),
        ShellRequest::OpenUrl {
            url: "https://github.com/wintermaples/meloet/releases/tag/v99.0.0".into()
        }
    );
}

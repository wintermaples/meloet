//! (c) The meloet side: the fake tools served on a Unix socket under $XDG_RUNTIME_DIR.
//!
//!   app [--expect-uid N]
//!
//! `--expect-uid` pretends meloet runs as another uid, to see the reject path.

use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use rmcp::ServiceExt;
use s10_mcp::FakeMeloet;
use tokio::net::UnixListener;
use tokio::signal::unix::{SignalKind, signal};

const DIR: &str = "s10-mcp";

fn socket_path() -> Result<PathBuf, String> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR is not set")?;
    Ok(Path::new(&runtime).join(DIR).join("ai.sock"))
}

fn prepare_dir(dir: &Path, uid: u32) -> Result<(), String> {
    match fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {}
        Err(e) => return Err(format!("mkdir {}: {e}", dir.display())),
    }
    let meta = fs::symlink_metadata(dir).map_err(|e| e.to_string())?;
    if !meta.is_dir() || meta.uid() != uid {
        return Err(format!("{} is not our directory", dir.display()));
    }
    if meta.mode() & 0o077 != 0 {
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Removes a socket left by a meloet that died, but only if nobody answers on it.
fn clear_stale(path: &Path) -> Result<(), String> {
    match std::os::unix::net::UnixStream::connect(path) {
        Ok(_) => Err(format!("{} is in use by another meloet", path.display())),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) if e.kind() == ErrorKind::ConnectionRefused => {
            eprintln!("app: removing stale socket {}", path.display());
            fs::remove_file(path).map_err(|e| e.to_string())
        }
        Err(e) => Err(format!("connect {}: {e}", path.display())),
    }
}

fn bind_private(path: &Path) -> std::io::Result<UnixListener> {
    // umask is per process: in meloet this must run before other threads create files,
    // or the bind must happen in a short window that nothing else races with.
    let old = unsafe { libc::umask(0o177) };
    let listener = UnixListener::bind(path);
    unsafe { libc::umask(old) };
    listener
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let my_uid = unsafe { libc::geteuid() };
    let mut expect_uid = my_uid;
    while let Some(a) = args.next() {
        if a == "--expect-uid" {
            expect_uid = args
                .next()
                .and_then(|v| v.parse().ok())
                .ok_or("--expect-uid N")?;
        }
    }

    let path = socket_path()?;
    prepare_dir(path.parent().unwrap(), my_uid)?;
    clear_stale(&path)?;
    let listener = bind_private(&path).map_err(|e| format!("bind: {e}"))?;
    let mode = fs::metadata(&path).map_err(|e| e.to_string())?.mode() & 0o777;
    let dir_mode = fs::metadata(path.parent().unwrap())
        .map_err(|e| e.to_string())?
        .mode()
        & 0o777;
    eprintln!(
        "app: listening on {} (socket {mode:o}, dir {dir_mode:o})",
        path.display()
    );

    let meloet = FakeMeloet::new();
    let mut term = signal(SignalKind::terminate()).map_err(|e| e.to_string())?;
    let mut int = signal(SignalKind::interrupt()).map_err(|e| e.to_string())?;
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, _) = match accepted {
                    Ok(s) => s,
                    Err(e) => { eprintln!("app: accept: {e}"); continue; }
                };
                let cred = match stream.peer_cred() {
                    Ok(c) => c,
                    Err(e) => { eprintln!("app: peer_cred: {e}"); continue; }
                };
                if cred.uid() != expect_uid {
                    eprintln!("app: refused uid {} pid {:?} (want uid {expect_uid})", cred.uid(), cred.pid());
                    drop(stream);
                    continue;
                }
                eprintln!("app: accepted uid {} pid {:?}", cred.uid(), cred.pid());
                let service = meloet.clone();
                tokio::spawn(async move {
                    match service.serve(stream).await {
                        Ok(running) => {
                            let reason = running.waiting().await;
                            eprintln!("app: connection ended: {reason:?}");
                        }
                        Err(e) => eprintln!("app: handshake failed: {e}"),
                    }
                });
            }
            _ = term.recv() => break,
            _ = int.recv() => break,
        }
    }
    let _ = fs::remove_file(&path);
    eprintln!("app: socket removed, bye");
    Ok(())
}

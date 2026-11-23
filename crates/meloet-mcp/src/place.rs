//! Where meloet's socket may be, and what to tell when it could not be reached.
//!
//! An AI app may start the bridge with only a few variables (Claude Desktop passes on HOME,
//! PATH and the like, not `XDG_RUNTIME_DIR`), or with `XDG_RUNTIME_DIR` moved into a folder
//! of its own under the user's. So the user's folder `/run/user/<uid>` is tried as well.

use std::fmt::Write as _;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

pub struct Place {
    pub path: PathBuf,
    /// How the path was found, for the report.
    pub from: &'static str,
}

fn uid() -> Option<u32> {
    std::fs::metadata("/proc/self").ok().map(|m| m.uid())
}

fn var(name: &str) -> Option<String> {
    let value = std::env::var_os(name).filter(|v| !v.is_empty())?;
    Some(value.to_string_lossy().into_owned())
}

fn socket_in(runtime: &Path) -> PathBuf {
    runtime.join("meloet").join("ai.sock")
}

pub fn places() -> Vec<Place> {
    let named = var("XDG_RUNTIME_DIR").map(PathBuf::from);
    let users = uid().map(|uid| PathBuf::from(format!("/run/user/{uid}")));
    let mut places = Vec::new();
    if let Some(named) = &named {
        places.push(Place {
            path: socket_in(named),
            from: "XDG_RUNTIME_DIR",
        });
    }
    if let Some(users) = users
        && named
            .as_ref()
            .is_none_or(|n| n != &users && n.starts_with(&users))
    {
        places.push(Place {
            path: socket_in(&users),
            from: "/run/user/<uid>",
        });
    }
    places
}

/// What is at a path that could not be connected to.
pub fn seen(path: &Path) -> String {
    let what = |path: &Path| match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_socket() => "a socket".to_owned(),
        Ok(m) if m.is_dir() => "a folder".to_owned(),
        Ok(_) => "there, but not a socket".to_owned(),
        Err(e) => format!("not seen ({:?}: {e})", e.kind()),
    };
    let folder = path.parent().map(what).unwrap_or_default();
    format!("the path is {}; its folder is {folder}", what(path))
}

/// The lines put after the fixed words: at which step it failed, what was tried, and how
/// the bridge was started.
pub fn report(stage: &str, tried: &[String]) -> String {
    let mut text = format!("[meloet-mcp {}] step: {stage}", env!("CARGO_PKG_VERSION"));
    for line in tried {
        let _ = write!(text, "\n- {line}");
    }
    let or_unset = |name: &str| var(name).unwrap_or_else(|| "(not set)".into());
    let uid = uid().map_or_else(|| "(unknown)".into(), |uid| uid.to_string());
    let cwd =
        std::env::current_dir().map_or_else(|e| format!("({e})"), |d| d.display().to_string());
    let _ = write!(
        text,
        "\nstarted with: uid={uid} pid={} cwd={cwd}",
        std::process::id()
    );
    for name in ["XDG_RUNTIME_DIR", "HOME", "USER", "PATH"] {
        let _ = write!(text, "\n  {name}={}", or_unset(name));
    }
    for name in ["SNAP", "FLATPAK_ID", "container", "APPIMAGE"] {
        if let Some(value) = var(name) {
            let _ = write!(text, "\n  {name}={value}");
        }
    }
    text.push_str("\nsettings files read: none (the bridge only looks for the socket)");
    text
}

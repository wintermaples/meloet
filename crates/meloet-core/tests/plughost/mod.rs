//! The real `meloet-plughost` binary for the tests of this crate, and its fake plugins.

#![allow(dead_code)]

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const NAME: &str = "meloet-plughost";

/// The fake plugins exist only for children started with this (never set for the tests'
/// own process).
pub fn fake_env() -> Vec<(OsString, OsString)> {
    vec![("MELOET_PLUGHOST_FAKE".into(), "1".into())]
}

/// The binary of this build. Cargo builds it for a test of this package only when the whole
/// workspace is tested, so it is built here first; that does nothing when it is up to date.
pub fn exe() -> PathBuf {
    static EXE: OnceLock<PathBuf> = OnceLock::new();
    EXE.get_or_init(|| {
        let test = std::env::current_exe().expect("the test's own path");
        // The test is somewhere under `<target>/debug` or `<target>/release`; the binary is
        // right in that folder.
        let profile = test
            .ancestors()
            .find(|dir| {
                dir.file_name()
                    .is_some_and(|n| n == "debug" || n == "release")
            })
            .unwrap_or_else(|| panic!("{} is not in a target folder", test.display()));
        let target = profile.parent().expect("the target folder");
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut build = Command::new(cargo);
        // The fake plugins are a development hook of the binary.
        build.args(["build", "--quiet", "-p", NAME, "--features", "dev-hooks"]);
        build.arg("--target-dir");
        build.arg(target);
        if profile.ends_with("release") {
            build.arg("--release");
        }
        let built = build.status().expect("cargo runs");
        assert!(built.success(), "{NAME} did not build");
        let exe = profile.join(NAME);
        assert!(exe.is_file(), "{} is not there", exe.display());
        exe
    })
    .clone()
}

/// A folder under the system's temporary folder, empty.
pub fn temp_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("meloet-core-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a temporary folder");
    dir
}

/// The pids of the `meloet-plughost` processes whose command line names `marker`.
pub fn hosts_running(marker: &Path) -> Vec<u32> {
    let marker = marker.to_string_lossy();
    let mut found = Vec::new();
    for entry in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(line) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let line = String::from_utf8_lossy(&line);
        let exe = line.split('\0').next().unwrap_or_default();
        if exe.ends_with(NAME) && line.contains(&*marker) {
            found.push(pid);
        }
    }
    found
}

/// Whether the process is there, running or not yet waited for.
pub fn alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Waits until `done` gives something, for at most `within`.
pub fn wait_for<T>(within: Duration, mut done: impl FnMut() -> Option<T>) -> Option<T> {
    let until = Instant::now() + within;
    loop {
        if let Some(value) = done() {
            return Some(value);
        }
        if Instant::now() > until {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

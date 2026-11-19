//! The plugin host process: runs VST3 and LV2 plugins outside meloet, so that a plugin that
//! crashes or hangs takes only this process with it (docs/architecture.md 20-1).
//!
//! The command line is in `meloet_plugin::launch`. Nothing is read from standard input and
//! nothing meant for meloet is written to standard output or standard error.

mod control;
mod formats;
mod rt;
mod rtcheck;
mod scan;

use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::os::unix::net::UnixStream;
use std::path::Path;

use meloet_plugin::{PluginFormat, launch};

fn value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let at = args.iter().position(|a| a == name)?;
    args.get(at + 1).map(String::as_str)
}

/// Every value of an option that may be given more than once.
fn values(args: &[String], name: &str) -> Vec<String> {
    args.windows(2)
        .filter(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
        .collect()
}

/// A descriptor this process inherited for itself, by the number on the command line.
fn inherited(args: &[String], name: &str) -> Option<OwnedFd> {
    let fd: RawFd = value(args, name)?.parse().ok()?;
    // SAFETY: a plain call. Above 2 only: 0 to 2 are the standard ones, redirected already.
    if fd <= 2 || unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0 {
        return None;
    }
    // SAFETY: the parent opened this descriptor for this process and names it only once.
    Some(unsafe { OwnedFd::from_raw_fd(fd) })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    launch::quiet_stdio(value(&args, "--log-fd").and_then(|fd| fd.parse().ok()));
    launch::die_with_parent(value(&args, "--parent").and_then(|pid| pid.parse().ok()));
    if std::env::var_os("MELOET_PLUGHOST_NO_DUMP").is_some_and(|v| !v.is_empty()) {
        // For tests that crash this process on purpose: no core file, no crash report.
        // SAFETY: a plain call.
        unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0) };
    }

    let code = match args.get(1).map(String::as_str) {
        Some("host") => match (
            inherited(&args, "--control-fd"),
            inherited(&args, "--memory-fd"),
        ) {
            (Some(control), Some(memory)) => control::run(UnixStream::from(control), memory),
            _ => 2,
        },
        Some("scan") => {
            let format = args.get(2).and_then(|f| PluginFormat::from_prefix(f));
            match (format, args.get(3), inherited(&args, "--out-fd")) {
                (Some(format), Some(bundle), Some(out)) => {
                    scan::run(format, Path::new(bundle), &values(&args, "--skip"), out)
                }
                _ => 2,
            }
        }
        _ => 2,
    };
    // Not `exit`: that would run what the plugins' libraries registered for the end of the
    // process, which is where a plugin crashes or hangs once more. Everything of meloet's is
    // written by now; the plugins that were unloaded were ended the way their format says.
    // SAFETY: a plain call.
    unsafe { libc::_exit(code) }
}

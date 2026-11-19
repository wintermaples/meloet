//! Starting `meloet-plughost`, and what that process does first.
//!
//! The process gets its file descriptors by inheriting them at the numbers its command line
//! names:
//!
//! ```text
//! meloet-plughost host --parent <pid> --control-fd <n> --memory-fd <n> [--log-fd <n>]
//! meloet-plughost scan <vst3|lv2> <bundle> --parent <pid> --out-fd <n> [--log-fd <n>]
//! ```
//!
//! `host` answers [`crate::wire`] requests on the control socket and runs plugins in the
//! slots of the shared memory until the socket closes. `scan` writes one frame holding a
//! [`crate::wire::ScanOutput`] to `--out-fd` and ends.

use std::os::fd::{AsRawFd, BorrowedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::key::PluginFormat;

/// Lets the child inherit these descriptors at their numbers. They must stay open in this
/// process until the child is spawned.
fn inherit(command: &mut Command, fds: Vec<RawFd>) {
    // SAFETY: the closure runs between fork and exec and only calls fcntl, which is
    // async-signal-safe. It allocates nothing: `fds` was built before the fork.
    unsafe {
        command.pre_exec(move || {
            for &fd in &fds {
                let flags = libc::fcntl(fd, libc::F_GETFD);
                if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
            }
            Ok(())
        });
    }
}

fn base(exe: &Path, mode: &str) -> Command {
    let mut command = Command::new(exe);
    command
        .arg(mode)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn finish(mut command: Command, mut fds: Vec<RawFd>, log: Option<BorrowedFd<'_>>) -> Command {
    command.arg("--parent").arg(std::process::id().to_string());
    if let Some(log) = log {
        command.arg("--log-fd").arg(log.as_raw_fd().to_string());
        fds.push(log.as_raw_fd());
    }
    inherit(&mut command, fds);
    command
}

/// The command for a host process. Spawn it from a thread that lives as long as the child
/// may: the child asks to be killed when the thread that spawned it ends
/// ([`die_with_parent`]).
///
/// `log`: where the process's standard output and standard error go (what plugins print);
/// `None` is `/dev/null`.
pub fn host_command(
    exe: &Path,
    control: BorrowedFd<'_>,
    memory: BorrowedFd<'_>,
    log: Option<BorrowedFd<'_>>,
) -> Command {
    let mut command = base(exe, "host");
    command
        .arg("--control-fd")
        .arg(control.as_raw_fd().to_string())
        .arg("--memory-fd")
        .arg(memory.as_raw_fd().to_string());
    finish(command, vec![control.as_raw_fd(), memory.as_raw_fd()], log)
}

/// The command that scans one bundle and writes what it found to `out`.
pub fn scan_command(
    exe: &Path,
    format: PluginFormat,
    bundle: &Path,
    out: BorrowedFd<'_>,
    log: Option<BorrowedFd<'_>>,
) -> Command {
    let mut command = base(exe, "scan");
    command
        .arg(format.prefix())
        .arg(bundle)
        .arg("--out-fd")
        .arg(out.as_raw_fd().to_string());
    finish(command, vec![out.as_raw_fd()], log)
}

/// In the child, first of all: standard input from `/dev/null`, standard output and standard
/// error to `log` or `/dev/null`. Whatever a plugin prints ends up there and nowhere else.
pub fn quiet_stdio(log: Option<RawFd>) {
    // SAFETY: plain descriptor calls; a failure leaves the descriptor as it was.
    unsafe {
        let null = libc::open(c"/dev/null".as_ptr(), libc::O_RDWR | libc::O_CLOEXEC);
        if null >= 0 {
            libc::dup2(null, 0);
        }
        let out = log
            .filter(|&fd| libc::fcntl(fd, libc::F_GETFD) >= 0)
            .unwrap_or(null);
        if out >= 0 {
            libc::dup2(out, 1);
            libc::dup2(out, 2);
        }
        if null > 2 {
            libc::close(null);
        }
    }
}

/// In the child: be killed when the thread that started this process ends, so no host process
/// is left behind by a meloet that crashed. Ends the process at once if the parent is already
/// gone (it went between the fork and this call).
pub fn die_with_parent(parent: Option<u32>) {
    // SAFETY: plain calls.
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
        if parent.is_some_and(|pid| libc::getppid() as u32 != pid) {
            libc::_exit(0);
        }
    }
}

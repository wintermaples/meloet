//! Starting `meloet-plughost`, and what that process does first.
//!
//! ```text
//! meloet-plughost host [--offline] --parent <pid> [--log]
//! meloet-plughost scan <vst3|lv2> <bundle> [--skip <id>]... --parent <pid> [--log]
//! ```
//!
//! The process gets its descriptors as its standard ones: `host` the control socket as
//! standard input and the shared memory as standard output, `scan` the socket it writes to as
//! standard output, and both, with `--log`, the log as standard error. The first thing the
//! process does is to move them away and to put `/dev/null` (or the log) in their place
//! ([`take_stdio`]), because plugins print to the standard ones.
//!
//! Nothing here runs between fork and exec (no `pre_exec`), so that `Command::spawn` uses
//! `posix_spawn`, which shares meloet's memory with the child until the exec. A fork would
//! copy the page tables of all of meloet's memory and make every page of it copy-on-write,
//! also the ones the audio thread writes to. What had to happen in between is done by the
//! child itself: [`die_with_parent`], [`low_priority`].
//!
//! A `scan` and a `host --offline` run at a low priority ([`low_priority`]).
//!
//! `host` answers [`crate::wire`] requests on the control socket and runs plugins in the
//! slots of the shared memory until the socket closes. `scan` writes frames of
//! [`crate::wire::ScanOutput`], one plugin after another and passing over the ids `--skip`
//! names, and ends after `Done` or `Broken`.

use std::ffi::OsString;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use crate::key::PluginFormat;

/// The argument of `host` that makes the process one for an export.
pub const OFFLINE: &str = "--offline";

fn base(exe: &Path, mode: &str) -> Command {
    let mut command = Command::new(exe);
    command
        .arg(mode)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

/// A copy of `fd` for the child's standard input, output or error. The command owns it:
/// drop the command once the child is spawned, or a socket handed over never reads as closed.
fn hand(fd: BorrowedFd<'_>) -> std::io::Result<Stdio> {
    Ok(Stdio::from(fd.try_clone_to_owned()?))
}

fn finish(mut command: Command, log: Option<BorrowedFd<'_>>) -> std::io::Result<Command> {
    command.arg("--parent").arg(std::process::id().to_string());
    if let Some(log) = log {
        command.arg("--log").stderr(hand(log)?);
    }
    Ok(command)
}

/// The command for a host process. Spawn it from a thread that lives as long as the child
/// may: the child asks to be killed when the thread that spawned it ends
/// ([`die_with_parent`]).
///
/// `log`: where the process's standard output and standard error go (what plugins print);
/// `None` is `/dev/null`.
///
/// `offline`: the process is for an export (docs/architecture.md 22-11). It is said here and
/// not over the socket, because the process's audio thread starts, and asks for its
/// priority, before the first message.
pub fn host_command(
    exe: &Path,
    control: BorrowedFd<'_>,
    memory: BorrowedFd<'_>,
    log: Option<BorrowedFd<'_>>,
    offline: bool,
) -> std::io::Result<Command> {
    let mut command = base(exe, "host");
    if offline {
        command.arg(OFFLINE);
    }
    command.stdin(hand(control)?).stdout(hand(memory)?);
    finish(command, log)
}

/// The command that scans one bundle and writes what it found to `out`. The plugins whose ids
/// are in `skip` are not read. The child runs at a low priority ([`low_priority`]).
pub fn scan_command(
    exe: &Path,
    format: PluginFormat,
    bundle: &Path,
    skip: &[String],
    out: BorrowedFd<'_>,
    log: Option<BorrowedFd<'_>>,
) -> std::io::Result<Command> {
    let mut command = base(exe, "scan");
    command.arg(format.prefix()).arg(bundle);
    for id in skip {
        command.arg("--skip").arg(id);
    }
    command.stdout(hand(out)?);
    finish(command, log)
}

/// Starts `command`, one child at a time in all of meloet. Under LLDB (CodeLLDB 1.12.3 in
/// VS Code) meloet stayed stopped for good when the scan's threads started children at the
/// same moment; gdb and no debugger were fine. A start takes about a millisecond.
///
/// When meloet runs from an AppImage, the child gets the PC's environment back
/// ([`outside_appimage`]).
pub fn spawn(command: &mut Command) -> std::io::Result<Child> {
    static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());
    outside_appimage(command);
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    command.spawn()
}

/// For a child that runs on the PC's own libraries (a plugin host loads the PC's VST3 and
/// LV2 plugins; `xdg-open` starts the PC's apps): takes away what the AppImage's runtime and
/// `AppRun` put in meloet's environment. Nothing changes outside an AppImage.
pub fn outside_appimage(command: &mut Command) {
    apply(command, appimage_changes(std::env::vars_os()));
}

fn apply(command: &mut Command, changes: Vec<(OsString, Option<OsString>)>) {
    for (name, value) in changes {
        match value {
            Some(value) => command.env(name, value),
            None => command.env_remove(name),
        };
    }
}

/// Set by the AppImage runtime, by `AppRun` and by the GTK hook of linuxdeploy with values
/// that do not name a place inside the AppImage (`GTK_THEME` is always the hook's, Adwaita).
const APPIMAGE_ONLY: &[&str] = &[
    "APPIMAGE",
    "APPDIR",
    "OWD",
    "ARGV0",
    "PYTHONDONTWRITEBYTECODE",
    "GTK_THEME",
];

/// What to change in an environment for a child that must not see the AppImage: `None`
/// removes the variable. Empty unless `APPIMAGE` is set. In every variable, the entries of
/// a `:` list that lie inside `APPDIR` go (with the empty entries next to them, which would
/// mean the current folder); a variable left with nothing goes.
fn appimage_changes(
    vars: impl Iterator<Item = (OsString, OsString)>,
) -> Vec<(OsString, Option<OsString>)> {
    let vars: Vec<(OsString, OsString)> = vars.collect();
    if !vars.iter().any(|(name, _)| name == "APPIMAGE") {
        return Vec::new();
    }
    let appdir = vars
        .iter()
        .find(|(name, _)| name == "APPDIR")
        .map(|(_, value)| value.as_bytes())
        .map(|dir| dir.strip_suffix(b"/").unwrap_or(dir))
        .filter(|dir| !dir.is_empty());
    let inside = |entry: &[u8]| {
        appdir.is_some_and(|dir| {
            entry
                .strip_prefix(dir)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with(b"/"))
        })
    };
    let mut changes = Vec::new();
    for (name, value) in &vars {
        if APPIMAGE_ONLY.iter().any(|only| name == only) {
            changes.push((name.clone(), None));
            continue;
        }
        let entries: Vec<&[u8]> = value.as_bytes().split(|&b| b == b':').collect();
        if !entries.iter().any(|entry| inside(entry)) {
            continue;
        }
        let kept: Vec<&[u8]> = entries
            .into_iter()
            .filter(|entry| !entry.is_empty() && !inside(entry))
            .collect();
        let value = (!kept.is_empty()).then(|| OsString::from_vec(kept.join(&b':')));
        changes.push((name.clone(), value));
    }
    changes
}

/// In a scanning child, in a host process for an export, and on meloet's thread of an
/// export: be the least urgent of the ordinary ones (nice 19). A scan or an export must not
/// take the processor from the screen, or from a playing host process that got no real-time
/// priority, while a person plays. Not `SCHED_IDLE`: a scan's time limit and an export's
/// watch for a plugin that hangs are wall-clock time, and on a busy PC an idle-class thread
/// may get no time at all and be taken for a plugin that hangs. meloet's audio thread is
/// real-time and is ahead of any ordinary process either way.
///
/// On Linux the nice value is a thread's (setpriority(2), BUGS): this is for the calling
/// thread, and for the threads and processes it starts from now on. A process calls it before
/// it starts a thread.
pub fn low_priority() {
    // SAFETY: a plain system call.
    unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, 19) };
}

/// Waits until `fd` has something to read, or its other end closed. False when `timeout`
/// passed first.
pub fn wait_readable(fd: BorrowedFd<'_>, timeout: Duration) -> bool {
    let mut poll = libc::pollfd {
        fd: fd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    let ms = timeout.as_millis().min(i32::MAX as u128) as libc::c_int;
    // SAFETY: one valid pollfd.
    unsafe { libc::poll(&mut poll, 1, ms) > 0 }
}

/// A handle on a child process that stays the same process whatever happens to its pid
/// (docs/history/research/plugin-ipc.md). Open it before the child is waited for.
pub struct PidFd(OwnedFd);

impl PidFd {
    pub fn open(pid: u32) -> std::io::Result<PidFd> {
        // SAFETY: a plain system call; the descriptor it returns is ours alone. It is
        // close-on-exec by itself.
        let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid as libc::pid_t, 0) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: just opened, owned by nothing else.
        Ok(PidFd(unsafe { OwnedFd::from_raw_fd(fd as RawFd) }))
    }

    /// Whether the process has ended, waiting up to `timeout` for it.
    pub fn exited(&self, timeout: Duration) -> bool {
        wait_readable(self.0.as_fd(), timeout)
    }

    /// SIGKILL. An error when the process is gone already.
    pub fn kill(&self) -> std::io::Result<()> {
        // SAFETY: a plain system call on a descriptor this owns.
        let done = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.0.as_raw_fd(),
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0,
            )
        };
        if done < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }
}

/// What the parent handed over as standard input and standard output.
pub struct Handed {
    pub input: Option<OwnedFd>,
    pub output: Option<OwnedFd>,
}

/// In the child, first of all: takes the descriptors the parent handed over as the standard
/// ones, and puts `/dev/null` in their place: standard input, and standard output and
/// standard error unless `log`, which sends both to what came as standard error. Whatever a
/// plugin prints ends up there and nowhere else.
pub fn take_stdio(log: bool) -> Handed {
    // SAFETY: plain descriptor calls; a failure leaves the descriptor as it was. The copies
    // are above 2, close-on-exec and owned by nothing else.
    unsafe {
        let take = |fd: RawFd| {
            let copy = libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3);
            (copy >= 0).then(|| OwnedFd::from_raw_fd(copy))
        };
        let handed = Handed {
            input: take(0),
            output: take(1),
        };
        let null = libc::open(c"/dev/null".as_ptr(), libc::O_RDWR | libc::O_CLOEXEC);
        if null >= 0 {
            libc::dup2(null, 0);
        }
        let out = if log { 2 } else { null };
        if out >= 0 {
            libc::dup2(out, 1);
            libc::dup2(out, 2);
        }
        if null > 2 {
            libc::close(null);
        }
        handed
    }
}

/// Whether `fd` is a socket.
pub fn is_socket(fd: BorrowedFd<'_>) -> bool {
    // SAFETY: fstat fills the zeroed struct or fails.
    unsafe {
        let mut stat: libc::stat = std::mem::zeroed();
        libc::fstat(fd.as_raw_fd(), &mut stat) == 0 && stat.st_mode & libc::S_IFMT == libc::S_IFSOCK
    }
}

/// In the child: be killed when the thread that started this process ends, so no host process
/// is left behind by a meloet that crashed. Ends the process at once if the parent is already
/// gone (it went before this call).
pub fn die_with_parent(parent: Option<u32>) {
    // SAFETY: plain calls.
    unsafe {
        libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
        if parent.is_some_and(|pid| libc::getppid() as u32 != pid) {
            libc::_exit(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    fn env(pairs: &[(&str, &str)]) -> impl Iterator<Item = (OsString, OsString)> {
        pairs
            .iter()
            .map(|(n, v)| (OsString::from(n), OsString::from(v)))
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn changes(pairs: &[(&str, &str)]) -> Vec<(String, Option<String>)> {
        super::appimage_changes(env(pairs))
            .into_iter()
            .map(|(n, v)| {
                (
                    n.into_string().unwrap(),
                    v.map(|v| v.into_string().unwrap()),
                )
            })
            .collect()
    }

    #[test]
    fn outside_an_appimage_nothing_changes() {
        let pairs = [
            ("APPDIR", "/tmp/.mount_x"),
            ("LD_LIBRARY_PATH", "/tmp/.mount_x/usr/lib"),
            ("PATH", "/usr/bin"),
        ];
        assert!(changes(&pairs).is_empty());
    }

    #[test]
    fn what_the_appimage_added_goes_and_the_rest_stays() {
        let d = "/tmp/.mount_meloetAbc";
        let pairs = [
            ("APPIMAGE", "/home/a/meloet_0.3.0_amd64.AppImage"),
            ("APPDIR", d),
            ("OWD", "/home/a"),
            ("ARGV0", "./meloet.AppImage"),
            ("PYTHONDONTWRITEBYTECODE", "1"),
            ("GTK_THEME", "Adwaita:dark"),
            ("HOME", "/home/a"),
            (
                "PATH",
                "/tmp/.mount_meloetAbc/usr/bin/:/tmp/.mount_meloetAbc/usr/sbin/:/usr/local/bin:/usr/bin",
            ),
            (
                "LD_LIBRARY_PATH",
                "/tmp/.mount_meloetAbc/usr/lib/:/tmp/.mount_meloetAbc/lib64/:",
            ),
            (
                "XDG_DATA_DIRS",
                "/tmp/.mount_meloetAbc/usr/share/:/usr/local/share:/usr/share",
            ),
            ("GTK_DATA_PREFIX", d),
            (
                "GSETTINGS_SCHEMA_DIR",
                "/tmp/.mount_meloetAbc/usr/share/glib-2.0/schemas/:",
            ),
            ("NEAR", "/tmp/.mount_meloetAbcdef/usr/lib"),
            ("LV2_PATH", "/home/a/.lv2::/usr/lib/lv2"),
        ];
        let mut got = changes(&pairs);
        got.sort();
        let mut want: Vec<(String, Option<String>)> = vec![
            ("APPDIR".into(), None),
            ("APPIMAGE".into(), None),
            ("ARGV0".into(), None),
            ("GSETTINGS_SCHEMA_DIR".into(), None),
            ("GTK_DATA_PREFIX".into(), None),
            ("GTK_THEME".into(), None),
            ("LD_LIBRARY_PATH".into(), None),
            ("OWD".into(), None),
            ("PATH".into(), Some("/usr/local/bin:/usr/bin".into())),
            ("PYTHONDONTWRITEBYTECODE".into(), None),
            (
                "XDG_DATA_DIRS".into(),
                Some("/usr/local/share:/usr/share".into()),
            ),
        ];
        want.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn a_command_loses_the_appimage_variables() {
        let mut command = std::process::Command::new("true");
        let changes = super::appimage_changes(env(&[
            ("APPIMAGE", "/a.AppImage"),
            ("APPDIR", "/tmp/m/"),
            ("LD_LIBRARY_PATH", "/tmp/m/usr/lib:/opt/x"),
        ]));
        super::apply(&mut command, changes);
        let mut set: Vec<_> = command
            .get_envs()
            .map(|(n, v)| (n.to_os_string(), v.map(|v| v.to_os_string())))
            .collect();
        set.sort();
        assert_eq!(
            set,
            vec![
                (OsString::from("APPDIR"), None),
                (OsString::from("APPIMAGE"), None),
                (
                    OsString::from("LD_LIBRARY_PATH"),
                    Some(OsString::from("/opt/x"))
                ),
            ]
        );
    }

    #[test]
    fn a_low_priority_is_the_calling_threads_and_of_what_it_starts() {
        // SAFETY: a plain system call.
        let nice = || unsafe { libc::getpriority(libc::PRIO_PROCESS, 0) };
        let before = nice();
        let lowered = std::thread::spawn(move || {
            super::low_priority();
            (nice(), std::thread::spawn(nice).join().unwrap())
        });
        assert_eq!(lowered.join().unwrap(), (19, 19));
        assert_eq!(nice(), before);
    }
}

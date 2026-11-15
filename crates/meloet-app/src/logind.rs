//! Before the PC powers off or restarts, logind says `PrepareForShutdown(true)` to whoever holds
//! a "delay" lock, and waits (up to its `InhibitDelayMaxSec`) until the lock is let go. meloet
//! holds one, so that the recovery file is written before the PC starts to end the programs.
//! Logging out says nothing here: the signals do it (`webprocess::exit_on_signals`).

use std::sync::Weak;

use crate::core_handle::CoreHandle;

/// What the lock says to `systemd-inhibit --list`.
#[cfg(target_os = "linux")]
const WHY: &str = "Writes unsaved changes to the recovery file";

/// Takes the lock on a thread of its own. Without logind (or the system bus) meloet goes on
/// without it.
#[cfg(target_os = "linux")]
pub fn watch(core: Weak<dyn CoreHandle>) {
    let started = std::thread::Builder::new()
        .name("meloet-logind".into())
        .spawn(move || {
            if let Err(e) = imp::run(&core) {
                eprintln!("meloet: no shutdown lock from logind: {e}");
            }
        });
    if let Err(e) = started {
        eprintln!("meloet: cannot start the logind thread: {e}");
    }
}

#[cfg(not(target_os = "linux"))]
pub fn watch(_: Weak<dyn CoreHandle>) {}

#[cfg(target_os = "linux")]
mod imp {
    use std::sync::{Arc, Mutex, Weak};
    use std::time::Duration;

    use dbus::arg::OwnedFd;
    use dbus::blocking::Connection;
    use dbus::message::MatchRule;

    use super::WHY;
    use crate::core_handle::CoreHandle;

    const DEST: &str = "org.freedesktop.login1";
    const PATH: &str = "/org/freedesktop/login1";
    const MANAGER: &str = "org.freedesktop.login1.Manager";
    const CALL_TIMEOUT: Duration = Duration::from_secs(5);

    fn take(conn: &Connection) -> Result<OwnedFd, dbus::Error> {
        let proxy = conn.with_proxy(DEST, PATH, CALL_TIMEOUT);
        let (fd,): (OwnedFd,) =
            proxy.method_call(MANAGER, "Inhibit", ("shutdown", "meloet", WHY, "delay"))?;
        Ok(fd)
    }

    pub fn run(core: &Weak<dyn CoreHandle>) -> Result<(), dbus::Error> {
        let conn = Connection::new_system()?;
        // Only this thread touches it; the callback must be `Send` all the same.
        let lock = Arc::new(Mutex::new(Some(take(&conn)?)));
        let rule = MatchRule::new_signal(MANAGER, "PrepareForShutdown")
            .with_sender(DEST)
            .with_path(PATH);
        let (on_signal, ending_core) = (lock.clone(), core.clone());
        conn.add_match(rule, move |(ending,): (bool,), conn, _| {
            if ending {
                if let Some(core) = ending_core.upgrade() {
                    core.pc_ending(crate::ENDING_WAIT);
                }
                // Let go: the PC goes on ending.
                drop(on_signal.lock().unwrap_or_else(|e| e.into_inner()).take());
            } else {
                // The shutdown was called off.
                match take(conn) {
                    Ok(fd) => *on_signal.lock().unwrap_or_else(|e| e.into_inner()) = Some(fd),
                    Err(e) => eprintln!("meloet: cannot take the shutdown lock again: {e}"),
                }
            }
            true
        })?;
        while core.strong_count() > 0 {
            conn.process(Duration::from_secs(1))?;
        }
        Ok(())
    }
}

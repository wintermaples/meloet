//! A real-time priority for the thread cpal runs a stream's callbacks on.
//!
//! cpal (built without its `realtime` feature) runs them on a plain thread of its own
//! (`pw_out`, `pw_in`). The first callback leaves its thread id where a helper thread finds it,
//! and the helper asks for the priority the way the plugin host's audio threads do: with
//! `sched_setscheduler` where `RLIMIT_RTPRIO` allows it, through rtkit otherwise. The callback
//! makes no call that may wait. Without a priority the stream runs all the same, and the log
//! says so once. `MELOET_ENGINE_RT` chooses for tests and measurements: `off` asks for
//! nothing, `rtkit` asks rtkit even where the limits allow.

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

/// The same as the plugin host's audio threads (the most rtkit gives), so meloet's thread
/// never stands below the threads it waits for. PipeWire's own threads stay above (88 by
/// limit, 20 through rtkit for its clients).
pub(crate) const PRIORITY: u32 = 20;
const RT_ENV: &str = "MELOET_ENGINE_RT";
/// How often the helper looks for the first callback.
const LOOK: Duration = Duration::from_millis(10);

/// Lives in a stream's callback. [`ThreadNote::note`] once per callback.
pub(crate) struct ThreadNote {
    thread: Arc<AtomicI32>,
    noted: bool,
}

impl ThreadNote {
    #[inline]
    pub(crate) fn note(&mut self) {
        if !self.noted {
            self.noted = true;
            // SAFETY: a plain call.
            let id = unsafe { libc::gettid() };
            self.thread.store(id, Ordering::Release);
        }
    }
}

/// Starts the helper for one stream (`what` names it in the log). The helper ends when it
/// has asked, or when the stream is gone before its first callback.
pub(crate) fn ask_for(what: &'static str) -> ThreadNote {
    let thread = Arc::new(AtomicI32::new(0));
    let seen = thread.clone();
    let started = std::thread::Builder::new()
        .name("meloet-rt-ask".into())
        .spawn(move || {
            loop {
                let id = seen.load(Ordering::Acquire);
                // The count says the callback still lives, so its thread does too.
                if Arc::strong_count(&seen) == 1 {
                    return;
                }
                if id != 0 {
                    report(what, raise(id));
                    return;
                }
                // The helper's own thread, never the stream's.
                #[allow(clippy::disallowed_methods)]
                std::thread::sleep(LOOK);
            }
        });
    if let Err(e) = started {
        eprintln!("meloet: no real-time priority for the {what} thread: no helper: {e}");
    }
    ThreadNote {
        thread,
        noted: false,
    }
}

fn report(what: &str, got: Result<(u32, &'static str), String>) {
    match got {
        Ok((priority, how)) => {
            eprintln!("meloet: the {what} thread runs at real-time priority {priority} ({how})")
        }
        Err(why) => eprintln!("meloet: no real-time priority for the {what} thread: {why}"),
    }
}

/// Asks for [`PRIORITY`] for thread `id` of this process, as `MELOET_ENGINE_RT` says.
pub(crate) fn raise(id: libc::pid_t) -> Result<(u32, &'static str), String> {
    let choice = std::env::var(RT_ENV).unwrap_or_default();
    match choice.as_str() {
        "off" => Err("switched off".into()),
        "rtkit" => by_rtkit(id).map(|p| (p, "rtkit")),
        _ => by_limit(id).map(|p| (p, "limit")).or_else(|limit| {
            by_rtkit(id)
                .map(|p| (p, "rtkit"))
                .map_err(|rtkit| format!("{limit}; {rtkit}"))
        }),
    }
}

fn by_limit(id: libc::pid_t) -> Result<u32, String> {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: plain calls; `limit` is written by the first before it is read.
    unsafe {
        if libc::getrlimit(libc::RLIMIT_RTPRIO, &mut limit) != 0 || limit.rlim_cur < 1 {
            return Err("RLIMIT_RTPRIO is 0".into());
        }
        let priority = u64::from(PRIORITY).min(limit.rlim_cur) as u32;
        let param = libc::sched_param {
            sched_priority: priority as i32,
        };
        let policy = libc::SCHED_FIFO | libc::SCHED_RESET_ON_FORK;
        if libc::sched_setscheduler(id, policy, &param) != 0 {
            return Err(format!(
                "sched_setscheduler: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(priority)
    }
}

fn by_rtkit(id: libc::pid_t) -> Result<u32, String> {
    use dbus::blocking::Connection;
    use dbus::blocking::stdintf::org_freedesktop_dbus::Properties;

    const RTKIT: &str = "org.freedesktop.RealtimeKit1";
    let error = |e: dbus::Error| format!("rtkit: {e}");
    let bus = Connection::new_system().map_err(error)?;
    let rtkit = bus.with_proxy(
        RTKIT,
        "/org/freedesktop/RealtimeKit1",
        Duration::from_secs(2),
    );
    let most: i32 = rtkit.get(RTKIT, "MaxRealtimePriority").map_err(error)?;
    let run_us: i64 = rtkit.get(RTKIT, "RTTimeUSecMax").map_err(error)?;
    let priority = PRIORITY.min(most.max(1) as u32);
    // rtkit refuses a process whose real-time threads may run without end: it wants a limit
    // on how long one runs without sleeping, no longer than its own.
    // SAFETY: plain calls; `limit` is written by the first before it is read.
    unsafe {
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        libc::getrlimit(libc::RLIMIT_RTTIME, &mut limit);
        let most = run_us.max(0) as libc::rlim_t;
        limit.rlim_cur = limit.rlim_cur.min(most);
        limit.rlim_max = limit.rlim_max.min(most);
        if libc::setrlimit(libc::RLIMIT_RTTIME, &limit) != 0 {
            return Err(format!(
                "RLIMIT_RTTIME: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    rtkit
        .method_call::<(), _, _, _>(RTKIT, "MakeThreadRealtime", (id as u64, priority))
        .map_err(error)?;
    Ok(priority)
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    fn policy(id: libc::pid_t) -> (i32, i32) {
        let mut param = libc::sched_param { sched_priority: 0 };
        // SAFETY: plain calls; `param` is written by the second.
        unsafe {
            let policy = libc::sched_getscheduler(id);
            libc::sched_getparam(id, &mut param);
            (policy & !libc::SCHED_RESET_ON_FORK, param.sched_priority)
        }
    }

    #[test]
    fn the_helper_raises_the_thread_of_the_first_callback() {
        let mut note = ask_for("test");
        let callback = std::thread::spawn(move || {
            note.note();
            // SAFETY: a plain call.
            let me = unsafe { libc::gettid() };
            // The helper sees the callback alive until this thread ends.
            for _ in 0..200 {
                let (policy, priority) = policy(me);
                if policy == libc::SCHED_FIFO {
                    return Some(priority);
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            None
        });
        match callback.join().unwrap() {
            Some(priority) => assert!((1..=PRIORITY as i32).contains(&priority)),
            // A PC that gives no real-time priority runs the thread as it was.
            None => eprintln!("this PC gave no real-time priority; the thread ran as it was"),
        }
    }

    #[test]
    fn the_helper_ends_when_the_stream_goes_before_its_first_callback() {
        let note = ask_for("test");
        let slot = Arc::downgrade(&note.thread);
        drop(note);
        for _ in 0..100 {
            if slot.strong_count() == 0 {
                return;
            }
            std::thread::sleep(LOOK);
        }
        panic!("the helper still holds the slot");
    }
}

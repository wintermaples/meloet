//! The run loop VST3 has a Linux host give its plugins: file descriptors to watch and timers
//! to call, all on the main thread. A plugin's window lives on these (its own X11 connection
//! is one of the descriptors).
//!
//! One loop for the process, whatever object a plugin asked for it (the host context, the
//! frame of a view). What a plugin registers from another thread than the main one is never
//! called.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use vst3::ComPtr;
use vst3::ComRef;
use vst3::Steinberg::Linux::{
    FileDescriptor, IEventHandler, IEventHandlerTrait, ITimerHandler, ITimerHandlerTrait,
    TimerInterval,
};
use vst3::Steinberg::{kInvalidArgument, kResultOk, tresult};

struct Timer {
    every: Duration,
    due: Instant,
    handler: ComPtr<ITimerHandler>,
}

#[derive(Default)]
struct Loop {
    watched: Vec<(FileDescriptor, ComPtr<IEventHandler>)>,
    timers: Vec<Timer>,
}

thread_local! {
    static LOOP: RefCell<Loop> = RefCell::new(Loop::default());
}

/// # Safety
/// `handler` is null or a valid `IEventHandler`.
pub unsafe fn watch(handler: *mut IEventHandler, fd: FileDescriptor) -> tresult {
    // SAFETY: as the caller says.
    let Some(handler) = (unsafe { ComRef::from_raw(handler) }) else {
        return kInvalidArgument;
    };
    LOOP.with_borrow_mut(|l| l.watched.push((fd, handler.to_com_ptr())));
    kResultOk
}

pub fn unwatch(handler: *mut IEventHandler) -> tresult {
    let gone = LOOP.with_borrow_mut(|l| {
        let mut gone = Vec::new();
        let mut at = 0;
        while at < l.watched.len() {
            if l.watched[at].1.as_ptr() == handler {
                gone.push(l.watched.remove(at));
            } else {
                at += 1;
            }
        }
        gone
    });
    // Let go of outside the borrow: the plugin may come back in here from its release.
    let found = !gone.is_empty();
    drop(gone);
    if found { kResultOk } else { kInvalidArgument }
}

/// # Safety
/// `handler` is null or a valid `ITimerHandler`.
pub unsafe fn start_timer(handler: *mut ITimerHandler, milliseconds: TimerInterval) -> tresult {
    // SAFETY: as the caller says.
    let Some(handler) = (unsafe { ComRef::from_raw(handler) }) else {
        return kInvalidArgument;
    };
    let every = Duration::from_millis(milliseconds.max(1));
    LOOP.with_borrow_mut(|l| {
        l.timers.push(Timer {
            every,
            due: Instant::now() + every,
            handler: handler.to_com_ptr(),
        });
    });
    kResultOk
}

pub fn stop_timer(handler: *mut ITimerHandler) -> tresult {
    let gone = LOOP.with_borrow_mut(|l| {
        let mut gone = Vec::new();
        let mut at = 0;
        while at < l.timers.len() {
            if l.timers[at].handler.as_ptr() == handler {
                gone.push(l.timers.remove(at));
            } else {
                at += 1;
            }
        }
        gone
    });
    let found = !gone.is_empty();
    drop(gone);
    if found { kResultOk } else { kInvalidArgument }
}

/// Adds the descriptors the plugins want watched, for `poll`.
pub fn watched(polled: &mut Vec<libc::pollfd>) {
    LOOP.with_borrow(|l| {
        polled.extend(l.watched.iter().map(|(fd, _)| libc::pollfd {
            fd: *fd,
            events: libc::POLLIN,
            revents: 0,
        }));
    });
}

/// How long until the next timer is due; `None` with no timer.
pub fn next_timer() -> Option<Duration> {
    let now = Instant::now();
    LOOP.with_borrow(|l| {
        (l.timers.iter())
            .map(|timer| timer.due.saturating_duration_since(now))
            .min()
    })
}

/// Calls the handlers of the descriptors that `poll` found readable (`polled` as
/// [`watched`] made it) and the timers that are due.
pub fn turn(polled: &[libc::pollfd]) {
    if LOOP.with_borrow(|l| l.watched.is_empty() && l.timers.is_empty()) {
        return;
    }
    // The handlers are taken out before any is called: a plugin registers and unregisters
    // from inside its handlers.
    let ready: Vec<(FileDescriptor, ComPtr<IEventHandler>)> = LOOP.with_borrow(|l| {
        (l.watched.iter())
            .filter(|(fd, _)| polled.iter().any(|p| p.fd == *fd && p.revents != 0))
            .cloned()
            .collect()
    });
    for (fd, handler) in ready {
        let still = LOOP.with_borrow(|l| {
            (l.watched.iter()).any(|(at, h)| *at == fd && h.as_ptr() == handler.as_ptr())
        });
        if still {
            // SAFETY: a handler the plugin registered and has not taken back.
            unsafe { handler.onFDIsSet(fd) };
        }
    }
    let now = Instant::now();
    let due: Vec<ComPtr<ITimerHandler>> = LOOP.with_borrow_mut(|l| {
        (l.timers.iter_mut())
            .filter(|timer| timer.due <= now)
            .map(|timer| {
                timer.due = now + timer.every;
                timer.handler.clone()
            })
            .collect()
    });
    for handler in due {
        let still =
            LOOP.with_borrow(|l| (l.timers.iter()).any(|t| t.handler.as_ptr() == handler.as_ptr()));
        if still {
            // SAFETY: as above.
            unsafe { handler.onTimer() };
        }
    }
}

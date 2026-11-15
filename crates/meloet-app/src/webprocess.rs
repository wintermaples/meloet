//! The WebKit web process, which draws the screen in a process of its own.
//!
//! When the connection to meloet closes, WebKitGTK 2.52 tidies its GL contexts away before the
//! web process ends, and with the NVIDIA driver that tidying can crash inside EGL. meloet never
//! lets it get there: before it ends, it kills the web process (SIGKILL, through
//! `webkit_web_view_terminate_web_process`), which leaves nothing that needs tidying. The song
//! lives in the core, in this process, so nothing of it is lost with the web process.
//!
//! A web process that ends by itself while meloet runs is started again with the page, which
//! takes the whole state from the core again (hello → subscribe → snapshot).

#[cfg(target_os = "linux")]
mod imp {
    use std::cell::{Cell, RefCell};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
    use std::time::{Duration, Instant};

    use tauri::{AppHandle, WebviewWindow};

    use crate::core_handle::CoreHandle;
    use webkit2gtk::{WebProcessTerminationReason, WebViewExt};

    /// More crashes than this within `CRASH_WINDOW` and the page is not loaded again.
    const CRASH_LIMIT: usize = 3;
    const CRASH_WINDOW: Duration = Duration::from_secs(30);

    // GTK objects live on the main thread only.
    thread_local! {
        static VIEW: RefCell<Option<webkit2gtk::WebView>> = const { RefCell::new(None) };
        static ENDING: Cell<bool> = const { Cell::new(false) };
        static CRASHES: RefCell<Vec<Instant>> = const { RefCell::new(Vec::new()) };
    }

    /// Main thread only (the setup hook), so that the WebView is kept before anything can end.
    pub fn watch(window: &WebviewWindow) {
        let asked = window.with_webview(|platform| {
            let view = platform.inner();
            view.connect_web_process_terminated(on_terminated);
            VIEW.with(|v| *v.borrow_mut() = Some(view));
        });
        if let Err(e) = asked {
            eprintln!("meloet: cannot watch the web process: {e}");
        }
    }

    fn on_terminated(view: &webkit2gtk::WebView, reason: WebProcessTerminationReason) {
        if ENDING.get() || reason == WebProcessTerminationReason::TerminatedByApi {
            return;
        }
        eprintln!("meloet: web process terminated: {reason:?}");
        let now = Instant::now();
        let again = CRASHES.with(|c| {
            let mut c = c.borrow_mut();
            c.retain(|t| now.duration_since(*t) < CRASH_WINDOW);
            c.push(now);
            c.len() <= CRASH_LIMIT
        });
        if again {
            view.reload();
        } else {
            eprintln!("meloet: the web process keeps ending; the page is not loaded again");
        }
    }

    /// Main thread only. Kills the web process before meloet ends. Safe to call more than once.
    pub fn end() {
        ENDING.set(true);
        if let Some(view) = VIEW.with(|v| v.borrow_mut().take()) {
            view.terminate_web_process();
        }
    }

    /// SIGTERM, SIGINT and SIGHUP: the PC is ending. meloet writes the recovery file (waiting
    /// for it no longer than `crate::ENDING_WAIT`, off the main thread), then ends the usual way
    /// (see `end`). A second signal meanwhile changes nothing.
    pub fn exit_on_signals(app: &AppHandle, core: Arc<dyn CoreHandle>) {
        let ending = Arc::new(AtomicBool::new(false));
        for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            let (app, core, ending) = (app.clone(), core.clone(), ending.clone());
            gtk::glib::unix_signal_add_local(signal, move || {
                if !ending.swap(true, Relaxed) {
                    let (waiting, core) = (app.clone(), core.clone());
                    let waited = std::thread::Builder::new()
                        .name("meloet-ending".into())
                        .spawn(move || {
                            if !core.pc_ending(crate::ENDING_WAIT) {
                                eprintln!("meloet: the recovery file was not written in time");
                            }
                            waiting.exit(0);
                        });
                    if waited.is_err() {
                        app.exit(0);
                    }
                }
                gtk::glib::ControlFlow::Continue
            });
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    use std::sync::Arc;

    use tauri::{AppHandle, WebviewWindow};

    use crate::core_handle::CoreHandle;

    pub fn watch(_: &WebviewWindow) {}
    pub fn end() {}
    pub fn exit_on_signals(_: &AppHandle, _: Arc<dyn CoreHandle>) {}
}

pub use imp::{end, exit_on_signals, watch};

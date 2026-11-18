//! The Tauri shell: hands commands and Channels across, and owns no state.

mod clock;
mod core_handle;
mod devhooks;
mod logind;
mod pcwindow;
mod takes;
mod webprocess;

use std::sync::atomic::{AtomicU16, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use meloet_core::ShellRequest;
use meloet_protocol::{
    Change, Command, HelloReply, PROTOCOL_VERSION, SendReply, ServerMessage, Snapshot,
};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::webview::PageLoadEvent;
use tauri::{AppHandle, LogicalSize, Manager, RunEvent, State, WebviewWindow, WindowEvent};

use crate::core_handle::CoreHandle;

type Core = Arc<dyn CoreHandle>;

/// How long the PC's ending waits for the recovery file. The session ends the programs 5 s
/// after SIGTERM (systemd's `TimeoutStopSec` for app scopes); this leaves time to end.
const ENDING_WAIT: Duration = Duration::from_secs(3);

/// JavaScriptCore options for the page, as the environment WebKitWebProcess inherits.
/// WebKitGTK collects the page in idle time with young-only (Eden) collections only, and in
/// one long-lived page at 60 Hz the full collection almost never comes: what each frame held
/// for a moment piles up in the old space (+0.7..0.8 MB/s of RSS while playing or recording).
/// Without generations every one of those idle collections is full: about 3 a second, mostly
/// 1..3 ms each (`docs/measurements/2026-11-15-round3-fix-memory.md`, section g). A value set
/// from outside wins, to compare.
const JSC_OPTIONS: &[(&str, &str)] = &[("JSC_useGenerationalGC", "0")];

fn set_jsc_options() {
    for (name, value) in JSC_OPTIONS {
        if std::env::var_os(name).is_none() {
            // SAFETY: called first in main, before any thread exists.
            unsafe { std::env::set_var(name, value) };
        }
    }
}

#[tauri::command]
fn hello(protocol_version: u32) -> HelloReply {
    HelloReply {
        protocol_version: PROTOCOL_VERSION,
        ok: protocol_version == PROTOCOL_VERSION,
        mono_us: clock::mono_us(),
    }
}

/// Counts the pages the WebView started to load. The Channels of an earlier page still "send"
/// without an error, into a page that is gone, so they are dropped by this number.
static PAGE: AtomicU64 = AtomicU64::new(0);

#[tauri::command]
fn subscribe(
    messages: Channel<ServerMessage>,
    telemetry: Channel<InvokeResponseBody>,
    core: State<'_, Core>,
) {
    let page = PAGE.load(Relaxed);
    core.subscribe(
        Box::new(move |message| PAGE.load(Relaxed) == page && messages.send(message).is_ok()),
        Box::new(move |bytes| {
            PAGE.load(Relaxed) == page
                && telemetry
                    .send(InvokeResponseBody::Raw(bytes.to_vec()))
                    .is_ok()
        }),
    );
}

#[tauri::command]
fn snapshot(core: State<'_, Core>) -> Snapshot {
    core.snapshot()
}

#[tauri::command]
fn send(command: Command, core: State<'_, Core>) -> SendReply {
    SendReply {
        rev: core.send(command),
    }
}

#[tauri::command]
fn clock() -> u64 {
    clock::mono_us()
}

/// Development only: whether `MELOET_MEASURE_DIR` is set.
#[tauri::command]
fn measure_enabled() -> bool {
    meloet_core::measure::dir_from_env().is_some()
}

/// Development only: writes `json` to `<MELOET_MEASURE_DIR>/<name>`.
#[tauri::command]
fn measure_report(name: String, json: String) -> Result<(), String> {
    let dir = meloet_core::measure::dir_from_env().ok_or("MELOET_MEASURE_DIR is not set")?;
    meloet_core::measure::write_report(&dir, &name, &json)
}

/// Development only: the screen's input script (`MELOET_UI_SCRIPT`), only while measuring.
#[tauri::command]
fn ui_script() -> Option<String> {
    devhooks::ui_script()
}

/// The inside of the window in logical px. Only on the main thread: a getter waits for it.
fn content_size(window: &WebviewWindow) -> Option<LogicalSize<f64>> {
    let factor = window.scale_factor().ok()?;
    // With decorations drawn by GTK itself (Wayland) the window's own "inner size" counts the
    // title bar and the shadows; the webview's size does not.
    let webview: &tauri::Webview = window.as_ref();
    let size = webview.size().ok()?.to_logical::<f64>(factor);
    (size.width > 1.0 && size.height > 1.0).then_some(size)
}

/// What the window's own size counts besides its inside, in logical px: the title bar and the
/// shadows where GTK draws them itself (Wayland), nothing where the window manager does.
/// `None` before the window is on the screen. Only on the main thread.
#[cfg(target_os = "linux")]
fn decorations(window: &WebviewWindow) -> Option<(f64, f64)> {
    use gtk::prelude::WidgetExt;

    // Both are allocations of the same layout pass, so they agree even in the middle of a drag;
    // the size of the newest configure event may be one pass ahead of the webview.
    let whole = window.gtk_window().ok()?;
    let inside = content_size(window)?;
    let (width, height) = (
        f64::from(whole.allocated_width()),
        f64::from(whole.allocated_height()),
    );
    (width >= inside.width && height >= inside.height)
        .then_some((width - inside.width, height - inside.height))
}

#[cfg(not(target_os = "linux"))]
fn decorations(_: &WebviewWindow) -> Option<(f64, f64)> {
    Some((0.0, 0.0))
}

/// Keeps the inside of the window from getting smaller than the smallest window at the zoom.
#[derive(Default)]
struct SmallestSize {
    scale: AtomicU16,
    /// The zoom and the decorations the size limit was last set for.
    applied: Mutex<Option<(u16, (f64, f64))>>,
}

impl SmallestSize {
    /// Sets the limit again when the zoom or the decorations changed, and grows a window that
    /// is below it. Only on the main thread.
    fn apply(&self, window: &WebviewWindow) {
        let scale = self.scale.load(Relaxed);
        // Not known yet: start without, and come back when the window is on the screen.
        let around = decorations(window).unwrap_or((0.0, 0.0));
        let mut applied = self.applied.lock().unwrap_or_else(|e| e.into_inner());
        if *applied == Some((scale, around)) {
            return;
        }
        *applied = Some((scale, around));
        let min = meloet_core::layout::min_window(scale);
        // The limit is on the window's own size, which counts the decorations.
        let limit = LogicalSize::new(min.width + around.0, min.height + around.1);
        if let Err(e) = window.set_min_size(Some(limit)) {
            eprintln!("meloet: cannot set the smallest window size: {e}");
        }
        if let Some(now) = content_size(window)
            && (now.width < min.width || now.height < min.height)
        {
            // This size is the inside.
            let size = LogicalSize::new(now.width.max(min.width), now.height.max(min.height));
            if let Err(e) = window.set_size(size) {
                eprintln!("meloet: cannot grow the window: {e}");
            }
        }
    }
}

/// The shell learns the zoom the way the screen does: it listens to the patches first, then
/// reads the snapshot. The core stays the only owner of the zoom.
fn follow_scale(core: &Core, window: WebviewWindow) {
    let smallest = Arc::new(SmallestSize::default());
    let (listener, on_patch) = (window.clone(), smallest.clone());
    core.subscribe(
        Box::new(move |message| {
            if let ServerMessage::Patch(patch) = message {
                for change in &patch.changes {
                    // The workspace section also changes at every resize; only a new zoom is
                    // passed on.
                    if let Change::Workspace(workspace) = change
                        && on_patch.scale.swap(workspace.scale, Relaxed) != workspace.scale
                    {
                        // This is the core thread, and the main thread may be waiting for it.
                        let (window, smallest) = (listener.clone(), on_patch.clone());
                        let _ = listener.run_on_main_thread(move || smallest.apply(&window));
                    }
                }
            }
            true
        }),
        // No telemetry wanted: this sink is dropped at the first reading.
        Box::new(|_| false),
    );
    let scale = core.snapshot().state.workspace.scale;
    smallest.scale.store(scale, Relaxed);
    smallest.apply(&window);
    // The decorations are known only once the window is on the screen, and can change.
    let resized = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Resized(_) = event {
            smallest.apply(&resized);
        }
    });
}

/// What the screen the window is on can give to the inside of the window, in logical px: its
/// work area minus what the window takes around its inside.
fn screen_space(window: &WebviewWindow) -> Option<(f32, f32)> {
    let monitor = window.current_monitor().ok()??;
    let area = monitor
        .work_area()
        .size
        .to_logical::<f64>(monitor.scale_factor());
    let factor = window.scale_factor().ok()?;
    let outer = window.outer_size().ok()?.to_logical::<f64>(factor);
    let inside = content_size(window);
    if std::env::var_os("MELOET_DEBUG_SCREEN").is_some() {
        eprintln!("meloet: work area {area:?}, window {outer:?}, inside {inside:?}");
    }
    // Neither size is known before the window is on the screen.
    let around = |outer: f64, inside: Option<f64>| inside.map_or(0.0, |i| (outer - i).max(0.0));
    let width = area.width - around(outer.width, inside.map(|s| s.width));
    let height = area.height - around(outer.height, inside.map(|s| s.height));
    (width > 0.0 && height > 0.0).then_some((width as f32, height as f32))
}

/// Tells the core the screen space now and whenever it may have changed. The core is told only
/// when the numbers differ, and never waits here: this runs on the main thread.
fn follow_screen(core: &Core, window: &WebviewWindow) {
    let tell = {
        let (core, window) = (core.clone(), window.clone());
        let last = Mutex::new(None);
        move || {
            let space = screen_space(&window);
            let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
            if let Some((width, height)) = space
                && *last != space
            {
                *last = space;
                core.set_screen_space(width, height);
            }
        }
    };
    tell();
    window.on_window_event(move |event| {
        if matches!(
            event,
            WindowEvent::Moved(_)
                | WindowEvent::Resized(_)
                | WindowEvent::ScaleFactorChanged { .. }
        ) {
            tell();
        }
    });
}

/// What the core asks of the shell. On the core thread: nothing here waits for the main thread.
fn shell_request(
    app: &AppHandle,
    window: &WebviewWindow,
    core: &Weak<dyn CoreHandle>,
    request: ShellRequest,
) {
    let (pick, folder) = match request {
        ShellRequest::Exit => {
            app.exit(0);
            return;
        }
        ShellRequest::ShowFile { path } => {
            pcwindow::show_file(path);
            return;
        }
        ShellRequest::PickSongFile { folder } => {
            if let Some(path) = devhooks::take_open_song() {
                if let Some(core) = core.upgrade() {
                    core.song_file_picked(Some(path));
                }
                return;
            }
            (pcwindow::Pick::SongFile, folder)
        }
        ShellRequest::PickFolder { folder } => (pcwindow::Pick::Folder, folder),
        ShellRequest::PickSaveAs { folder, name } => (pcwindow::Pick::SaveAs { name }, folder),
    };
    let answer = {
        let (core, pick) = (core.clone(), pick.clone());
        move |path| {
            if let Some(core) = core.upgrade() {
                match pick {
                    pcwindow::Pick::SongFile => core.song_file_picked(path),
                    pcwindow::Pick::Folder => core.folder_picked(path),
                    pcwindow::Pick::SaveAs { .. } => core.save_as_picked(path),
                }
            }
        }
    };
    let on_main = window.clone();
    let shared = Arc::new(Mutex::new(Some(answer)));
    let fallback = shared.clone();
    let asked = window.run_on_main_thread(move || {
        if let Some(answer) = shared.lock().unwrap_or_else(|e| e.into_inner()).take() {
            pcwindow::pick(&on_main, pick, folder, answer);
        }
    });
    if let Err(e) = asked {
        eprintln!("meloet: cannot show the PC's window: {e}");
        if let Some(answer) = fallback.lock().unwrap_or_else(|e| e.into_inner()).take() {
            answer(None);
        }
    }
}

fn main() {
    set_jsc_options();
    devhooks::read_env();
    let core = Arc::new(meloet_core::Core::start(
        meloet_core::CoreOptions::from_env(),
    ));
    let shell_side: Core = core.clone();
    let window_side = shell_side.clone();
    tauri::Builder::default()
        .manage(shell_side)
        .invoke_handler(tauri::generate_handler![
            hello,
            subscribe,
            snapshot,
            send,
            clock,
            measure_enabled,
            measure_report,
            ui_script,
            takes::take_notes,
            takes::live_notes
        ])
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                follow_scale(&window_side, window.clone());
                follow_screen(&window_side, &window);
                devhooks::apply_window_size(&window);
                webprocess::watch(&window);
                let (app_handle, shell_window) = (app.handle().clone(), window.clone());
                let weak = Arc::downgrade(&window_side);
                window_side.set_shell(Box::new(move |request| {
                    shell_request(&app_handle, &shell_window, &weak, request);
                }));
                // The core decides when meloet ends (after asking about unsaved changes), and
                // ends it through `ShellRequest::Exit`; the web process is stopped only then.
                let closing = window_side.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        closing.close_requested();
                    }
                });
            }
            webprocess::exit_on_signals(app.handle(), window_side.clone());
            logind::watch(Arc::downgrade(&window_side));
            Ok(())
        })
        .on_page_load(|webview, payload| {
            if payload.event() == PageLoadEvent::Started {
                PAGE.fetch_add(1, Relaxed);
            }
            devhooks::on_page_load(webview, payload);
        })
        .build(tauri::generate_context!())
        .expect("failed to run meloet")
        .run(move |_app, event| {
            if let RunEvent::Exit = event {
                webprocess::end();
                core.stop();
            }
        });
}

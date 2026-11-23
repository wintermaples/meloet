//! The Tauri shell: hands commands and Channels across, and owns no state.

mod clock;
mod core_handle;
#[cfg(feature = "dev-hooks")]
mod devhooks;
mod logind;
#[cfg(target_os = "linux")]
mod mcp;
mod pcwindow;
#[cfg(target_os = "linux")]
mod single;
mod takes;
mod webprocess;

use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use meloet_core::ShellRequest;
use meloet_protocol::{
    Change, Command, HelloReply, Language, PROTOCOL_VERSION, SendReply, ServerMessage, Snapshot,
};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::webview::PageLoadEvent;
use tauri::{
    AppHandle, DragDropEvent, LogicalSize, Manager, RunEvent, State, WebviewWindow, WindowEvent,
};

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
/// 1..3 ms each (`docs/history/measurements/2026-11-15-round3-fix-memory.md`, section g).
/// The page is drawn without the compositor: with it, a key waits for the compositor's next
/// frame (8..10 ms more to the screen at p50) and an open mixer costs more
/// (`docs/history/measurements/2026-11-22-round8-quiet.md`). A value set from outside wins, to compare.
const JSC_OPTIONS: &[(&str, &str)] = &[
    ("JSC_useGenerationalGC", "0"),
    ("WEBKIT_DISABLE_COMPOSITING_MODE", "1"),
];

fn set_jsc_options() {
    for (name, value) in JSC_OPTIONS {
        if std::env::var_os(name).is_none() {
            // SAFETY: called first in main, before any thread exists.
            unsafe { std::env::set_var(name, value) };
        }
    }
}

#[tauri::command]
fn hello(protocol_version: u32, core: State<'_, Core>) -> HelloReply {
    let language = core.snapshot().state.workspace.language;
    ENGLISH.store(language == Language::En, Relaxed);
    HelloReply {
        protocol_version: PROTOCOL_VERSION,
        ok: protocol_version == PROTOCOL_VERSION,
        mono_us: clock::mono_us(),
        language,
    }
}

/// The language of the page, as its greeting took it: the page loads itself again whenever
/// the language changes, so the PC's windows the shell shows follow it.
static ENGLISH: AtomicBool = AtomicBool::new(false);

fn page_language() -> Language {
    if ENGLISH.load(Relaxed) {
        Language::En
    } else {
        Language::Ja
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

/// `send` without the reply: the screen does not wait for the core (the keys of the PC
/// keyboard, R2).
#[tauri::command]
fn post(command: Command, core: State<'_, Core>) {
    core.post(command);
}

#[tauri::command]
fn clock() -> u64 {
    clock::mono_us()
}

/// Development only: whether `MELOET_MEASURE_DIR` is set.
#[cfg(feature = "dev-hooks")]
#[tauri::command]
fn measure_enabled() -> bool {
    meloet_core::measure::dir_from_env().is_some()
}

/// Development only: writes `json` to `<MELOET_MEASURE_DIR>/<name>`.
#[cfg(feature = "dev-hooks")]
#[tauri::command]
fn measure_report(name: String, json: String) -> Result<(), String> {
    let dir = meloet_core::measure::dir_from_env().ok_or("MELOET_MEASURE_DIR is not set")?;
    meloet_core::measure::write_report(&dir, &name, &json)
}

/// Development only: the screen's input script (`MELOET_UI_SCRIPT`), only while measuring.
#[cfg(feature = "dev-hooks")]
#[tauri::command]
fn ui_script() -> Option<String> {
    devhooks::ui_script()
}

/// Where a drag over the window is, in px of the page from the top left corner of the
/// window's inside.
///
/// On Linux the numbers are the `x` and `y` of GTK's `drag-motion` and `drag-drop` signals
/// on the WebView (wry 0.57.2 `src/webkitgtk/drag_drop.rs`): the widget's own coordinates,
/// which GTK counts in logical px whatever the screen's scale factor. tauri-runtime-wry
/// 2.12.3 (`src/lib.rs`, `with_drag_drop_handler`) wraps them in a `PhysicalPosition`
/// without scaling them, so they are not physical px here. meloet zooms with CSS variables
/// and never the WebView's own zoom, so a logical px of the widget is a px of the page.
/// The place `Enter` gives is the last motion before the files were known, (0, 0) when
/// there was none; the next motion comes as an `Over`.
fn page_px(position: &tauri::PhysicalPosition<f64>) -> (i32, i32) {
    (position.x.round() as i32, position.y.round() as i32)
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
    #[cfg(feature = "dev-hooks")]
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
        ShellRequest::OpenUrl { url } => {
            pcwindow::open_url(url);
            return;
        }
        ShellRequest::OpenFile { path } => {
            pcwindow::open_file(path);
            return;
        }
        ShellRequest::PickSongFile { folder } => {
            #[cfg(feature = "dev-hooks")]
            if let Some(path) = devhooks::take_open_song() {
                if let Some(core) = core.upgrade() {
                    core.song_file_picked(Some(path));
                }
                return;
            }
            (pcwindow::Pick::SongFile, folder)
        }
        ShellRequest::PickFolder { folder } => (pcwindow::Pick::Folder, folder),
        ShellRequest::PickPluginPlace { folder } => (pcwindow::Pick::PluginPlace, folder),
        ShellRequest::PickSaveAs { folder, name } => (pcwindow::Pick::SaveAs { name }, folder),
        ShellRequest::PickSf2 { folder } => (pcwindow::Pick::Sf2, folder),
        ShellRequest::PickAudio { folder, many } => (pcwindow::Pick::Audio { many }, folder),
        ShellRequest::PickExportPlace {
            folder,
            folder_only: true,
            ..
        } => (pcwindow::Pick::ExportFolder, folder),
        ShellRequest::PickExportPlace { folder, name, .. } => {
            (pcwindow::Pick::ExportFile { name }, folder)
        }
        ShellRequest::PickExportPicture { folder } => (pcwindow::Pick::Picture, folder),
    };
    let answer = {
        let (core, pick) = (core.clone(), pick.clone());
        move |paths: Vec<std::path::PathBuf>| {
            if let Some(core) = core.upgrade() {
                if let pcwindow::Pick::Audio { .. } = pick {
                    core.audio_picked(paths);
                    return;
                }
                let path = paths.into_iter().next();
                match pick {
                    pcwindow::Pick::SongFile => core.song_file_picked(path),
                    pcwindow::Pick::Folder => core.folder_picked(path),
                    pcwindow::Pick::PluginPlace => core.plugin_place_picked(path),
                    pcwindow::Pick::SaveAs { .. } => core.save_as_picked(path),
                    pcwindow::Pick::Sf2 => core.sf2_picked(path),
                    pcwindow::Pick::ExportFile { .. } | pcwindow::Pick::ExportFolder => {
                        core.export_place_picked(path);
                    }
                    pcwindow::Pick::Picture => core.export_picture_picked(path),
                    pcwindow::Pick::Audio { .. } => {}
                }
            }
        }
    };
    let on_main = window.clone();
    let shared = Arc::new(Mutex::new(Some(answer)));
    let fallback = shared.clone();
    let asked = window.run_on_main_thread(move || {
        if let Some(answer) = shared.lock().unwrap_or_else(|e| e.into_inner()).take() {
            pcwindow::pick(&on_main, pick, page_language(), folder, answer);
        }
    });
    if let Err(e) = asked {
        eprintln!("meloet: cannot show the PC's window: {e}");
        if let Some(answer) = fallback.lock().unwrap_or_else(|e| e.into_inner()).take() {
            answer(Vec::new());
        }
    }
}

/// The socket of 「AI とつなぐ」 follows the switch; the core holds the link, and ends it when
/// the core ends.
fn start_ai_link(core: &Arc<meloet_core::Core>) {
    let meloet = Arc::new(meloet_ai::CoreLink(Arc::downgrade(core)));
    match meloet_ai::AiLink::start(meloet, meloet_ai::Options::from_env()) {
        Ok(link) => core.set_ai_switch(Box::new(move |on| link.switch(on))),
        Err(e) => eprintln!("meloet: no thread for the AI link: {e}"),
    }
}

/// Becomes the only meloet of this user, or hands the song file to the one that runs and ends
/// here, before anything two would fight over is started.
#[cfg(target_os = "linux")]
fn first_or_hand_over() -> (Option<single::First>, Option<std::path::PathBuf>) {
    let handed = single::Handed::from_start();
    let runtime = meloet_ai::Options::from_env().runtime_dir;
    match single::start(runtime.as_deref(), single::my_uid(), &handed) {
        single::Start::First(first) => (Some(first), handed.song),
        single::Start::Handed => std::process::exit(0),
        single::Start::NoAnswer(e) => {
            eprintln!("meloet: another meloet is running and does not answer: {e}");
            std::process::exit(1);
        }
        single::Start::Alone(why) => {
            eprintln!("meloet: cannot check for another meloet: {why}");
            (None, handed.song)
        }
    }
}

/// What a later meloet handed over: the song is opened, and the window comes to the front.
#[cfg(target_os = "linux")]
fn listen_for_later(
    first: &single::First,
    core: Weak<dyn CoreHandle>,
    window: Arc<OnceLock<WebviewWindow>>,
) {
    let listener = match first.listener.try_clone() {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("meloet: the socket for a second meloet: {e}");
            return;
        }
    };
    let listened = single::listen(listener, single::my_uid(), move |handed| {
        if let (Some(song), Some(core)) = (handed.song, core.upgrade()) {
            core.open_given(song);
        }
        if let Some(window) = window.get() {
            to_front(window, handed.token);
        }
    });
    if let Err(e) = listened {
        eprintln!("meloet: no thread for a second meloet: {e}");
    }
}

#[cfg(target_os = "linux")]
fn to_front(window: &WebviewWindow, token: Option<String>) {
    use gtk::prelude::GtkWindowExt;

    let on_main = window.clone();
    let asked = window.run_on_main_thread(move || {
        let _ = on_main.unminimize();
        match on_main.gtk_window() {
            Ok(gtk_window) => {
                // The token the desktop gave the later meloet: without it, a Wayland desktop
                // may only say that meloet wants attention.
                if let Some(token) = &token {
                    gtk_window.set_startup_id(token);
                }
                gtk_window.present();
            }
            Err(_) => {
                let _ = on_main.set_focus();
            }
        }
    });
    if let Err(e) = asked {
        eprintln!("meloet: cannot bring the window to the front: {e}");
    }
}

fn main() {
    #[cfg(target_os = "linux")]
    mcp::become_bridge_if_asked();
    set_jsc_options();
    #[cfg(feature = "dev-hooks")]
    devhooks::read_env();
    #[cfg(target_os = "linux")]
    let (first, song) = first_or_hand_over();
    let core = Arc::new(meloet_core::Core::start(
        meloet_core::CoreOptions::from_env(),
    ));
    start_ai_link(&core);
    let shell_side: Core = core.clone();
    let window_side = shell_side.clone();
    let main_window: Arc<OnceLock<WebviewWindow>> = Arc::default();
    #[cfg(target_os = "linux")]
    {
        if let Some(song) = song {
            shell_side.open_given(song);
        }
        if let Some(first) = &first {
            listen_for_later(first, Arc::downgrade(&shell_side), main_window.clone());
        }
    }
    tauri::Builder::default()
        .manage(shell_side)
        .invoke_handler(tauri::generate_handler![
            hello,
            subscribe,
            snapshot,
            send,
            post,
            clock,
            #[cfg(feature = "dev-hooks")]
            measure_enabled,
            #[cfg(feature = "dev-hooks")]
            measure_report,
            #[cfg(feature = "dev-hooks")]
            ui_script,
            takes::take_notes,
            takes::peaks,
            takes::lane,
            takes::history_rows,
            takes::live_notes,
            takes::plugin_params
        ])
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = main_window.set(window.clone());
                follow_scale(&window_side, window.clone());
                follow_screen(&window_side, &window);
                #[cfg(feature = "dev-hooks")]
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
                // Files held over the window and let go of there (an SF2 file to add, audio
                // files to bring in). The core decides what they are and keeps their paths;
                // the screen shows what the core says, and for audio files works out from the
                // pointer's place where in the song they go.
                let dropping = window_side.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::DragDrop(drag) = event {
                        match drag {
                            DragDropEvent::Enter { paths, position } => {
                                dropping.files_over(paths.clone());
                                let (x, y) = page_px(position);
                                dropping.files_at(x, y);
                            }
                            DragDropEvent::Over { position } => {
                                let (x, y) = page_px(position);
                                dropping.files_at(x, y);
                            }
                            DragDropEvent::Drop { paths, position } => {
                                let (x, y) = page_px(position);
                                dropping.files_at(x, y);
                                dropping.files_dropped(paths.clone());
                            }
                            DragDropEvent::Leave => dropping.files_left(),
                            // The enum may grow.
                            _ => {}
                        }
                    }
                });
            }
            webprocess::exit_on_signals(app.handle(), window_side.clone());
            logind::watch(Arc::downgrade(&window_side));
            Ok(())
        })
        .on_page_load(|_webview, payload| {
            if payload.event() == PageLoadEvent::Started {
                PAGE.fetch_add(1, Relaxed);
            }
            #[cfg(feature = "dev-hooks")]
            devhooks::on_page_load(_webview, payload);
        })
        .build(tauri::generate_context!())
        .expect("failed to run meloet")
        .run(move |_app, event| {
            if let RunEvent::Exit = event {
                webprocess::end();
                core.stop();
                #[cfg(target_os = "linux")]
                if let Some(first) = &first {
                    first.remove_socket();
                }
            }
        });
}

#[cfg(test)]
mod embedded_page {
    use tauri::utils::assets::AssetKey;

    /// The page as this build carries it inside the program, written out file by file so that
    /// `scripts/check-release.sh` can look for the development hooks in it.
    #[test]
    #[ignore = "a check of the release: writes the page to MELOET_ASSETS_OUT"]
    fn write_the_embedded_page() {
        let out = std::path::PathBuf::from(
            std::env::var_os("MELOET_ASSETS_OUT").expect("MELOET_ASSETS_OUT names a folder"),
        );
        let context: tauri::Context<tauri::Wry> = tauri::generate_context!();
        let assets = context.assets();
        let keys: Vec<String> = assets.iter().map(|(key, _)| key.into_owned()).collect();
        assert!(!keys.is_empty(), "no page inside the program");
        for key in keys {
            let bytes = assets
                .get(&AssetKey::from(key.as_str()))
                .unwrap_or_else(|| panic!("{key} does not read"));
            let path = out.join(key.trim_start_matches('/'));
            std::fs::create_dir_all(path.parent().expect("a folder")).unwrap();
            std::fs::write(&path, &bytes).unwrap();
        }
    }
}

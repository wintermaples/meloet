//! Development hooks. Each one does nothing unless its environment variable is set.
//!
//! - `MELOET_WINDOW=1280x800`: inner size of the window, in logical px.
//! - `MELOET_SNAPSHOT=/path/out.png` (+ `MELOET_SNAPSHOT_AFTER_MS=1500`): that long after the
//!   page finished loading, write what the WebView shows to the PNG and exit.
//! - `MELOET_COMMANDS='[{"afterMs":800,"command":{"type":"chooserConfirm"}}]'`: that long after
//!   the page finished loading, send each intent to the core, the way the screen does.
//! - `MELOET_EXIT_AFTER_MS=20000`: that long after the page finished loading, exit the usual
//!   way (the core stops and writes its measurements).
//! - `MELOET_CLOSE_AFTER_MS=2500`: that long after the page finished loading, close the window
//!   the way its close button does.
//! - `MELOET_PCWINDOW_SHOT=/path/out.png`: 1.5 s after the PC's open or folder window is shown,
//!   print what GTK has up under its title and draw it into the PNG. A window the desktop portal
//!   shows is another program's and is not found.
//! - `MELOET_DEBUG_SCREEN=1`: print the work area, the window and its inside to stderr each time
//!   the shell works out the screen space (read in `main.rs`).

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use meloet_protocol::Command;
use tauri::webview::{PageLoadEvent, PageLoadPayload};
use tauri::{LogicalSize, Manager, Webview, WebviewWindow};

const DEFAULT_SNAPSHOT_AFTER_MS: u64 = 1500;

#[derive(Debug, Default, PartialEq)]
struct Script {
    commands: Vec<(u64, Command)>,
    exit_after_ms: Option<u64>,
}

static SCRIPT: OnceLock<Script> = OnceLock::new();

/// Reads `MELOET_COMMANDS` and `MELOET_EXIT_AFTER_MS`. A value that cannot be read is reported
/// on stderr and does nothing.
pub fn read_env() {
    let var = |name: &str| std::env::var(name).ok();
    let (script, complaints) = parse_script(
        var("MELOET_COMMANDS").as_deref(),
        var("MELOET_EXIT_AFTER_MS").as_deref(),
    );
    for complaint in complaints {
        eprintln!("{complaint}");
    }
    let _ = SCRIPT.set(script);
}

fn parse_script(commands: Option<&str>, exit_after_ms: Option<&str>) -> (Script, Vec<String>) {
    let mut script = Script::default();
    let mut complaints = Vec::new();
    match commands.map(parse_commands) {
        Some(Ok(commands)) => script.commands = commands,
        Some(Err(e)) => complaints.push(format!("MELOET_COMMANDS: {e}")),
        None => {}
    }
    if let Some(text) = exit_after_ms {
        script.exit_after_ms = text.trim().parse().ok();
        if script.exit_after_ms.is_none() {
            complaints.push(format!(
                "MELOET_EXIT_AFTER_MS: expected milliseconds, got {text:?}"
            ));
        }
    }
    (script, complaints)
}

/// All or nothing: one entry that cannot be read makes the whole list an error.
fn parse_commands(text: &str) -> Result<Vec<(u64, Command)>, String> {
    let entries: Vec<serde_json::Value> = serde_json::from_str(text).map_err(|e| e.to_string())?;
    entries
        .into_iter()
        .enumerate()
        .map(|(index, mut entry)| {
            let after_ms = entry
                .get("afterMs")
                .and_then(serde_json::Value::as_u64)
                .ok_or(format!("entry {index}: afterMs must be a whole number"))?;
            let command = entry
                .get_mut("command")
                .map(serde_json::Value::take)
                .ok_or(format!("entry {index}: no command"))?;
            let command = serde_json::from_value(command)
                .map_err(|e| format!("entry {index}: command: {e}"))?;
            Ok((after_ms, command))
        })
        .collect()
}

pub fn apply_window_size(window: &WebviewWindow) {
    let Ok(text) = std::env::var("MELOET_WINDOW") else {
        return;
    };
    match parse_size(&text) {
        Some((w, h)) => {
            if let Err(e) = window.set_size(LogicalSize::new(w, h)) {
                eprintln!("MELOET_WINDOW: {e}");
            }
        }
        None => eprintln!("MELOET_WINDOW: expected WIDTHxHEIGHT, got {text:?}"),
    }
}

fn parse_size(text: &str) -> Option<(f64, f64)> {
    let (w, h) = text.split_once('x')?;
    let (w, h) = (w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?);
    (w > 0 && h > 0).then_some((f64::from(w), f64::from(h)))
}

pub fn on_page_load(webview: &Webview, payload: &PageLoadPayload<'_>) {
    static STARTED: AtomicBool = AtomicBool::new(false);

    if payload.event() != PageLoadEvent::Finished {
        return;
    }
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    run_script(webview);
    let Some(path) = std::env::var_os("MELOET_SNAPSHOT").map(PathBuf::from) else {
        return;
    };
    let after = std::env::var("MELOET_SNAPSHOT_AFTER_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_SNAPSHOT_AFTER_MS);
    let webview = webview.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(after));
        let app = tauri::Manager::app_handle(&webview).clone();
        let asked = webview.with_webview(move |platform| snapshot(platform, path, app));
        if let Err(e) = asked {
            eprintln!("MELOET_SNAPSHOT: {e}");
            std::process::exit(1);
        }
    });
}

fn run_script(webview: &Webview) {
    let Some(script) = SCRIPT.get() else {
        return;
    };
    let app = webview.app_handle();
    for (after_ms, command) in script.commands.iter().cloned() {
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(after_ms));
            app.state::<crate::Core>().send(command);
        });
    }
    if let Some(after_ms) = script.exit_after_ms {
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(after_ms));
            app.exit(0);
        });
    }
    if let Some(after_ms) = std::env::var("MELOET_CLOSE_AFTER_MS")
        .ok()
        .and_then(|v| v.trim().parse().ok())
    {
        let window = webview.window();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(after_ms));
            if let Err(e) = window.close() {
                eprintln!("MELOET_CLOSE_AFTER_MS: {e}");
            }
        });
    }
}

#[cfg(target_os = "linux")]
fn snapshot(platform: tauri::webview::PlatformWebview, path: PathBuf, app: tauri::AppHandle) {
    use webkit2gtk::{SnapshotOptions, SnapshotRegion, WebViewExt};

    platform.inner().snapshot(
        SnapshotRegion::Visible,
        SnapshotOptions::NONE,
        gio::Cancellable::NONE,
        move |result| {
            let written = result.map_err(|e| e.to_string()).and_then(|surface| {
                let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
                surface.write_to_png(&mut file).map_err(|e| e.to_string())
            });
            match written {
                Ok(()) => {
                    eprintln!("MELOET_SNAPSHOT: wrote {}", path.display());
                    app.exit(0);
                }
                Err(e) => {
                    eprintln!("MELOET_SNAPSHOT: {e}");
                    app.exit(1);
                }
            }
        },
    );
}

#[cfg(not(target_os = "linux"))]
fn snapshot(_: tauri::webview::PlatformWebview, _: PathBuf, app: tauri::AppHandle) {
    eprintln!("MELOET_SNAPSHOT: only on Linux (webkit2gtk)");
    app.exit(1);
}

/// Main thread only, right after the PC's window titled `title` was shown.
#[cfg(target_os = "linux")]
pub fn after_pc_window(title: &'static str) {
    use gtk::prelude::{Cast, GtkWindowExt, WidgetExt};

    let Some(path) = std::env::var_os("MELOET_PCWINDOW_SHOT").map(PathBuf::from) else {
        return;
    };
    gtk::glib::timeout_add_local_once(Duration::from_millis(1500), move || {
        let shown: Vec<gtk::Window> = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Window>().ok())
            .filter(|w| w.title().as_deref() == Some(title) && w.is_visible())
            .collect();
        eprintln!(
            "MELOET_PCWINDOW_SHOT: {} window(s) titled {title:?}",
            shown.len()
        );
        let Some(window) = shown.first() else {
            return;
        };
        let (width, height) = (window.allocated_width(), window.allocated_height());
        eprintln!(
            "MELOET_PCWINDOW_SHOT: {width}x{height}, mapped {}",
            window.is_mapped()
        );
        let drawn = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)
            .map_err(|e| e.to_string())
            .and_then(|surface| {
                let cr = cairo::Context::new(&surface).map_err(|e| e.to_string())?;
                window.draw(&cr);
                drop(cr);
                let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
                surface.write_to_png(&mut file).map_err(|e| e.to_string())
            });
        match drawn {
            Ok(()) => eprintln!("MELOET_PCWINDOW_SHOT: wrote {}", path.display()),
            Err(e) => eprintln!("MELOET_PCWINDOW_SHOT: {e}"),
        }
    });
}

#[cfg(not(target_os = "linux"))]
pub fn after_pc_window(_: &'static str) {}

#[cfg(test)]
mod tests {
    use meloet_protocol::Command;

    use super::{Script, parse_commands, parse_script, parse_size};

    #[test]
    fn window_size_text() {
        assert_eq!(parse_size("1280x800"), Some((1280.0, 800.0)));
        assert_eq!(parse_size("1280"), None);
        assert_eq!(parse_size("0x800"), None);
        assert_eq!(parse_size("axb"), None);
    }

    #[test]
    fn commands_text() {
        let text = r#"[
            {"afterMs": 800, "command": {"type": "chooserConfirm"}},
            {"afterMs": 0, "command": {"type": "openSettings", "advanced": true}}
        ]"#;
        assert_eq!(
            parse_commands(text),
            Ok(vec![
                (800, Command::ChooserConfirm),
                (0, Command::OpenSettings { advanced: true }),
            ])
        );
        assert_eq!(parse_commands("[]"), Ok(vec![]));
    }

    #[test]
    fn one_broken_entry_drops_every_command() {
        let good = r#"{"afterMs": 1, "command": {"type": "back"}}"#;
        for bad in [
            "not json".to_string(),
            r#"{"afterMs": 1}"#.to_string(),
            format!(r#"[{good}, {{"afterMs": 1}}]"#),
            format!(r#"[{good}, {{"command": {{"type": "back"}}}}]"#),
            format!(r#"[{good}, {{"afterMs": -1, "command": {{"type": "back"}}}}]"#),
            format!(r#"[{good}, {{"afterMs": 1, "command": {{"type": "noSuchThing"}}}}]"#),
        ] {
            assert!(parse_commands(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn script_from_the_two_variables() {
        assert_eq!(parse_script(None, None), (Script::default(), vec![]));

        let (script, complaints) = parse_script(
            Some(r#"[{"afterMs": 5, "command": {"type": "back"}}]"#),
            Some(" 2500 "),
        );
        assert_eq!(script.commands, vec![(5, Command::Back)]);
        assert_eq!(script.exit_after_ms, Some(2500));
        assert!(complaints.is_empty());

        let (script, complaints) = parse_script(Some("["), Some("soon"));
        assert_eq!(script, Script::default());
        assert_eq!(complaints.len(), 2);
        assert!(complaints[0].starts_with("MELOET_COMMANDS: "));
        assert!(complaints[1].starts_with("MELOET_EXIT_AFTER_MS: "));
    }
}

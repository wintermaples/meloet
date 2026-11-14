//! Development hooks. Each one does nothing unless its environment variable is set.
//!
//! - `MELOET_WINDOW=1280x800`: inner size of the window, in logical px.
//! - `MELOET_SNAPSHOT=/path/out.png` (+ `MELOET_SNAPSHOT_AFTER_MS=1500`): that long after the
//!   page finished loading, write what the WebView shows to the PNG and exit.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::webview::{PageLoadEvent, PageLoadPayload};
use tauri::{LogicalSize, Webview, WebviewWindow};

const DEFAULT_SNAPSHOT_AFTER_MS: u64 = 1500;

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
    let Some(path) = std::env::var_os("MELOET_SNAPSHOT").map(PathBuf::from) else {
        return;
    };
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
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

#[cfg(test)]
mod tests {
    use super::parse_size;

    #[test]
    fn window_size_text() {
        assert_eq!(parse_size("1280x800"), Some((1280.0, 800.0)));
        assert_eq!(parse_size("1280"), None);
        assert_eq!(parse_size("0x800"), None);
        assert_eq!(parse_size("axb"), None);
    }
}

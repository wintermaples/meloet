#[cfg(target_os = "linux")]
mod webkit_features;

use std::{collections::HashMap, sync::Mutex, time::Duration};

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use tauri::{LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder};

struct Out(Mutex<Option<String>>, serde_json::Value);

#[tauri::command]
fn bench_done(result: String, app: tauri::AppHandle, out: tauri::State<'_, Out>) {
    let mut v: serde_json::Value = serde_json::from_str(&result).unwrap_or(serde_json::Value::Null);
    v["host"] = out.1.clone();
    if let Some(path) = out.0.lock().unwrap().take() {
        std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap()).expect("write result");
    }
    app.exit(0);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut a: HashMap<String, String> = HashMap::new();
    for pair in args[1..].chunks(2) {
        if let [k, v] = pair {
            a.insert(k.trim_start_matches("--").to_string(), v.clone());
        }
    }
    let get = |k: &str, d: &str| a.get(k).cloned().unwrap_or_else(|| d.to_string());
    let (w, h): (f64, f64) = (get("w", "1280").parse().unwrap(), get("h", "800").parse().unwrap());
    let stress: u64 = get("resize-stress", "0").parse().unwrap();
    let no60 = get("no60", "0") == "1";
    let query = format!(
        "renderer={}&meters={}&dur={}&warm={}&display={}",
        get("renderer", "2d"), get("meters", "0"), get("dur", "10000"), get("warm", "1500"), get("display", "16.667")
    );
    let env: serde_json::Map<String, serde_json::Value> = [
        "WEBKIT_DISABLE_DMABUF_RENDERER", "__NV_DISABLE_EXPLICIT_SYNC", "WEBKIT_DISABLE_COMPOSITING_MODE", "GDK_BACKEND",
    ]
    .iter()
    .map(|k| (k.to_string(), std::env::var(k).map(serde_json::Value::from).unwrap_or(serde_json::Value::Null)))
    .collect();
    let out_path = a.get("out").cloned();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![bench_done])
        .setup(move |app| {
            let win = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("s1-render")
                .inner_size(w, h)
                .resizable(true)
                .initialization_script(format!("window.__BENCH = {query:?};"))
                .build()?;
            let backend = match win.window_handle().map(|h| h.as_raw()) {
                Ok(RawWindowHandle::Wayland(_)) => "wayland",
                Ok(RawWindowHandle::Xlib(_)) => "x11",
                Ok(_) => "other",
                Err(_) => "unavailable",
            };
            let mut host = serde_json::Map::new();
            host.insert("env".into(), env.into());
            host.insert("backend".into(), backend.into());
            host.insert("requested".into(), serde_json::json!([w, h]));
            host.insert("no60".into(), no60.into());
            #[cfg(target_os = "linux")]
            if std::env::var_os("MELOET_DUMP_FEATURES").is_some() {
                win.with_webview(|wv| webkit_features::dump(&wv.inner()))?;
            }
            #[cfg(target_os = "linux")]
            if no60 {
                win.with_webview(|wv| {
                    let v = wv.inner();
                    let before = webkit_features::set(&v, "PreferPageRenderingUpdatesNear60FPS", false);
                    eprintln!("PreferPageRenderingUpdatesNear60FPS was {before:?}, now false");
                    webkit2gtk::WebViewExt::reload(&v);
                })?;
            }
            app.manage(Out(Mutex::new(out_path), host.into()));
            if stress > 0 {
                let win = win.clone();
                std::thread::spawn(move || {
                    for i in 0..(stress * 1000 / 120) {
                        let s = if i % 2 == 0 { LogicalSize::new(w * 0.6, h * 0.7) } else { LogicalSize::new(w, h) };
                        let _ = win.set_size(s);
                        std::thread::sleep(Duration::from_millis(120));
                    }
                    let _ = win.set_size(LogicalSize::new(w, h));
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("tauri run");
}

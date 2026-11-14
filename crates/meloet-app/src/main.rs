//! The Tauri shell: hands commands and Channels across, and owns no state.

mod clock;
mod core_handle;
mod devhooks;

use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering::Relaxed};

use meloet_protocol::{
    Change, Command, HelloReply, PROTOCOL_VERSION, SendReply, ServerMessage, Snapshot,
};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{LogicalSize, Manager, RunEvent, State, WebviewWindow};

use crate::core_handle::CoreHandle;

type Core = Arc<dyn CoreHandle>;

#[tauri::command]
fn hello(protocol_version: u32) -> HelloReply {
    HelloReply {
        protocol_version: PROTOCOL_VERSION,
        ok: protocol_version == PROTOCOL_VERSION,
        mono_us: clock::mono_us(),
    }
}

#[tauri::command]
fn subscribe(
    messages: Channel<ServerMessage>,
    telemetry: Channel<InvokeResponseBody>,
    core: State<'_, Core>,
) {
    core.subscribe(
        Box::new(move |message| messages.send(message).is_ok()),
        Box::new(move |bytes| {
            telemetry
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

fn set_min_size(window: &WebviewWindow, scale: u16) {
    let min = meloet_core::layout::min_window(scale);
    if let Err(e) = window.set_min_size(Some(LogicalSize::new(min.width, min.height))) {
        eprintln!("meloet: cannot set the smallest window size: {e}");
    }
}

/// The shell learns the zoom the way the screen does: it listens to the patches first, then
/// reads the snapshot. The core stays the only owner of the zoom.
fn follow_scale(core: &Core, window: WebviewWindow) {
    let listener = window.clone();
    // The workspace section also changes at every resize; only a new zoom is passed on.
    let last = AtomicU16::new(0);
    core.subscribe(
        Box::new(move |message| {
            if let ServerMessage::Patch(patch) = message {
                for change in &patch.changes {
                    if let Change::Workspace(workspace) = change
                        && last.swap(workspace.scale, Relaxed) != workspace.scale
                    {
                        set_min_size(&listener, workspace.scale);
                    }
                }
            }
            true
        }),
        // No telemetry wanted: this sink is dropped at the first reading.
        Box::new(|_| false),
    );
    set_min_size(&window, core.snapshot().state.workspace.scale);
}

fn main() {
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
            measure_report
        ])
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                follow_scale(&window_side, window.clone());
                devhooks::apply_window_size(&window);
            }
            Ok(())
        })
        .on_page_load(devhooks::on_page_load)
        .build(tauri::generate_context!())
        .expect("failed to run meloet")
        .run(move |_app, event| {
            if let RunEvent::Exit = event {
                core.stop();
            }
        });
}

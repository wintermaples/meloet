//! The Tauri shell: hands commands and Channels across, and owns no state.

mod clock;
mod core_handle;
mod devhooks;
mod stub;

use std::sync::Arc;

use meloet_protocol::{Command, HelloReply, PROTOCOL_VERSION, SendReply, ServerMessage, Snapshot};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{Manager, State};

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

fn main() {
    let core: Core = stub::StubCore::start();
    tauri::Builder::default()
        .manage(core)
        .invoke_handler(tauri::generate_handler![
            hello, subscribe, snapshot, send, clock
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                devhooks::apply_window_size(&window);
            }
            Ok(())
        })
        .on_page_load(devhooks::on_page_load)
        .run(tauri::generate_context!())
        .expect("failed to run meloet");
}

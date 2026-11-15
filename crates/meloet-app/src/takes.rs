//! The notes of takes and of the recording going on (docs/architecture.md 3-1, 3-4).

use std::sync::Arc;

use meloet_protocol::LiveNotesSnapshot;
use tauri::State;
use tauri::ipc::Response;

use crate::core_handle::CoreHandle;

/// The take binary. A take that is not in the song now is an error; the screen asks again
/// when it shows up.
#[tauri::command]
pub fn take_notes(id: u32, core: State<'_, Arc<dyn CoreHandle>>) -> Result<Response, String> {
    core.take_notes(id)
        .map(Response::new)
        .ok_or_else(|| format!("no take {id}"))
}

#[tauri::command]
pub fn live_notes(core: State<'_, Arc<dyn CoreHandle>>) -> LiveNotesSnapshot {
    core.live_notes()
}

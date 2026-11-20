//! What the screen fetches by name: the notes of takes and of the recording going on, the points
//! of automation lanes, the rows of the history band, and the parameters of a plugin
//! (docs/architecture.md 3-1, 3-4, 21-15).

use std::sync::Arc;

use meloet_protocol::{HistoryRow, LiveNotesSnapshot, ParamTable};
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

/// The point binary of a lane. A lane that is not in the song now is an error, as for takes.
#[tauri::command]
pub fn lane(lane_id: u32, core: State<'_, Arc<dyn CoreHandle>>) -> Result<Response, String> {
    core.lane(lane_id)
        .map(Response::new)
        .ok_or_else(|| format!("no lane {lane_id}"))
}

/// The peaks binary of an audio file (docs/architecture.md 22-17). A file that is not in
/// the song now, or whose peaks are not made yet, is an error; the screen asks again later.
#[tauri::command]
pub fn peaks(
    audio: u32,
    level: u8,
    first: u32,
    count: u32,
    core: State<'_, Arc<dyn CoreHandle>>,
) -> Result<Response, String> {
    core.peaks(audio, level, first, count)
        .map(Response::new)
        .ok_or_else(|| format!("no peaks of audio {audio}"))
}

#[tauri::command]
pub fn history_rows(from: u32, to: u32, core: State<'_, Arc<dyn CoreHandle>>) -> Vec<HistoryRow> {
    core.history_rows(from, to)
}

#[tauri::command]
pub fn live_notes(core: State<'_, Arc<dyn CoreHandle>>) -> LiveNotesSnapshot {
    core.live_notes()
}

/// `None`: the panel shows another table by now; the screen fetches the one the state names.
#[tauri::command]
pub fn plugin_params(table: u32, core: State<'_, Arc<dyn CoreHandle>>) -> Option<ParamTable> {
    core.plugin_params(table)
}

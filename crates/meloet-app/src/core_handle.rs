//! The line between the shell and whoever owns the state. The shell sees only this.

use std::path::PathBuf;
use std::time::Duration;

use meloet_core::{MessageSink, ShellSink, TelemetrySink};
use meloet_protocol::{Command, HistoryRow, LiveNotesSnapshot, ParamTable, Snapshot};

pub trait CoreHandle: Send + Sync + 'static {
    /// The whole state and its rev.
    fn snapshot(&self) -> Snapshot;

    /// Applies an intent. Returns the rev after it (unchanged when nothing changed).
    fn send(&self, command: Command) -> u64;

    /// What the screen the window is on can give to the inside of the window, in logical px.
    fn set_screen_space(&self, width: f32, height: f32);

    /// From this call on, every patch reaches `messages` in rev order: a snapshot taken after
    /// `subscribe` returns misses nothing.
    fn subscribe(&self, messages: MessageSink, telemetry: TelemetrySink);

    /// The take binary of a take in the song now.
    fn take_notes(&self, id: u32) -> Option<Vec<u8>>;

    /// The point binary of an automation lane in the song now.
    fn lane(&self, lane_id: u32) -> Option<Vec<u8>>;

    /// The peaks binary of an audio file of the song now.
    fn peaks(&self, audio: u32, level: u8, first: u32, count: u32) -> Option<Vec<u8>>;

    /// Rows `from..to` of the history band.
    fn history_rows(&self, from: u32, to: u32) -> Vec<HistoryRow>;

    /// Every note of the recording going on.
    fn live_notes(&self) -> LiveNotesSnapshot;

    /// The parameters of the plugin in the parameters panel, when `table` still names them.
    fn plugin_params(&self, table: u32) -> Option<ParamTable>;

    /// Where the core's requests to the shell go (the PC's windows, the end).
    fn set_shell(&self, shell: ShellSink);

    /// The window's close button. Does not wait.
    fn close_requested(&self);

    /// The answer of the PC's open window. Does not wait.
    fn song_file_picked(&self, path: Option<PathBuf>);

    /// A song file the PC handed over (a double click, `meloet <file>`). Does not wait.
    fn open_given(&self, path: PathBuf);

    /// The answer of the PC's folder window. Does not wait.
    fn folder_picked(&self, path: Option<PathBuf>);

    /// The answer of the PC's folder window for a place to look for plugins in. Does not wait.
    fn plugin_place_picked(&self, path: Option<PathBuf>);

    /// The answer of the PC's save window. Does not wait.
    fn save_as_picked(&self, path: Option<PathBuf>);

    /// The answer of the PC's open window for an SF2 file. Does not wait.
    fn sf2_picked(&self, path: Option<PathBuf>);

    /// The answer of the PC's window for where an export goes. Does not wait.
    fn export_place_picked(&self, path: Option<PathBuf>);

    /// The answer of the PC's open window for audio files: none when it was put away. Does
    /// not wait.
    fn audio_picked(&self, paths: Vec<PathBuf>);

    /// Files are held over the window. Does not wait.
    fn files_over(&self, paths: Vec<PathBuf>);

    /// The files held over the window left it, or the drag was given up. Does not wait.
    fn files_left(&self);

    /// The files held over the window were let go of there. Does not wait.
    fn files_dropped(&self, paths: Vec<PathBuf>);

    /// Where the pointer is while files are held over the window, in px of the page. Does
    /// not wait.
    fn files_at(&self, x: i32, y: i32);

    /// The PC is ending: waits up to `timeout` for the recovery file. Returns whether it was
    /// written in time (or there was nothing to write).
    fn pc_ending(&self, timeout: Duration) -> bool;
}

impl CoreHandle for meloet_core::Core {
    fn snapshot(&self) -> Snapshot {
        meloet_core::Core::snapshot(self)
    }

    fn send(&self, command: Command) -> u64 {
        meloet_core::Core::send(self, command)
    }

    fn set_screen_space(&self, width: f32, height: f32) {
        meloet_core::Core::set_screen_space(self, width, height);
    }

    fn subscribe(&self, messages: MessageSink, telemetry: TelemetrySink) {
        meloet_core::Core::subscribe(self, messages, telemetry);
    }

    fn take_notes(&self, id: u32) -> Option<Vec<u8>> {
        meloet_core::Core::take_notes(self, id)
    }

    fn lane(&self, lane_id: u32) -> Option<Vec<u8>> {
        meloet_core::Core::lane(self, lane_id)
    }

    fn peaks(&self, audio: u32, level: u8, first: u32, count: u32) -> Option<Vec<u8>> {
        meloet_core::Core::peaks(self, audio, level, first, count)
    }

    fn history_rows(&self, from: u32, to: u32) -> Vec<HistoryRow> {
        meloet_core::Core::history_rows(self, from, to)
    }

    fn live_notes(&self) -> LiveNotesSnapshot {
        meloet_core::Core::live_notes(self)
    }

    fn plugin_params(&self, table: u32) -> Option<ParamTable> {
        meloet_core::Core::plugin_params(self, table)
    }

    fn set_shell(&self, shell: ShellSink) {
        meloet_core::Core::set_shell(self, shell);
    }

    fn close_requested(&self) {
        meloet_core::Core::close_requested(self);
    }

    fn song_file_picked(&self, path: Option<PathBuf>) {
        meloet_core::Core::song_file_picked(self, path);
    }

    fn open_given(&self, path: PathBuf) {
        meloet_core::Core::open_given(self, path);
    }

    fn folder_picked(&self, path: Option<PathBuf>) {
        meloet_core::Core::folder_picked(self, path);
    }

    fn plugin_place_picked(&self, path: Option<PathBuf>) {
        meloet_core::Core::plugin_place_picked(self, path);
    }

    fn save_as_picked(&self, path: Option<PathBuf>) {
        meloet_core::Core::save_as_picked(self, path);
    }

    fn sf2_picked(&self, path: Option<PathBuf>) {
        meloet_core::Core::sf2_picked(self, path);
    }

    fn export_place_picked(&self, path: Option<PathBuf>) {
        meloet_core::Core::export_place_picked(self, path);
    }

    fn audio_picked(&self, paths: Vec<PathBuf>) {
        meloet_core::Core::audio_picked(self, paths);
    }

    fn files_over(&self, paths: Vec<PathBuf>) {
        meloet_core::Core::files_over(self, paths);
    }

    fn files_left(&self) {
        meloet_core::Core::files_left(self);
    }

    fn files_dropped(&self, paths: Vec<PathBuf>) {
        meloet_core::Core::files_dropped(self, paths);
    }

    fn files_at(&self, x: i32, y: i32) {
        meloet_core::Core::files_at(self, x, y);
    }

    fn pc_ending(&self, timeout: Duration) -> bool {
        meloet_core::Core::pc_ending(self, timeout)
    }
}

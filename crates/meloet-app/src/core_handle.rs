//! The line between the shell and whoever owns the state. The shell sees only this.

use std::path::PathBuf;

use meloet_core::{MessageSink, ShellSink, TelemetrySink};
use meloet_protocol::{Command, LiveNotesSnapshot, Snapshot};

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

    /// Every note of the recording going on.
    fn live_notes(&self) -> LiveNotesSnapshot;

    /// Where the core's requests to the shell go (the PC's windows, the end).
    fn set_shell(&self, shell: ShellSink);

    /// The window's close button. Does not wait.
    fn close_requested(&self);

    /// The answer of the PC's open window. Does not wait.
    fn song_file_picked(&self, path: Option<PathBuf>);

    /// The answer of the PC's folder window. Does not wait.
    fn folder_picked(&self, path: Option<PathBuf>);
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

    fn live_notes(&self) -> LiveNotesSnapshot {
        meloet_core::Core::live_notes(self)
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

    fn folder_picked(&self, path: Option<PathBuf>) {
        meloet_core::Core::folder_picked(self, path);
    }
}

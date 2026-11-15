//! The line between the shell and whoever owns the state. The shell sees only this.

use meloet_core::{MessageSink, TelemetrySink};
use meloet_protocol::{Command, Snapshot};

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
}

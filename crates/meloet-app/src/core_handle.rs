//! The line between the shell and whoever owns the state. The shell sees only this.

use meloet_protocol::{Command, ServerMessage, Snapshot};

/// Returns false when the receiver is gone; the owner then drops the sink.
pub type MessageSink = Box<dyn Fn(ServerMessage) -> bool + Send>;
/// Gets one telemetry message (version 1 binary). Returns false when the receiver is gone.
pub type TelemetrySink = Box<dyn Fn(&[u8]) -> bool + Send>;

pub trait CoreHandle: Send + Sync + 'static {
    /// The whole state and its rev.
    fn snapshot(&self) -> Snapshot;

    /// Applies an intent. Returns the rev after it (unchanged when nothing changed).
    fn send(&self, command: Command) -> u64;

    /// From this call on, every patch reaches `messages` in rev order: a snapshot taken after
    /// `subscribe` returns misses nothing.
    fn subscribe(&self, messages: MessageSink, telemetry: TelemetrySink);
}

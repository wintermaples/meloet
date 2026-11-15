//! The state the screens see, its rev, and the sinks that carry it.

use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::{Mutex, MutexGuard};

use meloet_protocol::{AppState, Event, LiveNotes, Patch, ServerMessage, Snapshot};

/// Returns false when the receiver is gone; the sink is then dropped.
pub type MessageSink = Box<dyn Fn(ServerMessage) -> bool + Send>;
/// Gets one telemetry message (the binary of docs/architecture.md 3-3). Returns false when the
/// receiver is gone.
pub type TelemetrySink = Box<dyn Fn(&[u8]) -> bool + Send>;

pub(crate) struct Hub {
    /// One lock for the state, the rev and the message sinks: a patch goes out under the same
    /// lock that changes the state, so a snapshot taken after `subscribe` returned misses
    /// nothing, and messages leave in the order they were made.
    messages: Mutex<Messages>,
    telemetry: Mutex<Vec<TelemetrySink>>,
    /// A new telemetry sink has not had the current readings yet.
    telemetry_fresh: AtomicBool,
}

struct Messages {
    rev: u64,
    state: AppState,
    sinks: Vec<MessageSink>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Hub {
    pub fn new(state: AppState) -> Hub {
        Hub {
            messages: Mutex::new(Messages {
                rev: 1,
                state,
                sinks: Vec::new(),
            }),
            telemetry: Mutex::new(Vec::new()),
            telemetry_fresh: AtomicBool::new(false),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let messages = lock(&self.messages);
        Snapshot {
            rev: messages.rev,
            state: messages.state.clone(),
        }
    }

    pub fn rev(&self) -> u64 {
        lock(&self.messages).rev
    }

    pub fn subscribe(&self, messages: MessageSink, telemetry: TelemetrySink) {
        lock(&self.messages).sinks.push(messages);
        lock(&self.telemetry).push(telemetry);
        self.telemetry_fresh.store(true, Relaxed);
    }

    /// Makes `state` the current state. When a section differs, the rev goes up by one and one
    /// patch goes to every sink. Returns the rev after.
    pub fn publish(&self, state: AppState) -> u64 {
        let mut messages = lock(&self.messages);
        let changes = AppState::changes_from(&messages.state, &state);
        if changes.is_empty() {
            return messages.rev;
        }
        messages.rev += 1;
        messages.state = state;
        let patch = Patch {
            rev: messages.rev,
            changes,
        };
        messages
            .sinks
            .retain(|sink| sink(ServerMessage::Patch(patch.clone())));
        messages.rev
    }

    pub fn event(&self, event: Event) {
        lock(&self.messages)
            .sinks
            .retain(|sink| sink(ServerMessage::Event(event)));
    }

    pub fn live_notes(&self, notes: LiveNotes) {
        lock(&self.messages)
            .sinks
            .retain(|sink| sink(ServerMessage::LiveNotes(notes.clone())));
    }

    /// True once after each `subscribe`.
    pub fn take_telemetry_fresh(&self) -> bool {
        self.telemetry_fresh.swap(false, Relaxed)
    }

    pub fn telemetry(&self, bytes: &[u8]) {
        lock(&self.telemetry).retain(|sink| sink(bytes));
    }
}

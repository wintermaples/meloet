//! Automation: what a lane moves (docs/architecture.md 21-5). The lanes themselves, their
//! points and the steps on them come with round 6 step 2.

use crate::mixer::{Bus, InsertId};

/// What a lane of a track moves. The returns and the master have no lanes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AutoTarget {
    /// The fader.
    Volume,
    Pan,
    Send(Bus),
    Mute,
    /// A parameter of the plugin that is the track's instrument, by the plugin's own number.
    Instrument {
        param: u32,
    },
    /// A parameter of what sits in an insert slot of the track: its place in the table for an
    /// effect that comes with meloet, the plugin's own number for a plugin.
    Insert {
        insert: InsertId,
        param: u32,
    },
}

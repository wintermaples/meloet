//! What meloet and its plugin host process (`meloet-plughost`) share, whatever the plugin
//! format (docs/architecture.md 20).
//!
//! - [`key`]: the exact key of a plugin and the key the shelf merges rows by (20-5).
//! - [`category`]: the kinds of the shelf, from what a plugin says about itself (spec 3, 14-A).
//! - [`scan`]: what a scan found, and the file it is kept in (20-4).
//! - [`ipc`]: the shared memory the sound crosses in, its futex and its rings (20-2).
//! - [`wire`]: the messages of the control socket; everything that is not sound.
//! - [`launch`]: starting the host process with its file descriptors.

pub mod category;
pub mod ipc;
pub mod key;
pub mod launch;
pub mod scan;
pub mod wire;

pub use category::{EffectCategory, InstrumentCategory, PluginKind, PluginRole};
pub use key::{MergeKey, PluginFormat, PluginKey};
pub use scan::{
    BundleRecord, BundleStamp, EditorKind, PluginInfo, ScanCache, StopReason, StoppedEntry,
};

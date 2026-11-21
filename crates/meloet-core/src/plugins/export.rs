//! The plugins of an export: processes of their own, apart from the ones the playing engine
//! sounds through (docs/architecture.md 22-11, 22-20).
//!
//! [`export_seats`] is the one way the export's engine gets its plugins. It starts a
//! process for each kind of plugin (the same rule as [`super::hosts`]: one process for a
//! key, started one at a time), loads every plugin with the state it is given, and hands
//! back what the engine takes in its seats.

use std::path::PathBuf;
use std::sync::Arc;

use meloet_engine::{Effect, Instrument, PluginBlock, PluginWatch};
use meloet_model::PluginPlace;
use meloet_plugin::PluginKey;

use super::hosts::HostOptions;

/// One plugin of the song, as the export is to load it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPlugin {
    pub place: PluginPlace,
    /// For the person, when it does not load or stops.
    pub name: Arc<str>,
    pub key: PluginKey,
    pub bundle: PathBuf,
    /// Everything the plugin says of itself; empty: as it starts.
    pub state: Vec<u8>,
}

/// What the export wants of its plugin processes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportWant {
    /// The song's plugins that sound: not the ones that have stopped or are not found.
    pub plugins: Vec<ExportPlugin>,
    /// The rate of the export's engine, which is the rate the playing engine runs at.
    pub sample_rate: u32,
    /// The export's engine never asks for a longer block.
    pub max_frames: u32,
}

/// The plugins of an export, ready for its engine. Dropping this ends their processes; what
/// was taken out of `instruments` and `effects` must be dropped before.
pub struct ExportSeats {
    /// By where they sit in the song; the engine's builder knows the seat of each place.
    pub instruments: Vec<(PluginPlace, Box<dyn Instrument>)>,
    pub effects: Vec<(PluginPlace, Box<dyn Effect>)>,
    /// A plugin says it cannot run faster than it plays (LV2 `lv2:isLive`): the export's
    /// thread is to pace its blocks by the clock.
    pub realtime: bool,
    /// What tells that a plugin answered no more.
    pub watches: Vec<(PluginPlace, Arc<PluginWatch>)>,
}

impl ExportSeats {
    /// For a song without plugins.
    pub fn none() -> ExportSeats {
        ExportSeats {
            instruments: Vec::new(),
            effects: Vec::new(),
            realtime: false,
            watches: Vec::new(),
        }
    }

    /// A plugin that answered no more. The export's thread looks after every block; with
    /// one the export has failed.
    pub fn stopped(&self) -> Option<PluginPlace> {
        (self.watches.iter()).find_map(|(place, watch)| watch.stopped().then_some(*place))
    }
}

impl std::fmt::Debug for ExportSeats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportSeats")
            .field("instruments", &self.instruments.len())
            .field("effects", &self.effects.len())
            .field("realtime", &self.realtime)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportPluginError {
    /// Songs with plugins cannot be exported yet.
    NotYet,
    /// A process could not be started. The message is for the log.
    Start(String),
    /// This plugin did not load, or did not take its state.
    Load { place: PluginPlace, name: Arc<str> },
}

/// Starts the export's plugin processes and loads every plugin of `want` into them.
///
/// - **Thread**: the export's own thread, before its first block. It waits for the
///   processes to start and for every plugin to load (seconds), so never the core thread
///   and never an audio thread.
/// - **`block`**: the `PluginBlock` of the export's engine (`EngineHandle::plugin_block`),
///   not the playing engine's: the seats read the song position of the export from it.
/// - **Waiting**: a seat handed back waits for its process in every block until the answer
///   is there. The block's deadline is not used and a late block is never left silent, so
///   two runs of the same song give the same file. The only limit is
///   `meloet_plugin::ipc::HANG_MS`: a plugin that holds a block that long has stopped
///   ([`ExportSeats::stopped`]), and the export fails as a whole. The processes run
///   offline (VST3 `kOffline`, LV2 `freeWheeling`) and take no realtime priority.
/// - **States**: `ExportPlugin::state` is filled by the caller, on the core thread, before
///   the export's thread starts: what the playing plugin says its state is now
///   (`Hosts::state`, as saving the song asks it), and for one that does not answer what
///   the `Doc` holds (`PluginRef::state`).
pub fn export_seats(
    options: &HostOptions,
    want: ExportWant,
    block: Arc<PluginBlock>,
) -> Result<ExportSeats, ExportPluginError> {
    let _ = (options, block);
    if want.plugins.is_empty() {
        Ok(ExportSeats::none())
    } else {
        Err(ExportPluginError::NotYet)
    }
}

#[cfg(test)]
mod tests {
    use meloet_model::TrackId;
    use meloet_plugin::ipc::Geometry;

    use super::*;

    fn options() -> HostOptions {
        HostOptions {
            exe: PathBuf::from("/nowhere/meloet-plughost"),
            env: Vec::new(),
            geometry: Geometry {
                max_frames: 1024,
                channels: 2,
                slots: 16,
                to_plugin_events: 256,
                from_plugin_events: 64,
            },
        }
    }

    fn want(plugins: Vec<ExportPlugin>) -> ExportWant {
        ExportWant {
            plugins,
            sample_rate: 48_000,
            max_frames: 1024,
        }
    }

    #[test]
    fn a_song_without_plugins_needs_no_process() {
        let block = Arc::new(PluginBlock::default());
        let seats = export_seats(&options(), want(Vec::new()), block).unwrap();
        assert!(seats.instruments.is_empty() && seats.effects.is_empty());
        assert!(!seats.realtime);
        assert_eq!(seats.stopped(), None);
    }

    #[test]
    fn a_song_with_a_plugin_is_not_exported_yet() {
        let plugin = ExportPlugin {
            place: PluginPlace::Instrument(TrackId(5)),
            name: "Surge XT".into(),
            key: "lv2:https://surge-synthesizer.github.io/lv2/surge-xt"
                .parse()
                .unwrap(),
            bundle: PathBuf::from("/usr/lib/lv2/Surge XT.lv2"),
            state: Vec::new(),
        };
        let block = Arc::new(PluginBlock::default());
        assert_eq!(
            export_seats(&options(), want(vec![plugin]), block).unwrap_err(),
            ExportPluginError::NotYet
        );
    }
}

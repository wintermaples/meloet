//! What the export's engine is told of a song (docs/architecture.md 22-11).
//!
//! The playing engine hears of a song from the rules ([`Play`]): they send it the
//! instruments, the mixer, the notes, the lanes and the vocal clips when a song is opened.
//! The export asks the same rules, with a `Play` of its own that is made for the copy of
//! the `Doc`, opens it as a song, and is thrown away. So there is one place that says how
//! a song becomes what an engine plays, and the export cannot come to sound another way.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use meloet_model::{Doc, PluginPlace, Song, TrackId};
use meloet_protocol::{PluginSlotRef, PluginWindowKind};

use crate::play::{EngineOp, Play, PlugState};
use crate::state::Effect;

/// The plugins this PC has, as the rules are told them (`Catalog::bundles`).
pub type KnownPlugins = Vec<(Arc<str>, PathBuf, PluginWindowKind)>;

/// A plugin of the song that sounds in the export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToldPlugin {
    /// Names it in [`EngineOp::Plug`] of [`Told::plugged`].
    pub ticket: u64,
    pub place: PluginPlace,
    pub name: Arc<str>,
    pub key: Arc<str>,
    pub bundle: PathBuf,
    /// As the `Doc` has it; empty: as the plugin starts.
    pub state: Arc<[u8]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Told {
    /// Everything for an engine that was told nothing yet, in order. A plugin's place is
    /// silent (an instrument) or empty (an insert) after these.
    pub ops: Vec<EngineOp>,
    pub plugins: Vec<ToldPlugin>,
    /// For once the plugins are loaded: each goes into its place, and the lanes that move
    /// their parameters come again.
    pub plugged: Vec<EngineOp>,
}

impl Told {
    /// `solo`: only this track plays (「トラックごと」); a muted one still does not.
    /// `skip`: plugins that have stopped. The export passes them by, as the playing does.
    pub fn of(
        doc: &Doc,
        solo: Option<TrackId>,
        known: &KnownPlugins,
        skip: &[PluginSlotRef],
    ) -> Told {
        let mut play = Play::new(false, 1);
        play.set_known_plugins(known.clone());
        let mut session = Song::new_default().session;
        session.play_target = doc.tracks.first().map(|t| t.id);
        // The export goes once through its range: the loop does not turn.
        session.loop_on = false;
        session.soloed = solo.into_iter().collect::<BTreeSet<TrackId>>();
        let song = Song {
            doc: doc.clone(),
            session,
        };
        // As meloet starts: the engine gets the song the rules begin with, and then the
        // song is opened. What the two songs share (the returns and the master of a new
        // song) is told only the first time.
        let mut ops = play.engine_seats();
        let mut effects = Vec::new();
        play.replace_song(song, false, &mut effects);
        // As the rules do after every input.
        play.sync_silent(&mut effects);
        play.sync_target(&mut effects);
        play.sync_mix(&mut effects);
        play.sync_plugs(&mut effects);
        play.sync_automation(&mut effects);

        let mut loads = Vec::new();
        for effect in effects {
            match effect {
                Effect::Engine(op) => ops.push(op),
                Effect::LoadPlugin {
                    ticket,
                    key,
                    bundle,
                    state,
                    ..
                } => loads.push((ticket, key, bundle, state)),
                _ => {}
            }
        }
        let mut after = Vec::new();
        for (ticket, ..) in &loads {
            play.plugin_loaded(*ticket, &mut after);
        }
        let places = play.plugin_tickets();
        let skipped: Vec<PluginPlace> = skip.iter().map(|s| Play::slot_place(*s)).collect();
        let plugins: Vec<ToldPlugin> = (loads.into_iter())
            .filter_map(|(ticket, key, bundle, state)| {
                let place = places.iter().find(|(_, t)| *t == ticket)?.0;
                let name = doc.plugin(place).map(|p| p.name.clone())?;
                (!skipped.contains(&place)).then_some(ToldPlugin {
                    ticket,
                    place,
                    name,
                    key,
                    bundle,
                    state: match state {
                        PlugState::Bytes(state) => state,
                        PlugState::Fresh | PlugState::Of(_) => Arc::from([]),
                    },
                })
            })
            .collect();
        let plugged = (after.into_iter())
            .filter_map(|effect| match effect {
                Effect::Engine(EngineOp::Plug { ticket, .. })
                    if !plugins.iter().any(|p| p.ticket == ticket) =>
                {
                    None
                }
                Effect::Engine(op) => Some(op),
                _ => None,
            })
            .collect();
        Told {
            ops,
            plugins,
            plugged,
        }
    }

    /// The seats that are silent, as the last word of [`Told::ops`] on it.
    pub fn silent(&self) -> Vec<usize> {
        (self.ops.iter().rev())
            .find_map(|op| match op {
                EngineOp::SilentTracks { tracks } => Some(tracks.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }
}

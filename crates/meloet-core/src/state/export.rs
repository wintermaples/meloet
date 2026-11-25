//! The export as the rest of the rules see it: what [`crate::export::Exporting`] is told of
//! the song, and where its asks go.

use meloet_protocol::{Command, ExportStoppedPlugin, ExportView, TrackRef};

use super::{Effect, Model};
use crate::export::{ExportDirs, ExportNews, Scene};

impl Model {
    pub fn with_export_dirs(mut self, dirs: ExportDirs) -> Model {
        let artist = self.export.artist().to_owned();
        self.export = crate::export::Exporting::new(dirs).with_artist(artist);
        self
    }

    pub(super) fn export_scene(&self) -> Scene<'_> {
        let doc = &self.play.song().doc;
        let soloed = &self.play.song().session.soloed;
        Scene {
            doc,
            serial: self.play.serial(),
            song_path: self.files.path.as_deref(),
            sample_rate: (self.open.as_ref()).map_or(self.sound.sample_rate, |o| o.sample_rate),
            busy: self.play.busy(),
            stopped_plugins: (self.play.stopped_plugins().into_iter())
                .map(|(slot, name)| ExportStoppedPlugin { name, slot })
                .collect(),
            soloed: (doc.tracks.iter().enumerate())
                .filter(|(_, t)| soloed.contains(&t.id))
                .map(|(i, t)| TrackRef {
                    number: i as u32 + 1,
                    name: t.name.clone(),
                })
                .collect(),
            selected: self.play.song().session.play_target,
            plugin_kits: self.play.plugin_kits(),
        }
    }

    /// The plugins this PC has, as the rules of a song are told them: an export tells
    /// its own copy of the rules the same.
    pub(crate) fn known_plugins(&self) -> crate::export::told::KnownPlugins {
        self.catalog.bundles()
    }

    pub(super) fn export_view(&self) -> ExportView {
        self.export.view(&self.export_scene())
    }

    pub(super) fn export_command(&mut self, command: &Command, effects: &mut Vec<Effect>) {
        if let (Command::ShowExported, Some(path)) = (command, self.export.exported()) {
            effects.push(Effect::ShowFile(path.to_path_buf()));
            return;
        }
        if let (Command::PlayExported, Some(path)) = (command, self.export.played()) {
            effects.push(Effect::OpenFile(path.to_path_buf()));
            return;
        }
        // One job of the second engine at a time (round 12, A7).
        if matches!(command, Command::OpenExport) && self.bouncing() {
            self.notice = Some(meloet_protocol::StatusMessage::ExportWaitsForBounce);
            return;
        }
        let mut export = std::mem::take(&mut self.export);
        let mut asks = Vec::new();
        let artist = export.artist().to_owned();
        export.command(command, &self.export_scene(), &mut asks);
        self.export = export;
        effects.extend(asks.into_iter().map(Effect::Export));
        // F2 「作者」 is kept with the PC's settings.
        if self.export.artist() != artist {
            effects.push(Effect::SaveSettings(self.settings()));
        }
    }

    pub(super) fn export_news(&mut self, news: ExportNews) {
        if self.bounce_export_news(&news) {
            return;
        }
        if let Some(notice) = self.export.news(news) {
            self.notice = Some(notice);
        }
    }

    /// After every input.
    pub(super) fn settle_export(&mut self, effects: &mut Vec<Effect>) {
        effects.extend(self.export.take_left().map(Effect::Export));
        if !self.export.dialog_open() {
            return;
        }
        let mut export = std::mem::take(&mut self.export);
        let mut asks = Vec::new();
        export.settle(&self.export_scene(), &mut asks);
        self.export = export;
        effects.extend(asks.into_iter().map(Effect::Export));
    }
}

//! The export as the rest of the rules see it: what [`crate::export::Exporting`] is told of
//! the song, and where its asks go.

use meloet_protocol::{Command, ExportStoppedPlugin, ExportView};

use super::{Effect, Model};
use crate::export::{ExportDirs, ExportNews, Scene};

impl Model {
    pub fn with_export_dirs(mut self, dirs: ExportDirs) -> Model {
        self.export = crate::export::Exporting::new(dirs);
        self
    }

    fn export_scene(&self) -> Scene<'_> {
        Scene {
            doc: &self.play.song().doc,
            serial: self.play.serial(),
            song_path: self.files.path.as_deref(),
            sample_rate: (self.open.as_ref()).map_or(self.sound.sample_rate, |o| o.sample_rate),
            busy: self.play.busy(),
            stopped_plugins: (self.play.stopped_plugins().into_iter())
                .map(|(slot, name)| ExportStoppedPlugin { name, slot })
                .collect(),
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
        let mut export = std::mem::take(&mut self.export);
        let mut asks = Vec::new();
        export.command(command, &self.export_scene(), &mut asks);
        self.export = export;
        effects.extend(asks.into_iter().map(Effect::Export));
    }

    pub(super) fn export_news(&mut self, news: ExportNews) {
        self.export.news(news);
    }

    /// After every input.
    pub(super) fn settle_export(&mut self, effects: &mut Vec<Effect>) {
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

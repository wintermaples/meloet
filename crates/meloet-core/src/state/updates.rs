//! 「新しい版をたしかめる」 (round 12, D1): when the look goes out, what it found, and the
//! status line's 「新しい版 X.Y.Z があります」.

use super::{Effect, Model, StatusMessage, UpdatesView};

#[derive(Debug, Default)]
pub(super) struct UpdatesSide {
    screen_up: bool,
    /// The look went out in this run.
    asked: bool,
    /// Newer than this meloet ("0.4.0"). Kept for the run, shown only while
    /// `Model::check_updates` is on.
    new_version: Option<String>,
    /// The status line said it while the song was stopped.
    said: bool,
    /// 「あとで」, 「ページをひらく」, or the song moved after it was said: not said again in
    /// this run.
    put_away: bool,
}

impl Model {
    pub(super) fn updates_screen_up(&mut self, effects: &mut Vec<Effect>) {
        self.updates.screen_up = true;
        self.ask_for_updates(effects);
    }

    fn ask_for_updates(&mut self, effects: &mut Vec<Effect>) {
        let u = &mut self.updates;
        if self.check_updates && u.screen_up && !u.asked {
            u.asked = true;
            effects.push(Effect::CheckUpdates);
        }
    }

    pub(super) fn set_check_updates(&mut self, on: bool, effects: &mut Vec<Effect>) {
        if self.check_updates == on {
            return;
        }
        self.check_updates = on;
        effects.push(Effect::SaveSettings(self.settings()));
        if on {
            self.ask_for_updates(effects);
        }
    }

    /// The look found `version`, newer than this meloet.
    pub(super) fn new_version_found(&mut self, version: String) {
        self.updates.new_version = Some(version);
    }

    /// What the look found, while the switch is on.
    fn shown_version(&self) -> Option<&String> {
        self.updates
            .new_version
            .as_ref()
            .filter(|_| self.check_updates)
    }

    pub(super) fn open_release_page(&mut self, effects: &mut Vec<Effect>) {
        if let Some(version) = self.shown_version() {
            effects.push(Effect::OpenUrl(crate::updates::release_page(version)));
            self.updates.put_away = true;
        }
    }

    pub(super) fn dismiss_new_version(&mut self) {
        self.updates.put_away = true;
    }

    /// After every input: once the status line said it, the song moving puts it away.
    pub(super) fn settle_updates(&mut self) {
        if self.shown_version().is_none() || self.updates.put_away {
            return;
        }
        if self.play.is_moving() {
            self.updates.put_away = self.updates.said;
        } else if !self.updates.said {
            let status = self.status(self.chosen_state());
            self.updates.said = matches!(status, Some(StatusMessage::NewVersion { .. }));
        }
    }

    /// Only while stopped: never in the count-in, while recording or playing.
    pub(super) fn new_version_status(&self) -> Option<StatusMessage> {
        if self.updates.put_away || self.play.is_moving() {
            return None;
        }
        let version = self.shown_version()?.clone();
        Some(StatusMessage::NewVersion { version })
    }

    pub(super) fn updates_view(&self) -> UpdatesView {
        UpdatesView {
            check: self.check_updates,
            new_version: self.shown_version().cloned(),
        }
    }
}
#[cfg(test)]
mod tests;

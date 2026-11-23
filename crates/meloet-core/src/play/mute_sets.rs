//! Mute sets (`meloet_model::mute_sets`): the screen's intents of the mixer band's menu and
//! the AI link's asks (`crate::ai`). Saving, recalling, deleting and renaming one are steps;
//! the solo a recall brings is the session's, as with S, and no step.

use meloet_model::mute_sets::{self, MuteSetError};
use meloet_protocol::MuteSetView;

use super::Play;
use super::ai::AiWrote;
use crate::ai::AiRefusal;
use crate::state::Effect;

fn refusal(e: MuteSetError, name: &str) -> AiRefusal {
    match e {
        MuteSetError::NoSuchSet => AiRefusal::NoSuchMuteSet(name.trim().to_owned()),
        MuteSetError::Taken => AiRefusal::MuteSetExists(name.trim().to_owned()),
        MuteSetError::TooMany => AiRefusal::TooManyMuteSets,
        MuteSetError::BadName => AiRefusal::Cannot("that is no name for a mute set".into()),
        MuteSetError::Unchanged => AiRefusal::Unchanged,
    }
}

/// "As it is already" is no refusal.
fn done(result: Result<(), MuteSetError>) -> Result<(), MuteSetError> {
    match result {
        Err(MuteSetError::Unchanged) => Ok(()),
        other => other,
    }
}

impl Play {
    /// `SongView::mute_sets`.
    pub(crate) fn mute_sets_view(&self) -> Vec<MuteSetView> {
        let soloed = &self.song.session.soloed;
        (self.doc().mute_sets.iter())
            .map(|set| MuteSetView {
                name: set.name.clone(),
                as_now: set.is_now(self.doc(), soloed),
            })
            .collect()
    }

    fn keep_mute_set(
        &mut self,
        name: &str,
        overwrite: bool,
        effects: &mut Vec<Effect>,
    ) -> Result<(), MuteSetError> {
        let soloed = &self.song.session.soloed;
        let e = mute_sets::save_mute_set(self.doc(), soloed, name, overwrite)?;
        self.apply(e, effects);
        Ok(())
    }

    /// The mutes as one step when any changes; the solo as the set has it in any case.
    fn bring_mute_set(
        &mut self,
        name: &str,
        effects: &mut Vec<Effect>,
    ) -> Result<(), MuteSetError> {
        let soloed = &self.song.session.soloed;
        let recall = mute_sets::recall_mute_set(self.doc(), soloed, name)?;
        if let Some(e) = recall.edit {
            self.apply(e, effects);
        }
        self.song.session.soloed = recall.soloed;
        Ok(())
    }

    fn drop_mute_set(&mut self, name: &str, effects: &mut Vec<Effect>) -> Result<(), MuteSetError> {
        let e = mute_sets::delete_mute_set(self.doc(), name)?;
        self.apply(e, effects);
        Ok(())
    }

    fn name_mute_set(
        &mut self,
        name: &str,
        to: &str,
        effects: &mut Vec<Effect>,
    ) -> Result<(), MuteSetError> {
        let e = mute_sets::rename_mute_set(self.doc(), name, to)?;
        self.apply(e, effects);
        Ok(())
    }

    /// `Command::SaveMuteSet`. Like every screen intent here, nothing while counting in or
    /// recording, and nothing (with no status line) when it does not fit.
    pub fn save_mute_set(&mut self, name: &str, overwrite: bool, effects: &mut Vec<Effect>) {
        if !self.busy() {
            let _ = self.keep_mute_set(name, overwrite, effects);
        }
    }

    /// `Command::RecallMuteSet`.
    pub fn recall_mute_set(&mut self, name: &str, effects: &mut Vec<Effect>) {
        if !self.busy() {
            let _ = self.bring_mute_set(name, effects);
        }
    }

    /// `Command::DeleteMuteSet`.
    pub fn delete_mute_set(&mut self, name: &str, effects: &mut Vec<Effect>) {
        if !self.busy() {
            let _ = self.drop_mute_set(name, effects);
        }
    }

    /// `Command::RenameMuteSet`.
    pub fn rename_mute_set(&mut self, name: &str, to: &str, effects: &mut Vec<Effect>) {
        if !self.busy() {
            let _ = self.name_mute_set(name, to, effects);
        }
    }

    /// A set that would be as it is already makes no step and is no refusal.
    pub(crate) fn ai_save_mute_set(
        &mut self,
        name: &str,
        overwrite: bool,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        done(self.keep_mute_set(name, overwrite, effects)).map_err(|e| refusal(e, name))?;
        Ok(AiWrote::default())
    }

    pub(crate) fn ai_recall_mute_set(
        &mut self,
        name: &str,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        self.bring_mute_set(name, effects)
            .map_err(|e| refusal(e, name))?;
        Ok(AiWrote::default())
    }

    pub(crate) fn ai_delete_mute_set(
        &mut self,
        name: &str,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        self.drop_mute_set(name, effects)
            .map_err(|e| refusal(e, name))?;
        Ok(AiWrote::default())
    }

    /// The name it has already makes no step and is no refusal. A name taken is refused as
    /// for a save.
    pub(crate) fn ai_rename_mute_set(
        &mut self,
        name: &str,
        to: &str,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        done(self.name_mute_set(name, to, effects)).map_err(|e| match e {
            MuteSetError::Taken => refusal(e, to),
            e => refusal(e, name),
        })?;
        Ok(AiWrote::default())
    }
}

#[cfg(test)]
mod tests;

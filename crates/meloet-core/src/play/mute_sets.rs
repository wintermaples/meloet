//! Mute sets for the AI link (`crate::ai`, `meloet_model::mute_sets`). Saving, recalling and
//! deleting one are steps; the solo a recall brings is the session's, as with S, and no step.

use meloet_model::mute_sets::{self, MuteSetError};

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

impl Play {
    /// A set that would be as it is already makes no step and is no refusal.
    pub(crate) fn ai_save_mute_set(
        &mut self,
        name: &str,
        overwrite: bool,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let soloed = &self.song.session.soloed;
        match mute_sets::save_mute_set(self.doc(), soloed, name, overwrite) {
            Ok(e) => self.apply(e, effects),
            Err(MuteSetError::Unchanged) => {}
            Err(e) => return Err(refusal(e, name)),
        }
        Ok(AiWrote::default())
    }

    /// The mutes as one step when any changes; the solo as the set has it in any case.
    pub(crate) fn ai_recall_mute_set(
        &mut self,
        name: &str,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let soloed = &self.song.session.soloed;
        let recall =
            mute_sets::recall_mute_set(self.doc(), soloed, name).map_err(|e| refusal(e, name))?;
        if let Some(e) = recall.edit {
            self.apply(e, effects);
        }
        self.song.session.soloed = recall.soloed;
        Ok(AiWrote::default())
    }

    pub(crate) fn ai_delete_mute_set(
        &mut self,
        name: &str,
        effects: &mut Vec<Effect>,
    ) -> Result<AiWrote, AiRefusal> {
        let e = mute_sets::delete_mute_set(self.doc(), name).map_err(|e| refusal(e, name))?;
        self.apply(e, effects);
        Ok(AiWrote::default())
    }
}

#[cfg(test)]
mod tests;

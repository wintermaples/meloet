//! Mute sets: the mutes of the tracks and the returns, and which tracks are soloed, kept in
//! the song under a name, to switch between combinations of tracks in one call.

use meloet_core::ai::{AiAsk, AiSong};
use meloet_model::mute_sets::{self, MuteSet};
use meloet_model::{Bus, TrackId};
use rmcp::model::CallToolResult;
use rmcp::{
    ErrorData as McpError, handler::server::wrapper::Parameters, schemars, tool, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Tools, done, failed, name_ok};

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct SaveMuteSetArgs {
    /// The set's name, 1 to 64 characters, like "爽快セット".
    #[schemars(length(min = 1, max = 64))]
    pub name: String,
    /// Write over a set of that name (default false: that is an error).
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct MuteSetArgs {
    /// The set's name, as list_mute_sets gives it.
    #[schemars(length(min = 1, max = 64))]
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct ListMuteSetsArgs {
    /// Only the set of this name. Leave out for every set.
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub name: Option<String>,
}

/// The number (from 1) the track of `id` has now.
fn number(read: &AiSong, id: TrackId) -> Option<u32> {
    let at = read.song.doc.tracks.iter().position(|t| t.id == id)?;
    Some(at as u32 + 1)
}

fn numbers(read: &AiSong, ids: &[TrackId]) -> Vec<u32> {
    let mut numbers: Vec<u32> = ids.iter().filter_map(|&id| number(read, id)).collect();
    numbers.sort_unstable();
    numbers
}

fn returns(muted: [bool; 2]) -> Vec<&'static str> {
    (Bus::ALL.into_iter())
        .filter(|b| muted[b.index()])
        .map(|b| match b {
            Bus::A => "return_a",
            Bus::B => "return_b",
        })
        .collect()
}

/// A set as the song is now: its tracks by number, tracks deleted since by id, tracks added
/// since, and whether a recall would change anything.
fn set_value(read: &AiSong, set: &MuteSet) -> Value {
    let soloed = &read.song.session.soloed;
    let recall = mute_sets::recall_mute_set(&read.song.doc, soloed, &set.name).ok();
    json!({
        "name": set.name,
        "tracks": numbers(read, &set.tracks),
        "muted": numbers(read, &set.muted),
        "soloed": numbers(read, &set.soloed),
        "muted_returns": returns(set.muted_returns),
        "deleted_track_ids": recall.as_ref().map(|r| r.gone.iter().map(|t| t.0).collect::<Vec<_>>()),
        "not_in_set": recall.as_ref().map(|r| numbers(read, &r.not_in_set)),
        "as_now": recall.is_some_and(|r| r.edit.is_none() && &r.soloed == soloed),
    })
}

#[tool_router(router = sets_router, vis = "pub(super)")]
impl Tools {
    /// Saves the mute of every track and of the two returns, and which tracks are soloed, as
    /// a mute set of the song (saved with it), as one undo step. A name that is there is
    /// written over only with `overwrite: true`; saving what the set holds already changes
    /// nothing (`changed: false`). Recall it with recall_mute_set.
    #[tool(annotations(
        title = "Save mute set",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn save_mute_set(
        &self,
        Parameters(a): Parameters<SaveMuteSetArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let name = check!(name_ok(&a.name));
        let ask = AiAsk::SaveMuteSet {
            name: name.clone(),
            overwrite: a.overwrite,
        };
        let d = check!(self.ask(ask).await);
        let read = check!(self.read().await);
        let set = read.song.doc.mute_set(&name).map(|s| set_value(&read, s));
        let changed = d.step.is_some();
        let value = json!({
            "changed": changed,
            "set": set,
            "step": d.step,
            "undo_steps": d.undo_count,
        });
        let said = match changed {
            true => format!("Saved the mute set \"{name}\"."),
            false => format!("The mute set \"{name}\" holds this already; nothing changed."),
        };
        Ok(done(format!("{said}\n{value}"), value))
    }

    /// The song's mute sets: for each, the tracks it mutes and solos (numbers from 1, as the
    /// tracks are now), the returns it mutes, tracks deleted since (by id; skipped), tracks
    /// added since (`not_in_set`; a recall leaves them as they are), and whether the song is
    /// as the set says (`as_now`).
    #[tool(annotations(
        title = "List mute sets",
        read_only_hint = true,
        open_world_hint = false
    ))]
    async fn list_mute_sets(
        &self,
        Parameters(a): Parameters<ListMuteSetsArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let sets: Vec<Value> = (read.song.doc.mute_sets.iter())
            .filter(|s| a.name.as_deref().is_none_or(|n| s.name == n.trim()))
            .map(|s| set_value(&read, s))
            .collect();
        if let Some(name) = &a.name
            && sets.is_empty()
        {
            return Ok(failed(format!(
                "the song has no mute set \"{}\"",
                name.trim()
            )));
        }
        let value = json!({ "mute_sets": sets });
        let names: Vec<&str> = (read.song.doc.mute_sets.iter())
            .map(|s| s.name.as_str())
            .collect();
        Ok(done(
            format!("{} mute set(s): {}\n{value}", names.len(), names.join(", ")),
            value,
        ))
    }

    /// Mutes the tracks and the returns as the set says, as one undo step, and solos the
    /// tracks it soloed (solo is no step: undo leaves it). Tracks deleted since are skipped;
    /// tracks added since stay as they are (`not_in_set`). A mute lane that moves a changed
    /// mute rests, as with M. If the song is so already, `changed: false`.
    #[tool(annotations(
        title = "Recall mute set",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = true,
        open_world_hint = false
    ))]
    async fn recall_mute_set(
        &self,
        Parameters(a): Parameters<MuteSetArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let name = a.name.trim().to_owned();
        let before = check!(self.read().await);
        let soloed = &before.song.session.soloed;
        // What the recall does, for the answer; the core does the same on its song.
        let recall = mute_sets::recall_mute_set(&before.song.doc, soloed, &name).ok();
        let d = check!(self.ask(AiAsk::RecallMuteSet { name: name.clone() }).await);
        let after = check!(self.read().await);
        let solo_changed = after.song.session.soloed != *soloed;
        let changed = d.step.is_some() || solo_changed;
        let muted: Vec<u32> = (after.song.doc.tracks.iter().enumerate())
            .filter(|(_, t)| t.muted)
            .map(|(i, _)| i as u32 + 1)
            .collect();
        let soloed_now: Vec<u32> = (after.song.doc.tracks.iter().enumerate())
            .filter(|(_, t)| after.song.session.soloed.contains(&t.id))
            .map(|(i, _)| i as u32 + 1)
            .collect();
        let gone: Vec<u32> = recall
            .as_ref()
            .map_or_else(Vec::new, |r| r.gone.iter().map(|t| t.0).collect());
        let value = json!({
            "changed": changed,
            "set": name,
            "step": d.step,
            "undo_steps": d.undo_count,
            "solo_changed": solo_changed,
            "muted": muted,
            "muted_returns": returns(after.song.doc.returns.each_ref().map(|r| r.muted)),
            "soloed": soloed_now,
            "deleted_track_ids": gone,
            "not_in_set": recall.as_ref().map(|r| numbers(&before, &r.not_in_set)),
            "rested_lanes": recall.as_ref().map(|r| numbers(&before, &r.rested)),
        });
        let mut lines = vec![match changed {
            true => format!("The song is as the mute set \"{name}\" says now."),
            false => format!("The song was as the mute set \"{name}\" says; nothing changed."),
        }];
        if !gone.is_empty() {
            lines.push(format!(
                "{} track(s) of the set were deleted since; skipped.",
                gone.len()
            ));
        }
        lines.push(value.to_string());
        Ok(done(lines.join("\n"), value))
    }

    /// Deletes a mute set, as one undo step. The mutes of the tracks stay as they are.
    #[tool(annotations(
        title = "Delete mute set",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn delete_mute_set(
        &self,
        Parameters(a): Parameters<MuteSetArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let name = a.name.trim().to_owned();
        let d = check!(self.ask(AiAsk::DeleteMuteSet { name: name.clone() }).await);
        let value = json!({
            "deleted": name,
            "step": d.step,
            "undo_steps": d.undo_count,
        });
        Ok(done(
            format!("Deleted the mute set \"{name}\".\n{value}"),
            value,
        ))
    }
}

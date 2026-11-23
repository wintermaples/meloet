//! The fake meloet side: two tools over a pretend song. Shared by the stdio server (a)
//! and the Unix socket server (c).

use std::sync::{Arc, Mutex};

use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Note {
    /// MIDI key, 0..=127 (60 = middle C).
    pub key: u8,
    /// Start in ticks (a quarter note = 1920).
    pub start: u64,
    /// Length in ticks.
    pub length: u64,
    /// 1..=127.
    pub velocity: u8,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct AddNotes {
    /// The track to add to, as shown by song_overview.
    pub track: String,
    pub notes: Vec<Note>,
}

#[derive(Clone)]
pub struct FakeMeloet {
    notes: Arc<Mutex<Vec<(String, Note)>>>,
    tool_router: ToolRouter<FakeMeloet>,
}

impl Default for FakeMeloet {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl FakeMeloet {
    pub fn new() -> Self {
        Self {
            notes: Arc::default(),
            tool_router: Self::tool_router(),
        }
    }

    /// Shows the open song: tempo, time signature, tracks and how many notes each has.
    #[tool(annotations(
        title = "Song overview",
        read_only_hint = true,
        open_world_hint = false
    ))]
    async fn song_overview(&self) -> Result<CallToolResult, McpError> {
        let notes = self.notes.lock().unwrap();
        let piano = notes.iter().filter(|(t, _)| t == "Piano").count();
        let text = format!(
            "song: \"spike song\" (fake)\ntempo: 120 BPM, 4/4, tick = 1/1920 quarter\ntracks:\n- Piano: {piano} notes\n- Drums: 0 notes"
        );
        Ok(CallToolResult::success(vec![ContentBlock::text(text)]))
    }

    /// Adds notes to a track. One call is one undo step in meloet.
    #[tool(annotations(
        title = "Add notes",
        read_only_hint = false,
        destructive_hint = false,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn add_notes(
        &self,
        Parameters(req): Parameters<AddNotes>,
    ) -> Result<CallToolResult, McpError> {
        if req.track != "Piano" && req.track != "Drums" {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "no track named \"{}\"; call song_overview to see the tracks",
                req.track
            ))]));
        }
        if let Some(bad) = req
            .notes
            .iter()
            .find(|n| n.key > 127 || n.velocity == 0 || n.velocity > 127)
        {
            return Ok(CallToolResult::error(vec![ContentBlock::text(format!(
                "note out of range: {bad:?}"
            ))]));
        }
        let count = req.notes.len();
        let mut notes = self.notes.lock().unwrap();
        notes.extend(req.notes.into_iter().map(|n| (req.track.clone(), n)));
        Ok(CallToolResult::success(vec![ContentBlock::text(format!(
            "added {count} notes to {}",
            req.track
        ))]))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for FakeMeloet {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("meloet-spike", "0.0.0"))
            .with_instructions("Fake meloet for the s10-mcp spike.")
    }
}

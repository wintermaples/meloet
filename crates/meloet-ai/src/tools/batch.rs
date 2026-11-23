//! Several write tools as one undo step. Each operation goes through the tool's own code,
//! found by name in the router, so its arguments are checked as in a call of its own and a
//! tool added later works here with nothing to add. The tool runs against [`InBatch`],
//! which sends its asks to the core as asks of the open batch: the core makes one step of
//! their steps at the end, or takes them all back.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use meloet_core::ai::{
    AiAnswer, AiAsk, AiLevels, AiMeasure, AiRefusal, AiRender, AiRendered, AiSong,
};
use meloet_protocol::AiPort;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::model::{CallToolRequestParams, CallToolResponse, CallToolResult, JsonObject};
use rmcp::service::RequestContext;
use rmcp::{
    ErrorData as McpError, RoleServer, handler::server::wrapper::Parameters, schemars, tool,
    tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{BURST, Bucket, Tools, done, failed, refusal_text};
use crate::Meloet;

/// Operations one `batch` takes.
pub const MAX_OPERATIONS: usize = 200;

/// Tools that are refused before anything runs, and why. Tools that only read are refused
/// by their `readOnlyHint`. A tool not listed here whose ask ends later is refused by the
/// core when its turn comes, and the batch is taken back.
const NOT_IN_A_BATCH: &[(&str, &str)] = &[
    (
        "set_plugin_state",
        "the plugin takes a state later; call it by itself",
    ),
    (
        "set_plugin_state_text",
        "the plugin takes a state later; call it by itself",
    ),
    (
        "surge_set_modulation",
        "the plugin takes a state later; call it by itself",
    ),
    (
        "surge_remove_modulation",
        "the plugin takes a state later; call it by itself",
    ),
    (
        "surge_set_patch_params",
        "the plugin takes a state later; call it by itself",
    ),
    ("batch", "a batch cannot hold a batch"),
    ("undo", "undo takes back a step; it cannot be a part of one"),
    ("save_song", "a save ends later; call it after the batch"),
    ("save_song_as", "a save ends later; call it after the batch"),
    ("open_song", "another song cannot be opened inside a batch"),
    ("new_song", "another song cannot be started inside a batch"),
    (
        "import_audio",
        "bringing in a file ends later; call it by itself",
    ),
    (
        "export_song",
        "an export takes a while and changes nothing in the song; call it by itself",
    ),
];

/// A number for each batch of this process: connections share the core.
static NEXT_BATCH: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct Operation {
    /// The name of a tool that changes the song, like "set_volume" or "write_notes".
    #[schemars(length(min = 1, max = 64))]
    pub tool: String,
    /// That tool's arguments, as in a call of its own. A value may be taken from the result
    /// of an earlier operation of this batch: write `{"result_of": 1, "field": "clip"}` in
    /// its place for the `clip` of operation 1's result (operations count from 1; results
    /// have `clip`, `take` and `track` where the tool gives them).
    #[serde(default)]
    pub arguments: Option<JsonObject>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct BatchArgs {
    /// 1 to 200 operations, run in this order. Each sees what the ones before it did.
    #[schemars(length(min = 1, max = 200))]
    pub operations: Vec<Operation>,
}

/// What a tool inside a batch reaches in place of meloet: the song as it is, and writes
/// that are asks of the batch. Nothing that renders.
struct InBatch {
    meloet: Arc<dyn Meloet>,
    batch: u64,
}

impl Meloet for InBatch {
    fn song(&self) -> Option<AiSong> {
        self.meloet.song()
    }

    fn ask(&self, ask: AiAsk) -> AiAnswer {
        match ask {
            // A read.
            AiAsk::PluginParams { .. } => self.meloet.ask(ask),
            ask => self.meloet.ask(AiAsk::InBatch {
                batch: self.batch,
                ask: Box::new(ask),
            }),
        }
    }

    fn measure(
        &self,
        _: AiMeasure,
        _: &mut dyn FnMut(u8),
        _: &dyn Fn() -> bool,
    ) -> Result<AiLevels, AiRefusal> {
        Err(AiRefusal::NotInBatch)
    }

    fn render(
        &self,
        _: AiRender,
        _: &mut dyn FnMut(u8),
        _: &dyn Fn() -> bool,
    ) -> Result<AiRendered, AiRefusal> {
        Err(AiRefusal::NotInBatch)
    }

    fn port(&self, _: AiPort) {}
}

/// The batch that is open in the core. Dropped before it was closed (the call was given
/// up), it is taken back.
struct Open {
    meloet: Arc<dyn Meloet>,
    batch: u64,
    closed: bool,
}

impl Drop for Open {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        let (meloet, batch) = (self.meloet.clone(), self.batch);
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn_blocking(move || meloet.ask(AiAsk::Rollback { batch }));
        }
    }
}

fn first_text(result: &CallToolResult) -> String {
    (result.content.first())
        .and_then(|block| block.as_text())
        .map(|text| text.text.clone())
        .unwrap_or_default()
}

/// The operation `result_of` names in `value`, if `value` is such a reference: its place
/// in the list (from 0) and the field.
fn reference(value: &Value) -> Option<Result<(usize, &str), String>> {
    let object = value.as_object()?;
    let of = object.get("result_of")?;
    let field = object.get("field").and_then(Value::as_str);
    let number = of.as_u64().and_then(|n| usize::try_from(n).ok());
    Some(match (number, field, object.len()) {
        (Some(n), Some(field), 2) if n >= 1 => Ok((n - 1, field)),
        _ => Err(
            "a reference is {\"result_of\": <operation number from 1>, \"field\": \"<name>\"}"
                .to_owned(),
        ),
    })
}

/// Every reference in `value` names an operation before the one at `at`.
fn references_ok(value: &Value, at: usize) -> Result<(), String> {
    match reference(value) {
        Some(Ok((of, _))) if of < at => Ok(()),
        Some(Ok((of, _))) => Err(format!(
            "result_of {} is not an operation before this one",
            of + 1
        )),
        Some(Err(why)) => Err(why),
        None => match value {
            Value::Array(items) => items.iter().try_for_each(|v| references_ok(v, at)),
            Value::Object(fields) => fields.values().try_for_each(|v| references_ok(v, at)),
            _ => Ok(()),
        },
    }
}

/// Puts the values of the earlier results (`{"tool", "result"}` each) in place of the
/// references to them.
fn resolve(value: &mut Value, results: &[Value]) -> Result<(), String> {
    if let Some(found) = reference(value) {
        let (of, field) = found?;
        let taken = results.get(of).and_then(|r| r["result"].get(field));
        *value = match taken {
            Some(v) if !v.is_null() => v.clone(),
            _ => {
                return Err(format!(
                    "the result of operation {} has no `{field}`",
                    of + 1
                ));
            }
        };
        return Ok(());
    }
    match value {
        Value::Array(items) => items.iter_mut().try_for_each(|v| resolve(v, results)),
        Value::Object(fields) => fields.values_mut().try_for_each(|v| resolve(v, results)),
        _ => Ok(()),
    }
}

impl Tools {
    /// Why the tool cannot be a part of a batch, known before anything runs.
    fn not_in_a_batch(&self, name: &str) -> Option<String> {
        if let Some((_, why)) = NOT_IN_A_BATCH.iter().find(|(tool, _)| *tool == name) {
            return Some((*why).to_owned());
        }
        let Some(tool) = self.tool_router.get(name) else {
            return Some("there is no such tool".to_owned());
        };
        let reads = (tool.annotations.as_ref()).and_then(|a| a.read_only_hint) == Some(true);
        reads.then(|| "it only reads; call it by itself".to_owned())
    }

    /// The tools as a batch runs them: writes are asks of the batch, and each operation may
    /// call as if it were the first call in a while.
    fn in_batch(&self, batch: u64) -> Tools {
        Tools {
            meloet: Arc::new(InBatch {
                meloet: self.meloet.clone(),
                batch,
            }),
            bucket: Arc::new(Mutex::new(Bucket {
                tokens: BURST,
                at: Instant::now(),
            })),
            scratch: self.scratch.clone(),
            tool_router: self.tool_router.clone(),
        }
    }
}

#[tool_router(router = batch_router, vis = "pub(super)")]
impl Tools {
    /// Runs several write tools in order as ONE undo step; each operation is a tool's name
    /// and its arguments. If one fails, all of it is taken back and nothing changes. Not
    /// for undo, files (save, open, new, import, export), tools that read, or a plugin's
    /// parameters.
    #[tool(annotations(
        title = "Batch: several changes as one undo step",
        read_only_hint = false,
        destructive_hint = true,
        idempotent_hint = false,
        open_world_hint = false
    ))]
    async fn batch(
        &self,
        Parameters(args): Parameters<BatchArgs>,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let count = args.operations.len();
        if count == 0 || count > MAX_OPERATIONS {
            return Ok(failed(format!("give 1 to {MAX_OPERATIONS} operations")));
        }
        for (i, op) in args.operations.iter().enumerate() {
            let wrong = self.not_in_a_batch(&op.tool).or_else(|| {
                let arguments = op.arguments.iter().flat_map(|a| a.values());
                (arguments.map(|v| references_ok(v, i)).find_map(Result::err))
                    .map(|why| format!("in its arguments, {why}"))
            });
            if let Some(why) = wrong {
                return Ok(failed(format!(
                    "Operation {} ({}) cannot be in a batch: {why}. Nothing was run.",
                    i + 1,
                    op.tool
                )));
            }
        }

        let batch = NEXT_BATCH.fetch_add(1, Ordering::Relaxed);
        let began = check!(self.ask(AiAsk::Begin { batch }).await);
        let mut open = Open {
            meloet: self.meloet.clone(),
            batch,
            closed: false,
        };
        let inner = self.in_batch(batch);
        let mut results: Vec<Value> = Vec::with_capacity(count);
        let mut failure = None;
        for (i, op) in args.operations.into_iter().enumerate() {
            if context.ct.is_cancelled() {
                failure = Some((i, op.tool, "the call was cancelled".to_owned()));
                break;
            }
            let mut arguments = Value::Object(op.arguments.unwrap_or_default());
            if let Err(why) = resolve(&mut arguments, &results) {
                failure = Some((i, op.tool, why));
                break;
            }
            let Value::Object(arguments) = arguments else {
                failure = Some((i, op.tool, "the arguments are not an object".to_owned()));
                break;
            };
            if let Ok(mut bucket) = inner.bucket.lock() {
                bucket.tokens = BURST;
            }
            let params = CallToolRequestParams::new(op.tool.clone()).with_arguments(arguments);
            let call = ToolCallContext::new(&inner, params, context.clone());
            let result = match inner.tool_router.call(call).await {
                Ok(CallToolResponse::Complete(result)) => result,
                Ok(_) => failed("the tool did not finish"),
                Err(error) => failed(error.message.to_string()),
            };
            if result.is_error == Some(true) {
                failure = Some((i, op.tool, first_text(&result)));
                break;
            }
            // The rows and counts in between are not those of the history afterwards.
            let mut value = (result.structured_content.clone())
                .unwrap_or_else(|| json!({ "text": first_text(&result) }));
            if let Some(fields) = value.as_object_mut() {
                fields.remove("step");
                fields.remove("undo_steps");
            }
            results.push(json!({ "tool": op.tool, "result": value }));
        }

        let Some((at, tool, why)) = failure else {
            let closed = self.answer(AiAsk::Commit { batch }).await;
            open.closed = true;
            let d = match check!(closed) {
                AiAnswer::Done(d) => *d,
                AiAnswer::Refused(r) => {
                    return Ok(failed(format!(
                        "The batch's {count} operations ran, but it could not be finished: {}. \
                         Call song_overview to see the song and its undo steps.",
                        refusal_text(&r)
                    )));
                }
                _ => return Ok(super::closing()),
            };
            let changed = results.iter().any(|r| r["result"]["changed"] != false);
            let value = json!({
                "changed": changed,
                "step": d.step,
                "undo_steps": d.undo_count,
                "operations": count,
                "results": results,
            });
            let summary = match (changed, d.step.is_some()) {
                (false, _) => "No change: the song already is like that.".to_owned(),
                (true, true) => format!("Done: {count} operations as one undo step."),
                (true, false) if d.undo_count > began.undo_count => format!(
                    "Done: {count} operations, but as undo steps of their own: someone else \
                     changed the song while the batch ran."
                ),
                (true, false) => format!(
                    "Done: {count} operations, with no undo step (solo and the loop's switch \
                     are not steps)."
                ),
            };
            return Ok(done(format!("{summary}\n{value}"), value));
        };

        let back = self.answer(AiAsk::Rollback { batch }).await;
        open.closed = true;
        let taken_back = matches!(back, Ok(AiAnswer::Done(_)));
        let undo_steps = self.read().await.ok().map(|read| read.undo_count);
        let rest = match &back {
            Ok(AiAnswer::Done(_)) => {
                "Nothing was changed: the operations before it were taken back.".to_owned()
            }
            Ok(AiAnswer::Refused(r)) => format!(
                "The {at} operations before it could NOT be taken back ({}): what they did \
                 stays, as undo steps of their own. Call song_overview before going on.",
                refusal_text(r)
            ),
            _ => "meloet is closing or does not answer.".to_owned(),
        };
        let mut result = failed(format!(
            "Batch stopped at operation {} of {count} ({tool}): {why}\n{rest}",
            at + 1
        ));
        result.structured_content = Some(json!({
            "changed": !taken_back && at > 0,
            "taken_back": taken_back,
            "failed": { "operation": at + 1, "tool": tool, "error": why },
            "undo_steps": undo_steps,
            "done_before": results,
        }));
        Ok(result)
    }
}

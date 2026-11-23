//! Which key of a track's sound is which drum (`get_key_names`).

use std::time::{Duration, Instant};

use meloet_core::ai::{AiAnswer, AiAsk, AiKey, AiKeyMap, AiKeyNames, AiKeyNote, AiKeySource};
use meloet_core::play::keynames::read_sf2;
use rmcp::model::CallToolResult;
use rmcp::{
    ErrorData as McpError, handler::server::wrapper::Parameters, schemars, tool, tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Tools, closing, done, failed, refusal_text, track_of};
use crate::song::{key_name, sound_name};

/// How long a plugin is given to name its keys.
const PLUGIN_WAIT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct KeyNamesArgs {
    /// The keys track's number (from 1), as song_overview lists it.
    #[schemars(range(min = 1, max = 64))]
    pub track: u32,
}

fn source_name(source: AiKeySource) -> &'static str {
    match source {
        AiKeySource::Builtin => "builtin",
        AiKeySource::Sf2SampleName => "sf2_sample_name",
        AiKeySource::Lv2Midnam => "lv2_midnam",
        AiKeySource::GeneralMidiGuess => "general_midi_guess",
        AiKeySource::None => "none",
    }
}

fn source_text(map: &AiKeyMap) -> &'static str {
    match map.source {
        AiKeySource::Builtin => {
            "meloet's own kit: the names are the rows its drum piano roll shows (in Japanese, as \
             the person sees them)."
        }
        AiKeySource::Sf2SampleName => {
            "An SF2 file has no names for its keys: these names are the names of the samples \
             each key plays, which usually say the drum."
        }
        AiKeySource::Lv2Midnam => "The names are the plugin's own (its MIDNAM note names).",
        AiKeySource::GeneralMidiGuess => {
            "Nothing in this sound names its keys: these are only the usual General MIDI drum \
             layout (the names meloet's drum piano roll shows for SF2 kits), and this sound may \
             put its drums elsewhere. Listen with render_note to be sure."
        }
        AiKeySource::None if map.kit => "This sound does not name its keys.",
        AiKeySource::None => {
            "Not a drum kit: its keys play one sound at different pitches, so they have no names."
        }
    }
}

fn note_text(note: AiKeyNote) -> &'static str {
    match note {
        AiKeyNote::NotInstalled => {
            "The sound pack on this PC does not have this instrument: the track plays a sine \
             wave in its place."
        }
        AiKeyNote::NotRead => {
            "Its samples are not in memory yet; the track is silent until they are read."
        }
        AiKeyNote::SampleNames => {
            "Several samples on one key (layers) are joined with \" / \"; the mark of a stereo \
             half (L, R) is left out."
        }
        AiKeyNote::Sf2Unreadable => "The SF2 file could not be read just now.",
        AiKeyNote::Sf2Missing => {
            "No SF2 file added on this PC has this preset: the track plays a sine wave in its \
             place."
        }
    }
}

fn key_value(k: &AiKey) -> Value {
    let mut value = json!({
        "key": k.key,
        "note": key_name(k.key),
        "name": k.name,
        "source": source_name(k.source),
    });
    if let Some(id) = k.id {
        value["id"] = json!(id);
    }
    if let Some(sounds) = k.sounds {
        value["sounds"] = json!(sounds);
    }
    value
}

#[tool_router(router = keynames_router, vis = "pub(super)")]
impl Tools {
    /// Which key of a track's sound is which drum (kick, snare, hi-hat…). Names come from
    /// meloet's kits, SF2 sample names or the plugin (LV2 midnam); if nothing names them and it
    /// looks like a drum kit, the General MIDI layout is given with `certain: false`. A non-kit
    /// instrument has no names, only its key range.
    #[tool(annotations(
        title = "Get key names",
        read_only_hint = true,
        open_world_hint = false
    ))]
    async fn get_key_names(
        &self,
        Parameters(args): Parameters<KeyNamesArgs>,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let track = check!(track_of(&read, args.track)).clone();
        let until = Instant::now() + PLUGIN_WAIT;
        let mut map = loop {
            match check!(self.answer(AiAsk::KeyNames { track: args.track }).await) {
                AiAnswer::KeyNames(AiKeyNames::Known(map)) => break *map,
                AiAnswer::KeyNames(AiKeyNames::Waiting) if Instant::now() < until => {}
                AiAnswer::KeyNames(AiKeyNames::Waiting) => {
                    return Ok(failed(format!(
                        "the plugin did not name its keys within {} s",
                        PLUGIN_WAIT.as_secs()
                    )));
                }
                AiAnswer::Refused(r) => return Ok(failed(refusal_text(&r))),
                _ => return Ok(closing()),
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        };
        if map.sf2_path.is_some() {
            map = match tokio::task::spawn_blocking(move || {
                read_sf2(&mut map);
                map
            })
            .await
            {
                Ok(map) => map,
                Err(_) => return Ok(closing()),
            };
        }
        let mut notes = vec![source_text(&map).to_owned()];
        if map.source == AiKeySource::Sf2SampleName && !map.certain {
            notes.push(
                "Keys whose source is general_midi_guess have no sample of their own and are \
                 named by the General MIDI layout, as a guess."
                    .to_owned(),
            );
        }
        notes.extend(map.notes.iter().map(|n| note_text(*n).to_owned()));
        let range = map.range.map(|(low, high)| {
            json!({ "low": low, "high": high, "low_note": key_name(low), "high_note": key_name(high) })
        });
        let value = json!({
            "track": args.track,
            "track_name": track.name,
            "sound": sound_name(&map.sound),
            "kit": map.kit,
            "source": source_name(map.source),
            "certain": map.certain,
            "keys": map.keys.iter().map(key_value).collect::<Vec<Value>>(),
            "range": range,
            "notes": notes,
        });
        let kind = if map.kit {
            "a drum kit"
        } else {
            "not a drum kit"
        };
        let summary = format!(
            "Track {} \"{}\" ({}): {kind}, {} named keys ({}{}).\n{}\n{value}",
            args.track,
            track.name,
            sound_name(&map.sound),
            map.keys.iter().filter(|k| k.name.is_some()).count(),
            source_name(map.source),
            if map.certain { "" } else { ", a guess" },
            notes.join(" "),
        );
        Ok(done(summary, value))
    }
}

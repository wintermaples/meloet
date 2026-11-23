//! The drum patterns of the shelf's 「パターン」 (round 12, P4): notes only, written by us in
//! `tools/patterns/drums.json` and built into meloet. A note names a sound of the kit
//! ([`DrumPiece`]), so it sits on the same key in the three kits of meloet and in an SF2 kit.

use std::sync::LazyLock;

use meloet_model::drums::DrumPiece;
use meloet_model::{Meter, Note};
use meloet_protocol::{PatternGroup, PatternView};
use serde::Deserialize;

const DATA: &str = include_str!("../../../tools/patterns/drums.json");
const FORMAT: &str = "meloet-drum-patterns";
const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    pub id: String,
    pub group: PatternGroup,
    /// Bars of 4/4.
    pub bars: u32,
    /// Every note starts on a multiple of it: 480 (a sixteenth) or 640 (an eighth-note
    /// triplet, for the shuffle).
    pub grid: u64,
    /// From the pattern's head, in order of start.
    pub notes: Vec<Hit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hit {
    pub piece: DrumPiece,
    pub at: u64,
    pub length: u64,
    pub velocity: u8,
}

impl Pattern {
    pub fn ticks(&self) -> u64 {
        u64::from(self.bars) * Meter::FOUR_FOUR.ticks_per_bar()
    }

    /// The notes placed with the pattern's head at `start`.
    pub fn notes_at(&self, start: u64) -> Vec<Note> {
        (self.notes.iter())
            .map(|h| Note {
                from: None,
                start: start + h.at,
                length: h.length,
                key: h.piece.note(),
                velocity: h.velocity,
            })
            .collect()
    }

    pub fn view(&self) -> PatternView {
        PatternView {
            id: self.id.clone(),
            group: self.group,
            bars: self.bars,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    format: String,
    version: u32,
    patterns: Vec<PatternData>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PatternData {
    id: String,
    group: PatternGroup,
    bars: u32,
    grid: u64,
    notes: Vec<HitData>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HitData {
    piece: String,
    at: u64,
    length: u64,
    velocity: u8,
}

fn piece(id: &str) -> Option<DrumPiece> {
    DrumPiece::ROWS.into_iter().find(|p| p.id() == id)
}

pub(crate) fn parse(text: &str) -> Result<Vec<Pattern>, String> {
    let file: File = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if file.format != FORMAT || file.version != VERSION {
        return Err(format!("not {FORMAT} {VERSION}"));
    }
    (file.patterns.into_iter())
        .map(|p| {
            let mut notes = (p.notes.into_iter())
                .map(|h| {
                    Ok(Hit {
                        piece: piece(&h.piece).ok_or(format!("{}: {}", p.id, h.piece))?,
                        at: h.at,
                        length: h.length,
                        velocity: h.velocity,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            notes.sort_by_key(|h| (h.at, h.piece.note()));
            Ok(Pattern {
                id: p.id,
                group: p.group,
                bars: p.bars,
                grid: p.grid,
                notes,
            })
        })
        .collect()
}

static ALL: LazyLock<Vec<Pattern>> =
    LazyLock::new(|| parse(DATA).expect("tools/patterns/drums.json reads (a test sees to it)"));

/// Every pattern, in the order of the shelf.
pub fn all() -> &'static [Pattern] {
    &ALL
}

pub fn find(id: &str) -> Option<&'static Pattern> {
    all().iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests;

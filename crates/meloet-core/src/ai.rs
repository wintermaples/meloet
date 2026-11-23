//! What the AI link (the crate `meloet-ai`, its socket and its MCP tools) asks of the core,
//! and what it gets back (docs/architecture.md 24-11). Every write is one undoable step with
//! a step name the history band already has; nothing here is a screen intent.

use meloet_model::{InstrumentRef, Note, Song, TickRange};
use meloet_protocol::HistoryRow;

/// Called on the core thread with the switch 「AI とつなぐ」, once when it is set and again
/// whenever the switch changes. Must not wait.
pub type AiSwitch = Box<dyn Fn(bool) + Send>;

/// Where written notes go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiPlace {
    /// The track of this number (from 1, as the arrange shows it): the clip that holds the
    /// earliest start, or else a new clip.
    Track(u32),
    /// This keys clip; it grows when the notes go past it.
    Clip(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiAsk {
    /// Song positions; `from` is ignored.
    WriteNotes {
        at: AiPlace,
        notes: Vec<Note>,
    },
    /// Indexes in the take, which must be the take a clip shows now.
    DeleteNotes {
        take: u32,
        notes: Vec<u32>,
    },
    /// `track` is a number from 1.
    CreateClip {
        track: u32,
        tick: u64,
    },
    RenameClip {
        clip: u32,
        name: String,
    },
    /// The tempo from `tick` (snapped to the nearest beat) on; 0 is the song's head.
    SetTempo {
        tick: u64,
        bpm: f64,
    },
    /// `tick` is snapped to the nearest beat.
    AddMarker {
        tick: u64,
        name: String,
    },
    /// A keys track below the last one.
    AddTrack {
        instrument: InstrumentRef,
    },
    Undo,
}

impl AiAsk {
    pub fn name(&self) -> &'static str {
        match self {
            AiAsk::WriteNotes { .. } => "write notes",
            AiAsk::DeleteNotes { .. } => "delete notes",
            AiAsk::CreateClip { .. } => "create a clip",
            AiAsk::RenameClip { .. } => "rename a clip",
            AiAsk::SetTempo { .. } => "set the tempo",
            AiAsk::AddMarker { .. } => "add a marker",
            AiAsk::AddTrack { .. } => "add a track",
            AiAsk::Undo => "undo",
        }
    }
}

/// Why the song may not be changed now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiBusy {
    CountingIn,
    Recording,
    Exporting,
}

/// The song as it is now.
#[derive(Debug, Clone, PartialEq)]
pub struct AiSong {
    pub song: Song,
    pub undo_count: u32,
    pub busy: Option<AiBusy>,
}

/// What a write did.
#[derive(Debug, Clone, PartialEq)]
pub struct AiDone {
    /// The row the step has in the history band; for `Undo`, the row that was undone.
    pub step: Option<HistoryRow>,
    pub undo_count: u32,
    /// The clip written, made or renamed.
    pub clip: Option<u32>,
    /// The take that clip shows afterwards.
    pub take: Option<u32>,
    /// The track added, as its number from 1.
    pub track: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiAnswer {
    Done(Box<AiDone>),
    Refused(AiRefusal),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AiRefusal {
    Busy(AiBusy),
    /// meloet is ending, or the core does not answer.
    Closing,
    NoSuchTrack(u32),
    NoSuchClip(u32),
    /// No clip shows this take now (the song changed since it was read).
    NoSuchTake(u32),
    NoSuchNote(u32),
    /// A vocal track or clip, which holds audio.
    NotKeys,
    /// It would overlap another clip of the track.
    Overlap,
    /// Past the end of the song meloet can hold.
    OffTheSong,
    BadNote,
    BadTempo,
    /// It would leave the song as it is.
    Unchanged,
    TooManyTracks,
    NothingToUndo,
    /// Another measurement is running.
    Measuring,
    /// The export failed; the text is for the log.
    ExportFailed(String),
}

/// A range to render and measure, through the export's own engine.
#[derive(Debug, Clone, PartialEq)]
pub struct AiMeasure {
    pub span: TickRange,
    /// Track numbers from 1; `None`: every track. Only these tracks are in the master's mix.
    pub tracks: Option<Vec<u32>>,
}

/// The levels of one rendering, stereo, from the range's start to its end (no tail).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PassLevels {
    /// The largest sample, before the output's clip; 1.0 is 0 dBFS.
    pub peak: f32,
    /// Over both channels.
    pub rms: f32,
    /// Places over 0 dB, as 「書き出す前のたしかめ」 counts them.
    pub over: u32,
    pub frames: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AiLevels {
    pub sample_rate: u32,
    pub master: PassLevels,
    /// Number from 1, name, levels of the track played alone (with its sends and the master).
    pub tracks: Vec<(u32, String, PassLevels)>,
}

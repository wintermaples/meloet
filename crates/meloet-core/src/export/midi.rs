//! MIDI in the rules of the export (protocol 36; spec 3 "MIDI export", spec 7 A-31): what
//! 「入れるトラック」 takes of the song, and the files 「書き出す」 writes at once through
//! the writer the AI link's `export_midi` uses ([`crate::midi`]).

use std::path::{Path, PathBuf};

use meloet_model::{Doc, TICKS_PER_QUARTER, TempoMap, TrackId};
use meloet_protocol::{
    ExportFailure, ExportMidiProblem, ExportMidiTrack, ExportMidiTracks, TickSpan,
};

use super::numbers::{self, Numbers};
use super::{ExportNews, Scene};
use crate::midi::{self, MidiTooLong};

/// What 「入れるトラック」 takes of the song as it is now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Taken {
    /// Places in `Doc::tracks` of the keys tracks taken, in their order.
    pub places: Vec<usize>,
    /// Of `ExportMidiTracks::Numbers`: places of the vocal tracks the numbers name.
    pub vocal: Vec<usize>,
    pub problem: Option<ExportMidiProblem>,
    /// The selected track is a keys track.
    pub selected_usable: bool,
}

pub(crate) fn taken(choice: ExportMidiTracks, text: &str, scene: &Scene) -> Taken {
    let doc = scene.doc;
    let keys = |i: &usize| !doc.tracks[*i].kind.is_vocal();
    let selected = (scene.selected)
        .and_then(|id| doc.tracks.iter().position(|t| t.id == id))
        .filter(keys);
    let (places, vocal, problem) = match choice {
        ExportMidiTracks::All => (
            (0..doc.tracks.len()).filter(keys).collect(),
            Vec::new(),
            None,
        ),
        ExportMidiTracks::Selected => (selected.into_iter().collect(), Vec::new(), None),
        ExportMidiTracks::Numbers => match numbers::read(text, doc.tracks.len() as u32) {
            Numbers::Unreadable => (Vec::new(), Vec::new(), Some(ExportMidiProblem::Unreadable)),
            Numbers::Read { tracks, missing } => {
                let (places, vocal) = (tracks.into_iter()).map(|n| n as usize - 1).partition(keys);
                let problem = missing.map(|number| ExportMidiProblem::NoSuchTrack { number });
                (places, vocal, problem)
            }
        },
    };
    let problem = problem.or_else(|| places.is_empty().then_some(ExportMidiProblem::NoTrack));
    Taken {
        places,
        vocal,
        problem,
        selected_usable: selected.is_some(),
    }
}

/// The tracks at `places` as the dialog lists them.
pub(crate) fn listed(doc: &Doc, places: &[usize]) -> Vec<ExportMidiTrack> {
    (places.iter())
        .map(|&i| ExportMidiTrack {
            number: i as u32 + 1,
            name: doc.tracks[i].name.clone(),
        })
        .collect()
}

/// A file 「書き出す」 writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidiOut {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

/// The files of a MIDI export of `span`, at 1920 ticks a quarter note: one at `place` with
/// every track of `places`; with `per_track` one in the folder `place` for each of them,
/// named as its track, with the first track of the tempos and the meters. `kits` as in
/// [`midi::midi_file`].
pub(crate) fn files(
    doc: &Doc,
    places: &[usize],
    kits: &[TrackId],
    span: TickSpan,
    place: &Path,
    per_track: bool,
) -> Result<Vec<MidiOut>, MidiTooLong> {
    let division = TICKS_PER_QUARTER as u16;
    let bytes = |places: &[usize]| {
        let made = midi::midi_file(doc, places, kits, None, span.start, span.end, division)?;
        Ok(midi::smf::write(&made.file))
    };
    if !per_track {
        return Ok(vec![MidiOut {
            path: place.to_path_buf(),
            bytes: bytes(places)?,
        }]);
    }
    let ending = meloet_protocol::ExportFormat::Midi.extension();
    let mut taken = Vec::new();
    let mut out = Vec::new();
    for &i in places {
        let name = super::job::free_name(&doc.tracks[i].name, ending, &taken);
        taken.push(name.clone());
        out.push(MidiOut {
            path: place.join(name),
            bytes: bytes(&[i])?,
        });
    }
    Ok(out)
}

/// How long `span` plays, in seconds.
pub(crate) fn length_s(doc: &Doc, span: TickSpan) -> u32 {
    let map = TempoMap::new(&doc.timeline);
    let seconds = map.seconds_at(span.end as f64) - map.seconds_at(span.start as f64);
    seconds.max(0.0).round() as u32
}

/// Writes the files of `job`, each safely in its place: the answer to
/// [`super::ExportAsk::WriteMidi`]. When one cannot be written, those written before it
/// are taken away again.
pub(crate) fn write(job: u64, files: &[MidiOut]) -> ExportNews {
    let mut written = Vec::new();
    for file in files {
        let dir = file.path.parent().unwrap_or(Path::new(""));
        let done =
            std::fs::create_dir_all(dir).and_then(|()| midi::write_beside(&file.path, &file.bytes));
        if let Err(e) = done {
            eprintln!("meloet: {} is not written: {e}", file.path.display());
            for path in &written {
                let _ = std::fs::remove_file(path);
            }
            return ExportNews::Failed {
                job,
                reason: ExportFailure::CannotWrite,
            };
        }
        written.push(file.path.clone());
    }
    ExportNews::Finished {
        job,
        files: written,
        wave: Vec::new(),
    }
}

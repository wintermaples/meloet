//! Standard MIDI files: the reader and the writer ([`smf`]), and the song, or a part of it,
//! as a file of format 1. The export dialog (spec 3 "MIDI export") and the AI link's
//! `export_midi` write the same file (docs/architecture.md 25-39).

pub mod smf;

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use meloet_model::{
    Clip, ClipId, Doc, InstrumentGroup, InstrumentRef, TICKS_PER_QUARTER, TempoMap, Timeline, Track,
};

use smf::{OutFile, OutNote, OutTrack};

/// A tempo ramp is written as steps this long (a sixteenth note).
const RAMP_STEP: u64 = TICKS_PER_QUARTER / 4;
/// The General MIDI drum channel (10), from 0.
pub const DRUM_CHANNEL: u8 = 9;

/// What [`midi_file`] made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MidiExport {
    pub file: OutFile,
    /// The tracks written, in the order of `file.tracks`.
    pub tracks: Vec<MidiTrackWritten>,
    /// The notes of every track.
    pub notes: usize,
    /// Notes that sounded on past the range's end and were cut there.
    pub cut: u32,
    /// The song has a tempo ramp: it is written as steps.
    pub ramps: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiTrackWritten {
    /// The track's place in `Doc::tracks`.
    pub index: usize,
    pub notes: usize,
    /// 0 to 15: [`DRUM_CHANNEL`] for a drum kit, else 0.
    pub channel: u8,
}

/// The range is longer than a MIDI file can hold ([`smf::MAX_OUT_TICKS`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MidiTooLong;

/// The tracks of `doc` at the places `tracks` (in that order; a place with no track is
/// passed by) in `from..to`, as a format 1 file of `division` ticks a quarter note whose
/// tick 0 is `from`. The first track holds the song's name, the tempos (a ramp as a step
/// each sixteenth note) and the meters. Notes are written as they sound: those of the
/// shown take of each keys clip that start in the range, ending at their clip's end or at
/// `to` at the latest. With `only_clip` only that clip's notes. A track with no notes there
/// is still written, empty. Drum kits are on channel 10, the others on channel 1. Mute and
/// solo do not matter.
pub fn midi_file(
    doc: &Doc,
    tracks: &[usize],
    only_clip: Option<ClipId>,
    from: u64,
    to: u64,
    division: u16,
) -> Result<MidiExport, MidiTooLong> {
    let to = to.max(from);
    let ticks = |tick: u64| smf::unscaled(tick - from, division);
    if ticks(to) > smf::MAX_OUT_TICKS {
        return Err(MidiTooLong);
    }
    let map = TempoMap::new(&doc.timeline);
    let mut out = Vec::new();
    let mut written = Vec::new();
    let (mut count, mut cut) = (0, 0);
    for &index in tracks {
        let Some(track) = doc.tracks.get(index) else {
            continue;
        };
        let mut notes = Vec::new();
        for clip in track.clips.iter().filter(|c| c.is_keys()) {
            if only_clip.is_some_and(|only| only != clip.id) {
                continue;
            }
            let (sounding, cut_here) = heard(clip, from, to);
            cut += cut_here;
            notes.extend(sounding.iter().map(|n| {
                let start = ticks(n.start);
                OutNote {
                    start,
                    length: ticks(n.end).saturating_sub(start).max(1),
                    key: n.key,
                    velocity: n.velocity,
                }
            }));
        }
        let channel = if is_drums(track) { DRUM_CHANNEL } else { 0 };
        count += notes.len();
        written.push(MidiTrackWritten {
            index,
            notes: notes.len(),
            channel,
        });
        out.push(OutTrack {
            name: track.name.clone(),
            channel,
            notes,
        });
    }
    let timeline = &doc.timeline;
    let mut tempos: Vec<(u64, u32)> = Vec::new();
    let mut steps = tempo_steps(timeline, &map, from, to);
    if steps.is_empty() {
        // An empty range still says the tempo it starts at.
        steps.push((from, map.tempo_at(from as f64)));
    }
    for (tick, bpm) in steps {
        let us = (60_000_000.0 / bpm.max(1.0)).round() as u32;
        if tempos.last().is_none_or(|last| last.1 != us) {
            tempos.push((ticks(tick), us));
        }
    }
    let head = map.meter_at(from).meter;
    let mut meters = vec![(0, head.num(), head.den())];
    for change in &timeline.meter_changes {
        let tick = map.tick_of_bar(u64::from(change.bar));
        if tick > from && tick < to {
            meters.push((ticks(tick), change.meter.num(), change.meter.den()));
        }
    }
    let ramps = timeline.head_ramp || timeline.tempo_points.iter().any(|p| p.ramp);
    Ok(MidiExport {
        file: OutFile {
            division,
            name: doc.name.clone(),
            tempos,
            meters,
            length: ticks(to),
            tracks: out,
        },
        tracks: written,
        notes: count,
        cut,
        ramps,
    })
}

/// Writes `bytes` beside `path` and moves them over it, so a failed write leaves the file
/// that was there.
pub fn write_beside(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut beside = path.as_os_str().to_owned();
    beside.push(format!(".{}.tmp", std::process::id()));
    let beside = PathBuf::from(beside);
    let written = (|| {
        let mut file = std::fs::File::create(&beside)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&beside, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&beside);
    }
    written
}

fn is_drums(track: &Track) -> bool {
    matches!(&track.instrument, InstrumentRef::Builtin(b) if b.group() == InstrumentGroup::Drums)
}

/// A note of a clip as it sounds: where it starts and ends in the song.
struct Heard {
    start: u64,
    end: u64,
    key: u8,
    velocity: u8,
}

/// The notes of a keys clip that start in `from..to`, each ending at the clip's end or at
/// `to` at the latest, as the clip plays them; and how many were cut at `to`.
fn heard(clip: &Clip, from: u64, to: u64) -> (Vec<Heard>, u32) {
    let shown = clip.active_take();
    let zero = clip.zero(shown);
    let mut cut = 0;
    let mut notes = Vec::new();
    for n in &shown.take.notes {
        let on = zero + n.start as i64;
        if on < clip.start as i64 || on >= clip.end as i64 {
            continue;
        }
        let on = on as u64;
        if on < from || on >= to {
            continue;
        }
        let off = (on + n.length).min(clip.end);
        if off > to {
            cut += 1;
        }
        notes.push(Heard {
            start: on,
            end: off.min(to),
            key: n.key,
            velocity: n.velocity,
        });
    }
    (notes, cut)
}

/// The song's tempo in `from..to` as steps: a tick and the tempo from there. A ramp
/// becomes a step each sixteenth note, with the tempo of the step's middle.
fn tempo_steps(timeline: &Timeline, map: &TempoMap, from: u64, to: u64) -> Vec<(u64, f64)> {
    let mut points = vec![(0, timeline.head_ramp)];
    points.extend(timeline.tempo_points.iter().map(|p| (p.tick, p.ramp)));
    let mut steps = Vec::new();
    for (i, &(start, ramp)) in points.iter().enumerate() {
        let next = points.get(i + 1).map(|p| p.0);
        let (a, b) = (start.max(from), next.unwrap_or(u64::MAX).min(to));
        if a >= b {
            continue;
        }
        if !ramp || next.is_none() {
            steps.push((a, map.tempo_at(a as f64)));
            continue;
        }
        let mut t = a;
        while t < b {
            let end = (start + ((t - start) / RAMP_STEP + 1) * RAMP_STEP).min(b);
            steps.push((t, map.tempo_at((t + end) as f64 / 2.0)));
            t = end;
        }
    }
    steps
}

#[cfg(test)]
pub(crate) mod tests;

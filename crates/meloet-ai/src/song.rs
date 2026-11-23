//! What the read tools say about the song, made from the core's copy of it, and the
//! positions the tools take ("bar.beat.tick" from 1, or ticks).

use meloet_core::ai::{AiBusy, AiSong};
use meloet_model::{
    BuiltinInstrument, InstrumentGroup, InstrumentRef, ScaleMode, TICKS_PER_QUARTER, TempoMap,
    Track,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// A preset of an added SF2 file.
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct Sf2Arg {
    /// The file's name, as list_instruments gives it.
    #[schemars(length(min = 1, max = 255))]
    pub file: String,
    /// 0 to 128 (128 holds the drum kits).
    #[schemars(range(min = 0, max = 128))]
    pub bank: u32,
    #[schemars(range(min = 0, max = 127))]
    pub program: u32,
}

const NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// 60 is C4.
pub fn key_name(key: u8) -> String {
    format!("{}{}", NAMES[key as usize % 12], i32::from(key) / 12 - 1)
}

pub fn at(map: &TempoMap, tick: u64) -> String {
    let b = map.bar_beat_tick(tick);
    format!("{}.{}.{}", b.bar, b.beat, b.tick)
}

pub fn place(map: &TempoMap, tick: u64) -> Value {
    json!({ "at": at(map, tick), "tick": tick })
}

/// A place in the song: ticks from its start, or "bar.beat.tick" text like "5.1.0".
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(inline)]
pub enum Place {
    Tick(u64),
    At(String),
}

impl Place {
    pub fn tick(&self, map: &TempoMap) -> Result<u64, String> {
        let tick = match self {
            Place::Tick(tick) => *tick,
            Place::At(text) => parse_at(map, text)?,
        };
        if tick > crate::MAX_TICK {
            return Err(format!("that is past bar {}", crate::MAX_BAR));
        }
        Ok(tick)
    }
}

/// "bar.beat.tick", "bar.beat" or "bar", each from 1 but the tick (from 0, less than a beat).
pub fn parse_at(map: &TempoMap, text: &str) -> Result<u64, String> {
    let bad = || format!("\"{text}\" is not a position: write \"bar.beat.tick\", like \"5.1.0\"");
    let parts: Vec<&str> = text.trim().split('.').collect();
    if parts.is_empty() || parts.len() > 3 {
        return Err(bad());
    }
    let mut n = [1u64, 1, 0];
    for (i, p) in parts.iter().enumerate() {
        n[i] = p.trim().parse().map_err(|_| bad())?;
    }
    let [bar, beat, tick] = n;
    if !(1..=crate::MAX_BAR).contains(&bar) {
        return Err(format!(
            "bar {bar} is out of range (1 to {})",
            crate::MAX_BAR
        ));
    }
    let head = map.tick_of_bar(bar - 1);
    let meter = map.meter_at(head).meter;
    if beat == 0 || beat > u64::from(meter.num()) {
        return Err(format!(
            "bar {bar} is in {}/{}: the beat must be 1 to {}",
            meter.num(),
            meter.den(),
            meter.num()
        ));
    }
    if tick >= meter.ticks_per_beat() {
        return Err(format!(
            "a beat of bar {bar} has {} ticks: the tick must be below that",
            meter.ticks_per_beat()
        ));
    }
    Ok(head + (beat - 1) * meter.ticks_per_beat() + tick)
}

pub fn sound_name(instrument: &InstrumentRef) -> String {
    match instrument {
        InstrumentRef::Builtin(b) => b.name().to_owned(),
        InstrumentRef::Plugin(p) => format!("plugin: {} ({})", p.name, p.key),
        InstrumentRef::Sf2(s) => format!(
            "sf2: {} ({} bank {} program {})",
            s.name, s.file, s.bank, s.program
        ),
    }
}

fn group_name(group: InstrumentGroup) -> &'static str {
    match group {
        InstrumentGroup::PianoKeys => "piano and keys",
        InstrumentGroup::Guitar => "guitar",
        InstrumentGroup::Bass => "bass",
        InstrumentGroup::Drums => "drums",
        InstrumentGroup::PadStrings => "pads and strings",
        InstrumentGroup::Wind => "wind",
        InstrumentGroup::Synth => "synth",
    }
}

pub fn sounds() -> Value {
    let sounds: Vec<Value> = BuiltinInstrument::all()
        .map(|b| json!({ "name": b.name(), "group": group_name(b.group()) }))
        .collect();
    json!({ "sounds": sounds })
}

pub fn busy_name(busy: AiBusy) -> &'static str {
    match busy {
        AiBusy::CountingIn => "counting in",
        AiBusy::Recording => "recording",
        AiBusy::Exporting => "exporting",
    }
}

fn track_value(number: usize, track: &Track, soloed: bool) -> Value {
    json!({
        "number": number + 1,
        "id": crate::trackref::id_text(track.id),
        "name": track.name,
        "kind": if track.kind.is_vocal() { "vocal" } else { "keys" },
        "sound": if track.kind.is_vocal() { None } else { Some(sound_name(&track.instrument)) },
        "muted": track.muted,
        "soloed": soloed,
        "volume_db": track.channel.gain.db(),
    })
}

pub fn overview(read: &AiSong) -> Value {
    let doc = &read.song.doc;
    let session = &read.song.session;
    let map = TempoMap::new(&doc.timeline);
    let timeline = &doc.timeline;
    let mut tempo = vec![json!({
        "at": "1.1.0", "tick": 0, "bpm": timeline.tempo.bpm(), "ramp_to_next": timeline.head_ramp,
    })];
    tempo.extend(timeline.tempo_points.iter().map(|p| {
        json!({ "at": at(&map, p.tick), "tick": p.tick, "bpm": p.tempo.bpm(), "ramp_to_next": p.ramp })
    }));
    let mut meters = vec![json!({
        "bar": 1, "tick": 0, "meter": format!("{}/{}", timeline.meter.num(), timeline.meter.den()),
    })];
    meters.extend(timeline.meter_changes.iter().map(|c| {
        json!({
            "bar": c.bar + 1,
            "tick": map.tick_of_bar(u64::from(c.bar)),
            "meter": format!("{}/{}", c.meter.num(), c.meter.den()),
        })
    }));
    let end = (doc.tracks.iter().flat_map(|t| t.clips.iter()))
        .map(|c| c.end)
        .max()
        .unwrap_or(0);
    let markers: Vec<Value> = (doc.markers.iter())
        .map(|m| json!({ "id": m.id.0, "name": m.name, "at": at(&map, m.tick), "tick": m.tick }))
        .collect();
    let scale = session.roll.scale;
    let mut clips = Vec::new();
    for (i, track) in doc.tracks.iter().enumerate() {
        for clip in &track.clips {
            clips.push(json!({
                "id": clip.id.0,
                "track": i + 1,
                "track_id": crate::trackref::id_text(track.id),
                "name": clip.name,
                "kind": if clip.is_keys() { "keys" } else { "audio" },
                "start": place(&map, clip.start),
                "end": place(&map, clip.end),
                "takes": clip.takes.len(),
                "notes": clip.is_keys().then(|| clip.active_take().take.notes.len()),
            }));
        }
    }
    let tracks: Vec<Value> = (doc.tracks.iter().enumerate())
        .map(|(i, t)| track_value(i, t, session.soloed.contains(&t.id)))
        .collect();
    json!({
        "name": doc.name,
        "ticks_per_quarter": TICKS_PER_QUARTER,
        "tempo": tempo,
        "meters": meters,
        "length": { "end": place(&map, end), "seconds": (map.seconds_at(end as f64) * 100.0).round() / 100.0 },
        "markers": markers,
        "loop": doc.loop_range.map(|r| json!({
            "on": session.loop_on, "start": place(&map, r.start()), "end": place(&map, r.end()),
        })),
        "scale": {
            "root": NAMES[scale.root() as usize],
            "mode": match scale.mode {
                ScaleMode::Major => "major",
                ScaleMode::Minor => "minor",
                ScaleMode::HarmonicMinor => "harmonic_minor",
                ScaleMode::MelodicMinor => "melodic_minor",
                ScaleMode::MajorPentatonic => "major_pentatonic",
                ScaleMode::MinorPentatonic => "minor_pentatonic",
                ScaleMode::Blues => "blues",
                ScaleMode::Dorian => "dorian",
                ScaleMode::Mixolydian => "mixolydian",
            },
            "locked": scale.lock,
        },
        "busy": read.busy.map(busy_name),
        "undo_steps": read.undo_count,
        "file": read.file.as_ref().map(|f| f.display().to_string()),
        "unsaved_changes": read.unsaved,
        "sample_rate": read.sample_rate,
        "tracks": tracks,
        "clips": clips,
    })
}

/// The notes of the take a clip shows, `first .. first + max` of them.
pub fn notes(read: &AiSong, clip: u32, first: usize, max: usize) -> Result<Value, String> {
    let doc = &read.song.doc;
    let map = TempoMap::new(&doc.timeline);
    let found = (doc.tracks.iter().enumerate())
        .find_map(|(i, t)| t.clips.iter().find(|c| c.id.0 == clip).map(|c| (i, c)));
    let Some((track, clip)) = found else {
        return Err(format!(
            "there is no clip {clip}; call song_overview to see the clips"
        ));
    };
    if !clip.is_keys() {
        return Err(format!("clip {} holds audio, not notes", clip.id.0));
    }
    let shown = clip.active_take();
    let zero = clip.zero(shown);
    let all = &shown.take.notes;
    let notes: Vec<Value> = (all.iter().enumerate().skip(first).take(max))
        .map(|(index, n)| {
            let start = zero + n.start as i64;
            let in_clip = start >= clip.start as i64 && start < clip.end as i64;
            let at_text = (start >= 0).then(|| at(&map, start as u64));
            json!({
                "index": index,
                "start": start,
                "at": at_text,
                "length": n.length,
                "key": n.key,
                "name": key_name(n.key),
                "velocity": n.velocity,
                "in_clip": in_clip,
            })
        })
        .collect();
    Ok(json!({
        "clip": clip.id.0,
        "track": track + 1,
        "track_id": crate::trackref::id_text(doc.tracks[track].id),
        "take": shown.take.id.0,
        "start": place(&map, clip.start),
        "end": place(&map, clip.end),
        "total": all.len(),
        "first": first,
        "notes": notes,
    }))
}

#[cfg(test)]
mod tests {
    use meloet_model::{Meter, Tempo, Timeline};

    use super::*;

    #[test]
    fn key_names() {
        assert_eq!(key_name(60), "C4");
        assert_eq!(key_name(61), "C#4");
        assert_eq!(key_name(0), "C-1");
        assert_eq!(key_name(127), "G9");
    }

    #[test]
    fn positions_go_both_ways() {
        let map = TempoMap::new(&Timeline::new(
            Tempo::from_bpm(120.0).unwrap(),
            Meter::FOUR_FOUR,
        ));
        assert_eq!(parse_at(&map, "1.1.0"), Ok(0));
        assert_eq!(parse_at(&map, "2"), Ok(7680));
        assert_eq!(parse_at(&map, "2.3.480"), Ok(7680 + 3840 + 480));
        assert_eq!(at(&map, 7680 + 3840 + 480), "2.3.480");
        assert!(parse_at(&map, "1.5.0").is_err());
        assert!(parse_at(&map, "1.1.1920").is_err());
        assert!(parse_at(&map, "0.1.0").is_err());
        assert!(parse_at(&map, "x").is_err());
    }
}

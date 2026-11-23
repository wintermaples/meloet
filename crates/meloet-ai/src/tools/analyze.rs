//! Listening by numbers: what `measure_mix` and `analyze_bands` share (the range, the tracks
//! of the mix and those rendered alone), and `analyze_bands`, which tells how the sound
//! spreads over frequency bands, where two tracks sound in the same band at the same time,
//! and how the levels move from section to section. Everything is rendered by the export's
//! own engine and heard there (`meloet_core::export::analysis`); nothing changes the song.

use meloet_core::ai::{
    AiAnalysis, AiAnalysisAsk, AiBandLayout, AiLevels, AiLoudness, AiMeasure, AiSong,
};
use meloet_model::{TempoMap, TickRange};
use rmcp::model::{CallToolResult, ProgressNotificationParam, RequestMetaObject};
use rmcp::{
    ErrorData as McpError, Peer, RoleServer, handler::server::wrapper::Parameters, schemars, tool,
    tool_router,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use super::{
    MAX_MEASURE_S, Tools, closing, db, done, failed, levels_value, refusal_text, shown_db,
};
use crate::MAX_BAR;
use crate::song::at;

/// Tracks whose every pair `analyze_bands` compares, at most.
const MAX_COMPARE: usize = 8;
/// Sections, and section rows (sections times renderings), at most.
const MAX_SECTIONS: usize = 32;
const MAX_SECTION_ROWS: usize = 256;
/// `top_band` is a band whose shared level is at least this.
const TOP_BAND_DBFS: f64 = -60.0;

const HOW_TO_READ: &str = "shared_dbfs: what both tracks play in the band at the same moments \
(in each window of window_s seconds the smaller of the two band powers), as a level over the \
range. score: that over the louder track's power in the band; 0 they never sound there \
together, 1 both are as loud there all along. A guide to where two parts may cover each \
other, not a model of hearing (masking): left and right are added, so panning is not seen.";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum BandsArg {
    /// low <120 Hz, low_mid 120-500, mid 500-2k, high_mid 2k-6k, high 6k and up.
    #[default]
    Five,
    /// Octaves centred on 31.5 Hz to 16 kHz.
    Octave,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
pub struct AnalyzeArgs {
    /// The first bar (from 1).
    #[schemars(range(min = 1, max = 9999))]
    pub from_bar: u32,
    /// The last bar, analysed to its end.
    #[schemars(range(min = 1, max = 9999))]
    pub to_bar: u32,
    /// Track numbers (from 1) to analyse, each alone; the mix then holds only these. Leave
    /// out for every track.
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub tracks: Option<Vec<u32>>,
    /// Track numbers to analyse each alone while the mix stays every track, mutes as in the
    /// song. Not together with `tracks`.
    #[serde(default)]
    #[schemars(length(min = 1, max = 64))]
    pub alone: Option<Vec<u32>>,
    /// Analyse each track alone as if it were not muted; the mix keeps the song's mutes.
    #[serde(default)]
    pub ignore_mute: bool,
    /// "five" (default) or "octave".
    #[serde(default)]
    pub bands: BandsArg,
    /// 2 to 8 of the tracks analysed alone; every pair is compared band by band over time.
    /// Default: those tracks when they are 2 to 8.
    #[serde(default)]
    #[schemars(length(min = 2, max = 8))]
    pub compare: Option<Vec<u32>>,
    /// Also give levels for each section between the song's markers.
    #[serde(default)]
    pub sections: bool,
    /// Also cut sections every this many bars.
    #[serde(default)]
    #[schemars(range(min = 1, max = 256))]
    pub every_bars: Option<u32>,
}

/// LUFS as the tools give them.
pub(super) fn lufs(value: Option<f32>) -> Option<f64> {
    value.map(|l| (f64::from(l) * 100.0).round() / 100.0)
}

pub(super) fn shown_lufs(v: &Value) -> String {
    v.as_f64()
        .map_or_else(|| "no".to_owned(), |l| format!("{l:.1}"))
}

/// What a line about a track says of its mute.
pub(super) fn muted_note(muted: bool, ignore_mute: bool) -> &'static str {
    match (muted, ignore_mute) {
        (false, _) => "",
        (true, true) => " (muted in the song, measured as if not)",
        (true, false) => {
            " (muted in the song, so silent here; give ignore_mute: true to measure it as if \
             not)"
        }
    }
}

/// The range and the tracks of `measure_mix` and `analyze_bands`, checked.
pub(super) fn measure_ask(
    read: &AiSong,
    from_bar: u32,
    to_bar: u32,
    tracks: Option<&[u32]>,
    alone: Option<&[u32]>,
    ignore_mute: bool,
) -> Result<AiMeasure, CallToolResult> {
    if from_bar == 0 || to_bar < from_bar || u64::from(to_bar) > MAX_BAR {
        return Err(failed(format!("give 1 <= from_bar <= to_bar <= {MAX_BAR}")));
    }
    for (name, list) in [("tracks", tracks), ("alone", alone)] {
        if list.is_some_and(|t| t.is_empty() || t.len() > 64) {
            return Err(failed(format!(
                "give 1 to 64 track numbers in `{name}`, or leave it out"
            )));
        }
    }
    if tracks.is_some() && alone.is_some() {
        return Err(failed(
            "give `tracks` or `alone`, not both: `tracks` makes the mix of only those tracks \
             and measures each alone; `alone` measures those alone and keeps the whole mix",
        ));
    }
    let map = TempoMap::new(&read.song.doc.timeline);
    let start = map.tick_of_bar(u64::from(from_bar) - 1);
    let end = map.tick_of_bar(u64::from(to_bar));
    let seconds = map.seconds_at(end as f64) - map.seconds_at(start as f64);
    if seconds > MAX_MEASURE_S {
        return Err(failed(format!(
            "that is {seconds:.0} s of the song; measure at most {MAX_MEASURE_S:.0} s at a time"
        )));
    }
    let span = TickRange::new(start, end).ok_or_else(|| failed("the range is empty"))?;
    Ok(AiMeasure {
        span,
        tracks: tracks.map(<[u32]>::to_vec),
        alone: alone.map(<[u32]>::to_vec),
        ignore_mute,
        analysis: None,
    })
}

/// How the mix of `ask` is made, in words.
pub(super) fn mix_text(ask: &AiMeasure) -> String {
    match &ask.tracks {
        None => "all tracks, mutes as in the song".to_owned(),
        Some(tracks) => {
            let numbers: Vec<String> = tracks.iter().map(u32::to_string).collect();
            format!("only tracks {}, mutes as in the song", numbers.join(", "))
        }
    }
}

/// The numbers of the tracks `ask` renders alone, as the core will.
fn alone_of(read: &AiSong, ask: &AiMeasure) -> Vec<u32> {
    let mut numbers = match (&ask.alone, &ask.tracks) {
        (Some(n), _) | (None, Some(n)) => n.clone(),
        (None, None) => (1..=read.song.doc.tracks.len() as u32).collect(),
    };
    numbers.sort_unstable();
    numbers.dedup();
    numbers
}

fn loudness_value(l: &AiLoudness) -> Value {
    json!({
        "peak_dbfs": db(l.peak),
        "rms_dbfs": db(l.rms),
        "lufs": lufs(l.lufs),
        "short_term_max_lufs": lufs(l.lufs_short_max),
    })
}

fn power_db(power: f64) -> Option<f64> {
    (power > 0.0).then(|| (10.0 * power.log10() * 100.0).round() / 100.0)
}

fn bands_value(a: &AiAnalysis, pass: usize) -> Value {
    let levels = a.levels.get(pass).map_or(&[][..], Vec::as_slice);
    (a.bands.iter().zip(levels))
        .map(|(b, l)| {
            json!({
                "band": b.name,
                "dbfs": power_db(l.power),
                "share_percent": (l.share * 1000.0).round() / 10.0,
            })
        })
        .collect()
}

fn shown_bands(v: &Value) -> String {
    let parts: Vec<String> = (v.as_array().into_iter().flatten())
        .map(|b| {
            format!(
                "{} {} ({}%)",
                b["band"].as_str().unwrap_or(""),
                b["dbfs"]
                    .as_f64()
                    .map_or_else(|| "silent".to_owned(), |d| format!("{d:.1}")),
                b["share_percent"]
            )
        })
        .collect();
    parts.join(", ")
}

impl Tools {
    /// Measures `ask` on a thread of the blocking pool, telling the progress when a
    /// `progressToken` came.
    pub(super) async fn run_measure(
        &self,
        ask: AiMeasure,
        meta: &RequestMetaObject,
        peer: &Peer<RoleServer>,
        ct: &CancellationToken,
    ) -> Result<AiLevels, CallToolResult> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<u8>();
        let meloet = self.meloet.clone();
        let stop = ct.clone();
        let mut job = tokio::task::spawn_blocking(move || {
            let mut progress = |percent| {
                let _ = tx.send(percent);
            };
            meloet.measure(ask, &mut progress, &|| stop.is_cancelled())
        });
        let token = meta.get_progress_token();
        let mut last = None;
        let answer = loop {
            tokio::select! {
                answer = &mut job => break answer,
                Some(percent) = rx.recv() => {
                    if let Some(token) = &token
                        && last.is_none_or(|l| percent > l)
                    {
                        last = Some(percent);
                        let note = ProgressNotificationParam::new(token.clone(), f64::from(percent))
                            .with_total(100.0);
                        let _ = peer.notify_progress(note).await;
                    }
                }
            }
        };
        match answer {
            Ok(Ok(levels)) => Ok(levels),
            Ok(Err(r)) => Err(failed(format!("Could not measure: {}.", refusal_text(&r)))),
            Err(_) => Err(closing()),
        }
    }
}

/// Where sections begin after the range's start: at markers, every `every` bars, or both.
fn cuts(
    read: &AiSong,
    map: &TempoMap,
    span: TickRange,
    from_bar: u32,
    markers: bool,
    every: Option<u32>,
) -> Vec<u64> {
    let inside = |t: &u64| *t > span.start() && *t < span.end();
    let mut cuts: Vec<u64> = Vec::new();
    if markers {
        cuts.extend(read.song.doc.markers.iter().map(|m| m.tick).filter(inside));
    }
    if let Some(every) = every.filter(|e| *e > 0) {
        let bars =
            (1u64..).map(|k| map.tick_of_bar(u64::from(from_bar) - 1 + k * u64::from(every)));
        cuts.extend(bars.take_while(|t| *t < span.end()).filter(inside));
    }
    cuts.sort_unstable();
    cuts.dedup();
    cuts
}

/// The name of the last marker at or before `tick`.
fn marker_at(read: &AiSong, tick: u64) -> Option<&str> {
    (read.song.doc.markers.iter())
        .filter(|m| m.tick <= tick)
        .max_by_key(|m| m.tick)
        .map(|m| m.name.as_str())
}

#[tool_router(router = analyze_router, vis = "pub(super)")]
impl Tools {
    /// Renders bars like measure_mix and gives, for the mix and each track alone, the level
    /// in frequency bands (dBFS and share of the energy) and LUFS; how much pairs of tracks
    /// sound in the same band at the same time; and levels per section (markers, or every N
    /// bars). Numbers only; changes nothing.
    #[tool(annotations(
        title = "Analyze bands",
        read_only_hint = true,
        open_world_hint = false
    ))]
    async fn analyze_bands(
        &self,
        Parameters(a): Parameters<AnalyzeArgs>,
        meta: RequestMetaObject,
        peer: Peer<RoleServer>,
        ct: CancellationToken,
    ) -> Result<CallToolResult, McpError> {
        check!(self.allowed());
        let read = check!(self.read().await);
        let mut ask = check!(measure_ask(
            &read,
            a.from_bar,
            a.to_bar,
            a.tracks.as_deref(),
            a.alone.as_deref(),
            a.ignore_mute,
        ));
        let alone = alone_of(&read, &ask);
        let compare = match &a.compare {
            Some(compare) => {
                let mut compare = compare.clone();
                compare.sort_unstable();
                compare.dedup();
                if !(2..=MAX_COMPARE).contains(&compare.len()) {
                    return Ok(failed(format!(
                        "give 2 to {MAX_COMPARE} different tracks in `compare`"
                    )));
                }
                if let Some(n) = compare.iter().find(|n| !alone.contains(n)) {
                    return Ok(failed(format!(
                        "track {n} is not analysed alone; `compare` takes tracks of `tracks` \
                         or `alone` (every track when both are left out)"
                    )));
                }
                compare
            }
            None if (2..=MAX_COMPARE).contains(&alone.len()) => alone.clone(),
            None => Vec::new(),
        };
        let map = TempoMap::new(&read.song.doc.timeline);
        let span = ask.span;
        let cuts = cuts(&read, &map, span, a.from_bar, a.sections, a.every_bars);
        let asked_sections = a.sections || a.every_bars.is_some();
        let count = cuts.len() + 1;
        if asked_sections && count > MAX_SECTIONS {
            return Ok(failed(format!(
                "that is {count} sections; at most {MAX_SECTIONS} (cut every more bars, or a \
                 shorter range)"
            )));
        }
        let rows = count * (alone.len() + 1);
        if asked_sections && rows > MAX_SECTION_ROWS {
            return Ok(failed(format!(
                "that is {rows} section rows (sections times the mix and each track); at most \
                 {MAX_SECTION_ROWS}: ask for fewer tracks or fewer sections"
            )));
        }
        ask.analysis = Some(AiAnalysisAsk {
            bands: match a.bands {
                BandsArg::Five => AiBandLayout::Five,
                BandsArg::Octave => AiBandLayout::Octave,
            },
            compare: compare.clone(),
            // Without sections asked for, the range is one section: none are given.
            cuts: if asked_sections { cuts } else { Vec::new() },
        });
        let mix = mix_text(&ask);
        let levels = check!(self.run_measure(ask, &meta, &peer, &ct).await);
        let Some(analysis) = &levels.analysis else {
            return Ok(closing());
        };
        let rate = levels.sample_rate;

        let pass_value = |pass: usize, l: &meloet_core::ai::PassLevels| {
            let mut v = levels_value(l, rate);
            v["bands"] = bands_value(analysis, pass);
            v
        };
        let master = pass_value(0, &levels.master);
        let tracks: Vec<Value> = (levels.tracks.iter().enumerate())
            .map(|(i, (n, name, l))| {
                let mut v = pass_value(i + 1, l);
                v["number"] = json!(n);
                v["name"] = json!(name);
                v["muted"] = json!(levels.muted.contains(n));
                v
            })
            .collect();
        let number = |i: usize| levels.tracks.get(i).map(|t| t.0);
        let pairs: Vec<Value> = (analysis.overlaps.iter())
            .map(|o| {
                let bands: Vec<Value> = (analysis.bands.iter().zip(&o.bands))
                    .map(|(b, s)| {
                        json!({
                            "band": b.name,
                            "shared_dbfs": power_db(s.power),
                            "score": (s.score * 100.0).round() / 100.0,
                        })
                    })
                    .collect();
                let top = (analysis.bands.iter().zip(&o.bands))
                    .filter(|(_, s)| power_db(s.power).is_some_and(|d| d >= TOP_BAND_DBFS))
                    .max_by(|x, y| x.1.power.total_cmp(&y.1.power))
                    .map(|(b, _)| b.name);
                json!({ "a": number(o.a), "b": number(o.b), "top_band": top, "bands": bands })
            })
            .collect();
        let overlap = (!pairs.is_empty()).then(|| {
            json!({
                "how_to_read": HOW_TO_READ,
                "window_s": (analysis.window_s * 1000.0).round() / 1000.0,
                "top_band": format!("the band with the highest shared_dbfs, when it is at \
                                     least {TOP_BAND_DBFS} dBFS"),
                "pairs": pairs,
            })
        });
        let sections: Vec<Value> = (analysis.sections.iter())
            .map(|s| {
                let seconds = map.seconds_at(s.end as f64) - map.seconds_at(s.start as f64);
                let tracks: Vec<Value> = (levels.tracks.iter().zip(s.passes.iter().skip(1)))
                    .map(|((n, ..), l)| {
                        let mut v = loudness_value(l);
                        v["number"] = json!(n);
                        v
                    })
                    .collect();
                json!({
                    "from": at(&map, s.start),
                    "to": at(&map, s.end),
                    "marker": marker_at(&read, s.start),
                    "seconds": (seconds * 100.0).round() / 100.0,
                    "master": s.passes.first().map(loudness_value),
                    "tracks": tracks,
                })
            })
            .collect();
        let band_edges: Vec<Value> = (analysis.bands.iter())
            .map(|b| {
                json!({ "name": b.name, "low_hz": b.low_hz.round(), "high_hz": b.high_hz.round() })
            })
            .collect();
        let window = (rate > 0).then(|| (analysis.window_s * f64::from(rate)).round());
        let method = format!(
            "Welch: Hann windows of {} frames ({:.3} s), half overlapping; the powers of left \
             and right are averaged, so a band's dBFS is on the scale of rms_dbfs; \
             share_percent is of the rendering's whole power (all frequencies).",
            window.unwrap_or(0.0),
            analysis.window_s
        );

        let shown_edges: Vec<String> = (analysis.bands.iter())
            .map(|b| format!("{} {:.0}-{:.0} Hz", b.name, b.low_hz, b.high_hz))
            .collect();
        let mut lines = vec![
            format!(
                "Bars {} to {} (mix: {mix}; solo ignored). Bands: {}.",
                a.from_bar,
                a.to_bar,
                shown_edges.join(", ")
            ),
            format!(
                "Mix: RMS {}, {} LUFS; {}.",
                shown_db(&master["rms_dbfs"]),
                shown_lufs(&master["lufs"]),
                shown_bands(&master["bands"])
            ),
        ];
        for t in &tracks {
            lines.push(format!(
                "Track {} \"{}\"{}: RMS {}, {} LUFS; {}.",
                t["number"],
                t["name"].as_str().unwrap_or(""),
                muted_note(t["muted"] == true, a.ignore_mute),
                shown_db(&t["rms_dbfs"]),
                shown_lufs(&t["lufs"]),
                shown_bands(&t["bands"])
            ));
        }
        if let Some(overlap) = &overlap {
            for p in overlap["pairs"].as_array().into_iter().flatten() {
                let top = p["top_band"].as_str();
                let band = (p["bands"].as_array().into_iter().flatten())
                    .find(|b| top.is_some_and(|t| b["band"] == t));
                lines.push(match band {
                    Some(b) => format!(
                        "Tracks {} and {} share most in {} (score {}, {} dBFS).",
                        p["a"],
                        p["b"],
                        b["band"].as_str().unwrap_or(""),
                        b["score"],
                        b["shared_dbfs"]
                    ),
                    None => format!(
                        "Tracks {} and {} share no band above {TOP_BAND_DBFS} dBFS.",
                        p["a"], p["b"]
                    ),
                });
            }
        } else if compare.is_empty() && alone.len() > MAX_COMPARE {
            lines.push(format!(
                "No pairs were compared: give `compare` with 2 to {MAX_COMPARE} tracks."
            ));
        }
        if asked_sections {
            lines.push(format!("{count} sections; see `sections`."));
        }
        let mut value = json!({
            "from_bar": a.from_bar,
            "to_bar": a.to_bar,
            "sample_rate": rate,
            "mix": mix,
            "solo_ignored": true,
            "ignore_mute": a.ignore_mute,
            "bands": band_edges,
            "method": method,
            "master": master,
            "tracks": tracks,
            "overlap": overlap,
        });
        if asked_sections {
            value["sections"] = json!(sections);
        }
        lines.push(value.to_string());
        Ok(done(lines.join("\n"), value))
    }
}

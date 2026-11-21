//! The table the screen's copy of [`FileClock`] is checked against
//! (`ui/src/components/arrange/follow.ts`, `ui/tests/arrange.follow.test.ts`): where in its
//! file a vocal clip is at a tick of the song, following the tempo and not.

use std::path::PathBuf;

use meloet_model::{
    FileClock, Meter, SourceTempo, TICKS_PER_QUARTER, Tempo, TempoMap, TempoPoint, Timeline,
};

const Q: u64 = TICKS_PER_QUARTER;
const BAR: u64 = 4 * Q;

fn tempo(centi: u32) -> Tempo {
    Tempo::from_centi(centi).unwrap()
}

fn point(tick: u64, centi: u32, ramp: bool) -> TempoPoint {
    TempoPoint {
        tick,
        tempo: tempo(centi),
        ramp,
    }
}

fn song(centi: u32, head_ramp: bool, points: Vec<TempoPoint>) -> Timeline {
    Timeline {
        head_ramp,
        tempo_points: points,
        ..Timeline::new(tempo(centi), Meter::FOUR_FOUR)
    }
}

fn source(centi: u32, head_ramp: bool, points: Vec<TempoPoint>) -> SourceTempo {
    SourceTempo {
        tempo: tempo(centi),
        head_ramp,
        points,
    }
}

struct Case {
    name: &'static str,
    song: Timeline,
    start: u64,
    end: u64,
    take_offset: i64,
    first_frame: u64,
    sample_rate: u32,
    /// `Some`: the clip follows.
    source: Option<SourceTempo>,
}

fn cases() -> Vec<Case> {
    // 96, a ramp from bar 5 (104) to bar 9 (120): the spec's example.
    let ramped = || {
        song(
            9_600,
            false,
            vec![point(4 * BAR, 10_400, true), point(8 * BAR, 12_000, false)],
        )
    };
    vec![
        Case {
            name: "does not follow, one tempo",
            song: song(12_000, false, Vec::new()),
            start: BAR,
            end: 3 * BAR,
            take_offset: 0,
            first_frame: 0,
            sample_rate: 44_100,
            source: None,
        },
        Case {
            name: "does not follow, over a ramp, the right half of a cut clip",
            song: ramped(),
            start: 6 * BAR,
            end: 8 * BAR,
            take_offset: -2 * BAR as i64,
            first_frame: 12_000,
            sample_rate: 48_000,
            source: None,
        },
        Case {
            name: "follows: sung at 96, the song at 120 (x1.25)",
            song: song(12_000, false, Vec::new()),
            start: 4 * BAR,
            end: 6 * BAR,
            take_offset: 0,
            first_frame: 0,
            sample_rate: 48_000,
            source: Some(source(9_600, false, Vec::new())),
        },
        Case {
            name: "follows: one tempo of its own (90) through the song's ramp",
            song: ramped(),
            start: 3 * BAR,
            end: 10 * BAR,
            take_offset: 0,
            first_frame: 4_410,
            sample_rate: 44_100,
            source: Some(source(9_000, false, Vec::new())),
        },
        Case {
            name: "follows: recorded in this very map from bar 4 (no stretching)",
            song: ramped(),
            start: 3 * BAR,
            end: 10 * BAR,
            take_offset: 0,
            first_frame: 960,
            sample_rate: 48_000,
            source: Some(SourceTempo::from_timeline(&ramped(), 3 * BAR)),
        },
        Case {
            name: "follows: ramps in both, the take's tick 0 before the clip and the song",
            song: song(
                9_600,
                true,
                vec![
                    point(4 * BAR, 14_400, false),
                    point(6 * BAR, 8_000, true),
                    point(7 * BAR + Q, 13_000, false),
                ],
            ),
            start: BAR,
            end: 9 * BAR,
            take_offset: -(2 * BAR as i64) - 333,
            first_frame: 100_000,
            sample_rate: 96_000,
            source: Some(source(
                12_000,
                true,
                vec![
                    point(2 * BAR, 9_000, false),
                    point(5 * BAR, 9_000, true),
                    point(8 * BAR, 15_000, false),
                ],
            )),
        },
    ]
}

fn points_json(points: &[TempoPoint]) -> String {
    let rows: Vec<String> = (points.iter())
        .map(|p| {
            format!(
                "{{\"tick\": {}, \"tempoBpm\": {:?}, \"ramp\": {}}}",
                p.tick,
                p.tempo.bpm(),
                p.ramp
            )
        })
        .collect();
    format!("[{}]", rows.join(", "))
}

fn table_json() -> String {
    let mut out = String::from(
        "{\n  \"note\": \"Written by `cargo test -p meloet-model --test follow`. Do not edit \
         this file manually. song: as the section `timeline`. clip: of `ClipView` and its \
         `audio`; sourceTempo is null for a clip that does not follow. rows: a tick of the \
         song, the frame of the file there, the song's tempo over the take's there (null \
         when it does not follow).\",\n  \"cases\": [\n",
    );
    let cases = cases();
    for (i, case) in cases.iter().enumerate() {
        assert_eq!(case.song.check(), Ok(()));
        let map = TempoMap::new(&case.song);
        let zero = case.start as i64 + case.take_offset;
        let clock = FileClock::new(
            zero,
            case.first_frame as f64,
            f64::from(case.sample_rate),
            case.source.as_ref(),
        );
        assert_eq!(clock.follows(), case.source.is_some());
        let (start, end) = (case.start as f64, case.end as f64);
        let mut ticks = vec![
            start,
            start + 0.5,
            start + 1.0,
            end - 0.25,
            end,
            zero as f64,
            zero as f64 - Q as f64,
            end + BAR as f64,
        ];
        ticks.extend((1..40).map(|k| start + (end - start) * f64::from(k) / 40.0));
        // Every place the tempo of either may turn, and a hair to each side of it.
        let own =
            (case.source.iter()).flat_map(|s| s.points.iter().map(|p| zero as f64 + p.tick as f64));
        let turns: Vec<f64> = (case.song.tempo_points.iter().map(|p| p.tick as f64))
            .chain(own)
            .collect();
        for turn in turns {
            ticks.extend([turn - 0.001, turn, turn + 0.001]);
        }
        let rows: Vec<String> = (ticks.iter())
            .map(|&tick| {
                let ratio = clock.ratio_at(&map, tick);
                format!(
                    "        [{tick:?}, {:?}, {}]",
                    clock.frame_at(&map, tick),
                    ratio.map_or("null".to_owned(), |r| format!("{r:?}"))
                )
            })
            .collect();
        let source = case.source.as_ref().map_or("null".to_owned(), |s| {
            format!(
                "{{\"tempoBpm\": {:?}, \"headRamp\": {}, \"tempoPoints\": {}}}",
                s.tempo.bpm(),
                s.head_ramp,
                points_json(&s.points)
            )
        });
        out.push_str(&format!(
            "    {{\n      \"name\": \"{}\",\n      \"song\": {{\"tempoBpm\": {:?}, \
             \"meterNum\": 4, \"meterDen\": 4, \"headRamp\": {}, \"tempoPoints\": {}, \
             \"meterChanges\": []}},\n      \"clip\": {{\"start\": {}, \"end\": {}, \
             \"takeOffset\": {}, \"firstFrame\": {}, \"sampleRate\": {}, \"sourceTempo\": {}}},\n      \
             \"rows\": [\n{}\n      ]\n    }}{}\n",
            case.name,
            case.song.tempo.bpm(),
            case.song.head_ramp,
            points_json(&case.song.tempo_points),
            case.start,
            case.end,
            case.take_offset,
            case.first_frame,
            case.sample_rate,
            source,
            rows.join(",\n"),
            if i + 1 < cases.len() { "," } else { "" }
        ));
    }
    out.push_str("  ]\n}\n");
    out
}

/// Writes `ui/src/components/arrange/follow.table.json`, as `tests/tempo_map.rs` writes its
/// table: one that is not the committed one shows up in `git status`.
#[test]
fn write_the_table_for_the_screen() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../ui/src/components/arrange/follow.table.json");
    let table = table_json();
    if std::fs::read_to_string(&path).ok().as_deref() != Some(table.as_str()) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &table).unwrap();
    }
    assert!(!table.contains("NaN") && !table.contains("inf"));
}

/// The case recorded in the song's own map: following it and not are the same place, to a
/// thousandth of a frame.
#[test]
fn recorded_in_the_map_is_the_same_followed_or_not() {
    let case = cases()
        .into_iter()
        .find(|c| c.name.contains("recorded in this very map"))
        .unwrap();
    let map = TempoMap::new(&case.song);
    let zero = case.start as i64 + case.take_offset;
    let (first, rate) = (case.first_frame as f64, f64::from(case.sample_rate));
    let follow = FileClock::new(zero, first, rate, case.source.as_ref());
    let free = FileClock::new(zero, first, rate, None);
    for tick in (case.start..=case.end).step_by(77) {
        let (a, b) = (
            follow.frame_at(&map, tick as f64),
            free.frame_at(&map, tick as f64),
        );
        assert!((a - b).abs() < 1e-3, "{tick}: {a} {b}");
    }
}

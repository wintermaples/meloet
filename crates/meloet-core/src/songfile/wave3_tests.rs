//! Round 12, waves 3 and 4 in the song file (`wave3`): the arpeggio, the knobs, a comp, a
//! tune and the master's lane come back; a song without them reads with the defaults; what is
//! not understood reads as the default.

use meloet_model::{Arp, ArpRate, ArpShape, CompPiece, SoundKnob, SoundKnobs, Tune};
use serde_json::{Value, json};

use super::vocal_tests::vocal_song;
use super::*;

const BAR: u64 = 4 * meloet_model::TICKS_PER_QUARTER;

/// `vocal_song` with every field of waves 3 and 4 away from its default.
fn with_all() -> Song {
    let mut song = vocal_song();
    let piano = Arc::make_mut(&mut song.doc.tracks[0]);
    piano.arp = Arp {
        on: true,
        shape: ArpShape::UpDown,
        rate: ArpRate::Eighth,
        triplet: true,
        octaves: 2,
        hold: true,
    };
    piano.knobs = (SoundKnobs::default().with(SoundKnob::Brightness, 62.5))
        .and_then(|k| k.with(SoundKnob::Vibrato, 30.0))
        .unwrap();
    let vocal = Arc::make_mut(&mut song.doc.tracks[1]);
    let clip = Arc::make_mut(&mut vocal.clips[0]);
    clip.comp = vec![
        CompPiece { from: 0, take: 1 },
        CompPiece {
            from: 2 * BAR,
            take: 0,
        },
    ];
    let ClipTakes::Vocal { shape, .. } = &mut clip.takes else {
        panic!("a vocal clip");
    };
    shape.tune = Some(Tune {
        root: 9,
        mode: ScaleMode::Minor,
        strength: 60,
    });
    song.doc.master_lanes = vec![Arc::new(Lane {
        id: LaneId(song.doc.next_id),
        target: AutoTarget::Volume,
        step: false,
        points: vec![
            AutoPoint {
                tick: 0,
                value: 0.75,
            },
            AutoPoint {
                tick: 4 * BAR,
                value: 0.0,
            },
        ]
        .into(),
        resting: true,
    })];
    song.doc.next_id += 1;
    song
}

/// What a lane holds but its id, which is given when the song is read.
type Held = (AutoTarget, bool, bool, Vec<(u64, f32)>);

fn master_lanes(song: &Song) -> Vec<Held> {
    (song.doc.master_lanes.iter())
        .map(|l| {
            let points = l.points.iter().map(|p| (p.tick, p.value)).collect();
            (l.target, l.step, l.resting, points)
        })
        .collect()
}

fn without_master_lanes(song: &Song) -> Song {
    let mut song = song.clone();
    song.doc.master_lanes.clear();
    song.doc.next_id = 0;
    song
}

fn json_of(song: &Song) -> Value {
    serde_json::from_slice(&encode(song)).unwrap()
}

fn read(v: &Value) -> Option<Song> {
    decode(&serde_json::to_vec(v).unwrap()).ok()
}

#[test]
fn the_fields_of_waves_3_and_4_come_back() {
    let song = with_all();
    let v = json_of(&song);
    assert_eq!(v["version"], 7);
    let tracks = &v["song"]["tracks"];
    assert_eq!(
        tracks[0]["arp"],
        json!({"on": true, "shape": "upDown", "rate": 8, "triplet": true, "octaves": 2,
               "hold": true})
    );
    assert_eq!(
        tracks[0]["knobs"],
        json!({"brightness": 62.5, "attack": 50.0, "release": 50.0, "vibrato": 30.0})
    );
    let clip = &tracks[1]["clips"][0];
    assert_eq!(clip["comp"], json!([[0, 1], [2 * BAR, 0]]));
    assert_eq!(
        clip["shape"]["tune"],
        json!({"root": 9, "mode": "minor", "strength": 60})
    );
    assert_eq!(
        v["song"]["mixer"]["master"]["automation"],
        json!([{"target": {"kind": "volume"}, "step": false, "resting": true,
                "points": [[0, 0.75], [4 * BAR, 0.0]]}])
    );
    let back = decode(&encode(&song)).unwrap();
    assert_eq!(master_lanes(&back), master_lanes(&song));
    assert_eq!(without_master_lanes(&back), without_master_lanes(&song));
}

#[test]
fn a_song_without_them_is_written_as_before_and_reads_with_the_defaults() {
    let song = vocal_song();
    let v = json_of(&song);
    for track in v["song"]["tracks"].as_array().unwrap() {
        assert_eq!((track.get("arp"), track.get("knobs")), (None, None));
        for clip in track["clips"].as_array().unwrap() {
            assert_eq!(clip.get("comp"), None);
            assert_eq!(clip["shape"].get("tune"), None);
        }
    }
    assert_eq!(v["song"]["mixer"]["master"].get("automation"), None);
    let back = read(&v).unwrap();
    assert_eq!(back, song);
    let piano = &back.doc.tracks[0];
    assert_eq!(
        (piano.arp, piano.knobs),
        (Arp::default(), SoundKnobs::default())
    );
    assert!(back.doc.master_lanes.is_empty());
}

#[test]
fn what_is_not_understood_reads_as_the_default() {
    let mut v = json_of(&with_all());
    let piano = &mut v["song"]["tracks"][0];
    piano["arp"] = json!({"on": true, "shape": "spiral", "rate": 12, "octaves": 9});
    piano["knobs"] = json!({"brightness": 140.0, "vibrato": -5.0});
    v["song"]["tracks"][1]["clips"][0]["shape"]["tune"] = json!({"root": 12, "mode": "minor",
                                                               "strength": 60});
    let song = read(&v).unwrap();
    let piano = &song.doc.tracks[0];
    assert_eq!(
        piano.arp,
        Arp {
            on: true,
            ..Arp::default()
        }
    );
    assert_eq!(
        (
            piano.knobs.get(SoundKnob::Brightness),
            piano.knobs.get(SoundKnob::Vibrato)
        ),
        (100.0, 0.0)
    );
    let clip = &song.doc.tracks[1].clips[0];
    assert_eq!(clip.takes.shape().unwrap().tune, None);

    // A comp that cannot be is left out; the clip plays its shown take.
    for comp in [
        json!([[960, 1]]),
        json!([[0, 1], [0, 0]]),
        json!([[0, 1], [3840, 1]]),
        json!([[0, 2]]),
        json!([[0, 0], [99_999_999, 1]]),
        json!("all of take 2"),
    ] {
        let mut v = json_of(&with_all());
        v["song"]["tracks"][1]["clips"][0]["comp"] = comp.clone();
        let song = read(&v).unwrap();
        assert!(song.doc.tracks[1].clips[0].comp.is_empty(), "{comp}");
    }

    // The master has a lane of its volume only: another is skipped, two of the volume break
    // the song as two lanes of a track do.
    let mut v = json_of(&with_all());
    v["song"]["mixer"]["master"]["automation"][0]["target"] = json!({"kind": "pan"});
    assert!(read(&v).unwrap().doc.master_lanes.is_empty());
    let mut v = json_of(&with_all());
    let lane = v["song"]["mixer"]["master"]["automation"][0].clone();
    v["song"]["mixer"]["master"]["automation"] = json!([lane.clone(), lane]);
    assert!(read(&v).is_none());
    let mut v = json_of(&with_all());
    v["song"]["mixer"]["master"]["automation"][0]["points"] = json!([[0, 1.5]]);
    assert!(read(&v).is_none());
}

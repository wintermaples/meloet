//! Listening by numbers through the socket: `measure_mix` with tracks alone and mutes
//! ignored, and `analyze_bands` (bands, overlap, sections), with sounds made by computation.

mod common;
use common::*;

use std::time::Instant;

use serde_json::{Value, json};

/// A note of a whole 4/4 bar from the head of `bar`.
fn bar_note(bar: u32, key: u32) -> Value {
    json!({ "at": format!("{bar}.1.0"), "length": 7680, "key": key, "velocity": 100 })
}

/// Adds a track of `sound` that holds `key` through bars 1 to `bars`. Returns its number.
fn track_playing(c: &mut Client, sound: &str, key: u32, bars: u32) -> u32 {
    let added = ok(c, "add_track", json!({ "sound": sound }));
    let track = added["track"].as_u64().unwrap() as u32;
    let notes: Vec<Value> = (1..=bars).map(|b| bar_note(b, key)).collect();
    ok(c, "write_notes", json!({ "track": track, "notes": notes }));
    track
}

/// The band of a rendering's `bands` with the largest share.
fn loudest_band(pass: &Value) -> String {
    let bands = pass["bands"].as_array().unwrap();
    let most = (bands.iter())
        .max_by(|a, b| {
            let share = |v: &Value| v["share_percent"].as_f64().unwrap();
            share(a).total_cmp(&share(b))
        })
        .unwrap();
    most["band"].as_str().unwrap().to_owned()
}

fn pair(answer: &Value, a: u32, b: u32) -> &Value {
    (answer["overlap"]["pairs"].as_array().unwrap().iter())
        .find(|p| p["a"] == a && p["b"] == b)
        .unwrap_or_else(|| panic!("no pair {a} {b}: {answer}"))
}

fn score(pair: &Value, band: &str) -> f64 {
    (pair["bands"].as_array().unwrap().iter())
        .find(|b| b["band"] == band)
        .and_then(|b| b["score"].as_f64())
        .unwrap()
}

#[test]
fn a_muted_track_is_measured_alone_as_if_it_sounded_and_the_mix_keeps_its_mutes() {
    let r = running("analyze-mute");
    let mut c = Client::start(&r.socket);
    let quiet = track_playing(&mut c, "sine", 69, 1);
    let loud = track_playing(&mut c, "sine", 57, 1);
    ok(&mut c, "set_mute", json!({ "track": quiet, "on": true }));
    let before = r.core.ai_song().unwrap();

    let kept = ok(
        &mut c,
        "measure_mix",
        json!({ "from_bar": 1, "to_bar": 1, "alone": [quiet] }),
    );
    assert_eq!(kept["mix"], "all tracks, mutes as in the song", "{kept}");
    assert_eq!(kept["solo_ignored"], true);
    assert_eq!(kept["tracks"].as_array().unwrap().len(), 1);
    assert_eq!(kept["tracks"][0]["number"], quiet);
    assert_eq!(kept["tracks"][0]["muted"], true);
    assert_eq!(kept["tracks"][0]["peak_dbfs"], Value::Null, "{kept}");

    let ignored = ok(
        &mut c,
        "measure_mix",
        json!({ "from_bar": 1, "to_bar": 1, "alone": [quiet, loud], "ignore_mute": true }),
    );
    let sounding = ignored["tracks"][0]["peak_dbfs"].as_f64();
    assert!(sounding.is_some_and(|p| p < 0.0), "{ignored}");
    assert!(ignored["tracks"][0]["lufs"].as_f64().is_some(), "{ignored}");
    assert_eq!(ignored["tracks"][1]["muted"], false);
    // The mix is the same with the mute kept: the muted track is not in it.
    assert_eq!(ignored["master"], kept["master"]);
    let alone_loud = &ignored["tracks"][1];
    assert_eq!(ignored["master"]["rms_dbfs"], alone_loud["rms_dbfs"]);

    // `tracks` makes the mix of those only.
    let only = ok(
        &mut c,
        "measure_mix",
        json!({ "from_bar": 1, "to_bar": 1, "tracks": [quiet], "ignore_mute": true }),
    );
    assert_eq!(only["mix"], "only tracks 2, mutes as in the song", "{only}");
    assert_eq!(only["master"]["peak_dbfs"], Value::Null, "{only}");
    assert!(only["tracks"][0]["peak_dbfs"].as_f64().is_some(), "{only}");
    let both = refused(
        &mut c,
        "measure_mix",
        json!({ "from_bar": 1, "to_bar": 1, "tracks": [quiet], "alone": [loud] }),
    );
    assert!(both.contains("not both"), "{both}");

    // Measuring changes nothing.
    let after = r.core.ai_song().unwrap();
    assert_eq!(after.undo_count, before.undo_count);
    assert_eq!(after.song.doc, before.song.doc);
}

#[test]
fn a_bass_and_a_high_track_land_in_different_bands() {
    let r = running("analyze-bands");
    let mut c = Client::start(&r.socket);
    let bass = track_playing(&mut c, "synthBass", 36, 2);
    let high = track_playing(&mut c, "sine", 96, 2);
    let answer = ok(
        &mut c,
        "analyze_bands",
        json!({ "from_bar": 1, "to_bar": 2, "alone": [bass, high] }),
    );
    let tracks = answer["tracks"].as_array().unwrap();
    assert_eq!(loudest_band(&tracks[0]), "low", "{answer}");
    assert_eq!(loudest_band(&tracks[1]), "high_mid", "{answer}");
    let shares: f64 = (tracks[1]["bands"].as_array().unwrap().iter())
        .map(|b| b["share_percent"].as_f64().unwrap())
        .sum();
    assert!((shares - 100.0).abs() < 0.5, "{answer}");
    let edges = answer["bands"].as_array().unwrap();
    assert_eq!(edges.len(), 5);
    assert_eq!(edges[0]["high_hz"], 120.0);
    assert!(answer["master"]["lufs"].as_f64().is_some(), "{answer}");
    let apart = pair(&answer, bass, high);
    assert!(
        (apart["bands"].as_array().unwrap().iter()).all(|b| b["score"].as_f64().unwrap() < 0.1),
        "{answer}"
    );
    assert!(answer["overlap"]["how_to_read"].as_str().unwrap().len() > 50);

    let octaves = ok(
        &mut c,
        "analyze_bands",
        json!({ "from_bar": 1, "to_bar": 2, "alone": [high], "bands": "octave" }),
    );
    // C7 is 2093 Hz: the octave of 2 kHz (1413 to 2818 Hz).
    assert_eq!(loudest_band(&octaves["tracks"][0]), "2kHz", "{octaves}");
    assert_eq!(octaves["overlap"], Value::Null);
}

#[test]
fn two_tracks_that_play_the_same_notes_overlap_more_than_two_far_apart() {
    let r = running("analyze-overlap");
    let mut c = Client::start(&r.socket);
    let first = track_playing(&mut c, "sine", 69, 2);
    let same = track_playing(&mut c, "sine", 69, 2);
    let far = track_playing(&mut c, "sine", 33, 2);
    let before = r.core.ai_song().unwrap();
    let mut seen = Vec::new();
    let answer = c.request(
        "tools/call",
        json!({
            "name": "analyze_bands",
            "arguments": { "from_bar": 1, "to_bar": 2, "alone": [first, same, far] },
            "_meta": { "progressToken": "bands" },
        }),
        &mut seen,
    );
    assert_eq!(answer["result"]["isError"], false, "{answer}");
    let answer = &answer["result"]["structuredContent"];
    // A4 is 440 Hz: low_mid. A1 is 55 Hz: low.
    let together = score(pair(answer, first, same), "low_mid");
    assert!(together > 0.9, "{answer}");
    let apart = pair(answer, first, far);
    for band in ["low", "low_mid", "mid", "high_mid", "high"] {
        assert!(score(apart, band) < 0.1, "{band}: {answer}");
    }
    assert_eq!(pair(answer, first, same)["top_band"], "low_mid");
    assert!(
        (seen.iter()).any(|m| m["method"] == "notifications/progress"),
        "{seen:?}"
    );

    // Only some pairs, and only of tracks analysed alone.
    let some = ok(
        &mut c,
        "analyze_bands",
        json!({ "from_bar": 1, "to_bar": 1, "alone": [first, same, far], "compare": [first, far] }),
    );
    assert_eq!(some["overlap"]["pairs"].as_array().unwrap().len(), 1);
    let outside = refused(
        &mut c,
        "analyze_bands",
        json!({ "from_bar": 1, "to_bar": 1, "alone": [first, same], "compare": [first, far] }),
    );
    assert!(outside.contains("not analysed alone"), "{outside}");
    assert_eq!(r.core.ai_song().unwrap().undo_count, before.undo_count);
}

#[test]
fn sections_follow_the_markers_and_the_bars() {
    let r = running("analyze-sections");
    let mut c = Client::start(&r.socket);
    let track = track_playing(&mut c, "sine", 69, 1);
    ok(
        &mut c,
        "add_marker",
        json!({ "name": "Intro", "at": "1.1.0" }),
    );
    ok(&mut c, "add_marker", json!({ "name": "B", "at": "2.1.0" }));
    let marked = ok(
        &mut c,
        "analyze_bands",
        json!({ "from_bar": 1, "to_bar": 3, "alone": [track], "sections": true }),
    );
    let sections = marked["sections"].as_array().unwrap();
    assert_eq!(sections.len(), 2, "{marked}");
    assert_eq!(sections[0]["marker"], "Intro");
    assert_eq!(
        (sections[0]["from"].as_str(), sections[0]["to"].as_str()),
        (Some("1.1.0"), Some("2.1.0"))
    );
    assert_eq!(sections[1]["marker"], "B");
    assert_eq!(sections[1]["to"], "4.1.0");
    // The note is in the first bar only.
    assert!(sections[0]["tracks"][0]["rms_dbfs"].as_f64().is_some());
    assert!(sections[0]["master"]["lufs"].as_f64().is_some(), "{marked}");
    let second = sections[1]["tracks"][0]["rms_dbfs"].as_f64();
    let first = sections[0]["tracks"][0]["rms_dbfs"].as_f64().unwrap();
    assert!(second.is_none_or(|s| s < first - 20.0), "{marked}");

    let every = ok(
        &mut c,
        "analyze_bands",
        json!({ "from_bar": 1, "to_bar": 3, "alone": [track], "every_bars": 1 }),
    );
    let sections = every["sections"].as_array().unwrap();
    let starts: Vec<&str> = sections
        .iter()
        .map(|s| s["from"].as_str().unwrap())
        .collect();
    assert_eq!(starts, ["1.1.0", "2.1.0", "3.1.0"], "{every}");
    assert_eq!(sections[2]["marker"], "B");

    let none = ok(
        &mut c,
        "analyze_bands",
        json!({ "from_bar": 1, "to_bar": 3, "alone": [track] }),
    );
    assert_eq!(none.get("sections"), None);
    let many = refused(
        &mut c,
        "analyze_bands",
        json!({ "from_bar": 1, "to_bar": 40, "every_bars": 1 }),
    );
    assert!(many.contains("sections"), "{many}");
}

/// How long 16 bars of 6 tracks take: `cargo test -p meloet-ai --release --test analyze --
/// --ignored --nocapture`.
#[test]
#[ignore = "a measurement"]
fn sixteen_bars_of_six_tracks() {
    let r = running("analyze-time");
    let mut c = Client::start(&r.socket);
    for (sound, key) in [
        ("synthBass", 36),
        ("warmPad", 60),
        ("synthLead", 72),
        ("sine", 69),
        ("electroKit", 38),
    ] {
        track_playing(&mut c, sound, key, 16);
    }
    let tracks = r.core.ai_song().unwrap().song.doc.tracks.len();
    assert_eq!(tracks, 6);
    for bands in ["five", "octave"] {
        let started = Instant::now();
        let answer = c.call(
            "analyze_bands",
            json!({ "from_bar": 1, "to_bar": 16, "bands": bands, "every_bars": 4,
                    "compare": [2, 3, 4, 5, 6] }),
        );
        let took = started.elapsed();
        assert_eq!(answer["isError"], false, "{answer}");
        let text = answer["content"][0]["text"].as_str().unwrap();
        eprintln!(
            "analyze_bands, 16 bars, 6 tracks, {bands}: {took:?}, {} bytes",
            text.len()
        );
        if bands == "five" {
            eprintln!("{text}");
        }
    }
    let started = Instant::now();
    ok(
        &mut c,
        "measure_mix",
        json!({ "from_bar": 1, "to_bar": 16 }),
    );
    eprintln!("measure_mix, 16 bars, 6 tracks: {:?}", started.elapsed());
}

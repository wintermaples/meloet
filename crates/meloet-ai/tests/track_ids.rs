//! Tracks named by their stable id ("id:<n>") as well as by their number, through the AI
//! link's socket.

mod common;
use common::*;

use serde_json::{Value, json};

fn overview(c: &mut Client) -> Value {
    ok(c, "song_overview", json!({}))
}

/// The ids of the tracks, by number from 1 (index 0 is track 1).
fn ids(c: &mut Client) -> Vec<String> {
    let ov = overview(c);
    (ov["tracks"].as_array().unwrap().iter())
        .map(|t| t["id"].as_str().unwrap().to_owned())
        .collect()
}

/// The track of `id` in song_overview.
fn track(c: &mut Client, id: &str) -> Value {
    let ov = overview(c);
    (ov["tracks"].as_array().unwrap().iter())
        .find(|t| t["id"] == id)
        .unwrap_or_else(|| panic!("no track {id} in {ov}"))
        .clone()
}

fn add_track(c: &mut Client) -> String {
    let added = ok(c, "add_track", json!({ "sound": "sine" }));
    let id = added["track_id"].as_str().unwrap().to_owned();
    let ids = ids(c);
    assert_eq!(ids.last(), Some(&id), "{added}");
    assert_eq!(added["track"], json!(ids.len()), "{added}");
    id
}

#[test]
fn an_id_keeps_naming_its_track_while_numbers_shift() {
    let r = running("ids-shift");
    let mut c = Client::start(&r.socket);
    add_track(&mut c);
    add_track(&mut c);

    let ov = overview(&mut c);
    let tracks = ov["tracks"].as_array().unwrap();
    assert!(tracks.len() >= 3, "{ov}");
    for (i, t) in tracks.iter().enumerate() {
        assert_eq!(t["number"], json!(i + 1));
        let id = t["id"].as_str().unwrap();
        let n: u32 = id.strip_prefix("id:").unwrap().parse().unwrap();
        assert_eq!(r.core.ai_song().unwrap().song.doc.tracks[i].id.0, n);
    }
    let ids_now = ids(&mut c);
    let a = ids_now[0].clone();

    // By id: rename, mute, write notes, volume.
    ok(
        &mut c,
        "rename_track",
        json!({ "track": a, "name": "Alpha" }),
    );
    ok(&mut c, "set_mute", json!({ "track": a, "on": true }));
    let wrote = ok(
        &mut c,
        "write_notes",
        json!({ "track": a, "notes": [[0, 1920, 60], [1920, 1920, 62]] }),
    );
    assert_eq!(wrote["written"], 2, "{wrote}");
    assert_eq!(wrote["clip_place"]["track"], 1, "{wrote}");
    assert_eq!(wrote["clip_place"]["track_id"], a, "{wrote}");
    if !wrote["created_clip"].is_null() {
        assert_eq!(wrote["created_clip"]["track_id"], a, "{wrote}");
    }
    let clip = wrote["clip_place"]["id"].clone();
    ok(&mut c, "set_volume", json!({ "track": a, "db": -12.0 }));
    let t = track(&mut c, &a);
    assert_eq!(t["name"], "Alpha");
    assert_eq!(t["muted"], true);
    assert_eq!(t["volume_db"], -12.0);
    let ov = overview(&mut c);
    let listed = (ov["clips"].as_array().unwrap().iter())
        .find(|k| k["id"] == clip)
        .unwrap();
    assert_eq!(listed["track_id"], a, "{listed}");
    let notes = ok(&mut c, "get_notes", json!({ "clip": clip }));
    assert_eq!(notes["track_id"], a, "{notes}");

    // Track 1 moves to the end: its id follows it, number 1 is another track now.
    let count = ids_now.len();
    ok(&mut c, "move_track", json!({ "track": a, "to": count }));
    assert_eq!(ids(&mut c)[count - 1], a);
    ok(&mut c, "rename_track", json!({ "track": 1, "name": "One" }));
    assert_eq!(track(&mut c, &a)["name"], "Alpha");
    let one = ids(&mut c)[0].clone();
    assert_ne!(one, a);
    assert_eq!(track(&mut c, &one)["name"], "One");

    // A track added and put first shifts every number again.
    let added = add_track(&mut c);
    ok(&mut c, "move_track", json!({ "track": added, "to": 1 }));
    assert_eq!(ids(&mut c)[0], added);
    ok(
        &mut c,
        "rename_track",
        json!({ "track": a, "name": "Still alpha" }),
    );
    let t = track(&mut c, &a);
    assert_eq!(t["name"], "Still alpha");
    assert_eq!(t["number"], json!(count + 1));
    let mixer = ok(&mut c, "get_mixer", json!({}));
    let strip = (mixer["tracks"].as_array().unwrap().iter())
        .find(|t| t["id"] == a)
        .unwrap();
    assert_eq!(strip["name"], "Still alpha");
    assert_eq!(strip["number"], json!(count + 1));

    // Deleted: the id names nothing, and the answer says which id.
    ok(&mut c, "delete_track", json!({ "track": a }));
    let text = refused(&mut c, "rename_track", json!({ "track": a, "name": "x" }));
    assert!(text.contains(&format!("\"{a}\"")), "{text}");
    assert!(text.contains("deleted"), "{text}");
    let text = refused(
        &mut c,
        "set_mute",
        json!({ "track": "id:999999", "on": true }),
    );
    assert!(text.contains("\"id:999999\""), "{text}");

    // Bad forms are refused before anything is done.
    let before = steps(&r);
    for bad in [json!("2"), json!("id:"), json!("id:x"), json!(0), json!(-1)] {
        let answer = c.call("rename_track", json!({ "track": bad, "name": "x" }));
        assert_eq!(answer["isError"], true, "{bad}: {answer}");
    }
    assert_eq!(steps(&r), before);
}

#[test]
fn lists_take_numbers_and_ids_together() {
    let r = running("ids-lists");
    let mut c = Client::start(&r.socket);
    add_track(&mut c);
    add_track(&mut c);
    let ids = ids(&mut c);
    ok(
        &mut c,
        "write_notes",
        json!({ "track": ids[1], "notes": [[0, 1920, 60, 100]] }),
    );
    let measured = ok(
        &mut c,
        "measure_mix",
        json!({ "from_bar": 1, "to_bar": 1, "alone": [1, ids[1]] }),
    );
    let tracks = measured["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 2, "{measured}");
    assert_eq!(tracks[0]["number"], 1);
    assert_eq!(tracks[0]["id"], ids[0]);
    assert_eq!(tracks[1]["number"], 2);
    assert_eq!(tracks[1]["id"], ids[1]);

    let measured = ok(
        &mut c,
        "measure_mix",
        json!({ "from_bar": 1, "to_bar": 1, "tracks": [ids[2], 2] }),
    );
    let numbers: Vec<&Value> = (measured["tracks"].as_array().unwrap().iter())
        .map(|t| &t["number"])
        .collect();
    assert_eq!(numbers, [&json!(2), &json!(3)], "{measured}");

    let text = refused(
        &mut c,
        "measure_mix",
        json!({ "from_bar": 1, "to_bar": 1, "alone": [1, "id:999999"] }),
    );
    assert!(text.contains("\"id:999999\""), "{text}");

    // Mute sets give ids beside the numbers.
    ok(&mut c, "set_mute", json!({ "track": ids[2], "on": true }));
    ok(&mut c, "save_mute_set", json!({ "name": "three off" }));
    let sets = ok(&mut c, "list_mute_sets", json!({}));
    let set = &sets["mute_sets"][0];
    assert_eq!(set["muted"], json!([3]), "{sets}");
    assert_eq!(set["muted_ids"], json!([ids[2]]), "{sets}");
    assert_eq!(set["track_ids"], json!(ids), "{sets}");
}

#[test]
fn a_batch_takes_the_id_of_a_track_it_added() {
    let r = running("ids-batch");
    let mut c = Client::start(&r.socket);
    let done = ok(
        &mut c,
        "batch",
        json!({ "operations": [
            { "tool": "add_track", "arguments": { "sound": "sine" } },
            { "tool": "rename_track", "arguments": {
                "track": { "result_of": 1, "field": "track_id" }, "name": "Made" } },
            { "tool": "set_volume", "arguments": {
                "track": { "result_of": 1, "field": "track" }, "db": -6.0 } },
        ] }),
    );
    let id = done["results"][0]["result"]["track_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let t = track(&mut c, &id);
    assert_eq!(t["name"], "Made", "{done}");
    assert_eq!(t["volume_db"], -6.0, "{done}");
}

/// Whether `schema` takes a track's id: somewhere in it is the "id:<n>" string form.
fn takes_ids(schema: &Value) -> bool {
    match schema {
        Value::Object(fields) => {
            (fields.get("type") == Some(&json!("string"))
                && fields.get("pattern") == Some(&json!("^id:[0-9]+$")))
                || fields.values().any(takes_ids)
        }
        Value::Array(items) => items.iter().any(takes_ids),
        _ => false,
    }
}

#[test]
fn every_track_argument_takes_an_id() {
    let r = running("ids-schema");
    let mut c = Client::start(&r.socket);
    let list = c.request("tools/list", json!({}), &mut Vec::new());
    let tools = list["result"]["tools"].as_array().unwrap();
    let mut seen = 0;
    for t in tools {
        let schema = &t["inputSchema"];
        assert_eq!(schema["type"], "object", "{}", t["name"]);
        let Some(properties) = schema["properties"].as_object() else {
            continue;
        };
        for (name, p) in properties {
            if !["track", "tracks", "to_track", "alone", "compare"].contains(&name.as_str()) {
                continue;
            }
            // `tracks` of move_clips is how many rows the clips move.
            if t["name"] == "move_clips" && name == "tracks" {
                continue;
            }
            assert!(takes_ids(p), "{} `{name}`: {p}", t["name"]);
            assert!(
                p.to_string().contains("\"minimum\":1"),
                "{} `{name}`: {p}",
                t["name"]
            );
            seen += 1;
        }
    }
    assert!(seen >= 40, "{seen} track arguments");
}

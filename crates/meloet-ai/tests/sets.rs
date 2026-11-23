//! Mute sets and the list of songs, through the AI link's socket.

mod common;
use common::*;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use meloet_ai::{AiLink, CoreLink};
use meloet_core::{Core, CoreOptions};
use meloet_protocol::AiPort;
use serde_json::{Value, json};

fn mute(c: &mut Client, track: u32, on: bool) {
    ok(c, "set_mute", json!({ "track": track, "on": on }));
}

fn muted(r: &Running) -> Vec<bool> {
    let song = r.core.ai_song().unwrap().song;
    song.doc.tracks.iter().map(|t| t.muted).collect()
}

fn soloed(r: &Running) -> Vec<bool> {
    let song = r.core.ai_song().unwrap().song;
    (song.doc.tracks.iter())
        .map(|t| song.session.soloed.contains(&t.id))
        .collect()
}

#[test]
fn switching_between_two_sets_is_one_step_each_and_undo_brings_the_song_back() {
    let r = running("sets");
    let mut c = Client::start(&r.socket);
    for _ in 0..3 {
        ok(&mut c, "add_track", json!({ "sound": "sine" }));
    }
    let before = r.core.ai_song().unwrap();

    // 「爽快セット」: tracks 2 and 4 muted, track 3 soloed.
    mute(&mut c, 2, true);
    mute(&mut c, 4, true);
    ok(&mut c, "set_solo", json!({ "track": 3, "on": true }));
    let steps_before = steps(&r);
    let saved = ok(&mut c, "save_mute_set", json!({ "name": "爽快セット" }));
    assert_eq!(steps(&r), steps_before + 1, "{saved}");
    assert_eq!(saved["changed"], true);
    assert_eq!(saved["step"]["name"]["kind"], "saveMuteSet");
    assert_eq!(saved["step"]["what"]["name"], "爽快セット");
    assert_eq!(saved["set"]["muted"], json!([2, 4]));
    assert_eq!(saved["set"]["soloed"], json!([3]));
    assert_eq!(saved["set"]["as_now"], true);

    // 「クールセット」: tracks 1 and 3 muted, return A muted, no solo.
    ok(&mut c, "set_solo", json!({ "track": 3, "on": false }));
    mute(&mut c, 2, false);
    mute(&mut c, 4, false);
    mute(&mut c, 1, true);
    mute(&mut c, 3, true);
    ok(&mut c, "set_mute", json!({ "bus": "return_a", "on": true }));
    ok(&mut c, "save_mute_set", json!({ "name": "クールセット" }));
    let text = refused(&mut c, "save_mute_set", json!({ "name": "クールセット" }));
    assert!(text.contains("overwrite"), "{text}");
    let again = ok(
        &mut c,
        "save_mute_set",
        json!({ "name": "クールセット", "overwrite": true }),
    );
    assert_eq!(again["changed"], false, "the same: no step");

    let listed = ok(&mut c, "list_mute_sets", json!({}));
    let sets = listed["mute_sets"].as_array().unwrap();
    assert_eq!(sets.len(), 2);
    assert_eq!(sets[0]["name"], "爽快セット");
    assert_eq!(sets[0]["as_now"], false);
    assert_eq!(sets[1]["muted"], json!([1, 3]));
    assert_eq!(sets[1]["muted_returns"], json!(["return_a"]));
    assert_eq!(sets[1]["as_now"], true);
    let one = ok(&mut c, "list_mute_sets", json!({ "name": "クールセット" }));
    assert_eq!(one["mute_sets"].as_array().unwrap().len(), 1);
    refused(&mut c, "list_mute_sets", json!({ "name": "ない" }));

    // Each switch is one step.
    for (name, mutes, solos) in [
        (
            "爽快セット",
            [false, true, false, true],
            [false, false, true, false],
        ),
        ("クールセット", [true, false, true, false], [false; 4]),
        (
            "爽快セット",
            [false, true, false, true],
            [false, false, true, false],
        ),
    ] {
        let before = steps(&r);
        let recalled = ok(&mut c, "recall_mute_set", json!({ "name": name }));
        assert_eq!(steps(&r), before + 1, "{name}: {recalled}");
        assert_eq!(recalled["changed"], true);
        assert_eq!(recalled["step"]["what"]["name"], name);
        assert_eq!(muted(&r), mutes, "{name}");
        assert_eq!(soloed(&r), solos, "{name}");
    }
    let same = ok(&mut c, "recall_mute_set", json!({ "name": "爽快セット" }));
    assert_eq!(same["changed"], false, "{same}");
    let text = refused(&mut c, "recall_mute_set", json!({ "name": "ない" }));
    assert!(text.contains("list_mute_sets"), "{text}");

    // A track added since is left as it is; a track deleted since is skipped.
    ok(&mut c, "add_track", json!({ "sound": "sine" }));
    mute(&mut c, 5, true);
    ok(&mut c, "delete_track", json!({ "track": 4 }));
    let recalled = ok(&mut c, "recall_mute_set", json!({ "name": "クールセット" }));
    assert_eq!(recalled["not_in_set"], json!([4]), "{recalled}");
    assert_eq!(recalled["deleted_track_ids"].as_array().unwrap().len(), 1);
    assert_eq!(muted(&r), [true, false, true, true]);

    let deleted = ok(&mut c, "delete_mute_set", json!({ "name": "クールセット" }));
    assert_eq!(deleted["step"]["what"]["name"], "クールセット");
    refused(&mut c, "delete_mute_set", json!({ "name": "クールセット" }));

    // Undo brings back the song; solo is no step (the last recall ended it).
    assert_eq!(soloed(&r), [false; 4]);
    undo_all(&mut c, &r, &before);
    assert!(r.core.ai_song().unwrap().song.doc.mute_sets.is_empty());
}

#[test]
fn a_set_is_renamed_as_one_step_and_keeps_what_it_holds() {
    let r = running("sets-rename");
    let mut c = Client::start(&r.socket);
    ok(&mut c, "add_track", json!({ "sound": "sine" }));
    mute(&mut c, 2, true);
    ok(&mut c, "save_mute_set", json!({ "name": "A" }));
    mute(&mut c, 2, false);
    ok(&mut c, "save_mute_set", json!({ "name": "B" }));
    let before = steps(&r);

    let renamed = ok(
        &mut c,
        "rename_mute_set",
        json!({ "name": "A", "new_name": " サビ " }),
    );
    assert_eq!(steps(&r), before + 1, "{renamed}");
    assert_eq!(renamed["changed"], true);
    assert_eq!(renamed["step"]["name"]["kind"], "renameMuteSet");
    assert_eq!(
        renamed["step"]["what"],
        json!({"kind": "renamed", "from": "A", "to": "サビ"})
    );
    assert_eq!(renamed["set"]["name"], "サビ");
    assert_eq!(renamed["set"]["muted"], json!([2]));
    let listed = ok(&mut c, "list_mute_sets", json!({}));
    let names: Vec<&str> = (listed["mute_sets"].as_array().unwrap().iter())
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["サビ", "B"], "in its place");

    let same = ok(
        &mut c,
        "rename_mute_set",
        json!({ "name": "サビ", "new_name": "サビ" }),
    );
    assert_eq!(same["changed"], false, "{same}");
    let text = refused(
        &mut c,
        "rename_mute_set",
        json!({ "name": "サビ", "new_name": "B" }),
    );
    assert!(text.contains("another mute set \"B\""), "{text}");
    let text = refused(
        &mut c,
        "rename_mute_set",
        json!({ "name": "A", "new_name": "C" }),
    );
    assert!(text.contains("list_mute_sets"), "{text}");
    assert_eq!(steps(&r), before + 1);

    // Undo gives the old name back.
    ok(&mut c, "undo", json!({}));
    let song = r.core.ai_song().unwrap().song;
    let names: Vec<&str> = song.doc.mute_sets.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["A", "B"]);
}

#[test]
fn the_tools_say_what_they_are() {
    let r = running("sets-tools");
    let mut c = Client::start(&r.socket);
    let list = c.request("tools/list", json!({}), &mut Vec::new());
    let tools = list["result"]["tools"].as_array().unwrap();
    let tool = |name: &str| {
        (tools.iter())
            .find(|t| t["name"] == name)
            .unwrap_or_else(|| panic!("{name}"))
            .clone()
    };
    for (name, read_only, destructive) in [
        ("list_songs", true, None),
        ("list_mute_sets", true, None),
        ("save_mute_set", false, Some(false)),
        ("recall_mute_set", false, Some(false)),
        ("delete_mute_set", false, Some(true)),
        ("rename_mute_set", false, Some(false)),
    ] {
        let t = tool(name);
        let hints = &t["annotations"];
        assert_eq!(hints["openWorldHint"], false, "{t}");
        assert_eq!(hints["readOnlyHint"], read_only, "{t}");
        if let Some(destructive) = destructive {
            assert_eq!(hints["destructiveHint"], destructive, "{t}");
        }
    }
}

/// A core whose first saves go into `songs`.
fn running_with_songs(name: &str, songs: &Path) -> Running {
    let runtime = scratch(name);
    let options = options(&runtime);
    let socket = options.socket_path().unwrap();
    let core = Arc::new(Core::start(CoreOptions {
        songs_dir: Some(songs.to_path_buf()),
        ..offline(&runtime)
    }));
    let link = AiLink::start(Arc::new(CoreLink(Arc::downgrade(&core))), options).unwrap();
    core.set_ai_switch(Box::new(move |on| link.switch(on)));
    core.send(meloet_protocol::Command::SetAiLink { on: true });
    wait_for("the socket", || socket.exists());
    wait_for("Waiting", || port_of(&core) == AiPort::Waiting);
    Running { core, socket }
}

fn paths(songs: &Value) -> Vec<String> {
    (songs.as_array().unwrap().iter())
        .map(|s| s["path"].as_str().unwrap().to_owned())
        .collect()
}

fn note(at: &str) -> Value {
    json!({ "at": at, "length": 480, "key": 60, "velocity": 100 })
}

#[test]
fn list_songs_gives_song_files_only() {
    let dir = scratch("sets-songs");
    let usual = scratch("sets-usual");
    let r = running_with_songs("sets-list", &usual);
    let mut c = Client::start(&r.socket);

    let nothing = ok(&mut c, "list_songs", json!({}));
    assert_eq!(nothing["song_open_now"], Value::Null);
    assert_eq!(nothing["recent"], json!([]));
    assert_eq!(
        nothing["songs_folder"]["folder"],
        usual.display().to_string()
    );
    assert_eq!(nothing["songs_folder"]["songs"], json!([]));

    // Two songs; the first saved twice, so that it has a .prev beside it.
    ok(
        &mut c,
        "write_notes",
        json!({ "track": 1, "notes": [note("1.1.0")] }),
    );
    let first = ok(
        &mut c,
        "save_song_as",
        json!({ "folder": dir, "name": "一曲目" }),
    );
    let first = PathBuf::from(first["file"].as_str().unwrap());
    ok(
        &mut c,
        "write_notes",
        json!({ "track": 1, "notes": [note("2.1.0")] }),
    );
    ok(&mut c, "save_song", json!({}));
    assert!(dir.join("一曲目").join("一曲目.meloet.prev").is_file());
    let second = ok(
        &mut c,
        "save_song_as",
        json!({ "folder": dir, "name": "二曲目" }),
    );
    let second = PathBuf::from(second["file"].as_str().unwrap());
    // What is not a song, or is out of reach.
    std::fs::write(dir.join("memo.txt"), "x").unwrap();
    std::fs::create_dir_all(dir.join("a").join("b")).unwrap();
    std::fs::copy(&first, dir.join("a").join("b").join("深い.meloet")).unwrap();
    std::os::unix::fs::symlink(&first, dir.join("link.meloet")).unwrap();
    std::os::unix::fs::symlink(dir.join("一曲目"), dir.join("link")).unwrap();

    let listed = ok(&mut c, "list_songs", json!({ "folder": dir }));
    let mut found = paths(&listed["songs"]);
    found.sort();
    let mut want = vec![first.display().to_string(), second.display().to_string()];
    want.sort();
    assert_eq!(found, want, "{listed}");
    assert_eq!(listed["total"], 2);
    assert_eq!(listed["complete"], true);
    let song = (listed["songs"].as_array().unwrap().iter())
        .find(|s| s["path"] == second.display().to_string())
        .unwrap();
    assert_eq!(song["name"], "二曲目");
    assert_eq!(song["open_now"], true);
    assert!(song["size"].as_u64().unwrap() > 0);
    let modified = song["modified"].as_str().unwrap();
    assert!(
        modified.ends_with('Z') && modified.len() == 20,
        "{modified}"
    );

    let page = ok(
        &mut c,
        "list_songs",
        json!({ "folder": dir, "limit": 1, "offset": 1 }),
    );
    assert_eq!(page["songs"].as_array().unwrap().len(), 1);
    assert_eq!(page["total"], 2);
    refused(&mut c, "list_songs", json!({ "folder": "songs" }));
    refused(&mut c, "list_songs", json!({ "folder": dir.join("..") }));
    refused(&mut c, "list_songs", json!({ "folder": dir.join("none") }));

    // The songs opened or saved lately, newest first; a file gone is left out.
    let known = ok(&mut c, "list_songs", json!({}));
    assert_eq!(known["song_open_now"]["path"], second.display().to_string());
    assert_eq!(
        paths(&known["recent"]),
        [second.display().to_string(), first.display().to_string()]
    );
    let state = r.core.ai_song().unwrap();
    assert_eq!(state.recent, [second.clone(), first.clone()]);
    std::fs::remove_file(&first).unwrap();
    let known = ok(&mut c, "list_songs", json!({}));
    assert_eq!(paths(&known["recent"]), [second.display().to_string()]);
}

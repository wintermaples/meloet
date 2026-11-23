//! A plugin's whole state through the socket, with the real `meloet-plughost` and its fake
//! plugins (the fake sine's state is its level, an f64 from 0 to 1).

mod common;
use common::*;

#[path = "../../meloet-core/tests/plughost/mod.rs"]
mod plughost;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use meloet_ai::{AiLink, CoreLink};
use meloet_core::plugins::PluginOptions;
use meloet_core::{Core, CoreOptions};
use meloet_model::PluginPlace;
use meloet_protocol::{AiPort, Command};
use serde_json::{Value, json};

const SINE: &str = "lv2:urn:meloet:fake:sine";

/// A core whose plugins are the fake ones of a bundle in `root/lv2`, with its AI link.
fn running_with_plugins(name: &str, places: Vec<PathBuf>, env: bool) -> Running {
    let runtime = scratch(name);
    let options = options(&runtime);
    let socket = options.socket_path().unwrap();
    let core_options = CoreOptions {
        songs_dir: Some(runtime.join("songs")),
        plugins: Some(PluginOptions {
            host_exe: plughost::exe(),
            cache_dir: Some(runtime.join("cache")),
            usual_places: places,
            scan_timeout: Duration::from_secs(30),
            child_env: if env {
                plughost::fake_env()
            } else {
                Vec::new()
            },
            wait_for_screen: false,
        }),
        ..offline(&runtime)
    };
    let core = Arc::new(Core::start(core_options));
    let link = AiLink::start(Arc::new(CoreLink(Arc::downgrade(&core))), options).unwrap();
    core.set_ai_switch(Box::new(move |on| link.switch(on)));
    core.send(Command::SetAiLink { on: true });
    wait_for("the socket", || socket.exists());
    wait_for("Waiting", || port_of(&core) == AiPort::Waiting);
    Running { core, socket }
}

fn fake_bundle(name: &str) -> PathBuf {
    let lv2 = scratch(&format!("{name}-lv2"));
    let bundle = lv2.join("found.lv2");
    std::fs::create_dir_all(&bundle).unwrap();
    std::fs::write(bundle.join("plugin.so"), [0u8; 16]).unwrap();
    lv2
}

/// Waits (a minute at most) until `look` gives something.
fn patiently<T>(what: &str, look: impl FnMut() -> Option<T>) -> T {
    waiting(what, Duration::from_secs(60), look)
}

fn waiting<T>(what: &str, most: Duration, mut look: impl FnMut() -> Option<T>) -> T {
    let until = std::time::Instant::now() + most;
    loop {
        if let Some(found) = look() {
            return found;
        }
        assert!(
            std::time::Instant::now() < until,
            "waited too long for {what}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn has_plugin(c: &mut Client, key: &str) -> bool {
    let list = ok(c, "list_instruments", json!({}));
    let effects = ok(c, "list_effects", json!({ "limit": 500 }));
    (list["plugins"].as_array().into_iter().flatten())
        .chain(effects["plugins"].as_array().into_iter().flatten())
        .any(|p| p["key"] == key)
}

/// The state of a plugin once it is loaded.
fn state_when_loaded(c: &mut Client, at: Value) -> Value {
    patiently("the plugin to be loaded", || {
        let answer = c.call("get_plugin_state", at.clone());
        (answer["isError"] == false).then(|| answer["structuredContent"].clone())
    })
}

fn level_state(level: f64) -> String {
    b64(&level.to_le_bytes())
}

fn b64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n =
            (chunk.iter().enumerate()).fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            match i <= chunk.len() {
                true => out.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize])),
                false => out.push('='),
            }
        }
    }
    out
}

fn sha(state: &Value) -> String {
    state["sha256"].as_str().unwrap().to_owned()
}

fn level_param(c: &mut Client, track: u32) -> f64 {
    let params = ok(c, "get_plugin_params", json!({ "track": track }));
    params["params"][0]["value"].as_f64().unwrap()
}

#[test]
fn a_plugins_state_is_read_put_back_undone_saved_and_a_broken_one_changes_nothing() {
    let lv2 = fake_bundle("state");
    let r = running_with_plugins("state", vec![lv2], true);
    let mut c = Client::start(&r.socket);
    patiently("the scan", || has_plugin(&mut c, SINE).then_some(()));
    ok(&mut c, "add_track", json!({ "plugin": SINE }));
    let at = json!({ "track": 2, "inline": true });

    // Read: as the plugin says it now, inline and as a private file.
    let first = state_when_loaded(&mut c, at.clone());
    assert_eq!(first["plugin"]["key"], SINE, "{first}");
    assert_eq!(first["plugin"]["format"], "lv2");
    assert_eq!(first["bytes"], 8);
    assert_eq!(first["base64"], level_state(0.5));
    assert_eq!(first["file"], Value::Null);
    assert_eq!(first["readable"], Value::Null);
    let filed = ok(&mut c, "get_plugin_state", json!({ "track": 2 }));
    let file = PathBuf::from(filed["file"].as_str().expect("a file"));
    assert_eq!(std::fs::read(&file).unwrap(), 0.5f64.to_le_bytes());
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::metadata(&file).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    assert_eq!(sha(&filed), sha(&first));

    // A parameter moved, then the first state put back: one step each.
    let steps0 = steps(&r);
    ok(
        &mut c,
        "set_param",
        json!({ "track": 2, "param": 0, "value": 0.25 }),
    );
    let moved = ok(&mut c, "get_plugin_state", at.clone());
    assert_eq!(moved["base64"], level_state(0.25));
    let put = json!({ "track": 2, "base64": first["base64"] });
    let set = ok(&mut c, "set_plugin_state", put.clone());
    assert_eq!(steps(&r), steps0 + 2);
    assert_eq!(set["undo_steps"], steps0 + 2, "{set}");
    assert_eq!(set["given"]["sha256"], first["sha256"]);
    assert_eq!(set["now"]["sha256"], first["sha256"]);
    assert_eq!(level_param(&mut c, 2), 0.5);
    let song = r.core.ai_song().unwrap();
    let place = PluginPlace::Instrument(song.song.doc.tracks[1].id);
    let in_song = song.song.doc.plugin(place).unwrap().state.clone();
    assert_eq!(&*in_song, 0.5f64.to_le_bytes());

    // Undo and redo put the states into the plugin.
    ok(&mut c, "undo", json!({}));
    assert_eq!(
        ok(&mut c, "get_plugin_state", at.clone())["sha256"],
        moved["sha256"]
    );
    r.core.send(Command::Redo);
    wait_for("the redo", || steps(&r) == steps0 + 2);
    let again = patiently("the plugin to have it again", || {
        let now = ok(&mut c, "get_plugin_state", at.clone());
        (now["sha256"] == first["sha256"]).then_some(now)
    });
    assert_eq!(again["bytes"], 8);

    // From a file.
    let from = scratch("state-file").join("level.bin");
    std::fs::write(&from, 0.75f64.to_le_bytes()).unwrap();
    let path = from.display().to_string();
    ok(
        &mut c,
        "set_plugin_state",
        json!({ "track": 2, "file": path }),
    );
    assert_eq!(level_param(&mut c, 2), 0.75);

    // Broken states: refused by the plugin, nothing changes.
    let before = r.core.ai_song().unwrap();
    let held = ok(&mut c, "get_plugin_state", at.clone());
    for broken in [b64(b"broken"), level_state(2.0), b64(&[0u8; 3])] {
        let args = json!({ "track": 2, "base64": broken });
        let text = refused(&mut c, "set_plugin_state", args);
        assert!(text.contains("refused"), "{text}");
    }
    let after = r.core.ai_song().unwrap();
    assert_eq!(after.undo_count, before.undo_count);
    assert_eq!(after.song.doc.tracks, before.song.doc.tracks);
    assert_eq!(
        ok(&mut c, "get_plugin_state", at.clone())["sha256"],
        held["sha256"]
    );
    assert_eq!(level_param(&mut c, 2), 0.75);

    // The state it has already: a success that changes nothing.
    let same = ok(
        &mut c,
        "set_plugin_state",
        json!({ "track": 2, "base64": level_state(0.75) }),
    );
    assert_eq!(same["changed"], false, "{same}");
    assert_eq!(r.core.ai_song().unwrap().undo_count, before.undo_count);

    // Wrong asks.
    let missing = scratch("state-none").join("none.bin").display().to_string();
    for (args, says) in [
        (json!({ "track": 2 }), "one of them"),
        (
            json!({ "track": 2, "base64": "QQ==", "file": path }),
            "one of them",
        ),
        (json!({ "track": 2, "base64": "not base64!" }), "not base64"),
        (json!({ "track": 2, "base64": "" }), "empty"),
        (json!({ "track": 2, "file": "level.bin" }), "absolute"),
        (json!({ "track": 2, "file": missing }), "cannot be read"),
        (
            json!({ "track": 1, "base64": level_state(0.5) }),
            "no plugin",
        ),
        (
            json!({ "bus": "master", "base64": level_state(0.5) }),
            "slot",
        ),
        (
            json!({ "track": 9, "base64": level_state(0.5) }),
            "no track",
        ),
    ] {
        let text = refused(&mut c, "set_plugin_state", args.clone());
        assert!(text.contains(says), "{args}: {text}");
    }
    assert!(refused(&mut c, "get_plugin_state", json!({ "track": 1 })).contains("no plugin"));
    assert_eq!(steps(&r), before.undo_count);

    // Saved and opened again: the plugin comes back with the same state.
    let folder = scratch("state-songs").display().to_string();
    ok(
        &mut c,
        "save_song_as",
        json!({ "folder": folder, "name": "states" }),
    );
    let file = Path::new(&folder).join("states/states.meloet");
    assert!(file.is_file());
    ok(&mut c, "new_song", json!({}));
    ok(
        &mut c,
        "open_song",
        json!({ "path": file.display().to_string() }),
    );
    let opened = state_when_loaded(&mut c, at.clone());
    assert_eq!(opened["sha256"], held["sha256"]);
}

/// Surge XT (VST3 and LV2) and ZamComp (an LV2 effect) of this PC: a parameter moved, the
/// state before put back, undo and redo, saved and opened, and broken states.
/// `cargo test -p meloet-ai --test plugstate -- --ignored --nocapture`
#[test]
#[ignore = "needs Surge XT and ZamComp installed"]
fn real_plugins_take_their_state_back() {
    // Only these VST3 bundles: the scan of every plugin of this PC takes a minute.
    let vst3 = scratch("real-vst3");
    for name in ["Surge XT.vst3", "ZamComp.vst3"] {
        std::os::unix::fs::symlink(Path::new("/usr/lib/vst3").join(name), vst3.join(name)).unwrap();
    }
    let r = running_with_plugins("real", vec![vst3, PathBuf::from("/usr/lib/lv2")], false);
    let mut c = Client::start(&r.socket);
    // Every LV2 bundle of this PC is scanned: minutes.
    let scan = Duration::from_secs(600);
    waiting("the scan", scan, || find_plugin(&mut c, "ZamComp", "lv2"));
    let surge_vst3 = patiently("Surge XT (VST3)", || {
        find_plugin(&mut c, "Surge XT", "vst3")
    });
    let surge_lv2 = patiently("Surge XT (LV2)", || find_plugin(&mut c, "Surge XT", "lv2"));
    let zam_lv2 = patiently("ZamComp (LV2)", || find_plugin(&mut c, "ZamComp", "lv2"));
    let zam_vst3 = patiently("ZamComp (VST3)", || find_plugin(&mut c, "ZamComp", "vst3"));
    eprintln!("keys: {surge_vst3}, {surge_lv2}, {zam_lv2}, {zam_vst3}");
    ok(&mut c, "add_track", json!({ "plugin": surge_vst3 }));
    ok(&mut c, "add_track", json!({ "plugin": surge_lv2 }));
    ok(
        &mut c,
        "insert_effect",
        json!({ "track": 2, "slot": 1, "effect": zam_lv2 }),
    );
    ok(
        &mut c,
        "insert_effect",
        json!({ "track": 3, "slot": 1, "effect": zam_vst3 }),
    );
    let places = [
        ("Surge XT VST3", json!({ "track": 2 })),
        ("Surge XT LV2", json!({ "track": 3 })),
        ("ZamComp LV2", json!({ "track": 2, "slot": 1 })),
        ("ZamComp VST3", json!({ "track": 3, "slot": 1 })),
    ];
    let mut kept = Vec::new();
    for (name, at) in &places {
        let first = state_when_loaded(&mut c, at.clone());
        eprintln!(
            "{name}: {} bytes, sha256 {}",
            first["bytes"], first["sha256"]
        );
        let file = first["file"].as_str().unwrap().to_owned();
        let params = ok(&mut c, "get_plugin_params", at.clone());
        let smooth = |p: &&Value| p["steps"] == 0 && p["automatable"] == true;
        let list = params["params"].as_array().unwrap();
        // One whose value the plugin tells, else any (an LV2 `patch` value is told only
        // once set): the state's sha256 shows the change too.
        let param = (list.iter().filter(smooth).find(|p| p["value"].is_number()))
            .or_else(|| list.iter().find(smooth))
            .expect("a smooth parameter")
            .clone();
        let id = param["id"].as_u64().unwrap();
        let was = param["value"].as_f64();
        let to = match was {
            Some(was) if was > 0.5 => was - 0.3,
            Some(was) => was + 0.3,
            None => 0.7,
        };
        let mut set = at.clone();
        set["param"] = json!(id);
        set["value"] = json!(to);
        let steps0 = steps(&r);
        let answer = c.call("set_param", set);
        // ZamComp's VST3 says the state it had after a parameter moved: no step comes.
        if answer["isError"] == true {
            let text = &answer["content"][0]["text"];
            eprintln!("{name}: the parameter made no step: {text}");
        } else {
            let value_of = |c: &mut Client| {
                let params = ok(c, "get_plugin_params", at.clone());
                (params["params"].as_array().unwrap().iter())
                    .find(|p| p["id"] == id)
                    .and_then(|p| p["value"].as_f64())
            };
            let sha_of = |c: &mut Client| sha(&ok(c, "get_plugin_state", at.clone()));
            let moved = value_of(&mut c);
            let moved_sha = sha_of(&mut c);
            eprintln!(
                "{name}: parameter {id} {} moved {was:?} -> {moved:?}; sha256 {moved_sha}",
                param["name"]
            );
            assert_ne!(moved_sha, sha(&first), "{name}: the state changed");
            let mut put = at.clone();
            put["file"] = json!(file);
            let set = ok(&mut c, "set_plugin_state", put);
            eprintln!("{name}: put back: {set}");
            assert_eq!(steps(&r), steps0 + 2);
            let near = |a: Option<f64>, b: Option<f64>| match (a, b) {
                (Some(a), Some(b)) => (a - b).abs() < 0.02,
                _ => true,
            };
            patiently("the first state back", || {
                let back = value_of(&mut c);
                (near(back, was) && sha_of(&mut c) == sha(&first)).then_some(())
            });
            eprintln!(
                "{name}: back: value {:?}, sha256 as at first",
                value_of(&mut c)
            );
            ok(&mut c, "undo", json!({}));
            patiently("undo", || {
                (near(value_of(&mut c), moved) && sha_of(&mut c) == moved_sha).then_some(())
            });
            r.core.send(Command::Redo);
            patiently("redo", || {
                (near(value_of(&mut c), was) && sha_of(&mut c) == sha(&first)).then_some(())
            });
            eprintln!("{name}: undo and redo put the states back");
        }

        // Broken: garbage, and the state cut in half.
        let before = r.core.ai_song().unwrap();
        let whole = std::fs::read(&file).unwrap();
        for (what, broken) in [
            ("garbage", b64(b"this is not a plugin state at all")),
            ("half", b64(&whole[..whole.len() / 2])),
            ("garbage in the container", b64(&inside(&whole, false))),
            ("cut in the container", b64(&inside(&whole, true))),
        ] {
            let mut args = at.clone();
            args["base64"] = json!(broken);
            let answer = c.call("set_plugin_state", args);
            eprintln!("{name}: {what}: {}", answer["content"][0]["text"]);
            let after = r.core.ai_song().unwrap();
            let same = answer["structuredContent"]["changed"] == false;
            if answer["isError"] == true || same {
                assert_eq!(after.undo_count, before.undo_count, "{name} {what}");
                assert_eq!(after.song.doc.tracks, before.song.doc.tracks);
            } else {
                // Taken by the plugin as a state: one step, which undo takes back.
                assert_eq!(after.undo_count, before.undo_count + 1, "{name} {what}");
                ok(&mut c, "undo", json!({}));
            }
            let alive = state_when_loaded(&mut c, at.clone());
            eprintln!("{name}: after {what}: {} bytes", alive["bytes"]);
        }
        let held = state_when_loaded(&mut c, at.clone());
        kept.push((name.to_string(), at.clone(), sha(&held)));
    }

    let folder = scratch("real-songs").display().to_string();
    ok(
        &mut c,
        "save_song_as",
        json!({ "folder": folder, "name": "real" }),
    );
    let file = Path::new(&folder).join("real/real.meloet");
    ok(&mut c, "new_song", json!({}));
    ok(
        &mut c,
        "open_song",
        json!({ "path": file.display().to_string() }),
    );
    for (name, at, sha256) in kept {
        let opened = state_when_loaded(&mut c, at);
        eprintln!(
            "{name}: opened again: sha256 {} (saved {sha256})",
            opened["sha256"]
        );
        assert_eq!(sha(&opened), sha256, "{name}");
    }
}

/// A state whose container (`MV3S` or `ML2S`, see the plugin host's formats) is whole but
/// whose inside is garbage, or the plugin's own state cut in half: what reaches the plugin.
fn inside(whole: &[u8], cut: bool) -> Vec<u8> {
    let garbage = [0xA5u8; 40];
    if whole.starts_with(b"MV3S") {
        let len = u64::from_le_bytes(whole[4..12].try_into().unwrap()) as usize;
        let component = &whole[12..12 + len];
        let body = if cut {
            &component[..len / 2]
        } else {
            &garbage[..]
        };
        let mut state = b"MV3S".to_vec();
        state.extend_from_slice(&(body.len() as u64).to_le_bytes());
        state.extend_from_slice(body);
        state
    } else {
        let count = u32::from_le_bytes(whole[4..8].try_into().unwrap()) as usize;
        let text = &whole[8 + count * 12..];
        let body: &[u8] = if cut {
            &text[..text.len() / 2]
        } else {
            b"@prefix x: <urn:x#> . this is not turtle"
        };
        let mut state = b"ML2S".to_vec();
        state.extend_from_slice(&0u32.to_le_bytes());
        state.extend_from_slice(body);
        state
    }
}

/// The key of a plugin by its name and format.
fn find_plugin(c: &mut Client, name: &str, format: &str) -> Option<String> {
    let list = ok(c, "list_instruments", json!({}));
    let effects = ok(c, "list_effects", json!({ "name": name, "limit": 500 }));
    let all = (list["plugins"].as_array().into_iter().flatten())
        .chain(effects["plugins"].as_array().into_iter().flatten());
    // A row of one plugin in two formats names one key; `formats` has both.
    let keys = all
        .filter(|p| p["name"] == name)
        .flat_map(|p| p["formats"].as_array().cloned().unwrap_or_default())
        .filter_map(|f| f["key"].as_str().map(str::to_owned));
    keys.into_iter()
        .find(|key| key.starts_with(&format!("{format}:")))
}

#[test]
fn the_state_tools_say_what_they_are() {
    let r = running("state-list");
    let mut c = Client::start(&r.socket);
    let list = c.request("tools/list", json!({}), &mut Vec::new());
    let tools = list["result"]["tools"].as_array().unwrap();
    let named = |n: &str| (tools.iter()).find(|t| t["name"] == n).unwrap().clone();
    let get = named("get_plugin_state");
    assert_eq!(get["annotations"]["readOnlyHint"], true);
    assert_eq!(get["annotations"]["openWorldHint"], false);
    let set = named("set_plugin_state");
    assert_eq!(set["annotations"]["destructiveHint"], true);
    assert_eq!(set["annotations"]["openWorldHint"], false);
    // No plugins in this core: the place is checked first.
    let text = refused(&mut c, "get_plugin_state", json!({ "track": 1 }));
    assert!(text.contains("no plugin"), "{text}");
}

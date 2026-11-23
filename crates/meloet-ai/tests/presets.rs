//! Presets through the socket. The fake LV2 `choices` of `meloet-plughost` has two presets of
//! its own (level 0.125 and 0.875) and lists the preset files of the bundles meloet finds for
//! it in the test's own HOME-like folder. VST3 preset files are read and written by
//! `meloet_ai`'s `presets::vst3` (its unit tests).
//!
//! The ignored test at the end tries the real plugins of this PC (ZamComp LV2 and VST3,
//! ZynAddSubFX's presets): `cargo test -p meloet-ai --test presets -- --ignored --nocapture`.

mod common;
use common::*;

#[path = "../../meloet-core/tests/plughost/mod.rs"]
mod plughost;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use meloet_ai::{AiLink, CoreLink, Options, PresetDirs};
use meloet_core::Core;
use meloet_core::plugins::PluginOptions;
use meloet_protocol::{AiPort, Command};
use serde_json::{Value, json};

const CHOICES: &str = "lv2:urn:meloet:fake:choices";

/// A core whose plugins are found in `places`, with its AI link; presets are looked for in
/// `presets`.
fn running_with(runtime: &Path, places: Vec<PathBuf>, fake: bool, presets: PresetDirs) -> Running {
    let options = Options {
        presets,
        ..options(runtime)
    };
    let socket = options.socket_path().unwrap();
    let core = Arc::new(Core::start(meloet_core::CoreOptions {
        songs_dir: Some(runtime.join("songs")),
        plugins: Some(PluginOptions {
            host_exe: plughost::exe(),
            cache_dir: Some(runtime.join("cache")),
            usual_places: places,
            scan_timeout: Duration::from_secs(30),
            child_env: if fake {
                plughost::fake_env()
            } else {
                Vec::new()
            },
            wait_for_screen: false,
        }),
        ..offline(runtime)
    }));
    let link = AiLink::start(Arc::new(CoreLink(Arc::downgrade(&core))), options).unwrap();
    core.set_ai_switch(Box::new(move |on| link.switch(on)));
    core.send(Command::SetAiLink { on: true });
    wait_for("the socket", || socket.exists());
    wait_for("Waiting", || port_of(&core) == AiPort::Waiting);
    Running { core, socket }
}

/// Waits until a call works, for `within` at most.
fn until_ok(c: &mut Client, tool: &str, args: Value, within: Duration) -> Value {
    let until = Instant::now() + within;
    loop {
        let answer = c.call(tool, args.clone());
        if answer["isError"] == false {
            return answer["structuredContent"].clone();
        }
        assert!(Instant::now() < until, "{tool} never worked: {answer}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn names(list: &Value) -> Vec<String> {
    (list["presets"].as_array().unwrap().iter())
        .map(|p| p["name"].as_str().unwrap().to_owned())
        .collect()
}

fn level(c: &mut Client) -> f64 {
    let params = ok(c, "get_plugin_params", json!({ "track": 2 }));
    params["params"][0]["value"].as_f64().unwrap()
}

#[test]
fn presets_of_an_lv2_plugin_are_listed_loaded_saved_and_kept_with_the_song() {
    let runtime = scratch("presets");
    let places = runtime.join("plugins");
    std::fs::create_dir_all(places.join("choices.lv2")).unwrap();
    std::fs::write(places.join("choices.lv2/plugin.so"), [0u8; 16]).unwrap();
    let home = runtime.join("home");
    let r = running_with(&runtime, vec![places], true, PresetDirs::home(&home));
    let mut c = Client::start(&r.socket);
    let patience = Duration::from_secs(60);
    until_ok(&mut c, "add_track", json!({ "plugin": CHOICES }), patience);
    let at = json!({ "track": 2 });

    // Its own two, in their bank.
    let all = until_ok(&mut c, "list_presets", at.clone(), patience);
    assert_eq!(names(&all), ["Loud", "Quiet"], "{all}");
    assert_eq!(all["plugin"]["key"], CHOICES);
    assert_eq!(all["plugin"]["format"], "lv2");
    assert_eq!(all["total"], 2);
    assert_eq!(all["next_offset"], Value::Null);
    let loud = all["presets"][0].clone();
    assert_eq!(loud["category"], "Fake Bank");
    assert_eq!(loud["factory"], true);
    assert_eq!(loud["source"], "lv2");
    assert_eq!(loud["id"], "lv2:urn:meloet:fake:preset:loud");
    assert_eq!(
        all["categories"],
        json!([{ "name": "Fake Bank", "count": 2 }])
    );
    assert_eq!(all["sources"][0]["source"], "lv2");
    assert_eq!(all["sources"][0]["found"], 2);

    // Filters and pages.
    let with = |extra: Value| {
        let mut args = at.clone();
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        args
    };
    let quiet = ok(&mut c, "list_presets", with(json!({ "name": "QUI" })));
    assert_eq!(names(&quiet), ["Quiet"]);
    let bank = ok(&mut c, "list_presets", with(json!({ "category": "bank" })));
    assert_eq!(bank["total"], 2);
    let none = ok(&mut c, "list_presets", with(json!({ "category": "drums" })));
    assert_eq!(none["total"], 0);
    let user = ok(&mut c, "list_presets", with(json!({ "kind": "user" })));
    assert_eq!(user["total"], 0);
    let first = ok(&mut c, "list_presets", with(json!({ "limit": 1 })));
    assert_eq!(
        (first["count"].clone(), first["next_offset"].clone()),
        (json!(1), json!(1))
    );
    let second = ok(
        &mut c,
        "list_presets",
        with(json!({ "limit": 1, "offset": 1 })),
    );
    assert_eq!(names(&second), ["Quiet"]);
    assert_eq!(second["next_offset"], Value::Null);

    // Loading: one step, undone like any other; the same again is no change.
    let before = steps(&r);
    let loaded = ok(&mut c, "load_preset", with(json!({ "id": loud["id"] })));
    assert_eq!(loaded["changed"], true, "{loaded}");
    assert_eq!(loaded["preset"]["id"], loud["id"]);
    assert_eq!(steps(&r), before + 1);
    assert_eq!(level(&mut c), 0.875);
    let again = ok(&mut c, "load_preset", with(json!({ "id": loud["id"] })));
    assert_eq!(again["changed"], false, "{again}");
    assert_eq!(steps(&r), before + 1);
    ok(&mut c, "undo", json!({}));
    assert_eq!(level(&mut c), 0.5);
    ok(&mut c, "load_preset", with(json!({ "id": loud["id"] })));
    assert_eq!(steps(&r), before + 1);
    assert_eq!(level(&mut c), 0.875);

    // Saved as the person's preset: a bundle of their own LV2 folder, as jalv names it.
    let saved = ok(&mut c, "save_preset", with(json!({ "name": "My Pad #1" })));
    let bundle = home.join(".lv2/Fake_Choices_My_Pad__1.preset.lv2");
    let written = PathBuf::from(saved["written"].as_str().unwrap());
    assert!(written.starts_with(&bundle), "{saved}");
    assert!(written.is_file());
    assert_eq!(saved["overwrote"], false);
    assert_eq!(saved["replaced_characters"], json!([" ", "#"]));
    assert_eq!(saved["preset"]["name"], "My Pad #1");
    assert_eq!(saved["preset"]["factory"], false);
    assert_eq!(steps(&r), before + 1, "saving a preset is no step");
    let mine = ok(&mut c, "list_presets", with(json!({ "kind": "user" })));
    assert_eq!(names(&mine), ["My Pad #1"]);
    assert_eq!(mine["presets"][0]["id"], saved["preset"]["id"]);
    assert_eq!(ok(&mut c, "list_presets", at.clone())["total"], 3);

    // The same name: only with `overwrite`; the file is not touched without it.
    let text = refused(&mut c, "save_preset", with(json!({ "name": "My Pad #1" })));
    assert!(text.contains("overwrite"), "{text}");
    ok(
        &mut c,
        "set_param",
        with(json!({ "param": 0, "value": 0.25 })),
    );
    let over = ok(
        &mut c,
        "save_preset",
        with(json!({ "name": "My Pad #1", "overwrite": true })),
    );
    assert_eq!(over["overwrote"], true);
    ok(&mut c, "load_preset", with(json!({ "id": loud["id"] })));
    assert_eq!(level(&mut c), 0.875);
    ok(
        &mut c,
        "load_preset",
        with(json!({ "id": saved["preset"]["id"] })),
    );
    assert_eq!(level(&mut c), 0.25);

    // Not in a batch, and wrong asks change nothing.
    let held = steps(&r);
    let ops = json!({ "operations": [{ "tool": "load_preset", "arguments": with(json!({ "id": loud["id"] })) }] });
    assert!(refused(&mut c, "batch", ops).contains("cannot be in a batch"));
    for (args, says) in [
        (
            with(json!({ "id": "lv2:urn:meloet:fake:preset:none" })),
            "no preset",
        ),
        (with(json!({ "id": "vstpreset:/etc/passwd" })), "no preset"),
        (with(json!({ "id": "nothing" })), "no preset"),
        (json!({ "track": 1, "id": loud["id"] }), "no plugin"),
    ] {
        let text = refused(&mut c, "load_preset", args.clone());
        assert!(text.contains(says), "{args}: {text}");
    }
    for name in ["a/b", "..", "", "\u{7}"] {
        let text = refused(&mut c, "save_preset", with(json!({ "name": name })));
        assert!(text.contains("1 to 64"), "{name:?}: {text}");
    }
    assert!(refused(&mut c, "list_presets", json!({ "track": 1 })).contains("no plugin"));
    assert_eq!(steps(&r), held);

    // Kept with the song: saved, opened again, the plugin has the preset's state.
    ok(&mut c, "load_preset", with(json!({ "id": loud["id"] })));
    let folder = scratch("presets-songs").display().to_string();
    ok(
        &mut c,
        "save_song_as",
        json!({ "folder": folder, "name": "presets" }),
    );
    let file = Path::new(&folder).join("presets/presets.meloet");
    ok(&mut c, "new_song", json!({}));
    ok(
        &mut c,
        "open_song",
        json!({ "path": file.display().to_string() }),
    );
    let state = until_ok(
        &mut c,
        "get_plugin_state",
        json!({ "track": 2, "inline": true }),
        patience,
    );
    assert_eq!(state["bytes"], 24);
    let params = until_ok(&mut c, "get_plugin_params", at.clone(), patience);
    assert_eq!(params["params"][0]["value"], 0.875);
}

#[test]
fn the_preset_tools_say_what_they_are() {
    let r = running("presets-list");
    let mut c = Client::start(&r.socket);
    let list = c.request("tools/list", json!({}), &mut Vec::new());
    let tools = list["result"]["tools"].as_array().unwrap();
    let named = |n: &str| (tools.iter()).find(|t| t["name"] == n).unwrap().clone();
    assert_eq!(named("list_presets")["annotations"]["readOnlyHint"], true);
    for n in ["load_preset", "save_preset"] {
        assert_eq!(named(n)["annotations"]["destructiveHint"], true, "{n}");
        assert_eq!(named(n)["annotations"]["readOnlyHint"], false, "{n}");
    }
    let text = refused(&mut c, "list_presets", json!({ "track": 1 }));
    assert!(text.contains("no plugin"), "{text}");
    let text = refused(
        &mut c,
        "save_preset",
        json!({ "bus": "master", "name": "x" }),
    );
    assert!(text.contains("slot"), "{text}");
}

/// The plugin of `name` in `format` once the scan has found it.
fn find_plugin(c: &mut Client, name: &str, format: &str) -> Option<String> {
    let list = ok(c, "list_instruments", json!({}));
    let effects = ok(c, "list_effects", json!({}));
    let all = (list["plugins"].as_array().into_iter().flatten())
        .chain(effects["plugins"].as_array().into_iter().flatten());
    let keys = (all.filter(|p| p["name"] == name))
        .flat_map(|p| p["formats"].as_array().cloned().unwrap_or_default())
        .filter_map(|f| f["key"].as_str().map(str::to_owned));
    keys.into_iter()
        .find(|key| key.starts_with(&format!("{format}:")))
}

fn param_values(c: &mut Client, at: &Value) -> Vec<Value> {
    let params = ok(c, "get_plugin_params", at.clone());
    (params["params"].as_array().unwrap().iter())
        .map(|p| p["value"].clone())
        .collect()
}

/// ZamComp (LV2 and VST3) and ZynAddSubFX of this PC, with presets looked for in the
/// person's folders of a temporary HOME and in `/usr/lib/lv2`: LV2 presets listed, one
/// loaded (one step, the parameters change, undo brings them back), saved with the song and
/// opened again; a VST3 preset saved, the sound changed, and the preset loaded back.
#[test]
#[ignore = "needs ZamComp (LV2 and VST3) and ZynAddSubFX installed"]
fn real_presets() {
    let runtime = scratch("presets-real");
    let places = runtime.join("plugins");
    std::fs::create_dir_all(&places).unwrap();
    for bundle in [
        "/usr/lib/lv2/ZamComp.lv2",
        "/usr/lib/lv2/ZynAddSubFX.lv2",
        "/usr/lib/vst3/ZamComp.vst3",
    ] {
        let name = Path::new(bundle).file_name().unwrap();
        std::os::unix::fs::symlink(bundle, places.join(name)).unwrap();
    }
    let home = runtime.join("home");
    let presets = PresetDirs {
        lv2_shared: vec![PathBuf::from("/usr/lib/lv2")],
        vst3_shared: vec![PathBuf::from("/usr/share/vst3/presets")],
        ..PresetDirs::home(&home)
    };
    let r = running_with(&runtime, vec![places], false, presets);
    let mut c = Client::start(&r.socket);
    let scan = Duration::from_secs(120);
    let until = Instant::now() + scan;
    let (zam_lv2, zam_vst3, zyn) = loop {
        let found = (
            find_plugin(&mut c, "ZamComp", "lv2"),
            find_plugin(&mut c, "ZamComp", "vst3"),
            find_plugin(&mut c, "ZynAddSubFX", "lv2"),
        );
        if let (Some(a), Some(b), Some(z)) = found {
            break (a, b, z);
        }
        assert!(Instant::now() < until, "the scan found {found:?}");
        std::thread::sleep(Duration::from_millis(200));
    };
    eprintln!("keys: {zam_lv2}, {zam_vst3}, {zyn}");
    ok(&mut c, "add_track", json!({ "sound": "grandPiano" }));
    ok(
        &mut c,
        "insert_effect",
        json!({ "track": 2, "slot": 1, "effect": zam_lv2 }),
    );
    ok(
        &mut c,
        "insert_effect",
        json!({ "track": 2, "slot": 2, "effect": zam_vst3 }),
    );
    ok(&mut c, "add_track", json!({ "plugin": zyn }));
    let lv2_at = json!({ "track": 2, "slot": 1 });
    let vst3_at = json!({ "track": 2, "slot": 2 });
    let zyn_at = json!({ "track": 3 });

    // ZamComp's three LV2 presets: one loaded, one step; undo brings the values back.
    let listed = until_ok(&mut c, "list_presets", lv2_at.clone(), scan);
    eprintln!("ZamComp LV2: {}", listed["presets"]);
    assert_eq!(listed["total"], 3, "{listed}");
    let poppy = (listed["presets"].as_array().unwrap().iter())
        .find(|p| p["name"] == "PoppySnare")
        .expect("PoppySnare")
        .clone();
    let before_values = param_values(&mut c, &lv2_at);
    let before = steps(&r);
    let mut with = lv2_at.clone();
    with["id"] = poppy["id"].clone();
    let loaded = ok(&mut c, "load_preset", with.clone());
    eprintln!("loaded: {loaded}");
    assert_eq!(steps(&r), before + 1);
    let poppy_values = param_values(&mut c, &lv2_at);
    eprintln!("values {before_values:?} -> {poppy_values:?}");
    assert_ne!(poppy_values, before_values);
    assert_eq!(ok(&mut c, "load_preset", with.clone())["changed"], false);
    ok(&mut c, "undo", json!({}));
    assert_eq!(param_values(&mut c, &lv2_at), before_values);
    ok(&mut c, "load_preset", with.clone());
    assert_eq!(param_values(&mut c, &lv2_at), poppy_values);

    // ZynAddSubFX: its bundle of presets in /usr/lib/lv2, in banks.
    let started = Instant::now();
    let zyn_list = until_ok(&mut c, "list_presets", zyn_at.clone(), scan);
    eprintln!(
        "ZynAddSubFX: {} presets, {} banks, listed in {:?}; first page {} chars; {}",
        zyn_list["total"],
        zyn_list["categories_total"],
        started.elapsed(),
        zyn_list.to_string().len(),
        zyn_list["categories"]
    );
    assert!(zyn_list["total"].as_u64().unwrap() >= 1000);
    let bass = ok(
        &mut c,
        "list_presets",
        json!({ "track": 3, "category": "bass", "limit": 3 }),
    );
    eprintln!("Bass: {}", bass["presets"]);
    let started = Instant::now();
    let zyn_loaded = ok(
        &mut c,
        "load_preset",
        json!({ "track": 3, "id": bass["presets"][0]["id"] }),
    );
    eprintln!(
        "ZynAddSubFX preset loaded in {:?}: {zyn_loaded}",
        started.elapsed()
    );

    // The VST3 one: saved, changed, loaded back.
    let vst3_list = until_ok(&mut c, "list_presets", vst3_at.clone(), scan);
    eprintln!("ZamComp VST3: {vst3_list}");
    let mut save = vst3_at.clone();
    save["name"] = json!("k11 Snare");
    save["category"] = json!("Tests");
    let saved = ok(&mut c, "save_preset", save);
    eprintln!("saved: {saved}");
    let file = PathBuf::from(saved["written"].as_str().unwrap());
    assert!(
        file.starts_with(home.join(".vst3/presets")),
        "{}",
        file.display()
    );
    let kept = until_ok(&mut c, "get_plugin_state", vst3_at.clone(), scan);
    // ZamComp's VST3 takes a parameter only in a block, and no sound runs here: its state
    // (DPF's, "name\0value\0" pairs) is changed instead, the release from 80 to 40 ms.
    let bytes = std::fs::read(kept["file"].as_str().unwrap()).unwrap();
    let at =
        (bytes.windows(7).position(|w| w == b"rel\x0080\x00")).expect("the release in the state");
    let mut other = bytes.clone();
    other[at + 4..at + 6].copy_from_slice(b"40");
    let other_file = runtime.join("other-state.bin");
    std::fs::write(&other_file, &other).unwrap();
    let mut set = vst3_at.clone();
    set["file"] = json!(other_file.display().to_string());
    ok(&mut c, "set_plugin_state", set);
    let changed = ok(&mut c, "get_plugin_state", vst3_at.clone());
    assert_ne!(changed["sha256"], kept["sha256"]);
    let listed = ok(
        &mut c,
        "list_presets",
        json!({ "track": 2, "slot": 2, "kind": "user" }),
    );
    assert_eq!(listed["presets"][0]["name"], "k11 Snare", "{listed}");
    let mut load = vst3_at.clone();
    load["id"] = listed["presets"][0]["id"].clone();
    ok(&mut c, "load_preset", load);
    let back = ok(&mut c, "get_plugin_state", vst3_at.clone());
    eprintln!(
        "VST3 sha256: kept {}, changed {}, back {}",
        kept["sha256"], changed["sha256"], back["sha256"]
    );
    assert_eq!(back["sha256"], kept["sha256"]);

    // Saved with the song and opened again: the LV2 preset's values are there.
    let folder = scratch("presets-real-songs").display().to_string();
    ok(
        &mut c,
        "save_song_as",
        json!({ "folder": folder, "name": "real" }),
    );
    let song = Path::new(&folder).join("real/real.meloet");
    ok(&mut c, "new_song", json!({}));
    ok(
        &mut c,
        "open_song",
        json!({ "path": song.display().to_string() }),
    );
    until_ok(&mut c, "get_plugin_state", lv2_at.clone(), scan);
    assert_eq!(param_values(&mut c, &lv2_at), poppy_values);
    eprintln!("opened again with the preset's values");
}

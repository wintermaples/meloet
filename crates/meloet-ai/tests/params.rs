//! Parameters through the socket: pages and filters of a plugin's list, the names of the
//! values of choice parameters, setting by value or by name, several as one step, and the
//! answer that says whether the song holds what was set. The plugin is the fake `choices` of
//! `meloet-plughost` (its `Loud` is not in its state); the core runs no audio.
//!
//! The real plugins of this PC (Surge XT, ZamComp; VST3 and LV2) are tried by the ignored
//! test at the end, with no sound running, or with the sound on an output of
//! `MELOET_PARAMS_OUTPUT` (an id of `state.outputs.list`; `list` lists them):
//!
//! ```text
//! cargo test -p meloet-ai --test params -- --ignored --nocapture
//! MELOET_PARAMS_OUTPUT=<id> cargo test -p meloet-ai --test params -- --ignored --nocapture
//! ```

mod common;
use common::*;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use meloet_ai::{AiLink, CoreLink};
use meloet_core::plugins::PluginOptions;
use meloet_core::{Core, CoreOptions};
use serde_json::{Value, json};

const CHOICES: &str = "lv2:urn:meloet:fake:choices";

/// The `meloet-plughost` of this build, with its fake plugins (a development hook). Built
/// into the target folder of this test; that does nothing when it is up to date.
fn plughost() -> PathBuf {
    static EXE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    EXE.get_or_init(|| {
        let test = std::env::current_exe().unwrap();
        let profile = (test.ancestors())
            .find(|d| {
                d.file_name()
                    .is_some_and(|n| n == "debug" || n == "release")
            })
            .expect("a target folder");
        let target = profile.parent().unwrap();
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut build = std::process::Command::new(cargo);
        build.args([
            "build",
            "--quiet",
            "-p",
            "meloet-plughost",
            "--features",
            "dev-hooks",
        ]);
        build.arg("--target-dir").arg(target);
        if profile.ends_with("release") {
            build.arg("--release");
        }
        assert!(build.status().unwrap().success(), "meloet-plughost builds");
        profile.join("meloet-plughost")
    })
    .clone()
}

fn bundle(root: &Path, path: &str) {
    let bundle = root.join(path);
    std::fs::create_dir_all(&bundle).unwrap();
    std::fs::write(bundle.join("plugin.so"), [0u8; 16]).unwrap();
}

/// A core in `runtime` whose plugins are found in `places` (with `env` for the plug-in
/// hosts), and its AI link, on. With `output` (an id of `state.outputs.list`) its sound runs
/// there.
fn running_with(
    runtime: PathBuf,
    places: Vec<PathBuf>,
    env: Vec<(OsString, OsString)>,
    output: Option<&str>,
) -> Running {
    let options = options(&runtime);
    let socket = options.socket_path().unwrap();
    let config = runtime.join("config");
    if let Some(output) = output {
        std::fs::create_dir_all(&config).unwrap();
        let settings =
            json!({ "version": 1, "output": output, "output_name": output, "scale": 100 });
        std::fs::write(config.join("settings.json"), settings.to_string()).unwrap();
    }
    let core = Arc::new(Core::start(CoreOptions {
        config_dir: Some(config),
        state_dir: Some(runtime.join("state")),
        devices: output.is_some(),
        audio: output.is_some(),
        plugins: Some(PluginOptions {
            host_exe: plughost(),
            cache_dir: Some(runtime.join("cache")),
            usual_places: places,
            scan_timeout: Duration::from_secs(30),
            child_env: env,
            wait_for_screen: false,
        }),
        ..CoreOptions::offline()
    }));
    if output.is_some() {
        wait_for("the sound to run", || core.snapshot().state.audio.running);
    }
    let link = AiLink::start(Arc::new(CoreLink(Arc::downgrade(&core))), options).unwrap();
    core.set_ai_switch(Box::new(move |on| link.switch(on)));
    core.send(meloet_protocol::Command::SetAiLink { on: true });
    wait_for("the socket", || socket.exists());
    Running { core, socket }
}

fn running_fake(name: &str) -> Running {
    let root = scratch(&format!("{name}-plugins"));
    bundle(&root, "lv2/choices.lv2");
    bundle(&root, "lv2/found.lv2");
    let env = vec![("MELOET_PLUGHOST_FAKE".into(), "1".into())];
    running_with(scratch(name), vec![root.join("lv2")], env, None)
}

/// Waits until a call works (the scan, the load), for `within` at most.
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

/// A new track with the plugin `key`, loaded; its number.
fn plugin_track(c: &mut Client, key: &str, within: Duration) -> u32 {
    let until = Instant::now() + within;
    while !(c.call("list_instruments", json!({}))["structuredContent"]["plugins"].to_string())
        .contains(key)
    {
        assert!(Instant::now() < until, "{key} was not found");
        std::thread::sleep(Duration::from_millis(100));
    }
    let added = ok(c, "add_track", json!({ "plugin": key }));
    let track = added["track"].as_u64().unwrap() as u32;
    until_ok(
        c,
        "get_plugin_params",
        json!({ "track": track, "limit": 1 }),
        within,
    );
    track
}

fn names(params: &Value) -> Vec<&str> {
    (params.as_array().unwrap().iter())
        .map(|p| p["name"].as_str().unwrap())
        .collect()
}

fn text(answer: &Value) -> &str {
    answer["content"][0]["text"].as_str().unwrap()
}

#[test]
fn a_plugins_list_comes_in_pages_and_filters() {
    let r = running_fake("params-list");
    let mut c = Client::start(&r.socket);
    let track = plugin_track(&mut c, CHOICES, Duration::from_secs(30));

    let all = ok(&mut c, "get_plugin_params", json!({ "track": track }));
    assert_eq!(all["total"], 4);
    assert_eq!(all["next_offset"], Value::Null);
    assert_eq!(names(&all["params"]), ["Level", "Mode", "Loud", "Type"]);
    assert_eq!(all["plugin"]["key"], CHOICES);
    let mode = &all["params"][1];
    assert_eq!(mode["kind"], "steps");
    assert_eq!(
        mode["choices"],
        json!([{ "value": 0.0, "text": "Soft" }, { "value": 0.5, "text": "Hard" },
            { "value": 1.0, "text": "Odd" }])
    );
    // The plugin marks no steps for `Type`, but its text is a name.
    assert_eq!(all["params"][3]["kind"], "smooth");
    assert_eq!(all["params"][3]["likely_choice"], true);
    assert_eq!(all["params"][0].get("likely_choice"), None);

    let first = ok(
        &mut c,
        "get_plugin_params",
        json!({ "track": track, "limit": 3 }),
    );
    assert_eq!(
        (first["count"].clone(), first["next_offset"].clone()),
        (json!(3), json!(3))
    );
    let rest = ok(
        &mut c,
        "get_plugin_params",
        json!({ "track": track, "limit": 3, "offset": 3 }),
    );
    assert_eq!(names(&rest["params"]), ["Type"]);
    assert_eq!(rest["next_offset"], Value::Null);

    let found = ok(
        &mut c,
        "get_plugin_params",
        json!({ "track": track, "name": "TYP" }),
    );
    assert_eq!(names(&found["params"]), ["Type"]);
    let shape = ok(
        &mut c,
        "get_plugin_params",
        json!({ "track": track, "group": "shape" }),
    );
    assert_eq!(shape["total"], 3);
    let groups = ok(
        &mut c,
        "get_plugin_params",
        json!({ "track": track, "groups_only": true }),
    );
    assert_eq!(
        groups["groups"],
        json!([{ "name": null, "count": 1 }, { "name": "Shape", "count": 3 }])
    );
    let brief = ok(
        &mut c,
        "get_plugin_params",
        json!({ "track": track, "brief": true, "ids": [0] }),
    );
    let mut keys: Vec<&String> = brief["params"][0].as_object().unwrap().keys().collect();
    keys.sort();
    assert_eq!(keys, ["id", "name", "text", "value"]);
    // Nothing is changed yet; then the level is.
    let changed = json!({ "track": track, "changed_only": true });
    assert_eq!(ok(&mut c, "get_plugin_params", changed.clone())["total"], 0);
    ok(
        &mut c,
        "set_param",
        json!({ "track": track, "param": "Level", "value": 0.9 }),
    );
    let now = ok(&mut c, "get_plugin_params", changed);
    assert_eq!(names(&now["params"]), ["Level"]);

    // The names of the values, where the plugin names none.
    let scanned = ok(
        &mut c,
        "get_plugin_params",
        json!({ "track": track, "name": "type", "choices": true }),
    );
    let texts: Vec<&str> = (scanned["params"][0]["choices"].as_array().unwrap().iter())
        .map(|c| c["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, ["Saw", "Square", "Sine", "Noise"]);
}

#[test]
fn choices_by_name_and_sets_that_say_what_the_song_holds() {
    let r = running_fake("params-set");
    let mut c = Client::start(&r.socket);
    let track = plugin_track(&mut c, CHOICES, Duration::from_secs(30));

    let choices = ok(
        &mut c,
        "get_param_choices",
        json!({ "track": track, "param": "Type" }),
    );
    assert_eq!(
        (choices["list"].clone(), choices["source"].clone()),
        (json!(true), json!("scan"))
    );
    let list = choices["choices"].as_array().unwrap();
    assert_eq!(list.len(), 4);
    assert_eq!(list[0], json!({ "value": 0.0, "text": "Saw" }));
    assert_eq!(list[3], json!({ "value": 1.0, "text": "Noise" }));
    // Its value is inside the quarter of "Sine".
    let sine = list[2]["value"].as_f64().unwrap();
    assert!((0.5..0.75).contains(&sine), "{sine}");
    let stepped = ok(
        &mut c,
        "get_param_choices",
        json!({ "track": track, "param": 1 }),
    );
    assert_eq!(stepped["source"], "steps");
    let smooth = ok(
        &mut c,
        "get_param_choices",
        json!({ "track": track, "param": "Level" }),
    );
    assert_eq!(smooth["list"], false);
    assert!(
        smooth["note"].as_str().unwrap().contains("no list"),
        "{smooth}"
    );

    // By name: one step, with the value and text the plugin took.
    let before = steps(&r);
    let set = ok(
        &mut c,
        "set_param",
        json!({ "track": track, "param": "Type", "text": "sine" }),
    );
    assert_eq!(steps(&r), before + 1);
    assert_eq!(
        (set["changed"].clone(), set["in_song"].clone()),
        (json!(true), json!(true))
    );
    assert_eq!(set["text"], "Sine");
    assert_eq!(set["before"]["text"], "Saw");
    assert_eq!(set["undo_steps"], before + 1);
    assert!(set["step"].is_object(), "{set}");
    // Again: no step, and no error.
    let again = c.call(
        "set_param",
        json!({ "track": track, "param": "Type", "text": "Sine" }),
    );
    assert_eq!(again["isError"], false, "{again}");
    assert_eq!(again["structuredContent"]["changed"], false);
    assert!(text(&again).starts_with("Nothing changed"), "{again}");
    assert_eq!(steps(&r), before + 1);
    // A name it does not have: the names it has.
    let wrong = refused(
        &mut c,
        "set_param",
        json!({ "track": track, "param": "Type", "text": "Triangle" }),
    );
    assert!(
        wrong.contains("\"Saw\", \"Square\", \"Sine\", \"Noise\""),
        "{wrong}"
    );
    let smooth = refused(
        &mut c,
        "set_param",
        json!({ "track": track, "param": "Level", "text": "Loud" }),
    );
    assert!(smooth.contains("give `value`"), "{smooth}");

    // The plugin does not keep `Loud` in its state: the song cannot hold it, and says so.
    let loud = c.call(
        "set_param",
        json!({ "track": track, "param": "Loud", "value": 1.0 }),
    );
    assert_eq!(loud["isError"], true, "{loud}");
    let why = text(&loud);
    assert!(why.contains("does not hold"), "{why}");
    assert!(why.contains("\"in_song\":false"), "{why}");
    assert_eq!(steps(&r), before + 1);

    // Several: one step; each one's value and text.
    let set = ok(
        &mut c,
        "set_params",
        json!({ "track": track, "params": [
            { "param": "Level", "value": 0.25 },
            { "param": "Mode", "text": "Odd" },
            { "param": 3, "text": "Noise" },
        ] }),
    );
    assert_eq!(steps(&r), before + 2);
    let rows = set["params"].as_array().unwrap();
    let said: Vec<(&str, &str)> = (rows.iter())
        .map(|p| (p["name"].as_str().unwrap(), p["text"].as_str().unwrap()))
        .collect();
    assert_eq!(
        said,
        [("Level", "25 %"), ("Mode", "Odd"), ("Type", "Noise")]
    );
    assert_eq!(set["unchanged"], json!([]));
    // One unknown: nothing is set.
    let none = refused(
        &mut c,
        "set_params",
        json!({ "track": track, "params": [
            { "param": "Level", "value": 0.75 },
            { "param": "Volume", "value": 0.5 },
        ] }),
    );
    assert!(
        none.starts_with("Nothing was set") && none.contains("params[1]"),
        "{none}"
    );
    let level = ok(
        &mut c,
        "get_plugin_params",
        json!({ "track": track, "ids": [0] }),
    );
    assert_eq!(level["params"][0]["value"], 0.25);
    assert_eq!(steps(&r), before + 2);
    // Undo takes the several back at once.
    ok(&mut c, "undo", json!({}));
    let level = ok(
        &mut c,
        "get_plugin_params",
        json!({ "track": track, "ids": [0] }),
    );
    assert_eq!(level["params"][0]["value"], 0.5);
}

#[test]
fn fifty_sets_in_a_row_each_end_as_a_step() {
    let r = running_fake("params-fifty");
    let mut c = Client::start(&r.socket);
    let track = plugin_track(&mut c, CHOICES, Duration::from_secs(30));
    let before = steps(&r);
    let mut failed = Vec::new();
    for i in 0..50 {
        let value = 0.1 + 0.015 * f64::from(i);
        let answer = c.call(
            "set_param",
            json!({ "track": track, "param": 0, "value": value }),
        );
        if answer["isError"] != false || answer["structuredContent"]["changed"] != true {
            failed.push(answer);
        }
    }
    assert!(failed.is_empty(), "{} of 50: {failed:?}", failed.len());
    assert_eq!(steps(&r), before + 50);
}

#[test]
fn built_in_effects_by_value_and_by_name_and_the_list_of_effects() {
    let r = running_fake("params-builtin");
    let mut c = Client::start(&r.socket);
    ok(
        &mut c,
        "insert_effect",
        json!({ "track": 1, "slot": 1, "effect": "delay" }),
    );
    let before = steps(&r);
    let set = ok(
        &mut c,
        "set_params",
        json!({ "track": 1, "slot": 1, "params": [
            { "param": "intervalStep", "text": "sixteenth" },
            { "param": "feedback", "value": 50 },
            { "param": "sync", "text": "off" },
        ] }),
    );
    assert_eq!(steps(&r), before + 1);
    let said: Vec<&str> = (set["params"].as_array().unwrap().iter())
        .map(|p| p["text"].as_str().unwrap())
        .collect();
    assert_eq!(said, ["sixteenth", "50 %", "off"]);
    let again = ok(
        &mut c,
        "set_param",
        json!({ "track": 1, "slot": 1, "param": "feedback", "value": 50 }),
    );
    assert_eq!(again["changed"], false);
    assert_eq!(steps(&r), before + 1);
    let cut = ok(
        &mut c,
        "set_param",
        json!({ "track": 1, "slot": 1, "param": "highCut", "text": "2 kHz" }),
    );
    assert_eq!(
        (cut["value"].clone(), cut["text"].clone()),
        (json!(2000.0), json!("2000 Hz"))
    );
    assert_eq!(steps(&r), before + 2);
    let none = refused(
        &mut c,
        "set_params",
        json!({ "track": 1, "slot": 1, "params": [
            { "param": "mix", "value": 10 },
            { "param": "intervalStep", "text": "whole" },
        ] }),
    );
    assert!(none.starts_with("Nothing was set"), "{none}");
    assert_eq!(steps(&r), before + 2);
    let choices = ok(
        &mut c,
        "get_param_choices",
        json!({ "track": 1, "slot": 1, "param": "intervalStep" }),
    );
    assert_eq!(
        choices["choices"][4],
        json!({ "value": 4.0, "text": "sixteenth" })
    );

    // The list of effects: pages and filters.
    let all = ok(&mut c, "list_effects", json!({}));
    assert_eq!(all["builtin"].as_array().unwrap().len(), 6);
    assert_eq!(all["total"], 1);
    assert_eq!(all["plugins"][0]["name"], "Fake Pass");
    let reverb = ok(&mut c, "list_effects", json!({ "name": "rev" }));
    assert_eq!(reverb["builtin"][0]["effect"], "reverb");
    assert_eq!(reverb["total"], 0);
    let lv2 = ok(
        &mut c,
        "list_effects",
        json!({ "format": "lv2", "kind": "other" }),
    );
    assert_eq!(
        (lv2["total"].clone(), lv2["builtin"].clone()),
        (json!(1), json!([]))
    );
    let vst3 = ok(&mut c, "list_effects", json!({ "format": "vst3" }));
    assert_eq!(vst3["total"], 0);
    let paged = ok(&mut c, "list_effects", json!({ "limit": 1, "offset": 1 }));
    assert_eq!(
        (paged["count"].clone(), paged["builtin"].clone()),
        (json!(0), json!([]))
    );
    let brief = ok(&mut c, "list_effects", json!({ "brief": true }));
    assert_eq!(brief["builtin"][0], json!({ "effect": "eq4" }));
    assert_eq!(
        brief["plugins"][0],
        json!({ "key": "lv2:urn:meloet:fake:pass", "name": "Fake Pass", "vendor": "meloet",
            "kind": "other" })
    );
}

/// Surge XT and ZamComp of this PC, in both formats: choices, sets by name, fifty sets in a
/// row and the size of the answers. Says what it finds; a plugin that is not installed is
/// passed over.
#[test]
#[ignore = "needs Surge XT and ZamComp"]
fn real_plugins() {
    if std::env::var("MELOET_PARAMS_OUTPUT").is_ok_and(|v| v == "list") {
        let core = Core::start(CoreOptions {
            devices: true,
            ..CoreOptions::offline()
        });
        std::thread::sleep(Duration::from_secs(2));
        for o in core.snapshot().state.outputs.list {
            eprintln!("{:?} {:?}", o.id, o.name);
        }
        core.stop();
        return;
    }
    let root = scratch("params-real-plugins");
    let mut places = Vec::new();
    for (format, bundle) in [
        ("vst3", "/usr/lib/vst3/Surge XT.vst3"),
        ("lv2", "/usr/lib/lv2/Surge XT.lv2"),
        ("vst3", "/usr/lib/vst3/ZamComp.vst3"),
        ("lv2", "/usr/lib/lv2/ZamComp.lv2"),
    ] {
        let place = root.join(format);
        std::fs::create_dir_all(&place).unwrap();
        if Path::new(bundle).exists() {
            let name = Path::new(bundle).file_name().unwrap();
            let _ = std::os::unix::fs::symlink(bundle, place.join(name));
        }
        if !places.contains(&place) {
            places.push(place);
        }
    }
    let output = std::env::var("MELOET_PARAMS_OUTPUT").ok();
    let r = running_with(
        scratch("params-real"),
        places,
        Vec::new(),
        output.as_deref(),
    );
    let mut c = Client::start(&r.socket);
    let patience = Duration::from_secs(120);
    for key in [
        "vst3:ABCDEF019182FAEB566D624153675854",
        "lv2:https://surge-synthesizer.github.io/lv2/surge-xt",
    ] {
        let track = plugin_track(&mut c, key, patience);
        let all = c.call("get_plugin_params", json!({ "track": track }));
        let total = all["structuredContent"]["total"].clone();
        let size = all.to_string().len();
        let osc = c.call(
            "get_plugin_params",
            json!({ "track": track, "name": "a osc 1 type" }),
        );
        let brief = c.call(
            "get_plugin_params",
            json!({ "track": track, "name": "filter 1 type", "brief": true }),
        );
        eprintln!(
            "{key}: {total} parameters; first page {size} chars; \"a osc 1 type\" {} chars, \
             \"filter 1 type\" brief {} chars",
            osc.to_string().len(),
            brief.to_string().len()
        );
        for name in ["A Osc 1 Type", "A Filter 1 Type"] {
            let choices = c.call(
                "get_param_choices",
                json!({ "track": track, "param": name }),
            );
            eprintln!("  {name}: {}", choices["structuredContent"]);
        }
        for (param, text) in [("A Osc 1 Type", "Sine"), ("A Filter 1 Type", "LP 24 dB")] {
            let set = c.call(
                "set_param",
                json!({ "track": track, "param": param, "text": text }),
            );
            eprintln!("  set {param} to {text}: {}", set["content"][0]["text"]);
        }
        let mut errors = 0;
        let started = Instant::now();
        for i in 0..50 {
            let value = 0.1 + 0.015 * f64::from(i);
            let set = c.call(
                "set_param",
                json!({ "track": track, "param": "A Osc 1 Pitch", "value": value }),
            );
            if set["isError"] != false {
                errors += 1;
                if errors < 3 {
                    eprintln!("  error: {}", set["content"][0]["text"]);
                }
            }
        }
        eprintln!(
            "  50 sets in a row: {errors} errors, {:.1} s",
            started.elapsed().as_secs_f64()
        );
    }
    for key in [
        "vst3:2046504473616C63504D435A00000000",
        "lv2:urn:zamaudio:ZamComp",
    ] {
        ok(&mut c, "add_track", json!({ "sound": "grandPiano" }));
        let track = r.core.ai_song().unwrap().song.doc.tracks.len() as u32;
        ok(
            &mut c,
            "insert_effect",
            json!({ "track": track, "slot": 1, "effect": key }),
        );
        until_ok(
            &mut c,
            "get_plugin_params",
            json!({ "track": track, "slot": 1 }),
            patience,
        );
        let list = ok(
            &mut c,
            "get_plugin_params",
            json!({ "track": track, "slot": 1, "brief": true }),
        );
        eprintln!("{key}: {}", list["params"]);
        let mut errors = 0;
        for i in 0..50 {
            let value = 0.1 + 0.015 * f64::from(i);
            let set = c.call(
                "set_param",
                json!({ "track": track, "slot": 1, "param": "Attack", "value": value }),
            );
            if set["isError"] != false {
                errors += 1;
                if errors < 3 {
                    eprintln!("  error: {}", set["content"][0]["text"]);
                }
            }
        }
        let sound = if output.is_some() {
            "sound running"
        } else {
            "no sound running"
        };
        eprintln!("  50 sets in a row ({sound}): {errors} errors");
    }
}

/// The effects of this PC (every usual place), from a copy of this user's scan file when
/// there is one: how long `list_effects` was, and is a page at a time.
#[test]
#[ignore = "reads every plugin of this PC"]
fn effects_of_this_pc() {
    let root = scratch("params-effects");
    let home = PathBuf::from(std::env::var_os("HOME").unwrap());
    let cache = root.join("cache");
    std::fs::create_dir_all(&cache).unwrap();
    let _ = std::fs::copy(
        home.join(".cache/meloet/plugins/scan.json"),
        cache.join("scan.json"),
    );
    let places = [
        "/usr/lib/lv2",
        "/usr/local/lib/lv2",
        "/usr/lib/vst3",
        "/usr/local/lib/vst3",
    ];
    let places = places.iter().map(PathBuf::from).collect();
    let r = running_with(root, places, Vec::new(), None);
    let mut c = Client::start(&r.socket);
    // Until the scan has ended: the count stays the same for 10 s.
    let (mut last, mut since) = (0, Instant::now());
    let until = Instant::now() + Duration::from_secs(180);
    while Instant::now() < until && since.elapsed() < Duration::from_secs(10) {
        let total = ok(&mut c, "list_effects", json!({ "limit": 1 }))["total"].clone();
        let total = total.as_u64().unwrap();
        if total != last {
            (last, since) = (total, Instant::now());
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    let first = c.call("list_effects", json!({}));
    let mut all = 0;
    let mut offset = 0;
    loop {
        let page = c.call("list_effects", json!({ "limit": 500, "offset": offset }));
        all += page["structuredContent"]["plugins"].to_string().len();
        match page["structuredContent"]["next_offset"].as_u64() {
            Some(next) => offset = next,
            None => break,
        }
    }
    let builtin = first["structuredContent"]["builtin"].to_string().len();
    let reverbs = c.call("list_effects", json!({ "kind": "reverb" }));
    let brief = c.call("list_effects", json!({ "brief": true }));
    let text = |answer: &Value| answer["content"][0]["text"].as_str().unwrap().len();
    eprintln!(
        "{last} effect plugins: all of them {} chars (with the built-in six, as list_effects \
         gave before); the first page now {} chars of text, {} as a whole; brief {} chars of \
         text; kind reverb: {} plugins, {} chars of text",
        all + builtin,
        text(&first),
        first.to_string().len(),
        text(&brief),
        reverbs["structuredContent"]["total"],
        text(&reverbs),
    );
}

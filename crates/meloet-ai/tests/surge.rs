//! Surge XT through the socket: its patch as text, its modulation routing, its patch files.
//! The real plugin's tests are ignored: `cargo test -p meloet-ai --test surge -- --ignored
//! --nocapture --test-threads 1` (Surge XT VST3 and LV2 of this PC).

mod common;
use common::*;

#[path = "../../meloet-core/tests/plughost/mod.rs"]
mod plughost;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use meloet_ai::surge::presets::{self, Roots};
use meloet_ai::{AiLink, CoreLink};
use meloet_core::plugins::PluginOptions;
use meloet_core::{Core, CoreOptions};
use meloet_model::PluginRef;
use meloet_protocol::{AiPort, Command};
use serde_json::{Value, json};

const SINE: &str = "lv2:urn:meloet:fake:sine";

/// A core whose plugins are found in `places`, with its AI link.
fn running_with_plugins(name: &str, places: Vec<PathBuf>, fake: bool) -> Running {
    let runtime = scratch(name);
    let options = options(&runtime);
    let socket = options.socket_path().unwrap();
    let core_options = CoreOptions {
        songs_dir: Some(runtime.join("songs")),
        plugins: Some(PluginOptions {
            host_exe: plughost::exe(),
            cache_dir: Some(runtime.join("cache")),
            usual_places: places,
            scan_timeout: Duration::from_secs(60),
            child_env: if fake {
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

fn waiting<T>(what: &str, most: Duration, mut look: impl FnMut() -> Option<T>) -> T {
    let until = Instant::now() + most;
    loop {
        if let Some(found) = look() {
            return found;
        }
        assert!(Instant::now() < until, "waited too long for {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn patiently<T>(what: &str, look: impl FnMut() -> Option<T>) -> T {
    waiting(what, Duration::from_secs(90), look)
}

fn has_plugin(c: &mut Client, key: &str) -> bool {
    let list = ok(c, "list_instruments", json!({}));
    (list["plugins"].as_array().into_iter().flatten()).any(|p| p["key"] == key)
}

/// The key of a plugin by its name and format.
fn find_plugin(c: &mut Client, name: &str, format: &str) -> Option<String> {
    let list = ok(c, "list_instruments", json!({}));
    let all = list["plugins"].as_array().into_iter().flatten();
    let keys = all
        .filter(|p| p["name"] == name)
        .flat_map(|p| p["formats"].as_array().cloned().unwrap_or_default())
        .filter_map(|f| f["key"].as_str().map(str::to_owned));
    keys.into_iter()
        .find(|key| key.starts_with(&format!("{format}:")))
}

/// A core with only Surge XT's two bundles of this PC, and its keys (VST3, LV2).
fn surge_core(name: &str) -> (Running, Client, String, String) {
    let vst3 = scratch(&format!("{name}-vst3"));
    let lv2 = scratch(&format!("{name}-lv2"));
    std::os::unix::fs::symlink("/usr/lib/vst3/Surge XT.vst3", vst3.join("Surge XT.vst3")).unwrap();
    std::os::unix::fs::symlink("/usr/lib/lv2/Surge XT.lv2", lv2.join("Surge XT.lv2")).unwrap();
    let r = running_with_plugins(name, vec![vst3, lv2], false);
    let mut c = Client::start(&r.socket);
    let vst3 = patiently("Surge XT (VST3)", || {
        find_plugin(&mut c, "Surge XT", "vst3")
    });
    let lv2 = patiently("Surge XT (LV2)", || find_plugin(&mut c, "Surge XT", "lv2"));
    (r, c, vst3, lv2)
}

/// The state of a plugin once it is loaded.
fn state_when_loaded(c: &mut Client, at: Value) -> Value {
    patiently("the plugin to be loaded", || {
        let answer = c.call("get_plugin_state", at.clone());
        (answer["isError"] == false).then(|| answer["structuredContent"].clone())
    })
}

/// The patch XML of the plugin at `at` now.
fn patch_xml(c: &mut Client, at: &Value) -> String {
    let mut args = at.clone();
    args["readable_inline"] = json!(true);
    let got = state_when_loaded(c, args);
    assert_eq!(got["readable"]["kind"], "xml", "{got}");
    got["readable"]["text"].as_str().unwrap().to_owned()
}

/// The `<meta name="…">` of a patch.
fn patch_name(xml: &str) -> String {
    let at = xml.find("<meta name=\"").unwrap() + 12;
    xml[at..at + xml[at..].find('"').unwrap()].to_owned()
}

/// The value of a parameter element of a patch.
fn patch_value(xml: &str, name: &str) -> Option<f64> {
    let at = xml.find(&format!("<{name} "))?;
    let element = &xml[at..at + xml[at..].find('>')?];
    let v = element.find("value=\"")? + 7;
    element[v..v + element[v..].find('"')?].parse().ok()
}

fn routings(c: &mut Client, at: &Value, target: &str) -> Vec<Value> {
    let mut args = at.clone();
    args["target"] = json!(target);
    let list = ok(c, "surge_list_modulations", args);
    list["routings"].as_array().cloned().unwrap_or_default()
}

#[test]
fn surge_tools_refuse_other_plugins_and_say_what_they_are() {
    let lv2 = scratch("surge-fake-lv2");
    let bundle = lv2.join("found.lv2");
    std::fs::create_dir_all(&bundle).unwrap();
    std::fs::write(bundle.join("plugin.so"), [0u8; 16]).unwrap();
    let r = running_with_plugins("surge-fake", vec![lv2], true);
    let mut c = Client::start(&r.socket);
    patiently("the scan", || has_plugin(&mut c, SINE).then_some(()));
    ok(&mut c, "add_track", json!({ "plugin": SINE }));
    let loaded = state_when_loaded(&mut c, json!({ "track": 2 }));
    assert_eq!(loaded["readable"], Value::Null);
    let steps0 = steps(&r);
    for (tool, args) in [
        ("surge_list_modulations", json!({ "track": 2 })),
        ("surge_get_patch_params", json!({ "track": 2 })),
        (
            "surge_set_modulation",
            json!({ "track": 2, "target": "a_pitch", "source": "LFO 1", "depth": 0.5 }),
        ),
        (
            "surge_remove_modulation",
            json!({ "track": 2, "target": "a_pitch" }),
        ),
        (
            "surge_set_patch_params",
            json!({ "track": 2, "params": [{ "name": "a_lfo0_rate", "hz": 5 }] }),
        ),
    ] {
        let text = refused(&mut c, tool, args);
        assert!(text.contains("is not Surge XT"), "{tool}: {text}");
    }
    let text = refused(
        &mut c,
        "set_plugin_state_text",
        json!({ "track": 2, "text": "<patch/>" }),
    );
    assert!(text.contains("only Surge XT"), "{text}");
    // Arguments are checked before the plugin is.
    let text = refused(
        &mut c,
        "surge_set_modulation",
        json!({ "track": 2, "target": "a_pitch", "source": "LFO 9", "depth": 1 }),
    );
    assert!(text.contains("there is no source"), "{text}");
    let text = refused(
        &mut c,
        "surge_set_patch_params",
        json!({ "track": 2, "params": [{ "name": "fx1_type", "value": 2 }] }),
    );
    assert!(text.contains("set_param"), "{text}");
    for (args, says) in [
        (json!({ "track": 2 }), "one of them"),
        (json!({ "track": 2, "file": "patch.xml" }), "absolute"),
        (json!({ "track": 1, "text": "<patch/>" }), "no plugin"),
    ] {
        let text = refused(&mut c, "set_plugin_state_text", args.clone());
        assert!(text.contains(says), "{args}: {text}");
    }
    assert_eq!(steps(&r), steps0);

    let list = c.request("tools/list", json!({}), &mut Vec::new());
    let tools = list["result"]["tools"].as_array().unwrap();
    let named = |n: &str| (tools.iter()).find(|t| t["name"] == n).unwrap().clone();
    for name in ["surge_list_modulations", "surge_get_patch_params"] {
        assert_eq!(named(name)["annotations"]["readOnlyHint"], true, "{name}");
    }
    for name in [
        "surge_set_modulation",
        "surge_remove_modulation",
        "surge_set_patch_params",
        "set_plugin_state_text",
    ] {
        let tool = named(name);
        assert_eq!(tool["annotations"]["readOnlyHint"], false, "{name}");
        assert_eq!(tool["annotations"]["openWorldHint"], false, "{name}");
        let text = refused(
            &mut c,
            "batch",
            json!({ "operations": [{ "tool": name, "arguments": { "track": 2 } }] }),
        );
        assert!(text.contains("by itself"), "{name}: {text}");
    }
    assert_eq!(
        named("surge_remove_modulation")["annotations"]["destructiveHint"],
        true
    );
}

// ---- Measuring the pitch of a rendered note ----

/// The first channel of a 16-bit PCM WAV file, and its sample rate.
fn wav(path: &Path) -> (Vec<f64>, f64) {
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    let (mut at, mut channels, mut rate, mut samples) = (12, 0usize, 0u32, Vec::new());
    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = &bytes[at + 8..(at + 8 + len).min(bytes.len())];
        if id == b"fmt " {
            channels = usize::from(u16::from_le_bytes([body[2], body[3]]));
            rate = u32::from_le_bytes(body[4..8].try_into().unwrap());
            assert_eq!(u16::from_le_bytes([body[14], body[15]]), 16);
        } else if id == b"data" {
            samples = (body.chunks_exact(2 * channels))
                .map(|f| f64::from(i16::from_le_bytes([f[0], f[1]])) / 32768.0)
                .collect();
        }
        at += 8 + len + (len & 1);
    }
    (samples, f64::from(rate))
}

/// The pitch at each period of a near-sine: (time, Hz) from one rising zero crossing to the
/// next (each found between samples by a straight line).
fn periods(samples: &[f64], rate: f64) -> Vec<(f64, f64)> {
    let peak = samples.iter().fold(0.0f64, |m, s| m.max(s.abs()));
    let mut crossings = Vec::new();
    for i in 1..samples.len() {
        let (a, b) = (samples[i - 1], samples[i]);
        if a < 0.0 && b >= 0.0 && peak > 0.0 {
            crossings.push((i as f64 - 1.0 + a / (a - b)) / rate);
        }
    }
    (crossings.windows(2))
        .map(|w| ((w[0] + w[1]) / 2.0, 1.0 / (w[1] - w[0])))
        .collect()
}

/// Over `from..to` seconds: the middle pitch, how far the pitch goes from it each way in
/// semitones (half of the swing between the 5th and 95th percentile), and the rate of the
/// wobble in Hz (rising crossings of the middle).
fn wobble(track: &[(f64, f64)], from: f64, to: f64) -> (f64, f64, f64) {
    let part: Vec<(f64, f64)> = (track.iter().copied())
        .filter(|(t, _)| (from..to).contains(t))
        .collect();
    let mut sorted: Vec<f64> = part.iter().map(|(_, f)| *f).collect();
    sorted.sort_by(f64::total_cmp);
    let pick = |q: f64| sorted[((sorted.len() - 1) as f64 * q) as usize];
    let middle = pick(0.5);
    let semis = |f: f64| 12.0 * (f / middle).log2();
    let swing = (semis(pick(0.95)) - semis(pick(0.05))) / 2.0;
    let mut ups = Vec::new();
    for w in part.windows(2) {
        if w[0].1 < middle && w[1].1 >= middle {
            ups.push(w[1].0);
        }
    }
    let rate = match ups.len() {
        0 | 1 => 0.0,
        n => (n - 1) as f64 / (ups[n - 1] - ups[0]),
    };
    (middle, swing, rate)
}

/// Renders key 69 for 3 s through the track's instrument and measures its pitch: before the
/// LFO's delay and after it.
fn measure(c: &mut Client, track: u32, what: &str) -> ((f64, f64, f64), (f64, f64, f64)) {
    let rendered = ok(
        c,
        "render_note",
        json!({ "track": track, "key": 69, "velocity": 100, "seconds": 3.0 }),
    );
    let file = PathBuf::from(rendered["files"][0].as_str().unwrap());
    let (samples, rate) = wav(&file);
    let track_hz = periods(&samples, rate);
    let early = wobble(&track_hz, 0.05, 0.45);
    let late = wobble(&track_hz, 0.9, 2.9);
    eprintln!(
        "{what}: {} Hz; 0.05-0.45 s: {:.2} Hz, ±{:.3} semitone, wobble {:.2} Hz; 0.9-2.9 s: \
         {:.2} Hz, ±{:.3} semitone, wobble {:.2} Hz",
        rate, early.0, early.1, early.2, late.0, late.1, late.2
    );
    (early, late)
}

/// The patch "Init Sine" (a plain sine) put into the plugin at `track` through the state.
fn init_sine(c: &mut Client, plugin: &PluginRef, at: &Value) -> Value {
    let now = state_when_loaded(c, at.clone());
    let state = std::fs::read(now["file"].as_str().unwrap()).unwrap();
    let id = "surge:factory/Templates/Init Sine.fxp";
    let put = presets::load(plugin, id, &state).unwrap();
    let file = scratch("surge-init-sine").join(format!("{}.bin", now["plugin"]["format"]));
    std::fs::write(&file, put).unwrap();
    let mut args = at.clone();
    args["file"] = json!(file.display().to_string());
    ok(c, "set_plugin_state", args)
}

fn surge_ref(key: &str) -> PluginRef {
    PluginRef::new(key, "Surge XT", "Surge Synth Team", "")
}

/// The owner's completion condition (requirement 2): LFO 1 moves scene A's pitch with a
/// depth and a delay, and the vibrato is heard; undo, redo, saved and opened again. Then
/// requirement 1 at this level: a factory patch loaded, one saved and loaded back. Then
/// what an effect's type does to its parameters.
#[test]
#[ignore = "needs Surge XT installed"]
fn real_vibrato_presets_and_effect_types() {
    let (r, mut c, vst3, lv2) = surge_core("surge-real");
    ok(&mut c, "add_track", json!({ "plugin": vst3 }));
    ok(&mut c, "add_track", json!({ "plugin": lv2 }));
    let places = [(2, vst3.clone()), (3, lv2.clone())];
    let mut kept = Vec::new();
    for (track, key) in &places {
        let at = json!({ "track": track });
        let name = if key.starts_with("vst3") {
            "VST3"
        } else {
            "LV2"
        };
        let plugin = surge_ref(key);
        // A factory patch through the state: one step; the plugin then names that patch.
        let before = steps(&r);
        let put = init_sine(&mut c, &plugin, &at);
        assert_eq!(steps(&r), before + 1, "{name}: {put}");
        let xml = patch_xml(&mut c, &at);
        assert_eq!(patch_name(&xml), "Init Sine", "{name}");
        eprintln!(
            "{name}: Init Sine: a_lfo0_rate {:?}, delay {:?}, shape {:?}, trigmode {:?}",
            patch_value(&xml, "a_lfo0_rate"),
            patch_value(&xml, "a_lfo0_delay"),
            patch_value(&xml, "a_lfo0_shape"),
            patch_value(&xml, "a_lfo0_trigmode")
        );
        let (early, late) = measure(&mut c, *track, &format!("{name} without the routing"));
        assert!(early.1 < 0.03 && late.1 < 0.03, "{name}: steady");

        // The vibrato: one step.
        let before = steps(&r);
        let set = ok(
            &mut c,
            "surge_set_modulation",
            json!({
                "track": track, "target": "a_pitch", "source": "LFO 1", "depth": 0.5,
                "params": [
                    { "name": "a_lfo0_rate", "hz": 5.0 },
                    { "name": "a_lfo0_delay", "seconds": 0.5, "deactivated": false },
                    { "name": "a_lfo0_shape", "value": 0 },
                    { "name": "a_lfo0_trigmode", "value": 1 },
                ],
            }),
        );
        eprintln!("{name}: surge_set_modulation: {set}");
        assert_eq!(steps(&r), before + 1, "{name}");
        assert_eq!(set["changed"], true);
        let now = &set["routings_now"][0];
        assert_eq!(now["source"]["id"], 17, "{set}");
        assert_eq!(now["depth"], 0.5, "{set}");
        assert_eq!(now["target_name"], "A Pitch", "{set}");
        let params = set["params_now"].as_array().unwrap();
        let rate = (params.iter())
            .find(|p| p["name"] == "a_lfo0_rate")
            .unwrap();
        assert!(
            (rate["natural"]["value"].as_f64().unwrap() - 5.0).abs() < 1e-3,
            "{rate}"
        );
        let (early, late) = measure(&mut c, *track, &format!("{name} with the routing"));
        assert!(early.1 < 0.05, "{name}: steady before the delay: {early:?}");
        assert!(
            (late.1 - 0.5).abs() < 0.1,
            "{name}: ±0.5 semitone after it: {late:?}"
        );
        assert!((late.2 - 5.0).abs() < 0.5, "{name}: 5 Hz: {late:?}");
        assert!((early.0 - 440.0).abs() < 2.0 && (late.0 - 440.0).abs() < 2.0);

        // The same again: no step.
        let same = ok(
            &mut c,
            "surge_set_modulation",
            json!({ "track": track, "target": "a_pitch", "source": 17, "depth": 0.5 }),
        );
        assert_eq!(same["changed"], false, "{same}");

        // Undo takes the routing away, redo brings it back.
        ok(&mut c, "undo", json!({}));
        patiently("undo", || {
            routings(&mut c, &at, "a_pitch").is_empty().then_some(())
        });
        r.core.send(Command::Redo);
        patiently("redo", || {
            let now = routings(&mut c, &at, "a_pitch");
            (now.len() == 1 && now[0]["depth"] == 0.5).then_some(())
        });
        eprintln!("{name}: undo and redo take the routing away and back");

        // A depth changed, then the routing taken away and put back by undo.
        let deeper = ok(
            &mut c,
            "surge_set_modulation",
            json!({ "track": track, "target": "a_pitch", "source": "vlfo_1", "depth": 1.0 }),
        );
        assert_eq!(deeper["routings_now"][0]["depth"], 1.0, "{deeper}");
        let gone = ok(
            &mut c,
            "surge_remove_modulation",
            json!({ "track": track, "target": "a_pitch", "source": "LFO 1" }),
        );
        assert_eq!(gone["routings_now"], json!([]), "{gone}");
        ok(&mut c, "undo", json!({}));
        ok(&mut c, "undo", json!({}));
        patiently("undo twice", || {
            let now = routings(&mut c, &at, "a_pitch");
            (now.len() == 1 && now[0]["depth"] == 0.5).then_some(())
        });

        // Refused before the plugin is asked; nothing changes.
        let before = steps(&r);
        for (args, says) in [
            (
                json!({ "track": track, "target": "a_osc1_type", "source": 17, "depth": 1 }),
                "not a float",
            ),
            (
                json!({ "track": track, "target": "volume", "source": 17, "depth": 1 }),
                "whole scene",
            ),
            (
                json!({ "track": track, "target": "a_ptch", "source": 17, "depth": 1 }),
                "a_pitch",
            ),
        ] {
            let text = refused(&mut c, "surge_set_modulation", args.clone());
            assert!(text.contains(says), "{args}: {text}");
        }
        assert_eq!(steps(&r), before);

        // The patch parameters, also in the plugin's own list.
        let lfo = ok(
            &mut c,
            "surge_get_patch_params",
            json!({ "track": track, "name": "a_lfo0_" }),
        );
        eprintln!("{name}: a_lfo0_*: {lfo}");
        let delay = (lfo["params"].as_array().unwrap().iter())
            .find(|p| p["name"] == "a_lfo0_delay")
            .unwrap()
            .clone();
        assert_eq!(delay["param_name"], "A LFO 1 Delay", "{delay}");
        assert!((delay["natural"]["value"].as_f64().unwrap() - 0.5).abs() < 1e-3);

        // The patch as text, edited and put back: one step.
        let xml = patch_xml(&mut c, &at);
        let edited = xml.replacen("depth=\"0.5\"", "depth=\"0.25\"", 1);
        assert_ne!(edited, xml);
        let before = steps(&r);
        let put = ok(
            &mut c,
            "set_plugin_state_text",
            json!({ "track": track, "text": edited }),
        );
        eprintln!("{name}: set_plugin_state_text: {put}");
        assert_eq!(steps(&r), before + 1);
        assert_eq!(routings(&mut c, &at, "a_pitch")[0]["depth"], 0.25);
        let broken = refused(
            &mut c,
            "set_plugin_state_text",
            json!({ "track": track, "text": xml.replace("</patch>", "") }),
        );
        assert!(broken.contains("not well-formed"), "{broken}");
        assert_eq!(steps(&r), before + 1);
        ok(&mut c, "undo", json!({}));
        kept.push((at.clone(), name));
    }

    // Saved and opened again: the routings are there.
    let folder = scratch("surge-real-songs").display().to_string();
    ok(
        &mut c,
        "save_song_as",
        json!({ "folder": folder, "name": "vibrato" }),
    );
    let file = Path::new(&folder).join("vibrato/vibrato.meloet");
    ok(&mut c, "new_song", json!({}));
    ok(
        &mut c,
        "open_song",
        json!({ "path": file.display().to_string() }),
    );
    for (at, name) in &kept {
        let now = patiently("the routing after opening", || {
            let answer = c.call(
                "surge_list_modulations",
                json!({ "track": at["track"], "target": "a_pitch" }),
            );
            let now = answer["structuredContent"]["routings"].clone();
            (answer["isError"] == false && now.as_array().is_some_and(|r| r.len() == 1))
                .then_some(now)
        });
        assert_eq!(now[0]["depth"], 0.5, "{name}");
        eprintln!("{name}: opened again: {now}");
        let (_, late) = measure(&mut c, at["track"].as_u64().unwrap() as u32, name);
        assert!((late.1 - 0.5).abs() < 0.1, "{name}: {late:?}");
    }

    // Requirement 1 at this level: the factory patches, one saved as the person's and
    // loaded back.
    let roots = Roots {
        user: scratch("surge-real-user").join("Patches"),
        ..Roots::from_env()
    };
    let plugin = surge_ref(&vst3);
    let started = Instant::now();
    let list = presets::list_in(&roots, &plugin).unwrap();
    eprintln!("{} patches listed in {:?}", list.len(), started.elapsed());
    assert!(list.len() > 3000);
    let at = json!({ "track": 2 });
    let now = state_when_loaded(&mut c, at.clone());
    let state = std::fs::read(now["file"].as_str().unwrap()).unwrap();
    let saved =
        presets::save_in(&roots, &plugin, &state, "Vibrato Sine", Some("Mine"), false).unwrap();
    assert_eq!(saved.id, "surge:user/Mine/Vibrato Sine.fxp");
    let mine: Vec<_> = (presets::list_in(&roots, &plugin).unwrap().into_iter())
        .filter(|p| !p.factory)
        .collect();
    assert_eq!(mine, std::slice::from_ref(&saved));
    let lead = "surge:factory/Leads/Sine Lead.fxp";
    let put = presets::load_in(&roots, &plugin, lead, &state).unwrap();
    let file = scratch("surge-real-lead").join("lead.bin");
    std::fs::write(&file, put).unwrap();
    let before = steps(&r);
    ok(
        &mut c,
        "set_plugin_state",
        json!({ "track": 2, "file": file.display().to_string() }),
    );
    assert_eq!(steps(&r), before + 1);
    assert_eq!(patch_name(&patch_xml(&mut c, &at)), "Sinlead");
    let now = state_when_loaded(&mut c, at.clone());
    let state = std::fs::read(now["file"].as_str().unwrap()).unwrap();
    let back = presets::load_in(&roots, &plugin, &saved.id, &state).unwrap();
    std::fs::write(&file, back).unwrap();
    ok(
        &mut c,
        "set_plugin_state",
        json!({ "track": 2, "file": file.display().to_string() }),
    );
    let xml = patch_xml(&mut c, &at);
    assert_eq!(patch_name(&xml), "Vibrato Sine");
    assert_eq!(routings(&mut c, &at, "a_pitch")[0]["depth"], 0.5);
    eprintln!("a patch saved as the person's comes back with its routing");
}

/// A core with only Surge XT's VST3 and LV2, and the sound running through a real output:
/// `MELOET_SURGE_OUTPUT` (an id of `state.outputs.list`), else the first one meloet can use.
fn live_core(name: &str) -> (Running, Client, String, String) {
    let pick = || {
        let probe = Core::start(CoreOptions {
            devices: true,
            ..CoreOptions::offline()
        });
        std::thread::sleep(Duration::from_secs(2));
        let list = probe.snapshot().state.outputs.list;
        for o in &list {
            eprintln!("output {:?} {:?} unusable={:?}", o.id, o.name, o.unusable);
        }
        let found = list
            .into_iter()
            .find(|o| o.unusable.is_none())
            .map(|o| o.id);
        probe.stop();
        found.expect("an output meloet can use")
    };
    let output = std::env::var("MELOET_SURGE_OUTPUT").unwrap_or_else(|_| pick());
    let runtime = scratch(name);
    let config = runtime.join("config");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("settings.json"),
        format!(
            r#"{{ "version": 1, "output": "{output}", "output_name": "{output}", "scale": 100 }}"#
        ),
    )
    .unwrap();
    let vst3 = scratch(&format!("{name}-vst3"));
    let lv2 = scratch(&format!("{name}-lv2"));
    std::os::unix::fs::symlink("/usr/lib/vst3/Surge XT.vst3", vst3.join("Surge XT.vst3")).unwrap();
    std::os::unix::fs::symlink("/usr/lib/lv2/Surge XT.lv2", lv2.join("Surge XT.lv2")).unwrap();
    let options = options(&runtime);
    let socket = options.socket_path().unwrap();
    let core = Arc::new(Core::start(CoreOptions {
        config_dir: Some(config),
        devices: true,
        audio: true,
        songs_dir: Some(runtime.join("songs")),
        plugins: Some(PluginOptions {
            host_exe: plughost::exe(),
            cache_dir: Some(runtime.join("cache")),
            usual_places: vec![vst3, lv2],
            scan_timeout: Duration::from_secs(60),
            child_env: Vec::new(),
            wait_for_screen: false,
        }),
        ..offline(&runtime)
    }));
    waiting("the sound to run", Duration::from_secs(15), || {
        let audio = core.snapshot().state.audio;
        (audio.running && audio.actual_frames.is_some()).then_some(())
    });
    let audio = core.snapshot().state.audio;
    eprintln!(
        "output {output}: {:?} frames, {:?} Hz",
        audio.actual_frames, audio.sample_rate
    );
    let link = AiLink::start(Arc::new(CoreLink(Arc::downgrade(&core))), options).unwrap();
    core.set_ai_switch(Box::new(move |on| link.switch(on)));
    core.send(Command::SetAiLink { on: true });
    wait_for("the socket", || socket.exists());
    let r = Running { core, socket };
    let mut c = Client::start(&r.socket);
    let vst3 = patiently("Surge XT (VST3)", || {
        find_plugin(&mut c, "Surge XT", "vst3")
    });
    let lv2 = patiently("Surge XT (LV2)", || find_plugin(&mut c, "Surge XT", "lv2"));
    (r, c, vst3, lv2)
}

/// While the sound runs, Surge XT takes a state on its audio thread: the routing and the
/// effect types as they are then.
#[test]
#[ignore = "needs Surge XT and a real output (silent: no key is played)"]
fn real_with_the_sound_running() {
    let (r, mut c, vst3, lv2) = live_core("surge-live");
    ok(&mut c, "add_track", json!({ "plugin": vst3 }));
    ok(&mut c, "add_track", json!({ "plugin": lv2 }));
    for (track, key) in [(2, &vst3), (3, &lv2)] {
        let at = json!({ "track": track });
        let plugin = surge_ref(key);
        // Patches one after another: each is one step, and the plugin has it.
        for (round, (id, name)) in [
            ("Templates/Init Sine", "Init Sine"),
            ("Templates/Init Square", "Init Square"),
            ("Leads/Sine Lead", "Sinlead"),
            ("Templates/Init Sine", "Init Sine"),
        ]
        .into_iter()
        .enumerate()
        {
            let now = state_when_loaded(&mut c, at.clone());
            let state = std::fs::read(now["file"].as_str().unwrap()).unwrap();
            let put = presets::load(&plugin, &format!("surge:factory/{id}.fxp"), &state).unwrap();
            let file = scratch("surge-live-put").join(format!("{round}.bin"));
            std::fs::write(&file, put).unwrap();
            let before = steps(&r);
            ok(
                &mut c,
                "set_plugin_state",
                json!({ "track": track, "file": file.display().to_string() }),
            );
            assert_eq!(steps(&r), before + 1, "{key} {id}");
            assert_eq!(patch_name(&patch_xml(&mut c, &at)), name, "{key}");
        }
        for (round, depth) in [0.5, 0.25, 1.0, 0.75, 0.5].into_iter().enumerate() {
            let before = steps(&r);
            let answer = c.call(
                "surge_set_modulation",
                json!({ "track": track, "target": "a_pitch", "source": "LFO 1", "depth": depth }),
            );
            let text = answer["content"][0]["text"]
                .as_str()
                .unwrap_or("")
                .to_owned();
            eprintln!(
                "{key} round {round}: depth {depth}: {}",
                &text[..text.len().min(300)]
            );
            assert_eq!(answer["isError"], false, "{key} round {round}: {text}");
            assert_eq!(steps(&r), before + 1, "{key} round {round}");
            assert_eq!(routings(&mut c, &at, "a_pitch")[0]["depth"], depth);
        }
    }
    effect_types(&mut c, &r, &surge_ref(&vst3), &json!({ "track": 2 }));
}

/// An effect's type set through the plugin's parameter, and through the patch: what the
/// effect's own parameters are after.
fn effect_types(c: &mut Client, r: &Running, plugin: &PluginRef, at: &Value) {
    let fx = |c: &mut Client| {
        let xml = patch_xml(c, at);
        let values: Vec<Option<f64>> = (0..12)
            .map(|i| patch_value(&xml, &format!("fx1_p{i}")))
            .collect();
        (patch_value(&xml, "fx1_type"), values)
    };
    init_sine(c, plugin, at);
    let off = fx(c);
    eprintln!("FX A1 in Init Sine: {off:?}");
    // An empty slot is not in the patch: its type is found in the plugin's list.
    let params = ok(
        c,
        "get_plugin_params",
        json!({ "track": at["track"], "name": "FX A1 FX Type" }),
    );
    let id = params["params"][0]["id"].clone();
    assert!(id.is_number(), "{params}");
    // Through the plugin's parameter: the effect's own parameters become its defaults.
    let mut by_param = Vec::new();
    for text in ["Delay", "Reverb 1"] {
        let set = c.call(
            "set_param",
            json!({ "track": at["track"], "param": id, "text": text }),
        );
        eprintln!(
            "set_param FX A1 FX Type {text}: {}",
            set["content"][0]["text"]
        );
        let now = fx(c);
        eprintln!("FX A1 as {text} through the parameter: {now:?}");
        by_param.push(now);
    }
    // Through the patch: the type of Delay given to the patch of Reverb 1.
    let reverb = patch_xml(c, at);
    let delay_type = by_param[0].0.unwrap();
    let text = reverb.replacen(
        &format!("<fx1_type type=\"0\" value=\"{}\"", by_param[1].0.unwrap()),
        &format!("<fx1_type type=\"0\" value=\"{delay_type}\""),
        1,
    );
    assert_ne!(text, reverb);
    let before = steps(r);
    ok(
        c,
        "set_plugin_state_text",
        json!({ "track": at["track"], "text": text }),
    );
    assert_eq!(steps(r), before + 1);
    let by_patch = fx(c);
    eprintln!("FX A1 as Delay through the patch, from Reverb 1: {by_patch:?}");
    eprintln!(
        "through the patch the values are {} the defaults of Delay",
        if by_patch.1 == by_param[0].1 {
            ""
        } else {
            "NOT"
        }
    );
}

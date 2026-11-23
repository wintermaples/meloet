//! `get_key_names` through the socket: meloet's own kits, an instrument, a plugin of the fake
//! plugin host that names its keys, and FluidR3's drum kit when this PC has the file.

mod common;
use common::*;

#[path = "../../meloet-core/tests/plughost/mod.rs"]
mod plughost;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use meloet_ai::{AiLink, CoreLink};
use meloet_core::Core;
use meloet_core::plugins::PluginOptions;
use meloet_protocol::{AiPort, Command, PluginState, SoundView};
use serde_json::{Value, json};

fn track(c: &mut Client, args: Value) -> u64 {
    ok(c, "add_track", args)["track"].as_u64().expect("a track")
}

fn keys(answer: &Value) -> Vec<(u64, String, Option<String>, String)> {
    (answer["keys"].as_array().unwrap().iter())
        .map(|k| {
            (
                k["key"].as_u64().unwrap(),
                k["note"].as_str().unwrap().to_owned(),
                k["name"].as_str().map(str::to_owned),
                k["source"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

#[test]
fn meloets_own_kits_name_their_drums() {
    let r = running("keynames-builtin");
    let mut c = Client::start(&r.socket);
    let list = c.request("tools/list", json!({}), &mut Vec::new());
    let tool = (list["result"]["tools"].as_array().unwrap().iter())
        .find(|t| t["name"] == "get_key_names")
        .expect("listed")
        .clone();
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    assert_eq!(tool["annotations"]["openWorldHint"], false);

    let electro = track(&mut c, json!({ "sound": "electroKit" }));
    let studio = track(&mut c, json!({ "sound": "studioKit" }));
    let before = steps(&r);

    let answer = ok(&mut c, "get_key_names", json!({ "track": electro }));
    assert_eq!(answer["kit"], true);
    assert_eq!(answer["source"], "builtin");
    assert_eq!(answer["certain"], true);
    assert_eq!(answer["sound"], "electroKit");
    let named = keys(&answer);
    assert_eq!(named.len(), 12);
    let kick = (
        36,
        "C2".to_owned(),
        Some("キック".to_owned()),
        "builtin".to_owned(),
    );
    assert_eq!(named[0], kick);
    assert_eq!(named[2].2.as_deref(), Some("スネア"));
    assert_eq!(named[4].2.as_deref(), Some("クローズハイハット"));
    assert_eq!(answer["keys"][0]["id"], "kick");
    assert_eq!(answer["keys"][0]["sounds"], true);
    assert_eq!(answer["range"]["low_note"], "C2");

    // No sound pack in the tests: the names, and that it plays a sine wave meanwhile.
    let answer = ok(&mut c, "get_key_names", json!({ "track": studio }));
    assert_eq!(answer["kit"], true);
    assert_eq!(keys(&answer).len(), 12);
    let notes = answer["notes"].to_string();
    assert!(notes.contains("sine wave"), "{notes}");

    // The first track's piano: not a kit, no names.
    let answer = ok(&mut c, "get_key_names", json!({ "track": 1 }));
    assert_eq!(answer["kit"], false);
    assert_eq!(answer["source"], "none");
    assert!(keys(&answer).is_empty());

    let text = refused(&mut c, "get_key_names", json!({ "track": 9 }));
    assert!(text.contains("no track 9"), "{text}");
    assert_eq!(steps(&r), before, "reading is no step");
}

/// A core whose AI link is on, with the plugins of `places` and the SF2 files of `sf2`.
fn plugged(runtime: &std::path::Path, places: Vec<PathBuf>, fake: bool) -> (Arc<Core>, Client) {
    let core = Arc::new(Core::start(meloet_core::CoreOptions {
        config_dir: Some(runtime.join("config")),
        sf2_dir: Some(runtime.join("sf2")),
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
    let options = options(runtime);
    let socket = options.socket_path().unwrap();
    let link = AiLink::start(Arc::new(CoreLink(Arc::downgrade(&core))), options).unwrap();
    core.set_ai_switch(Box::new(move |on| link.switch(on)));
    core.send(Command::SetAiLink { on: true });
    wait_for("Waiting", || port_of(&core) == AiPort::Waiting);
    let c = Client::start(&socket);
    (core, c)
}

fn wait_ready(core: &Core, track: u64) {
    let until = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        let state = core.snapshot().state;
        let sound = &state.song.tracks[track as usize - 1].sound;
        if matches!(sound, SoundView::Plugin(p) if p.state == PluginState::Ready) {
            return;
        }
        assert!(std::time::Instant::now() < until, "the plugin is not ready");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The real Black Pearl (avldrums.lv2) through the whole of meloet: scanned, put on a track,
/// asked over its midnam, answered through the socket.
#[test]
#[ignore = "needs avldrums.lv2"]
fn black_pearl_names_its_drums_through_the_socket() {
    let bundle = std::path::Path::new("/usr/lib/lv2/avldrums.lv2");
    if !bundle.exists() {
        eprintln!("avldrums.lv2 is not installed; nothing tested");
        return;
    }
    let runtime = scratch("keynames-avl");
    let lv2 = runtime.join("lv2");
    std::fs::create_dir_all(&lv2).unwrap();
    std::os::unix::fs::symlink(bundle, lv2.join("avldrums.lv2")).unwrap();
    let (core, mut c) = plugged(&runtime, vec![lv2], false);
    wait_for("the scan", || {
        let plugins = core.snapshot().state.library.plugins;
        plugins.iter().any(|p| p.name == "Black Pearl Drumkit")
    });
    let key = "lv2:http://gareus.org/oss/lv2/avldrums#BlackPearl";
    let drums = track(&mut c, json!({ "plugin": key }));
    wait_ready(&core, drums);
    let answer = c.call("get_key_names", json!({ "track": drums }));
    println!("{}", answer["content"][0]["text"].as_str().unwrap_or(""));
    let answer = answer["structuredContent"].clone();
    assert_eq!(answer["source"], "lv2_midnam");
    assert_eq!(answer["kit"], true);
    let named = keys(&answer);
    assert_eq!(named.len(), 26);
    assert_eq!(named[0].2.as_deref(), Some("Kick Drum"));
    assert_eq!(named[2].2.as_deref(), Some("Snare Center"));
    assert_eq!(named[6].2.as_deref(), Some("Closed Hat"));
    drop(c);
    drop(core);
    let _ = std::fs::remove_dir_all(&runtime);
}

fn fluid_r3() -> Option<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    [
        format!("{home}/.cache/meloet/sounds-src/fluidr3/FluidR3_GM.sf2"),
        "/usr/share/sounds/sf2/FluidR3_GM.sf2".to_string(),
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.is_file())
}

#[test]
fn a_plugin_and_an_sf2_kit_name_their_keys() {
    let runtime = scratch("keynames-plugin");
    let bundle = runtime.join("lv2/drums.lv2");
    std::fs::create_dir_all(&bundle).unwrap();
    std::fs::write(bundle.join("plugin.so"), [0u8; 16]).unwrap();
    let sf2 = runtime.join("sf2");
    std::fs::create_dir_all(&sf2).unwrap();
    let fluid = fluid_r3();
    if let Some(fluid) = &fluid {
        std::os::unix::fs::symlink(fluid, sf2.join("FluidR3_GM.sf2")).unwrap();
    } else {
        eprintln!("FluidR3_GM.sf2 is not on this PC: the SF2 part is not checked");
    }
    let (core, mut c) = plugged(&runtime, vec![runtime.join("lv2")], true);
    wait_for("the scan", || {
        core.snapshot().state.library.plugins.len() == 2
    });

    let drums = track(&mut c, json!({ "plugin": "lv2:urn:meloet:fake:drums" }));
    wait_ready(&core, drums);
    let answer = ok(&mut c, "get_key_names", json!({ "track": drums }));
    assert_eq!(answer["source"], "lv2_midnam");
    assert_eq!(answer["kit"], true);
    assert_eq!(answer["certain"], true);
    assert_eq!(
        keys(&answer),
        [
            (
                36,
                "C2".to_owned(),
                Some("Fake Kick".to_owned()),
                "lv2_midnam".to_owned()
            ),
            (
                38,
                "D2".to_owned(),
                Some("Fake Snare".to_owned()),
                "lv2_midnam".to_owned()
            ),
        ]
    );

    if fluid.is_some() {
        let sf2 = json!({ "file": "FluidR3_GM.sf2", "bank": 128, "program": 0 });
        let kit = track(&mut c, json!({ "sf2": sf2 }));
        let answer = ok(&mut c, "get_key_names", json!({ "track": kit }));
        assert_eq!(answer["source"], "sf2_sample_name", "{answer}");
        assert_eq!(answer["certain"], true);
        let named = keys(&answer);
        let name = |key: u64| named.iter().find(|k| k.0 == key).and_then(|k| k.2.clone());
        assert_eq!(name(36).as_deref(), Some("Std Kick"));
        assert_eq!(name(38).as_deref(), Some("Std Snr 1"));
        assert_eq!(name(42).as_deref(), Some("Hi-Hat Closed"));
        assert!(!answer.to_string().contains(".cache"), "no path is shown");
        let notes = answer["notes"].to_string();
        assert!(notes.contains("names of the samples"), "{notes}");

        let sf2 = json!({ "file": "FluidR3_GM.sf2", "bank": 0, "program": 0 });
        let piano = track(&mut c, json!({ "sf2": sf2 }));
        let answer = ok(&mut c, "get_key_names", json!({ "track": piano }));
        assert_eq!(answer["kit"], false);
        assert!(keys(&answer).is_empty());
        assert!(answer["range"]["low"].as_u64().is_some(), "{answer}");
    }
    drop(c);
    drop(core);
    let _ = std::fs::remove_dir_all(&runtime);
}

//! The output sets of a plugin instrument through the socket (docs/architecture.md 20-15):
//! `get_mixer`'s `outputs`, `set_plugin_output` and `measure_plugin_outputs`. A meloet of
//! the test's own shows what the tools make of sets a plugin said; a real core holds a song
//! whose plugin this PC does not have; the tests marked `#[ignore]` load plugins that have
//! output sets.

mod common;
use common::*;

#[path = "../../meloet-core/tests/plughost/mod.rs"]
mod plughost;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use meloet_ai::{AiLink, CoreLink, Meloet};
use meloet_core::ai::{
    AiAnswer, AiAsk, AiLevels, AiMeasure, AiOutputLevel, AiOutputLevels, AiOutputsMeasure,
    AiRefusal, AiRender, AiRendered, AiSong, OutputSet, PassLevels,
};
use meloet_core::plugins::PluginOptions;
use meloet_core::{Core, CoreOptions};
use meloet_model::{
    Gain, InstrumentRef, OutputSetting, PluginOutputs, PluginRef, Song, Track, TrackId,
};
use meloet_protocol::{AiPort, Command, PluginState, SoundView};
use serde_json::{Value, json};

/// The fake instrument of the plugin host with four output sets: "a" a stereo pair, "b"
/// mono in the middle, "c" mono on the left, all three on, and "d" mono and off.
const FOUR_OUTPUTS: &str = "lv2:urn:meloet:fake:outs";

const KIT: &str = "lv2:urn:meloet:test:kit";

fn set(id: &str, name: &str, channels: u8, on: bool, pan: f32) -> OutputSet {
    OutputSet {
        id: id.into(),
        name: name.into(),
        channels,
        on,
        pan,
    }
}

fn four() -> Vec<OutputSet> {
    vec![
        set("a", "Main", 2, true, 0.0),
        set("b", "Kick", 1, true, 0.0),
        set("c", "Snare", 1, true, -1.0),
        set("d", "Room", 1, false, 0.0),
    ]
}

/// Track 1 sounds with a plugin that has "b" at -6 dB and keeps "x" off, a set it does not
/// have; track 2 with the grand piano.
fn kit_song() -> Song {
    let mut song = Song::new_default();
    let piano = song.doc.tracks[0].clone();
    let outputs = PluginOutputs::new(
        1,
        [
            OutputSetting {
                gain: Gain::from_db(-6.0),
                ..OutputSetting::new("b")
            },
            OutputSetting {
                on: Some(false),
                ..OutputSetting::new("x")
            },
        ],
    );
    let plugin = PluginRef::new(KIT, "Kit", "Maker", "/nowhere/kit.lv2").with_outputs(outputs);
    song.doc.tracks[0] = Arc::new(Track {
        name: "Drums".into(),
        instrument: InstrumentRef::Plugin(Arc::new(plugin)),
        ..Track::clone(&piano)
    });
    song.doc.tracks.push(Arc::new(Track {
        id: TrackId(2),
        ..Track::clone(&piano)
    }));
    song
}

fn levels(peak: f32) -> PassLevels {
    PassLevels {
        peak,
        rms: peak / 2.0,
        over: 0,
        frames: 96_000,
        lufs: Some(-20.0),
        lufs_short_max: Some(-18.0),
    }
}

/// A meloet that answers from what it is given.
struct Given {
    sets: Option<Vec<OutputSet>>,
    measured: Result<AiOutputLevels, AiRefusal>,
    asked: Mutex<Vec<AiAsk>>,
}

impl Meloet for Given {
    fn song(&self) -> Option<AiSong> {
        Some(AiSong {
            song: kit_song(),
            undo_count: 3,
            busy: None,
            library: Default::default(),
            file: None,
            recent: Vec::new(),
            songs_dir: None,
            unsaved: false,
            sample_rate: 48_000,
            outcomes: Vec::new(),
            instrument_sets: (self.sets.iter())
                .map(|sets| (TrackId(1), sets.clone().into()))
                .collect(),
        })
    }

    fn ask(&self, ask: AiAsk) -> AiAnswer {
        self.asked.lock().unwrap().push(ask);
        AiAnswer::Refused(AiRefusal::Unchanged)
    }

    fn measure(
        &self,
        _: AiMeasure,
        _: &mut dyn FnMut(u8),
        _: &dyn Fn() -> bool,
    ) -> Result<AiLevels, AiRefusal> {
        Err(AiRefusal::Closing)
    }

    fn measure_outputs(
        &self,
        ask: AiOutputsMeasure,
        progress: &mut dyn FnMut(u8),
        _: &dyn Fn() -> bool,
    ) -> Result<AiOutputLevels, AiRefusal> {
        assert_eq!(ask.track, 1);
        progress(100);
        self.measured.clone()
    }

    fn render(
        &self,
        _: AiRender,
        _: &mut dyn FnMut(u8),
        _: &dyn Fn() -> bool,
    ) -> Result<AiRendered, AiRefusal> {
        Err(AiRefusal::Closing)
    }

    fn port(&self, _: AiPort) {}
}

fn given(name: &str, meloet: Given) -> (Arc<Given>, Client, AiLink) {
    let runtime = scratch(name);
    let options = options(&runtime);
    let socket = options.socket_path().unwrap();
    let meloet = Arc::new(meloet);
    let link = AiLink::start(meloet.clone(), options).unwrap();
    link.switch(true);
    wait_for("the socket", || socket.exists());
    (meloet, Client::start(&socket), link)
}

#[test]
fn get_mixer_gives_the_outputs_of_a_loaded_plugin_with_the_values_in_force() {
    let meloet = Given {
        sets: Some(four()),
        measured: Err(AiRefusal::Closing),
        asked: Mutex::new(Vec::new()),
    };
    let (_, mut c, _link) = given("outputs-mixer", meloet);
    let mixer = ok(&mut c, "get_mixer", json!({}));
    let drums = &mixer["tracks"][0];
    assert_eq!(drums["name"], "Drums");
    let outputs = drums["outputs"].as_array().expect("a list");
    assert_eq!(outputs.len(), 4);
    assert_eq!(
        outputs[0],
        json!({ "id": "a", "name": "Main", "channels": 2, "on": true, "volume_db": 0.0,
            "pan": 0, "default": { "on": true, "pan": 0 } })
    );
    assert_eq!(outputs[1]["volume_db"], -6.0);
    assert_eq!(outputs[2]["pan"], -100);
    assert_eq!(outputs[2]["default"]["pan"], -100);
    assert_eq!(outputs[3]["on"], false);
    assert_eq!(
        drums["output_settings_without_output"],
        json!([{ "id": "x", "on": false, "volume_db": 0.0, "pan": null }])
    );
    // A track whose instrument is no plugin has neither.
    let piano = mixer["tracks"][1].as_object().unwrap();
    assert!(
        !piano.contains_key("outputs") && !piano.contains_key("output_settings_without_output")
    );
}

#[test]
fn get_mixer_says_null_while_the_plugin_is_not_loaded() {
    let meloet = Given {
        sets: None,
        measured: Err(AiRefusal::Closing),
        asked: Mutex::new(Vec::new()),
    };
    let (_, mut c, _link) = given("outputs-unloaded", meloet);
    let mixer = ok(&mut c, "get_mixer", json!({}));
    let drums = &mixer["tracks"][0];
    assert_eq!(drums["outputs"], Value::Null);
    let kept = drums["output_settings_without_output"].as_array().unwrap();
    let ids: Vec<&str> = kept.iter().map(|k| k["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["b", "x"], "every setting the song keeps");
    assert_eq!(kept[0]["volume_db"], -6.0);
    assert_eq!(kept[0]["on"], Value::Null);
}

#[test]
fn measure_plugin_outputs_gives_the_levels_of_each_set_and_null_for_one_not_kept() {
    let measured = AiOutputLevels {
        sample_rate: 48_000,
        track: (1, "Drums".into()),
        sets: (four().into_iter().enumerate())
            .map(|(i, set)| AiOutputLevel {
                levels: (i != 2).then(|| levels(0.5)),
                set,
            })
            .collect(),
    };
    let meloet = Given {
        sets: Some(four()),
        measured: Ok(measured),
        asked: Mutex::new(Vec::new()),
    };
    let (_, mut c, _link) = given("outputs-measure", meloet);
    let args = json!({ "track": "id:1", "from_bar": 1, "to_bar": 2 });
    let answer = ok(&mut c, "measure_plugin_outputs", args);
    assert_eq!(
        answer["track"],
        json!({ "number": 1, "id": "id:1", "name": "Drums" })
    );
    assert_eq!(answer["sample_rate"], 48_000);
    assert!(
        answer["measured"]
            .as_str()
            .unwrap()
            .contains("before its own volume, pan and on/off")
    );
    let outputs = answer["outputs"].as_array().unwrap();
    assert_eq!(outputs.len(), 4);
    assert_eq!(outputs[1]["id"], "b");
    assert_eq!(
        outputs[1]["volume_db"], -6.0,
        "the setting, beside the levels"
    );
    assert_eq!(outputs[1]["peak_dbfs"], -6.02);
    assert_eq!(outputs[1]["lufs"], -20.0);
    assert_eq!(outputs[1]["seconds"], 2.0);
    assert_eq!(outputs[2]["peak_dbfs"], Value::Null);
    assert_eq!(outputs[2]["seconds"], Value::Null);
    assert_eq!(outputs[3]["on"], false);
    let text = refused(
        &mut c,
        "measure_plugin_outputs",
        json!({ "track": 2, "from_bar": 1, "to_bar": 2 }),
    );
    assert!(text.contains("no instrument plugin"), "{text}");
}

#[test]
fn a_range_too_long_to_keep_says_how_long_it_may_be() {
    let meloet = Given {
        sets: Some(four()),
        measured: Err(AiRefusal::CaptureTooLong {
            sets: 9,
            longest_s: 77.67,
        }),
        asked: Mutex::new(Vec::new()),
    };
    let (_, mut c, _link) = given("outputs-long", meloet);
    let args = json!({ "track": 1, "from_bar": 1, "to_bar": 200 });
    let text = refused(&mut c, "measure_plugin_outputs", args);
    assert!(
        text.contains("shorter range") && text.contains("77 seconds"),
        "{text}"
    );
    assert!(text.contains("9 outputs"), "{text}");
}

#[test]
fn set_plugin_output_checks_what_it_is_given_before_asking() {
    let meloet = Given {
        sets: Some(four()),
        measured: Err(AiRefusal::Closing),
        asked: Mutex::new(Vec::new()),
    };
    let (meloet, mut c, _link) = given("outputs-args", meloet);
    let text = refused(
        &mut c,
        "set_plugin_output",
        json!({ "track": 1, "output": "b" }),
    );
    assert!(text.contains("at least one"), "{text}");
    let text = refused(
        &mut c,
        "set_plugin_output",
        json!({ "track": 1, "output": "b", "volume_db": 7.0 }),
    );
    assert!(text.contains("-60 to 6"), "{text}");
    let text = refused(
        &mut c,
        "set_plugin_output",
        json!({ "track": 1, "output": "b", "pan": 101 }),
    );
    assert!(text.contains("-100 to 100"), "{text}");
    let text = refused(
        &mut c,
        "set_plugin_output",
        json!({ "track": 2, "output": "b", "on": false }),
    );
    assert!(text.contains("no instrument plugin"), "{text}");
    assert!(meloet.asked.lock().unwrap().is_empty());
    // As it is already: no change, and the values in force.
    let answer = ok(
        &mut c,
        "set_plugin_output",
        json!({ "track": 1, "output": "b", "volume_db": -6.0 }),
    );
    assert_eq!(answer["changed"], false);
    assert_eq!(answer["track"], 1);
    assert_eq!(answer["track_id"], "id:1");
    assert_eq!(answer["track_name"], "Drums");
    assert_eq!(answer["output"]["name"], "Kick");
    assert_eq!(answer["output"]["volume_db"], -6.0);
}

/// A core with no plugins, holding a song whose plugin this PC does not have.
fn missing_kit(name: &str) -> (Running, Client) {
    let r = running(name);
    let mut c = Client::start(&r.socket);
    let path = r.socket.parent().unwrap().join("kit.meloet");
    meloet_core::songfile::save(&path, &kit_song()).unwrap();
    ok(
        &mut c,
        "open_song",
        json!({ "path": path.display().to_string() }),
    );
    (r, c)
}

#[test]
fn outputs_of_a_plugin_that_is_not_loaded_are_set_kept_and_undone() {
    let (r, mut c) = missing_kit("outputs-missing");
    let before = r.core.ai_song().unwrap();
    let mixer = ok(&mut c, "get_mixer", json!({}));
    assert_eq!(mixer["tracks"][0]["outputs"], Value::Null);

    // Not loaded: any id is taken.
    let set = json!({ "track": 1, "output": "out3", "volume_db": -3.0, "pan": -40 });
    let answer = ok(&mut c, "set_plugin_output", set.clone());
    assert_eq!(answer["changed"], true);
    assert!(answer["step"].is_object(), "{answer}");
    assert_eq!(
        answer["output"],
        json!({ "id": "out3", "name": null, "channels": null, "on": null, "volume_db": -3.0,
            "pan": -40, "default": null })
    );
    assert_eq!(steps(&r), before.undo_count + 1);
    let again = ok(&mut c, "set_plugin_output", set);
    assert_eq!(again["changed"], false);
    assert_eq!(steps(&r), before.undo_count + 1);

    let kept = &ok(&mut c, "get_mixer", json!({}))["tracks"][0]["output_settings_without_output"];
    let ids: Vec<&str> = (kept.as_array().unwrap().iter())
        .map(|k| k["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["b", "out3", "x"]);

    // Reset first, then what else is given.
    let answer = ok(
        &mut c,
        "set_plugin_output",
        json!({ "track": 1, "output": "out3", "reset": true, "on": false }),
    );
    assert_eq!(answer["output"]["volume_db"], 0.0);
    assert_eq!(answer["output"]["pan"], Value::Null);
    assert_eq!(answer["output"]["on"], false);

    // The history names the plugin and the output.
    let row = &answer["step"]["name"];
    assert_eq!(row["kind"], "setPluginParam", "{answer}");
    assert_eq!(row["plugin"], "Kit");
    assert_eq!(row["param"], "out3");

    undo_all(&mut c, &r, &before);
    let text = refused(
        &mut c,
        "set_plugin_output",
        json!({ "track": 2, "output": "a", "on": false }),
    );
    assert!(text.contains("no instrument plugin"), "{text}");
}

#[test]
fn sixteen_outputs_in_a_batch_are_one_step() {
    let (r, mut c) = missing_kit("outputs-batch");
    let before = r.core.ai_song().unwrap();
    let operations: Vec<Value> = (1..=16)
        .map(|n| {
            json!({ "tool": "set_plugin_output", "arguments": {
                "track": "id:1", "output": format!("out{n}"), "volume_db": -f64::from(n) } })
        })
        .collect();
    let answer = ok(&mut c, "batch", json!({ "operations": operations }));
    assert_eq!(answer["changed"], true, "{answer}");
    assert_eq!(answer["results"].as_array().unwrap().len(), 16);
    assert_eq!(
        answer["results"][15]["result"]["output"]["volume_db"],
        -16.0
    );
    assert_eq!(steps(&r), before.undo_count + 1, "one step");
    assert_eq!(answer["step"]["name"]["kind"], "batch", "{answer}");
    let doc = r.core.ai_song().unwrap().song.doc;
    let InstrumentRef::Plugin(plugin) = &doc.tracks[0].instrument else {
        panic!("a plugin");
    };
    // "b" and "x" of the song, and the sixteen.
    assert_eq!(plugin.outputs.sets.len(), 18);
    ok(&mut c, "undo", json!({}));
    let back = r.core.ai_song().unwrap();
    assert_eq!(back.song.doc.tracks, before.song.doc.tracks);
}

#[test]
fn automation_cannot_name_an_output() {
    let (_r, mut c) = missing_kit("outputs-automation");
    for (tool, args) in [
        ("add_lane", json!({ "track": 1, "target": "output" })),
        (
            "add_lane",
            json!({ "track": 1, "target": "output_set", "param": 0 }),
        ),
        (
            "write_automation",
            json!({ "track": 1, "target": "output", "points": [{ "tick": 0, "value": 0.5 }] }),
        ),
    ] {
        let answer = c.request(
            "tools/call",
            json!({ "name": tool, "arguments": args }),
            &mut Vec::new(),
        );
        let refused = answer["error"].is_object() || answer["result"]["isError"] == true;
        assert!(refused, "{tool} {args}: {answer}");
    }
}

/// A core with the plugins of `places`, its AI link on.
fn plugged(name: &str, places: Vec<PathBuf>, fake: bool) -> (Arc<Core>, Client, PathBuf) {
    let runtime = scratch(name);
    let core = Arc::new(Core::start(CoreOptions {
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
        ..offline(&runtime)
    }));
    let options = options(&runtime);
    let socket = options.socket_path().unwrap();
    let link = AiLink::start(Arc::new(CoreLink(Arc::downgrade(&core))), options).unwrap();
    core.set_ai_switch(Box::new(move |on| link.switch(on)));
    core.send(Command::SetAiLink { on: true });
    wait_for("Waiting", || port_of(&core) == AiPort::Waiting);
    let c = Client::start(&socket);
    (core, c, runtime)
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

/// Waits until get_mixer lists the outputs of the track's plugin.
fn outputs_listed(c: &mut Client, track: u64) -> Vec<Value> {
    let until = std::time::Instant::now() + Duration::from_secs(60);
    loop {
        let mixer = ok(c, "get_mixer", json!({}));
        if let Some(list) = mixer["tracks"][track as usize - 1]["outputs"].as_array() {
            return list.clone();
        }
        assert!(std::time::Instant::now() < until, "no outputs: {mixer}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Mean square over both channels of a level of measure_mix or measure_plugin_outputs.
fn power(level: &Value) -> f64 {
    level["rms_dbfs"]
        .as_f64()
        .map_or(0.0, |db| 10f64.powf(db / 10.0))
}

fn track_power(c: &mut Client, track: u64, to_bar: u32) -> f64 {
    let args = json!({ "from_bar": 1, "to_bar": to_bar, "tracks": [track] });
    power(&ok(c, "measure_mix", args)["tracks"][0])
}

fn set_powers(c: &mut Client, track: u64, to_bar: u32) -> Vec<(String, f64)> {
    let args = json!({ "track": track, "from_bar": 1, "to_bar": to_bar });
    let answer = ok(c, "measure_plugin_outputs", args);
    (answer["outputs"].as_array().unwrap().iter())
        .map(|o| (o["id"].as_str().unwrap().to_owned(), power(o)))
        .collect()
}

/// Notes on `track` for its first `bars` bars: keys 36 to 51 in turn, a sixteenth each.
fn notes(c: &mut Client, track: u64, bars: u64) {
    let notes: Vec<Value> = (0..bars * 16)
        .map(|i| json!([i * 480, 400, 36 + i % 16, 110]))
        .collect();
    ok(c, "write_notes", json!({ "track": track, "notes": notes }));
}

/// What the end-to-end tests check of a plugin whose outputs have sound: each is measured,
/// one switched off takes out of the track what it gave and leaves the others as they were,
/// undo brings it back, and a saved song opens with the setting.
fn switching_one_off(c: &mut Client, core: &Core, track: u64, runtime: &Path, in_phase: bool) {
    let to_bar = 2;
    let sets = set_powers(c, track, to_bar);
    println!("each output alone: {sets:?}");
    let heard: Vec<&(String, f64)> = sets.iter().filter(|(_, p)| *p > 0.0).collect();
    assert!(heard.len() >= 2, "{sets:?}");
    let mixer = ok(c, "get_mixer", json!({}));
    let on = |id: &str| {
        (mixer["tracks"][track as usize - 1]["outputs"]
            .as_array()
            .unwrap()
            .iter())
        .find(|o| o["id"] == id)
        .is_some_and(|o| o["on"] == true)
    };
    let (off_id, off_power) = (heard.iter())
        .find(|(id, _)| on(id))
        .map(|(id, p)| (id.clone(), *p))
        .expect("a set that is on and sounds");

    let whole = track_power(c, track, to_bar);
    let steps_before = core.ai_song().unwrap().undo_count;
    let args = json!({ "track": track, "output": off_id, "on": false });
    let answer = ok(c, "set_plugin_output", args);
    assert_eq!(answer["output"]["on"], false);
    let without = track_power(c, track, to_bar);
    println!("the track: {whole} with {off_id}, {without} without it ({off_power} alone)");
    // The powers of sets add up where their sounds are not alike: loosely, what the set
    // gave is gone. Sets in phase (the fake's constants) add by amplitude instead.
    assert!(without < whole, "{without} < {whole}");
    assert!(
        in_phase || (whole - without - off_power).abs() < 0.5 * off_power.max(whole - without),
        "{whole} - {without} is near {off_power}"
    );
    // The other sets are measured as they were: before their own on/off.
    let after = set_powers(c, track, to_bar);
    for ((id, was), (_, now)) in sets.iter().zip(&after) {
        assert!(
            (was - now).abs() <= 1e-3 * was.max(1e-12),
            "{id}: {was} then {now}"
        );
    }

    ok(c, "undo", json!({}));
    assert_eq!(core.ai_song().unwrap().undo_count, steps_before);
    let back = track_power(c, track, to_bar);
    assert!(
        (back - whole).abs() <= 0.01 * whole,
        "{back} is {whole} again"
    );

    // Saved with the set off, the song opens with it off and sounds that way.
    ok(
        c,
        "set_plugin_output",
        json!({ "track": track, "output": off_id, "on": false }),
    );
    let folder = runtime.join("songs");
    std::fs::create_dir_all(&folder).unwrap();
    let saved = ok(
        c,
        "save_song_as",
        json!({ "folder": folder.display().to_string(), "name": "outputs" }),
    );
    let file = saved["file"].as_str().unwrap().to_owned();
    ok(c, "new_song", json!({}));
    ok(c, "open_song", json!({ "path": file }));
    wait_ready(core, track);
    let listed = outputs_listed(c, track);
    let off = listed.iter().find(|o| o["id"] == off_id.as_str()).unwrap();
    assert_eq!(off["on"], false, "{listed:?}");
    let opened = track_power(c, track, to_bar);
    assert!(
        (opened - without).abs() <= 0.01 * without.max(1e-12),
        "{opened} is {without}"
    );
}

#[test]
#[ignore = "needs the plugin host's fake instrument with four output sets (the other half of \
            the output sets work); FOUR_OUTPUTS names it"]
fn the_fake_instrument_with_four_outputs_through_the_socket() {
    let lv2 = scratch("outputs-fake-lv2");
    let bundle = lv2.join("outs.lv2");
    std::fs::create_dir_all(&bundle).unwrap();
    std::fs::write(bundle.join("plugin.so"), [0u8; 16]).unwrap();
    let (core, mut c, runtime) = plugged("outputs-fake", vec![lv2], true);
    wait_for("the scan", || {
        let plugins = core.snapshot().state.library.plugins;
        plugins.iter().any(|p| p.key == FOUR_OUTPUTS)
    });
    let track = ok(&mut c, "add_track", json!({ "plugin": FOUR_OUTPUTS }))["track"]
        .as_u64()
        .unwrap();
    wait_ready(&core, track);
    let listed = outputs_listed(&mut c, track);
    let shown: Vec<(&str, u64, bool, i64)> = (listed.iter())
        .map(|o| {
            let id = o["id"].as_str().unwrap();
            let channels = o["channels"].as_u64().unwrap();
            (id, channels, o["on"] == true, o["pan"].as_i64().unwrap())
        })
        .collect();
    assert_eq!(
        shown,
        [
            ("a", 2, true, 0),
            ("b", 1, true, 0),
            ("c", 1, true, -100),
            ("d", 1, false, 0)
        ]
    );
    let text = refused(
        &mut c,
        "set_plugin_output",
        json!({ "track": track, "output": "e", "on": true }),
    );
    assert!(text.contains("its outputs are a, b, c, d"), "{text}");
    notes(&mut c, track, 2);

    // "d" is off: measured all the same.
    let sets = set_powers(&mut c, track, 2);
    assert_eq!(sets.len(), 4);
    assert!(sets.iter().all(|(_, p)| *p > 0.0), "{sets:?}");
    switching_one_off(&mut c, &core, track, &runtime, true);
    drop(c);
    drop(core);
    let _ = std::fs::remove_dir_all(&runtime);
}

/// avldrums' Black Pearl with an output for each drum: 5 mono sets and 2 stereo pairs.
#[test]
#[ignore = "needs avldrums.lv2 and the plugin host's output sets (the other half of the \
            output sets work)"]
fn black_pearl_multi_measures_and_switches_its_outputs() {
    let bundle = Path::new("/usr/lib/lv2/avldrums.lv2");
    if !bundle.exists() {
        eprintln!("avldrums.lv2 is not installed; nothing tested");
        return;
    }
    let lv2 = scratch("outputs-avl-lv2");
    std::os::unix::fs::symlink(bundle, lv2.join("avldrums.lv2")).unwrap();
    let (core, mut c, runtime) = plugged("outputs-avl", vec![lv2], false);
    let key = "lv2:http://gareus.org/oss/lv2/avldrums#BlackPearlMulti";
    wait_for("the scan", || {
        let plugins = core.snapshot().state.library.plugins;
        plugins.iter().any(|p| p.key == key)
    });
    let track = ok(&mut c, "add_track", json!({ "plugin": key }))["track"]
        .as_u64()
        .unwrap();
    wait_ready(&core, track);
    let listed = outputs_listed(&mut c, track);
    println!("{listed:#?}");
    let channels: Vec<u64> = listed
        .iter()
        .map(|o| o["channels"].as_u64().unwrap())
        .collect();
    assert_eq!(
        channels.iter().filter(|c| **c == 1).count(),
        5,
        "{channels:?}"
    );
    assert_eq!(
        channels.iter().filter(|c| **c == 2).count(),
        2,
        "{channels:?}"
    );
    notes(&mut c, track, 2);
    switching_one_off(&mut c, &core, track, &runtime, false);
    drop(c);
    drop(core);
    let _ = std::fs::remove_dir_all(&runtime);
}

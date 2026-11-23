//! The owner's completion conditions for Surge XT, through the MCP tools, with the sound
//! running through a real output (silent: no key is played), on Surge XT's VST3 and LV2 of this
//! PC. Ignored: they need Surge XT and an output.
//!
//! ```text
//! PIPEWIRE_RUNTIME_DIR=/run/user/1000 PIPEWIRE_REMOTE=/run/user/1000/pipewire-0 \
//! MELOET_SURGE_OUTPUT=<an id of state.outputs.list> \
//! cargo test -p meloet-ai --test conditions -- --ignored --nocapture --test-threads 1
//! ```
//!
//! Requirement 3 (notes in and out) is `tests/notes_io.rs`, which is not ignored.

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
use meloet_protocol::Command;
use serde_json::{Value, json};

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

/// A core with only Surge XT's VST3 and LV2, the sound running through a real output
/// (`MELOET_SURGE_OUTPUT`, else the first one meloet can use), the person's Surge XT patches
/// in a folder of the test, and the AI link; with Surge XT's keys (VST3, LV2).
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
    presets::set_roots(Some(Roots {
        user: runtime.join("surge-user").join("Patches"),
        ..Roots::from_env()
    }));
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

fn with(at: &Value, extra: Value) -> Value {
    let mut args = at.clone();
    args.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    args
}

/// The plugin's state once it is loaded: get_plugin_state's answer, with the patch XML.
fn state_when_loaded(c: &mut Client, at: &Value) -> Value {
    patiently("the plugin to be loaded", || {
        let answer = c.call(
            "get_plugin_state",
            with(at, json!({ "readable_inline": true })),
        );
        (answer["isError"] == false).then(|| answer["structuredContent"].clone())
    })
}

/// The `<meta name="…">` of the patch the plugin at `at` has now, and its state's sha256.
fn patch_now(c: &mut Client, at: &Value) -> (String, String) {
    let got = state_when_loaded(c, at);
    assert_eq!(got["readable"]["kind"], "xml", "{got}");
    let xml = got["readable"]["text"].as_str().unwrap();
    let from = xml.find("<meta name=\"").unwrap() + 12;
    let name = xml[from..from + xml[from..].find('"').unwrap()].to_owned();
    (name, got["sha256"].as_str().unwrap().to_owned())
}

/// The state the song has for the instrument of track `track` (from 1).
fn song_state(r: &Running, track: usize) -> Arc<[u8]> {
    let song = r.core.ai_song().unwrap().song;
    match &song.doc.tracks[track - 1].instrument {
        meloet_model::InstrumentRef::Plugin(p) => p.state.clone(),
        other => panic!("{other:?}"),
    }
}

fn names(list: &Value) -> Vec<String> {
    (list["presets"].as_array().unwrap().iter())
        .map(|p| p["name"].as_str().unwrap().to_owned())
        .collect()
}

/// Requirement 1: 「Surge XT で工場出荷パッチを一覧し、一つ読み込んで保存し、開き直しても残って
/// いる。」
#[test]
#[ignore = "needs Surge XT and a real output (silent: no key is played)"]
fn requirement_1_factory_patches_listed_loaded_saved_and_kept() {
    let (r, mut c, vst3, lv2) = live_core("conditions-1");
    let songs = scratch("conditions-1-songs");
    let mut kept = Vec::new();
    for (key, format) in [(&vst3, "VST3"), (&lv2, "LV2")] {
        let added = ok(&mut c, "add_track", json!({ "plugin": key }));
        let track = added["track"].as_u64().unwrap() as usize;
        let at = json!({ "track": track });
        let (first, _) = patch_now(&mut c, &at);
        eprintln!("{format}: track {track}, the patch it starts with: {first:?}");

        // The listing: every factory patch, by name, by category, a page at a time.
        let started = Instant::now();
        let factory = ok(
            &mut c,
            "list_presets",
            with(&at, json!({ "kind": "factory", "limit": 5 })),
        );
        let total = factory["total"].as_u64().unwrap();
        eprintln!(
            "{format}: {total} factory patches, {} categories, listed in {:?}; the first page: \
             {:?}",
            factory["categories_total"],
            started.elapsed(),
            names(&factory)
        );
        assert!(total > 2000, "{factory}");
        assert_eq!(factory["count"], 5);
        assert_eq!(factory["next_offset"], 5);
        let second = ok(
            &mut c,
            "list_presets",
            with(&at, json!({ "kind": "factory", "limit": 5, "offset": 5 })),
        );
        assert_eq!(second["offset"], 5);
        assert_ne!(names(&second), names(&factory));
        let sine = ok(
            &mut c,
            "list_presets",
            with(&at, json!({ "name": "sine", "limit": 500 })),
        );
        eprintln!("{format}: \"sine\" names {} patches", sine["total"]);
        assert!(
            names(&sine)
                .iter()
                .all(|n| n.to_lowercase().contains("sine"))
        );
        assert!(names(&sine).iter().any(|n| n == "Init Sine"), "{sine}");
        let leads = ok(
            &mut c,
            "list_presets",
            with(&at, json!({ "category": "Leads", "name": "sine lead" })),
        );
        eprintln!("{format}: Leads with \"sine lead\": {:?}", names(&leads));
        let lead = (leads["presets"].as_array().unwrap().iter())
            .find(|p| p["id"] == "surge:factory/Leads/Sine Lead.fxp")
            .expect("Leads/Sine Lead")
            .clone();
        assert_eq!(lead["factory"], true);
        assert_eq!(lead["category"], "Leads");

        // A factory patch loaded: one step; the plugin names it; undo and redo.
        let before = steps(&r);
        let (old, _) = patch_now(&mut c, &at);
        let loaded = ok(
            &mut c,
            "load_preset",
            with(&at, json!({ "id": lead["id"] })),
        );
        eprintln!("{format}: load_preset Sine Lead: {loaded}");
        assert_eq!(loaded["changed"], true, "{loaded}");
        assert_eq!(steps(&r), before + 1);
        assert_eq!(loaded["step"]["index"], before + 1, "{loaded}");
        let (now, sha) = patch_now(&mut c, &at);
        assert_eq!(now, "Sinlead", "{format}");
        assert_eq!(&*song_state(&r, track), &*plugin_bytes(&mut c, &at));
        // The same patch again: no change.
        let again = ok(
            &mut c,
            "load_preset",
            with(&at, json!({ "id": lead["id"] })),
        );
        assert_eq!(again["changed"], false, "{again}");
        assert_eq!(steps(&r), before + 1);
        ok(&mut c, "undo", json!({}));
        patiently("the old patch back", || {
            (patch_now(&mut c, &at).0 == old).then_some(())
        });
        r.core.send(Command::Redo);
        patiently("the new patch again", || {
            (patch_now(&mut c, &at).0 == "Sinlead").then_some(())
        });
        assert_eq!(
            patch_now(&mut c, &at).1,
            sha,
            "{format}: redo gives the same state"
        );
        eprintln!("{format}: undo gives {old:?} back, redo Sinlead");

        // Saved as the person's own patch: listed as theirs; once more only with overwrite.
        let name = format!("k11 Cond {format}");
        let saved = ok(
            &mut c,
            "save_preset",
            with(&at, json!({ "name": name, "category": "Mine" })),
        );
        eprintln!("{format}: save_preset: {saved}");
        let id = saved["preset"]["id"].as_str().unwrap().to_owned();
        assert_eq!(id, format!("surge:user/Mine/{name}.fxp"));
        assert_eq!(saved["preset"]["factory"], false);
        let mine = ok(&mut c, "list_presets", with(&at, json!({ "kind": "user" })));
        assert!(names(&mine).contains(&name), "{mine}");
        let refused = refused(
            &mut c,
            "save_preset",
            with(&at, json!({ "name": name, "category": "Mine" })),
        );
        assert!(refused.contains("overwrite"), "{refused}");
        let over = ok(
            &mut c,
            "save_preset",
            with(
                &at,
                json!({ "name": name, "category": "Mine", "overwrite": true }),
            ),
        );
        assert_eq!(over["preset"]["id"], id.as_str(), "{over}");
        kept.push((track, format, sha, id, name));
    }

    // The song saved, a new one, and the song opened again: the patches are there.
    ok(
        &mut c,
        "save_song_as",
        json!({ "folder": songs.display().to_string(), "name": "conditions" }),
    );
    let file = songs.join("conditions/conditions.meloet");
    assert!(file.is_file());
    ok(&mut c, "new_song", json!({}));
    ok(
        &mut c,
        "open_song",
        json!({ "path": file.display().to_string() }),
    );
    for (track, format, sha, id, name) in &kept {
        let at = json!({ "track": track });
        let (now, sha_now) = patch_now(&mut c, &at);
        eprintln!("{format}: opened again: {now:?}, sha256 {sha_now}");
        assert_eq!(now, "Sinlead", "{format}");
        assert_eq!(&sha_now, sha, "{format}: the same state");

        // Another factory patch, then the person's own back.
        ok(
            &mut c,
            "load_preset",
            with(
                &at,
                json!({ "id": "surge:factory/Templates/Init Square.fxp" }),
            ),
        );
        assert_eq!(patch_now(&mut c, &at).0, "Init Square", "{format}");
        let before = steps(&r);
        let back = ok(&mut c, "load_preset", with(&at, json!({ "id": id })));
        assert_eq!(back["changed"], true, "{back}");
        assert_eq!(steps(&r), before + 1);
        assert_eq!(&patch_now(&mut c, &at).0, name, "{format}");
        eprintln!("{format}: the patch saved as the person's loads back as {name:?}");
    }
    presets::set_roots(None);
}

/// The bytes of the plugin's state now.
fn plugin_bytes(c: &mut Client, at: &Value) -> Vec<u8> {
    let got = ok(c, "get_plugin_state", at.clone());
    std::fs::read(got["file"].as_str().unwrap()).unwrap()
}

/// While the sound runs, Surge XT takes a state in only with its next block. Patches put in
/// one after another, each after a parameter was set (so that an LV2 state's head of known
/// values differs from the song's): each is one step, and the song then has what the plugin
/// has. Read too early, the VST3 said its old state (no step) and the LV2 the old patch with
/// the new head (a step that changed nothing): the song and the plugin differed after every
/// put.
#[test]
#[ignore = "needs Surge XT and a real output (silent: no key is played)"]
fn states_put_while_the_sound_runs_are_what_the_song_keeps() {
    let (r, mut c, vst3, lv2) = live_core("conditions-puts");
    let file = scratch("conditions-puts-state").join("state.bin");
    for (key, format) in [(&vst3, "VST3"), (&lv2, "LV2")] {
        let added = ok(&mut c, "add_track", json!({ "plugin": key }));
        let track = added["track"].as_u64().unwrap() as usize;
        let at = json!({ "track": track });
        state_when_loaded(&mut c, &at);
        let plugin = meloet_model::PluginRef::new(key, "Surge XT", "Surge Synth Team", "");
        for round in 0..6 {
            let (id, name) = match round % 2 {
                0 => ("surge:factory/Templates/Init Square.fxp", "Init Square"),
                _ => ("surge:factory/Templates/Init Sine.fxp", "Init Sine"),
            };
            let rate = 0.3 + 0.1 * f64::from(round);
            let set = json!({ "param": "A LFO 1 Rate", "value": rate });
            ok(&mut c, "set_param", with(&at, set));
            let put = presets::load(&plugin, id, &plugin_bytes(&mut c, &at)).unwrap();
            std::fs::write(&file, put).unwrap();
            let before = steps(&r);
            let args = json!({ "file": file.display().to_string() });
            let answer = ok(&mut c, "set_plugin_state", with(&at, args));
            assert_eq!(answer["changed"], true, "{format} round {round}: {answer}");
            assert_eq!(steps(&r), before + 1, "{format} round {round}");
            let song = song_state(&r, track);
            std::thread::sleep(Duration::from_millis(100));
            assert_eq!(
                &*song,
                &*plugin_bytes(&mut c, &at),
                "{format} round {round}"
            );
            assert_eq!(patch_now(&mut c, &at).0, name, "{format} round {round}");
        }
        eprintln!("{format}: 6 patches put in, each one step that the plugin has");
    }
}

// ---- Requirement 2: measuring the pitch of a rendered note ----

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
/// next.
fn periods(samples: &[f64], rate: f64) -> Vec<(f64, f64)> {
    let mut crossings = Vec::new();
    for i in 1..samples.len() {
        let (a, b) = (samples[i - 1], samples[i]);
        if a < 0.0 && b >= 0.0 {
            crossings.push((i as f64 - 1.0 + a / (a - b)) / rate);
        }
    }
    (crossings.windows(2))
        .map(|w| ((w[0] + w[1]) / 2.0, 1.0 / (w[1] - w[0])))
        .collect()
}

/// Over `from..to` seconds: the middle pitch, how far the pitch goes from it each way in
/// semitones (half the swing between the 5th and the 95th percentile), and the wobble's rate.
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
    let ups: Vec<f64> = (part.windows(2))
        .filter(|w| w[0].1 < middle && w[1].1 >= middle)
        .map(|w| w[1].0)
        .collect();
    let rate = match ups.len() {
        0 | 1 => 0.0,
        n => (n - 1) as f64 / (ups[n - 1] - ups[0]),
    };
    (middle, swing, rate)
}

/// Renders key 69 for 3 s through the track's instrument and measures its pitch before the
/// LFO's delay (0.05-0.45 s) and after it (0.9-2.9 s).
fn measure(c: &mut Client, track: usize, what: &str) -> ((f64, f64, f64), (f64, f64, f64)) {
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
        "{what}: {rate} Hz; 0.05-0.45 s: {:.2} Hz, ±{:.3} semitone, wobble {:.2} Hz; 0.9-2.9 s: \
         {:.2} Hz, ±{:.3} semitone, wobble {:.2} Hz",
        early.0, early.1, early.2, late.0, late.1, late.2
    );
    (early, late)
}

/// Requirement 2: 「Surge XT で LFO 1 をピッチに割り当て、量と「鳴ってから遅れてかかる」設定を
/// 付けたビブラートが鳴る。」 The note is rendered by the export's engine from the song's
/// state, which is the plugin's state after the step.
#[test]
#[ignore = "needs Surge XT and a real output (silent: no key is played)"]
fn requirement_2_a_delayed_vibrato_on_lfo_1_with_the_sound_running() {
    let (r, mut c, vst3, lv2) = live_core("conditions-2");
    for (key, format) in [(&vst3, "VST3"), (&lv2, "LV2")] {
        let added = ok(&mut c, "add_track", json!({ "plugin": key }));
        let track = added["track"].as_u64().unwrap() as usize;
        let at = json!({ "track": track });
        state_when_loaded(&mut c, &at);
        let init = ok(
            &mut c,
            "load_preset",
            with(
                &at,
                json!({ "id": "surge:factory/Templates/Init Sine.fxp" }),
            ),
        );
        assert_eq!(init["changed"], true, "{init}");
        assert_eq!(patch_now(&mut c, &at).0, "Init Sine");
        let (early, late) = measure(&mut c, track, &format!("{format} without the routing"));
        assert!(early.1 < 0.03 && late.1 < 0.03, "{format}: steady");

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
        eprintln!("{format}: surge_set_modulation: {set}");
        assert_eq!(set["changed"], true);
        assert_eq!(steps(&r), before + 1, "{format}: one step");
        assert_eq!(set["routings_now"][0]["depth"], 0.5, "{set}");
        // What the export's engine loads is the song's state: the plugin's since the step.
        assert_eq!(&*song_state(&r, track), &*plugin_bytes(&mut c, &at));
        let (early, late) = measure(&mut c, track, &format!("{format} with the routing"));
        assert!(
            early.1 < 0.05,
            "{format}: steady before the delay: {early:?}"
        );
        assert!(
            (late.1 - 0.5).abs() < 0.1,
            "{format}: ±0.5 semitone after it: {late:?}"
        );
        assert!((late.2 - 5.0).abs() < 0.5, "{format}: 5 Hz: {late:?}");
        assert!((early.0 - 440.0).abs() < 2.0 && (late.0 - 440.0).abs() < 2.0);
    }
}

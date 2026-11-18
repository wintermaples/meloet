//! Reading a pack: what loads, and what each broken file is told.

mod support;

use meloet_sound::sampler::{load_instrument, read_pack};
use support::pack::{TestPack, level, region, tone};

fn pack_with_samples(name: &str) -> TestPack {
    let pack = TestPack::new(name);
    pack.wav("mono", 44_100, &[&tone(441.0, 44_100, 4410, 0.5)]);
    pack.wav("stereo", 48_000, &[&level(0.5, 480), &level(0.25, 480)]);
    pack
}

/// Loads a pack with one region made of `fields` and gives the error's text.
fn region_error(name: &str, fields: &str) -> String {
    let pack = pack_with_samples(name);
    pack.instrument("", &[region("mono", 0, 127, 60, fields)]);
    let error = pack.try_load().err().expect("should not load");
    assert!(error.path.ends_with("instrument.json"), "{error}");
    error.to_string()
}

#[test]
fn a_good_instrument_loads_with_any_number_of_threads() {
    let pack = pack_with_samples("good");
    let json = pack.instrument(
        "\"credits\": [\"somebody\"], \"gainDb\": -3, \"polyphony\": 12, \"sustainPedal\": true",
        &[
            region("mono", 0, 60, 48, "\"loopStart\": 100, \"loopEnd\": 4410"),
            region("mono", 61, 127, 72, ""),
            region("stereo", 0, 0, 60, "\"trigger\": \"pedalDown\""),
        ],
    );
    for threads in [0, 1, 2, 4, 64] {
        let bank = load_instrument(&json, threads).unwrap();
        assert_eq!(bank.kind(), "test");
        assert_eq!(bank.polyphony(), 12);
        assert!(bank.sustain_pedal());
        assert_eq!(bank.region_count(), 3);
        // A file two regions use is in memory once.
        assert_eq!(bank.sample_count(), 2);
        assert_eq!(bank.sample_bytes(), (4410 + 480 * 2) * 2);
        assert!((bank.sample_seconds() - 0.11).abs() < 1.0e-9);
    }
}

#[test]
fn what_is_left_out_takes_its_usual_value() {
    let pack = pack_with_samples("defaults");
    pack.instrument("", &[region("mono", 60, 60, 60, "")]);
    let bank = pack.load();
    assert_eq!(bank.polyphony(), 64);
    assert!(!bank.sustain_pedal());
}

#[test]
fn broken_json_says_where() {
    let pack = pack_with_samples("broken");
    pack.write_json(
        "{\"format\": \"meloet-instrument\", \"version\": 1,\n \"kind\": \"x\", \"regions\": [",
    );
    let error = pack.try_load().err().unwrap();
    assert!(error.path.ends_with("instrument.json"));
    assert!(error.what.contains("line 2"), "{error}");

    pack.write_json("{\"format\": \"meloet-instrument\", \"version\": 1, \"regions\": []}");
    assert!(pack.try_load().err().unwrap().what.contains("kind"));

    pack.write_json(
        "{\"format\": \"meloet-sounds\", \"version\": 1, \"kind\": \"x\", \"regions\": []}",
    );
    assert!(
        pack.try_load()
            .err()
            .unwrap()
            .what
            .contains("meloet-instrument")
    );

    pack.write_json(
        "{\"format\": \"meloet-instrument\", \"version\": 2, \"kind\": \"x\", \"regions\": []}",
    );
    assert!(pack.try_load().err().unwrap().what.contains("version is 2"));

    pack.write_json(
        "{\"format\": \"meloet-instrument\", \"version\": 1, \"kind\": \"x\", \"regions\": \
         [{\"sample\": \"samples/mono.wav\", \"keyLo\": 0, \"keyHi\": 127}]}",
    );
    assert!(pack.try_load().err().unwrap().what.contains("root"));

    let missing = load_instrument(&pack.dir.join("nowhere.json"), 2)
        .err()
        .unwrap();
    assert!(missing.path.ends_with("nowhere.json"));
}

#[test]
fn a_misspelled_or_wrongly_typed_field_is_named() {
    for (name, fields, said) in [
        ("typo", "\"loopstart\": 3", "loopstart"),
        ("type", "\"oneShot\": 1", "oneShot"),
        ("trigger", "\"trigger\": \"hit\"", "hit"),
        ("negative", "\"rrCount\": -1, \"rrIndex\": 1", "-1"),
        ("choked-by-one", "\"chokedBy\": 1", "chokedBy"),
    ] {
        let text = region_error(name, fields);
        assert!(
            text.contains(said) || text.contains("line"),
            "{name}: {text}"
        );
    }
}

#[test]
fn values_out_of_range_name_the_region_and_the_field() {
    for (name, fields, said) in [
        ("vel-zero", "\"velLo\": 0", "velocities 0 to 127"),
        (
            "vel-order",
            "\"velLo\": 90, \"velHi\": 80",
            "velocities 90 to 80",
        ),
        ("vel-top", "\"velHi\": 128", "velocities 1 to 128"),
        ("tune", "\"tuneCents\": 5000", "tuneCents"),
        ("gain", "\"gainDb\": 100", "gainDb"),
        ("veltrack", "\"ampVeltrack\": 200", "ampVeltrack"),
        ("keytrack", "\"pitchKeytrack\": -5000", "pitchKeytrack"),
        ("attack", "\"attackMs\": -1", "attackMs"),
        ("release", "\"releaseMs\": -1", "releaseMs"),
        ("rt", "\"rtDecayDbPerS\": -3", "rtDecayDbPerS"),
        ("loop-decay", "\"loopDecayDbPerS\": -3", "loopDecayDbPerS"),
        ("pan", "\"pan\": 1.5", "pan"),
        (
            "loop-past",
            "\"loopStart\": 10, \"loopEnd\": 4411",
            "4410 frames",
        ),
        (
            "loop-order",
            "\"loopStart\": 300, \"loopEnd\": 200",
            "loop 300 to 200",
        ),
        (
            "loop-short",
            "\"loopStart\": 300, \"loopEnd\": 303",
            "at least",
        ),
        ("loop-half", "\"loopStart\": 300", "come together"),
        (
            "rr-past",
            "\"rrIndex\": 3, \"rrCount\": 2",
            "rrIndex is 3 of rrCount 2",
        ),
        ("rr-zero", "\"rrIndex\": 0, \"rrCount\": 2", "counts from 1"),
        ("choke-group", "\"chokeGroup\": 64", "chokeGroup is 64"),
        ("choked-by", "\"chokedBy\": [1, 99]", "chokedBy is 99"),
        ("rr-half", "\"rrCount\": 2", "come together"),
    ] {
        let text = region_error(name, fields);
        assert!(
            text.contains("regions[0] (samples/mono.wav)"),
            "{name}: {text}"
        );
        assert!(text.contains(said), "{name}: {text}");
    }

    let pack = pack_with_samples("keys");
    pack.instrument("", &[region("mono", 70, 60, 60, "")]);
    assert!(
        pack.try_load()
            .err()
            .unwrap()
            .what
            .contains("keys 70 to 60")
    );
    pack.instrument("", &[region("mono", 0, 128, 60, "")]);
    assert!(
        pack.try_load()
            .err()
            .unwrap()
            .what
            .contains("keys 0 to 128")
    );
    pack.instrument("", &[region("mono", 0, 127, 128, "")]);
    assert!(pack.try_load().err().unwrap().what.contains("root is 128"));
    // The second region is the bad one.
    pack.instrument(
        "",
        &[
            region("mono", 0, 127, 60, ""),
            region("stereo", 0, 127, 60, "\"pan\": -2"),
        ],
    );
    let text = pack.try_load().err().unwrap().what;
    assert!(text.contains("regions[1] (samples/stereo.wav)"), "{text}");

    for (top, said) in [
        ("\"polyphony\": 0", "polyphony is 0"),
        ("\"polyphony\": 1000", "polyphony is 1000"),
        ("\"gainDb\": 90", "gainDb"),
    ] {
        pack.instrument(top, &[region("mono", 0, 127, 60, "")]);
        let text = pack.try_load().err().unwrap().what;
        assert!(text.contains(said), "{top}: {text}");
    }
}

#[test]
fn a_sample_that_is_missing_broken_or_outside_the_pack_is_named() {
    let pack = pack_with_samples("files");
    pack.instrument(
        "",
        &[
            region("mono", 0, 60, 60, ""),
            region("gone", 61, 127, 60, ""),
        ],
    );
    let error = pack.try_load().err().unwrap();
    assert!(error.path.ends_with("samples/gone.wav"), "{error}");

    std::fs::write(
        pack.dir.join("samples/noise.wav"),
        b"this is not a sound file at all",
    )
    .unwrap();
    pack.instrument("", &[region("noise", 0, 127, 60, "")]);
    let error = pack.try_load().err().unwrap();
    assert!(error.path.ends_with("samples/noise.wav"), "{error}");

    let flat = level(0.1, 100);
    pack.wav("three", 48_000, &[&flat, &flat, &flat]);
    pack.instrument("", &[region("three", 0, 127, 60, "")]);
    let error = pack.try_load().err().unwrap();
    assert!(error.what.contains("3 channels"), "{error}");

    for outside in [
        "../samples/mono.wav",
        "/etc/passwd",
        "samples/../../x.wav",
        "",
    ] {
        pack.write_json(&format!(
            "{{\"format\": \"meloet-instrument\", \"version\": 1, \"kind\": \"x\", \"regions\": \
             [{{\"sample\": \"{outside}\", \"keyLo\": 0, \"keyHi\": 127, \"root\": 60}}]}}"
        ));
        let error = pack.try_load().err().unwrap();
        assert!(error.what.contains("regions[0].sample"), "{error}");
        assert!(error.what.contains("not a path inside the pack"), "{error}");
    }
}

#[test]
fn pack_json_lists_the_instruments() {
    let pack = TestPack::new("pack-json");
    let write = |text: &str| std::fs::write(pack.dir.join("pack.json"), text).unwrap();
    write(
        "{\"format\": \"meloet-sounds\", \"version\": 1, \"pack\": \"2026.11.0\", \
         \"instruments\": {\"studioKit\": \"studio-kit/instrument.json\", \
         \"grandPiano\": \"grand-piano/instrument.json\"}, \"credits\": \"credits.json\"}",
    );
    let read = read_pack(&pack.dir).unwrap();
    assert_eq!(read.pack, "2026.11.0");
    let names: Vec<&str> = read
        .instruments
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(names, ["grandPiano", "studioKit"]);
    assert_eq!(
        read.instruments[0].1,
        pack.dir.join("grand-piano/instrument.json")
    );

    write(
        "{\"format\": \"meloet-instrument\", \"version\": 1, \"pack\": \"x\", \"instruments\": {}}",
    );
    assert!(
        read_pack(&pack.dir)
            .err()
            .unwrap()
            .what
            .contains("meloet-sounds")
    );
    write(
        "{\"format\": \"meloet-sounds\", \"version\": 1, \"pack\": \"x\", \"instruments\": {\"a\": \"../a.json\"}}",
    );
    assert!(
        read_pack(&pack.dir)
            .err()
            .unwrap()
            .what
            .contains("instruments.a")
    );
    std::fs::remove_file(pack.dir.join("pack.json")).unwrap();
    assert!(
        read_pack(&pack.dir)
            .err()
            .unwrap()
            .path
            .ends_with("pack.json")
    );
}

//! The shapes protocol 33 added (round 12, waves 3 and 4): the arpeggio (E2), the knobs of a
//! built-in sound (E3), the spectrum's switch (E7), 「モノで聞く」 (M3), sections (A2), keys
//! made audio (A7), the master's volume lane and the fade-out (M5), 「いいとこ取り」 (A8),
//! 「音程を寄せる」 (M7) and the language (D4).

use serde_json::{Value, json};

use super::*;

fn to_json<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap()
}

fn both_ways<T>(v: T, json: Value)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    assert_eq!(to_json(&v), json);
    assert_eq!(serde_json::from_value::<T>(json).unwrap(), v);
}

#[test]
fn the_arpeggio() {
    let arp = Arp {
        on: true,
        shape: ArpShape::Random,
        rate: ArpRate::ThirtySecond,
        triplet: false,
        octaves: 3,
        hold: false,
    };
    both_ways(
        Command::SetArp { track: 7, arp },
        json!({"type": "setArp", "track": 7, "arp": {"on": true, "shape": "random",
               "rate": "thirtySecond", "triplet": false, "octaves": 3, "hold": false}}),
    );
    assert_eq!(
        to_json(&ArpShape::ALL),
        json!(["up", "down", "upDown", "random"])
    );
    assert_eq!(
        to_json(&ArpRate::ALL),
        json!(["quarter", "eighth", "sixteenth", "thirtySecond"])
    );
    assert_eq!((ARP_OCTAVES_MAX, ARP_KEYS_MAX), (3, 16));
    let off = Arp::default();
    assert!(!off.on);
    assert_eq!(
        (off.shape, off.rate, off.octaves),
        (ArpShape::Up, ArpRate::Sixteenth, 1)
    );
    let state = to_json(&tests::sample_state());
    assert_eq!(
        state["song"]["tracks"][0]["arp"],
        json!({"on": true, "shape": "upDown", "rate": "eighth", "triplet": true, "octaves": 2,
               "hold": true})
    );
}

#[test]
fn the_knobs_of_a_built_in_sound() {
    both_ways(
        Command::SetSoundKnob {
            track: 7,
            instrument: InstrumentKind::GrandPiano,
            knob: SoundKnob::Brightness,
            value: 62.5,
            commit: false,
        },
        json!({"type": "setSoundKnob", "track": 7, "instrument": "grandPiano",
               "knob": "brightness", "value": 62.5, "commit": false}),
    );
    assert_eq!(
        to_json(&SoundKnob::ALL),
        json!(["brightness", "attack", "release", "vibrato"])
    );
    assert_eq!(SOUND_KNOB_DEFAULTS, [50.0, 50.0, 50.0, 0.0]);
    both_ways(
        SelectedParams::Sound(SelectedSound {
            track: 7,
            sound: SoundName::Builtin {
                instrument: InstrumentKind::StudioKit,
            },
            knobs: Some(SoundKnobsView {
                values: [40.0, 50.0, 75.0, 0.0],
                usable: [true, false, true, false],
                row: Some(2),
            }),
        }),
        json!({"kind": "sound", "track": 7, "sound": {"kind": "builtin", "instrument": "studioKit"},
               "knobs": {"values": [40.0, 50.0, 75.0, 0.0], "usable": [true, false, true, false],
                         "row": 2}}),
    );
    both_ways(
        LaneName::Knob {
            knob: SoundKnob::Vibrato,
        },
        json!({"kind": "knob", "knob": "vibrato"}),
    );
}

#[test]
fn the_spectrum_mono_and_language_switches() {
    both_ways(
        Command::SetSpectrum { on: false },
        json!({"type": "setSpectrum", "on": false}),
    );
    both_ways(
        Command::SetMonoListen { on: true },
        json!({"type": "setMonoListen", "on": true}),
    );
    both_ways(
        Command::SetLanguage {
            language: Language::En,
        },
        json!({"type": "setLanguage", "language": "en"}),
    );
    assert_eq!(to_json(&Language::ALL), json!(["ja", "en"]));
    assert_eq!(Language::default(), Language::Ja);
    both_ways(
        HelloReply {
            protocol_version: PROTOCOL_VERSION,
            ok: true,
            mono_us: 5,
            language: Language::En,
        },
        json!({"protocolVersion": PROTOCOL_VERSION, "ok": true, "monoUs": 5, "language": "en"}),
    );
    let ws = to_json(&tests::sample_state().workspace);
    assert_eq!(
        (&ws["spectrum"], &ws["monoListen"], &ws["language"]),
        (&json!(true), &json!(true), &json!("ja"))
    );
}

#[test]
fn sections() {
    both_ways(
        Command::DuplicateSection { marker: 4 },
        json!({"type": "duplicateSection", "marker": 4}),
    );
    both_ways(
        Command::DeleteSection { marker: 4 },
        json!({"type": "deleteSection", "marker": 4}),
    );
    both_ways(
        Command::InsertSpace { marker: 4, bars: 8 },
        json!({"type": "insertSpace", "marker": 4, "bars": 8}),
    );
    assert_eq!((INSERT_SPACE_BARS_DEFAULT, INSERT_SPACE_BARS_MAX), (4, 64));
    let markers = &to_json(&tests::sample_state().timeline)["markers"];
    assert_eq!(
        (&markers[0]["section"], &markers[1]["section"]),
        (&json!("usable"), &json!("notOnBars"))
    );
    assert_eq!(to_json(&SectionUse::Empty), json!("empty"));
    both_ways(
        StepWhat::Section {
            name: "サビ".into(),
            bars: 8,
        },
        json!({"kind": "section", "name": "サビ", "bars": 8}),
    );
    both_ways(ValueField::Bars, json!("bars"));
}

#[test]
fn keys_made_audio() {
    both_ways(
        Command::BounceToAudio { clips: vec![4, 9] },
        json!({"type": "bounceToAudio", "clips": [4, 9]}),
    );
    both_ways(
        Command::BounceTrack { track: 7 },
        json!({"type": "bounceTrack", "track": 7}),
    );
    both_ways(Command::CancelBounce, json!({"type": "cancelBounce"}));
    both_ways(
        AudioWorkView {
            jobs: Vec::new(),
            bounce: Some(BounceView {
                track: TrackRef {
                    number: 1,
                    name: "ピアノ".into(),
                },
                percent: 40,
            }),
        },
        json!({"jobs": [], "bounce": {"track": {"number": 1, "name": "ピアノ"}, "percent": 40}}),
    );
}

#[test]
fn the_masters_lane_and_the_fade_out() {
    both_ways(
        Command::AddLane {
            home: LaneHome::Master,
            target: AutoTarget::Volume,
        },
        json!({"type": "addLane", "home": {"kind": "master"}, "target": {"kind": "volume"}}),
    );
    both_ways(
        LaneRef {
            home: LaneHome::Track { track: 7 },
            target: AutoTarget::Pan,
        },
        json!({"home": {"kind": "track", "track": 7}, "target": {"kind": "pan"}}),
    );
    assert_eq!(LaneHome::Track { track: 7 }.track(), Some(7));
    assert_eq!(LaneHome::Master.track(), None);
    both_ways(
        Command::AddFadeOut { bars: 4 },
        json!({"type": "addFadeOut", "bars": 4}),
    );
    assert_eq!((FADE_OUT_BARS_DEFAULT, FADE_OUT_BARS_MAX), (4, 16));
    let master = &to_json(&tests::sample_state().song)["master"];
    assert_eq!(
        master["lanes"],
        json!([{"target": {"kind": "volume"}, "name": {"kind": "volume"}, "laneId": 40,
                "points": 2, "step": false, "resting": false}])
    );
    both_ways(StepPlace::Master, json!({"kind": "master"}));
}

#[test]
fn comps() {
    let pieces = vec![
        CompPiece { from: 0, take: 31 },
        CompPiece {
            from: 2 * 1920,
            take: 33,
        },
    ];
    both_ways(
        Command::SetComp {
            clip: 9,
            span: TickSpan {
                start: 7680,
                end: 23_040,
            },
            pieces: pieces.clone(),
        },
        json!({"type": "setComp", "clip": 9, "span": {"start": 7680, "end": 23040},
               "pieces": [{"from": 0, "take": 31}, {"from": 3840, "take": 33}]}),
    );
    both_ways(
        Command::ClearComp { clip: 9 },
        json!({"type": "clearComp", "clip": 9}),
    );
    both_ways(
        AudioTakeView {
            take_id: 33,
            number: 3,
            audio: 30,
            first_frame: 96_000,
            frames: 48_000,
            take_offset: -1920,
            source_tempo: None,
            state: AudioClipState::Ready,
        },
        json!({"takeId": 33, "number": 3, "audio": 30, "firstFrame": 96000, "frames": 48000,
               "takeOffset": -1920, "sourceTempo": null, "state": "ready"}),
    );
    assert_eq!(COMP_ROWS_MAX, 8);
}

#[test]
fn tuning_a_vocal_clip() {
    both_ways(
        Command::SetClipStretch {
            clip: 9,
            change: StretchChange::TuneStrength { percent: 60.0 },
        },
        json!({"type": "setClipStretch", "clip": 9,
               "change": {"kind": "tuneStrength", "percent": 60.0}}),
    );
    both_ways(
        Command::SetClipStretch {
            clip: 9,
            change: StretchChange::TuneScale,
        },
        json!({"type": "setClipStretch", "clip": 9, "change": {"kind": "tuneScale"}}),
    );
    both_ways(
        TuneView {
            root: 9,
            mode: ScaleMode::Minor,
            strength: 60,
        },
        json!({"root": 9, "mode": "minor", "strength": 60}),
    );
    assert_eq!(TUNE_STRENGTH_MAX, 100);
}

#[test]
fn the_new_step_names() {
    let names = [
        (StepName::SetArp, "setArp"),
        (StepName::SetSoundKnob, "setSoundKnob"),
        (StepName::DuplicateSection, "duplicateSection"),
        (StepName::DeleteSection, "deleteSection"),
        (StepName::InsertSpace, "insertSpace"),
        (StepName::Bounce, "bounce"),
        (StepName::AddFadeOut, "addFadeOut"),
        (StepName::SetComp, "setComp"),
        (StepName::ClearComp, "clearComp"),
        (StepName::SetClipTune, "setClipTune"),
    ];
    for (name, kind) in names {
        assert!(StepName::every_kind().contains(&name), "{kind}");
        both_ways(name, json!({"kind": kind}));
    }
    both_ways(
        StepWhat::SoundKnob {
            track: TrackRef {
                number: 1,
                name: "ピアノ".into(),
            },
            knob: SoundKnob::Release,
            from: 50.0,
            to: 80.0,
        },
        json!({"kind": "soundKnob", "track": {"number": 1, "name": "ピアノ"}, "knob": "release",
               "from": 50.0, "to": 80.0}),
    );
}

#[test]
fn the_new_status_messages() {
    let piano = || TrackRef {
        number: 1,
        name: "ピアノ".into(),
    };
    let messages = [
        (
            StatusMessage::SoundKnobSelected {
                loop_bars: None,
                track: piano(),
                knob: SoundKnob::Attack,
            },
            json!({"id": "soundKnobSelected", "loopBars": null,
                   "track": {"number": 1, "name": "ピアノ"}, "knob": "attack"}),
        ),
        (StatusMessage::MonoListening, json!({"id": "monoListening"})),
        (StatusMessage::SongTooLong, json!({"id": "songTooLong"})),
        (
            StatusMessage::Bounced { track: piano() },
            json!({"id": "bounced", "track": {"number": 1, "name": "ピアノ"}}),
        ),
        (
            StatusMessage::BounceCancelled,
            json!({"id": "bounceCancelled"}),
        ),
        (
            StatusMessage::BounceFailed {
                reason: ExportFailure::CannotWrite,
            },
            json!({"id": "bounceFailed", "reason": {"kind": "cannotWrite"}}),
        ),
        (StatusMessage::BounceGone, json!({"id": "bounceGone"})),
        (StatusMessage::BounceBusy, json!({"id": "bounceBusy"})),
        (
            StatusMessage::NothingToBounce,
            json!({"id": "nothingToBounce"}),
        ),
        (
            StatusMessage::ExportWaitsForBounce,
            json!({"id": "exportWaitsForBounce"}),
        ),
        (
            StatusMessage::MasterAutomationWriting,
            json!({"id": "masterAutomationWriting"}),
        ),
        (
            StatusMessage::MasterAutomationRested,
            json!({"id": "masterAutomationRested"}),
        ),
        (
            StatusMessage::MasterAutomationRestored,
            json!({"id": "masterAutomationRestored"}),
        ),
        (
            StatusMessage::FadeOutNoClips,
            json!({"id": "fadeOutNoClips"}),
        ),
        (
            StatusMessage::AutomationMoving {
                tracks: vec![piano()],
                master: true,
            },
            json!({"id": "automationMoving", "tracks": [{"number": 1, "name": "ピアノ"}],
                   "master": true}),
        ),
    ];
    for (message, want) in messages {
        both_ways(message, want);
    }
}

#[test]
fn the_numbers_are_the_models() {
    assert_eq!(SOUND_KNOB_DEFAULTS, meloet_model::SOUND_KNOB_DEFAULTS);
    assert_eq!(ARP_OCTAVES_MAX, meloet_model::ARP_OCTAVES_MAX);
    assert_eq!(ARP_KEYS_MAX as usize, meloet_model::ARP_KEYS_MAX);
    assert_eq!(TUNE_STRENGTH_MAX, meloet_model::TUNE_STRENGTH_MAX);
    assert_eq!(SoundKnob::ALL.len(), meloet_model::SoundKnob::ALL.len());
    assert_eq!(ArpShape::ALL.len(), meloet_model::ArpShape::ALL.len());
    assert_eq!(ArpRate::ALL.len(), meloet_model::ArpRate::ALL.len());
}

fn generated_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/src/bridge/generated")
}

/// Writes `ui/src/bridge/generated/round12wave3.ts`: the numbers and lists protocol 33 added.
#[test]
fn export_round12_wave3_numbers() {
    let dir = generated_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let list = |v: Value| serde_json::to_string(&v).unwrap();
    std::fs::write(
        dir.join("round12wave3.ts"),
        format!(
            "// Written by `cargo test -p meloet-protocol`. Do not edit this file manually.\n\
             import type {{ ArpRate }} from \"./ArpRate\";\n\
             import type {{ ArpShape }} from \"./ArpShape\";\n\
             import type {{ Language }} from \"./Language\";\n\
             import type {{ SoundKnob }} from \"./SoundKnob\";\n\n\
             /** 「アルペジオ」: 「形」 and 「速さ」 in the order of the window, the most octaves, and the most keys it plays. */\n\
             export const ARP_SHAPES: readonly ArpShape[] = {};\n\
             export const ARP_RATES: readonly ArpRate[] = {};\n\
             export const ARP_OCTAVES_MAX = {ARP_OCTAVES_MAX};\n\
             export const ARP_KEYS_MAX = {ARP_KEYS_MAX};\n\n\
             /** The knobs of a built-in sound in the order of the rows (a knob's place is its `param`), and where each starts (percent). */\n\
             export const SOUND_KNOBS: readonly SoundKnob[] = {};\n\
             export const SOUND_KNOB_DEFAULTS: readonly number[] = {};\n\n\
             /** 「この前に空きを入れる…」: the bars its box starts at, and the most. */\n\
             export const INSERT_SPACE_BARS_DEFAULT = {INSERT_SPACE_BARS_DEFAULT};\n\
             export const INSERT_SPACE_BARS_MAX = {INSERT_SPACE_BARS_MAX};\n\n\
             /** 「曲の終わりにフェードアウトをつける…」: the bars its box starts at, and the most. */\n\
             export const FADE_OUT_BARS_DEFAULT = {FADE_OUT_BARS_DEFAULT};\n\
             export const FADE_OUT_BARS_MAX = {FADE_OUT_BARS_MAX};\n\n\
             /** 「いいとこ取り」: the most rows of takes the band shows (the newest). */\n\
             export const COMP_ROWS_MAX = {COMP_ROWS_MAX};\n\n\
             /** 「音程を寄せる」: the strength at its end (percent). */\n\
             export const TUNE_STRENGTH_MAX = {TUNE_STRENGTH_MAX};\n\n\
             /** 「ことば / Language」, in the order of the choice. */\n\
             export const LANGUAGES: readonly Language[] = {};\n",
            list(json!(ArpShape::ALL)),
            list(json!(ArpRate::ALL)),
            list(json!(SoundKnob::ALL)),
            list(json!(SOUND_KNOB_DEFAULTS)),
            list(json!(Language::ALL)),
        ),
    )
    .unwrap();
}

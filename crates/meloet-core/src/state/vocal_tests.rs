//! The rules of vocal tracks, the input and the latency (protocol 23).

use meloet_devices::{Availability, ClockSettings, LinkKind};
use meloet_model::record::HeldControls;
use meloet_model::{ClipTakes, TrackId};
use meloet_protocol::{
    Band, LatencyFailure, LatencyMeasure, LatencySource, RecordFormat, TrackKindView, TrackRef,
    VocalRef,
};

use super::*;
use crate::settings::MeasuredLatency;
use crate::vocalrec::Marks;

const OUT: &str = "card:out";
const OUT_NODE: &str = "alsa_output.card";
const MIC: &str = "usb:mic";
const MIC_NODE: &str = "alsa_input.usb";
const Q: u64 = 1_920;

fn endpoint(id: &str, node: &str, availability: Availability, channels: Option<u8>) -> Endpoint {
    Endpoint {
        id: id.into(),
        name: id.into(),
        friendly: None,
        availability,
        link: LinkKind::Plug,
        active: true,
        node_name: Some(node.into()),
        device_name: None,
        device_description: None,
        channels,
    }
}

fn devices(inputs: Vec<Endpoint>) -> Input {
    Input::Devices(DevicesPicture {
        outputs: vec![endpoint(OUT, OUT_NODE, Availability::Yes, Some(2))],
        inputs,
        clock: ClockSettings::default(),
    })
}

fn mic(channels: u8) -> Endpoint {
    endpoint(MIC, MIC_NODE, Availability::Yes, Some(channels))
}

fn cmd(m: &mut Model, command: Command) -> Vec<Effect> {
    m.handle(Input::Command(command))
}

/// The output chosen and open at 48 kHz, with these inputs.
fn running(inputs: Vec<Endpoint>) -> Model {
    let mut m = Model::new(Settings {
        output: Some(OUT.into()),
        count_in_bars: 0,
        ..Settings::default()
    });
    let effects = m.handle(devices(inputs));
    assert!(effects.contains(&Effect::OpenOutput {
        node_name: OUT_NODE.into()
    }));
    m.handle(Input::AudioOpened {
        node_name: OUT_NODE.into(),
        sample_rate: 48_000,
    });
    m
}

/// With a vocal track added (track 2, id 4... whatever id it got) and the input open.
fn with_vocal(channels: u8) -> (Model, u32) {
    let mut m = running(vec![mic(channels)]);
    let effects = cmd(&mut m, Command::AddVocalTrack);
    assert!(
        effects.contains(&Effect::OpenInput {
            node_name: MIC_NODE.into(),
            sink_monitor: false
        }),
        "{effects:?}"
    );
    m.handle(Input::InputOpened {
        node_name: MIC_NODE.into(),
        channels: u16::from(channels),
        sample_rate: 48_000,
    });
    let id = m.view().song.tracks[1].id;
    (m, id)
}

fn vocal(m: &Model, id: u32) -> TrackKindView {
    let view = m.view();
    view.song.tracks.iter().find(|t| t.id == id).unwrap().kind
}

fn voices(effects: &[Effect]) -> Vec<(usize, Option<crate::play::VoiceSpec>)> {
    (effects.iter())
        .filter_map(|e| match e {
            Effect::Engine(EngineOp::Voice { seat, voice }) => Some((*seat, *voice)),
            _ => None,
        })
        .collect()
}

fn status(m: &Model) -> Option<StatusMessage> {
    m.view().status.message
}

#[test]
fn a_vocal_track_is_added_selected_and_armed_and_the_keys_stay_where_they_were() {
    let mut m = running(vec![mic(1)]);
    assert!(!m.view().inputs.open);
    assert_eq!(m.view().inputs.chosen.as_deref(), Some(MIC));
    let undo = m.view().song.undo_count;

    let effects = cmd(&mut m, Command::AddVocalTrack);
    let view = m.view();
    assert_eq!(view.song.tracks.len(), 2);
    let track = &view.song.tracks[1];
    assert_eq!(track.name, "歌");
    assert_eq!(view.song.undo_count, undo + 1);
    assert_eq!(view.song.play_target, Some(track.id));
    assert_eq!(
        view.song.keys_target,
        Some(1),
        "the keys still play the piano"
    );
    assert_eq!(
        track.kind,
        TrackKindView::Vocal {
            input: meloet_protocol::VocalInput::Mono1,
            input_ready: true,
            armed: true,
            monitor: false,
        }
    );
    assert_eq!(
        view.status.message,
        Some(StatusMessage::VocalTrackAdded {
            track: TrackRef {
                number: 2,
                name: "歌".into()
            },
            input: meloet_protocol::VocalInput::Mono1,
        })
    );
    // The keys' seat is not moved, the track sounds the input (silent until monitored), and
    // the input is opened.
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::Engine(EngineOp::PlayTarget { .. })))
    );
    assert_eq!(voices(&effects).len(), 1);
    assert!(voices(&effects)[0].1.is_some_and(|v| !v.on));
    assert!(effects.contains(&Effect::OpenInput {
        node_name: MIC_NODE.into(),
        sink_monitor: false
    }));
    assert!(!m.view().inputs.open, "not before it has answered");
    m.handle(Input::InputOpened {
        node_name: MIC_NODE.into(),
        channels: 1,
        sample_rate: 48_000,
    });
    assert!(m.view().inputs.open);

    // A second one: 「歌 2」.
    cmd(&mut m, Command::AddVocalTrack);
    assert_eq!(m.view().song.tracks[2].name, "歌 2");
    assert_eq!(m.view().song.keys_target, Some(1));

    // Selecting the piano again closes nothing: both vocal tracks keep their ●.
    let effects = cmd(
        &mut m,
        Command::SelectTrack {
            track: 1,
            from_arm: false,
        },
    );
    assert!(!effects.contains(&Effect::CloseInput));
    assert_eq!(m.play.song().session.last_keys, None);
}

#[test]
fn the_input_is_open_only_while_a_vocal_track_is_selected_or_armed() {
    let (mut m, id) = with_vocal(2);
    cmd(
        &mut m,
        Command::SelectTrack {
            track: 1,
            from_arm: false,
        },
    );
    assert!(m.view().inputs.open, "its ● is still on");
    let effects = cmd(&mut m, Command::ToggleVocalArm { track: id });
    assert!(effects.contains(&Effect::CloseInput), "{effects:?}");
    assert!(!m.view().inputs.open);
    assert!(matches!(
        vocal(&m, id),
        TrackKindView::Vocal { armed: false, .. }
    ));

    // Selecting it turns the ● on again (spec 3, 24-C) and opens the input.
    let effects = cmd(
        &mut m,
        Command::SelectTrack {
            track: id,
            from_arm: false,
        },
    );
    assert!(matches!(
        vocal(&m, id),
        TrackKindView::Vocal { armed: true, .. }
    ));
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::OpenInput { .. }))
    );
    assert_eq!(m.view().song.keys_target, Some(1));
    assert_eq!(m.play.song().session.last_keys, Some(TrackId(1)));

    // A keys track's ● is not a vocal one's.
    assert!(cmd(&mut m, Command::ToggleVocalArm { track: 1 }).is_empty());

    // The output goes away: the input goes with it, and comes back with it.
    let effects = m.handle(devices(vec![mic(2)]));
    assert!(!effects.contains(&Effect::CloseInput));
    let effects = m.handle(Input::Devices(DevicesPicture {
        outputs: vec![],
        inputs: vec![mic(2)],
        clock: ClockSettings::default(),
    }));
    assert!(effects.contains(&Effect::CloseInput), "{effects:?}");
    assert!(effects.contains(&Effect::CloseOutput));
    assert!(!m.view().inputs.open);
}

#[test]
fn no_input_is_said_and_shown() {
    // The only input is not plugged in.
    let unplugged = endpoint(MIC, MIC_NODE, Availability::No, Some(2));
    let mut m = running(vec![unplugged]);
    assert_eq!(m.view().inputs.chosen, None);
    let effects = cmd(&mut m, Command::AddVocalTrack);
    assert_eq!(status(&m), Some(StatusMessage::NoInput));
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::OpenInput { .. }))
    );
    let id = m.view().song.tracks[1].id;
    assert!(matches!(
        vocal(&m, id),
        TrackKindView::Vocal {
            input_ready: false,
            armed: true,
            ..
        }
    ));
    // R: the piano is recorded, and the line says why the voice is not.
    let effects = cmd(&mut m, Command::Record);
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::StartVocalRecording { .. }))
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Engine(EngineOp::Record { .. })))
    );
    assert!(m.view().transport.recording.is_some());
    assert!(m.view().transport.recording_vocals.is_empty());

    // A mono microphone has no second channel.
    let (mut m, id) = with_vocal(1);
    cmd(
        &mut m,
        Command::SetVocalInput {
            track: id,
            input: meloet_protocol::VocalInput::Stereo12,
        },
    );
    assert!(matches!(
        vocal(&m, id),
        TrackKindView::Vocal {
            input: meloet_protocol::VocalInput::Stereo12,
            input_ready: false,
            ..
        }
    ));
    assert_eq!(m.view().song.undo_count, 2, "the input is a step");

    // The input chosen in the settings is used when it is there.
    let other = endpoint("card:line", "alsa_input.line", Availability::Yes, Some(2));
    let mut m = running(vec![mic(1), other]);
    assert_eq!(m.view().inputs.chosen.as_deref(), Some(MIC));
    let effects = cmd(
        &mut m,
        Command::SettingsSelectInput {
            input: Some("card:line".into()),
        },
    );
    assert!(
        matches!(&effects[..], [Effect::SaveSettings(s)] if s.sound.input.as_deref() == Some("card:line"))
    );
    assert_eq!(m.view().inputs.chosen.as_deref(), Some("card:line"));
    assert_eq!(m.view().inputs.list.len(), 2);
    assert_eq!(m.view().inputs.list[0].channels, Some(1));
    assert!(
        cmd(
            &mut m,
            Command::SettingsSelectInput {
                input: Some("nothing".into())
            }
        )
        .is_empty()
    );
}

#[test]
fn the_monitor_is_heard_only_with_the_arm_on_and_says_how_late_it_is_once() {
    let (mut m, id) = with_vocal(2);
    m.handle(Input::AudioDelays(Some(Delays {
        round_us: 8_000,
        monitor_us: 13_300,
    })));
    let effects = cmd(&mut m, Command::ToggleMonitor { track: id });
    assert_eq!(
        status(&m),
        Some(StatusMessage::MonitorFirstOn {
            latency_ms: Some(13.3)
        })
    );
    let on = voices(&effects);
    assert!(matches!(&on[..], [(_, Some(v))] if v.on), "{on:?}");
    assert!(matches!(
        vocal(&m, id),
        TrackKindView::Vocal { monitor: true, .. }
    ));
    assert_eq!(m.view().song.undo_count, 1, "no step");

    // The ● off: silent, with the monitor still on.
    let effects = cmd(&mut m, Command::ToggleVocalArm { track: id });
    assert!(matches!(&voices(&effects)[..], [(_, Some(v))] if !v.on));
    let effects = cmd(&mut m, Command::ToggleVocalArm { track: id });
    assert!(matches!(&voices(&effects)[..], [(_, Some(v))] if v.on));

    // Off and on again: nothing is said the second time.
    cmd(&mut m, Command::ToggleMonitor { track: id });
    cmd(&mut m, Command::ToggleMonitor { track: id });
    assert!(!matches!(
        status(&m),
        Some(StatusMessage::MonitorFirstOn { .. })
    ));
    assert!(cmd(&mut m, Command::ToggleMonitor { track: 1 }).is_empty());

    // The track goes: its seat no longer sounds the input.
    let effects = cmd(&mut m, Command::DeleteTrack { track: id });
    assert!(
        voices(&effects).iter().any(|(_, v)| v.is_none()),
        "{effects:?}"
    );
    assert!(m.play.song().session.armed_vocals.is_empty());
    assert!(m.play.song().session.monitored.is_empty());
}

#[test]
fn a_vocal_track_has_no_piano_roll_and_the_mic_can_be_too_loud() {
    let (mut m, id) = with_vocal(1);
    cmd(
        &mut m,
        Command::ToggleBand {
            band: Band::PianoRoll,
        },
    );
    assert_eq!(status(&m), Some(StatusMessage::VocalHasNoPianoRoll));
    assert!(m.view().workspace.layout.lower.is_empty());
    m.handle(Input::MicClipped);
    assert_eq!(status(&m), Some(StatusMessage::MicTooLoud));
    cmd(
        &mut m,
        Command::SelectTrack {
            track: 1,
            from_arm: false,
        },
    );
    cmd(
        &mut m,
        Command::ToggleBand {
            band: Band::PianoRoll,
        },
    );
    assert_eq!(m.view().workspace.layout.lower.len(), 1);
    let _ = id;
}

fn started(m: &mut Model, from: u64) {
    m.handle(Input::Record(RecordReport::Started {
        from,
        held: HeldControls::default(),
    }));
}

/// Starts a recording from the head at 96 (a bar is 2.5 s, 120000 frames): the files are
/// there, and the song rolls 12000 frames (less a quarter) into them.
fn record_vocals(m: &mut Model) -> Vec<Effect> {
    let effects = cmd(m, Command::Record);
    let names = (effects.iter())
        .find_map(|e| match e {
            Effect::StartVocalRecording { files, .. } => {
                Some(files.iter().map(|f| f.name.clone()).collect())
            }
            _ => None,
        })
        .unwrap_or_default();
    started(m, 0);
    m.handle(Input::VocalsStarted { names });
    m.handle(Input::Record(RecordReport::Rolling {
        frame: 13_000,
        tick: 0.0,
        file_frame: Some(11_999.75),
    }));
    effects
}

fn written(frames: u64) -> Input {
    Input::VocalsWritten(Box::new(Written {
        frames,
        sample_rate: 48_000,
        ..Written::default()
    }))
}

#[test]
fn the_keys_and_the_voice_are_recorded_together_as_one_step() {
    let (mut m, id) = with_vocal(2);
    cmd(
        &mut m,
        Command::SettingsSetRecordFormat {
            format: RecordFormat::Bits32,
        },
    );
    assert!(m.view().transport.can_record);
    let undo = m.view().song.undo_count;
    let next_id = m.play.song().doc.next_id;
    let name = format!("歌 {next_id}.wav");

    let effects = record_vocals(&mut m);
    // The files are asked for before the engine is told.
    let start = effects
        .iter()
        .position(|e| matches!(e, Effect::StartVocalRecording { .. }));
    let engine = effects
        .iter()
        .position(|e| matches!(e, Effect::Engine(EngineOp::Record { .. })));
    assert!(start.is_some() && start < engine, "{effects:?}");
    assert_eq!(
        effects[start.unwrap()],
        Effect::StartVocalRecording {
            files: vec![VocalFileAsk {
                audio: next_id,
                name: name.clone(),
                channels: vec![0]
            }],
            format: RecordFormat::Bits32,
            x_us: 0,
        }
    );
    let view = m.view();
    assert_eq!(view.transport.recording.as_ref().map(|r| r.track), Some(1));
    assert_eq!(view.transport.recording_vocals.len(), 1);
    assert_eq!(
        (
            view.transport.recording_vocals[0].track,
            view.transport.recording_vocals[0].take
        ),
        (id, 1)
    );
    assert_eq!(
        view.status.message,
        Some(StatusMessage::RecordingVocals {
            keys: Some(TrackRef {
                number: 1,
                name: "ピアノ".into()
            }),
            vocals: vec![VocalRef {
                track: TrackRef {
                    number: 2,
                    name: "歌".into()
                },
                input: meloet_protocol::VocalInput::Mono1,
            }],
            monitor: false,
        })
    );

    // The note about the recording was written when the song rolled: where the files
    // belong is in it from then on.
    let note = m.play.noted_vocals().expect("a note");
    assert_eq!(note.roll, Some((11_999.75, 0.0)));
    assert_eq!((note.from, note.loop_range), (0, None));

    // A key, and the stop a bar in.
    m.handle(Input::Record(RecordReport::Event {
        kind: meloet_model::record::RecEventKind::NoteOn {
            key: 60,
            velocity: 90,
        },
        t_us: 10,
        tick: 100.0,
        lap: 0,
    }));
    m.handle(Input::Record(RecordReport::Event {
        kind: meloet_model::record::RecEventKind::NoteOff { key: 60 },
        t_us: 11,
        tick: 500.0,
        lap: 0,
    }));
    cmd(&mut m, Command::PlayStop);
    let effects = m.handle(Input::Record(RecordReport::Stopped {
        t_us: 12,
        tick: 4.0 * Q as f64,
        lap: 0,
    }));
    assert!(effects.contains(&Effect::StopVocalRecording), "{effects:?}");
    assert_eq!(m.view().song.undo_count, undo, "waits for the disk");
    assert!(m.play.busy());

    let effects = m.handle(written(140_000));
    assert!(effects.contains(&Effect::RemoveRecNote));
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::DropVocalFiles(_)))
    );
    assert!(!m.play.busy());
    let view = m.view();
    assert_eq!(view.song.undo_count, undo + 1, "one recording is one step");
    assert!(view.transport.recording_vocals.is_empty());
    let doc = &m.play.song().doc;
    assert_eq!(doc.audio.len(), 1);
    let source = &doc.audio[0];
    assert_eq!(
        (
            source.id.0,
            source.file.as_str(),
            source.frames,
            source.channels
        ),
        (next_id, name.as_str(), 140_000, 1)
    );
    assert_ne!(source.uid, [0; 16]);
    let clip = &doc.track(TrackId(id)).unwrap().clips[0];
    assert_eq!((clip.start, clip.end), (0, 4 * Q));
    let ClipTakes::Vocal { takes, .. } = &clip.takes else {
        panic!("a vocal clip");
    };
    // The song rolled 12000 frames (less a quarter) into the file; a bar is 120000 frames.
    assert_eq!(
        (takes[0].take.first_frame, takes[0].take.frames),
        (12_000, 120_000)
    );
    assert_eq!(
        doc.track(TrackId(1)).unwrap().clips.len(),
        1,
        "the keys' clip"
    );

    // Undo takes both away, redo brings both back.
    cmd(&mut m, Command::Undo);
    let doc = &m.play.song().doc;
    assert!(doc.audio.is_empty());
    assert!(doc.tracks.iter().all(|t| t.clips.is_empty()));
    cmd(&mut m, Command::Redo);
    assert_eq!(m.play.song().doc.audio.len(), 1);
}

#[test]
fn only_the_voice_is_recorded_when_there_is_no_keys_track_and_a_loop_makes_takes() {
    let (mut m, id) = with_vocal(2);
    cmd(&mut m, Command::DeleteTrack { track: 1 });
    assert_eq!(m.view().song.keys_target, None);
    assert!(m.view().transport.can_record);
    cmd(
        &mut m,
        Command::SetLoop {
            from_bar: 1,
            to_bar: 1,
        },
    );
    cmd(&mut m, Command::ToggleMonitor { track: id });

    record_vocals(&mut m);
    assert_eq!(m.view().transport.recording, None);
    assert!(matches!(
        status(&m),
        Some(StatusMessage::RecordingVocals {
            keys: None,
            monitor: true,
            ..
        })
    ));
    // The disk writer says where the first lap's mark is; the second is counted on.
    for (lap, frame, file_frame) in [(1, 133_000, Some(131_999.75)), (2, 253_000, None)] {
        m.handle(Input::Record(RecordReport::LapChanged {
            lap,
            frame,
            file_frame,
        }));
    }
    assert_eq!(m.view().transport.recording_vocals[0].take, 3);
    // The wave of the lap going on: from the loop's start, with the file's frame there.
    let rec = m.view().transport.recording_vocals[0].clone();
    assert_eq!((rec.from, rec.first_frame), (0, Some(252_000)));
    cmd(&mut m, Command::PlayStop);
    m.handle(Input::Record(RecordReport::Stopped {
        t_us: 1,
        tick: 2.0 * Q as f64,
        lap: 2,
    }));
    m.handle(written(400_000));
    let doc = &m.play.song().doc;
    let clip = &doc.track(TrackId(id)).unwrap().clips[0];
    assert_eq!((clip.start, clip.end, clip.active), (0, 4 * Q, 2));
    let frames: Vec<(u64, u64)> = (clip.takes.vocal().iter())
        .map(|t| (t.take.first_frame, t.take.frames))
        .collect();
    assert_eq!(
        frames,
        [(12_000, 120_000), (132_000, 120_000), (252_000, 60_000)]
    );
    assert!(
        clip.takes
            .vocal()
            .iter()
            .all(|t| Arc::ptr_eq(&t.take.source, &doc.audio[0]))
    );
    let next_id = doc.next_id;

    // Another recording over the clip is its fourth take; its file has the next id.
    let effects = record_vocals(&mut m);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::StartVocalRecording { files, .. }
        if files[0].name == format!("歌 {next_id}.wav")))
    );
    assert_eq!(m.view().transport.recording_vocals[0].take, 4);
}

#[test]
fn a_recording_stopped_before_it_rolls_leaves_no_file() {
    let (mut m, _) = with_vocal(2);
    cmd(&mut m, Command::SetCountIn { bars: 1 });
    let effects = cmd(&mut m, Command::Record);
    let Some(Effect::StartVocalRecording { files, .. }) = effects
        .iter()
        .find(|e| matches!(e, Effect::StartVocalRecording { .. }))
    else {
        panic!("{effects:?}");
    };
    let name = files[0].name.clone();
    started(&mut m, 0);
    let effects = m.handle(Input::VocalsStarted {
        names: vec![name.clone()],
    });
    let Some(Effect::WriteRecNote(note)) = effects
        .iter()
        .find(|e| matches!(e, Effect::WriteRecNote(_)))
    else {
        panic!("{effects:?}");
    };
    assert_eq!(
        (note.sample_rate, note.bits, note.files.len()),
        (48_000, 24, 1)
    );
    assert_eq!(
        (
            note.files[0].file.as_str(),
            note.files[0].track_name.as_str()
        ),
        (name.as_str(), "歌")
    );
    assert_eq!(
        note.marks,
        Marks {
            from: 0,
            ..Marks::default()
        }
    );
    cmd(&mut m, Command::PlayStop);
    let effects = m.handle(Input::Record(RecordReport::Stopped {
        t_us: 1,
        tick: -900.0,
        lap: 0,
    }));
    assert!(effects.contains(&Effect::StopVocalRecording));
    let effects = m.handle(written(20_000));
    assert!(effects.contains(&Effect::RemoveRecNote));
    assert!(
        effects.contains(&Effect::DropVocalFiles(vec![name])),
        "{effects:?}"
    );
    assert!(m.play.song().doc.audio.is_empty());
    assert!(!m.play.busy());

    // Files that could not be made: the keys are recorded all the same.
    cmd(&mut m, Command::SetCountIn { bars: 0 });
    cmd(&mut m, Command::Record);
    started(&mut m, 0);
    m.handle(Input::VocalsNotStarted);
    assert!(m.view().transport.recording_vocals.is_empty());
    assert!(m.view().transport.recording.is_some());
    cmd(&mut m, Command::PlayStop);
    let effects = m.handle(Input::Record(RecordReport::Stopped {
        t_us: 1,
        tick: 100.0,
        lap: 0,
    }));
    assert!(!effects.contains(&Effect::StopVocalRecording));
    assert!(!m.play.busy());

    // The disk is full, or the microphone is pulled out: the recording is stopped.
    record_vocals(&mut m);
    let effects = m.handle(Input::VocalsCannotGoOn);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::Engine(EngineOp::Stop { .. }))),
        "{effects:?}"
    );
}

#[test]
fn the_advanced_sound_settings_are_kept_and_the_stream_is_opened_anew() {
    let (mut m, _) = with_vocal(2);
    let audio = m.view().audio;
    assert_eq!(
        (audio.requested_frames, audio.requested_sample_rate),
        (256, 48_000)
    );
    assert_eq!(audio.buffer_choices, [64, 128, 256, 512, 1024]);
    assert_eq!(audio.record_format, RecordFormat::Bits24);

    let effects = cmd(&mut m, Command::SettingsSetBuffer { frames: 128 });
    assert!(matches!(&effects[0], Effect::SaveSettings(s) if s.sound.buffer == 128));
    assert!(effects.contains(&Effect::CloseInput));
    assert!(effects.contains(&Effect::OpenOutput {
        node_name: OUT_NODE.into()
    }));
    assert!(!m.view().audio.running);
    assert_eq!(m.view().audio.requested_frames, 128);
    let effects = m.handle(Input::AudioOpened {
        node_name: OUT_NODE.into(),
        sample_rate: 48_000,
    });
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::OpenInput { .. })),
        "the input comes back"
    );
    // What runs is what the PC says, not what was asked for.
    m.handle(Input::AudioFrames(Some(256)));
    assert_eq!(m.view().audio.actual_frames, Some(256));

    assert!(cmd(&mut m, Command::SettingsSetBuffer { frames: 100 }).is_empty());
    assert!(cmd(&mut m, Command::SettingsSetBuffer { frames: 128 }).is_empty());
    let effects = cmd(&mut m, Command::SettingsSetSampleRate { hz: 44_100 });
    assert!(matches!(&effects[0], Effect::SaveSettings(s) if s.sound.sample_rate == 44_100));
    assert_eq!(m.view().audio.requested_sample_rate, 44_100);
    assert!(cmd(&mut m, Command::SettingsSetSampleRate { hz: 12_345 }).is_empty());

    let effects = cmd(
        &mut m,
        Command::SettingsSetRecordFormat {
            format: RecordFormat::Bits16,
        },
    );
    assert!(
        matches!(&effects[..], [Effect::SaveSettings(s)] if s.sound.record_format == RecordFormat::Bits16)
    );
    assert_eq!(m.view().audio.record_format, RecordFormat::Bits16);
}

#[test]
fn the_latency_is_what_the_pc_says_until_it_is_measured_or_typed() {
    let (mut m, _) = with_vocal(2);
    assert_eq!(m.view().audio.latency.reported_ms, None);
    m.handle(Input::AudioDelays(Some(Delays {
        round_us: 8_000,
        monitor_us: 13_300,
    })));
    let latency = m.view().audio.latency;
    assert_eq!(
        (latency.reported_ms, latency.measured_ms, latency.manual_ms),
        (Some(8.0), None, None)
    );
    assert_eq!(
        (latency.used, latency.used_ms),
        (LatencySource::Reported, Some(8.0))
    );
    assert_eq!(m.latency_x_us(), 0);

    // The dialog: five clicks, their middle, and 「この数を使う」.
    cmd(&mut m, Command::OpenLatencyMeasure);
    assert_eq!(status(&m), Some(StatusMessage::LatencyMeasureOpen));
    assert_eq!(
        m.view().workspace.latency_dialog.map(|d| d.measure),
        Some(LatencyMeasure::Idle)
    );
    let effects = cmd(&mut m, Command::StartLatencyMeasure);
    assert_eq!(effects, [Effect::LatencyListen(true), Effect::LatencyClick]);
    assert!(!m.view().transport.can_record);
    assert!(
        cmd(&mut m, Command::Record).is_empty(),
        "not while measuring"
    );
    for (n, x) in [4_400.0, 4_410.0, 4_405.0, 4_395.0].into_iter().enumerate() {
        let effects = m.handle(Input::LatencyAnswer { x_us: Some(x) });
        assert_eq!(effects, [Effect::LatencyClick]);
        let Some(LatencyMeasure::Measuring { done, of, mean_ms }) =
            m.view().workspace.latency_dialog.map(|d| d.measure)
        else {
            panic!("measuring");
        };
        assert_eq!((done, of), (n as u8 + 1, 5));
        assert!(mean_ms.is_some_and(|ms| (ms - 12.4).abs() < 0.05));
    }
    let effects = m.handle(Input::LatencyAnswer {
        x_us: Some(4_402.0),
    });
    assert_eq!(effects, [Effect::LatencyListen(false)]);
    assert_eq!(
        m.view().workspace.latency_dialog.map(|d| d.measure),
        Some(LatencyMeasure::Done { measured_ms: 12.4 })
    );
    let effects = cmd(&mut m, Command::UseMeasuredLatency);
    let [Effect::SaveSettings(saved)] = &effects[..] else {
        panic!("{effects:?}");
    };
    assert!(
        matches!(&saved.sound.measured[..], [MeasuredLatency { output, input, sample_rate: 48_000, x_us: 4_402, .. }]
        if output == OUT && input == MIC)
    );
    assert_eq!(m.view().workspace.latency_dialog, None);
    let latency = m.view().audio.latency;
    assert_eq!(
        (latency.used, latency.used_ms),
        (LatencySource::Measured, Some(12.4))
    );
    assert_eq!(m.latency_x_us(), 4_402);

    // A recording is placed with it.
    let effects = cmd(&mut m, Command::Record);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::StartVocalRecording { x_us: 4_402, .. }))
    );
    cmd(&mut m, Command::PlayStop);
    m.handle(Input::Record(RecordReport::Stopped {
        t_us: 1,
        tick: 0.0,
        lap: 0,
    }));
    m.handle(written(0));

    // A typed number is used before the measured one; it is the whole round trip.
    let effects = cmd(
        &mut m,
        Command::SettingsSetManualLatency { ms: Some(900.0) },
    );
    assert!(
        matches!(&effects[..], [Effect::SaveSettings(s)] if s.sound.manual_latency_us == Some(500_000))
    );
    cmd(&mut m, Command::SettingsSetManualLatency { ms: Some(10.0) });
    let latency = m.view().audio.latency;
    assert_eq!(
        (latency.used, latency.used_ms),
        (LatencySource::Manual, Some(10.0))
    );
    assert_eq!(m.latency_x_us(), 2_000);
    cmd(&mut m, Command::SettingsSetManualLatency { ms: None });
    assert_eq!(m.view().audio.latency.used, LatencySource::Measured);
}

#[test]
fn a_measurement_that_cannot_be_trusted_says_why() {
    let (mut m, _) = with_vocal(2);
    cmd(&mut m, Command::OpenLatencyMeasure);
    let measure = |m: &Model| m.view().workspace.latency_dialog.map(|d| d.measure);

    cmd(&mut m, Command::StartLatencyMeasure);
    for _ in 0..5 {
        m.handle(Input::LatencyAnswer { x_us: None });
    }
    assert_eq!(
        measure(&m),
        Some(LatencyMeasure::Failed {
            reason: LatencyFailure::TooQuiet
        })
    );
    assert!(cmd(&mut m, Command::UseMeasuredLatency).is_empty());

    cmd(&mut m, Command::StartLatencyMeasure);
    for x in [1_000.0, 1_000.0, 1_100.0, 1_000.0, 900.0] {
        m.handle(Input::LatencyAnswer { x_us: Some(x) });
    }
    assert_eq!(
        measure(&m),
        Some(LatencyMeasure::Failed {
            reason: LatencyFailure::Unsteady
        })
    );

    // 「やめる」 while measuring, and closing while measuring.
    cmd(&mut m, Command::StartLatencyMeasure);
    m.handle(Input::LatencyAnswer { x_us: Some(1.0) });
    assert_eq!(
        cmd(&mut m, Command::StopLatencyMeasure),
        [Effect::LatencyListen(false)]
    );
    assert_eq!(measure(&m), Some(LatencyMeasure::Idle));
    assert!(
        m.handle(Input::LatencyAnswer { x_us: Some(1.0) })
            .is_empty(),
        "a late answer"
    );
    cmd(&mut m, Command::StartLatencyMeasure);
    assert_eq!(
        cmd(&mut m, Command::CloseLatencyMeasure),
        [Effect::LatencyListen(false)]
    );
    assert_eq!(measure(&m), None);

    // The dialog alone opens the input; with none there is nothing to measure with.
    let mut m = running(vec![]);
    cmd(&mut m, Command::OpenLatencyMeasure);
    cmd(&mut m, Command::StartLatencyMeasure);
    assert_eq!(
        measure(&m),
        Some(LatencyMeasure::Failed {
            reason: LatencyFailure::NoInput
        })
    );
    let mut m = running(vec![mic(1)]);
    let effects = cmd(&mut m, Command::OpenLatencyMeasure);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::OpenInput { .. })),
        "{effects:?}"
    );
    let effects = cmd(&mut m, Command::CloseLatencyMeasure);
    assert!(effects.contains(&Effect::CloseInput));
}

#[test]
fn the_monitor_of_a_sink_can_stand_in_for_the_input() {
    let mut m = Model::new(Settings {
        output: Some(OUT.into()),
        ..Settings::default()
    })
    .with_input_stand_in("some-sink");
    m.handle(devices(vec![]));
    m.handle(Input::AudioOpened {
        node_name: OUT_NODE.into(),
        sample_rate: 48_000,
    });
    let effects = cmd(&mut m, Command::AddVocalTrack);
    assert!(effects.contains(&Effect::OpenInput {
        node_name: "some-sink".into(),
        sink_monitor: true
    }));
    assert_eq!(m.view().inputs.chosen.as_deref(), Some("monitor:some-sink"));
    // It fails to open: not tried again until the devices change.
    let effects = m.handle(Input::InputOpenFailed {
        node_name: "some-sink".into(),
    });
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, Effect::OpenInput { .. }))
    );
    assert!(!m.view().inputs.open);
    let effects = m.handle(devices(vec![]));
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, Effect::OpenInput { .. }))
    );
}

#[test]
fn what_a_recording_skipped_is_told_while_it_goes_on_and_after() {
    let (mut m, _) = with_vocal(1);
    record_vocals(&mut m);
    let view = m.view();
    assert_eq!(view.transport.recording_faults, None);
    // The file's number, its rate and the frame sung where the clip starts: what the
    // screen draws the growing wave with.
    let next_id = m.play.song().doc.next_id;
    let rec = &view.transport.recording_vocals[0];
    assert_eq!(
        (rec.audio, rec.sample_rate, rec.first_frame),
        (next_id, 48_000, Some(12_000))
    );

    m.handle(Input::VocalFaults {
        gaps: 2,
        dropped: 0,
    });
    let faults = |m: &Model| m.view().transport.recording_faults;
    assert_eq!(
        faults(&m),
        Some(meloet_protocol::RecordingFaults {
            gaps: 2,
            dropped: 0
        })
    );
    cmd(&mut m, Command::PlayStop);
    m.handle(Input::Record(RecordReport::Stopped {
        t_us: 12,
        tick: 4.0 * Q as f64,
        lap: 0,
    }));
    m.handle(Input::VocalsWritten(Box::new(Written {
        frames: 140_000,
        sample_rate: 48_000,
        gaps: 700,
        gap_count: 3,
        dropped: 256,
        dropped_blocks: 1,
        ..Written::default()
    })));
    // Of the last recording, until the next one starts.
    assert_eq!(
        faults(&m),
        Some(meloet_protocol::RecordingFaults {
            gaps: 3,
            dropped: 1
        })
    );
    m.handle(Input::VocalFaults {
        gaps: 9,
        dropped: 9,
    });
    assert_eq!(faults(&m).map(|f| f.gaps), Some(3), "nothing is recorded");
    record_vocals(&mut m);
    assert_eq!(faults(&m), None);
}

#[test]
fn a_voice_recorded_when_the_output_goes_away_says_the_take_it_left() {
    let (mut m, id) = with_vocal(1);
    cmd(&mut m, Command::DeleteTrack { track: 1 });
    record_vocals(&mut m);
    m.set_now(
        2_000_000,
        Some(crate::play::Heard {
            tick: 2.0 * Q as f64,
            state: meloet_protocol::TransportState::Recording,
            lap: 0,
        }),
    );
    // The output is pulled out.
    let effects = m.handle(Input::AudioLost);
    assert!(effects.contains(&Effect::StopVocalRecording), "{effects:?}");
    assert!(
        !matches!(
            status(&m),
            Some(StatusMessage::RecordingStoppedOutputMissing { .. })
        ),
        "the take is not in the song yet"
    );
    // The disk writer has written what it held: the take is in, and it is said.
    m.handle(written(100_000));
    let doc = &m.play.song().doc;
    assert_eq!(doc.track(TrackId(id)).unwrap().clips.len(), 1);
    assert_eq!(
        status(&m),
        Some(StatusMessage::RecordingStoppedOutputMissing {
            output: OUT.into(),
            take: 1
        })
    );
}

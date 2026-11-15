use std::sync::mpsc::{Receiver, channel};

use meloet_devices::{Availability, DevicesPicture, Endpoint, LinkKind};
use meloet_midi::Keyboard;
use meloet_protocol::{Band, ServerMessage, StatusMessage};

use super::*;

fn keyboard() -> Keyboard {
    Keyboard {
        id: "Keystation 49 MK3:MIDI 1".into(),
        name: "Keystation 49 MK3".into(),
        port_name: "MIDI 1".into(),
        client: 28,
        port: 0,
    }
}

fn subscribe(core: &Core) -> (Receiver<ServerMessage>, Receiver<Vec<u8>>) {
    let (mtx, mrx) = channel();
    let (ttx, trx) = channel();
    core.subscribe(
        Box::new(move |m| mtx.send(m).is_ok()),
        Box::new(move |b| ttx.send(b.to_vec()).is_ok()),
    );
    (mrx, trx)
}

fn patches(messages: &Receiver<ServerMessage>) -> Vec<meloet_protocol::Patch> {
    messages
        .try_iter()
        .map(|m| match m {
            ServerMessage::Patch(patch) => patch,
            other => panic!("unexpected {other:?}"),
        })
        .collect()
}

#[test]
fn a_keyboard_that_was_there_before_the_screen_is_in_the_snapshot() {
    let core = Core::start(CoreOptions::offline());
    // What the MIDI thread reports at start: Added for what is there, then Ready (which
    // carries nothing for the state).
    core.input(Input::KeyboardAdded(keyboard()));

    let (messages, _telemetry) = subscribe(&core);
    let snapshot = core.snapshot();
    assert_eq!(snapshot.state.keyboards.list.len(), 1);
    assert_eq!(snapshot.state.keyboards.list[0].name, "Keystation 49 MK3");
    assert_eq!(
        snapshot.state.keyboards.active.as_deref(),
        Some("Keystation 49 MK3:MIDI 1")
    );
    assert_eq!(
        snapshot.state.status.message,
        Some(StatusMessage::KeyboardFound {
            keyboard: "Keystation 49 MK3".into()
        })
    );
    assert!(snapshot.state.workspace.chooser.is_some());
    assert!(patches(&messages).is_empty(), "nothing after the snapshot");
}

#[test]
fn rev_goes_up_by_one_per_patch_and_nothing_goes_out_when_nothing_changed() {
    let core = Core::start(CoreOptions::offline());
    let (messages, _telemetry) = subscribe(&core);
    let before = core.snapshot();
    assert_eq!(before.rev, 1);

    let toggle = Command::ToggleBand {
        band: Band::PianoRoll,
    };
    assert_eq!(core.send(toggle.clone()), 2);
    assert_eq!(core.send(Command::SetScale { percent: 100 }), 2);
    assert_eq!(core.send(Command::SetScale { percent: 110 }), 2);
    assert_eq!(core.send(Command::SettingsApply), 2);
    assert_eq!(core.send(Command::DismissHint), 2);
    assert_eq!(core.send(toggle), 3);
    assert_eq!(core.input(Input::KeyboardAdded(keyboard())), 4);

    let got = patches(&messages);
    assert_eq!(got.iter().map(|p| p.rev).collect::<Vec<_>>(), [2, 3, 4]);
    let mut state = before.state;
    for change in got.into_iter().flat_map(|p| p.changes) {
        state.apply(change);
    }
    let after = core.snapshot();
    assert_eq!(after.rev, 4);
    assert_eq!(state, after.state);
}

#[test]
fn a_sink_whose_receiver_is_gone_is_dropped() {
    let core = Core::start(CoreOptions::offline());
    let (messages, telemetry) = subscribe(&core);
    drop((messages, telemetry));
    let (messages, _telemetry) = subscribe(&core);
    core.send(Command::GoArrange);
    core.send(Command::ToggleBand { band: Band::Mixer });
    assert_eq!(patches(&messages).len(), 1);
}

#[test]
fn subscribing_while_the_state_changes_misses_nothing() {
    const CHANGES: u64 = 400;
    for _ in 0..20 {
        let core = Arc::new(Core::start(CoreOptions::offline()));
        let writer = {
            let core = core.clone();
            std::thread::spawn(move || {
                for i in 0..CHANGES {
                    let band = if i % 2 == 0 {
                        Band::Shelf
                    } else {
                        Band::Params
                    };
                    core.send(Command::ToggleBand { band });
                }
            })
        };
        // Somewhere in the middle of the writer's work.
        while core.snapshot().rev < 20 {
            std::thread::yield_now();
        }
        let (messages, _telemetry) = subscribe(&core);
        let snapshot = core.snapshot();
        writer.join().unwrap();

        // What the screen does: drop patches up to the snapshot's rev, apply the rest in order.
        let mut rev = snapshot.rev;
        let mut state = snapshot.state;
        for patch in patches(&messages) {
            if patch.rev <= rev {
                continue;
            }
            assert_eq!(patch.rev, rev + 1, "a patch is missing");
            rev = patch.rev;
            for change in patch.changes {
                state.apply(change);
            }
        }
        let end = core.snapshot();
        assert_eq!(rev, end.rev);
        assert_eq!(state, end.state);
    }
}

#[test]
fn the_chosen_output_is_saved_and_read_at_the_next_start() {
    let dir = crate::settings::temp_dir("core");
    let options = CoreOptions {
        config_dir: Some(dir.clone()),
        ..CoreOptions::offline()
    };
    let picture = DevicesPicture {
        outputs: vec![Endpoint {
            id: "card:analog-output-headphones".into(),
            name: "アナログヘッドフォン".into(),
            friendly: None,
            availability: Availability::Yes,
            link: LinkKind::Jack,
            active: true,
            node_name: Some("alsa_output.card.analog-stereo".into()),
            device_name: Some("card".into()),
            device_description: Some("Card".into()),
        }],
        ..DevicesPicture::default()
    };

    let core = Core::start(options.clone());
    core.input(Input::KeyboardAdded(keyboard()));
    core.input(Input::Devices(picture));
    core.send(Command::SetScale { percent: 125 });
    core.send(Command::ChooserConfirm);
    let state = core.snapshot().state;
    assert_eq!(
        state.outputs.chosen.as_deref(),
        Some("card:analog-output-headphones")
    );
    // Audio is off in this test: the open fails and the state says so.
    assert!(!state.audio.running);
    core.stop();
    core.stop();

    assert_eq!(
        settings::load(&dir),
        Settings {
            output: Some("card:analog-output-headphones".into()),
            output_name: Some("アナログヘッドフォン".into()),
            scale: 125,
            ..Settings::default()
        }
    );
    let core = Core::start(options);
    let state = core.snapshot().state;
    assert_eq!(state.workspace.scale, 125);
    assert_eq!(
        state.outputs.chosen.as_deref(),
        Some("card:analog-output-headphones")
    );
    core.input(Input::KeyboardAdded(keyboard()));
    assert_eq!(core.snapshot().state.workspace.chooser, None);
}

#[test]
fn a_remembered_scale_that_does_not_fit_the_screen_is_lowered_and_saved() {
    let dir = crate::settings::temp_dir("core-screen");
    let options = CoreOptions {
        config_dir: Some(dir.clone()),
        ..CoreOptions::offline()
    };
    let core = Core::start(options.clone());
    core.send(Command::SetScale { percent: 150 });
    core.stop();

    let core = Core::start(options);
    assert_eq!(core.snapshot().state.workspace.scale, 150);
    core.set_screen_space(1200.0, 700.0);
    // `set_screen_space` does not wait; a command that changes nothing does.
    core.send(Command::DismissHint);
    let state = core.snapshot().state;
    assert_eq!(state.workspace.scale, 125);
    assert_eq!(state.workspace.unfit_scales, [150]);
    // Sending a command took the notice away again.
    assert_eq!(
        state.status.message,
        Some(meloet_protocol::StatusMessage::SearchingKeyboard)
    );
    core.stop();
    assert_eq!(settings::load(&dir).scale, 125);
}

#[test]
fn telemetry_goes_once_to_a_new_sink_then_stops_until_something_changes() {
    let core = Core::start(CoreOptions::offline());
    let (_messages, telemetry) = subscribe(&core);

    let first = telemetry.recv_timeout(Duration::from_secs(2)).unwrap();
    let first = Telemetry::decode(&first).unwrap();
    assert_eq!(first.track_peaks.len(), 1);
    assert!(!first.running);
    assert!(
        telemetry.recv_timeout(Duration::from_millis(200)).is_err(),
        "nothing changed, so nothing more is sent"
    );

    let held = &core.shared.held;
    held.note(MidiEvent::NoteOn {
        note: 60,
        velocity: 90,
    });
    held.note(MidiEvent::NoteOn {
        note: 100,
        velocity: 70,
    });
    held.note(MidiEvent::Control {
        controller: 64,
        value: 127,
    });
    held.note(MidiEvent::PitchBend { value: -100 });
    let next = loop {
        let bytes = telemetry.recv_timeout(Duration::from_secs(2)).unwrap();
        let t = Telemetry::decode(&bytes).unwrap();
        if t.pitch_bend == -100 {
            break t;
        }
    };
    assert_eq!(next.keys_low, 1 << 60);
    assert_eq!(next.keys_high, 1 << (100 - 64));
    assert_eq!(next.last_velocity, 70);
    assert_eq!(next.sustain, 127);
    assert!(next.seq > first.seq);

    held.note(MidiEvent::NoteOff {
        note: 60,
        velocity: 0,
    });
    let bytes = telemetry.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(Telemetry::decode(&bytes).unwrap().keys_low, 0);
}

#[test]
fn midi_events_become_engine_and_protocol_events() {
    assert_eq!(
        instrument_event(MidiEvent::NoteOff {
            note: 60,
            velocity: 64
        }),
        InstrumentEvent::NoteOff { note: 60 }
    );
    assert_eq!(
        protocol_event(
            MidiEvent::NoteOn {
                note: 60,
                velocity: 99
            },
            7
        ),
        Event::Note {
            on: true,
            note: 60,
            velocity: 99,
            t_us: 7
        }
    );
    assert_eq!(
        protocol_event(MidiEvent::PitchBend { value: -1 }, 9),
        Event::Bend { value: -1, t_us: 9 }
    );
}

#[test]
fn key_to_sound_ignores_keys_that_waited_for_a_stream() {
    let record = |key_us, playback_us| LatencyRecord {
        key_us,
        callback_us: 0,
        playback_us,
    };
    assert_eq!(key_to_sound_ms(record(1_000, 14_500)), Some(13.5));
    assert_eq!(key_to_sound_ms(record(14_500, 1_000)), None);
    assert_eq!(key_to_sound_ms(record(0, 5_000_000)), None);
}

#[test]
fn the_load_holds_still_for_small_changes() {
    let sent = |load_permille| Telemetry {
        load_permille,
        ..Telemetry::default()
    };
    assert_eq!(steady_load(false, 500, Some(&sent(40))), 0);
    assert_eq!(steady_load(true, 37, None), 37);
    assert_eq!(steady_load(true, 45, Some(&sent(40))), 40);
    assert_eq!(steady_load(true, 31, Some(&sent(40))), 40);
    assert_eq!(steady_load(true, 50, Some(&sent(40))), 50);
    assert_eq!(steady_load(true, 1_000_000, Some(&sent(40))), u16::MAX);
}

/// Needs the Keystation 49 MK3 and the PC's headphone output.
#[test]
#[ignore = "needs the keyboard and PipeWire"]
fn the_real_keyboard_and_headphones_show_up_within_two_seconds() {
    let dir = crate::settings::temp_dir("devices");
    let core = Core::start(CoreOptions {
        config_dir: Some(dir),
        measure_dir: None,
        midi: Some(MidiOptions::default()),
        devices: true,
        audio: true,
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let state = core.snapshot().state;
        let keyboard = state
            .keyboards
            .list
            .iter()
            .any(|k| k.name == "Keystation 49 MK3");
        let headphones = state
            .outputs
            .list
            .iter()
            .any(|e| e.name == "アナログヘッドフォン");
        if keyboard && headphones {
            eprintln!("{:#?}", state.outputs);
            eprintln!("{:?}", state.keyboards);
            assert!(state.workspace.chooser.is_some());
            break;
        }
        assert!(
            Instant::now() < deadline,
            "keyboards {:?}, outputs {:?}",
            state.keyboards.list,
            state.outputs.list
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Needs the PC's headphone output to be plugged in. Opens the stream but plays nothing.
#[test]
#[ignore = "needs PipeWire and the headphone output"]
fn the_remembered_headphones_open_and_silence_sends_no_telemetry() {
    const HEADPHONES: &str = "alsa_card.pci-0000_0c_00.6:analog-output-headphones";
    let dir = crate::settings::temp_dir("open");
    let remembered = Settings {
        output: Some(HEADPHONES.into()),
        output_name: None,
        scale: 100,
        ..Settings::default()
    };
    settings::save(&dir, &remembered).unwrap();
    let core = Core::start(CoreOptions {
        config_dir: Some(dir),
        measure_dir: None,
        midi: None,
        devices: true,
        audio: true,
    });
    let (_messages, telemetry) = subscribe(&core);

    let deadline = Instant::now() + Duration::from_secs(3);
    let audio = loop {
        let audio = core.snapshot().state.audio;
        if audio.actual_frames.is_some() {
            break audio;
        }
        assert!(Instant::now() < deadline, "{:?}", core.snapshot().state);
        std::thread::sleep(Duration::from_millis(20));
    };
    eprintln!("{audio:?}");
    assert!(audio.running);
    assert_eq!(audio.sample_rate, Some(48_000));
    assert_eq!(
        audio.output_device.as_deref(),
        Some("alsa_output.pci-0000_0c_00.6.analog-stereo")
    );
    assert!(audio.latency_ms.is_some() && audio.latency_ms_at_requested.is_some());

    // The first readings of the running stream arrive, then silence sends nothing.
    let mut running = false;
    while let Ok(bytes) = telemetry.recv_timeout(Duration::from_millis(500)) {
        running = Telemetry::decode(&bytes).unwrap().running;
    }
    assert!(running);
    assert!(telemetry.recv_timeout(Duration::from_millis(500)).is_err());

    core.stop();
    assert!(!core.shared.stream.open.load(Relaxed));
}

/// Runs against the real ALSA sequencer; where it cannot be opened, nothing is checked.
#[test]
fn the_keyboard_played_last_becomes_the_active_one() {
    use meloet_midi::vkeys::VirtualKeyboard;

    fn wait_for_active(core: &Core, name: &str) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let keyboards = core.snapshot().state.keyboards;
            let active = keyboards
                .list
                .iter()
                .find(|k| Some(&k.id) == keyboards.active.as_ref());
            if active.is_some_and(|k| k.name == name) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "{name} is not active: {keyboards:?}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    let name = |tag: &str| format!("meloet-vkeys-core-{tag}-{}", std::process::id());
    let (first, second) = (name("a"), name("b"));
    let Ok(keys_a) = VirtualKeyboard::open(&first) else {
        eprintln!("skipped: the ALSA sequencer cannot be opened here");
        return;
    };
    let core = Core::start(CoreOptions {
        midi: Some(MidiOptions {
            accept_virtual: true,
            only_virtual: true,
            fail_after_events: None,
        }),
        ..CoreOptions::offline()
    });
    wait_for_active(&core, &first);
    let keys_b = VirtualKeyboard::open(&second).unwrap();
    wait_for_active(&core, &second);

    let key = |on| {
        if on {
            MidiEvent::NoteOn {
                note: 60,
                velocity: 90,
            }
        } else {
            MidiEvent::NoteOff {
                note: 60,
                velocity: 0,
            }
        }
    };
    keys_a.send(key(true)).unwrap();
    wait_for_active(&core, &first);
    assert_eq!(core.snapshot().state.keyboards.list.len(), 2);

    // Letting go, or a pedal, does not count as playing.
    keys_b.send(key(false)).unwrap();
    keys_b
        .send(MidiEvent::Control {
            controller: 64,
            value: 127,
        })
        .unwrap();
    keys_a.send(key(false)).unwrap();
    keys_b.send(key(true)).unwrap();
    wait_for_active(&core, &second);

    // Pulled and plugged in again, then the other one plugged in again: a key on the first
    // must still be noticed, though it is the same keyboard as the one played before.
    drop(keys_b);
    wait_for_active(&core, &first);
    let keys_b = VirtualKeyboard::open(&second).unwrap();
    wait_for_active(&core, &second);
    keys_a.send(key(true)).unwrap();
    wait_for_active(&core, &first);
    drop(keys_a);
    wait_for_active(&core, &second);
    let keys_a = VirtualKeyboard::open(&first).unwrap();
    wait_for_active(&core, &first);
    keys_b.send(key(true)).unwrap();
    wait_for_active(&core, &second);
    drop(keys_a);
    core.stop();
}

/// Runs against the real ALSA sequencer; where it cannot be opened, nothing is checked. The
/// MIDI thread is made to fail after two key events, again after every start.
#[test]
fn a_stopped_keyboard_input_releases_the_keys_and_starts_again() {
    use meloet_midi::vkeys::VirtualKeyboard;

    fn wait(core: &Core, what: &str, mut done: impl FnMut(&meloet_protocol::AppState) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let state = core.snapshot().state;
            if done(&state) {
                return;
            }
            assert!(Instant::now() < deadline, "{what}: {state:?}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    let name = format!("meloet-vkeys-core-fail-{}", std::process::id());
    let Ok(keys) = VirtualKeyboard::open(&name) else {
        eprintln!("skipped: the ALSA sequencer cannot be opened here");
        return;
    };
    let core = Core::start(CoreOptions {
        midi: Some(MidiOptions {
            accept_virtual: true,
            only_virtual: true,
            fail_after_events: Some(2),
        }),
        ..CoreOptions::offline()
    });
    let present = |s: &meloet_protocol::AppState| s.keyboards.list.iter().any(|k| k.name == name);
    wait(&core, "the keyboard is there", present);

    for round in 0..2 {
        keys.send(MidiEvent::NoteOn {
            note: 60,
            velocity: 90,
        })
        .unwrap();
        keys.send(MidiEvent::Control {
            controller: 64,
            value: 127,
        })
        .unwrap();
        let stopped_at = Instant::now();
        wait(&core, "the input is said to have stopped", |s| {
            s.keyboards.input_stopped
                && s.status.message == Some(StatusMessage::KeyboardInputStopped)
        });
        let held = &core.shared.held;
        assert_eq!(
            held.keys_low.load(Relaxed),
            0,
            "round {round}: the key is released"
        );
        assert_eq!(
            held.sustain.load(Relaxed),
            0,
            "round {round}: the pedal is released"
        );

        wait(&core, "the input runs again", |s| {
            !s.keyboards.input_stopped && present(s)
        });
        let took = stopped_at.elapsed();
        assert!(
            took >= Duration::from_millis(400) && took < Duration::from_millis(1500),
            "round {round}: started again after {took:?}"
        );
        assert!(matches!(
            core.snapshot().state.status.message,
            Some(StatusMessage::KeyboardFound { .. })
        ));
    }
    core.stop();
}

#[test]
fn notes_of_takes_and_of_the_recording_are_asked_on_the_core_thread() {
    let core = Core::start(CoreOptions::offline());
    let live = core.live_notes();
    assert_eq!((live.seq, live.recording), (0, None));
    assert!(live.notes.is_empty());
    assert_eq!(core.take_notes(1), None);
    // Without a stream nothing plays or records.
    core.send(Command::PlayStop);
    core.send(Command::Record);
    let t = core.snapshot().state.transport;
    assert_eq!(t.state, meloet_protocol::TransportState::Stopped);
    assert!(t.recording.is_none());
}

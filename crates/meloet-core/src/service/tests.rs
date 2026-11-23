use crate::seen::*;
use std::sync::mpsc::{Receiver, channel};

use meloet_devices::{Availability, DevicesPicture, Endpoint, LinkKind};
use meloet_engine::BlockTiming;
use meloet_midi::Keyboard;
use meloet_protocol::{Band, InstrumentKind, ServerMessage, ShelfMode, StatusMessage, Theme};

use super::*;
use crate::instruments::Samples;

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
            channels: None,
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
fn the_chosen_theme_is_kept_and_read_at_the_next_start() {
    let dir = crate::settings::temp_dir("core-theme");
    let options = CoreOptions {
        config_dir: Some(dir.clone()),
        ..CoreOptions::offline()
    };
    let core = Core::start(options.clone());
    assert_eq!(core.snapshot().state.workspace.theme, Theme::Dark);
    core.send(Command::SetTheme {
        theme: Theme::PinkLight,
    });
    let state = core.snapshot().state;
    assert_eq!(state.workspace.theme, Theme::PinkLight);
    assert_eq!(state.song.undo_count, 0, "not a step");
    core.stop();
    assert_eq!(settings::load(&dir).theme, Theme::PinkLight);

    let core = Core::start(options);
    assert_eq!(core.snapshot().state.workspace.theme, Theme::PinkLight);
    core.stop();
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
        songs_dir: None,
        state_dir: None,
        sounds_dir: None,
        pack: None,
        updates: None,
        plugins: None,
        sf2_dir: None,
        cache_dir: None,
        pc_language: meloet_protocol::Language::Ja,
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
        songs_dir: None,
        state_dir: None,
        sounds_dir: None,
        pack: None,
        updates: None,
        plugins: None,
        sf2_dir: None,
        cache_dir: None,
        pc_language: meloet_protocol::Language::Ja,
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
            virtual_names: vec![first.clone(), second.clone()],
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
            virtual_names: vec![name.clone()],
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

#[test]
fn the_fixed_velocity_is_saved_and_back_at_the_next_start() {
    let dir = crate::settings::temp_dir("core-fixed");
    let options = CoreOptions {
        config_dir: Some(dir.clone()),
        ..CoreOptions::offline()
    };
    let core = Core::start(options.clone());
    assert_eq!(core.shared.fixed_velocity.load(Relaxed), 0);
    core.send(Command::SetVelocityFixed {
        on: true,
        value: 200,
    });
    let fixed = core.snapshot().state.keyboards.velocity_fixed;
    assert_eq!((fixed.on, fixed.value), (true, 127), "1..=127");
    assert_eq!(core.shared.fixed_velocity.load(Relaxed), 127);
    core.stop();

    let core = Core::start(options.clone());
    let fixed = core.snapshot().state.keyboards.velocity_fixed;
    assert_eq!((fixed.on, fixed.value), (true, 127));
    assert_eq!(
        core.shared.fixed_velocity.load(Relaxed),
        127,
        "from the start"
    );
    core.send(Command::SetVelocityFixed {
        on: false,
        value: 0,
    });
    let fixed = core.snapshot().state.keyboards.velocity_fixed;
    assert_eq!((fixed.on, fixed.value), (false, 1));
    assert_eq!(core.shared.fixed_velocity.load(Relaxed), 0);
    core.stop();
    let loaded = settings::load(&dir);
    assert_eq!((loaded.velocity_fixed, loaded.fixed_velocity), (false, 1));
}

#[test]
fn the_fader_of_a_keyboard_is_one_step_when_it_rests_on_the_core_thread() {
    let core = Core::start(CoreOptions::offline());
    let level = |core: &Core| core.snapshot().state.song.tracks[0].gain_db;
    assert_eq!(level(&core), Some(0.0));
    // As the MIDI thread does it: the newest value, then the message.
    core.shared.fader.store(102, Relaxed);
    core.tx.send(Msg::Fader).unwrap();
    // A round trip to the core thread: 102 has been read.
    core.history_rows(1, 1);
    core.shared.fader.store(90, Relaxed);
    core.tx.send(Msg::Fader).unwrap();
    assert_eq!(core.snapshot().state.song.undo_count, 0);
    let deadline = Instant::now() + Duration::from_secs(3);
    while core.snapshot().state.song.undo_count == 0 {
        assert!(Instant::now() < deadline, "no step after the fader rested");
        std::thread::sleep(Duration::from_millis(20));
    }
    let want = meloet_model::Gain::from_position(90.0 / 127.0).db();
    assert_eq!(level(&core), want.map(|db| db as f32));
    assert_eq!(core.shared.fader.load(Relaxed), NO_FADER);
    core.stop();
}

/// Writes down what the engine plays on it.
struct Tape(Sender<InstrumentEvent>);

impl meloet_engine::Instrument for Tape {
    fn prepare(&mut self, _: f32, _: usize) {}
    fn event(&mut self, ev: InstrumentEvent) {
        let _ = self.0.send(ev);
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        left.fill(0.0);
        right.fill(0.0);
    }
    fn reset(&mut self) {}
}

#[test]
fn a_fixed_velocity_reaches_the_sound_the_recording_the_meter_and_the_screen() {
    let hub = Arc::new(Hub::new(Model::new(Settings::default()).view()));
    let (screen_tx, screen) = channel();
    hub.subscribe(
        Box::new(move |m| screen_tx.send(m).is_ok()),
        Box::new(|_| true),
    );
    let shared = Arc::new(Shared {
        held: Held::default(),
        fixed_velocity: AtomicU8::new(90),
        stream: StreamFacts::default(),
        stop: AtomicBool::new(false),
        clock: Mutex::new(Arc::new(TransportClock::default())),
        tempo_map: Mutex::new((
            Arc::new(TempoMap::new(&meloet_model::Timeline::new(
                meloet_model::Tempo::from_centi(12_000).unwrap(),
                meloet_model::Meter::FOUR_FOUR,
            ))),
            0,
        )),
        shell: Mutex::new(None),
        fader: AtomicU16::new(NO_FADER),
        mics: Mutex::new(None),
        typed_played: AtomicBool::new(false),
    });
    shared.stream.open.store(true, Relaxed);
    let (tx, _rx) = mpsc::channel();
    let links = Links {
        hub: hub.clone(),
        shared: shared.clone(),
        tx,
    };
    let mut parts = Engine::new(EngineConfig::default());
    let (tape_tx, tape) = channel();
    assert!(
        parts
            .handle
            .swap_instrument(0, Box::new(Tape(tape_tx)))
            .is_ok()
    );
    assert!(
        parts
            .handle
            .send(EngineCommand::Record {
                from: 0,
                count_in_beats: 0,
                muted: TrackSet::default(),
            })
            .is_ok()
    );
    let mut out = vec![0.0f32; 512];
    let mut block = |engine: &mut Engine, at_us: u64| {
        engine.process(
            &mut out,
            BlockTiming {
                callback_us: at_us,
                playback_us: at_us,
            },
        );
    };
    block(&mut parts.engine, 1_000_000);
    let mut sink = links.midi_sink(parts.midi_in);
    for event in [
        MidiEvent::NoteOn {
            note: 60,
            velocity: 30,
        },
        MidiEvent::NoteOff {
            note: 60,
            velocity: 12,
        },
    ] {
        sink(MidiMessage::Event {
            keyboard: "kb".into(),
            event,
            t_us: 1_002_000,
        });
    }
    block(&mut parts.engine, 1_005_333);

    let played: Vec<_> = tape.try_iter().collect();
    assert_eq!(
        played,
        [
            InstrumentEvent::NoteOn {
                note: 60,
                velocity: 90
            },
            InstrumentEvent::NoteOff { note: 60 }
        ]
    );
    let recorded: Vec<_> = std::iter::from_fn(|| parts.handle.pop_recorded())
        .filter_map(|r| match r {
            Recorded::Event { event, .. } => Some(event),
            _ => None,
        })
        .collect();
    assert_eq!(recorded, played, "recorded as played");
    assert_eq!(shared.held.last_velocity.load(Relaxed), 90, "the meter");
    let notes: Vec<_> = screen
        .try_iter()
        .filter_map(|m| match m {
            ServerMessage::Event(Event::Note { on, velocity, .. }) => Some((on, velocity)),
            _ => None,
        })
        .collect();
    assert_eq!(notes, [(true, 90), (false, 12)], "NoteOff as it came");
}

#[test]
fn buttons_and_the_fader_never_sound_and_all_notes_off_lets_go_of_that_keyboard() {
    let hub = Arc::new(Hub::new(Model::new(Settings::default()).view()));
    let (screen_tx, screen) = channel();
    hub.subscribe(
        Box::new(move |m| screen_tx.send(m).is_ok()),
        Box::new(|_| true),
    );
    let shared = Arc::new(Shared {
        held: Held::default(),
        fixed_velocity: AtomicU8::new(0),
        stream: StreamFacts::default(),
        stop: AtomicBool::new(false),
        clock: Mutex::new(Arc::new(TransportClock::default())),
        tempo_map: Mutex::new((
            Arc::new(TempoMap::new(&meloet_model::Timeline::new(
                meloet_model::Tempo::from_centi(12_000).unwrap(),
                meloet_model::Meter::FOUR_FOUR,
            ))),
            0,
        )),
        shell: Mutex::new(None),
        fader: AtomicU16::new(NO_FADER),
        mics: Mutex::new(None),
        typed_played: AtomicBool::new(false),
    });
    shared.stream.open.store(true, Relaxed);
    let (tx, core) = mpsc::channel();
    let links = Links {
        hub: hub.clone(),
        shared: shared.clone(),
        tx,
    };
    let mut parts = Engine::new(EngineConfig::default());
    let (tape_tx, tape) = channel();
    assert!(
        parts
            .handle
            .swap_instrument(0, Box::new(Tape(tape_tx)))
            .is_ok()
    );
    let record = EngineCommand::Record {
        from: 0,
        count_in_beats: 0,
        muted: TrackSet::default(),
    };
    assert!(parts.handle.send(record).is_ok());
    let mut out = vec![0.0f32; 512];
    let mut block = |engine: &mut Engine, at_us: u64| {
        let timing = BlockTiming {
            callback_us: at_us,
            playback_us: at_us,
        };
        engine.process(&mut out, timing);
    };
    block(&mut parts.engine, 1_000_000);
    let mut sink = links.midi_sink(parts.midi_in);
    let key = |keyboard: &str, event| MidiMessage::Event {
        keyboard: keyboard.into(),
        event,
        t_us: 1_002_000,
    };
    let control = |controller, value| MidiEvent::Control { controller, value };
    let on = |note| MidiEvent::NoteOn { note, velocity: 80 };
    sink(MidiMessage::Button {
        keyboard: "a".into(),
        button: meloet_midi::Button::Play,
    });
    sink(key("a", on(60)));
    sink(key("b", on(62)));
    sink(key("a", control(64, 127)));
    sink(key("a", control(7, 30)));
    sink(key("a", control(7, 31)));
    sink(key("a", control(123, 0)));
    sink(key("a", control(121, 0)));
    block(&mut parts.engine, 1_005_333);

    let played: Vec<_> = tape.try_iter().collect();
    assert_eq!(
        played,
        [
            InstrumentEvent::NoteOn {
                note: 60,
                velocity: 80
            },
            InstrumentEvent::NoteOn {
                note: 62,
                velocity: 80
            },
            InstrumentEvent::Control {
                controller: 64,
                value: 127
            },
            InstrumentEvent::NoteOff { note: 60 },
            InstrumentEvent::Control {
                controller: 64,
                value: 0
            },
        ],
        "no button, no fader, no CC 123 or 121; only what keyboard a held is let go"
    );
    let recorded: Vec<_> = std::iter::from_fn(|| parts.handle.pop_recorded())
        .filter_map(|r| match r {
            Recorded::Event { event, .. } => Some(event),
            _ => None,
        })
        .collect();
    assert_eq!(
        recorded, played,
        "the letting go is recorded, the controllers are not"
    );
    assert!(
        screen.try_iter().all(|m| !matches!(
            m,
            ServerMessage::Event(Event::Control {
                controller: 7 | 121 | 123,
                ..
            })
        )),
        "the screen hears no fader or reset"
    );

    let to_core: Vec<_> = core.try_iter().collect();
    assert!(matches!(
        to_core[0],
        Msg::Input(Input::KeyboardButton(meloet_midi::Button::Play), None)
    ));
    let faders = to_core.iter().filter(|m| matches!(m, Msg::Fader)).count();
    assert_eq!(faders, 1, "the second move waits in the same message");
    assert_eq!(shared.fader.load(Relaxed), 31, "the newest value");
}

#[test]
fn the_meters_go_out_in_the_order_of_the_song_whatever_the_seats() {
    let mut peaks = vec![[0.0f32; 2]; AUDITION_SEAT + 1];
    peaks[0] = [0.1, 0.1];
    peaks[2] = [0.3, 0.2];
    peaks[AUDITION_SEAT] = [0.9, 0.9];
    assert_eq!(
        peaks_in_song_order(&[Some(2), None, Some(0), Some(1)], &peaks),
        [[0.3, 0.2], [0.0, 0.0], [0.1, 0.1], [0.0, 0.0]]
    );
    assert!(peaks_in_song_order(&[], &peaks).is_empty());
}

/// The real engine with no stream, fed the ops of the rules: which of its tracks sounds when a
/// key is played.
struct SeatRig {
    model: Model,
    audio: Audio,
    midi: MidiFeed,
    meters: Arc<EngineMeters>,
}

impl SeatRig {
    fn new() -> SeatRig {
        let mut model = Model::new(Settings::default());
        let parts = Engine::new(EngineConfig {
            max_tracks: MAX_TRACKS,
            ..EngineConfig::default()
        });
        let meters = parts.handle.meters().clone();
        SeatRig {
            audio: Audio::new(
                parts.engine,
                parts.handle,
                false,
                Library::open(Vec::new(), Box::new(|_, _| {})),
                Sf2Store::open(None, Box::new(|_| {})),
                model.engine_seats(),
                (256, 48_000),
            ),
            model,
            midi: parts.midi_in,
            meters,
        }
    }

    fn send(&mut self, command: Command) {
        for effect in self.model.handle(Input::Command(command)) {
            if let Effect::Engine(op) = effect {
                self.audio.op(op);
            }
        }
    }

    /// Plays a key and returns the engine's tracks that sounded.
    fn sounding(&mut self) -> Vec<usize> {
        let engine = self.audio.engine.as_mut().expect("no stream has it");
        let mut out = [0.0f32; 512];
        for event in [
            InstrumentEvent::NoteOn {
                note: 69,
                velocity: 100,
            },
            InstrumentEvent::NoteOff { note: 69 },
        ] {
            self.midi.push(MidiIn { event, t_us: 0 }).unwrap();
            engine.process(&mut out, BlockTiming::default());
        }
        // Long enough for the released note to die away before the next key.
        for _ in 0..80 {
            engine.process(&mut out, BlockTiming::default());
        }
        (0..=AUDITION_SEAT)
            .filter(|&seat| self.meters.take_track_peak(seat)[0] > 0.01)
            .collect()
    }
}

#[test]
fn keys_sound_on_the_selected_track_and_on_what_the_shelf_auditions_without_a_stream() {
    let mut rig = SeatRig::new();
    assert_eq!(rig.sounding(), [0], "the first track of a new song");

    // A row of the shelf: the audition track, then the new track's seat.
    rig.send(Command::OpenShelf {
        mode: ShelfMode::NewTrack,
    });
    rig.send(Command::ShelfSelect {
        instrument: Some(InstrumentKind::Sine),
    });
    assert_eq!(rig.sounding(), [AUDITION_SEAT]);
    rig.send(Command::ShelfCancel);
    assert_eq!(rig.sounding(), [0]);
    rig.send(Command::ShelfSelect {
        instrument: Some(InstrumentKind::Sine),
    });
    rig.send(Command::ShelfConfirm);
    assert_eq!(rig.model.track_seats(), [Some(0), Some(1)]);
    assert_eq!(rig.sounding(), [1], "the new track is selected");

    rig.send(Command::SelectTrack {
        track: 1,
        from_arm: false,
    });
    assert_eq!(rig.sounding(), [0]);
    // Trying another instrument on track 2: the keys go there, and come back.
    rig.send(Command::OpenShelf {
        mode: ShelfMode::Replace { track: 4 },
    });
    rig.send(Command::ShelfSelect {
        instrument: Some(InstrumentKind::Sine),
    });
    assert_eq!(rig.sounding(), [1]);
    rig.send(Command::Back);
    assert_eq!(rig.sounding(), [0]);

    // The selected track goes: the one left is played. With no track, nothing sounds.
    rig.send(Command::DeleteTrack { track: 1 });
    assert_eq!(rig.model.track_seats(), [Some(1)]);
    assert_eq!(rig.sounding(), [1]);
    rig.send(Command::DeleteTrack { track: 4 });
    assert_eq!(rig.sounding(), [] as [usize; 0]);
    rig.send(Command::Undo);
    assert_eq!(rig.sounding(), [0], "back on the first free seat");

    // What the engine has is what the book says, for an engine made anew.
    let book = rig.audio.seats.clone();
    assert_eq!(book.target, Some(0));
    assert_eq!(
        book.own.iter().filter(|s| s.is_some()).count(),
        1,
        "{book:?}"
    );
    assert!(book.trial.is_none());

    // The engine is lost with a stream: the next one is set up from the book.
    rig.audio.engine = None;
    rig.send(Command::Undo);
    assert_eq!(rig.model.track_seats(), [Some(1), Some(0)]);
    let (engine, feed) = rig.audio.handle.rebuild();
    rig.audio.engine = Some(engine);
    rig.midi = feed;
    for op in rig.audio.seats.ops() {
        rig.audio.send_seat_op(op);
        rig.audio.settle();
    }
    assert_eq!(rig.sounding(), [0], "track 2, still the selected one");
    rig.send(Command::SelectTrack {
        track: 1,
        from_arm: false,
    });
    assert_eq!(
        rig.sounding(),
        [1],
        "track 1 is on the engine made anew too"
    );
}

#[test]
fn the_real_engine_has_the_songs_mixer_without_a_stream_and_when_made_anew() {
    use meloet_model::effects::limiter;
    use meloet_protocol::{EffectKind, InsertRef, SendBus, StripRef};

    let mut rig = SeatRig::new();
    let meters = rig.meters.clone();
    let returns = |rig: &mut SeatRig| {
        meters.take_return_peak(0);
        meters.take_return_peak(1);
        rig.sounding();
        [meters.take_return_peak(0)[0], meters.take_return_peak(1)[0]]
    };
    // A new song: the piano sends to return A, where the reverb is.
    let [a, b] = returns(&mut rig);
    assert!(a > 0.001 && b == 0.0, "{a} {b}");
    rig.send(Command::ToggleReturnMute { bus: SendBus::A });
    assert_eq!(returns(&mut rig), [0.0, 0.0]);
    rig.send(Command::ToggleReturnMute { bus: SendBus::A });
    // A drag is heard before it is a step.
    for commit in [false, true] {
        rig.send(Command::SetSend {
            track: 1,
            bus: SendBus::B,
            db: Some(-6.0),
            commit,
        });
        let [a, b] = returns(&mut rig);
        assert!(a > 0.001 && b > 0.001, "{a} {b}");
    }

    // The master's limiter comes after its fader: +6 dB on the fader does not pass the
    // ceiling.
    let limiter = |param, value| Command::SetParam {
        insert: InsertRef {
            strip: StripRef::Master,
            slot: 0,
        },
        effect: EffectKind::Limiter,
        param,
        value,
        commit: true,
    };
    rig.send(limiter(limiter::CEILING, -12.0));
    rig.send(limiter(limiter::GAIN, 12.0));
    for strip in [StripRef::Master, StripRef::Track { id: 1 }] {
        rig.send(Command::SetGain {
            strip,
            db: Some(6.0),
            commit: true,
        });
    }
    let ceiling = 10f32.powf(-12.0 / 20.0);
    let master = |rig: &mut SeatRig| {
        meters.take_master_peak();
        rig.sounding();
        meters.take_master_peak()[0]
    };
    // A running effect takes 30 ms to get to a new value: one key for that.
    rig.sounding();
    let loud = master(&mut rig);
    assert!(loud > ceiling * 0.9 && loud <= ceiling * 1.0001, "{loud}");

    // The engine is lost with a stream: the next one gets the mixer from the book.
    rig.audio.engine = None;
    rig.send(Command::SetPan {
        strip: StripRef::Track { id: 1 },
        pan: 100,
        commit: true,
    });
    let (engine, feed) = rig.audio.handle.rebuild();
    rig.audio.engine = Some(engine);
    rig.midi = feed;
    for op in (rig.audio.mix.ops().into_iter()).chain(rig.audio.seats.ops()) {
        rig.audio.send_seat_op(op);
        rig.audio.settle();
    }
    let loud = master(&mut rig);
    assert!(loud <= ceiling * 1.0001, "{loud}");
    meters.take_return_peak(1);
    rig.sounding();
    let b = meters.take_return_peak(1);
    assert!(
        b[0] == 0.0 && b[1] > 0.001,
        "the send follows the pan: {b:?}"
    );

    // A deleted track leaves nothing in the book.
    rig.send(Command::DeleteTrack { track: 1 });
    assert!(
        !(rig.audio.mix.ops().iter()).any(|op| matches!(
            op,
            EngineOp::Sends { .. }
                | EngineOp::Strip {
                    strip: crate::play::StripSeat::Track(_),
                    ..
                }
        )),
        "{:?}",
        rig.audio.mix.ops()
    );
}

/// The real engine with no stream and a little sound pack: the rules, the library and the
/// thread that reads, joined as the core thread joins them.
struct SoundRig {
    seats: SeatRig,
    read: Receiver<(BuiltinInstrument, Result<Arc<SampleBank>, LoadError>)>,
    dir: PathBuf,
}

impl SoundRig {
    fn new(label: &str, instruments: &[BuiltinInstrument]) -> SoundRig {
        let dir = crate::sounds::tests::tiny_pack(label, instruments);
        let (tx, read) = channel();
        let done = move |instrument, bank| {
            let _ = tx.send((instrument, bank));
        };
        let library = Library::open(crate::sounds::read_pinned(&dir), Box::new(done));
        let mut model = Model::new(Settings::default()).with_sounds(library.installed());
        let parts = Engine::new(EngineConfig {
            max_tracks: MAX_TRACKS,
            ..EngineConfig::default()
        });
        let meters = parts.handle.meters().clone();
        let audio = Audio::new(
            parts.engine,
            parts.handle,
            false,
            library,
            Sf2Store::open(None, Box::new(|_| {})),
            model.engine_seats(),
            (256, 48_000),
        );
        let seats = SeatRig {
            audio,
            model,
            midi: parts.midi_in,
            meters,
        };
        SoundRig { seats, read, dir }
    }

    fn handle(&mut self, input: Input) {
        let rig = &mut self.seats;
        for effect in rig.model.handle(input) {
            match effect {
                Effect::Engine(op) => rig.audio.op(op),
                Effect::LoadSound(instrument) => rig.audio.sounds.load(instrument),
                Effect::DropSound(instrument) => rig.audio.sounds.let_go(instrument),
                _ => {}
            }
        }
    }

    fn send(&mut self, command: Command) {
        self.handle(Input::Command(command));
    }

    /// Waits for the reading thread's next answer and takes it in.
    fn read_one(&mut self) -> BuiltinInstrument {
        let (instrument, read) = self.read.recv_timeout(Duration::from_secs(10)).unwrap();
        let bank = read.unwrap();
        self.seats.audio.sounds.keep(instrument, bank);
        self.handle(Input::SoundLoaded(instrument));
        instrument
    }

    fn in_memory(&self) -> Vec<BuiltinInstrument> {
        self.seats.audio.sounds.in_memory()
    }
}

impl Drop for SoundRig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn seats_are_silent_until_their_samples_are_read_and_samples_nothing_needs_are_let_go() {
    use BuiltinInstrument as B;
    let mut rig = SoundRig::new("seats", &[B::GrandPiano, B::Strings, B::Organ]);
    assert_eq!(
        rig.seats.sounding(),
        [] as [usize; 0],
        "the piano is not read yet"
    );
    rig.handle(Input::Tick);
    assert_eq!(rig.read_one(), B::GrandPiano);
    assert_eq!(rig.seats.sounding(), [0], "the piano of the new song");
    assert_eq!(rig.in_memory(), [B::GrandPiano]);

    // A row of the shelf: silent on the audition seat, then sounding.
    rig.send(Command::OpenShelf {
        mode: ShelfMode::NewTrack,
    });
    rig.send(Command::ShelfSelect {
        instrument: Some(InstrumentKind::Strings),
    });
    assert_eq!(
        rig.seats.model.view().shelf.loading(),
        Some(InstrumentKind::Strings)
    );
    assert_eq!(rig.seats.sounding(), [] as [usize; 0]);
    assert_eq!(rig.read_one(), B::Strings);
    assert_eq!(rig.seats.model.view().shelf.loading(), None);
    assert_eq!(rig.seats.sounding(), [AUDITION_SEAT]);

    // The next row; the strings stay as the row before, and go with the row after that.
    rig.send(Command::ShelfSelect {
        instrument: Some(InstrumentKind::Organ),
    });
    assert_eq!(rig.read_one(), B::Organ);
    assert_eq!(rig.in_memory(), [B::GrandPiano, B::Strings, B::Organ]);
    rig.send(Command::ShelfSelect {
        instrument: Some(InstrumentKind::SynthLead),
    });
    assert_eq!(rig.in_memory(), [B::GrandPiano, B::Organ]);
    assert_eq!(rig.seats.sounding(), [AUDITION_SEAT], "a computed one");
    // Entered as a track; the shelf closes: only what the song sounds with stays.
    rig.send(Command::ShelfSelect {
        instrument: Some(InstrumentKind::Organ),
    });
    rig.send(Command::ShelfConfirm);
    assert_eq!(rig.seats.sounding(), [1]);
    rig.send(Command::ToggleBand { band: Band::Shelf });
    assert_eq!(rig.in_memory(), [B::GrandPiano, B::Organ]);
    rig.send(Command::Undo);
    assert_eq!(rig.in_memory(), [B::GrandPiano]);
    assert_eq!(rig.seats.sounding(), [0]);

    // The samples of an instrument that left go when the last instrument made with them does.
    let Samples::Ready(piano) = rig.seats.audio.sounds.samples(B::GrandPiano) else {
        panic!("the piano is read");
    };
    assert_eq!(
        Arc::strong_count(&piano),
        3,
        "the library, the seat and here"
    );
    rig.send(Command::DeleteTrack { track: 1 });
    assert_eq!(rig.in_memory(), [] as [B; 0]);
    assert_eq!(
        Arc::strong_count(&piano),
        1,
        "the instrument came back and was dropped"
    );
}

#[test]
fn a_song_from_a_pc_with_more_sounds_plays_as_the_sine_wave() {
    use BuiltinInstrument as B;
    let mut rig = SoundRig::new("missing", &[B::Strings]);
    rig.handle(Input::Tick);
    assert_eq!(
        rig.seats.sounding(),
        [0],
        "the piano this pack does not have"
    );
    assert!(rig.read.try_recv().is_err(), "nothing was read for it");
    let view = rig.seats.model.view();
    assert_eq!(view.song.tracks[0].instrument(), InstrumentKind::GrandPiano);
    assert!(!view.shelf.available.contains(&InstrumentKind::GrandPiano));
    assert!(view.shelf.available.contains(&InstrumentKind::Strings));
}

#[test]
fn the_core_reads_the_sounds_off_its_thread_and_says_when_a_row_is_ready() {
    use BuiltinInstrument as B;
    let dir = crate::sounds::tests::tiny_pack("core", &[B::GrandPiano, B::StudioKit]);
    std::fs::write(dir.join("studioKit/instrument.json"), "not json").unwrap();
    let core = Core::start(CoreOptions {
        sounds_dir: Some(dir.clone()),
        pack: None,
        ..CoreOptions::offline()
    });
    let shelf = core.snapshot().state.shelf;
    assert_eq!(shelf.available[0], InstrumentKind::GrandPiano);
    assert!(shelf.available.contains(&InstrumentKind::StudioKit));
    assert_eq!(shelf.available.len(), 7);

    core.send(Command::OpenShelf {
        mode: ShelfMode::NewTrack,
    });
    core.send(Command::ShelfSelect {
        instrument: Some(InstrumentKind::GrandPiano),
    });
    let until = Instant::now() + Duration::from_secs(10);
    while core.snapshot().state.shelf.loading().is_some() {
        assert!(Instant::now() < until, "the piano was never read");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        core.snapshot().state.shelf.selected(),
        Some(InstrumentKind::GrandPiano)
    );

    // Samples that do not read: the row is put away and the instrument leaves the shelf.
    core.send(Command::ShelfSelect {
        instrument: Some(InstrumentKind::StudioKit),
    });
    while core.snapshot().state.shelf.available.len() == 7 {
        assert!(Instant::now() < until, "the kit never left the shelf");
        std::thread::sleep(Duration::from_millis(5));
    }
    let shelf = core.snapshot().state.shelf;
    assert!(!shelf.available.contains(&InstrumentKind::StudioKit));
    assert_eq!((shelf.selected(), shelf.loading()), (None, None));
    core.stop();
    let _ = std::fs::remove_dir_all(dir);
}

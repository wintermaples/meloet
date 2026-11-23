//! R2: the keys of the PC keyboard on the core thread.

use std::sync::mpsc::channel;

use meloet_engine::BlockTiming;
use meloet_protocol::{ServerMessage, StatusMessage, TYPED_KEY_VELOCITY, TYPED_KEYBOARD_ID};

use super::*;

/// Writes down what the engine plays on it.
struct Tape(Sender<InstrumentEvent>);

impl Instrument for Tape {
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

/// One block of the engine the core holds while no stream runs.
fn block(core: &Core) {
    core.peek(|worker| {
        let engine = worker.audio.engine.as_mut().expect("no stream runs");
        let now = monotonic_us();
        let timing = BlockTiming {
            callback_us: now,
            playback_us: now + 5_000,
        };
        engine.process(&mut [0.0; 512], timing);
    });
}

fn key(note: u8, down: bool, t_us: u64) -> Command {
    Command::TypedKey { note, down, t_us }
}

fn held(core: &Core) -> u128 {
    core.shared.held.now().keys
}

fn notes(screen: &Receiver<ServerMessage>) -> Vec<(bool, u8, u8, u64)> {
    (screen.try_iter())
        .filter_map(|m| match m {
            ServerMessage::Event(Event::Note {
                on,
                note,
                velocity,
                t_us,
            }) => Some((on, note, velocity, t_us)),
            _ => None,
        })
        .collect()
}

fn on(note: u8, velocity: u8) -> InstrumentEvent {
    InstrumentEvent::NoteOn { note, velocity }
}

#[test]
fn a_typed_key_reaches_the_sound_the_meter_and_the_screen_and_off_lets_go_of_it() {
    let core = Core::start(CoreOptions::offline());
    let (screen_tx, screen) = channel();
    core.subscribe(
        Box::new(move |m| screen_tx.send(m).is_ok()),
        Box::new(|_| true),
    );
    let (tape_tx, tape) = channel();
    core.peek(move |worker| {
        let tape = Box::new(Tape(tape_tx));
        assert!(worker.audio.handle.swap_instrument(0, tape).is_ok());
    });
    // As when a stream runs: keys go into the engine.
    core.shared.stream.open.store(true, Relaxed);

    let t = monotonic_us();
    core.send(key(50, true, t));
    assert_eq!((held(&core), notes(&screen)), (0, vec![]), "off: nothing");

    core.send(Command::TypedKeys { on: true });
    let keyboards = core.snapshot().state.keyboards;
    assert!(keyboards.typed_keys);
    assert_eq!(keyboards.active.as_deref(), Some(TYPED_KEYBOARD_ID));

    let t = monotonic_us();
    core.post(key(60, true, t - 3_000));
    core.post(key(64, true, t - 2_000));
    // The screen drops the key's repeats; one that comes anyway does not sound again.
    core.post(key(64, true, t - 1_500));
    core.post(key(64, false, t - 1_000));
    core.post(key(65, false, t - 900));
    // Sent: waits for what was posted before.
    core.send(key(200, true, t - 800));
    assert_eq!(held(&core), 1 << 60, "the meter");
    assert_eq!(
        core.shared.held.last_velocity.load(Relaxed),
        TYPED_KEY_VELOCITY
    );
    assert_eq!(
        notes(&screen),
        [
            (true, 60, TYPED_KEY_VELOCITY, t - 3_000),
            (true, 64, TYPED_KEY_VELOCITY, t - 2_000),
            (false, 64, 0, t - 1_000),
        ],
        "at once, at the screen's time"
    );
    block(&core);
    let v = TYPED_KEY_VELOCITY;
    assert_eq!(
        tape.try_iter().collect::<Vec<_>>(),
        [on(60, v), on(64, v), InstrumentEvent::NoteOff { note: 64 }],
        "the sound"
    );

    // The fixed velocity, as for a keyboard.
    core.send(Command::SetVelocityFixed {
        on: true,
        value: 30,
    });
    core.send(key(67, true, monotonic_us()));
    assert_eq!(notes(&screen)[0].2, 30);

    // Off lets go of every key it holds: the sound, the meter, the screen.
    core.send(Command::TypedKeys { on: false });
    assert_eq!(held(&core), 0);
    let let_go: Vec<_> = notes(&screen).iter().map(|n| (n.0, n.1)).collect();
    assert_eq!(let_go, [(false, 60), (false, 67)]);
    block(&core);
    assert_eq!(
        tape.try_iter().collect::<Vec<_>>(),
        [
            on(67, 30),
            InstrumentEvent::NoteOff { note: 60 },
            InstrumentEvent::NoteOff { note: 67 }
        ]
    );
    let state = core.snapshot().state;
    assert!(!state.keyboards.typed_keys);
    assert!(state.keyboards.list.is_empty());
    assert_ne!(state.status.message, Some(StatusMessage::TypedKeysOn));
    assert!(
        core.shared.typed_played.load(Relaxed),
        "the MIDI thread says its keyboard"
    );
    core.send(key(60, false, monotonic_us()));
    core.send(key(61, true, monotonic_us()));
    assert_eq!((held(&core), notes(&screen)), (0, vec![]), "off: nothing");
    core.stop();
}

fn shared(fixed_velocity: u8) -> Arc<Shared> {
    Arc::new(Shared {
        held: Held::default(),
        fixed_velocity: AtomicU8::new(fixed_velocity),
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
    })
}

/// The way of `Worker::typed_key` without the core thread, into an engine that records: as
/// `a_fixed_velocity_reaches_the_sound_the_recording_the_meter_and_the_screen` for a keyboard.
#[test]
fn a_typed_key_is_recorded_as_it_sounds_like_a_keyboards() {
    let hub = Arc::new(Hub::new(Model::new(Settings::default()).view()));
    let (screen_tx, screen) = channel();
    hub.subscribe(
        Box::new(move |m| screen_tx.send(m).is_ok()),
        Box::new(|_| true),
    );
    let shared = shared(0);
    shared.stream.open.store(true, Relaxed);
    let (tx, _rx) = mpsc::channel();
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
    let mut hands = typed::TypedHands::default();
    for (note, down, t_us) in [(60, true, 1_002_000), (60, false, 1_003_000)] {
        let event = hands.key(note, down, 0).expect("a change");
        let t_us = hands.time(t_us, 1_004_000);
        play_key(event, t_us, &shared, parts.handle.typed_in(), &hub, &tx);
    }
    block(&mut parts.engine, 1_005_333);

    let played: Vec<_> = tape.try_iter().collect();
    let v = TYPED_KEY_VELOCITY;
    assert_eq!(played, [on(60, v), InstrumentEvent::NoteOff { note: 60 }]);
    let recorded: Vec<_> = std::iter::from_fn(|| parts.handle.pop_recorded())
        .filter_map(|r| match r {
            Recorded::Event { event, t_us, .. } => Some((event, t_us)),
            _ => None,
        })
        .collect();
    assert_eq!(
        recorded,
        [(played[0], 1_002_000), (played[1], 1_003_000)],
        "recorded as played, at its time"
    );
    assert_eq!(shared.held.last_velocity.load(Relaxed), v, "the meter");
    let shown: Vec<_> = notes(&screen).iter().map(|n| (n.0, n.1, n.2)).collect();
    assert_eq!(shown, [(true, 60, v), (false, 60, 0)], "the screen");
}

#[test]
fn after_a_typed_key_the_keyboard_played_next_is_said_even_if_it_was_played_before() {
    let hub = Arc::new(Hub::new(Model::new(Settings::default()).view()));
    let shared = shared(0);
    let (tx, core) = mpsc::channel();
    let links = Links {
        hub,
        shared: shared.clone(),
        tx,
    };
    let parts = Engine::new(EngineConfig::default());
    let mut sink = links.midi_sink(parts.midi_in);
    let press = |note| MidiMessage::Event {
        keyboard: "a".into(),
        event: MidiEvent::NoteOn { note, velocity: 80 },
        t_us: 1_000,
    };
    let said = |core: &Receiver<Msg>| {
        (core.try_iter())
            .filter(|m| matches!(m, Msg::Input(Input::KeyboardPlayed { .. }, None)))
            .count()
    };
    sink(press(60));
    sink(press(62));
    assert_eq!(said(&core), 1, "once for the same keyboard");
    shared.typed_played.store(true, Relaxed);
    sink(press(64));
    sink(press(65));
    assert_eq!(said(&core), 1, "again after the PC keyboard, once");
    assert!(!shared.typed_played.load(Relaxed));
}
